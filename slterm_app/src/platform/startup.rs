//! 进程级启动准备 + 开机启动/静默启动(09 篇 M10.6)。
//!
//! 裁剪对照上游 `platform/startup.rs`:macOS locale/通知准备、
//! unix 登录项整支消亡(多平台整包砍);保留 console 归一化、首帧
//! present-then-show 与首窗 DPI 预取。
//!
//! - **开机启动** = 用户 Startup 已知文件夹中的 `slTerminal.lnk`
//!   (`IShellLinkW` 幂等重写,免注册表/免管理员);设置页与安装器管理
//!   同一快捷方式。
//! - **静默启动** = `silent_start && tray` 两条件合取。关窗即退定位下
//!   上游 `hide_window_on_close` 因子恒真、从公式消去(09 篇改造节 1);
//!   无托盘的隐藏进程没有任何回窗入口,故托盘是硬前提。

mod console;
pub(crate) mod first_frame;

use std::path::{Path, PathBuf};

use slterm_settings::RuntimeSettings;

/// 便携版不在 PATH 时,非 PTY 子进程回退到本 exe 的环境变量(01 篇 C 表
/// `SLTERM_CLI`)。常量本体先在启动链落位;agent_env 模块(03 篇)迁入时归位,
/// per-pane 环境组装是 `SLTERM_PANE_ID` 那条链的邻居。
pub(crate) const CLI_ENV: &str = "SLTERM_CLI";

/// 快捷方式文件名(01 篇 P-8:系统面品牌字符串归 09;与安装器同一路径)。
const SHORTCUT_FILE_NAME: &str = "slTerminal.lnk";

/// 启动失败上弹面(logger 未起时的最后通知手段);调用方同时把错误返回 stderr。
pub(crate) fn report_error(error: &dyn std::fmt::Display, gui_launch: bool) {
    crate::panic::report_startup_error(error, gui_launch);
}

/// 静默启动判定(09 篇 M10.6):设置两条件合取。
///
/// 消费点 = 启动真值表测试;壳侧持有的是 GUI 视图 `Settings`,经
/// [`start_hidden_for`] 共用同一公式,不重复折算 `RuntimeSettings`。
#[allow(dead_code)] // 设计签名保留;壳侧消费 = app_shell::config::Settings::start_hidden。
pub(crate) fn start_hidden(settings: &RuntimeSettings) -> bool {
    start_hidden_for(settings.silent_start, settings.tray)
}

/// 公式本体:两条真值缺一不可——托盘关闭时隐藏窗口 = 不可达进程。
pub(crate) fn start_hidden_for(silent_start: bool, tray: bool) -> bool {
    silent_start && tray
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
///
/// 消费点 = `app_shell::windowing::startup_geometry::preferred_size`
/// (09 篇 M10.7 出口验收:首窗创建前主屏 DPI 一步到位;运行时多显示器
/// DPI 变化整包交 GPUI 平台层,不 fork 窗口库)。
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

// ─── 开机启动 .lnk(09 篇 M10.6) ────────────────────────────────────────

/// 登录启动快捷方式的内容(与 COM 对象解耦,写读 round-trip 的断言面)。
///
/// 上游的 `--gpui` 是旧双人格选择参数;slTerminal 单进程单人格,且静默
/// 启动由设置两条件决定,故生产快捷方式不携带参数——参数形态仍由写读
/// API 完整支持(安装器/未来启动参数可直接注入)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shortcut {
    /// 目标可执行文件。
    pub(crate) target: PathBuf,
    /// 命令行参数串(原样写入/读回)。
    pub(crate) arguments: String,
    /// 启动工作目录(home;上游同因果)。
    pub(crate) working_directory: Option<PathBuf>,
    /// 图标来源(索引恒 0);None = 沿用目标的默认图标。
    pub(crate) icon: Option<PathBuf>,
}

/// 开/关登录启动(设置页唯一入口;安装器走同一路径)。
///
/// 写入是幂等重写:重复开启覆盖同一 `.lnk`;关闭删除文件并把「本来就不
/// 存在」视为成功(NotFound→Ok 的删除语义)。
pub fn set_launch_at_login(enabled: bool) -> std::io::Result<()> {
    // 测试构建不碰真实 Startup 文件夹:写读 round-trip 经 `_at` 形态注入
    // tempdir;真实目录由构建产物 smoke 覆盖(11 篇豁免登记)。
    if cfg!(test) {
        return Ok(());
    }
    let path = startup_shortcut()?;
    let shortcut = current_shortcut()?;
    set_launch_at_login_at(&path, enabled, &shortcut)
}

/// 登录启动是否已登记(设置页显示以实际 `.lnk` 为准:安装器与设置页
/// 管理同一 Startup 快捷方式)。
pub(crate) fn launch_at_login() -> bool {
    startup_shortcut().is_ok_and(|path| path.is_file())
}

/// 当前进程对应的快捷方式内容:目标 exe、home 工作目录、exe 自带图标。
fn current_shortcut() -> std::io::Result<Shortcut> {
    let executable = std::env::current_exe()?;
    Ok(Shortcut {
        target: executable.clone(),
        arguments: String::new(),
        working_directory: super::dirs::home_dir(),
        icon: Some(executable),
    })
}

/// 显式路径的写/删入口(tempdir 测试与安装器注入共用)。
pub(crate) fn set_launch_at_login_at(
    path: &Path,
    enabled: bool,
    shortcut: &Shortcut,
) -> std::io::Result<()> {
    if !enabled {
        return match std::fs::remove_file(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_shortcut(path.to_path_buf(), shortcut.clone())
}

/// 读取快捷方式内容(09 篇 M10.6 round-trip 断言端;生产读侧只需
/// [`launch_at_login`] 的存在判定)。
#[allow(dead_code)] // 消费点 = .lnk 写读 round-trip 测试与安装器核验(11 篇)。
pub(crate) fn read_shortcut_at(path: &Path) -> std::io::Result<Shortcut> {
    read_shortcut(path.to_path_buf())
}

/// 用户 Startup 已知文件夹中的快捷方式路径(免注册表/免管理员)。
fn startup_shortcut() -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Startup, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

    // SAFETY: 已知文件夹缓冲在释放 COM 分配之前先复制。
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None)
            .map_err(std::io::Error::other)?;
        let directory = std::ffi::OsString::from_wide(path.as_wide());
        CoTaskMemFree(Some(path.0.cast()));
        Ok(PathBuf::from(directory).join(SHORTCUT_FILE_NAME))
    }
}

/// COM 对象全部释放后才反初始化(09 篇 M10.1 生命周期红线,反序实测
/// 0xc0000005)。专用 STA 线程隔离调用线程的公寓状态——GPUI 主线程已被
/// `OleInitialize` 初始化,测试线程的公寓状态也不可控。
fn with_com<T, F>(work: F) -> std::io::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> windows::core::Result<T> + Send + 'static,
{
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

    let handle = std::thread::Builder::new()
        .name("slterm-startup-com".into())
        .spawn(move || {
            // SAFETY: 本线程独占 COM 单元;初始化的每一份成功都配对反初始化。
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .map_err(std::io::Error::other)?;
            let result = work();
            // SAFETY: work 内的接口已在返回前释放(生命周期红线)。
            unsafe { CoUninitialize() };
            result.map_err(std::io::Error::other)
        })
        .map_err(std::io::Error::other)?;
    handle
        .join()
        .map_err(|_| std::io::Error::other("快捷方式 COM 线程异常退出"))?
}

/// IShellLinkW + IPersistFile 幂等写入。
fn write_shortcut(path: PathBuf, shortcut: Shortcut) -> std::io::Result<()> {
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface};

    with_com(move || {
        // SAFETY: 全部接口在本闭包内创建并释放,with_com 在返回后才反初始化。
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&HSTRING::from(shortcut.target.as_os_str()))?;
            link.SetArguments(&HSTRING::from(shortcut.arguments.as_str()))?;
            if let Some(directory) = &shortcut.working_directory {
                link.SetWorkingDirectory(&HSTRING::from(directory.as_os_str()))?;
            }
            if let Some(icon) = &shortcut.icon {
                link.SetIconLocation(&HSTRING::from(icon.as_os_str()), 0)?;
            }
            let persist: IPersistFile = link.cast()?;
            persist.Save(&HSTRING::from(path.as_os_str()), true)
        }
    })
}

/// IShellLinkW + IPersistFile 读回;`SLGP_RAWPATH` 避免 shell 解析改写
/// 原始路径(round-trip 保真)。
fn read_shortcut(path: PathBuf) -> std::io::Result<Shortcut> {
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, SLGP_RAWPATH, ShellLink};
    use windows::core::{HSTRING, Interface};

    with_com(move || {
        // SAFETY: 与 write_shortcut 同序;缓冲为栈上有效切片。
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.cast::<IPersistFile>()?
                .Load(&HSTRING::from(path.as_os_str()), STGM_READ)?;

            let mut target = [0u16; 1024];
            link.GetPath(&mut target, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)?;
            let mut arguments = [0u16; 1024];
            link.GetArguments(&mut arguments)?;
            let mut directory = [0u16; 1024];
            link.GetWorkingDirectory(&mut directory)?;
            let mut icon = [0u16; 1024];
            let mut icon_index = 0i32;
            link.GetIconLocation(&mut icon, &mut icon_index)?;

            Ok(Shortcut {
                target: PathBuf::from(wide_string(&target)),
                arguments: wide_string(&arguments).to_string_lossy().into_owned(),
                working_directory: non_empty_path(wide_string(&directory)),
                icon: non_empty_path(wide_string(&icon)),
            })
        }
    })
}

/// 截到首个 NUL 的宽字符串。
fn wide_string(buffer: &[u16]) -> std::ffi::OsString {
    use std::os::windows::ffi::OsStringExt;

    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    std::ffi::OsString::from_wide(&buffer[..end])
}

/// 空串 = 该字段未设置。
fn non_empty_path(value: std::ffi::OsString) -> Option<PathBuf> {
    (!value.is_empty()).then(|| PathBuf::from(value))
}

#[cfg(test)]
mod startup_tests {
    use slterm_settings::RawSettings;

    use super::*;

    fn runtime(json: &str) -> RuntimeSettings {
        RuntimeSettings::from_raw(&RawSettings::from_json_bytes(json.as_bytes()))
    }

    /// 两条件真值表:silent_start 与 tray 缺一不可——无托盘的静默启动
    /// 会把应用藏成不可达进程,故 `silent_start=1` 单独返回 false。
    #[test]
    fn start_hidden_truth_table_requires_silent_start_and_tray() {
        for (json, expected) in [
            ("{}", false),
            (
                r#"{"session":{"silent_start":true},"system":{"tray":false}}"#,
                false,
            ),
            (
                r#"{"session":{"silent_start":false},"system":{"tray":true}}"#,
                false,
            ),
            (
                r#"{"session":{"silent_start":true},"system":{"tray":true}}"#,
                true,
            ),
        ] {
            assert_eq!(start_hidden(&runtime(json)), expected, "配置: {json}");
        }
    }

    /// 目标/参数/工作目录/图标经真实 COM IShellLinkW 写读 round-trip;
    /// 全部文件落 tempdir(假 Startup 目录),不碰真实用户目录。
    #[test]
    fn shortcut_round_trips_through_real_com_in_a_temp_directory() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SHORTCUT_FILE_NAME);
        let target = directory.path().join("slterm.exe");
        std::fs::write(&target, b"not a real executable").unwrap();
        let shortcut = Shortcut {
            target: target.clone(),
            arguments: "--working-directory \"C:\\work area\"".into(),
            working_directory: Some(directory.path().to_path_buf()),
            icon: Some(target),
        };

        set_launch_at_login_at(&path, true, &shortcut).unwrap();
        assert!(path.is_file(), "开启必须落地 .lnk 文件");
        assert_eq!(
            read_shortcut_at(&path).unwrap(),
            shortcut,
            "写读 round-trip 必须逐字段保真"
        );
    }

    /// 关闭 = 删除快捷方式;重复关闭幂等(NotFound 不算失败)。
    #[test]
    fn disabling_login_start_removes_the_shortcut_idempotently() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SHORTCUT_FILE_NAME);
        let target = directory.path().join("slterm.exe");
        std::fs::write(&target, b"not a real executable").unwrap();
        let shortcut = Shortcut {
            target,
            arguments: String::new(),
            working_directory: None,
            icon: None,
        };

        set_launch_at_login_at(&path, true, &shortcut).unwrap();
        set_launch_at_login_at(&path, false, &shortcut).unwrap();
        assert!(!path.exists(), "关闭必须删除 .lnk");
        set_launch_at_login_at(&path, false, &shortcut)
            .expect("重复关闭不得报错(NotFound 视为成功)");
    }

    /// 生产路径 = 用户 Startup 已知文件夹 + 品牌文件名(P-8),不写注册表。
    #[test]
    fn startup_shortcut_lands_in_the_user_startup_folder() {
        let path = startup_shortcut().unwrap();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(SHORTCUT_FILE_NAME)
        );
        let parent = path.parent().expect("Startup 快捷方式必须有父目录");
        let leaf = parent.file_name().unwrap_or_default().to_string_lossy();
        assert!(
            leaf.eq_ignore_ascii_case("Startup"),
            "快捷方式必须落在用户 Startup 文件夹,实际: {parent:?}"
        );
    }
}
