//! WinRT toast 投递与 AUMID 注册(照上游平台层裁剪)。
//!
//! 常驻 MTA worker 持有全部 WinRT 对象:Show 是跨进程 RPC,永不上事件
//! 循环;成功 Show 不足以让 Activated 回调存活,活通知对象必须保留
//! (有界 64 条,溢出丢最老)。AUMID 走 `HKCU\Software\Classes\
//! AppUserModelId\<AUMID>` 注册表路线:免 COM、免快捷方式、免安装器、
//! 免管理员,幂等重写即自愈。

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};

use ::windows::Data::Xml::Dom::XmlDocument;
use ::windows::Foundation::TypedEventHandler;
use ::windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
};
use ::windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use ::windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use ::windows::core::{IInspectable, Interface};
use slterm_settings::NotificationDuration;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};

use super::{ToastAction, ToastActivation};

/// AppUserModelID(toast 归因/任务栏归组;值锚点归 P-3)。
pub(super) const AUMID: &str = crate::brand::WINDOWS_APP_ID;

/// worker 队列上限:满即丢(通知是 best-effort,不阻塞调用方)。
#[cfg_attr(test, allow(dead_code))] // 测试构建不投递真实 toast(见父模块)。
const QUEUE_CAPACITY: usize = 32;
/// 活通知保留量:不足会提前释放 Activated 回调。
#[cfg_attr(test, allow(dead_code))]
const LIVE_RETENTION: usize = 64;

struct Request {
    title: String,
    body: String,
    open: Option<ToastActivation>,
    actions: Vec<ToastAction>,
}

#[cfg_attr(test, allow(dead_code))] // 测试构建只走 show_sync 诊断面。
pub(super) fn enqueue(
    title: &str,
    body: &str,
    open: Option<ToastActivation>,
    actions: Vec<ToastAction>,
) {
    static WORKER: OnceLock<Option<mpsc::SyncSender<Request>>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("slterm-native-notifications".into())
            .spawn(move || run(receiver))
            .map(|_| sender)
            .map_err(|error| log::warn!("notify: could not start native worker: {error}"))
            .ok()
    });
    let Some(worker) = worker else { return };
    if let Err(error) = worker.try_send(Request {
        title: title.to_owned(),
        body: body.to_owned(),
        open,
        actions,
    }) {
        log::warn!("notify: native queue unavailable: {error}");
    }
}

#[cfg_attr(test, allow(dead_code))]
fn run(receiver: mpsc::Receiver<Request>) {
    // 全部 WinRT 对象活在这条 MTA 线程;回调只向壳事件队列投递。
    let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let owns_com = init.is_ok();
    if init.is_err() && init != RPC_E_CHANGED_MODE {
        log::warn!("notify: COM initialization failed: {init:?}");
        return;
    }
    let mut live = VecDeque::new();
    for request in receiver {
        // 时长是设置键的消费面:在组装 xml 的 worker 处读当前值。
        let duration = slterm_settings::RuntimeSettings::load().notification_duration;
        match show_request(request, duration) {
            Ok(notification) => {
                if live.len() == LIVE_RETENTION {
                    live.pop_front();
                }
                live.push_back(notification);
            }
            Err(error) => log::warn!("notify: native delivery failed: {error}"),
        }
    }
    drop(live);
    if owns_com {
        unsafe { CoUninitialize() };
    }
}

/// 诊断通道的同步投递(CLI 进程;当前线程初始化 MTA)。
pub(super) fn show_sync(title: &str, body: &str) -> Result<(), String> {
    let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let owns_com = init.is_ok();
    if init.is_err() && init != RPC_E_CHANGED_MODE {
        return Err(format!("COM initialization failed: {init:?}"));
    }
    let duration = slterm_settings::RuntimeSettings::load().notification_duration;
    let result = show_request(
        Request {
            title: title.to_owned(),
            body: body.to_owned(),
            open: None,
            actions: Vec::new(),
        },
        duration,
    );
    // 顺序红线:WinRT 对象必须先于 CoUninitialize 释放。反过来(release
    // 一个仍活着的 ToastNotification)会访问已拆的 COM apartment,
    // 真机实测 0xc0000005。
    let outcome = match result {
        Ok(notification) => {
            drop(notification);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    };
    if owns_com {
        unsafe { CoUninitialize() };
    }
    outcome
}

fn show_request(
    request: Request,
    duration: NotificationDuration,
) -> ::windows::core::Result<ToastNotification> {
    ensure_aumid();
    let icon = icon_path();
    let document = XmlDocument::new()?;
    document.LoadXml(&xml(&request, duration, icon.as_deref()).into())?;
    let notification = ToastNotification::CreateToastNotification(&document)?;
    notification.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
        move |_, args| {
            let argument = args
                .as_ref()
                .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
                .and_then(|args| args.Arguments().ok())
                .map(|value| value.to_string())
                .unwrap_or_default();
            if argument.is_empty() {
                if let Some(open) = &request.open {
                    open();
                }
            } else if let Some(index) = argument
                .strip_prefix("choice-")
                .and_then(|id| id.parse::<usize>().ok())
                && let Some(action) = request.actions.get(index)
            {
                (action.activate)();
            }
            Ok(())
        },
    ))?;
    ToastNotificationManager::CreateToastNotifierWithId(&AUMID.into())?.Show(&notification)?;
    Ok(notification)
}

/// ToastGeneric 文档:标题/正文转义、控制字符过滤、可选 logo、动作按钮
/// (最多 5 个)与 duration 槽(Default 不注入,与上游逐字节一致)。
fn xml(request: &Request, duration: NotificationDuration, icon: Option<&Path>) -> String {
    let duration = duration_attribute(duration)
        .map(|value| format!(" duration=\"{value}\""))
        .unwrap_or_default();
    let mut xml = format!(
        "<toast{duration}><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text>",
        escape(&request.title),
        escape(&request.body),
    );
    if let Some(path) = icon {
        xml.push_str(&format!(
            "<image placement=\"appLogoOverride\" src=\"{}\"/>",
            escape(&path.to_string_lossy())
        ));
    }
    xml.push_str("</binding></visual>");
    if !request.actions.is_empty() {
        xml.push_str("<actions>");
        for (index, action) in request.actions.iter().take(5).enumerate() {
            xml.push_str(&format!(
                "<action content=\"{}\" arguments=\"choice-{index}\" activationType=\"foreground\"/>",
                escape(&action.label)
            ));
        }
        xml.push_str("</actions>");
    }
    xml.push_str("</toast>");
    xml
}

/// 设置键 `notification_duration` → WinRT 合法 duration 属性。WinRT toast
/// 只有 short/long 两档:5/10s 归 short,30/90s 与 persistent 归 long
/// (系统通道没有常驻档,persistent 取最大合法时长;壳内卡片时长另归 05)。
pub(super) fn duration_attribute(duration: NotificationDuration) -> Option<&'static str> {
    match duration {
        NotificationDuration::Default => None,
        NotificationDuration::FiveSeconds | NotificationDuration::TenSeconds => Some("short"),
        NotificationDuration::ThirtySeconds
        | NotificationDuration::NinetySeconds
        | NotificationDuration::Persistent => Some("long"),
    }
}

fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ─── AUMID 注册(HKCU,免 COM/管理员) ──────────────────────────────────

/// 幂等注册;进程内缓存,失败下次投递再试(自愈语义)。
pub(super) fn ensure_aumid() {
    static REGISTERED: Mutex<bool> = Mutex::new(false);
    let mut registered = REGISTERED.lock().unwrap_or_else(|error| error.into_inner());
    if *registered {
        return;
    }
    match register_aumid() {
        Ok(()) => *registered = true,
        Err(error) => {
            log::warn!("notify: AUMID registration failed (toast may not appear): {error}")
        }
    }
}

/// 写 `DisplayName` + `IconUri`(有图标资产时);`RegSetKeyValueW` 自动建
/// 缺失的子键链。图标 best-effort:写失败只丢 logo,不丢 toast。
pub(super) fn register_aumid() -> Result<(), String> {
    let subkey = format!(r"Software\Classes\AppUserModelId\{AUMID}");
    set_reg_sz(&subkey, "DisplayName", crate::brand::NAME)?;
    match icon_path() {
        Some(icon) => set_reg_sz(&subkey, "IconUri", &icon.display().to_string())?,
        None => log::debug!("notify: toast icon not materialized; banner shows no logo"),
    }
    Ok(())
}

/// 物化/发现的 logo 路径(诊断命令打印)。随包资产
/// `assets/app-icon.png` 优先;数据目录 `toast_icon.png` 与 exe 旁
/// `slterm.png` 作为历史散落形态兼容。
pub(super) fn icon_path() -> Option<PathBuf> {
    let mut candidates = vec![crate::platform::dirs::data_dir().join("toast_icon.png")];
    if let Ok(exe) = std::env::current_exe()
        && let Some(directory) = exe.parent()
    {
        candidates.insert(0, directory.join("assets").join("app-icon.png"));
        candidates.push(directory.join("slterm.png"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// 设一个 HKCU\`subkey` 下的 REG_SZ,必要时建键。
fn set_reg_sz(subkey: &str, name: &str, data: &str) -> Result<(), String> {
    let subkey_w = to_wide(subkey);
    let name_w = to_wide(name);
    let data_w = to_wide(data);
    let data_bytes = (data_w.len() * std::mem::size_of::<u16>()) as u32;

    // SAFETY: 三个指针都指向本帧存活的 NUL 结尾 UTF-16 缓冲;
    // RegSetKeyValueW 在返回前复制数据,并按需创建中间键。
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            subkey_w.as_ptr(),
            name_w.as_ptr(),
            REG_SZ,
            data_w.as_ptr().cast(),
            data_bytes,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!(
            "RegSetKeyValueW({name}) failed with status {status}"
        ))
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

// ─── 任务栏闪烁(FlashWindowEx 直调) ───────────────────────────────────

/// 闪烁直到窗口回到前台(`FLASHW_TIMERNOFG`);对已经在前台的窗口无可见
/// 效果,幂等静默。
pub(super) fn flash(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FLASHW_ALL, FLASHW_TIMERNOFG, FLASHWINFO, FlashWindowEx,
    };

    let info = FLASHWINFO {
        cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
        hwnd: hwnd as _,
        dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
        uCount: 0,
        dwTimeout: 0,
    };
    // SAFETY: 结构体字段全部按 Win32 布局初始化;调用不保留指针。
    unsafe {
        FlashWindowEx(&info);
    }
}

#[cfg(test)]
mod windows_tests {
    use super::*;

    fn request() -> Request {
        Request {
            title: "标题".into(),
            body: "正文".into(),
            open: None,
            actions: Vec::new(),
        }
    }

    /// 控制字符与 XML 标记不能破坏文档;换行保留由上层 clamp 折叠。
    #[test]
    fn terminal_controls_and_markup_cannot_break_the_toast_document() {
        assert_eq!(
            escape("<tool>\u{1b}\u{0}\"&'"),
            "&lt;tool&gt;&quot;&amp;&apos;"
        );
    }

    /// duration 槽:Default 不写属性(与上游一致),六档设置映射到
    /// WinRT 合法 short/long。
    #[test]
    fn notification_duration_maps_to_winrt_values() {
        assert_eq!(duration_attribute(NotificationDuration::Default), None);
        assert_eq!(
            duration_attribute(NotificationDuration::FiveSeconds),
            Some("short")
        );
        assert_eq!(
            duration_attribute(NotificationDuration::TenSeconds),
            Some("short")
        );
        assert_eq!(
            duration_attribute(NotificationDuration::ThirtySeconds),
            Some("long")
        );
        assert_eq!(
            duration_attribute(NotificationDuration::NinetySeconds),
            Some("long")
        );
        assert_eq!(
            duration_attribute(NotificationDuration::Persistent),
            Some("long")
        );
    }

    /// 非 Default 才注入 duration;标题/正文/动作按钮顺序稳定。
    #[test]
    fn xml_injects_duration_only_when_configured() {
        let document = xml(&request(), NotificationDuration::Default, None);
        assert!(document.starts_with("<toast><visual>"));
        assert!(!document.contains("duration="));
        assert!(document.contains("<text>标题</text><text>正文</text>"));

        let short = xml(&request(), NotificationDuration::FiveSeconds, None);
        assert!(short.starts_with("<toast duration=\"short\"><visual>"));
        let long = xml(&request(), NotificationDuration::NinetySeconds, None);
        assert!(long.starts_with("<toast duration=\"long\"><visual>"));
    }

    /// 动作按钮最多 5 个,arguments 序号与回调下标一致。
    #[test]
    fn xml_limits_actions_and_numbers_their_arguments() {
        let mut request = request();
        request.actions = (0..7)
            .map(|index| ToastAction {
                label: format!("选项{index}"),
                activate: std::sync::Arc::new(|| {}),
            })
            .collect();
        let document = xml(&request, NotificationDuration::Default, None);
        assert_eq!(document.matches("<action ").count(), 5);
        assert!(document.contains("arguments=\"choice-4\""));
        assert!(!document.contains("choice-5"));
    }
}
