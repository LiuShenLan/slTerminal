//! 进程级启动准备(D04-1 console 子系统语义反转落点,13 篇改造节 1)。
//!
//! 裁剪:macOS locale/通知准备、托盘联动(start_hidden)、开机启动项
//! (launch_at_login)随托盘/系统集成归 09 篇,本步不迁。

mod console;
pub(crate) mod first_frame;

/// 便携版不在 PATH 时,非 PTY 子进程回退到本 exe 的环境变量(01 篇 C 表
/// `SLTERM_CLI`)。常量本体先在启动链落位;agent_env 模块(03 篇)迁入时归位,
/// per-pane 环境组装是 `SLTERM_PANE_ID` 那条链的邻居。
pub(crate) const CLI_ENV: &str = "SLTERM_CLI";

/// 启动失败上弹面(logger 未起时的最后通知手段);调用方同时把错误返回 stderr。
pub(crate) fn report_error(error: &dyn std::fmt::Display, gui_launch: bool) {
    crate::panic::report_startup_error(error, gui_launch);
}

/// 在任何 worker/终端子进程存在之前,完成进程级 GUI 状态归一化。
///
/// D04-1 console 子系统下的语义反转三刀(13 篇改造节 1,因果链照抄):
/// - AttachConsole 臂消亡:console 子系统进程出生即继承父控制台(双击启动由
///   系统分配新控制台),显式 Attach 失去对象。
/// - FreeConsole 臂消亡:console 子系统下从 pwsh/cmd 启动,父 shell 同步等待
///   子进程退出;FreeConsole 会扔掉继承的控制台、断开标准流——标准流正确性
///   优先于闪窗,Detach 语义整体不迁,不立新环境变量名。
/// - SetConsoleCtrlHandler(None, 0) 保留:NULL-handler ignore 标志会被 ConPTY
///   子 shell 继承,必须在此归一化一次,永不围绕 spawn 翻位(console.rs)。
pub(crate) fn prepare_gui_process() -> std::io::Result<()> {
    // 便携构建可能不在 PATH 上;非 PTY 子进程需要与 pane 环境同源的 exe 回退。
    if let Ok(executable) = std::env::current_exe() {
        // SAFETY: main 在启动仍单线程时调用本函数。
        unsafe { std::env::set_var(CLI_ENV, executable) };
    }
    console::prepare_console_for_gui()?;
    Ok(())
}

/// 与 GPUI 主显示器 DPI 查询同源的缩放预取:原生窗口创建前拿到 scale,
/// 首窗尺寸不漂(开窗后改名/重排的代价是二次 SetWindowPos)。
#[allow(dead_code)] // 消费点 = M3.2 startup_geometry(116×30 基准网格反推像素尺寸)。
pub(crate) fn primary_display_scale() -> Option<f32> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    // SAFETY: 监视器句柄借用即弃,两个 DPI 输出均为有效局部指针。
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let (mut x, mut y) = (0, 0);
        if monitor.is_null()
            || GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) < 0
            || x == 0
            || x != y
        {
            return None;
        }
        Some(x as f32 / 96.0)
    }
}
