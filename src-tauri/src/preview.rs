// preview.rs — docViewer 预览宿主页自定义协议域（ADR-0019 安全域 / ADR-0021 载体）
//
// docViewer 面板（htmlviewer/markdownviewer）的预览内容渲染于【主窗内跨源沙箱
// iframe】（ADR-0021：旧独立 WebviewWindow 载体已推翻——拖动跟随延迟结构性
// 不可归零；窗口管理/内容存储/会话守卫全族随之消亡）。宿主 iframe 的 src 指向
// 本自定义协议域（scheme slterm-preview，tauri 在 Windows 映射为
// http://slterm-preview.localhost/…，register_uri_scheme_protocol 文档实证）：
//
//   - 主窗口 CSP 收紧（CP-012，script-src 'self'）后资产协议页仍被注入全局 CSP，
//     srcdoc 子文档继承后运行时内联脚本全灭——tauri 2.11 无 per-webview CSP
//     （spike 实证）。本自定义协议响应不经 tauri 资产处理器，不带全局 CSP 头 →
//     「预览 CSP 域」即本域：注入机制（injectScript + buildInjectedScript + nonce）
//     原样迁入域内宽松执行（CP-031 判定二/三由此可达）。
//   - 宿主页 = 固定桥接页：建 sandbox iframe（allow-scripts、无 allow-same-origin
//     ——Tauri CVE-2024-35222 红线延续）+ 内容注入 + 上下行中继；iframe 内容
//     自带 <script> 不经字符串转义进入渲染文档（escapeScriptClose 消亡，CP-031）。
//   - 消息桥 = 纯 window.postMessage（ADR-0021）：宿主页承载于主窗内 iframe，
//     main_frame_only 下 iframe 内无 tauri IPC 注入——上行 zoom/scroll/nav/
//     keyfwd/font_probe + 宿主状态信号（host_ready/iframe_loaded）；下行
//     host_content + reset/zoom_set/scroll_set；主窗 origin 白名单 +
//     source 归属双校验。

/// 宿主页自定义协议 scheme（Windows 上 tauri 注册为 http://<scheme>.localhost/…）
pub const PREVIEW_SCHEME: &str = "slterm-preview";

/// 宿主页（自定义协议静态内容）——固定桥接页：
/// 主窗 slterm_host_content 下行 → 置内容 iframe srcdoc → iframe 消息上行 relay 主窗；
/// 主窗控制下行（reset/zoom_set/scroll_set）relay 注入 iframe。
///
/// 实现注记（2026-09-08 实测语义）：
/// - iframe sandbox 仅 allow-scripts：内容隔离于宿主（opaque origin），doc 自带
///   <script> 在 srcdoc 内正常执行（域级 CSP meta 放行内联），无法触达宿主 DOM/桥（无
///   allow-same-origin + tauri 初始化脚本仅注入顶层 frame——CVE-2024-35222 修复
///   后 main_frame_only，webview.rs for_main_frame_only 实证）。
/// - 桥 = 纯 window.postMessage（ADR-0021）：宿主页改由主窗内跨源沙箱
///   iframe 承载，main_frame_only 下 iframe 内无 tauri IPC 注入——主窗
///   origin 白名单（tauri.localhost / dev localhost:1420）+ source 归属
///   双校验收发；旧独立窗口的 tauri IPC 事件桥整体退役。
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
    /// tauri IPC 注入——桥若回潮依赖 IPC 注入即静默全灭
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
}
