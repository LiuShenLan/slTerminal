//! 窗口管理 facade:回窗的编译期单一实现点。
//!
//! toast 点击、托盘回窗与 toast 动作按钮最终都汇到 [`focus`]:调用方只传目标(窗口 / 窗内 pane),窗口还原、前置与
//! HWND 查询只在本文件实现;`SetForegroundWindow`/`ShowWindow` 等 Win32
//! 焦点原语不得出现在别处(门禁用例
//! `win32_focus_calls_only_live_in_the_facade`)。
//!
//! **设计签名适配。** `focus_window(cx)`/`focus_pane(pane,
//! cx)` 写在上游多窗注册表语境;单窗壳的壳事件泵已持有 `&mut Window`,
//! 且 GPUI 禁止在窗口更新回调内再次 `AnyWindowHandle::update`(窗口已被
//! take)。故 facade 以显式 `&mut Window` 落点;目标选择与平台调用仍由本
//! 文件唯一承担。
//!
//! **还原语义。** 「显示窗口」= 最小化则 `IsIconic → SW_RESTORE`,
//! 再 `SetForegroundWindow`;正常窗口直接前置。判定纯函数化为
//! [`focus_steps`] 供单元层断言;真实 Win32 面按豁免走构建产物 smoke。
//!
//! **HWND 单源。** 开窗时 [`publish_window`] 发布原生句柄;任务栏进度/闪烁
//! 统一经 [`window_hwnd`] 查询。虚拟窗口(测试)无原生句柄 → None,调用方
//! 按既有契约以 0 兜底。

use std::sync::atomic::{AtomicIsize, Ordering};

use gpui::{App, Context, Window};
use slterm_split::PaneId;

use crate::app_shell::workspace::SltermWorkspace;

/// 单窗单实例的原生 HWND(0 = 尚未发布;虚拟窗口无原生句柄)。
static MAIN_WINDOW_HWND: AtomicIsize = AtomicIsize::new(0);

/// 回窗需要执行的平台步骤(顺序即语义)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusStep {
    /// 最小化窗口先还原:`ShowWindow(SW_RESTORE)`。
    Restore,
    /// 前台化:`BringWindowToTop` + `SetForegroundWindow`。
    Foreground,
}

/// 还原语义单点:最小化 → 先还原再前置;正常 → 只前置。
///
/// 纯函数,`focus_steps(true)` 的顺序即 `IsIconic` 分支的真值锚。
pub(crate) fn focus_steps(minimized: bool) -> &'static [FocusStep] {
    if minimized {
        &[FocusStep::Restore, FocusStep::Foreground]
    } else {
        &[FocusStep::Foreground]
    }
}

/// 回窗的唯一入口:目标 `None` = 只聚焦窗口;`Some(pane)` = 窗口 +
/// 选中所在 tab + 窗内聚焦 pane。
pub(crate) fn focus(
    workspace: &mut SltermWorkspace,
    target: Option<PaneId>,
    window: &mut Window,
    cx: &mut Context<'_, SltermWorkspace>,
) {
    match target {
        Some(pane) => focus_pane(workspace, pane, window, cx),
        None => focus_window(window),
    }
}

/// 窗口级回窗:最小化则先还原,再前置;虚拟窗口只走 GPUI 激活。
pub(crate) fn focus_window(window: &mut Window) {
    if let Some(hwnd) = native_window_hwnd(window) {
        restore_and_foreground(hwnd);
    }
    // GPUI 内部激活态与平台句柄保持同步;幂等,可重复调用。
    window.activate_window();
}

/// pane 级回窗 = 窗口级回窗 + 选中 pane 所在 tab + 聚焦该 pane。
fn focus_pane(
    workspace: &mut SltermWorkspace,
    pane: PaneId,
    window: &mut Window,
    cx: &mut Context<'_, SltermWorkspace>,
) {
    focus_window(window);
    workspace.focus_notification_pane(pane, window, cx);
}

/// 开窗时发布唯一主窗原生句柄(启动链调一次;虚拟窗口静默跳过)。
pub(crate) fn publish_window(window: &Window) {
    if let Some(hwnd) = native_window_hwnd(window) {
        MAIN_WINDOW_HWND.store(hwnd, Ordering::Relaxed);
    }
}

/// 任务栏进度/闪烁的 HWND 查询点(单窗单实例的唯一读口)。
///
/// `_cx` 保留设计签名槽:当前实现读开窗时发布的进程级句柄;多窗回扩时
/// 改为按 `cx` 的活动窗口查询。
pub(crate) fn window_hwnd(_cx: &App) -> Option<isize> {
    match MAIN_WINDOW_HWND.load(Ordering::Relaxed) {
        0 => None,
        hwnd => Some(hwnd),
    }
}

/// 托盘菜单的收合前置:`TrackPopupMenu` 是模态泵,owner 窗口必须在前台
/// 才能「点外部收合」。这不是回窗语义,集中在本文件只为让 Win32 焦点
/// 调用保持单文件(门禁用例)。
#[cfg(windows)]
pub(crate) fn foreground_menu_owner(hwnd: isize) {
    use std::ffi::c_void;

    use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    // SAFETY: hwnd 是托盘线程自己的隐藏窗口,生命周期覆盖菜单调用。
    unsafe {
        SetForegroundWindow(hwnd as *mut c_void);
    }
}

#[cfg(not(windows))]
pub(crate) fn foreground_menu_owner(_hwnd: isize) {}

/// `IsIconic → SW_RESTORE → 前置` 的唯一实现体。
#[cfg(windows)]
fn restore_and_foreground(hwnd: isize) {
    use std::ffi::c_void;

    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };

    // SAFETY: hwnd 来自当前进程的活 GPUI 窗口;下面只调用窗口自身的
    // 还原/焦点原语,不接管句柄所有权。
    unsafe {
        let hwnd = hwnd as *mut c_void;
        for step in focus_steps(IsIconic(hwnd) != 0) {
            match step {
                FocusStep::Restore => {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                FocusStep::Foreground => {
                    BringWindowToTop(hwnd);
                    if SetForegroundWindow(hwnd) == 0 {
                        log::debug!("window foreground request deferred to GPUI activation");
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn restore_and_foreground(_hwnd: isize) {}

/// 原生窗口句柄(单窗语义下进程级唯一);虚拟窗口(测试)返回 None。
fn native_window_hwnd(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let native = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::Win32(native) = native.as_raw() else {
        return None;
    };
    Some(native.hwnd.get())
}

#[cfg(test)]
mod focus_tests {
    use std::path::{Path, PathBuf};

    use super::*;

    /// 最小化窗口必须先 SW_RESTORE 再前置;正常窗口只前置。
    #[test]
    fn minimized_window_restores_before_foregrounding() {
        assert_eq!(
            focus_steps(true),
            [FocusStep::Restore, FocusStep::Foreground].as_slice()
        );
        assert_eq!(focus_steps(false), [FocusStep::Foreground].as_slice());
    }

    /// 编译期单一实现点门禁:Win32 焦点/还原原语只许出现在本文件。
    /// 注释行剥除后再匹配,避免文案里的 API 名误报。
    #[test]
    fn win32_focus_calls_only_live_in_the_facade() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let facade = source_root.join("windowing").join("focus.rs");
        let forbidden = [
            "SetForegroundWindow(",
            "ShowWindow(",
            "ShowWindowAsync(",
            "BringWindowToTop(",
            "SetActiveWindow(",
            "SetFocus(",
            "IsIconic(",
            "SW_RESTORE",
        ];
        let mut violations = Vec::new();
        let mut files = Vec::new();
        collect_rust_files(&source_root, &mut files);
        for path in files {
            if path == facade {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("读取 {} 失败: {error}", path.display()));
            for (line_index, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                for symbol in forbidden {
                    if code.contains(symbol) {
                        violations.push(format!(
                            "{}:{} 直接调用 {symbol}",
                            path.strip_prefix(&source_root).unwrap_or(&path).display(),
                            line_index + 1
                        ));
                    }
                }
            }
        }
        assert!(
            violations.is_empty(),
            "回窗必须汇到 windowing::focus;以下位置绕过了 facade:\n{}",
            violations.join("\n")
        );
    }

    /// 前序临时路径已收编:notify 不再持有 HWND 发布/查询,壳事件泵不再
    /// 各自调 `activate_window`。
    #[test]
    fn temporary_focus_paths_are_collected_into_the_facade() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let notify = std::fs::read_to_string(source_root.join("notify.rs")).unwrap();
        assert!(
            !notify.contains("WINDOW_HWND"),
            "notify.rs 不得保留窗口句柄静态;查询统一走 windowing::focus::window_hwnd"
        );
        let control =
            std::fs::read_to_string(source_root.join("app_shell/workspace/control.rs")).unwrap();
        assert!(
            !control.contains("activate_window("),
            "壳事件泵必须经 windowing::focus 回窗,不得自行 activate_window"
        );
    }

    fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("读取 {} 失败: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("目录项").path();
            if path.is_dir() {
                collect_rust_files(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
}
