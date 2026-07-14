// HtmlPanel — HTML 文件浏览器式预览面板
//
// 使用 iframe + srcDoc 渲染 HTML 文件内容。
// sandbox="allow-scripts"（不含 allow-same-origin），防止 Tauri 注入 App JS bundle。
// WebView2 sandboxed iframe 不支持 #fragment 导航（srcdoc→跳父URL、blob→"not allowed"），
// 故注入脚本拦截 <a href="#..."> 点击，preventDefault + 手动 scrollIntoView。
// 键盘转发（Ctrl+W）用注入脚本 + postMessage。
//
// 三态：loading → loaded (iframe) / error
// 通过 cancelled 标志防止组件卸载或快速切换 filePath 时的竞态。

import React, { useEffect, useState } from "react";
import { fs } from "../../ipc";
import { injectScript } from "../../lib";
import { getShortcutRegistry } from "../../features/shortcuts/ShortcutRegistry";
import { PANEL_BG, ERROR_FG, HTML_PANEL_LOADING_FG, HTML_PANEL_IFRAME_BG } from "../../theme";

/** HtmlPanel 接收的面板参数 */
interface HtmlPanelProps {
  params: {
    panelId: string;
    filePath?: string;
  };
}

/** 加载状态机 */
type LoadState =
  | { kind: "loading" }
  | { kind: "loaded"; html: string }
  | { kind: "error"; message: string };

/** iframe sandbox 权限：仅允许脚本执行，不含 allow-same-origin（防止 Tauri 注入 App JS） */
const SANDBOX_FLAGS = "allow-scripts";

/** 注入到 HTML 内容中的脚本标记（幂等检测） */
const INJECTED_MARKER = "__slterm_key";

/**
 * 注入到 HTML 内容中的脚本，包含三部分：
 * 1) 键盘转发——keydown capture → postMessage 到父窗口
 * 2) 片段链接拦截——<a href="#..."> 点击 preventDefault + class-based :target 模拟
 *    （WebView2 sandboxed iframe 不支持 location.hash 导航）
 * 3) CSS 注入——.slterm-target 基础样式（:target 备选）
 */
const INJECTED_SCRIPT =
  `<script>` +
  // CSS：.slterm-target 作为 :target 备选
  `var s=document.createElement("style");s.textContent=".slterm-target{display:block!important}";document.head.appendChild(s);` +
  // 键盘转发
  `document.addEventListener("keydown",function(e){window.parent.postMessage({type:"slterm_key",fingerprint:(e.ctrlKey?"Ctrl+":"")+(e.shiftKey?"Shift+":"")+(e.altKey?"Alt+":"")+(e.metaKey?"Meta+":"")+e.code,ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey,metaKey:e.metaKey,code:e.code,key:e.key},"*")},true);` +
  // 片段链接拦截 + class-based :target 模拟
  `var _h=null;document.addEventListener("click",function(e){var a=e.target.closest("a");if(!a)return;var h=a.getAttribute("href");if(!h||h.charAt(0)!=="#")return;e.preventDefault();var id=h.slice(1);` +
  // 点击 # → 清除状态
  `if(!id){if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash};window.scrollTo({top:0,behavior:"smooth"});return}` +
  // 同片段 → toggle
  `if(id===_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash;return}` +
  // 不同片段 → 切换
  `if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target")}` +
  `var el=document.getElementById(id);if(el){el.classList.add("slterm-target");el.scrollIntoView({behavior:"smooth"});document.documentElement.dataset.sltermHash=id;_h=id}` +
  `},true)` +
  `</script>`;

/** 居中容器样式 */
const centerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

/** iframe 全容器样式（白底——HTML 页面默认背景） */
const iframeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "none",
  background: HTML_PANEL_IFRAME_BG,
};

const HtmlPanel: React.FC<HtmlPanelProps> = ({ params }) => {
  const [state, setState] = useState<LoadState>({ kind: "loading" });

  /**
   * 监听 iframe 内 postMessage 发来的键盘事件。
   * 命中全局快捷键 → 合成 keydown 在父 window 上重放 → ShortcutRegistry 正常分发。
   */
  useEffect(() => {
    const handleMessage = (e: MessageEvent) => {
      if (!e.data || e.data.type !== "slterm_key") return;
      const fingerprint: string | undefined = e.data.fingerprint;
      if (!fingerprint) return;
      const registry = getShortcutRegistry();
      const globalBindings = registry.exportContextBindings("global");
      if (globalBindings.some((b) => b.keystroke === fingerprint)) {
        window.dispatchEvent(
          new KeyboardEvent("keydown", {
            ctrlKey: e.data.ctrlKey ?? false,
            shiftKey: e.data.shiftKey ?? false,
            altKey: e.data.altKey ?? false,
            metaKey: e.data.metaKey ?? false,
            code: e.data.code ?? "",
            key: e.data.key ?? "",
            bubbles: true,
            cancelable: true,
          }),
        );
      }
    };
    window.addEventListener("message", handleMessage);
    return () => window.removeEventListener("message", handleMessage);
  }, []);

  useEffect(() => {
    if (!params.filePath) {
      setState({ kind: "error", message: "未指定文件路径" });
      return;
    }

    let cancelled = false;

    (async () => {
      try {
        const content = await fs.readFile(params.filePath!);
        if (!cancelled) setState({ kind: "loaded", html: content });
      } catch (err) {
        if (!cancelled) {
          setState({
            kind: "error",
            message: err instanceof Error ? err.message : String(err),
          });
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [params.filePath]);

  if (state.kind === "loading") {
    return (
      <div style={centerStyle}>
        <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }}>加载中...</span>
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div style={centerStyle}>
        <span style={{ color: ERROR_FG, fontSize: 13 }}>
          加载失败: {state.message}
        </span>
      </div>
    );
  }

  return (
    <iframe
      sandbox={SANDBOX_FLAGS}
      srcDoc={injectScript(state.html, INJECTED_SCRIPT, INJECTED_MARKER)}
      title={`HTML 预览: ${params.filePath}`}
      style={iframeStyle}
    />
  );
};

export default HtmlPanel;
