// preview.rs — docViewer 预览渲染独立 WebviewWindow 管理（S10-② 迁移，ADR-0019）
//
// docViewer 面板（htmlviewer/markdownviewer）的预览内容不再经主窗口内 sandbox
// iframe srcDoc，改在独立 WebviewWindow（label = preview-<panelId>）中渲染——
// 预览窗口加载专用宿主页，宿主页由自定义协议域提供（scheme slterm-preview，
// tauri 在 Windows 映射为 http://slterm-preview.localhost/…，register_uri_scheme
// _protocol 文档实证）：
//
//   - 主窗口 CSP 收紧（CP-012，script-src 'self'）后资产协议页仍被注入全局 CSP，
//     srcdoc 子文档继承后运行时内联脚本全灭——tauri 2.11 无 per-webview CSP
//     （spike 实证）。本自定义协议响应不经 tauri 资产处理器，不带全局 CSP 头 →
//     「预览 CSP 域」即本域：注入机制（injectScript + buildInjectedScript + nonce）
//     原样迁入域内宽松执行（CP-031 判定二/三由此可达）。
//   - 宿主页 = 固定桥接页：建 sandbox iframe（allow-scripts、无 allow-same-origin
//     ——Tauri CVE-2024-35222 红线延续）+ 内容注入 + 上下行中继；iframe 内容
//     自带 <script> 不经字符串转义进入渲染文档（escapeScriptClose 消亡，CP-031）。
//   - 消息桥 = Tauri event/IPC（跨独立窗口无 window.postMessage——spike 实测，
//     CP-044「通道退役」分支）：上行 zoom/scroll/nav + 宿主状态事件；下行
//     reset/zoom_set/scroll_set；无命令/按键重放（CP-013）。
//
// 窗口形态：主窗口 owned 无边框窗口（跟随主窗最小化/置顶），focusable(false)
// ——键盘焦点恒留主窗口（ShortcutRegistry window capture 域），预览聚焦不吞
// 全局快捷键（CP-013 步骤 4 口径）；代价：预览文档内表单键入/系统复制快捷键
// 不可达，登记已知行为（docViewer/CLAUDE.md）。几何 = 主窗 inner 原点 +
// CSS 尺寸 × scale（前端锚定面板内容区后经 preview_sync 驱动）。

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;
use tauri::{Emitter, EventTarget, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::AppError;

/// 预览窗口 label 前缀（前端/WDIO 按 preview-<panelId> 寻址——spike 驱动契约）
pub const PREVIEW_LABEL_PREFIX: &str = "preview-";

/// 宿主页自定义协议 scheme（Windows 上 tauri 注册为 http://<scheme>.localhost/…）
pub const PREVIEW_SCHEME: &str = "slterm-preview";

/// 宿主页地址（Windows 形态：http://<scheme>.localhost/<path>，register_uri_scheme
/// _protocol 文档实证；main 配置窗口无 label → 默认 "main"）
pub const PREVIEW_HOST_URL: &str = "http://slterm-preview.localhost/preview-host.html";

/// 主窗口 label（tauri.conf.json windows[0] 未命名 → 默认 "main"）
const MAIN_WINDOW_LABEL: &str = "main";

/// 内容就绪通知事件（preview_render 存内容后定向通知宿主拉取）
pub const PREVIEW_RENDER_PING_EVENT: &str = "preview:render-ping";

/// label 合法字符集：ASCII 字母数字 + _ - : /（对齐 tauri 窗口 label 合法集
/// tauri-runtime-2.11.3 window.rs:534；panelId 页前缀协议形态 {pageId}:{localId}
/// 必含 ":"，SEC-01）
fn validate_label(label: &str) -> Result<(), AppError> {
    if !label.starts_with(PREVIEW_LABEL_PREFIX) {
        return Err(AppError::Validation(format!(
            "预览窗口 label 必须以 {PREVIEW_LABEL_PREFIX} 开头: {label}"
        )));
    }
    let rest = &label[PREVIEW_LABEL_PREFIX.len()..];
    if rest.is_empty()
        || rest.len() > 96
        || !rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':' || b == b'/')
    {
        return Err(AppError::Validation(format!("非法预览窗口 label: {label}")));
    }
    Ok(())
}

/// 单面板渲染内容（宿主页拉取；seq 递增防乱序/防陈旧覆盖）
struct StoredContent {
    seq: u64,
    /// 最终注入产物（injectScript 装配后的完整文档——含注入脚本）
    html: String,
    /// iframe 背景压闪白（html 面板传；缺省透明）
    bg: Option<String>,
}

/// 渲染内容存储（模块级——无跨模块共享，纯窗口域状态）
static CONTENT_STORE: LazyLock<Mutex<HashMap<String, StoredContent>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 预览窗口会话态（label → 会话）：「销毁后复活」僵尸的守卫状态——
/// 2026-09-08 归因：PreviewFrame 200ms 几何轮询的 in-flight previewSync 与
/// 卸载 cleanup 的 previewClose 并发——destroy 落主线程后迟到的 sync 命中
/// 「窗口不存在 + visible → 无条件重建」分支（2026-09-08 前实现）→ 僵尸孤儿
/// 窗口复活入列（e2e 句柄集 100ms 后复见）。修：前端挂载期生成随机 token 随
/// sync/close 请求传递，后端按 label 记录当前 token 与 closed 态——close 后
/// 同 token 迟到 sync 拒绝重建；新 token（真重挂载）放行。
struct WindowSession {
    /// 当前受理挂载的 token
    token: String,
    /// 当前 token 是否已 close（close 后同 token 迟到 sync 拒绝重建）
    closed: bool,
    /// 已退役 token（受理过 close / 被新挂载取代的旧 token）——迟到 sync
    /// 一律拒绝，防跨命令乱序下旧挂载的迟到 sync 复活/劫持新会话窗口
    /// （IPC 通道同 webview FIFO，正常时序到不了此面；保留为防御纵深）
    retired: Vec<String>,
}

/// 会话态退役 token 保留上限（防无界增长——每 label 仅存最近几代）
const MAX_RETIRED_TOKENS: usize = 4;

static SESSION_STATE: LazyLock<Mutex<HashMap<String, WindowSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// sync 放行判定（纯逻辑——命令与 L1 测试共用）。须在窗口域操作闭包
/// （run_on_main 主线程临界）内调用——与建窗/销毁同临界，防与 close 交错：
/// - 同 token 已 close / 已退役 token → 拒绝（迟到 sync，不重建窗口）
/// - 同 token 未 close → 放行（常规几何/显隐同步）
/// - 异 token（新挂载，旧会话被取代）→ 退役旧 token、登记并放行
fn session_sync_allowed(
    state: &mut HashMap<String, WindowSession>,
    label: &str,
    token: &str,
) -> bool {
    let entry = state.entry(label.to_string()).or_insert(WindowSession {
        token: token.to_string(),
        closed: false,
        retired: Vec::new(),
    });
    if entry.retired.iter().any(|t| t == token) {
        // 已退役 token：旧挂载的迟到 sync——拒绝（防复活/劫持）
        return false;
    }
    if entry.token == token {
        // 当前会话：close 后迟到 sync 拒绝；未 close 放行
        return !entry.closed;
    }
    // 新挂载：退役旧 token 并接管会话
    entry.retired.push(entry.token.clone());
    if entry.retired.len() > MAX_RETIRED_TOKENS {
        entry
            .retired
            .drain(..entry.retired.len() - MAX_RETIRED_TOKENS);
    }
    entry.token = token.to_string();
    entry.closed = false;
    true
}

/// close 受理判定（纯逻辑——命令与 L1 测试共用）。同样须在窗口域操作闭包内
/// 调用：同 token → 标记 closed 并受理（销毁 + 清内容）；未知会话 → 登记
/// closed 并受理（防御：从未 sync 直接 close 的形态）；异 token（旧会话迟到
/// close 或新挂载被其取代后）→ 拒绝（不破坏已取代它的新会话窗口与内容）。
fn session_close_claim(
    state: &mut HashMap<String, WindowSession>,
    label: &str,
    token: &str,
) -> bool {
    match state.get_mut(label) {
        Some(s) if s.token == token => {
            s.closed = true;
            true
        }
        Some(s) => {
            // 异 token 的迟到 close：登记其 token 为已退役（连带拒其迟到 sync），
            // 不触碰当前会话
            if !s.retired.iter().any(|t| t == token) {
                s.retired.push(token.to_string());
                if s.retired.len() > MAX_RETIRED_TOKENS {
                    s.retired.drain(..s.retired.len() - MAX_RETIRED_TOKENS);
                }
            }
            false
        }
        None => {
            state.insert(
                label.to_string(),
                WindowSession {
                    token: token.to_string(),
                    closed: true,
                    retired: Vec::new(),
                },
            );
            true
        }
    }
}

/// token 格式校验：非空 + ≤96 字符（语义 = 前端挂载期随机 hex 串；后端只做
/// 存在性/长度防御——空 token 会破坏「新挂载 ≠ 旧会话」判定）
fn validate_token(token: &str) -> Result<(), AppError> {
    if token.is_empty() || token.len() > 96 {
        return Err(AppError::Validation("非法预览会话 token".into()));
    }
    Ok(())
}

/// 查找（或创建）预览窗口——统一入口，所有命令共用
fn webview_window(app: &tauri::AppHandle, label: &str) -> Option<WebviewWindow> {
    app.get_webview_window(label)
}

/// 主窗口 inner 原点 + 缩放（预览几何换算依据；缺失时预览命令失败）
fn main_window_geometry(app: &tauri::AppHandle) -> Result<(i32, i32, f64), AppError> {
    let main = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| AppError::Unknown("主窗口不存在".into()))?;
    let pos = main
        .inner_position()
        .map_err(|e| AppError::Unknown(format!("读取主窗口位置失败: {e}")))?;
    let scale = main
        .scale_factor()
        .map_err(|e| AppError::Unknown(format!("读取主窗口缩放失败: {e}")))?;
    Ok((pos.x, pos.y, scale))
}

/// CSS 视口坐标 → 屏幕物理坐标（物理 = inner 原点 + round(css × scale)）
fn to_physical(css: f64, origin: i32, scale: f64) -> i32 {
    origin + (css * scale).round() as i32
}

/// 预览窗口物理矩形换算单点（纯函数，可测）：
/// CSS 视口矩形 + 主窗 inner 原点/scale → (px, py, pw, ph) 物理屏幕矩形。
/// 宽/高经右/下沿坐标差换算（round((x+w)×scale) - round(x×scale)），
/// 避免 x 与 w 各自 round 的累积误差；尺寸下限钳 1px（0 尺寸窗口 OS 拒绝）。
/// build_window 与 preview_sync 更新路径共用——坐标系统一在 Physical*
/// （tauri 2.11 builder 的 .position()/.inner_size() 只收逻辑像素，建窗路径
/// 不得经 builder 传几何，见 build_window）。
fn compute_physical_rect(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    ox: i32,
    oy: i32,
    scale: f64,
) -> (i32, i32, i32, i32) {
    let px = to_physical(x, ox, scale);
    let py = to_physical(y, oy, scale);
    let pw = to_physical(x + width, ox, scale).saturating_sub(px).max(1);
    let ph = to_physical(y + height, oy, scale).saturating_sub(py).max(1);
    (px, py, pw, ph)
}

/// 建窗参数：owned（跟随主窗）、无边框、不可聚焦、任务栏隐藏、初始隐藏
///
/// 几何坐标系红线：tauri 2.11 WebviewWindowBuilder 的 `.position()/.inner_size()`
/// 只收逻辑像素（无 Physical 重载）——物理矩形经 builder 传入会被 OS 再乘
/// scale（HiDPI 必现错位）。故 builder 不传几何，visible(false) 建窗后立即以
/// set_position/set_size 的 Physical* 重载落定（与 preview_sync 更新路径同
/// 坐标系），show 由调用方在几何落定后执行（无闪屏）。
fn build_window(
    app: &tauri::AppHandle,
    label: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<WebviewWindow, AppError> {
    let main = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| AppError::Unknown("主窗口不存在".into()))?;
    let url = WebviewUrl::External(
        PREVIEW_HOST_URL
            .parse()
            .map_err(|e| AppError::Unknown(format!("宿主页地址解析失败: {e}")))?,
    );
    let builder = WebviewWindowBuilder::new(app, label, url)
        .title("slTerminal 预览")
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .skip_taskbar(true)
        .focusable(false)
        .visible(false);
    let builder = builder
        .owner(&main)
        .map_err(|e| AppError::Unknown(format!("预览窗口挂主窗失败: {e}")))?;
    let win = builder
        .build()
        .map_err(|e| AppError::Unknown(format!("创建预览窗口失败: {e}")))?;
    win.set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|e| AppError::Unknown(format!("定位预览窗口失败: {e}")))?;
    win.set_size(tauri::PhysicalSize::new(
        width.max(1) as u32,
        height.max(1) as u32,
    ))
    .map_err(|e| AppError::Unknown(format!("调整预览窗口尺寸失败: {e}")))?;
    win.set_focusable(false)
        .map_err(|e| AppError::Unknown(format!("预览窗口置不可聚焦失败: {e}")))?;
    Ok(win)
}

/// 主线程执行窗口域操作并取回结果。
///
/// tauri 的窗口创建/几何/显隐等操作要求主线程（tao/wry 事件泵）——sync 命令在
/// 主线程执行时于 IPC 回调内创建 WebView2 窗口会等消息泵死锁（2026-09-08 实测
/// 30s execute 超时），故窗口域命令一律 async（tokio 工作线程）+ 经
/// run_on_main_thread 编排回主线程执行。
fn run_on_main<T: Send + 'static>(
    app: &tauri::AppHandle,
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|e| AppError::Unknown(format!("主线程调度失败: {e}")))?;
    rx.recv()
        .map_err(|_| AppError::Unknown("主线程执行预览窗口操作失败".into()))?
}

/// 创建或更新预览窗口几何/显隐（前端每帧感知变化后调用；幂等）
///
/// token = 前端挂载期随机串（随 preview_close 同传）——会话守卫见
/// session_sync_allowed：close 后同 token 迟到 sync 在此拒绝重建（防僵尸）。
/// 参数数量为 IPC 契约形态（几何五元 + 守卫 token），不可折叠。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn preview_sync(
    app: tauri::AppHandle,
    label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    visible: bool,
    token: String,
) -> Result<(), AppError> {
    validate_label(&label)?;
    validate_token(&token)?;
    // 防御：尺寸非法/过小按隐藏处理
    let visible = visible && width >= 1.0 && height >= 1.0;
    let app_owned = app.clone();
    run_on_main(&app, move || {
        // 会话守卫（与建窗/销毁同主线程临界）：close 后同 token 迟到 sync 拒绝
        // 重建——僵尸复活根因的守卫落点
        if !session_sync_allowed(&mut SESSION_STATE.lock(), &label, &token) {
            return Ok(());
        }
        let (ox, oy, scale) = main_window_geometry(&app_owned)?;
        let (px, py, pw, ph) = compute_physical_rect(x, y, width, height, ox, oy, scale);

        match webview_window(&app_owned, &label) {
            Some(win) => {
                win.set_position(tauri::PhysicalPosition::new(px, py))
                    .map_err(|e| AppError::Unknown(format!("定位预览窗口失败: {e}")))?;
                win.set_size(tauri::PhysicalSize::new(pw, ph))
                    .map_err(|e| AppError::Unknown(format!("调整预览窗口尺寸失败: {e}")))?;
                if visible {
                    win.show()
                        .map_err(|e| AppError::Unknown(format!("预览窗口显示失败: {e}")))?;
                } else {
                    win.hide()
                        .map_err(|e| AppError::Unknown(format!("预览窗口隐藏失败: {e}")))?;
                }
            }
            None => {
                // 窗口不存在：可见才创建（隐藏态面板不预建——内容由 render 推送
                // 存储，宿主加载后拉取兜底）
                if visible {
                    let win = build_window(&app_owned, &label, px, py, pw, ph)?;
                    win.show()
                        .map_err(|e| AppError::Unknown(format!("预览窗口显示失败: {e}")))?;
                }
            }
        }
        Ok(())
    })
}

/// 关闭预览窗口并清除内容（面板卸载/预览形态退出时调用）
///
/// token = 与 preview_sync 同传的挂载期随机串——会话守卫见 session_close_claim：
/// 异 token（旧会话迟到的 close）拒绝受理，不破坏已取代它的新会话窗口。
#[tauri::command]
pub async fn preview_close(
    app: tauri::AppHandle,
    label: String,
    token: String,
) -> Result<(), AppError> {
    validate_label(&label)?;
    validate_token(&token)?;
    let app_owned = app.clone();
    run_on_main(&app, move || {
        // 会话守卫（与销毁同主线程临界）：异 token = 旧会话迟到 close——不破坏
        // 已取代它的新会话窗口与内容
        if !session_close_claim(&mut SESSION_STATE.lock(), &label, &token) {
            return Ok(());
        }
        CONTENT_STORE.lock().remove(&label);
        if let Some(win) = webview_window(&app_owned, &label) {
            win.destroy()
                .map_err(|e| AppError::Unknown(format!("销毁预览窗口失败: {e}")))?;
        }
        Ok(())
    })
}

/// 推送渲染内容：存内容（seq 自增）并定向通知宿主页拉取（宿主未就绪时事件
/// 丢失——宿主页加载后会主动 pull 一次兜底）
#[tauri::command]
pub async fn preview_render(
    app: tauri::AppHandle,
    label: String,
    html: String,
    bg: Option<String>,
) -> Result<(), AppError> {
    let seq = store_render_content(label.clone(), html, bg)?;
    if webview_window(&app, &label).is_some() {
        app.emit_to(
            EventTarget::WebviewWindow { label },
            PREVIEW_RENDER_PING_EVENT,
            serde_json::json!({ "seq": seq }),
        )
        .map_err(|e| AppError::Unknown(format!("预览渲染通知失败: {e}")))?;
    }
    Ok(())
}

/// 渲染内容落库（seq 自增；纯逻辑——命令壳与 L1 测试共用）
fn store_render_content(label: String, html: String, bg: Option<String>) -> Result<u64, AppError> {
    validate_label(&label)?;
    if html.len() > 32 * 1024 * 1024 {
        return Err(AppError::Validation("预览内容超过 32MB 上限".into()));
    }
    let mut store = CONTENT_STORE.lock();
    let next = store
        .get(&label)
        .map(|c| c.seq.saturating_add(1))
        .unwrap_or(1);
    store.insert(
        label,
        StoredContent {
            seq: next,
            html,
            bg,
        },
    );
    Ok(next)
}

/// 宿主页拉取内容（宿主页加载后/收到 ping 时调用；无内容返回 null）
#[tauri::command]
pub fn preview_pull(
    _app: tauri::AppHandle,
    label: String,
) -> Result<Option<PreviewContent>, AppError> {
    pull_stored_content(&label)
}

/// 渲染内容读取（纯逻辑——命令壳与 L1 测试共用）
fn pull_stored_content(label: &str) -> Result<Option<PreviewContent>, AppError> {
    validate_label(label)?;
    let store = CONTENT_STORE.lock();
    Ok(store.get(label).map(|c| PreviewContent {
        seq: c.seq,
        html: c.html.clone(),
        bg: c.bg.clone(),
    }))
}

/// preview_pull 返回内容形态（宿主页桥消费）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewContent {
    seq: u64,
    html: String,
    bg: Option<String>,
}

/// 宿主页（自定义协议静态内容）——固定桥接页：
/// 建 sandbox iframe → 拉内容置 srcdoc → iframe 消息上行转发主窗 event →
/// 主窗下行 event 转 postMessage 注入 iframe。
///
/// 实现注记（2026-09-08 实测语义）：
/// - iframe sandbox 仅 allow-scripts：内容隔离于宿主（opaque origin），doc 自带
///   <script> 在 srcdoc 内正常执行（域级 CSP meta 放行内联），无法触达宿主 DOM/桥（无
///   allow-same-origin + tauri 初始化脚本仅注入顶层 frame——CVE-2024-35222 修复
///   后 main_frame_only，webview.rs for_main_frame_only 实证）。
/// - 桥 = 纯 window.postMessage（ADR-0021）：宿主页改由主窗内跨源沙箱
///   iframe 承载，main_frame_only 下 iframe 内无 tauri IPC 注入——主窗
///   origin 白名单（tauri.localhost / dev localhost:1420）+ source 归属
///   双校验收发；旧独立窗口的 __TAURI_INTERNALS__ 事件桥整体退役。
/// - 上行校验（iframe 消息）：origin "null" + source === iframe.contentWindow
///   （srcdoc opaque 序列化，SEC-03 同款）；具体类型/载荷校验在主窗侧。
/// - 域级 CSP 经 meta 承载（tauri 2.11 无 per-webview CSP 配置面）——指令表见上，加宽须复核 SEC 面。
const HOST_PAGE: &str = r##"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:">
<title>slTerminal preview</title>
<style>
  html,body{width:100%;height:100%;margin:0;background:#0a0a0b;overflow:hidden}
  iframe{display:block;width:100%;height:100%;border:none}
</style>
</head>
<body>
<iframe id="preview-frame" sandbox="allow-scripts"></iframe>
<script>
(function () {
  "use strict";
  // ADR-0021：宿主页改由主窗内跨源沙箱 iframe 承载——main_frame_only 下
  // iframe 内无 tauri IPC 注入，桥 = 纯 window.postMessage。
  var frame = document.getElementById("preview-frame");

  // 主窗 origin 白名单（生产 tauri.localhost / dev localhost:1420）——
  // 下行只受理主窗（source 归属 + origin 双校验）
  function isMain(e) {
    return e.source === window.parent &&
      (e.origin === "http://tauri.localhost" || e.origin === "http://localhost:1420");
  }

  window.addEventListener("message", function (e) {
    // ── 主窗下行 ──
    if (isMain(e)) {
      var d = e.data;
      if (!d || typeof d.type !== "string") return;
      // 内容推送 → 置内容 iframe srcdoc（背景色压重建闪白）
      if (d.type === "slterm_host_content") {
        if (typeof d.html !== "string") return;
        frame.style.background = typeof d.bg === "string" ? d.bg : "";
        frame.srcdoc = d.html;
        return;
      }
      // 控制下行（reset/zoom_set/scroll_set）→ relay 进内容 iframe
      //（type/nonce 由 iframe 侧校验；字段白名单式转发）
      if (d.type === "slterm_reset" || d.type === "slterm_zoom_set" || d.type === "slterm_scroll_set") {
        if (!frame.contentWindow) return;
        var msg = { type: d.type, nonce: d.nonce };
        if (typeof d.zoom === "number") msg.zoom = d.zoom;
        if (typeof d.ratio === "number") msg.ratio = d.ratio;
        frame.contentWindow.postMessage(msg, "*");
        return;
      }
      return;
    }
    // ── 内容 iframe 上行（opaque origin → e.origin === "null"）→ relay 主窗 ──
    //（类型白名单在主窗侧守卫；字段白名单式转发——新增上行字段须同步本名单，
    //  否则载荷静默丢弃）
    if (e.source !== frame.contentWindow) return;
    if (e.origin !== "null") return;
    var data = e.data;
    if (!data || typeof data.type !== "string") return;
    var up = { type: data.type, nonce: data.nonce };
    if (typeof data.zoom === "number") up.zoom = data.zoom;
    if (typeof data.ratio === "number") up.ratio = data.ratio;
    if (typeof data.href === "string") up.href = data.href;
    if (typeof data.loaded === "boolean") up.loaded = data.loaded;
    // keyfwd 收窄转发字段（ADR-0021/D2：code + 修饰键，不含 key）
    if (typeof data.code === "string") up.code = data.code;
    if (typeof data.ctrlKey === "boolean") up.ctrlKey = data.ctrlKey;
    if (typeof data.shiftKey === "boolean") up.shiftKey = data.shiftKey;
    if (typeof data.altKey === "boolean") up.altKey = data.altKey;
    if (typeof data.metaKey === "boolean") up.metaKey = data.metaKey;
    window.parent.postMessage(up, "*");
  });

  // 内容 iframe 每次加载完成 → 上行主窗（重建归 1 语义 + keepZoom/keepScrollRatio 恢复）
  frame.addEventListener("load", function () {
    window.parent.postMessage({ type: "slterm_iframe_loaded" }, "*");
  });

  // 桥就绪 → 上行主窗（主窗据此推送/重推内容兜底）
  window.parent.postMessage({ type: "slterm_host_ready" }, "*");
})();
</script>
</body>
</html>
"##;

/// 自定义协议处理器：仅服务宿主页（其余路径 404）——响应不带 CSP
///（预览 CSP 域 = 本协议域，注入机制须内联宽松执行）
pub fn host_protocol(
    _ctx: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    use std::borrow::Cow;
    let path = request.uri().path();
    if path == "/preview-host.html" || path == "/" {
        tauri::http::Response::builder()
            .status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(Cow::Borrowed(HOST_PAGE.as_bytes()))
            .expect("宿主页响应构造失败")
    } else {
        tauri::http::Response::builder()
            .status(404)
            .body(Cow::Borrowed(b"not found".as_slice()))
            .expect("404 响应构造失败")
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    /// 物理矩形换算：scale=1 恒等 + 原点平移
    #[test]
    fn physical_rect_identity_at_scale_1() {
        assert_eq!(
            compute_physical_rect(10.0, 20.0, 300.0, 150.0, 100, 200, 1.0),
            (110, 220, 300, 150)
        );
    }

    /// 物理矩形换算：HiDPI scale 放大坐标与尺寸
    #[test]
    fn physical_rect_scales_position_and_size() {
        assert_eq!(
            compute_physical_rect(10.0, 20.0, 300.0, 150.0, 100, 200, 1.5),
            (115, 230, 450, 225)
        );
    }

    /// 物理矩形换算：宽/高经 (x+w) 坐标差——x≠0 时不得压缩宽度
    /// （旧实现 to_physical(width) - px 会把 x 偏移误减进宽度，x=100/w=300
    /// 得 200——防复发锚点）
    #[test]
    fn physical_rect_width_independent_of_x() {
        let (_, _, pw, ph) = compute_physical_rect(100.0, 50.0, 300.0, 150.0, 1000, 1000, 1.0);
        assert_eq!((pw, ph), (300, 150));
    }

    /// 物理矩形换算：亚像素 round 边界 + 尺寸下限钳 1px
    #[test]
    fn physical_rect_rounding_and_min_clamp() {
        // round(0.4) = 0 → 尺寸钳 1；round(2.5) 按四舍五入到 3（f64::round 远离零）
        assert_eq!(
            compute_physical_rect(0.0, 0.0, 0.4, 0.4, 0, 0, 1.0),
            (0, 0, 1, 1)
        );
        assert_eq!(
            compute_physical_rect(0.0, 0.0, 2.5, 2.5, 0, 0, 1.0),
            (0, 0, 3, 3)
        );
    }

    /// label 校验：合法形态放行（preview- 前缀 + 字母数字下划线连字符冒号斜杠）
    #[test]
    fn label_accepts_legal_panel_ids() {
        for label in [
            "preview-e2e-html-123",
            "preview-md_1",
            "preview-A1_b-c",
            "preview-page-1:html-2",
            "preview-a/b",
        ] {
            assert!(validate_label(label).is_ok(), "合法 label 应放行: {label}");
        }
    }

    /// label 校验：缺前缀/空体/超长/非法字符拒绝
    #[test]
    fn label_rejects_illegal_forms() {
        for label in [
            "main",
            "preview-",
            &format!("preview-{}", "a".repeat(97)),
            "preview-中文",
            "preview-a b",
        ] {
            assert!(
                validate_label(label).is_err(),
                "非法 label 应拒绝: {label:?}"
            );
        }
    }

    /// 宿主页携带域级 CSP meta（SEC-02）：指令表锁死，加宽须复核 SEC 面
    #[test]
    fn host_page_carries_domain_csp() {
        assert!(HOST_PAGE.contains("Content-Security-Policy"));
        assert!(HOST_PAGE.contains("default-src 'none'"));
        assert!(HOST_PAGE.contains("img-src data:"));
        assert!(HOST_PAGE.contains("font-src data:"));
        assert!(HOST_PAGE.contains("script-src 'unsafe-inline'"));
        assert!(HOST_PAGE.contains("style-src 'unsafe-inline'"));
    }

    /// 宿主页桥转发字体探针载荷（TE-08）：loaded 布尔透传主窗——桥对未知
    /// 字段默认丢弃（zoom/ratio/href 白名单式转发），漏转发即链路断点
    #[test]
    fn host_page_bridge_forwards_font_probe_loaded() {
        assert!(HOST_PAGE.contains("typeof data.loaded === \"boolean\""));
        assert!(HOST_PAGE.contains("up.loaded = data.loaded"));
    }

    /// 宿主页桥 = 纯 postMessage（ADR-0021）：iframe 内 main_frame_only 无
    /// __TAURI_INTERNALS__ 注入——桥若回潮依赖 IPC 注入即静默全灭
    #[test]
    fn host_page_bridge_no_tauri_internals() {
        assert!(!HOST_PAGE.contains("__TAURI_INTERNALS__"));
        assert!(!HOST_PAGE.contains("plugin:event|"));
        assert!(!HOST_PAGE.contains("preview_pull"));
    }

    /// 主窗下行受理校验：source 归属 + origin 白名单双闸（生产/开发两形态）
    #[test]
    fn host_page_bridge_is_main_origin_whitelist() {
        assert!(HOST_PAGE.contains("e.source === window.parent"));
        assert!(HOST_PAGE.contains("\"http://tauri.localhost\""));
        assert!(HOST_PAGE.contains("\"http://localhost:1420\""));
    }

    /// 内容推送下行：宿主页置内容 iframe srcdoc + 背景色（压重建闪白）
    #[test]
    fn host_page_bridge_applies_host_content() {
        assert!(HOST_PAGE.contains("d.type === \"slterm_host_content\""));
        assert!(HOST_PAGE.contains("frame.srcdoc = d.html"));
        assert!(HOST_PAGE.contains("frame.style.background"));
    }

    /// 控制下行 relay 白名单：恰好 reset/zoom_set/scroll_set 三类进内容 iframe
    #[test]
    fn host_page_bridge_relays_control_downlink() {
        assert!(HOST_PAGE.contains("d.type === \"slterm_reset\""));
        assert!(HOST_PAGE.contains("d.type === \"slterm_zoom_set\""));
        assert!(HOST_PAGE.contains("d.type === \"slterm_scroll_set\""));
        assert!(HOST_PAGE.contains("frame.contentWindow.postMessage(msg, \"*\")"));
    }

    /// keyfwd 收窄转发字段（ADR-0021/D2）：code + 四修饰键白名单式透传——
    /// 漏转发即预览聚焦全局快捷键静默失效
    #[test]
    fn host_page_bridge_forwards_keyfwd_fields() {
        assert!(HOST_PAGE.contains("typeof data.code === \"string\""));
        assert!(HOST_PAGE.contains("up.code = data.code"));
        assert!(HOST_PAGE.contains("up.ctrlKey = data.ctrlKey"));
        assert!(HOST_PAGE.contains("up.shiftKey = data.shiftKey"));
        assert!(HOST_PAGE.contains("up.altKey = data.altKey"));
        assert!(HOST_PAGE.contains("up.metaKey = data.metaKey"));
    }

    /// 宿主层上行信号：iframe 加载完成 + 桥就绪（主窗重推内容兜底锚点）
    #[test]
    fn host_page_bridge_emits_host_signals() {
        assert!(HOST_PAGE.contains("type: \"slterm_iframe_loaded\""));
        assert!(HOST_PAGE.contains("type: \"slterm_host_ready\""));
    }

    /// 物理坐标换算：origin + round(css × scale)
    #[test]
    fn physical_conversion_rounds_css_by_scale() {
        assert_eq!(to_physical(10.0, 100, 1.5), 115);
        assert_eq!(to_physical(0.0, -8, 1.25), -8);
        assert_eq!(to_physical(2.4, 0, 1.0), 2);
    }

    /// 内容存储：render 自增 seq、pull 取回同值、非法 label 拒绝
    #[test]
    fn content_store_seq_and_clear() {
        CONTENT_STORE.lock().clear();
        assert_eq!(
            store_render_content("preview-t1".into(), "<h1>a</h1>".into(), None).unwrap(),
            1
        );
        assert_eq!(
            store_render_content("preview-t1".into(), "<h1>b</h1>".into(), None).unwrap(),
            2
        );
        let got = pull_stored_content("preview-t1")
            .unwrap()
            .expect("应有内容");
        assert_eq!(got.seq, 2, "seq 应自增");
        assert_eq!(got.html, "<h1>b</h1>");
        assert!(got.bg.is_none());

        // 非法 label 在存储路径同样拒绝
        assert!(store_render_content("main".into(), "x".into(), None).is_err());
        assert!(pull_stored_content("main").is_err());

        // 超限内容拒绝
        assert!(store_render_content(
            "preview-t1".into(),
            "x".repeat(32 * 1024 * 1024 + 1).into(),
            None
        )
        .is_err());
    }

    /// 会话守卫核心竞态：close 后同 token 迟到 sync 拒绝（销毁后复活僵尸根因）
    #[test]
    fn session_gate_rejects_late_sync_after_close() {
        let mut state = HashMap::new();
        // 挂载 sync 放行并登记当前会话
        assert!(session_sync_allowed(&mut state, "preview-p1", "tok-a"));
        // 面板卸载 close 受理（标记 closed）
        assert!(session_close_claim(&mut state, "preview-p1", "tok-a"));
        // 同 token 迟到 sync 拒绝——窗口不得按 visible 重建（preview_sync 的
        // 会话守卫即拦截于此，不落「窗口不存在 + visible → 建窗」分支）
        assert!(!session_sync_allowed(&mut state, "preview-p1", "tok-a"));
        // 重复 close 幂等受理（已 closed 再受理无副作用）
        assert!(session_close_claim(&mut state, "preview-p1", "tok-a"));
        assert!(!session_sync_allowed(&mut state, "preview-p1", "tok-a"));
    }

    /// 会话守卫：close 后新 token sync 放行（真重挂载——形态切换/重开面板往返）；
    /// 且被取代的旧 token 迟到 sync 不复活（退役登记生效）
    #[test]
    fn session_gate_allows_new_mount_after_close() {
        let mut state = HashMap::new();
        assert!(session_sync_allowed(&mut state, "preview-p1", "tok-a"));
        assert!(session_close_claim(&mut state, "preview-p1", "tok-a"));
        // 新挂载（异 token）→ 放行并接管会话（closed 复位）
        assert!(session_sync_allowed(&mut state, "preview-p1", "tok-b"));
        // 新会话 close 后：自身迟到 sync 拒绝
        assert!(session_close_claim(&mut state, "preview-p1", "tok-b"));
        assert!(!session_sync_allowed(&mut state, "preview-p1", "tok-b"));
        // 旧 token（tok-a，已退役）迟到 sync 同样拒绝——不得复活旧挂载
        assert!(!session_sync_allowed(&mut state, "preview-p1", "tok-a"));
    }

    /// 会话守卫：异 token 迟到 close 拒绝——不破坏已取代它的新会话窗口
    #[test]
    fn session_gate_rejects_stale_close_after_supersede() {
        let mut state = HashMap::new();
        assert!(session_sync_allowed(&mut state, "preview-p1", "tok-a"));
        // 新挂载取代旧会话（同一 label 重开面板）
        assert!(session_sync_allowed(&mut state, "preview-p1", "tok-b"));
        // 旧挂载迟到 close → 拒绝（不得销毁新会话窗口/内容）
        assert!(!session_close_claim(&mut state, "preview-p1", "tok-a"));
        // 新会话不受扰动：close 自身仍受理、其迟到 sync 仍拒绝
        assert!(session_close_claim(&mut state, "preview-p1", "tok-b"));
        assert!(!session_sync_allowed(&mut state, "preview-p1", "tok-b"));
    }

    /// 会话守卫：同 token 常规双 sync 幂等放行（几何轮询多拍不重置会话）
    #[test]
    fn session_gate_same_token_sync_stays_open() {
        let mut state = HashMap::new();
        assert!(session_sync_allowed(&mut state, "preview-p1", "tok-a"));
        assert!(session_sync_allowed(&mut state, "preview-p1", "tok-a"));
        // 多代挂载退役列表有界（MAX_RETIRED_TOKENS 裁剪生效）
        for i in 0..10 {
            let t = format!("tok-{i}");
            assert!(session_sync_allowed(&mut state, "preview-p2", &t));
        }
        let entry = state.get("preview-p2").expect("应有会话");
        assert!(entry.retired.len() <= MAX_RETIRED_TOKENS);
    }

    /// 会话守卫：close 落在未知会话（从未 sync）→ 受理并登记 closed（防御形态）
    #[test]
    fn session_gate_close_unknown_session_registers_closed() {
        let mut state = HashMap::new();
        assert!(session_close_claim(&mut state, "preview-p1", "tok-a"));
        assert!(!session_sync_allowed(&mut state, "preview-p1", "tok-a"));
    }

    /// token 校验：空/超长拒绝，常规挂载串放行
    #[test]
    fn token_rejects_empty_and_oversize() {
        assert!(validate_token("").is_err());
        assert!(validate_token(&"a".repeat(97)).is_err());
        assert!(validate_token(&"a".repeat(96)).is_ok());
        assert!(validate_token("0123456789abcdef0123456789abcdef").is_ok());
    }
}
