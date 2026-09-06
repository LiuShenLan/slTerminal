// PreviewFrame.tsx — docViewer 共享预览框（iframe + srcDoc 渲染容器）
//
// 由原 panels/html/HtmlPanel.tsx 的 iframe 渲染段迁出并参数化，docViewer 预览
// 家族（htmlviewer / markdownviewer）共用同一 iframe 生命周期与 postMessage
// 总线——iframe 沙箱与消息校验是安全红线最密集处（SEC-03/04），收敛单点是
// 复用而不复制的唯一方式。
//
// 职责：
//   - iframe 生命周期：sandbox="allow-scripts"（不含 allow-same-origin——
//     Tauri CVE-2024-35222 红线）+ srcDoc 注入（injectScript + buildInjectedScript）
//   - SEC-04 nonce：面板挂载期 crypto.getRandomValues 生成一次（惰性 ref），
//     拼入注入脚本；父窗口校验 e.data.nonce 一致才转发
//   - postMessage 总线：上行 slterm_key（命中 global 命令才合成重放）/
//     slterm_zoom（驱动 HUD）；下行 slterm_reset / slterm_zoom_set（keepZoom）
//   - 缩放 HUD：瞬态气泡（百分比 + 重置），3s 无操作自动消失、期间续期；
//     等值回声不复活气泡（Chrome 缩放气泡语义）
//   - overlay 槽：形态切换条（ModeSwitcher）等悬浮 UI 的承载位，HUD 自动下移避让
//   - keepZoom：srcDoc 重建后按父侧缩放镜像下行恢复（md 面板开；html 保持
//     「重建归 100%」现状语义关）
//
// 不拥有：文件读取、loading/error 态（面板各自持有）、业务链接策略
// （html/md 各自的注入段由调用方传 segments）。

import React, { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { injectScript } from "../../lib";
import { getShortcutRegistry } from "../../features/shortcuts/ShortcutRegistry";
import {
  SECONDARY_BG,
  SEPARATOR_BG,
  SIDEBAR_FG,
  ACCENT_FG,
} from "../../theme";
import {
  buildInjectedScript,
  InjectedSegment,
  INJECTED_MARKER,
  TRUSTED_MARKER,
} from "./buildInjectedScript";
import {
  ZOOM_MSG_TYPE,
  SCROLL_MSG_TYPE,
  NAV_MSG_TYPE,
  isFiniteZoom,
  isFiniteRatio,
  formatPercent,
  buildResetRequest,
  buildZoomSetRequest,
  buildScrollSetRequest,
} from "./previewMessages";

/** PreviewFrame 接收的面板参数 */
export interface PreviewFrameProps {
  /** 注入前的原始 HTML 文档字符串（PreviewFrame 负责 injectScript + nonce 装配） */
  html: string;
  /** iframe title（L2 按 title 查询 iframe，格式由调用方定） */
  title: string;
  /** 额外注入段（html 传 fragmentNav；md 传 linkRouter/scrollReport） */
  segments?: readonly InjectedSegment[];
  /** srcDoc 重建后是否按父侧镜像恢复缩放（md 开 / html 关） */
  keepZoom?: boolean;
  /** srcDoc 重建后是否按比例恢复滚动（md 开——scrollReport 段须同传） */
  keepScrollRatio?: boolean;
  /** iframe 背景色（压重建闪白用；缺省透明） */
  iframeBg?: string;
  /** 悬浮切换条等 UI 的承载位（渲染于右上角，HUD 下移避让） */
  overlay?: React.ReactNode;
  /** e2e 探针前缀（默认 "html"——既有 html-zoom-hud 断言兼容） */
  dataE2ePrefix?: string;
  /** 链接点击透传（linkRouter 段上行 slterm_nav 校验后回调——分类/打开在面板侧） */
  onNav?: (href: string) => void;
}

/** iframe sandbox 权限：仅允许脚本执行，不含 allow-same-origin（防止 Tauri 注入 App JS） */
const SANDBOX_FLAGS = "allow-scripts";

/** iframe 全容器样式（宽高撑满 + 无边框；背景由面板传 iframeBg 压闪白） */
const iframeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "none",
};

/** 缩放 HUD 隐藏延迟（毫秒）：无缩放操作即消失（Chrome 缩放气泡语义） */
const HUD_HIDE_MS = 3000;

/** overlay 槽位（切换条等）——absolute 右上角，位于 HUD 上方 zIndex。
 *  导出供面板在无 PreviewFrame 形态（如 edit 态）以同坐标渲染悬浮切换条。 */
export const overlayBarStyle: React.CSSProperties = {
  position: "absolute",
  top: 8,
  right: 8,
  zIndex: 20,
};

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
 * HUD 透明层——占满渲染区但不挡 iframe 交互（pointerEvents: none 透传），
 * 气泡本体在其内部自开 pointerEvents: auto。overlay（切换条）在场时以
 * 行内 paddingTop 44 下移避让（基础 top/right 8 语义不变）。
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

export const PreviewFrame: React.FC<PreviewFrameProps> = ({
  html,
  title,
  segments,
  keepZoom = false,
  keepScrollRatio = false,
  iframeBg,
  overlay,
  dataE2ePrefix = "html",
  onNav,
}) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  // SEC-04：面板生命周期绑定的随机 nonce——挂载期生成一次（惰性初始化 ref，
  // StrictMode 双渲染不重复生成），经注入脚本拼入 iframe 的 keydown postMessage；
  // 父窗口校验消息 nonce 一致才转发。若每次渲染重新生成（useState 初始化器 /
  // 直接调函数），注入脚本与校验值会漂移导致键盘转发失效。
  const nonceRef = useRef<string | null>(null);
  if (nonceRef.current === null) {
    nonceRef.current = createNonce();
  }
  const nonce = nonceRef.current;

  // HUD 瞬态缩放指示器（Chrome 缩放气泡语义）：
  // zoom 变化 → 显示「百分比 + 重置」，3s 无缩放操作自动消失，期间缩放续期。
  // hudZoomRef 为逻辑真值源 + keepZoom 的父侧缩放镜像（同步读写，避免 setState
  // 异步比较竞态）；hud state 仅驱动渲染。iframe 文档内 zoom 经 slterm_zoom 上行，
  // 等值消息（复位回声）不得复活已隐藏的气泡。
  const [hud, setHud] = useState<{ zoom: number; visible: boolean }>({
    zoom: 1,
    visible: false,
  });
  const hudZoomRef = useRef(1);
  const hideTimerRef = useRef<number | null>(null);
  /** 滚动比例父侧镜像（keepScrollRatio 重建恢复取值源；等值上报不重复处理） */
  const lastScrollRatioRef = useRef(0);
  /** onNav ref——回调经 ref 转发（handleMessage 空依赖 effect 内读取最新） */
  const onNavRef = useRef(onNav);
  onNavRef.current = onNav;

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
   *   【注意】e.origin === "null" 为 opaque origin 规范推断，未经真实 WebView2 实测，
   *   正确性由收尾 L4 验证
   * - 校验 e.source === 本面板 iframe.contentWindow（防止其他窗口伪装）
   * - 校验 e.data.nonce === 面板挂载期生成的随机 nonce（SEC-04：防 iframe 内任意脚本伪造）
   * slterm_key：命中全局快捷键 → 合成 keydown 在父 window 上重放 → ShortcutRegistry
   *   正常分发；合成事件添加 __slterm_postMessage 信任标记，供 ShortcutRegistry 识别来源。
   * slterm_zoom：缩放值上报 → 驱动 HUD（等值回声不复活气泡，3s 无操作自动隐藏）；
   *   同步父侧镜像（keepZoom 重建恢复的取值源）。
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
        ratio?: unknown;
        href?: unknown;
        fingerprint?: unknown;
        ctrlKey?: unknown;
        shiftKey?: unknown;
        altKey?: unknown;
        metaKey?: unknown;
        code?: unknown;
        key?: unknown;
      } | null;
      if (!data || typeof data.type !== "string") return;

      // ── 缩放上行：驱动 HUD + 父侧镜像 ──
      if (data.type === ZOOM_MSG_TYPE) {
        // SEC-04：nonce 校验同 slterm_key 防线。iframe 内脚本可提取 nonce 伪造
        //（见 buildInjectedScript 威胁模型）——伪造 zoom 上报后果仅 HUD 数值误导（低危）。
        if (typeof data.nonce !== "string" || data.nonce !== nonceRef.current) return;
        const zoom = data.zoom;
        if (!isFiniteZoom(zoom)) return;
        // 等值消息（复位回声等）不复活气泡；变化才显示 + 续期 3s 计时
        if (zoom === hudZoomRef.current) return;
        hudZoomRef.current = zoom;
        // HUD 是滚轮即时反馈（原生 message 事件内），紧急 UI 用 flushSync 同步
        // commit——原生事件里的 setState 走并发调度会延迟一帧，缩放时气泡滞后闪烁
        flushSync(() => setHud({ zoom, visible: true }));
        if (hideTimerRef.current !== null) {
          window.clearTimeout(hideTimerRef.current);
        }
        hideTimerRef.current = window.setTimeout(() => {
          hideTimerRef.current = null;
          setHud((prev) => (prev.visible ? { ...prev, visible: false } : prev));
        }, HUD_HIDE_MS);
        return;
      }

      // ── 滚动上行：父侧镜像（keepScrollRatio 重建恢复的取值源）──
      if (data.type === SCROLL_MSG_TYPE) {
        // SEC-04：nonce 校验同缩放通道。伪造滚动上报后果仅恢复位置偏差（低危）
        if (typeof data.nonce !== "string" || data.nonce !== nonceRef.current) return;
        const ratio = data.ratio;
        if (!isFiniteRatio(ratio)) return;
        lastScrollRatioRef.current = ratio;
        return;
      }

      // ── 链接点击上行：校验后透传面板（分类/打开在面板侧 linkPolicy）──
      if (data.type === NAV_MSG_TYPE) {
        if (typeof data.nonce !== "string" || data.nonce !== nonceRef.current) return;
        if (typeof data.href !== "string" || data.href.length === 0) return;
        onNavRef.current?.(data.href);
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

  /**
   * iframe srcDoc 重建（filePath 切换/内容变更/草稿刷新）后的处理：
   * - 新文档内 zoom 已归 1——清 HUD 残留，防旧百分比误导至超时（html 现状语义）
   * - keepZoom：先取父侧镜像再 hideHud（hideHud 会复位基准），镜像非 1 则下行
   *   slterm_zoom_set 恢复（iframe 侧钳制 [ZOOM_MIN, ZOOM_MAX] 后应用并上行，
   *   上行值驱动 HUD 显示——恢复即反馈）
   * - keepScrollRatio：按比例下行恢复（iframe 侧 60ms 延时等布局收敛——
   *   滚动为近似语义，登记已知行为）
   */
  const handleLoad = () => {
    const prevZoom = hudZoomRef.current;
    const prevRatio = lastScrollRatioRef.current;
    hideHud();
    if (keepZoom && prevZoom !== 1) {
      iframeRef.current?.contentWindow?.postMessage(
        buildZoomSetRequest(nonce, prevZoom),
        "*",
      );
    }
    if (keepScrollRatio && prevRatio > 0) {
      iframeRef.current?.contentWindow?.postMessage(
        buildScrollSetRequest(nonce, prevRatio),
        "*",
      );
    }
  };

  return (
    // 宿主 wrapper：overlay/HUD 透明层的定位锚（iframe 自身仍是渲染区唯一交互面）
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      {overlay !== undefined && <div style={overlayBarStyle}>{overlay}</div>}
      <iframe
        ref={iframeRef}
        sandbox={SANDBOX_FLAGS}
        srcDoc={injectScript(html, buildInjectedScript(nonce, segments), INJECTED_MARKER)}
        title={title}
        style={iframeBg ? { ...iframeStyle, background: iframeBg } : iframeStyle}
        onLoad={handleLoad}
      />
      {hud.visible && (
        <div
          style={overlay !== undefined ? { ...hudLayerStyle, paddingTop: 44 } : hudLayerStyle}
        >
          <div
            data-e2e={`${dataE2ePrefix}-zoom-hud`}
            style={hudChipStyle}
            title="缩放比例"
          >
            <span>{formatPercent(hud.zoom)}</span>
            <button
              data-e2e={`${dataE2ePrefix}-zoom-reset`}
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
