//! console 属性是进程全局的:每个场景跑在一次性子进程里(整文件随上游随迁,
//! 环境变量改名 `SLTERM_*`)。
//!
//! 承载形态(13 篇开放问题 2,待裁决):M3.1 挂 lib_tests 内嵌模块
//! (`console.rs` 的 `#[cfg(test)] mod console_tests;`),lib_tests 测试 exe 同为
//! console 子系统,继承语义可观测;若裁决保留独立 `windows_console_startup`
//! target(11 篇),本文件随迁回 `startup/` 旁位并改挂 `#[path]`,用例本体不变。
//! TEST 常量与挂载点耦合,迁挂时同步改。

use std::env;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use windows_sys::Win32::System::Console::{
    AllocConsole, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
};

const TEST: &str = "platform::startup::console::console_tests::gui_startup_restores_child_ctrl_c";

struct TestChild(Child);

impl Drop for TestChild {
    fn drop(&mut self) {
        // 仅失败清理:被 kill 的子进程不算一次中断。
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

struct ReadyFile(std::path::PathBuf);

impl Drop for ReadyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn wait(child: &mut TestChild) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "console test child exceeded its deadline"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn gui_startup_restores_child_ctrl_c() {
    let Ok(role) = env::var("SLTERM_CONSOLE_TEST_ROLE") else {
        for scenario in ["unchanged", "normalized", "detached"] {
            let mut child = TestChild(
                Command::new(env::current_exe().unwrap())
                    .args(["--exact", TEST, "--nocapture"])
                    .env("SLTERM_CONSOLE_TEST_ROLE", "host")
                    .env("SLTERM_CONSOLE_TEST_SCENARIO", scenario)
                    .spawn()
                    .unwrap(),
            );
            assert!(
                wait(&mut child).success(),
                "GUI console startup failed ({scenario})"
            );
        }
        return;
    };
    if role == "target" {
        // 本测试可执行体同为 console 子系统。此处永不 attach/alloc:
        // 两者都会重置继承来的标志位。
        let ready = env::var_os("SLTERM_CONSOLE_TEST_READY").unwrap();
        std::fs::write(ready, b"ready").unwrap();
        thread::sleep(Duration::from_secs(5));
        return;
    }

    // 永不向 cargo 或开发者的 console 广播。
    unsafe { FreeConsole() };
    assert_ne!(unsafe { AllocConsole() }, 0);
    assert_ne!(unsafe { SetConsoleCtrlHandler(None, 1) }, 0);
    let scenario = env::var("SLTERM_CONSOLE_TEST_SCENARIO").unwrap();
    let detached = scenario == "detached";
    if detached {
        assert_ne!(unsafe { FreeConsole() }, 0);
    }
    if scenario != "unchanged" {
        super::prepare_console_for_gui().unwrap();
    }
    if detached {
        // API 对 Explorer/脱壳启动也必须成立。
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let ready_file = ReadyFile(
        env::temp_dir().join(format!("slterm-ctrl-ready-{}-{stamp}", std::process::id())),
    );
    let ready = &ready_file.0;
    let mut target = TestChild(
        Command::new(env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env("SLTERM_CONSOLE_TEST_ROLE", "target")
            .env("SLTERM_CONSOLE_TEST_READY", ready)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready.exists() {
        assert!(Instant::now() < deadline, "target did not become ready");
        thread::sleep(Duration::from_millis(20));
    }
    // 只在子进程继承完生产启动态之后,给一次性驱动进程自身加盾;
    // 这改变不了已存在子进程的状态。
    assert_ne!(unsafe { SetConsoleCtrlHandler(None, 1) }, 0);
    let started = Instant::now();
    assert_ne!(unsafe { GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) }, 0);
    let status = wait(&mut target);
    if scenario == "unchanged" {
        // 阴性对照:继承来的 ignore 标志必须吞掉 Ctrl+C——没有这一条,
        // 子进程里的一次重置会让两种情形同时变绿。
        assert!(
            status.success(),
            "unfixed child unexpectedly handled Ctrl+C"
        );
        assert!(started.elapsed() >= Duration::from_secs(4));
    } else {
        assert_eq!(status.code().map(|code| code as u32), Some(0xc000013a));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "child did not interrupt promptly"
        );
    }
}
