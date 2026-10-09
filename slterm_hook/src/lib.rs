//! slterm_hook —— AI-CLI 生命周期 hook 与 slTerminal 之间的小桥。
//!
//! claude(`Stop` / `Notification` / `UserPromptSubmit` hooks)、kimi
//! (`config.toml` 的 `[[hooks]]` command,事件 JSON 同 claude 形态走 stdin)、
//! codex(`notify` 程序)、pi 与 opencode(随宿主落盘的扩展/插件,在
//! `session.idle` / `permission.updated` / user-prompt 时 shell out)每个回合
//! 事件都调用本进程:把原始载荷经命名管道转发给宿主 slTerminal 实例后退出。
//!
//! 设计约束,按序:
//!
//! 1. 隐形:Stop hook 的退出码对 claude 有意义(非零弹错误横幅,2 甚至阻断
//!    回合),kimi 的 `Stop` 同样是可阻断事件。每条路径——含 panic——都必须
//!    exit 0、快速。claude 与 kimi 还把载荷写到我们的 stdin,这些模式即便消息
//!    无处可去,也得在有界调用内抽干 stdin。调用方留着 stdin 不放、或接收方
//!    停滞,都不能让本 helper 无限挂着、占住已安装的可执行文件。
//! 2. 作用域:hook 配置是全局的(settings.json / kimi 的 config.toml),但
//!    效果必须只限 slTerminal。作用域闸门是环境:`SLTERM_NOTIFY_PIPE` 只对
//!    slTerminal 内 spawn 的进程存在;别处一律不转发、无声退出,不影响调用方。
//! 3. 有界:纯 std,不碰 JSON(宿主解析),一次管道写。转发有截止时间;
//!    启动与通知延迟取决于宿主。
//!
//! 用法(由宿主安装权威写进各家 CLI 配置,安装器链路归 app 侧 M4):
//! ```text
//! slterm-hook claude                              # stdin 载荷
//! slterm-hook kimi                                # stdin 载荷
//! slterm-hook codex <json>                        # 载荷在末位参数
//! slterm-hook codex --hooks=full                  # native hooks,stdin
//! slterm-hook codex --chain <exe> <fixed…> <json> # 附带 exec 原 notifier
//! slterm-hook opencode <json>                     # 载荷在末位参数
//! slterm-hook pi <json>                           # 载荷在末位参数
//! ```
//! `--chain` 存在的理由:codex 只有一个 `notify` 槽位,可能已被占(如 OpenAI
//! 自家 computer-use notifier)——我们先转发给 slTerminal,再以同载荷调用原程序。
//!
//! `opencode` 由宿主侧落盘的插件喂给:插件订阅 opencode 事件总线并 shell out
//! 到这里,载荷为归一化 `{"kind":...}` —— 与 codex 同线形。

use std::io::{Read, Write};

const MAX_PAYLOAD_BYTES: usize = 1 << 20;
const FORWARD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// 其他 agent 的 hook runner 独有的环境变量。
///
/// 这些 CLI 会主动读取并信任 `~/.claude/settings.json`,于是我们装在那里的
/// claude hook 也会在**它们的**事件上被触发。把那些事件当 claude 上报有两个
/// 后果:pane 贴错 provider 身份,以及把别家的 session id 交给
/// `claude --resume` —— 一个根本不存在的会话。每个 runner 都会向 hook 进程
/// 导出自己独有的变量,这是唯一可靠的判据。
///
/// 这道门是承重的,不是保险。它也是会**静默失效**的那一类:变量一旦被上游
/// 改名或停止导出,门就永远不再命中,而且没有任何报错。宿主侧因此不把它当
/// 唯一防线——载荷形状校验在 app 侧信封解析(M4.2)里独立拦一次,
/// `foreign_runner_payload_is_rejected_even_if_the_env_gate_fails` 就是钉住这
/// 个前提的测试。新增 provider 时在这里加一行。
const FOREIGN_HOOK_RUNNERS: &[&str] = &[
    // Grok Build 的 hook runner 注入(形如 "user:stop[0].hooks[0]")。它替代
    // 了早期的 GROK_SESSION_ID:后者已不再导出,只在被插值进 hook 命令时才
    // 解析得到——如果当初的门写在那个变量上,今天就是一扇不会响的门。
    "GROK_HOOK_NAME",
    "GROK_HOOK_EVENT",
];

/// 当前进程是否由别家 agent 的 hook runner 启动。
fn foreign_hook_runner() -> Option<&'static str> {
    foreign_hook_runner_from(|name| std::env::var_os(name))
}

fn foreign_hook_runner_from(
    mut read: impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> Option<&'static str> {
    FOREIGN_HOOK_RUNNERS
        .iter()
        .copied()
        .find(|name| read(name).is_some_and(|value| !value.is_empty()))
}

/// 单名环境读取:只认 `SLTERM_<suffix>`。上游的双名并写兼容层不迁——生产
/// 环境变量一律单名 `SLTERM_*` 是既定裁决,本仓无历史用户要兼容。
fn helper_env(name: &str) -> Option<std::ffi::OsString> {
    helper_env_from(name, |key| std::env::var_os(key))
}

fn helper_env_from(
    name: &str,
    mut read: impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> Option<std::ffi::OsString> {
    read(&format!("SLTERM_{name}"))
}

/// 一次调用的去向。写进侧信道日志,用来回答「通知为什么没出现」。
enum Outcome {
    PayloadTooLarge,
    /// 写进了本地命名管道。
    Sent,
    /// 不在 slTerminal 里运行:没有管道闸门。这是最常见的一种,也是设计如此
    /// (约束 2)。
    NotHosted,
    /// 管道存在但连不上(服务端正在换实例,或宿主已退出)。
    PipeUnavailable,
    /// 调用方是别家 agent 的 hook runner,串台门闭合。
    ForeignRunner,
}

impl Outcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::PayloadTooLarge => "payload-too-large",
            Self::Sent => "sent",
            Self::NotHosted => "not-hosted",
            Self::PipeUnavailable => "pipe-unavailable",
            Self::ForeignRunner => "foreign-runner",
        }
    }
}

/// JSON 字符串转义。`pane` 来自环境变量,任何进程都能把它设成带引号或控制字符
/// 的内容;不转义就会写出一行坏掉的 NDJSON。
fn escape_json(raw: &str, out: &mut String) {
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}

fn outcome_line(source: &str, pane: &str, bytes: usize, outcome: &Outcome, at_ms: u128) -> String {
    let mut line = String::with_capacity(160);
    line.push_str("{\"at_ms\":");
    line.push_str(&at_ms.to_string());
    line.push_str(",\"source\":\"");
    escape_json(source, &mut line);
    line.push_str("\",\"pane\":\"");
    escape_json(pane, &mut line);
    line.push_str("\",\"payload_bytes\":");
    line.push_str(&bytes.to_string());
    line.push_str(",\"outcome\":\"");
    line.push_str(outcome.as_str());
    line.push_str("\"}\n");
    line
}

/// 结构化侧信道。默认完全关闭:只有 `SLTERM_HOOK_LOG` 指向一个可追加的文件时
/// 才写,每次一行 NDJSON。
///
/// 为什么需要它:这个进程的所有失败都被有意吞掉(约束 1——退出码对调用方的
/// CLI 有意义),于是「完成通知没出现」这类问题事后完全无从取证。只记路由事实
/// 和去向,**绝不记载荷内容**:payload 里有 cwd、工具参数,甚至选区正文。
fn log_outcome(source: &str, pane: &str, bytes: usize, outcome: &Outcome) {
    let Some(path) = helper_env("HOOK_LOG").filter(|value| !value.is_empty()) else {
        return;
    };
    let at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let line = outcome_line(source, pane, bytes, outcome, at_ms);
    // 追加打开 + 一次写入。失败一律忽略:日志绝不能影响 agent。
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// 进程入口(约束 1 的执行面):任何失败绝不漏给调用方 CLI。bin 薄壳的唯一
/// 调用点。
pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let forwarding_args = args.clone();
    let (done, finished) = std::sync::mpsc::sync_channel(1);
    // 给整个转发操作(含 stdin 抽干与管道写)上界:只给读加超时仍会留下一个
    // 卡住的写线程。一次性进程永不 join 卡死的 worker:main 返回即回收其全部
    // 线程与句柄,provider 看到的退出码恒为 0。
    if std::thread::Builder::new()
        .name("hook-forward".into())
        .spawn(move || {
            let _ = std::panic::catch_unwind(|| run(&forwarding_args));
            let _ = done.send(());
        })
        .is_ok()
    {
        let _ = finished.recv_timeout(FORWARD_TIMEOUT);
    }
    // 用户自己的 notifier 与我们尽力而为的传输无关:永不等待,slTerminal 管道
    // 不可用时也不压制。
    chain_notifier(&args);
    // cursor 的提交前 hook 有响应合同;传输失败也不能阻止用户提交。
    if args.first().is_some_and(|source| source == "cursor")
        && native_event(&args) == Some("prompt")
    {
        let _ = std::io::stdout().lock().write_all(b"{\"continue\":true}\n");
    }
}

fn read_payload(mut reader: impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::with_capacity(4096);
    reader
        .by_ref()
        .take((MAX_PAYLOAD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        std::io::copy(&mut reader, &mut std::io::sink())?;
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}

/// 已知调用方白名单。不在名单里的第一参数视为误调用:约束 2 要求在 slTerminal
/// 之外也必须是无声 no-op。新增 provider 时同步在 `payload_on_stdin` 声明载荷通道。
fn known_source(source: &str) -> bool {
    matches!(
        source,
        "claude" | "codex" | "opencode" | "pi" | "kimi" | "omp" | "copilot" | "grok" | "cursor"
    )
}

/// 载荷通道。claude 与 kimi 都把事件 JSON 写到我们的 stdin(kimi 的 `[[hooks]]`
/// command 与 claude 的 hook 同形态,事件经 stdin 传入);走 stdin 的 source 必须
/// 无论如何都抽干管道(约束 1:未读的管道会在 CLI 侧变成 hook write error)。
/// 其余 CLI 把载荷追加为末位参数。
fn payload_on_stdin(source: &str) -> bool {
    matches!(source, "claude" | "kimi" | "copilot" | "grok" | "cursor")
}

fn native_event(args: &[String]) -> Option<&str> {
    if args.len() != 3
        || !matches!(args[0].as_str(), "copilot" | "grok" | "cursor")
        || args[1] != "--event"
    {
        return None;
    }
    let event = args[2].as_str();
    matches!(
        event,
        "session-start"
            | "session-end"
            | "prompt"
            | "tool-complete"
            | "done"
            | "failed"
            | "error"
            | "notification"
    )
    .then_some(event)
}

/// 侧信道信封:一行 `slterm-hook/1 source=<s> pane=<p>` 头加原始载荷。helper 不
/// 重编码,宿主侧按 source 路由到对应 provider 的解析分支。
fn envelope(source: &str, pane: &str, contract: &str, payload: &[u8]) -> Vec<u8> {
    let mut message = format!("slterm-hook/1 source={source} pane={pane}{contract}\n").into_bytes();
    message.extend_from_slice(payload);
    message
}

fn run(args: &[String]) {
    let Some(source) = args.first().filter(|s| known_source(s)) else {
        return;
    };

    // native hooks 走 stdin;legacy notify 桥把 JSON 追加为 argv。
    let native_codex = source == "codex"
        && args
            .get(1)
            .is_some_and(|arg| matches!(arg.as_str(), "--hooks=turns" | "--hooks=full"));
    let payload = if payload_on_stdin(source) || native_codex {
        match read_payload(std::io::stdin().lock()) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                log_outcome(source, "", MAX_PAYLOAD_BYTES + 1, &Outcome::PayloadTooLarge);
                return;
            }
            Err(_) => return,
        }
    } else {
        args.last().cloned().unwrap_or_default().into_bytes()
    };

    // 串台门。放在读完 stdin 之后:约束 1 要求 stdin 模式的 source 无论如何都把
    // stdin 抽干(未读的管道会在 CLI 侧变成 hook write error),所以先读再退。
    // Grok 同时读取 claude 与 cursor 配置;这些借用的入口不能认领 Grok 会话。
    if matches!(source.as_str(), "claude" | "cursor") && foreign_hook_runner().is_some() {
        log_outcome(source, "", payload.len(), &Outcome::ForeignRunner);
        return;
    }

    let pane = helper_env("PANE_ID")
        .and_then(|value| value.into_string().ok())
        .unwrap_or_default();
    let contract = if native_codex {
        format!(" codex_hooks={}", args[1].strip_prefix("--hooks=").unwrap())
    } else if matches!(source.as_str(), "copilot" | "grok" | "cursor") {
        let Some(event) = native_event(args) else {
            return;
        };
        format!(" event={event}")
    } else {
        String::new()
    };
    let message = envelope(source, &pane, &contract, &payload);

    // 本地 pane 走命名管道直写。上游的远端 OSC 777 臂(REMOTE_HOOK_TOKEN +
    // base64)随 SSH 语义整支裁剪,不迁。
    let mut outcome = Outcome::NotHosted;
    if let Some(pipe) = helper_env("NOTIFY_PIPE") {
        // 服务端一次只接受一个连接、间隙重建管道实例,竞态 connect 会失败几个
        // 微秒。短暂重试后静默放弃:通知是尽力而为。
        outcome = Outcome::PipeUnavailable;
        for _ in 0..20 {
            match send_local(&pipe, &message) {
                Ok(()) => {
                    outcome = Outcome::Sent;
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        }
    }
    log_outcome(source, &pane, payload.len(), &outcome);
}

/// 命名管道直写:Windows 命名管道以文件路径形态打开,纯 std 即达,无平台分支
/// 残留(unix socket 臂随 unix 支持整支裁剪,本 crate 仅面向 Win10/11)。
fn send_local(endpoint: &std::ffi::OsStr, message: &[u8]) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(endpoint)?
        .write_all(message)
}

fn chain_notifier(args: &[String]) {
    // 链模式:让用户预存的 codex notifier 继续工作。即便在 slTerminal 之外也
    // 要执行——原程序必须在各处照常触发。
    let strs: Vec<&str> = args.iter().map(String::as_str).collect();
    if let ["codex", "--chain", prog, rest @ ..] = &strs[..]
        && !rest.is_empty()
    {
        let (fixed, json) = rest.split_at(rest.len() - 1);
        let _ = std::process::Command::new(prog)
            .args(fixed)
            .args(json)
            .spawn();
    }
}

#[cfg(test)]
mod payload_tests {
    #[test]
    fn oversized_stdin_is_drained_but_never_forwarded_as_truncated_json() {
        let bytes = vec![b'x'; super::MAX_PAYLOAD_BYTES * 2];
        let mut input = std::io::Cursor::new(&bytes);
        assert!(super::read_payload(&mut input).unwrap().is_none());
        assert_eq!(input.position(), bytes.len() as u64);
        assert_eq!(
            super::read_payload(&b"{\"answer\":\"ok\"}"[..])
                .unwrap()
                .unwrap(),
            b"{\"answer\":\"ok\"}"
        );
    }
}

#[cfg(test)]
mod routing_tests {
    /// kimi 与 claude 共用 stdin 载荷通道,但信封头签自己的 source——宿主侧
    /// 靠这个字段把载荷路由给 kimi 的解析分支,签错名字会被静默丢弃。
    #[test]
    fn kimi_reads_stdin_and_signs_its_own_envelope() {
        for source in ["claude", "codex", "opencode", "pi", "kimi"] {
            assert!(super::known_source(source), "{source} 必须仍在白名单里");
        }
        assert!(
            !super::known_source("kimi-code"),
            "白名单只认精确的 source 名"
        );
        assert!(super::payload_on_stdin("claude"));
        assert!(super::payload_on_stdin("kimi"));
        for source in ["codex", "opencode", "pi"] {
            assert!(
                !super::payload_on_stdin(source),
                "{source} 的载荷在末位参数"
            );
        }

        let stdin_json: &[u8] =
            br#"{"hook_event_name":"Stop","session_id":"s1","client_type":"kimi_code_cli"}"#;
        let payload = super::read_payload(stdin_json).unwrap().unwrap();
        assert_eq!(payload, stdin_json);

        let message = super::envelope("kimi", "11", "", &payload);
        let header: &[u8] = b"slterm-hook/1 source=kimi pane=11\n";
        assert!(message.starts_with(header));
        assert_eq!(&message[header.len()..], stdin_json);
    }

    #[test]
    fn native_event_contract_accepts_only_known_sources_and_single_header_fields() {
        let args = |source: &str, event: &str| vec![source.into(), "--event".into(), event.into()];
        for source in ["copilot", "grok", "cursor"] {
            assert!(super::known_source(source) && super::payload_on_stdin(source));
            assert_eq!(super::native_event(&args(source, "prompt")), Some("prompt"));
            assert_eq!(super::native_event(&args(source, "done\npane=9")), None);
            assert_eq!(super::native_event(&args(source, "unknown")), None);
        }
        assert_eq!(super::native_event(&args("kimi", "done")), None);
        assert!(super::known_source("omp") && !super::payload_on_stdin("omp"));
    }
}

#[cfg(test)]
mod gate_tests {
    use super::{FOREIGN_HOOK_RUNNERS, foreign_hook_runner_from};
    use std::ffi::OsString;

    /// 这道门是承重的:命中任一别家 runner 的变量就必须闭合。空值不算命中——
    /// 有些 runner 会把变量导出成空串。
    ///
    /// 读取函数注入式测试:进程环境是全局状态,注入读取闭包后本族用例零环境
    /// 变量改动,与并行执行天然兼容(不依赖串行纪律)。
    #[test]
    fn foreign_runner_gate_closes_on_a_non_empty_marker() {
        assert!(
            !FOREIGN_HOOK_RUNNERS.is_empty(),
            "至少要保留一个已知 runner 判据"
        );
        let marker = FOREIGN_HOOK_RUNNERS[0];
        assert_eq!(foreign_hook_runner_from(|_| None), None);
        assert_eq!(
            foreign_hook_runner_from(|name| (name == marker).then(OsString::new)),
            None,
            "空值不足以证明调用方是别家 runner"
        );
        assert_eq!(
            foreign_hook_runner_from(
                |name| (name == marker).then(|| "user:stop[0].hooks[0]".into())
            ),
            Some(marker)
        );
    }
}

#[cfg(test)]
mod helper_env_tests {
    use std::ffi::OsString;

    /// 单名裁决:只查一次、只查 `SLTERM_<suffix>` 精确名,不存在别名回退。
    /// 显式空值原样返回——「设成空」与「没设」语义不同,helper 不替调用方猜。
    #[test]
    fn helper_env_reads_only_the_slterm_scoped_name() {
        let mut queried = Vec::new();
        let read = |name: &str| {
            queried.push(name.to_string());
            (name == "SLTERM_NOTIFY_PIPE").then(|| OsString::from("pipe-1"))
        };
        assert_eq!(
            super::helper_env_from("NOTIFY_PIPE", read),
            Some("pipe-1".into())
        );
        assert_eq!(queried, ["SLTERM_NOTIFY_PIPE"]);

        assert_eq!(super::helper_env_from("PANE_ID", |_| None), None);
        assert_eq!(
            super::helper_env_from("PANE_ID", |_| Some(OsString::new())),
            Some(OsString::new())
        );
    }
}

#[cfg(test)]
mod side_channel_tests {
    use super::{Outcome, outcome_line};

    /// 侧信道每行必须是合法 NDJSON,而且不能夹带载荷内容。`pane` 来自环境
    /// 变量,是这行里唯一可被外部塞进引号和控制字符的字段。
    #[test]
    fn outcome_line_is_valid_ndjson_and_carries_no_payload() {
        let line = outcome_line("claude", "7", 1234, &Outcome::Sent, 1_700_000_000_123);
        assert_eq!(
            line,
            "{\"at_ms\":1700000000123,\"source\":\"claude\",\"pane\":\"7\",\"payload_bytes\":1234,\"outcome\":\"sent\"}\n"
        );

        let hostile = outcome_line(
            "claude",
            "7\",\"outcome\":\"sent\n",
            0,
            &Outcome::NotHosted,
            1,
        );
        assert!(
            hostile.ends_with("\"outcome\":\"not-hosted\"}\n"),
            "结论字段必须是最后一个"
        );
        // 注入的引号和换行都被转义,整行仍然只有一行。
        assert_eq!(hostile.matches('\n').count(), 1);
        assert!(hostile.contains("\\\"") && hostile.contains("\\n"));
    }
}

/// 真实进程生命周期回归:provider 可能留着 stdin 句柄不放,管道接收方也可能
/// 停止排水。两者都不能钉住已安装的 helper。
#[cfg(test)]
mod lifetime_tests {
    use std::io::Write as _;
    use std::process::{Child, Command, Output, Stdio};
    use std::time::{Duration, Instant};

    /// helper 可执行文件路径。`CARGO_BIN_EXE_*` 只在真正构建测试 target 时注入,
    /// check/clippy 模式缺席——`option_env!` 容忍缺席,回落到 cargo 布局事实
    /// (测试 exe 在 `target/debug/deps/`,bin 在其上一级的 `slterm-hook.exe`)。
    fn hook_exe() -> std::path::PathBuf {
        option_env!("CARGO_BIN_EXE_slterm-hook").map_or_else(
            || {
                let mut path = std::env::current_exe().unwrap();
                path.pop();
                path.pop();
                path.push("slterm-hook.exe");
                path
            },
            std::path::PathBuf::from,
        )
    }

    struct Invocation(Option<Child>);

    impl Invocation {
        fn spawn(args: &[&str], pipe: Option<&str>) -> Self {
            let mut command = Command::new(hook_exe());
            command
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            // 子进程环境显式清场:作用域闸门与侧信道开关不受外部 shell 环境污染,
            // 串台门变量同理(别家 runner 判据绝不能泄漏进被测进程)。
            for suffix in ["NOTIFY_PIPE", "PANE_ID", "HOOK_LOG"] {
                command.env_remove(format!("SLTERM_{suffix}"));
            }
            for name in super::FOREIGN_HOOK_RUNNERS {
                command.env_remove(name);
            }
            if let Some(pipe) = pipe {
                command.env("SLTERM_NOTIFY_PIPE", pipe);
            }
            Self(Some(command.spawn().unwrap()))
        }

        fn finish(mut self) -> Output {
            let deadline = Instant::now() + Duration::from_secs(6);
            let child = self.0.as_mut().unwrap();
            loop {
                if child.try_wait().unwrap().is_some() {
                    let output = self.0.take().unwrap().wait_with_output().unwrap();
                    assert!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    return output;
                }
                assert!(
                    Instant::now() < deadline,
                    "helper retained a blocked invocation"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    impl Drop for Invocation {
        fn drop(&mut self) {
            if let Some(child) = &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    #[test]
    fn stdin_kept_open_cannot_pin_a_native_hook_executable() {
        for args in [&["codex", "--hooks=full"][..], &["claude"][..]] {
            let mut invocation = Invocation::spawn(args, None);
            let mut input = invocation.0.as_mut().unwrap().stdin.take().unwrap();
            input.write_all(b"{\"hook_event_name\":\"Stop\"}").unwrap();
            // 全程持有 stdin 不关闭:helper 退出不得依赖 EOF(2s 转发截止兜底)。
            let output = invocation.finish();
            assert!(output.stdout.is_empty());
            drop(input);
        }
    }

    #[test]
    fn cursor_still_allows_submission_when_stdin_never_closes() {
        let mut invocation = Invocation::spawn(&["cursor", "--event", "prompt"], None);
        let input = invocation.0.as_mut().unwrap().stdin.take().unwrap();
        let output = invocation.finish();
        assert_eq!(output.stdout, b"{\"continue\":true}\n");
        drop(input);
    }

    #[test]
    fn ordinary_completed_payload_exits_silently() {
        let mut invocation = Invocation::spawn(&["codex", "--hooks=full"], None);
        invocation
            .0
            .as_mut()
            .unwrap()
            .stdin
            .take()
            .unwrap()
            .write_all(b"{}")
            .unwrap();
        let output = invocation.finish();
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }

    /// 非白名单首参数 = 误调用:约束 2 要求即便在宿主之外也是无声 no-op。
    #[test]
    fn unknown_source_invocation_is_a_silent_no_op() {
        let invocation = Invocation::spawn(&["not-a-cli", "{}"], None);
        let output = invocation.finish();
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }

    #[test]
    fn legacy_notify_still_invokes_the_users_chained_program() {
        let executable = std::env::current_exe().unwrap();
        let invocation = Invocation::spawn(
            &[
                "codex",
                "--chain",
                executable.to_str().unwrap(),
                "--list",
                "--format=terse",
            ],
            None,
        );
        let output = invocation.finish();
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("ordinary_completed_payload_exits_silently")
        );
    }

    #[test]
    fn pi_prompt_notification_exits_cleanly_without_a_host() {
        let invocation = Invocation::spawn(
            &["pi", r#"{"kind":"prompt","session_id":"s1","cwd":"/tmp"}"#],
            None,
        );
        let output = invocation.finish();
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }

    /// 服务端命名管道的一个真实传输可能遗留的状态:已创建、从不 accept、从不
    /// 排水。默认测试 harness 在单进程内跑全部用例,每个管道名必须唯一。
    ///
    /// `#[link]` 链接指令只有编译期形态,无运行时分支可改——这是「测试
    /// cfg(windows) 原则上改运行时 cfg!()」规则的例外位(FFI 边界)。
    #[cfg(windows)]
    mod stalled_receiver {
        use std::ffi::c_void;

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateNamedPipeW(
                name: *const u16,
                mode: u32,
                pipe_mode: u32,
                instances: u32,
                output: u32,
                input: u32,
                timeout: u32,
                security: *const c_void,
            ) -> *mut c_void;
            fn CloseHandle(handle: *mut c_void) -> i32;
            fn GetNamedPipeClientProcessId(handle: *mut c_void, pid: *mut u32) -> i32;
        }

        pub struct Receiver(*mut c_void);

        impl Receiver {
            /// mode 1 = PIPE_ACCESS_INBOUND,单实例:镜像真实传输的
            /// 「一次一个客户端」形态。
            pub fn create(discriminator: &str) -> (Self, String) {
                let name = format!(r"\\.\pipe\slterm-hook-lifetime-test-{discriminator}");
                let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                let handle = unsafe {
                    CreateNamedPipeW(wide.as_ptr(), 1, 0, 1, 4096, 4096, 0, std::ptr::null())
                };
                assert_ne!(handle as isize, -1, "{}", std::io::Error::last_os_error());
                (Self(handle), name)
            }

            /// 内核对「这个进程连上来过吗」的回答——与真实传输对每个已接受
            /// 客户端所做的身份探针同源。
            pub fn connected_client(&self) -> Option<u32> {
                let mut pid = 0;
                let ok = unsafe { GetNamedPipeClientProcessId(self.0, &mut pid) };
                (ok != 0 && pid != 0).then_some(pid)
            }
        }

        impl Drop for Receiver {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_receiver_that_never_reads_cannot_pin_the_helper() {
        let (receiver, name) = stalled_receiver::Receiver::create("codex-argv");
        let payload = "x".repeat(16 * 1024);
        let invocation = Invocation::spawn(&["codex", &payload], Some(&name));
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            if receiver
                .connected_client()
                .is_some_and(|pid| pid == invocation.0.as_ref().unwrap().id())
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "helper never connected to the blocked receiver"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = invocation.finish();
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }

    /// 上游 issue #259:pi 的扩展以分离 Windows 进程启动
    /// `slterm-hook pi <prompt-json>` 且永不回收,一阵 prompt 突发可能比宿主
    /// 活得久。填充后的 context 字段超出接收方缓冲,把第一个连接钉在写中途
    /// ——这正是转发截止要防的「写停滞」条件。
    #[cfg(windows)]
    #[test]
    fn concurrent_pi_prompt_notifications_exit_against_a_wedged_receiver() {
        let (receiver, name) = stalled_receiver::Receiver::create("pi-burst");
        let prompt = |session: u32| {
            format!(
                r#"{{"kind":"prompt","session_id":"s-{session}","bridge_instance":"5c1e","bridge_sequence":"{session}","event_id":"5c1e:{session}","cwd":"C:\\Users\\dev\\project","context":"{}"}}"#,
                "x".repeat(16 * 1024),
            )
        };
        let pending: Vec<_> = (1..=5)
            .map(|session| Invocation::spawn(&["pi", &prompt(session)], Some(&name)))
            .collect();
        // 楔死的接收方把第一个连上的 helper 钉在写中途,于是在截止期回收它
        // 之前,这条连接保持可观察。
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            if receiver.connected_client().is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "no pi prompt helper connected to the wedged receiver"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        for invocation in pending {
            let output = invocation.finish();
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
        }
    }
}
