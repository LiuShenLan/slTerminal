//! slTerminal 可执行入口。
//!
//! console 子系统——从 pwsh/cmd 启动继承父控制台、标准流天然正确
//! (AI CLI agent 是 stdio 消费者);GUI 人格闪控制台窗为已接受后果,
//! 「父 shell 同步等待直至关窗」登记为开放问题,默认文档明示。
//! 启动链本体在 lib.rs `run`(测试形态以 lib.rs 为 crate 根)。

#![windows_subsystem = "console"]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slterm::run()
}
