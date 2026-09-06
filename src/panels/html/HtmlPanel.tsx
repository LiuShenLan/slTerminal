// HtmlPanel — HTML 文件浏览器式预览面板
//
// 使用 iframe + srcDoc 渲染 HTML 文件内容。
// sandbox="allow-scripts"（不含 allow-same-origin），防止 Tauri 注入 App JS bundle。
// WebView2 sandboxed iframe 不支持 #fragment 导航（srcdoc→跳父URL、blob→"not allowed"），
// 故注入脚本拦截 <a href="#..."> 点击，preventDefault + 手动 scrollIntoView。
// 键盘转发（Ctrl+W）用注入脚本 + postMessage。
// Ctrl+滚轮整体缩放：注入脚本在 iframe 文档内捕获 wheel → documentElement.style.zoom
// （opaque origin 下父窗口无法触达 iframe DOM），缩放值经 slterm_zoom 上行上报；
// HUD「百分比 + 重置」由父窗口渲染，重置经 slterm_reset 下行（详见 zoomRuntime.ts）。
// SEC-04：postMessage 携带面板挂载期生成的随机 nonce，父窗口校验一致才转发——
// 防 iframe 内任意脚本伪造 slterm_key / slterm_zoom 消息。
//
// 三态：loading → loaded (iframe) / error
// 通过 cancelled 标志防止组件卸载或快速切换 filePath 时的竞态。

import React, { useEffect, useRef, useState } from "react";
import { fs } from "../../ipc";
import { injectScript } from "../../lib";
import { getShortcutRegistry } from "../../features/shortcuts/ShortcutRegistry";
import {
  PANEL_BG,
  ERROR_FG,
  HTML_PANEL_LOADING_FG,
  HTML_PANEL_IFRAME_BG,
  SECONDARY_BG,
  SEPARATOR_BG,
  SIDEBAR_FG,
  ACCENT_FG,
} from "../../theme";
import {
  ZOOM_MSG_TYPE,
  isFiniteZoom,
  formatPercent,
  buildResetRequest,
} from "./zoomMath";
import { buildZoomRuntimeSource } from "./zoomRuntime";

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
 * 生成注入到 HTML 内容中的脚本，包含四部分：
 * 1) 键盘转发——keydown capture → postMessage 到父窗口（消息携带面板 nonce，SEC-04）
 * 2) 片段链接拦截——<a href="#..."> 点击 preventDefault + class-based :target 模拟
 *    （WebView2 sandboxed iframe 不支持 location.hash 导航）
 * 3) CSS 注入——.slterm-target 基础样式（:target 备选）
 * 4) Ctrl+滚轮缩放——wheel capture + 下行复位监听（zoomRuntime，逻辑见 zoomRuntime.ts）
 *
 * @param nonce 面板挂载期生成的随机值，拼入 keydown/zoom postMessage——父窗口据此校验消息来源
 *
 * 【SEC-04 威胁模型（D16 登记）】nonce 明文内联于 srcDoc——iframe 内任意脚本可读取
 * 文档中的注入脚本提取 nonce 并伪造 slterm_key / slterm_zoom 消息。nonce 仅防
 * 「不知密钥的外部伪造」，不防被预览 HTML 自身。伪造 zoom 上报的后果仅为 HUD
 * 百分比误导（低危）；键盘转发由 global context 命令集最小化兜底（当前仅
 * global.closeTab 关页签，低风险）——守卫测试 command-catalog.test.ts 锁死该集合，
 * 扩充 global 命令必须先评估本威胁模型。
 */
function buildInjectedScript(nonce: string): string {
  return (
  `<script>` +
  // CSS：.slterm-target 作为 :target 备选
  `var s=document.createElement("style");s.textContent=".slterm-target{display:block!important}";document.head.appendChild(s);` +
  // 键盘转发——postMessage targetOrigin 用 "*"（2026-09-06 实证：targetOrigin 必须匹配
  // 接收方窗口 origin；iframe 为 opaque origin 只影响消息到达父后的 e.origin 序列化，
  // 与发送 targetOrigin 无关——传 "null" 与父窗口 origin 不匹配会被 Chromium 静默丢弃，
  // 曾致键盘转发与缩放上行全灭。父侧 origin/source/nonce 校验兜底，"*" 无额外风险）
  `document.addEventListener("keydown",function(e){window.parent.postMessage({type:"slterm_key",nonce:"${nonce}",fingerprint:(e.ctrlKey?"Ctrl+":"")+(e.shiftKey?"Shift+":"")+(e.altKey?"Alt+":"")+(e.metaKey?"Meta+":"")+e.code,ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey,metaKey:e.metaKey,code:e.code,key:e.key},"*")},true);` +
  // 片段链接拦截 + class-based :target 模拟
  `var _h=null;document.addEventListener("click",function(e){var a=e.target.closest("a");if(!a)return;var h=a.getAttribute("href");if(!h||h.charAt(0)!=="#")return;e.preventDefault();var id=h.slice(1);` +
  // 点击 # → 清除状态
  `if(!id){if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash};window.scrollTo({top:0,behavior:"smooth"});return}` +
  // 同片段 → toggle
  `if(id===_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash;return}` +
  // 不同片段 → 切换
  `if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target")}` +
  `var el=document.getElementById(id);if(el){el.classList.add("slterm-target");el.scrollIntoView({behavior:"smooth"});document.documentElement.dataset.sltermHash=id;_h=id}` +
  // 注意：上一语句（click listener 注册）原为 script 末语句无分号——追加后续语句
  // 必须补分号，否则同串拼接 "},true)var" 触发 SyntaxError 致整段注入脚本失效
  `},true);` +
  // Ctrl+滚轮缩放运行时——匿名函数挂载，闭包状态随 iframe 文档存亡（页签会话级）
  `var sltermZoom=(${buildZoomRuntimeSource(nonce)});sltermZoom(document,window);` +
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

/** 缩放 HUD 隐藏延迟（毫秒）：无缩放操作即消失（Chrome 缩放气泡语义） */
const HUD_HIDE_MS = 3000;

/**
 * HUD 透明层——占满渲染区但不挡 iframe 交互（pointerEvents: none 透传），
 * 气泡本体在其内部自开 pointerEvents: auto。
 */
const hudLayerStyle: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  paddingTop: 8,
  paddingRight: 8,
  pointerEvents: "none",
  zIndex: 10,
};

/** HUD 气泡本体：百分比 + 重置按钮（瞬态，缩放停止 3s 后消失） */
const hudChipStyle: React.CSSProperties = {
  pointerEvents: "auto",
  display: "flex",
  alignItems: "center",
  gap: 4,
  background: SECONDARY_BG,
  border: `1px solid ${SEPARATOR_BG}`,
  borderRadius: 6,
  padding: "2px 4px 2px 10px",
  fontSize: 12,
  color: SIDEBAR_FG,
};

/** 重置按钮（去浏览器默认样式，accent 色标识可操作） */
const resetBtnStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  cursor: "pointer",
  color: ACCENT_FG,
  fontSize: 12,
  padding: "2px 6px",
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

  // HUD 瞬态缩放指示器（Chrome 缩放气泡语义）：
  // zoom 变化 → 显示「百分比 + 重置」，3s 无缩放操作自动消失，期间缩放续期。
  // hudZoomRef 为逻辑真值源（同步读写，避免 setState 异步比较竞态）；
  // hud state 仅驱动渲染。iframe 文档内 zoom 经 slterm_zoom 上行，等值消息
  // （复位回声）不得复活已隐藏的气泡。
  const [hud, setHud] = useState<{ zoom: number; visible: boolean }>({
    zoom: 1,
    visible: false,
  });
  const hudZoomRef = useRef(1);
  const hideTimerRef = useRef<number | null>(null);

  /** 立即隐藏 HUD 并复位 zoom 基准（重置点击 / iframe 重建） */
  const hideHud = () => {
    hudZoomRef.current = 1;
    if (hideTimerRef.current !== null) {
      window.clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
    setHud({ zoom: 1, visible: false });
  };

  /** 重置按钮：下行复位请求 → iframe 内 zoom 归 1 并上报；父侧立即隐藏不等回声 */
  const handleResetClick = () => {
    iframeRef.current?.contentWindow?.postMessage(
      buildResetRequest(nonce),
      // targetOrigin 统一 "*"（2026-09-06 实证，与上行同因）：子侧 source===parent +
      // nonce + type 三重校验保证安全，无需依赖 targetOrigin 收窄
      "*",
    );
    hideHud();
  };

  /**
   * 监听 iframe 内 postMessage 发来的消息（slterm_key 键盘 / slterm_zoom 缩放上行）。
   * 公共校验（两条通道一致）：
   * - 校验 e.origin === "null"（srcdoc iframe 为 opaque origin，按规范序列化为 "null"）
   *   【注意】e.origin === "null" 为 opaque origin 规范推断，未经真实 WebView2 实测，正确性由收尾 L4 验证
   * - 校验 e.source === 本面板 iframe.contentWindow（防止其他窗口伪装）
   * - 校验 e.data.nonce === 面板挂载期生成的随机 nonce（SEC-04：防 iframe 内任意脚本伪造）
   * slterm_key：命中全局快捷键 → 合成 keydown 在父 window 上重放 → ShortcutRegistry 正常分发；
   *   合成事件添加 __slterm_postMessage 信任标记，供 ShortcutRegistry 识别来源。
   * slterm_zoom：缩放值上报 → 驱动 HUD（等值回声不复活气泡，3s 无操作自动隐藏）。
   */
  useEffect(() => {
    const handleMessage = (e: MessageEvent) => {
      // 校验 origin：srcdoc iframe 为 opaque origin，序列化为 "null"
      if (e.origin !== "null") return;
      // 校验 source：仅接受本面板 iframe 发出的消息
      if (e.source !== iframeRef.current?.contentWindow) return;
      const data = e.data as {
        type?: unknown;
        nonce?: unknown;
        zoom?: unknown;
        fingerprint?: unknown;
        ctrlKey?: unknown;
        shiftKey?: unknown;
        altKey?: unknown;
        metaKey?: unknown;
        code?: unknown;
        key?: unknown;
      } | null;
      if (!data || typeof data.type !== "string") return;

      // ── 缩放上行：驱动 HUD ──
      if (data.type === ZOOM_MSG_TYPE) {
        // SEC-04：nonce 校验同 slterm_key 防线。iframe 内脚本可提取 nonce 伪造
        //（见上方威胁模型）——伪造 zoom 上报后果仅 HUD 数值误导（低危）。
        if (typeof data.nonce !== "string" || data.nonce !== nonceRef.current) return;
        const zoom = data.zoom;
        if (!isFiniteZoom(zoom)) return;
        // 等值消息（复位回声等）不复活气泡；变化才显示 + 续期 3s 计时
        if (zoom === hudZoomRef.current) return;
        hudZoomRef.current = zoom;
        setHud({ zoom, visible: true });
        if (hideTimerRef.current !== null) {
          window.clearTimeout(hideTimerRef.current);
        }
        hideTimerRef.current = window.setTimeout(() => {
          hideTimerRef.current = null;
          setHud((prev) => (prev.visible ? { ...prev, visible: false } : prev));
        }, HUD_HIDE_MS);
        return;
      }

      // ── 键盘转发：命中 global 命令才重放 ──
      if (data.type !== "slterm_key") return;
      // SEC-04：nonce 校验——注入脚本拼入的随机值（经 srcDoc 内联），不符静默丢弃。
      // iframe 内脚本可提取 nonce 伪造（见上方威胁模型）——nonce 拦截外部伪造，
      // 内部伪造由 global 命令集最小化兜底。
      if (typeof data.nonce !== "string" || data.nonce !== nonceRef.current) return;
      const fingerprint = data.fingerprint;
      if (typeof fingerprint !== "string" || fingerprint.length === 0) return;
      const registry = getShortcutRegistry();
      const globalBindings = registry.exportContextBindings("global");
      if (globalBindings.some((b) => b.keystroke === fingerprint)) {
        const event = new KeyboardEvent("keydown", {
          // 注入脚本只发 boolean/string 真值；收窄为 === true / typeof 守卫（防任意载荷）
          ctrlKey: data.ctrlKey === true,
          shiftKey: data.shiftKey === true,
          altKey: data.altKey === true,
          metaKey: data.metaKey === true,
          code: typeof data.code === "string" ? data.code : "",
          key: typeof data.key === "string" ? data.key : "",
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

  // 卸载清理 HUD 隐藏计时器（防跨实例泄漏）
  useEffect(() => {
    return () => {
      if (hideTimerRef.current !== null) {
        window.clearTimeout(hideTimerRef.current);
      }
    };
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
    // 宿主 wrapper：HUD 透明层的定位锚（iframe 自身仍是渲染区唯一交互面）
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <iframe
        ref={iframeRef}
        sandbox={SANDBOX_FLAGS}
        srcDoc={injectScript(state.html, buildInjectedScript(nonce), INJECTED_MARKER)}
        title={`HTML 预览: ${params.filePath}`}
        style={iframeStyle}
        // srcDoc 重建（filePath 切换/内容变更）后 iframe 文档内 zoom 已归 1——
        // 清 HUD 残留，防旧百分比误导至超时
        onLoad={hideHud}
      />
      {hud.visible && (
        <div style={hudLayerStyle}>
          <div
            data-e2e="html-zoom-hud"
            style={hudChipStyle}
            title="缩放比例"
          >
            <span>{formatPercent(hud.zoom)}</span>
            <button
              data-e2e="html-zoom-reset"
              style={resetBtnStyle}
              onClick={handleResetClick}
              title="重置为 100%"
            >
              重置
            </button>
          </div>
        </div>
      )}
    </div>
  );
};

export default HtmlPanel;
