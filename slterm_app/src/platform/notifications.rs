//! 原生通知注册、投递与激活回调。
//!
//! 策略(节流/前台门控/文案)全部归 `crate::notify`;本模块只承载 Windows
//! 平台面:未打包应用的 AUMID 注册(HKCU 注册表路线,免 COM/快捷方式/
//! 安装器/管理员)、WinRT toast 常驻 MTA worker、toast 点击回投。
//!
//! 投递纪律(边界 4):`Show` 是跨进程 RPC(几十 ms),永不在事件循环
//! 上跑;worker 队列有界(32,满即丢),活通知对象保留 64 条防 Activated
//! 回调 premature 释放;任何失败只落日志,不影响终端。
//!
//! 测试构建一律不投递真实系统 toast(通知中心/桌面会话不可重复),真实面由
//! `notify-test` 诊断命令与构建产物人工 smoke 兜底。

pub(crate) type ToastActivation = std::sync::Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub(crate) struct ToastAction {
    pub(crate) label: String,
    pub(crate) activate: ToastActivation,
}

mod windows;

/// 幂等 AUMID 注册(进程内缓存;best-effort)。壳启动早期调一次。
pub(crate) fn prepare() {
    windows::ensure_aumid();
}

/// 诊断/更新提示直发通道:不经漏斗、不带节流(调用方自担频率)。
pub(crate) fn show(title: &str, body: &str) {
    #[cfg(test)]
    {
        // 测试进程不得碰真实通知中心:真实投递面按豁免登记走人工 smoke。
        log::debug!("notify: native toast suppressed in test build: {title} · {body}");
    }
    #[cfg(not(test))]
    windows::enqueue(title, body, None, Vec::new());
}

/// 漏斗投递通道:交给 MTA worker(有界队列),事件循环只做入队。
pub(crate) fn dispatch(
    title: String,
    body: String,
    activation: Option<ToastActivation>,
    actions: Vec<ToastAction>,
) {
    #[cfg(test)]
    {
        let _ = (title, body, activation, actions);
        log::debug!("notify: native dispatch suppressed in test build");
    }
    #[cfg(not(test))]
    windows::enqueue(&title, &body, activation, actions);
}

/// 任务栏闪烁(FlashWindowEx,hwnd 直调不依赖窗口库)。恒发、静默、
/// 幂等,shell 自动合并;拿不到窗口句柄时静默跳过。
pub(crate) fn flash_taskbar(hwnd: Option<isize>) {
    if let Some(hwnd) = hwnd {
        windows::flash(hwnd);
    }
}

/// `slterm notify-test` 诊断入口:注册 + 时长 + 投递全链路同步执行,
/// 逐段打印 `OK`/`FAILED`(可机验)。刻意跳过前台门控与节流——测试者
/// 正看着屏幕。返回进程退出码。
pub(crate) fn notify_test() -> i32 {
    println!("[1/3] Registering AUMID '{}' ...", windows::AUMID);
    match windows::register_aumid() {
        Ok(()) => {
            println!(
                "      OK  (HKCU\\Software\\Classes\\AppUserModelId\\{})",
                windows::AUMID
            );
            match windows::icon_path() {
                Some(path) => println!("      icon: {}", path.display()),
                None => println!("      icon: unavailable (banner will have no logo)"),
            }
        }
        Err(error) => {
            println!("      FAILED: {error}");
            return 1;
        }
    }

    println!("[2/3] Reading notification_duration ...");
    let duration = slterm_settings::RuntimeSettings::load().notification_duration;
    match windows::duration_attribute(duration) {
        Some(value) => {
            println!("      OK  (notification_duration={duration:?} -> duration=\"{value}\")")
        }
        None => println!(
            "      OK  (notification_duration={duration:?} -> default: no duration attribute)"
        ),
    }

    println!("[3/3] Showing toast ...");
    match windows::show_sync(crate::brand::NAME, "通知链路正常：slterm notify-test") {
        Ok(()) => {
            println!("      OK  — a toast should be on screen now.");
            println!();
            println!("If nothing appeared, check Windows Settings > System > Notifications:");
            println!("the global toggle, Do Not Disturb / Focus Assist, and the slTerminal entry.");
            0
        }
        Err(error) => {
            println!("      FAILED: {error}");
            1
        }
    }
}
