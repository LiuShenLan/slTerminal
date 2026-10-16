//! 进程 console 属性归一化:必须抢在任何终端子进程继承它之前(整文件随上游
//! 随迁,语义零改动;环境变量改名归测试文件)。

#[cfg(test)]
mod console_tests;

pub(crate) fn prepare_console_for_gui() -> std::io::Result<()> {
    // NULL-handler ignore 标志会被 ConPTY shell 继承。在任何 worker/终端存在前
    // 归一化一次;永不围绕 spawn 翻位。
    // SAFETY: 只改本进程的 console 属性,不动父进程或任何已存在子进程;
    // 无附着 console 时调用同样成立。
    if unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
