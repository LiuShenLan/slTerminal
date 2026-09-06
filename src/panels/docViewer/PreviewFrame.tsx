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
//     slterm_zoom（校验后经 onZoomChange 上报——显示层在面板根悬浮区
//     FloatingArea/useZoomHud，2026-09-06 收敛）；下行 slterm_reset /
//     slterm_zoom_set（keepZoom）
//   - ref 命令接口（PreviewFrameHandle.resetZoom）：悬浮区重置按钮下行复位
//   - keepZoom/keepScrollRatio：srcDoc 重建后按父侧镜像下行恢复（md 开；
//     html 保持「重建归 100%」现状语义关）
//
// 不拥有：文件读取、loading/error 态（面板各自持有）、缩放 HUD 显示（面板经
// useZoomHud 持有，本组件只上报 zoom 变化与承接复位命令）、业务链接策略
// （html/md 各自的注入段由调用方传 segments）。
//
// 内部镜像 zoomRef 为 keepZoom 重建恢复的取值源：上行 zoom 变化时更新；
// handleLoad 重建归 1（下行恢复走 zoom_set → iframe 回声上行 → 外层重新显示）。

import React, { useCallback, useEffect, useImperativeHandle, useRef } from "react";
import { injectScript } from "../../lib";
import { getShortcutRegistry } from "../../features/shortcuts/ShortcutRegistry";
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
  buildResetRequest,
  buildZoomSetRequest,
  buildScrollSetRequest,
} from "./previewMessages";

/** PreviewFrame 命令接口（悬浮区重置按钮经 ref 调用下行复位） */
export interface PreviewFrameHandle {
  /** 下行 slterm_reset + 镜像归 1（iframe 内归 1 的回声上行由外层等值忽略） */
  resetZoom: () => void;
}

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
  /** 缩放变化上报（zoom !== 镜像时；含回落 100% 的变化——外层 HUD 状态机显示） */
  onZoomChange?: (zoom: number) => void;
  /** iframe 重建归 1 通知（重建即静默复位——外层 HUD 直接隐藏，非「显示 100%」） */
  onZoomReset?: () => void;
  /** 链接点击透传（linkRouter 段上行 slterm_nav 校验后回调——分类/打开在面板侧） */
  onNav?: (href: string) => void;
  /** React 19 ref as prop——命令接口（重置缩放下行） */
  ref?: React.Ref<PreviewFrameHandle>;
}

/** iframe sandbox 权限：仅允许脚本执行，不含 allow-same-origin（防止 Tauri 注入 App JS） */
const SANDBOX_FLAGS = "allow-scripts";

/** iframe 全容器样式（宽高撑满 + 无边框；背景由面板传 iframeBg 压闪白） */
const iframeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "none",
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

export const PreviewFrame: React.FC<PreviewFrameProps> = ({
  html,
  title,
  segments,
  keepZoom = false,
  keepScrollRatio = false,
  iframeBg,
  onZoomChange,
  onZoomReset,
  onNav,
  ref,
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

  /** 缩放镜像（keepZoom 重建恢复取值源；上行变化/复位同步，等值不重复处理） */
  const zoomRef = useRef(1);
  /** 滚动比例父侧镜像（keepScrollRatio 重建恢复取值源；等值上报不重复处理） */
  const lastScrollRatioRef = useRef(0);
  /** 上行回调 ref 转发（handleMessage/onLoad 内读取最新——onNav 同模式） */
  const onZoomChangeRef = useRef(onZoomChange);
  onZoomChangeRef.current = onZoomChange;
  const onZoomResetRef = useRef(onZoomReset);
  onZoomResetRef.current = onZoomReset;
  const onNavRef = useRef(onNav);
  onNavRef.current = onNav;

  /** 重置缩放命令：下行复位请求 → iframe 内 zoom 归 1 并回声上报；镜像归 1（外层 HUD 经 onReset 链同步隐藏） */
  const resetZoom = useCallback(() => {
    iframeRef.current?.contentWindow?.postMessage(
      buildResetRequest(nonce),
      // targetOrigin 统一 "*"（2026-09-06 实证，与上行同因）：子侧 source===parent +
      // nonce + type 三重校验保证安全，无需依赖 targetOrigin 收窄
      "*",
    );
    zoomRef.current = 1;
  }, [nonce]);
  useImperativeHandle(ref, () => ({ resetZoom }), [resetZoom]);

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
   * slterm_zoom：缩放值上报（变化才通知——等值回声不打扰外层 HUD）→ 同步父侧镜像
   *   （keepZoom 重建恢复的取值源）；显示层（HUD 气泡）在面板根悬浮区，经
   *   onZoomChange 上行驱动。
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

      // ── 缩放上行：父侧镜像更新 + 上报外层（HUD 显示在悬浮区）──
      if (data.type === ZOOM_MSG_TYPE) {
        // SEC-04：nonce 校验同 slterm_key 防线。iframe 内脚本可提取 nonce 伪造
        //（见 buildInjectedScript 威胁模型）——伪造 zoom 上报后果仅 HUD 数值误导（低危）。
        if (typeof data.nonce !== "string" || data.nonce !== nonceRef.current) return;
        const zoom = data.zoom;
        if (!isFiniteZoom(zoom)) return;
        // 等值消息（复位回声等）：镜像已同值——不重复上报外层（外层状态同步）
        if (zoom === zoomRef.current) return;
        zoomRef.current = zoom;
        onZoomChangeRef.current?.(zoom);
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

  /**
   * iframe srcDoc 重建（filePath 切换/内容变更/草稿刷新）后的处理：
   * - 新文档内 zoom/滚动已归零——镜像归 1 + 上报外层隐藏 HUD（防旧百分比误导
   *   至超时；html 现状语义）
   * - keepZoom：镜像非 1 则下行 slterm_zoom_set 恢复（iframe 侧钳制
   *   [ZOOM_MIN, ZOOM_MAX] 后应用并上行，上行值驱动外层 HUD 显示——恢复即反馈）
   * - keepScrollRatio：按比例下行恢复（iframe 侧 60ms 延时等布局收敛——
   *   滚动为近似语义，登记已知行为）
   */
  const handleLoad = () => {
    const prevZoom = zoomRef.current;
    const prevRatio = lastScrollRatioRef.current;
    zoomRef.current = 1;
    // 重建归 1 = 静默复位：通知外层隐藏 HUD（非「显示 100%」——onZoomChange
    // 只承载真实缩放变化）；keepZoom 恢复走下方 zoom_set → iframe 回声上行
    //（变化值）→ onZoomChange 重新显示——时序上先隐藏后恢复
    onZoomResetRef.current?.();
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
    // 宿主 wrapper：iframe 渲染区定位锚（背景由 iframeBg 压闪白）
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <iframe
        ref={iframeRef}
        sandbox={SANDBOX_FLAGS}
        srcDoc={injectScript(html, buildInjectedScript(nonce, segments), INJECTED_MARKER)}
        title={title}
        style={iframeBg ? { ...iframeStyle, background: iframeBg } : iframeStyle}
        onLoad={handleLoad}
      />
    </div>
  );
};
