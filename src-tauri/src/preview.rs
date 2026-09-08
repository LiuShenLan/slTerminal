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

/// label 合法字符集：仅 ASCII 字母数字 + 下划线/连字符（窗口 label 与驱动句柄共用）
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
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
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

/// 建窗参数：owned（跟随主窗）、无边框、不可聚焦、任务栏隐藏、初始隐藏
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
        .visible(false)
        .inner_size(width.max(1) as f64, height.max(1) as f64)
        .position(x as f64, y as f64);
    let builder = builder
        .owner(&main)
        .map_err(|e| AppError::Unknown(format!("预览窗口挂主窗失败: {e}")))?;
    let win = builder
        .build()
        .map_err(|e| AppError::Unknown(format!("创建预览窗口失败: {e}")))?;
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
#[tauri::command]
pub async fn preview_sync(
    app: tauri::AppHandle,
    label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    visible: bool,
) -> Result<(), AppError> {
    validate_label(&label)?;
    // 防御：尺寸非法/过小按隐藏处理
    let visible = visible && width >= 1.0 && height >= 1.0;
    let app_owned = app.clone();
    run_on_main(&app, move || {
        let (ox, oy, scale) = main_window_geometry(&app_owned)?;
        let px = to_physical(x, ox, scale);
        let py = to_physical(y, oy, scale);
        let pw = to_physical(width, ox, scale).saturating_sub(px).max(1);
        let ph = to_physical(height, oy, scale).saturating_sub(py).max(1);

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
#[tauri::command]
pub async fn preview_close(app: tauri::AppHandle, label: String) -> Result<(), AppError> {
    validate_label(&label)?;
    CONTENT_STORE.lock().remove(&label);
    let app_owned = app.clone();
    run_on_main(&app, move || {
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
///   <script> 在 srcdoc 内正常执行（域内无 CSP），无法触达宿主 DOM/桥（无
///   allow-same-origin + tauri 初始化脚本仅注入顶层 frame——CVE-2024-35222 修复
///   后 main_frame_only，webview.rs for_main_frame_only 实证）。
/// - 桥经 window.__TAURI_INTERNALS__ 原生事件 API（invoke plugin:event|listen /
///   emit）——宿主页为独立自定义域页，无打包模块可用（tauri 资产模块跨源
///   ESM 需 CORS，不引）；权限由 capabilities/preview.json 收敛。
/// - 上行校验（iframe 消息）：origin "null" + source === iframe.contentWindow
///   （srcdoc opaque 序列化，SEC-03 同款）；具体类型/载荷校验在主窗侧。
const HOST_PAGE: &str = r##"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
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
  var T = window.__TAURI_INTERNALS__;
  var label = (T && T.metadata && T.metadata.currentWebview && T.metadata.currentWebview.label) || "";
  var frame = document.getElementById("preview-frame");
  var appliedSeq = 0;
  var ready = false;

  function invoke(cmd, args) {
    if (!T || typeof T.invoke !== "function") return Promise.reject(new Error("no internals"));
    return T.invoke(cmd, args);
  }
  function listenEvent(eventName, handler) {
    if (!T) return;
    invoke("plugin:event|listen", {
      event: eventName,
      target: { kind: "Any" },
      handler: T.transformCallback(handler)
    }).catch(function () {});
  }
  function emitEvent(eventName, payload) {
    if (!T) return;
    invoke("plugin:event|emit", { event: eventName, payload: payload }).catch(function () {});
  }

  // 主窗下行（reset/zoom_set/scroll_set）→ 注入 iframe（type/nonce 由 iframe 侧校验）
  // 注：事件处理器收到的 p 为事件对象 {event, id, payload}——payload 才是载荷
  listenEvent("preview:downlink", function (p) {
    var d = p && p.payload ? p.payload : p;
    if (!d || d.label !== label || !frame.contentWindow) return;
    var msg = { type: d.type, nonce: d.nonce };
    if (typeof d.zoom === "number") msg.zoom = d.zoom;
    if (typeof d.ratio === "number") msg.ratio = d.ratio;
    frame.contentWindow.postMessage(msg, "*");
  });

  // 渲染内容就绪（主窗已推新内容）→ 拉取应用
  listenEvent("preview:render-ping", function (p) {
    var d = p && p.payload ? p.payload : p;
    pull(d && d.seq);
  });

  // iframe 文档上行（zoom/scroll/nav）→ 转发主窗（类型白名单在主窗侧守卫）
  window.addEventListener("message", function (e) {
    if (e.source !== frame.contentWindow) return;
    if (e.origin !== "null") return;
    var data = e.data;
    if (!data || typeof data.type !== "string") return;
    var up = { label: label, type: data.type, nonce: data.nonce };
    if (typeof data.zoom === "number") up.zoom = data.zoom;
    if (typeof data.ratio === "number") up.ratio = data.ratio;
    if (typeof data.href === "string") up.href = data.href;
    emitEvent("preview:uplink", up);
  });

  // iframe 每次加载完成 → 通知主窗（重建归 1 语义 + keepZoom/keepScrollRatio 恢复）
  frame.addEventListener("load", function () {
    emitEvent("preview:host-status", { label: label, status: "iframe-loaded" });
  });

  function pull(seqHint) {
    invoke("preview_pull", { label: label }).then(function (c) {
      if (!c) return;
      if (ready && c.seq <= appliedSeq) return;
      appliedSeq = c.seq;
      frame.style.background = c.bg || "";
      frame.srcdoc = c.html;
      ready = true;
    }).catch(function () {});
  }

  // 页面加载完成即拉一次（覆盖建窗期事件丢失窗口）
  pull(0);
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

    /// label 校验：合法形态放行（preview- 前缀 + 字母数字下划线连字符）
    #[test]
    fn label_accepts_legal_panel_ids() {
        for label in ["preview-e2e-html-123", "preview-md_1", "preview-A1_b-c"] {
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
            "preview-a/b",
        ] {
            assert!(
                validate_label(label).is_err(),
                "非法 label 应拒绝: {label:?}"
            );
        }
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
}
