//! Runtime API 行为级端到端。
//!
//! 条件跳过:需要真实桌面会话 + 预构建的 `slterm.exe`(经 `SLTERM_E2E_EXE`
//! 显式指定;无桌面会话或无产物时打印 SKIP 并计通过)。真机串行:
//! `SLTERM_E2E_EXE=target/debug/slterm.exe cargo test -p slterm --test runtime_conformance -- --test-threads=1`
//!
//! 覆盖:describe → snapshot → pane.prompt(提交后基线)→ pane.wait 全链;
//! 以及单实例移交——第二实例 ATTACH + tab.new 后退场,首实例多出一个 tab。

#![cfg(windows)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const PROTOCOL: &str = "slterm.runtime";
const VERSION: u16 = 1;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const HANDOVER_TIMEOUT: Duration = Duration::from_secs(20);

/// 不允许测试失败时残留 GUI 进程。
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SLTERM_E2E_EXE") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    None
}

fn unique_config_dir() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("slterm-conformance-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create isolated config dir");
    dir
}

fn spawn_app(exe: &Path, config_dir: &Path) -> KillOnDrop {
    let child = Command::new(exe)
        .env("SLTERM_CONFIG_DIR", config_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn slterm");
    KillOnDrop(child)
}

fn wait_for_port(config_dir: &Path, deadline: Instant) -> (u16, String) {
    let path = config_dir.join("runtime.port");
    loop {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            let mut parts = contents.split_whitespace();
            if let (Some(port), Some(token)) = (parts.next(), parts.next())
                && let Ok(port) = port.parse::<u16>()
                && port != 0
            {
                return (port, token.to_owned());
            }
        }
        assert!(
            Instant::now() < deadline,
            "runtime.port 未在期限内出现: {path:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn request(port: u16, token: &str, id: u64, method: &str, params: Value) -> Value {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect runtime");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("write timeout");
    let envelope = json!({
        "protocol": PROTOCOL,
        "version": VERSION,
        "id": format!("conformance-{id}"),
        "token": token,
        "method": method,
        "params": params,
    });
    serde_json::to_writer(&mut stream, &envelope).expect("write request");
    stream.write_all(b"\n").expect("newline");
    stream.flush().expect("flush");
    stream
        .shutdown(std::net::Shutdown::Write)
        .expect("shutdown write side");
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .expect("read response");
    serde_json::from_str(&line).expect("response JSON")
}

fn ok_result(response: Value) -> Value {
    assert_eq!(response["ok"], true, "runtime failed: {response}");
    response["result"].clone()
}

fn first_pane_id(snapshot: &Value) -> Option<u64> {
    snapshot["windows"]
        .as_array()?
        .iter()
        .flat_map(|window| window["tabs"].as_array().into_iter().flatten())
        .flat_map(|tab| tab["panes"].as_array().into_iter().flatten())
        .find_map(|pane| pane["id"].as_u64())
}

fn tab_count(snapshot: &Value) -> usize {
    snapshot["windows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|window| window["tabs"].as_array().map_or(0, Vec::len))
        .sum()
}

#[test]
fn runtime_api_conformance_and_second_instance_handover() {
    let Some(exe) = executable() else {
        eprintln!("SKIP runtime_conformance: 未设置 SLTERM_E2E_EXE(需要真实桌面与预构建 exe)");
        return;
    };
    let config_dir = unique_config_dir();
    let mut first = spawn_app(&exe, &config_dir);
    let mut next_id = 1_u64;

    // 1) 端点发现 + describe。
    let (port, token) = wait_for_port(&config_dir, Instant::now() + STARTUP_TIMEOUT);
    let describe = ok_result(request(
        port,
        &token,
        next_id,
        "runtime.describe",
        json!({}),
    ));
    next_id += 1;
    assert_eq!(describe["protocol"], PROTOCOL);
    assert_eq!(describe["protocol_version"], VERSION);

    // 2) snapshot:等首个窗口投影出现(渲染入账)。
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let (pane_id, before_seq, before_tabs) = loop {
        let snapshot = ok_result(request(
            port,
            &token,
            next_id,
            "runtime.snapshot",
            json!({}),
        ));
        next_id += 1;
        if let Some(pane_id) = first_pane_id(&snapshot) {
            let seq = snapshot["windows"][0]["tabs"][0]["panes"][0]["state_change_seq"]
                .as_u64()
                .unwrap_or(0);
            break (pane_id, seq, tab_count(&snapshot));
        }
        assert!(Instant::now() < deadline, "窗口快照未在期限内就绪");
        std::thread::sleep(Duration::from_millis(200));
    };

    // 3) pane.prompt:成功回包携带提交后快照与 state_change_seq。
    let prompt = ok_result(request(
        port,
        &token,
        next_id,
        "pane.prompt",
        json!({
            "pane_id": pane_id,
            "text": "echo slterm-conformance",
            "submit": true
        }),
    ));
    next_id += 1;
    assert_eq!(prompt["action"]["pane_id"], pane_id);
    let after_seq = prompt["snapshot"]["windows"][0]["tabs"][0]["panes"][0]["state_change_seq"]
        .as_u64()
        .expect("post-submit baseline");
    assert!(after_seq > 0, "提交后基线必须非零");
    assert_eq!(
        prompt["snapshot"]["windows"][0]["tabs"][0]["panes"][0]["task_state"], "running",
        "提交同步建立 running 边沿"
    );

    // 4) pane.wait:以提交前基线与 20s 预算等 running(提交后状态已命中)。
    let waited = ok_result(request(
        port,
        &token,
        next_id,
        "pane.wait",
        json!({
            "pane_id": pane_id,
            "state": "running",
            "timeout_ms": 20_000,
            "after_seq": before_seq
        }),
    ));
    next_id += 1;
    assert_eq!(
        waited["windows"][0]["tabs"][0]["panes"][0]["task_state"],
        "running"
    );

    // 5) 第二实例:owner 锁不可得 → ATTACH + tab.new 移交后退场(零窗口)。
    let handover_started = Instant::now();
    let mut second = spawn_app(&exe, &config_dir);
    let status = loop {
        if let Some(status) = second.0.try_wait().expect("poll second instance") {
            break status;
        }
        assert!(
            handover_started.elapsed() < HANDOVER_TIMEOUT,
            "第二实例未在期限内移交退场"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(status.success(), "移交成功 = 第二实例正常退出: {status}");

    // 6) 首实例仍活着,且移交链新建了一个 tab。
    let after = ok_result(request(
        port,
        &token,
        next_id,
        "runtime.snapshot",
        json!({}),
    ));
    assert_eq!(
        tab_count(&after),
        before_tabs + 1,
        "ATTACH + tab.new 必须在首实例新建一个 tab"
    );
    assert!(
        first.0.try_wait().expect("poll first instance").is_none(),
        "首实例不得随移交退出"
    );

    drop(first);
    let _ = std::fs::remove_dir_all(&config_dir);
}
