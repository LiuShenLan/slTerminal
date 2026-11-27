//! Windows 集成端口：application 访问托盘与开机启动的唯一接线面。
//!
//! 业务模块不得直接触碰 Win32 或读取注册表；真实实现委托既有平台模块，
//! 假实现供服务测试覆盖外部启动项变化与托盘不可用。

use std::path::{Path, PathBuf};

use slterm_settings::BlurMode;
use slterm_settings::json::Value;

use crate::platform::material::BlurSupport;
use crate::platform::startup::{self, LoginStartState};

/// application 访问 Windows 集成能力的唯一端口。
///
/// 新增 Windows 能力先在此登记方法、补真实实现与假实现，再接入业务服务。
pub(crate) trait WindowsIntegration: Send + Sync {
    /// 托盘当前是否真实可用：已启用、窗口已创建且图标已挂载。
    fn tray_available(&self) -> bool;

    /// 请求托盘启用状态；热应用与启动路径共用。
    fn set_tray_enabled(&self, enabled: bool);

    /// 静默启动前等待托盘真实可用；超时返回 false，调用方必须显示窗口。
    fn wait_tray_available(&self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.tray_available() {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Windows 实际开机启动项状态；不读取应用设置。
    fn login_start_state(&self) -> LoginStartState;

    /// 写入开机启动项；失败原样上抛。
    fn set_login_start(&self, enabled: bool) -> std::io::Result<()>;

    /// 窗口材质在当前系统的实际形态；不支持时设置页禁用并说明。
    fn blur_support(&self, mode: BlurMode) -> BlurSupport;

    /// 任务栏进度；无原生窗口句柄时静默跳过。
    fn set_taskbar_progress(&self, hwnd: Option<isize>, progress: crate::taskbar::TaskProgress);

    /// 另存为的原生目标选择；取消返回 None。只能在后台执行体调用。
    fn pick_save_target(
        &self,
        owner: Option<isize>,
        suggested: &Path,
    ) -> std::io::Result<Option<PathBuf>>;

    /// 文件选择的原生入口；取消返回 None。只能在后台执行体调用。
    fn pick_open_file(
        &self,
        _owner: Option<isize>,
        _start: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "平台不支持文件选择",
        ))
    }

    /// 目录选择的原生入口；取消返回 None。只能在后台执行体调用。
    fn pick_folder(
        &self,
        _owner: Option<isize>,
        _start: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "平台不支持目录选择",
        ))
    }

    /// 设置值与 Windows 实际启动项的对账报告。
    fn login_start_report(&self, setting_enabled: bool) -> LoginStartReport {
        LoginStartReport {
            setting_enabled,
            actual: self.login_start_state(),
        }
    }
}

/// 开机启动对账：设置值是意图，实际启动项才是生效状态。
#[derive(Clone, Debug)]
pub(crate) struct LoginStartReport {
    pub(crate) setting_enabled: bool,
    pub(crate) actual: LoginStartState,
}

impl LoginStartReport {
    /// 实际启动项是否真的会在登录时拉起本程序。
    pub(crate) fn effective_enabled(&self) -> bool {
        matches!(self.actual, LoginStartState::Enabled)
    }

    /// 应用设置与 Windows 实际状态不一致：外部修改后需要对账。
    pub(crate) fn stale(&self) -> bool {
        self.setting_enabled != self.effective_enabled()
    }

    /// 启动项指向其他程序时返回其目标。
    pub(crate) fn other_target(&self) -> Option<&Path> {
        match &self.actual {
            LoginStartState::OtherTarget(target) => Some(target),
            _ => None,
        }
    }

    /// 启动项不可读：不清楚就不伪造启用或禁用。
    pub(crate) fn unreadable(&self) -> bool {
        matches!(self.actual, LoginStartState::Unreadable)
    }
}

/// 静默驻留判定：显式恢复、设置两条件(由调用方经 `Settings::start_hidden`
/// 单点折算)与托盘真实可用缺一不可。
///
/// 托盘不可用时隐藏窗口会让窗口无入口，因此宁可正常显示。
pub(crate) fn start_hidden_with(
    autostart: bool,
    settings_allow: bool,
    windows: &dyn WindowsIntegration,
) -> bool {
    autostart && settings_allow && windows.tray_available()
}

/// 设置页需要展示的 Windows 实际状态说明。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IntegrationNote {
    /// 设置已开启，但 Windows 启动项不存在。
    LoginStartMissing,
    /// Windows 启动项指向其他程序。
    LoginStartOtherTarget(PathBuf),
    /// 启动项存在但读回失败：实际状态未知。
    LoginStartUnreadable,
    /// Windows 启动项已启用，但应用设置关闭。
    LoginStartEnabledButOff,
    /// 托盘不可用：静默驻留会回退为显示窗口。
    TrayUnavailable,
    /// 材质在当前系统以经典模糊近似。
    BlurFallback,
    /// 材质在当前系统不生效：档位已禁用。
    BlurUnsupported,
}

/// 按设置字段的当前值与端口事实生成说明；一致或无关字段返回 None。
pub(crate) fn settings_note(
    key: &str,
    setting: Option<&Value>,
    windows: &dyn WindowsIntegration,
) -> Option<IntegrationNote> {
    let setting_enabled = matches!(setting, Some(Value::Bool(true)));
    match key {
        "launch_at_login" => {
            let report = windows.login_start_report(setting_enabled);
            if report.unreadable() {
                return Some(IntegrationNote::LoginStartUnreadable);
            }
            if let Some(target) = report.other_target() {
                return Some(IntegrationNote::LoginStartOtherTarget(target.to_path_buf()));
            }
            if report.stale() {
                return Some(if report.effective_enabled() {
                    IntegrationNote::LoginStartEnabledButOff
                } else {
                    IntegrationNote::LoginStartMissing
                });
            }
            None
        }
        // 托盘设置关闭是用户意图，不是故障；只有请求启用却不可用才说明。
        "silent_start" | "tray" if setting_enabled && !windows.tray_available() => {
            Some(IntegrationNote::TrayUnavailable)
        }
        "blur" => {
            let mode = match setting {
                Some(Value::String(value)) => BlurMode::from_settings(value)?,
                _ => BlurMode::None,
            };
            match windows.blur_support(mode) {
                BlurSupport::Native => None,
                BlurSupport::BlurredFallback => Some(IntegrationNote::BlurFallback),
                BlurSupport::NoneFallback => Some(IntegrationNote::BlurUnsupported),
            }
        }
        _ => None,
    }
}

/// 设置页选项是否可选：不支持的材质档位明确禁用，不宣称静默生效。
pub(crate) fn choice_is_selectable(
    key: &str,
    option: &str,
    windows: &dyn WindowsIntegration,
) -> bool {
    if key != "blur" {
        return true;
    }
    BlurMode::from_settings(option).is_none_or(|mode| windows.blur_support(mode).is_selectable())
}

/// 生产实现：委托托盘与登录启动平台模块。
pub(crate) struct SystemWindowsIntegration;

impl WindowsIntegration for SystemWindowsIntegration {
    fn tray_available(&self) -> bool {
        crate::tray::available()
    }

    fn set_tray_enabled(&self, enabled: bool) {
        crate::tray::set_enabled(enabled);
    }

    fn login_start_state(&self) -> LoginStartState {
        startup::launch_at_login_state()
    }

    fn set_login_start(&self, enabled: bool) -> std::io::Result<()> {
        startup::set_launch_at_login(enabled)
    }

    fn blur_support(&self, mode: BlurMode) -> BlurSupport {
        crate::platform::material::blur_support(mode)
    }

    fn set_taskbar_progress(&self, hwnd: Option<isize>, progress: crate::taskbar::TaskProgress) {
        if let Some(hwnd) = hwnd {
            crate::taskbar::apply(hwnd, progress);
        }
    }

    fn pick_save_target(
        &self,
        owner: Option<isize>,
        suggested: &Path,
    ) -> std::io::Result<Option<PathBuf>> {
        crate::platform::save_dialog::pick_save_target(owner, suggested)
    }

    fn pick_folder(
        &self,
        owner: Option<isize>,
        start: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        crate::platform::folder_dialog::pick_folder(owner, start)
    }

    fn pick_open_file(
        &self,
        owner: Option<isize>,
        start: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        crate::platform::folder_dialog::pick_open_file(owner, start)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    /// 应用设置不是 Windows 实际启动项：外部改动必须在对账里显形。
    #[test]
    fn login_start_report_reconciles_setting_against_actual_startup_entry() {
        let windows = FakeWindowsIntegration::default();

        // 设置开启、实际无快捷方式：不生效且标记不一致。
        let report = windows.login_start_report(true);
        assert!(!report.effective_enabled());
        assert!(report.stale());
        assert!(report.other_target().is_none());
        assert!(!report.unreadable());

        // 启动项指向其他程序：仍不生效，并暴露目标。
        windows.set_actual_state(LoginStartState::OtherTarget(PathBuf::from("other.exe")));
        let report = windows.login_start_report(true);
        assert!(!report.effective_enabled());
        assert!(report.stale());
        assert_eq!(report.other_target(), Some(Path::new("other.exe")));

        // 读失败：不得伪造启用。
        windows.set_actual_state(LoginStartState::Unreadable);
        let report = windows.login_start_report(true);
        assert!(!report.effective_enabled());
        assert!(report.unreadable());
        assert!(report.stale());

        // 实际启用而设置关闭：有效启用且不一致。
        windows.set_actual_state(LoginStartState::Enabled);
        let report = windows.login_start_report(false);
        assert!(report.effective_enabled());
        assert!(report.stale());

        // 一致：不标记不一致。
        let report = windows.login_start_report(true);
        assert!(!report.stale());
    }

    /// 静默驻留依赖托盘真实可用：托盘不可用时绝不隐藏窗口。
    #[test]
    fn silent_residency_requires_a_usable_tray() {
        let windows = FakeWindowsIntegration::default();

        windows.set_tray_available(false);
        assert!(!start_hidden_with(true, true, &windows));

        windows.set_tray_available(true);
        assert!(start_hidden_with(true, true, &windows));
        assert!(!start_hidden_with(true, false, &windows));
        assert!(!start_hidden_with(false, true, &windows));
    }

    /// 设置页说明只看实际事实：设置值与启动项不一致必须显形。
    #[test]
    fn settings_notes_follow_the_actual_windows_state() {
        let windows = FakeWindowsIntegration::default();

        // 实际与设置一致：不产生说明。
        windows.set_actual_state(LoginStartState::Disabled);
        assert_eq!(
            settings_note("launch_at_login", Some(&Value::Bool(false)), &windows),
            None
        );
        windows.set_actual_state(LoginStartState::Enabled);
        assert_eq!(
            settings_note("launch_at_login", Some(&Value::Bool(true)), &windows),
            None
        );

        // 设置开启但启动项缺失。
        windows.set_actual_state(LoginStartState::Disabled);
        assert_eq!(
            settings_note("launch_at_login", Some(&Value::Bool(true)), &windows),
            Some(IntegrationNote::LoginStartMissing)
        );

        // 指向其他程序时带出真实目标。
        windows.set_actual_state(LoginStartState::OtherTarget(PathBuf::from("other.exe")));
        assert_eq!(
            settings_note("launch_at_login", Some(&Value::Bool(false)), &windows),
            Some(IntegrationNote::LoginStartOtherTarget(PathBuf::from(
                "other.exe"
            )))
        );

        // 读失败不伪造启用或禁用。
        windows.set_actual_state(LoginStartState::Unreadable);
        assert_eq!(
            settings_note("launch_at_login", Some(&Value::Bool(true)), &windows),
            Some(IntegrationNote::LoginStartUnreadable)
        );

        // 实际启用而应用设置关闭。
        windows.set_actual_state(LoginStartState::Enabled);
        assert_eq!(
            settings_note("launch_at_login", Some(&Value::Bool(false)), &windows),
            Some(IntegrationNote::LoginStartEnabledButOff)
        );

        // 托盘不可用时静默启动与托盘字段都说明回退。
        windows.set_actual_state(LoginStartState::Disabled);
        windows.set_tray_available(false);
        assert_eq!(
            settings_note("silent_start", Some(&Value::Bool(true)), &windows),
            Some(IntegrationNote::TrayUnavailable)
        );
        assert_eq!(
            settings_note("tray", Some(&Value::Bool(true)), &windows),
            Some(IntegrationNote::TrayUnavailable)
        );
        windows.set_tray_available(true);
        assert_eq!(
            settings_note("silent_start", Some(&Value::Bool(true)), &windows),
            None
        );
        assert_eq!(
            settings_note("unrelated", Some(&Value::Bool(true)), &windows),
            None
        );

        // 托盘设置本身关闭不是「不可用」：用户主动关闭不产生故障说明。
        windows.set_tray_available(false);
        assert_eq!(
            settings_note("tray", Some(&Value::Bool(false)), &windows),
            None
        );
        assert_eq!(
            settings_note("silent_start", Some(&Value::Bool(false)), &windows),
            None
        );
    }

    /// 材质档位不支持时给出回退/禁用说明，选项明确不可选。
    #[test]
    fn blur_notes_follow_the_material_capability() {
        let windows = FakeWindowsIntegration::default();
        let mica = Value::String("mica".to_owned());
        assert_eq!(settings_note("blur", Some(&mica), &windows), None);
        assert!(choice_is_selectable("blur", "mica", &windows));

        windows.set_blur_support(BlurSupport::BlurredFallback);
        assert_eq!(
            settings_note("blur", Some(&mica), &windows),
            Some(IntegrationNote::BlurFallback)
        );
        assert!(
            choice_is_selectable("blur", "mica", &windows),
            "经典模糊仍可选"
        );

        windows.set_blur_support(BlurSupport::NoneFallback);
        assert_eq!(
            settings_note("blur", Some(&mica), &windows),
            Some(IntegrationNote::BlurUnsupported)
        );
        assert!(
            !choice_is_selectable("blur", "mica", &windows),
            "不支持档位必须禁用"
        );
        assert!(choice_is_selectable("blur", "none", &windows));
        assert!(choice_is_selectable("blur", "aero", &windows));
        assert!(choice_is_selectable("font_size", "mica", &windows));

        // 当前选择为 none 时无说明。
        let none = Value::String("none".to_owned());
        assert_eq!(settings_note("blur", Some(&none), &windows), None);
    }

    /// 设置值与真实端口写入分离：写入失败原样上抛，不吞掉也不改实际状态。
    #[test]
    fn login_start_write_reports_failures_without_faking_the_state() {
        let windows = FakeWindowsIntegration::default();
        windows.fail_writes();

        let error = windows.set_login_start(true).expect_err("写入失败必须上报");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(windows.login_start_state(), LoginStartState::Disabled);

        windows.allow_writes();
        windows.set_login_start(true).expect("写入成功");
        assert_eq!(windows.login_start_state(), LoginStartState::Enabled);
    }
}

/// 供测试使用的 Windows 集成替身。
#[cfg(test)]
#[derive(Default)]
pub(crate) struct FakeWindowsIntegration {
    state: std::sync::Mutex<FakeState>,
}

#[cfg(test)]
struct FakeState {
    tray_available: bool,
    tray_enabled: bool,
    login_start: Option<LoginStartState>,
    fail_writes: bool,
    blur_support: BlurSupport,
    taskbar_progress: Vec<crate::taskbar::TaskProgress>,
    save_target: Option<PathBuf>,
    folder_target: Option<PathBuf>,
    open_file_target: Option<PathBuf>,
}

#[cfg(test)]
impl Default for FakeState {
    /// 测试环境默认托盘可用；用例显式关闭才覆盖不可用路径。
    fn default() -> Self {
        Self {
            tray_available: true,
            tray_enabled: false,
            login_start: None,
            fail_writes: false,
            blur_support: BlurSupport::Native,
            taskbar_progress: Vec::new(),
            save_target: None,
            folder_target: None,
            open_file_target: None,
        }
    }
}

#[cfg(test)]
impl FakeWindowsIntegration {
    pub(crate) fn set_tray_available(&self, available: bool) {
        self.lock().tray_available = available;
    }

    /// 覆盖依赖系统 backdrop 的档位形态；None/Aero 始终原生可用。
    pub(crate) fn set_blur_support(&self, support: BlurSupport) {
        self.lock().blur_support = support;
    }

    /// 取走已投递的任务栏进度序列。
    pub(crate) fn take_taskbar_progress(&self) -> Vec<crate::taskbar::TaskProgress> {
        std::mem::take(&mut self.lock().taskbar_progress)
    }

    /// 设定另存为目标选择的固定答案；None = 用户取消。
    pub(crate) fn set_save_target(&self, target: Option<PathBuf>) {
        self.lock().save_target = target;
    }

    /// 设定目录选择的固定答案；None = 用户取消。
    pub(crate) fn set_folder_target(&self, target: Option<PathBuf>) {
        self.lock().folder_target = target;
    }

    /// 设定文件选择的固定答案；None = 用户取消。
    pub(crate) fn set_open_file_target(&self, target: Option<PathBuf>) {
        self.lock().open_file_target = target;
    }

    pub(crate) fn set_actual_state(&self, state: LoginStartState) {
        self.lock().login_start = Some(state);
    }

    pub(crate) fn fail_writes(&self) {
        self.lock().fail_writes = true;
    }

    pub(crate) fn allow_writes(&self) {
        self.lock().fail_writes = false;
    }

    /// 端口是否收到了托盘启用请求（启动路径必须把设置交给端口）。
    pub(crate) fn tray_enabled(&self) -> bool {
        self.lock().tray_enabled
    }

    /// 当前实际启动项状态（GPUI 测试不经 trait 导入即可断言）。
    #[cfg(feature = "gpui-test-support")]
    pub(crate) fn current_login_start_state(&self) -> LoginStartState {
        self.lock()
            .login_start
            .clone()
            .unwrap_or(LoginStartState::Disabled)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

#[cfg(test)]
impl WindowsIntegration for FakeWindowsIntegration {
    fn tray_available(&self) -> bool {
        self.lock().tray_available
    }

    fn set_tray_enabled(&self, enabled: bool) {
        self.lock().tray_enabled = enabled;
    }

    /// 测试替身按当前状态立即回答，不等待真实托盘线程。
    fn wait_tray_available(&self, _timeout: std::time::Duration) -> bool {
        self.tray_available()
    }

    fn login_start_state(&self) -> LoginStartState {
        self.lock()
            .login_start
            .clone()
            .unwrap_or(LoginStartState::Disabled)
    }

    fn set_login_start(&self, enabled: bool) -> std::io::Result<()> {
        let mut state = self.lock();
        if state.fail_writes {
            return Err(std::io::Error::other("fake login start write failed"));
        }
        state.login_start = Some(if enabled {
            LoginStartState::Enabled
        } else {
            LoginStartState::Disabled
        });
        Ok(())
    }

    fn blur_support(&self, mode: BlurMode) -> BlurSupport {
        if matches!(mode, BlurMode::None | BlurMode::Aero) {
            return BlurSupport::Native;
        }
        self.lock().blur_support
    }

    fn set_taskbar_progress(&self, _hwnd: Option<isize>, progress: crate::taskbar::TaskProgress) {
        self.lock().taskbar_progress.push(progress);
    }

    fn pick_save_target(
        &self,
        _owner: Option<isize>,
        _suggested: &Path,
    ) -> std::io::Result<Option<PathBuf>> {
        Ok(self.lock().save_target.clone())
    }

    fn pick_folder(
        &self,
        _owner: Option<isize>,
        _start: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        Ok(self.lock().folder_target.clone())
    }

    fn pick_open_file(
        &self,
        _owner: Option<isize>,
        _start: Option<&Path>,
    ) -> std::io::Result<Option<PathBuf>> {
        Ok(self.lock().open_file_target.clone())
    }
}
