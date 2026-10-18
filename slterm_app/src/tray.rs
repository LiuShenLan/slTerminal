//! 系统托盘常驻图标 + agent attention 状态。
//!
//! 终端最常见的姿态是「压在别的窗口底下跑着 agent」:任务栏闪烁会被忽略、
//! toast 会过期,而托盘图标一直在。任一 agent 停下来等输入时图标翻转成
//! attention 态(右下角橙点),右键菜单列出所有 agent pane 与状态,点击
//! 直达来源 pane——与 toast 点击是同一条回窗语义(单窗:还原并前置唯一
//! 主窗,再选中 tab、聚焦 pane)。
//!
//! # 线程模型
//!
//! `Shell_NotifyIconW` 的回调消息需要一个窗口收;不能挂在 GPUI 壳窗上
//! (壳不背 OS 消息泵职责)。托盘因此拥有自己的线程:一扇永不显示的 Win32
//! 窗口 + 独立消息泵。app 侧只做两件事——把 agent 清单写进 [`STATE`]、
//! `PostMessageW` 摇醒托盘线程;托盘线程反向只通过 [`GPUI_COMMAND`] 回调
//! 发 [`GpuiTrayCommand`]。两个方向都不共享锁跨越阻塞调用。
//!
//! 状态推送挂在既有的 1 Hz 看门狗节拍(workspace 的屏幕看门狗):托盘是
//! 环境信息面,秒级延迟无感,换来的是零新增常驻 OS 定时器与天然去抖——
//! [`update`] 内容相等直接返回,1 Hz 调用方无需自行判重。
//!
//! # 收敛点
//!
//! - 单窗单实例:`Focus` 的 pane 参数只可能来自本窗快照;`Quit` 走关窗
//!   同一条退出链(终局快照 + 最后一窗关闭 = 进程退出)。
//! - 托盘图标资产(C24)归 M12:当前无内嵌品牌图,退到中性灰占位圆盘;
//!   资产落地后只在 [`brand_icon_pixels`] 一处接入,橙点像素管线不变。

use std::sync::{Mutex, MutexGuard, OnceLock};

use slterm_split::PaneId;

use crate::i18n::{Message, UiLanguage};

/// GPUI 托盘动作:聚焦某个 pane(None = 只聚焦主窗),或真退出进程。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GpuiTrayCommand {
    Focus(Option<PaneId>),
    Quit,
}

/// 托盘菜单里的一行:一个正在跑 AI CLI 的 pane。
///
/// 单窗化后 `window` 路由目标塌缩为唯一主窗,不进快照;菜单与侧栏读同一份
/// pane 事实源(workspace 的 `AgentHub`),不维护第二份 Agent 状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrayAgent {
    pub pane: PaneId,
    /// 菜单文案:「claude · 项目目录」形态 = 程序名 + cwd 尾段。
    pub label: String,
    /// CLI 停在半路等用户(权限确认/提问),比「回合完成」更强。
    pub needs_attention: bool,
}

/// 菜单动作(与 HMENU 命令 id 解耦,纯构建阶段可断言)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayAction {
    Focus(Option<PaneId>),
    Quit,
}

/// 菜单条目:分隔线,或一条带稳定命令 id 的动作项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrayMenuEntry {
    Separator,
    Item {
        command_id: usize,
        label: String,
        action: TrayAction,
    },
}

/// 菜单命令 id 的起点;`MENU_AGENT_BASE + i` = 聚焦第 i 个 agent。
const MENU_AGENT_BASE: usize = 1000;
/// 「显示 slTerminal」——没有 agent 时托盘仍要能把窗口捞回来。
const MENU_SHOW: usize = 1;
/// 单窗下「退出」是真退出:与关窗同一条退出链。
const MENU_QUIT: usize = 2;

#[cfg(windows)]
const WM_APP: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP;
#[cfg(not(windows))]
const WM_APP: u32 = 0x8000;

/// `Shell_NotifyIconW` 回调(经典 v0 语义:lParam 携带鼠标消息)。
const WM_APP_TRAY: u32 = WM_APP + 1;
/// 快照变了:重读并 NIM_MODIFY 图标/提示。
const WM_APP_REFRESH: u32 = WM_APP + 2;
/// 启用态变了:NIM_ADD / NIM_DELETE。
const WM_APP_SET: u32 = WM_APP + 3;
/// 进程收尾:删图标 + 退出消息泵(不删的话图标要等用户悬停才消失)。
const WM_APP_SHUTDOWN: u32 = WM_APP + 4;

/// 托盘线程与 GPUI 之间的唯一共享状态。内容相等即视为无变化(1 Hz 去抖)。
struct Shared {
    agents: Vec<TrayAgent>,
    enabled: bool,
    /// 菜单/提示文案语言(托盘线程不碰 GPUI,只能随快照带过去)。
    language: UiLanguage,
}

impl Shared {
    const fn new() -> Self {
        Self {
            agents: Vec::new(),
            enabled: false,
            language: UiLanguage::EnUs,
        }
    }

    /// 应用一份新快照;返回是否真的变化。变化判据包含语言——语言切换后
    /// 下一次菜单打开必须已经是新语言。
    fn apply(&mut self, agents: Vec<TrayAgent>, language: UiLanguage) -> bool {
        if self.agents == agents && self.language == language {
            return false;
        }
        self.agents = agents;
        self.language = language;
        true
    }

    fn attention_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|agent| agent.needs_attention)
            .count()
    }
}

static STATE: Mutex<Shared> = Mutex::new(Shared::new());

/// GPUI 主窗没有事件循环代理:托盘点击走命令回调。
static GPUI_COMMAND: OnceLock<Box<dyn Fn(GpuiTrayCommand) + Send + Sync>> = OnceLock::new();

#[cfg(test)]
type TestPush = (Vec<TrayAgent>, UiLanguage);
#[cfg(test)]
static TEST_PUSHES: Mutex<Vec<TestPush>> = Mutex::new(Vec::new());
/// 测试构建记录发往托盘线程的消息(不启真实线程/窗口)。
#[cfg(test)]
static TEST_POSTS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// 状态锁:中毒只表示某线程带锁 panic;清单本身是普通数据,继续用。
///
/// 测试构建里真托盘线程从不启动(见 [`set_enabled`]),STATE 只被用例
/// 经 [`reset_for_test`] 隔离读写,不存在 Win32 侧读者。
fn lock_state() -> MutexGuard<'static, Shared> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    *lock_state() = Shared::new();
    TEST_PUSHES
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
    TEST_POSTS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
}

#[cfg(test)]
pub(crate) fn take_updates_for_test() -> Vec<TestPush> {
    std::mem::take(
        &mut *TEST_PUSHES
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
    )
}

#[cfg(test)]
pub(crate) fn take_posts_for_test() -> Vec<u32> {
    std::mem::take(&mut *TEST_POSTS.lock().unwrap_or_else(|error| error.into_inner()))
}

/// 装好命令回调。真正的托盘线程按需(首次启用)才起。
pub(crate) fn init_gpui(on_command: impl Fn(GpuiTrayCommand) + Send + Sync + 'static) {
    let _ = GPUI_COMMAND.set(Box::new(on_command));
}

/// 开/关托盘图标(设置·高级)。开 = 保证线程在跑并 NIM_ADD;
/// 关 = 只删图标,线程留着(重开是高频往复,不值得反复建窗)。
///
/// B54:开启态每次调用都向托盘线程投递 `WM_APP_SET`,不只在状态翻转时投递。
/// `NIM_ADD` 在 explorer 未就绪/刚崩时会失败,1 Hz 快照调用方因此天然成为
/// 重试节拍;托盘线程在 `apply_enabled` 里对未挂载态重复 `NIM_ADD`。
pub(crate) fn set_enabled(enabled: bool) {
    let changed = {
        let mut shared = lock_state();
        let changed = shared.enabled != enabled;
        shared.enabled = enabled;
        changed
    };
    // 测试构建绝不建真窗口/真线程:托盘交互按豁免走构建产物 smoke。
    if enabled && !cfg!(test) {
        ensure_thread();
    }
    if enabled || changed {
        post(WM_APP_SET);
    }
}

/// 发布最新 agent 清单。内容没变就不打扰托盘线程(1 Hz 调用方无需去抖)。
pub(crate) fn update(agents: Vec<TrayAgent>, language: UiLanguage) {
    let (changed, enabled) = {
        let mut shared = lock_state();
        let changed = shared.apply(agents, language);
        (changed, shared.enabled)
    };
    if !changed || !enabled {
        return;
    }
    #[cfg(test)]
    {
        // 测试只记录真正会推送的快照(去抖断言的观测端);不触 Win32。
        let shared = lock_state();
        TEST_PUSHES
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push((shared.agents.clone(), shared.language));
    }
    #[cfg(not(test))]
    post(WM_APP_REFRESH);
}

/// 进程退出前删掉图标。best-effort:托盘线程没起过就什么都不用做。
pub(crate) fn shutdown() {
    #[cfg(windows)]
    win::shutdown();
}

fn post(message: u32) {
    #[cfg(test)]
    {
        // 测试构建记录消息、绝不触 Win32;断言覆盖「何时投递」而非线程行为。
        TEST_POSTS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(message);
    }
    #[cfg(not(test))]
    {
        #[cfg(windows)]
        win::post(message);
        #[cfg(not(windows))]
        let _ = message;
    }
}

#[cfg(windows)]
fn ensure_thread() {
    win::ensure_thread();
}
#[cfg(not(windows))]
fn ensure_thread() {}

fn send_command(command: GpuiTrayCommand) {
    if let Some(callback) = GPUI_COMMAND.get() {
        callback(command);
    }
}

/// 左键/菜单兜底的聚焦策略:先挑等输入的,再挑任意 agent,最后裸窗口。
fn best_agent(agents: &[TrayAgent]) -> Option<PaneId> {
    agents
        .iter()
        .find(|agent| agent.needs_attention)
        .or_else(|| agents.first())
        .map(|agent| agent.pane)
}

fn focus_best() {
    let target = {
        let shared = lock_state();
        best_agent(&shared.agents)
    };
    send_command(GpuiTrayCommand::Focus(target));
}

/// 命令 id → 动作。菜单打开期间快照可能被 1 Hz 更新,命令 id 必须映射回
/// 打开那一刻的清单(调用方传入当次克隆)。
fn action_for_command(command_id: usize, agents: &[TrayAgent]) -> Option<TrayAction> {
    match command_id {
        MENU_SHOW => Some(TrayAction::Focus(None)),
        MENU_QUIT => Some(TrayAction::Quit),
        id if id >= MENU_AGENT_BASE => agents
            .get(id - MENU_AGENT_BASE)
            .map(|agent| TrayAction::Focus(Some(agent.pane))),
        _ => None,
    }
}

/// 菜单构建纯函数:动作枚举 + 文案(i18n 两语)。构建阶段不开线程、不碰
/// Win32;原生 HMENU 只是这份模型的机械投影。
fn build_menu_entries(agents: &[TrayAgent], language: UiLanguage) -> Vec<TrayMenuEntry> {
    let mut entries = Vec::new();
    for (index, agent) in agents.iter().enumerate() {
        // 状态点进文案:菜单不画自定义图形,实心/空心圈已经把「等你」和
        // 「在跑」分开。
        let label = if agent.needs_attention {
            format!(
                "● {} — {}",
                agent.label,
                language.text(Message::TrayWaitingInput)
            )
        } else {
            format!("○ {}", agent.label)
        };
        entries.push(TrayMenuEntry::Item {
            command_id: MENU_AGENT_BASE + index,
            label,
            action: TrayAction::Focus(Some(agent.pane)),
        });
    }
    // 没有 agent 时整组都不存在,避免空提示或孤立分隔线占据菜单顶部。
    if !agents.is_empty() {
        entries.push(TrayMenuEntry::Separator);
    }
    entries.push(TrayMenuEntry::Item {
        command_id: MENU_SHOW,
        label: language.format(Message::TrayShowWindow, &[("app", crate::brand::NAME)]),
        action: TrayAction::Focus(None),
    });
    entries.push(TrayMenuEntry::Item {
        command_id: MENU_QUIT,
        label: language.format(Message::TrayQuit, &[("app", crate::brand::NAME)]),
        action: TrayAction::Quit,
    });
    entries
}

/// 图标变体由 attention 聚合驱动:任一 pane 等输入即橙点态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayIcon {
    Normal,
    Attention,
}

fn icon_variant(agents: &[TrayAgent]) -> TrayIcon {
    if agents.iter().any(|agent| agent.needs_attention) {
        TrayIcon::Attention
    } else {
        TrayIcon::Normal
    }
}

/// 通知区挂载动作(纯函数):未挂载且开启 → 每次触发都重试 `NIM_ADD`,
/// 直到成功;关闭态只需删一次。B54 的防复发点在此。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MountAction {
    Add,
    Remove,
    None,
}

fn mount_action(enabled: bool, added: bool) -> MountAction {
    match (enabled, added) {
        (true, false) => MountAction::Add,
        (false, true) => MountAction::Remove,
        _ => MountAction::None,
    }
}

/// 无品牌资产时的占位图标:中性灰实心圆盘。选中性灰而非主题色——
/// 托盘图标不知道主题,且灰盘在亮暗任务栏上都能与橙点形成对比。
fn placeholder_icon_pixels(size: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (size as usize) * (size as usize) * 4];
    let radius = size as f32 * 0.42;
    let center = size as f32 / 2.0 - 0.5;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            // 1px 羽化边,免得小尺寸下是一颗锯齿方块。
            let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let index = ((y * size + x) * 4) as usize;
            pixels[index] = 96;
            pixels[index + 1] = 96;
            pixels[index + 2] = 96;
            pixels[index + 3] = (255.0 * coverage) as u8;
        }
    }
    pixels
}

/// 品牌图标像素:内嵌 PNG(与随包 `assets/app-icon.png` 同源),按目标
/// 尺寸重采样。解码失败返回 None,调用方退到占位圆盘。
fn brand_icon_pixels(size: u32) -> Option<Vec<u8>> {
    static PNG: &[u8] = include_bytes!("../../assets/app-icon.png");
    let decoded = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?;
    let resized = decoded.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
    Some(resized.to_rgba8().into_raw())
}

/// 右下角实心橙点(约 5/8 直径比)。挑警示橙而不是主题 accent:托盘
/// 图标不知道主题,且橙点在亮暗任务栏上对比都够。`pixels` = 直 alpha RGBA。
fn draw_attention_dot(pixels: &mut [u8], size: u32) {
    let radius = size as f32 * 0.30;
    let center = size as f32 - radius - 0.5;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let index = ((y * size + x) * 4) as usize;
            let pixel = &mut pixels[index..index + 4];
            let blend = |src: u8, dst: u8| -> u8 {
                (src as f32 * coverage + dst as f32 * (1.0 - coverage)) as u8
            };
            pixel[0] = blend(255, pixel[0]);
            pixel[1] = blend(140, pixel[1]);
            pixel[2] = blend(0, pixel[2]);
            pixel[3] = blend(255, pixel[3]);
        }
    }
}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::{Mutex, MutexGuard};

    use super::{
        GpuiTrayCommand, TrayAction, TrayIcon, TrayMenuEntry, WM_APP_REFRESH, WM_APP_SET,
        WM_APP_SHUTDOWN, WM_APP_TRAY, action_for_command, build_menu_entries, focus_best,
        icon_variant, lock_state,
    };
    use crate::i18n::Message;

    /// 托盘窗口句柄(isize 形态跨线程存取;0 = 线程未起或窗口未建)。
    static HWND: AtomicIsize = AtomicIsize::new(0);
    static THREAD_STARTED: AtomicBool = AtomicBool::new(false);
    /// 收尾标志:窗口尚未建好就收到 shutdown 时,线程建完窗立刻退出。
    static SHUTDOWN: AtomicBool = AtomicBool::new(false);
    /// 托盘线程句柄:收尾时 join,保证图标删干净后才回到进程退出路径。
    static THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

    fn thread_slot() -> MutexGuard<'static, Option<std::thread::JoinHandle<()>>> {
        THREAD.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn post(message: u32) {
        let hwnd = HWND.load(Ordering::Acquire);
        if hwnd != 0 {
            // SAFETY: hwnd 由托盘线程发布且只在 WM_APP_SHUTDOWN 后失效;
            // PostMessageW 对已销毁窗口安全失败。
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd as _, message, 0, 0);
            }
        }
    }

    pub(super) fn ensure_thread() {
        if THREAD_STARTED.swap(true, Ordering::SeqCst) {
            return;
        }
        match std::thread::Builder::new()
            .name("slterm-tray".into())
            .spawn(tray_thread)
        {
            Ok(handle) => *thread_slot() = Some(handle),
            Err(error) => {
                THREAD_STARTED.store(false, Ordering::SeqCst);
                log::warn!("tray: failed to spawn tray thread: {error}");
            }
        }
    }

    pub(super) fn shutdown() {
        SHUTDOWN.store(true, Ordering::SeqCst);
        post(WM_APP_SHUTDOWN);
        if let Some(handle) = thread_slot().take() {
            // 泵在 WM_APP_SHUTDOWN 里 PostQuitMessage;join 保证删图标先于
            // 进程退出(否则任务栏残影要等用户悬停才消失)。
            let _ = handle.join();
        }
    }

    // ─── 托盘线程 ────────────────────────────────────────────────────────

    /// 托盘线程本地资源:两个状态图标 + 图标是否已挂在通知区。
    struct Icons {
        normal: isize,
        attention: isize,
        added: bool,
    }

    impl Drop for Icons {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(self.normal as _);
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(self.attention as _);
            }
        }
    }

    thread_local! {
        static ICONS: RefCell<Option<Icons>> = const { RefCell::new(None) };
    }

    fn tray_thread() {
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CW_USEDEFAULT, CreateWindowExW, DispatchMessageW, GetMessageW, MSG, RegisterClassW,
            TranslateMessage, WNDCLASSW, WS_OVERLAPPED,
        };

        let class_name = wide("SltermTrayWindow");
        let window_title = wide(crate::brand::NAME);
        // SAFETY: 类名/实例句柄在调用期间有效;重复注册返回 0 也无妨——
        // 单进程只会走到这里一次(THREAD_STARTED 闸)。
        let hwnd = unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(tray_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };
            RegisterClassW(&class);
            // 隐藏顶层窗口(不是 message-only:通知区回调对 HWND_MESSAGE
            // 窗口在部分 Windows 版本上不投递)。永不 ShowWindow。
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_title.as_ptr(),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            // B48:窗口创建失败必须复位闸门,让后续 set_enabled/init 重试;
            // 否则本进程内托盘入口永久缺失(静默启动下窗口已隐藏)。
            THREAD_STARTED.store(false, Ordering::SeqCst);
            log::warn!("tray: CreateWindowExW failed; tray icon disabled (will retry)");
            return;
        }
        HWND.store(hwnd as isize, Ordering::Release);

        // 建窗与 shutdown 竞态:shutdown 先到则不留图标直接退出。
        if SHUTDOWN.load(Ordering::SeqCst) {
            return;
        }

        let icons = build_icons();
        ICONS.with(|slot| {
            *slot.borrow_mut() = Some(Icons {
                normal: icons.0 as isize,
                attention: icons.1 as isize,
                added: false,
            })
        });
        // init 与首次 set_enabled 之间可能已经积累了状态:起跑先对齐。
        apply_enabled(hwnd);

        let mut message: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: 本线程的消息泵;GetMessageW 返回 0(WM_QUIT)或 -1 退出。
        while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: windows_sys::Win32::Foundation::HWND,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DefWindowProcW, PostQuitMessage, WM_LBUTTONUP, WM_RBUTTONUP,
        };
        match message {
            WM_APP_TRAY => {
                match lparam as u32 {
                    // 左键 = 最短路径:直达最需要人的 pane。
                    WM_LBUTTONUP => focus_best(),
                    WM_RBUTTONUP => show_menu(hwnd),
                    _ => {}
                }
                0
            }
            WM_APP_REFRESH => {
                // 先 apply_enabled:NIM_ADD 曾失败(explorer 未起)时借下一次
                // 快照变化自愈,不必等用户重开托盘开关。挂载动作本身已刷新
                // 图标;已挂载时再按新快照刷新一次。
                if !apply_enabled(hwnd) {
                    refresh_icon(hwnd);
                }
                0
            }
            WM_APP_SET => {
                apply_enabled(hwnd);
                0
            }
            WM_APP_SHUTDOWN => {
                remove_icon(hwnd);
                // SAFETY: 本线程的消息泵,退出后线程结束。
                unsafe { PostQuitMessage(0) };
                0
            }
            // SAFETY: 转发给默认过程,参数原样透传。
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    fn show_menu(hwnd: windows_sys::Win32::Foundation::HWND) {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DestroyMenu, GetCursorPos, PostMessageW, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
            TrackPopupMenu, WM_NULL,
        };

        // 菜单期间用快照:TrackPopupMenu 是模态泵,STATE 可能被 1 Hz 更新,
        // 命令 id 必须映射回打开菜单那一刻的清单。
        let (agents, language) = {
            let shared = lock_state();
            (shared.agents.clone(), shared.language)
        };
        let entries = build_menu_entries(&agents, language);

        // SAFETY: 菜单由本函数持有/销毁。owner 前置 + WM_NULL 保留
        // TrackPopupMenu 在托盘场景中点外部收合的行为;Win32 焦点调用
        // 收在 windowing facade。
        unsafe {
            let menu = build_native_menu(&entries);
            if menu.is_null() {
                return;
            }
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point);
            crate::windowing::focus::foreground_menu_owner(hwnd as isize);
            let command = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                point.x,
                point.y,
                0,
                hwnd,
                std::ptr::null(),
            ) as usize;
            PostMessageW(hwnd, WM_NULL, 0, 0);
            DestroyMenu(menu);

            if let Some(action) = action_for_command(command, &agents) {
                dispatch_action(action);
            }
        }
    }

    fn dispatch_action(action: TrayAction) {
        match action {
            TrayAction::Focus(pane) => super::send_command(GpuiTrayCommand::Focus(pane)),
            TrayAction::Quit => super::send_command(GpuiTrayCommand::Quit),
        }
    }

    /// 纯模型 → 原生 HMENU。构建与模态展示分离:回归测试不开线程验真实
    /// 菜单结构,id 映射回调用方传入的那份快照。
    fn build_native_menu(
        entries: &[TrayMenuEntry],
    ) -> windows_sys::Win32::UI::WindowsAndMessaging::HMENU {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, MF_SEPARATOR, MF_STRING,
        };

        // SAFETY: 文案缓冲活过 AppendMenuW(菜单复制内容);调用方负责销毁。
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return menu;
            }
            for entry in entries {
                match entry {
                    TrayMenuEntry::Separator => {
                        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                    }
                    TrayMenuEntry::Item {
                        command_id, label, ..
                    } => {
                        let text = wide(label);
                        AppendMenuW(menu, MF_STRING, *command_id, text.as_ptr());
                    }
                }
            }
            menu
        }
    }

    // ─── 通知区图标 ──────────────────────────────────────────────────────

    fn notify_data(
        hwnd: windows_sys::Win32::Foundation::HWND,
    ) -> windows_sys::Win32::UI::Shell::NOTIFYICONDATAW {
        // SAFETY: NOTIFYICONDATAW 是纯 POD,零值是合法起点。
        let mut data: windows_sys::Win32::UI::Shell::NOTIFYICONDATAW =
            unsafe { std::mem::zeroed() };
        data.cbSize = std::mem::size_of::<windows_sys::Win32::UI::Shell::NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data
    }

    /// 依据 STATE 计算图标形态并 NIM_MODIFY;未挂图标时什么都不做。
    fn refresh_icon(hwnd: windows_sys::Win32::Foundation::HWND) {
        use windows_sys::Win32::UI::Shell::{NIF_ICON, NIF_TIP, NIM_MODIFY, Shell_NotifyIconW};

        let (variant, agent_count, attention_count, language) = {
            let shared = lock_state();
            (
                icon_variant(&shared.agents),
                shared.agents.len(),
                shared.attention_count(),
                shared.language,
            )
        };
        let tip = if agent_count == 0 {
            crate::brand::NAME.to_owned()
        } else if attention_count > 0 {
            language.format(
                Message::TrayTipWaiting,
                &[
                    ("app", crate::brand::NAME),
                    ("count", &attention_count.to_string()),
                ],
            )
        } else {
            language.format(
                Message::TrayTipRunning,
                &[
                    ("app", crate::brand::NAME),
                    ("count", &agent_count.to_string()),
                ],
            )
        };
        ICONS.with(|slot| {
            let slot = slot.borrow();
            let Some(icons) = slot.as_ref().filter(|icons| icons.added) else {
                return;
            };
            let mut data = notify_data(hwnd);
            data.uFlags = NIF_ICON | NIF_TIP;
            data.hIcon = match variant {
                TrayIcon::Attention => icons.attention,
                TrayIcon::Normal => icons.normal,
            } as _;
            copy_tip(&mut data.szTip, &tip);
            // SAFETY: data 完整初始化且 hwnd 属于本线程。
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
        });
    }

    /// 对齐启用态与通知区挂载态;返回本次是否新挂载成功。
    ///
    /// B54:`NIM_ADD` 失败(explorer 未就绪/刚崩)时保持 `added=false`,
    /// 下一次 `WM_APP_SET`(1 Hz 快照每次都会投递)或 `WM_APP_REFRESH`
    /// 继续重试,直到成功。
    fn apply_enabled(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
        use windows_sys::Win32::UI::Shell::{
            NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, Shell_NotifyIconW,
        };

        let enabled = lock_state().enabled;
        let mut added_now = false;
        ICONS.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(icons) = slot.as_mut() else {
                return;
            };
            match super::mount_action(enabled, icons.added) {
                super::MountAction::Add => {
                    let mut data = notify_data(hwnd);
                    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
                    data.uCallbackMessage = WM_APP_TRAY;
                    data.hIcon = icons.normal as _;
                    copy_tip(&mut data.szTip, crate::brand::NAME);
                    // SAFETY: data 完整初始化且 hwnd 属于本线程。
                    if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } != 0 {
                        icons.added = true;
                        added_now = true;
                    }
                }
                super::MountAction::Remove => {
                    let data = notify_data(hwnd);
                    // SAFETY: 删除只需要 hWnd/uID。
                    unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
                    icons.added = false;
                }
                super::MountAction::None => {}
            }
        });
        // 只有新挂载成功才补图标/提示;已挂载的 1 Hz 重复 WM_APP_SET 不做
        // 无谓的 NIM_MODIFY(快照变化仍走 WM_APP_REFRESH)。
        if added_now {
            refresh_icon(hwnd);
        }
        added_now
    }

    fn remove_icon(hwnd: windows_sys::Win32::Foundation::HWND) {
        use windows_sys::Win32::UI::Shell::{NIM_DELETE, Shell_NotifyIconW};

        ICONS.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(icons) = slot.as_mut().filter(|icons| icons.added) {
                let data = notify_data(hwnd);
                // SAFETY: 同 apply_enabled 的删除分支。
                unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
                icons.added = false;
            }
        });
    }

    // ─── 图标像素 ────────────────────────────────────────────────────────

    /// 造两个 HICON:常态 + attention(右下角橙点)。品牌资产缺位时用
    /// 中性灰占位圆盘;失败退化为空句柄——Shell_NotifyIconW 会用系统
    /// 默认图标,托盘仍可用。
    fn build_icons() -> (
        windows_sys::Win32::UI::WindowsAndMessaging::HICON,
        windows_sys::Win32::UI::WindowsAndMessaging::HICON,
    ) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};

        // SAFETY: GetSystemMetrics 无前置条件。
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.clamp(16, 64) as u32;
        let base =
            super::brand_icon_pixels(size).unwrap_or_else(|| super::placeholder_icon_pixels(size));
        let normal = create_icon(size, &base);

        let mut attention_pixels = base;
        super::draw_attention_dot(&mut attention_pixels, size);
        let attention = create_icon(size, &attention_pixels);
        (normal, attention)
    }

    /// RGBA(直 alpha)→ HICON:32bpp 预乘 BGRA DIB + 单色掩码。
    fn create_icon(size: u32, rgba: &[u8]) -> windows_sys::Win32::UI::WindowsAndMessaging::HICON {
        use windows_sys::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
            DeleteObject,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};

        // SAFETY: header 描述的缓冲由 CreateDIBSection 分配并在拷贝期间有效;
        // 两个位图在 CreateIconIndirect 后立即释放(图标持有自己的副本)。
        unsafe {
            let mut header: BITMAPINFOHEADER = std::mem::zeroed();
            header.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            header.biWidth = size as i32;
            // 负高 = 自顶向下,行序与解码像素一致,免得手工翻转。
            header.biHeight = -(size as i32);
            header.biPlanes = 1;
            header.biBitCount = 32;
            header.biCompression = BI_RGB;
            let info = BITMAPINFO {
                bmiHeader: header,
                bmiColors: std::mem::zeroed(),
            };

            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let color = CreateDIBSection(
                std::ptr::null_mut(),
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if color.is_null() || bits.is_null() {
                return std::ptr::null_mut();
            }
            let output = std::slice::from_raw_parts_mut(bits.cast::<u8>(), rgba.len());
            for (destination, source) in output.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                let alpha = source[3] as u32;
                // 预乘:DWM 按 premultiplied 合成托盘图标,直 alpha 会在
                // 深色任务栏上泛白边。
                destination[0] = (source[2] as u32 * alpha / 255) as u8;
                destination[1] = (source[1] as u32 * alpha / 255) as u8;
                destination[2] = (source[0] as u32 * alpha / 255) as u8;
                destination[3] = source[3];
            }

            let mask = CreateBitmap(size as i32, size as i32, 1, 1, std::ptr::null());
            let icon_info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            let icon = CreateIconIndirect(&icon_info);
            DeleteObject(color);
            DeleteObject(mask);
            icon
        }
    }

    // ─── 小工具 ─────────────────────────────────────────────────────────

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// UTF-16 截断拷贝进 szTip(128 wchar,含 NUL)。
    fn copy_tip(tip: &mut [u16; 128], text: &str) {
        let mut cursor = 0;
        for unit in text.encode_utf16() {
            if cursor >= tip.len() - 1 {
                break;
            }
            tip[cursor] = unit;
            cursor += 1;
        }
        tip[cursor] = 0;
    }

    #[cfg(test)]
    mod native_menu_tests {
        use super::super::{MENU_AGENT_BASE, MENU_QUIT, MENU_SHOW, TrayAgent};
        use super::{build_menu_entries, build_native_menu};
        use slterm_split::PaneId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DestroyMenu, GetMenuItemCount, GetMenuItemID, GetMenuState, GetMenuStringW, HMENU,
            MF_BYPOSITION, MF_SEPARATOR,
        };

        struct Menu(HMENU);

        impl Drop for Menu {
            fn drop(&mut self) {
                // SAFETY: 测试独占此菜单,断言失败时也要释放原生句柄。
                unsafe { DestroyMenu(self.0) };
            }
        }

        /// 菜单构建不开线程:对快照清单产出 HMENU,命令 id 映射回打开
        /// 那一刻的清单(空白组不出现,show/quit 恒定在尾部)。
        #[test]
        fn native_menu_encodes_agent_snapshot_and_commands_without_a_thread() {
            let agents = [
                TrayAgent {
                    pane: PaneId(11),
                    label: "Codex".into(),
                    needs_attention: false,
                },
                TrayAgent {
                    pane: PaneId(22),
                    label: "Claude".into(),
                    needs_attention: true,
                },
            ];
            for entries in [&[][..], &agents[..]] {
                let model = build_menu_entries(entries, crate::i18n::UiLanguage::EnUs);
                let menu = Menu(build_native_menu(&model));
                assert!(!menu.0.is_null());
                let show_index = if entries.is_empty() { 0 } else { 3 };
                // SAFETY: 句柄有效,索引受构建的菜单长度约束,字符缓冲可写。
                unsafe {
                    assert_eq!(
                        GetMenuItemCount(menu.0),
                        show_index + 2,
                        "agent 组 + show + quit"
                    );
                    assert_eq!(GetMenuItemID(menu.0, show_index), MENU_SHOW as u32);
                    assert_eq!(GetMenuItemID(menu.0, show_index + 1), MENU_QUIT as u32);
                    if !entries.is_empty() {
                        assert_eq!(GetMenuItemID(menu.0, 0), MENU_AGENT_BASE as u32);
                        assert_eq!(GetMenuItemID(menu.0, 1), (MENU_AGENT_BASE + 1) as u32);
                        assert_ne!(
                            GetMenuState(menu.0, 2, MF_BYPOSITION) & MF_SEPARATOR,
                            0,
                            "agent 组后必须有分隔线"
                        );
                        let mut text = [0u16; 128];
                        let count = GetMenuStringW(
                            menu.0,
                            1,
                            text.as_mut_ptr(),
                            text.len() as i32,
                            MF_BYPOSITION,
                        );
                        assert_eq!(
                            String::from_utf16(&text[..count as usize]).unwrap(),
                            "● Claude — waiting for input"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tray_tests {
    use super::*;

    fn agent(pane: u64, label: &str, needs_attention: bool) -> TrayAgent {
        TrayAgent {
            pane: PaneId(pane),
            label: label.to_owned(),
            needs_attention,
        }
    }

    /// C24:内嵌品牌图标必须可解码且非全透明,否则托盘会静默退回占位圆盘。
    #[test]
    fn brand_icon_decodes_to_rgba_pixels() {
        for size in [16, 32, 48, 256] {
            let pixels = brand_icon_pixels(size).expect("embedded icon must decode");
            assert_eq!(pixels.len(), (size * size * 4) as usize);
            assert!(
                pixels.chunks_exact(4).any(|pixel| pixel[3] > 0),
                "size {size} 不得全透明"
            );
        }
    }

    /// B54 防复发:开启态即使状态未翻转也必须继续投递挂载请求,让 1 Hz
    /// 快照成为 `NIM_ADD` 失败后的重试节拍;关闭态翻转才投递。
    #[test]
    fn enabling_the_tray_reposts_mount_requests_until_mounted() {
        reset_for_test();
        set_enabled(true);
        assert!(
            take_posts_for_test().contains(&WM_APP_SET),
            "首次开启必须请求挂载"
        );

        // 旧实现只在 enabled 翻转时投递;explorer 未就绪导致 NIM_ADD 失败后
        // 这里不会再有消息,托盘图标静默缺失(B54)。
        set_enabled(true);
        assert!(
            take_posts_for_test().contains(&WM_APP_SET),
            "状态未变的 1 Hz 调用仍必须请求重试挂载"
        );

        set_enabled(false);
        assert!(
            take_posts_for_test().contains(&WM_APP_SET),
            "关闭必须请求删除图标"
        );
        set_enabled(false);
        assert!(
            take_posts_for_test().is_empty(),
            "关闭态重复调用不得空转打扰托盘线程"
        );
        reset_for_test();
    }

    /// B54 决策面:未挂载且开启时每次调用都返回 Add;挂载成功后退到 None。
    #[test]
    fn mount_action_retries_add_while_the_icon_is_not_mounted() {
        assert_eq!(mount_action(true, false), MountAction::Add);
        assert_eq!(
            mount_action(true, false),
            MountAction::Add,
            "未挂载的重复触发必须继续重试"
        );
        assert_eq!(mount_action(true, true), MountAction::None);
        assert_eq!(mount_action(false, true), MountAction::Remove);
        assert_eq!(mount_action(false, false), MountAction::None);
    }

    /// 1 Hz 去抖:同一份快照不推送;内容(含语言)变化才推送;托盘关闭时
    /// 只更新内部清单、不打扰线程。
    #[test]
    fn snapshot_updates_are_debounced_and_disabled_tray_stays_silent() {
        reset_for_test();
        set_enabled(true);
        let first = vec![agent(1, "claude · app", false)];
        update(first.clone(), UiLanguage::EnUs);
        assert_eq!(take_updates_for_test().len(), 1, "首次快照必须推送");

        update(first.clone(), UiLanguage::EnUs);
        assert!(
            take_updates_for_test().is_empty(),
            "内容不变的 1 Hz 快照不得重复推送"
        );

        let attention = vec![agent(1, "claude · app", true)];
        update(attention.clone(), UiLanguage::EnUs);
        assert_eq!(take_updates_for_test().len(), 1, "attention 翻转必须推送");

        update(attention.clone(), UiLanguage::ZhCn);
        assert_eq!(take_updates_for_test().len(), 1, "语言变化必须推送");

        set_enabled(false);
        update(vec![agent(2, "codex · app", false)], UiLanguage::EnUs);
        assert!(
            take_updates_for_test().is_empty(),
            "关闭态只更新清点,不驱动托盘线程"
        );
        reset_for_test();
    }

    /// attention 聚合决定双图标选择:任一 pane 等输入即橙点态。
    #[test]
    fn attention_aggregation_flips_the_icon_variant() {
        assert_eq!(icon_variant(&[]), TrayIcon::Normal);
        assert_eq!(
            icon_variant(&[agent(1, "claude", false), agent(2, "codex", false)]),
            TrayIcon::Normal
        );
        assert_eq!(
            icon_variant(&[agent(1, "claude", false), agent(2, "codex", true)]),
            TrayIcon::Attention
        );
    }

    /// 左键兜底:等输入的优先,其次任意 agent,最后裸窗口。
    #[test]
    fn best_agent_prefers_waiting_then_any_then_none() {
        assert_eq!(best_agent(&[]), None);
        assert_eq!(
            best_agent(&[agent(1, "claude", false), agent(2, "codex", false)]),
            Some(PaneId(1))
        );
        assert_eq!(
            best_agent(&[agent(1, "claude", false), agent(2, "codex", true)]),
            Some(PaneId(2))
        );
    }

    /// 菜单构建纯函数:空组不占位;两语文案都带状态点;动作与命令 id
    /// 映射回当次快照。
    #[test]
    fn menu_entries_are_localized_and_keep_snapshot_actions() {
        let agents = [
            agent(7, "claude · app", false),
            agent(9, "codex · app", true),
        ];

        let english = build_menu_entries(&agents, UiLanguage::EnUs);
        assert_eq!(
            english,
            vec![
                TrayMenuEntry::Item {
                    command_id: MENU_AGENT_BASE,
                    label: "○ claude · app".into(),
                    action: TrayAction::Focus(Some(PaneId(7))),
                },
                TrayMenuEntry::Item {
                    command_id: MENU_AGENT_BASE + 1,
                    label: "● codex · app — waiting for input".into(),
                    action: TrayAction::Focus(Some(PaneId(9))),
                },
                TrayMenuEntry::Separator,
                TrayMenuEntry::Item {
                    command_id: MENU_SHOW,
                    label: "Show slTerminal".into(),
                    action: TrayAction::Focus(None),
                },
                TrayMenuEntry::Item {
                    command_id: MENU_QUIT,
                    label: "Quit slTerminal".into(),
                    action: TrayAction::Quit,
                },
            ]
        );

        let chinese = build_menu_entries(&agents[1..], UiLanguage::ZhCn);
        assert_eq!(
            chinese[0],
            TrayMenuEntry::Item {
                command_id: MENU_AGENT_BASE,
                label: "● codex · app — 等待输入".into(),
                action: TrayAction::Focus(Some(PaneId(9))),
            }
        );
        assert_eq!(
            chinese[2],
            TrayMenuEntry::Item {
                command_id: MENU_SHOW,
                label: "显示 slTerminal".into(),
                action: TrayAction::Focus(None),
            }
        );
        assert_eq!(
            chinese[3],
            TrayMenuEntry::Item {
                command_id: MENU_QUIT,
                label: "退出 slTerminal".into(),
                action: TrayAction::Quit,
            }
        );

        // 空清单:没有 agent 组与孤立分隔线,show/quit 仍在。
        let empty = build_menu_entries(&[], UiLanguage::EnUs);
        assert_eq!(empty.len(), 2);
        assert!(matches!(
            &empty[0],
            TrayMenuEntry::Item {
                command_id: MENU_SHOW,
                ..
            }
        ));
    }

    /// 命令 id → 动作:show/quit 固定;agent id 越界(菜单打开后条目被
    /// 关掉)返回 None,不误聚焦别的 pane。
    #[test]
    fn command_ids_resolve_against_the_snapshot_taken_at_open_time() {
        let agents = [agent(3, "claude", false), agent(4, "codex", true)];
        assert_eq!(
            action_for_command(MENU_SHOW, &agents),
            Some(TrayAction::Focus(None))
        );
        assert_eq!(
            action_for_command(MENU_QUIT, &agents),
            Some(TrayAction::Quit)
        );
        assert_eq!(
            action_for_command(MENU_AGENT_BASE + 1, &agents),
            Some(TrayAction::Focus(Some(PaneId(4))))
        );
        assert_eq!(action_for_command(MENU_AGENT_BASE + 2, &agents), None);
        assert_eq!(action_for_command(0, &agents), None);
    }

    /// 占位图标:中心有像素、左上角透明;橙点只污染右下角。
    #[test]
    fn attention_dot_paints_only_the_bottom_right_corner() {
        let size = 16;
        let mut pixels = placeholder_icon_pixels(size);
        assert_eq!(pixels.len(), (size * size * 4) as usize);
        let center = ((8 * size + 8) * 4) as usize;
        assert_eq!(pixels[center + 3], 255, "圆盘中心必须不透明白");
        assert_eq!(pixels[3], 0, "左上角在圆外,alpha 为 0");

        let before = pixels.clone();
        draw_attention_dot(&mut pixels, size);
        let dot_center = ((11 * size + 11) * 4) as usize;
        assert_eq!(
            &pixels[dot_center..dot_center + 3],
            &[255, 140, 0],
            "橙点中心必须是警示橙"
        );
        assert_eq!(&pixels[0..4], &before[0..4], "橙点不得波及左上角");
    }
}
