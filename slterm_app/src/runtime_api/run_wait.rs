//! run 等待的分段限时与终端尾部证据(04 篇锚点)。
//!
//! `pane.run` 只认 OSC 133 真实 exit code。Enter 之后的短窗口内既没完成
//! 也没有 CommandStart、且屏幕不在产出,通常是有东西吃掉了输入;这时
//! 不该烧完整个超时。反之只要还在产出,就按原超时继续等——可能只是
//! shell 缺少 OSC 133 集成,命令本身跑得好。

use super::{
    ApiError, EventSink, MAX_READ_LINES, RUN_PROGRESS_PROBE_INTERVAL, RUN_PROGRESS_PROBE_LINES,
    RUN_START_GRACE, RuntimeCommand, RuntimeHub, RuntimeRunResult, TAIL_SCAN_EXTRA_LINES,
    transport,
};
use serde_json::{Value, json};
use std::time::Duration;

/// run 等待的分段限时。Enter 后的短窗口内既没完成也没有 CommandStart,
/// 且这一刻屏幕不在产出——那通常是有东西吃掉了输入(更新页、授权确认、
/// 任意抢走键盘的 TUI)。这种情况不该烧完整个超时;只要还在产出,就按
/// 原超时继续等(可能只是 shell 缺少 OSC 133 集成,命令本身在跑)。
pub(super) fn wait_run_phased(
    hub: &RuntimeHub,
    sink: &EventSink,
    window_id: u64,
    pane_id: u64,
    run_id: u64,
    timeout: Duration,
) -> Result<RuntimeRunResult, ApiError> {
    wait_run_phased_with_grace(
        hub,
        sink,
        window_id,
        pane_id,
        run_id,
        timeout,
        RUN_START_GRACE,
    )
}

pub(super) fn wait_run_phased_with_grace(
    hub: &RuntimeHub,
    sink: &EventSink,
    window_id: u64,
    pane_id: u64,
    run_id: u64,
    timeout: Duration,
    start_grace: Duration,
) -> Result<RuntimeRunResult, ApiError> {
    let grace = start_grace.min(timeout);
    let start_timeout = match hub.wait_run(window_id, pane_id, run_id, grace) {
        Err(error) if error.code == "run_start_timeout" => error,
        other => return other,
    };
    let remaining = timeout.saturating_sub(grace);
    if remaining.is_zero() {
        return Err(start_timeout);
    }
    let Some(tail) = quiet_pane_evidence(hub, sink, window_id, pane_id) else {
        return hub.wait_run(window_id, pane_id, run_id, remaining);
    };
    // 取样期间命令可能刚好收尾,完成结果已进缓存;宁可多查一次,也不要把
    // 已经成功的 run 报成没启动。
    if let Ok(result) = hub.wait_run(window_id, pane_id, run_id, Duration::ZERO) {
        return Ok(result);
    }
    Err(ApiError::new(
        "run_not_started",
        "the command was submitted but never reported CommandStart, and the pane is producing no \
         output; something may be holding the input. The command was not cancelled and the pane \
         was left untouched, so waiting again is safe.",
    )
    .details(json!({
        "window_id": window_id,
        "pane_id": pane_id,
        "run_id": run_id,
        "waited_ms": u64::try_from(grace.as_millis()).unwrap_or(u64::MAX),
        "remaining_ms": u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX),
        // 缺少 133 集成的 shell 里,静默的慢命令与被挡住看起来一样,
        // 因此这里只报证据,不宣称已经定性。
        "shell_integration": "unconfirmed",
        "tail": tail
    })))
}

/// 连续两次取样 pane 尾部;两次相同即认为此刻没有产出,返回现场。
fn quiet_pane_evidence(
    hub: &RuntimeHub,
    sink: &EventSink,
    window_id: u64,
    pane_id: u64,
) -> Option<String> {
    let first = read_pane_text(hub, sink, window_id, pane_id, RUN_PROGRESS_PROBE_LINES)?;
    std::thread::sleep(RUN_PROGRESS_PROBE_INTERVAL);
    let second = read_pane_text(hub, sink, window_id, pane_id, RUN_PROGRESS_PROBE_LINES)?;
    (first == second).then_some(second)
}

/// 读一次 pane 尾部文本。读取失败不升级成调用方错误——拿不到现场只是
/// 少一份证据,原始结论照旧。
fn read_pane_text(
    hub: &RuntimeHub,
    sink: &EventSink,
    window_id: u64,
    pane_id: u64,
    lines: usize,
) -> Option<String> {
    read_pane_tail_text(hub, sink, Some(window_id), pane_id, lines).map(|(text, _)| text)
}

/// 读 pane 的「最后 `lines` 个非空行」,并返回这次实际请求的行数。
///
/// `pane.read` 自身语义是「网格底部若干行」,内容不足一屏时会读到空白;
/// 做证据/静默比对需要真正看得见的内容,所以多扫一段再回退。`pane.read`
/// 的对外语义保持不变。
pub(crate) fn read_pane_tail_text(
    hub: &RuntimeHub,
    sink: &EventSink,
    window_id: Option<u64>,
    pane_id: u64,
    lines: usize,
) -> Option<(String, usize)> {
    let requested = lines
        .saturating_add(TAIL_SCAN_EXTRA_LINES)
        .min(MAX_READ_LINES);
    let result = transport::dispatch_runtime_command(
        RuntimeCommand::ReadPane {
            window_id,
            pane_id,
            lines: requested,
        },
        sink,
        hub,
    )
    .ok()?;
    let read = result.get("action").unwrap_or(&result);
    let text = read.get("text").and_then(Value::as_str)?;
    Some((last_non_empty_lines(text, lines), requested))
}

/// 从末尾去掉空白行,再取最后 `lines` 行。
fn last_non_empty_lines(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let end = all
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |index| index + 1);
    let start = end.saturating_sub(lines);
    all[start..end].join("\n")
}
