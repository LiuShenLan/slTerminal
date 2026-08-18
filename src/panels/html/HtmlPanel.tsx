// HtmlPanel — HTML 文件浏览器式预览面板
//
// 使用 iframe + srcDoc 渲染 HTML 文件内容。
// sandbox="allow-scripts"（不含 allow-same-origin），防止 Tauri 注入 App JS bundle。
// WebView2 sandboxed iframe 不支持 #fragment 导航（srcdoc→跳父URL、blob→"not allowed"），
// 故注入脚本拦截 <a href="#..."> 点击，preventDefault + 手动 scrollIntoView。
// 键盘转发（Ctrl+W）用注入脚本 + postMessage。
// SEC-04：postMessage 携带面板挂载期生成的随机 nonce，父窗口校验一致才转发——
// 防 iframe 内任意脚本伪造 slterm_key 消息触发全局快捷键。
//
// 三态：loading → loaded (iframe) / error
// 通过 cancelled 标志防止组件卸载或快速切换 filePath 时的竞态。

import React, { useEffect, useRef, useState } from "react";
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

/** 注入脚本的信任标记，ShortcutRegistry 分发前可识别 postMessage 重放事件 */
const TRUSTED_MARKER = "__slterm_postMessage";

/** 注入到 HTML 内容中的脚本标记（幂等检测） */
const INJECTED_MARKER = "__slterm_key";

/**
 * 生成面板生命周期绑定的随机 nonce（SEC-04）。
 * crypto.getRandomValues 取 128 位随机数，输出十六进制串（仅 [0-9a-f]，
 * 可直接拼入注入脚本字符串——无引号/反斜杠转义风险）。
 */
function createNonce(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  let out = "";
  for (const b of bytes) out += b.toString(16).padStart(2, "0");
  return out;
}

/**
 * 生成注入到 HTML 内容中的脚本，包含三部分：
 * 1) 键盘转发——keydown capture → postMessage 到父窗口（消息携带面板 nonce，SEC-04）
 * 2) 片段链接拦截——<a href="#..."> 点击 preventDefault + class-based :target 模拟
 *    （WebView2 sandboxed iframe 不支持 location.hash 导航）
 * 3) CSS 注入——.slterm-target 基础样式（:target 备选）
 *
 * @param nonce 面板挂载期生成的随机值，拼入 keydown postMessage——父窗口据此校验消息来源
 */
function buildInjectedScript(nonce: string): string {
  return (
  `<script>` +
  // CSS：.slterm-target 作为 :target 备选
  `var s=document.createElement("style");s.textContent=".slterm-target{display:block!important}";document.head.appendChild(s);` +
  // 键盘转发——postMessage 目标 origin 用 "null"（srcdoc iframe 为 opaque origin，按规范序列化为字符串 "null"）
  `document.addEventListener("keydown",function(e){window.parent.postMessage({type:"slterm_key",nonce:"${nonce}",fingerprint:(e.ctrlKey?"Ctrl+":"")+(e.shiftKey?"Shift+":"")+(e.altKey?"Alt+":"")+(e.metaKey?"Meta+":"")+e.code,ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey,metaKey:e.metaKey,code:e.code,key:e.key},"null")},true);` +
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
  `</script>`
  );
}

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
  const iframeRef = useRef<HTMLIFrameElement>(null);
  // SEC-04：面板生命周期绑定的随机 nonce——挂载期生成一次（惰性初始化 ref，StrictMode 双渲染不重复生成），
  // 经注入脚本拼入 iframe 的 keydown postMessage；父窗口校验消息 nonce 一致才转发。
  // 若 each 渲染重新生成（useState 初始化器 / 直接调函数），注入脚本与校验值会漂移导致键盘转发失效。
  const nonceRef = useRef<string | null>(null);
  if (nonceRef.current === null) {
    nonceRef.current = createNonce();
  }
  const nonce = nonceRef.current;

  /**
   * 监听 iframe 内 postMessage 发来的键盘事件。
   * - 校验 e.origin === "null"（srcdoc iframe 为 opaque origin，按规范序列化为 "null"）
   *   【注意】e.origin === "null" 为 opaque origin 规范推断，未经真实 WebView2 实测，正确性由收尾 L4 验证
   * - 校验 e.source === 本面板 iframe.contentWindow（防止其他窗口伪装）
   * - 校验 e.data.nonce === 面板挂载期生成的随机 nonce（SEC-04：防 iframe 内任意脚本伪造）
   * - 命中全局快捷键 → 合成 keydown 在父 window 上重放 → ShortcutRegistry 正常分发
   * - 合成事件添加 __slterm_postMessage 信任标记，供 ShortcutRegistry 识别来源
   */
  useEffect(() => {
    const handleMessage = (e: MessageEvent) => {
      // 校验 origin：srcdoc iframe 为 opaque origin，序列化为 "null"
      if (e.origin !== "null") return;
      // 校验 source：仅接受本面板 iframe 发出的消息
      if (e.source !== iframeRef.current?.contentWindow) return;
      if (!e.data || e.data.type !== "slterm_key") return;
      // SEC-04：nonce 校验——注入脚本拼入的随机值（经 srcDoc 内联），不符静默丢弃。
      // iframe 内任意脚本可伪造 origin/source 不可伪造的消息，但拿不到本面板 nonce，
      // 伪造消息在此被拦截（origin/source 校验只挡其他窗口/iframe，挡不住 iframe 自身内容）。
      if (typeof e.data.nonce !== "string" || e.data.nonce !== nonceRef.current) return;
      const fingerprint: string | undefined = e.data.fingerprint;
      if (!fingerprint) return;
      const registry = getShortcutRegistry();
      const globalBindings = registry.exportContextBindings("global");
      if (globalBindings.some((b) => b.keystroke === fingerprint)) {
        const event = new KeyboardEvent("keydown", {
          ctrlKey: e.data.ctrlKey ?? false,
          shiftKey: e.data.shiftKey ?? false,
          altKey: e.data.altKey ?? false,
          metaKey: e.data.metaKey ?? false,
          code: e.data.code ?? "",
          key: e.data.key ?? "",
          bubbles: true,
          cancelable: true,
        });
        // 信任标记——ShortcutRegistry 分发前可识别 postMessage 重放事件
        Object.defineProperty(event, TRUSTED_MARKER, { value: true });
        window.dispatchEvent(event);
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
      ref={iframeRef}
      sandbox={SANDBOX_FLAGS}
      srcDoc={injectScript(state.html, buildInjectedScript(nonce), INJECTED_MARKER)}
      title={`HTML 预览: ${params.filePath}`}
      style={iframeStyle}
    />
  );
};

export default HtmlPanel;
