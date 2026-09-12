// PreviewFrame.tsx — docViewer 共享预览宿主（主窗内跨源沙箱 iframe，ADR-0021）
//
// 预览内容渲染于【主窗 DOM 内跨源沙箱 iframe】（src = 自定义协议宿主页
// PREVIEW_HOST_URL，sandbox="allow-scripts"，宿主页内再嵌内容 iframe
// srcdoc）——与编辑器页签同属主窗 WebView2 DOM，OS 移窗/resize 天然像素级
// 跟随（旧独立 OS 预览窗口的轮询/节流/IPC 跟随链整体消亡，ADR-0019 载体
// 决策被 ADR-0021 推翻，安全域决策保留）。
//
// 职责：
//   - 渲染宿主 iframe 直填面板内容区（width/height 100%——显隐/几何由
//     Dockview/workspace CSS 天然驱动，无任何窗口编排代码）
//   - 内容装配：injectScript(html, buildInjectedScript(nonce, segments)) 原样
//     （注入机制在预览 CSP 域执行——宿主页域级 CSP meta 放行内联，CP-031）；
//     产物经 postMessage slterm_host_content 直推宿主（host_ready 重推兜底）
//   - 消息分派（window message 监听）：校验链 = source 归属（e.source ===
//     iframe.contentWindow）+ origin "null"（opaque 序列化，2026-09-12 spike
//     实证）→ 类型分派 → nonce + 数值守卫（文档层上行）；宿主层上行
//     （host_ready/iframe_loaded）无 nonce（桥自身信号，nonce 概念不覆盖）
//   - 下行 reset/zoom_set/scroll_set 经 postMessage（宿主页桥 relay 进内容
//     iframe——iframe 侧 source===parent + nonce 校验不变）
//   - keyfwd 上行（ADR-0021/D2 收窄转发）：nonce 校验后合成 KeyboardEvent
//     经 ShortcutRegistry resolve(ev, "global") 消费——全局快捷键预览聚焦
//     仍可用；表单焦点不转发（内容侧注入段跳过），表单键入/复制快捷键解禁
//   - ref 命令接口（PreviewFrameHandle.resetZoom）：悬浮区重置按钮下行复位
//   - keepZoom/keepScrollRatio：iframe 加载完成（slterm_iframe_loaded）后按
//     父侧镜像下行恢复（md 开；html 保持「重建归 100%」现状语义关）
//   - E2E 字体探针上行（TE-08）：slterm_font_probe 经 E2E_ENABLED 门控写
//     主窗全局 __slterm_e2e_fontProbe（键 = panelId）供 L4 断言
//
// 不拥有：文件读取、loading/error 态（面板各自持有）、缩放 HUD 显示（面板经
// useZoomHud 持有，本组件只上报 zoom 变化与承接复位命令）、业务链接策略
// （html/md 各自的注入段由调用方传 segments）。
//
// 内部镜像 zoomRef/ratioRef 为 keepZoom/keepScrollRatio 重建恢复的取值源：
// 上行变化时更新；iframe 加载完成重建归 1（下行恢复走 zoom_set → iframe 回声
// 上行 → 外层重新显示）。

import React, {
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import { injectScript } from "../../lib";
import { buildInjectedScript, InjectedSegment, INJECTED_MARKER } from "./buildInjectedScript";
import {
  ZOOM_MSG_TYPE,
  SCROLL_MSG_TYPE,
  NAV_MSG_TYPE,
  FONT_PROBE_MSG_TYPE,
  KEY_FWD_MSG_TYPE,
  HOST_READY_MSG_TYPE,
  HOST_IFRAME_LOADED_MSG_TYPE,
  HOST_CONTENT_MSG_TYPE,
  PREVIEW_HOST_URL,
  isFiniteZoom,
  isFiniteRatio,
  buildResetRequest,
  buildZoomSetRequest,
  buildScrollSetRequest,
} from "./previewMessages";
import { E2E_ENABLED } from "../../lib/e2eEnabled";
import { getShortcutRegistry } from "../../features/shortcuts/ShortcutRegistry";

/** PreviewFrame 命令接口（悬浮区重置按钮经 ref 调用下行复位） */
export interface PreviewFrameHandle {
  /** 下行 slterm_reset + 镜像归 1（iframe 内归 1 的回声上行由外层等值忽略） */
  resetZoom: () => void;
}

/** PreviewFrame 接收的面板参数 */
export interface PreviewFrameProps {
  /** 面板 panelId（data-e2e = preview-frame-<panelId> + fontProbe 全局键） */
  panelId: string;
  /** 注入前的原始 HTML 文档字符串（PreviewFrame 负责 injectScript + nonce 装配） */
  html: string;
  /** iframe title（a11y/调试定位） */
  title: string;
  /** 额外注入段（html 传 fragmentNav；md 传 linkRouter/scrollReport） */
  segments?: readonly InjectedSegment[];
  /** 内容重建后是否按父侧镜像恢复缩放（md 开 / html 关） */
  keepZoom?: boolean;
  /** 内容重建后是否按比例恢复滚动（md 开——scrollReport 段须同传） */
  keepScrollRatio?: boolean;
  /** iframe 背景色（压重建闪白用；缺省透明） */
  iframeBg?: string;
  /** 缩放变化上报（zoom !== 镜像时；含回落 100% 的变化——外层 HUD 状态机显示） */
  onZoomChange?: (zoom: number) => void;
  /** 内容重建归 1 通知（重建即静默复位——外层 HUD 直接隐藏，非「显示 100%」） */
  onZoomReset?: () => void;
  /** 链接点击透传（linkRouter 段上行 slterm_nav 校验后回调——分类/打开在面板侧） */
  onNav?: (href: string) => void;
  /** React 19 ref as prop——命令接口（重置缩放下行） */
  ref?: React.Ref<PreviewFrameHandle>;
}

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
  panelId,
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
  /** 宿主 iframe（主窗内跨源沙箱——postMessage source 归属校验锚点） */
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  // SEC-04：面板生命周期绑定的随机 nonce——挂载期生成一次（惰性初始化 ref，
  // StrictMode 双渲染不重复生成），拼入注入脚本 + 上下行消息校验。
  const nonceRef = useRef<string | null>(null);
  if (nonceRef.current === null) {
    nonceRef.current = createNonce();
  }
  const nonce = nonceRef.current;

  /** 宿主页桥就绪（slterm_host_ready 上行置位）——内容推送门控 */
  const [hostReady, setHostReady] = useState(false);

  /** 缩放镜像（keepZoom 重建恢复取值源；上行变化/复位同步，等值不重复处理） */
  const zoomRef = useRef(1);
  /** 滚动比例父侧镜像（keepScrollRatio 重建恢复取值源；等值上报不重复处理） */
  const lastScrollRatioRef = useRef(0);
  /** 上行回调 ref 转发（事件处理器内读取最新——onNav 同模式） */
  const onZoomChangeRef = useRef(onZoomChange);
  onZoomChangeRef.current = onZoomChange;
  const onZoomResetRef = useRef(onZoomReset);
  onZoomResetRef.current = onZoomReset;
  const onNavRef = useRef(onNav);
  onNavRef.current = onNav;

  /** 下行 postMessage（宿主桥 relay 进内容 iframe——type/nonce 由 iframe 侧校验） */
  const postDownlink = useCallback((msg: object) => {
    iframeRef.current?.contentWindow?.postMessage(msg, "*");
  }, []);

  /** 下行复位：→ 宿主桥 → iframe 内 zoom 归 1 并回声上报；镜像归 1 */
  const resetZoom = useCallback(() => {
    postDownlink(buildResetRequest(nonce));
    zoomRef.current = 1;
  }, [nonce, postDownlink]);
  useImperativeHandle(ref, () => ({ resetZoom }), [resetZoom]);

  // ── 内容装配（注入机制原样：injectScript + buildInjectedScript + nonce）──
  const segKey = useMemo(
    () => (segments ?? []).map((s) => s.kind).join(","),
    [segments],
  );
  // segments 为调用方每渲染新建数组——以 kind 序列键做依赖（内容/段型变化即重建）
  const srcDoc = useMemo(
    () => injectScript(html, buildInjectedScript(nonce, segments), INJECTED_MARKER),
    [html, nonce, segKey],
  );

  // ── 内容推送：宿主桥就绪后直推（host_ready 重推兜底——桥后于装配产物就绪
  //    时由 ready 上行触发补推；装配产物变化即重推）──
  useEffect(() => {
    if (!hostReady) return;
    const win = iframeRef.current?.contentWindow;
    if (!win) return;
    win.postMessage(
      { type: HOST_CONTENT_MSG_TYPE, html: srcDoc, bg: iframeBg ?? "" },
      "*",
    );
  }, [hostReady, srcDoc, iframeBg]);

  // ── 消息分派（window message 监听；校验链 = source 归属 + origin "null" →
  //    类型分派 → nonce + 数值守卫）──
  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      const frame = iframeRef.current;
      if (!frame || e.source !== frame.contentWindow) return;
      // opaque origin 序列化 "null"（2026-09-12 spike 实证）——跨源沙箱 iframe
      // 上行 origin 恒为 "null"，其余 origin 一律丢弃
      if (e.origin !== "null") return;
      const data = e.data as {
        type?: unknown;
        nonce?: unknown;
        zoom?: unknown;
        ratio?: unknown;
        href?: unknown;
        loaded?: unknown;
        code?: unknown;
        ctrlKey?: unknown;
        shiftKey?: unknown;
        altKey?: unknown;
        metaKey?: unknown;
      } | null;
      if (!data || typeof data.type !== "string") return;
      const type = data.type;

      // ── 宿主层上行（桥自身信号，无 nonce 概念）──
      if (type === HOST_READY_MSG_TYPE) {
        setHostReady(true);
        return;
      }
      if (type === HOST_IFRAME_LOADED_MSG_TYPE) {
        // 内容重建完成（新文档内 zoom/滚动已归零）：
        // - 镜像归 1 + 上报外层隐藏 HUD（防旧百分比误导至超时；html 现状语义）
        // - keepZoom：镜像非 1 则下行 slterm_zoom_set 恢复（iframe 侧钳制后
        //   应用并上行——恢复即反馈）
        // - keepScrollRatio：按比例下行恢复（60ms 延时等布局收敛，近似语义）
        const prevZoom = zoomRef.current;
        const prevRatio = lastScrollRatioRef.current;
        zoomRef.current = 1;
        onZoomResetRef.current?.();
        if (keepZoom && prevZoom !== 1) {
          postDownlink(buildZoomSetRequest(nonce, prevZoom));
        }
        if (keepScrollRatio && prevRatio > 0) {
          postDownlink(buildScrollSetRequest(nonce, prevRatio));
        }
        return;
      }

      // ── 文档层上行（nonce 校验——SEC-04；威胁模型见 buildInjectedScript）──
      if (typeof data.nonce !== "string" || data.nonce !== nonceRef.current) return;

      // ── keyfwd 收窄转发（ADR-0021/D2）：合成 KeyboardEvent 经
      //    ShortcutRegistry global context 解析消费（预览聚焦时全局快捷键
      //    可用）；不 dispatchEvent 重放（旧 slterm_key 命令重放语义不复活）──
      if (type === KEY_FWD_MSG_TYPE) {
        if (typeof data.code !== "string" || data.code.length === 0) return;
        const ev = new KeyboardEvent("keydown", {
          code: data.code,
          ctrlKey: data.ctrlKey === true,
          shiftKey: data.shiftKey === true,
          altKey: data.altKey === true,
          metaKey: data.metaKey === true,
        });
        getShortcutRegistry().resolve(ev, "global");
        return;
      }
      // ── 缩放上行：父侧镜像更新 + 上报外层（HUD 显示在悬浮区）──
      if (type === ZOOM_MSG_TYPE) {
        const zoom = data.zoom;
        if (!isFiniteZoom(zoom)) return;
        // 等值消息（复位回声等）：镜像已同值——不重复上报外层
        if (zoom === zoomRef.current) return;
        zoomRef.current = zoom;
        onZoomChangeRef.current?.(zoom);
        return;
      }
      // ── 滚动上行：父侧镜像（keepScrollRatio 重建恢复的取值源）──
      if (type === SCROLL_MSG_TYPE) {
        const ratio = data.ratio;
        if (!isFiniteRatio(ratio)) return;
        lastScrollRatioRef.current = ratio;
        return;
      }
      // ── 链接点击上行：校验后透传面板（分类/打开在面板侧 linkPolicy）──
      if (type === NAV_MSG_TYPE) {
        if (typeof data.href !== "string" || data.href.length === 0) return;
        onNavRef.current?.(data.href);
        return;
      }
      // ── E2E 字体加载探针上行（TE-08 分支 b）：E2E_ENABLED 门控写主窗全局
      //    （L4 断言读取；生产构建 E2E_ENABLED 编译期 false → 整块 tree-shake，
      //    且该段仅 VITE_E2E 构建注入——双保险；键 = panelId）──
      if (type === FONT_PROBE_MSG_TYPE) {
        if (E2E_ENABLED) {
          const w = window as unknown as {
            __slterm_e2e_fontProbe?: Record<string, boolean>;
          };
          w.__slterm_e2e_fontProbe = {
            ...(w.__slterm_e2e_fontProbe ?? {}),
            [panelId]: data.loaded === true,
          };
        }
        return;
      }
      // 其余类型（含已退役的旧键转发类型与未知类型）静默丢弃——上行终态集合
      // = UPLINK_MSG_TYPES 白名单（守卫测试锁死）
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
    // keepZoom/keepScrollRatio 语义随 props 变化即时生效（re-render 重建 effect）
  }, [nonce, panelId, keepZoom, keepScrollRatio, postDownlink]);

  return (
    // 宿主 iframe：面板内容区直填（跨源沙箱——自定义协议宿主页；显隐/几何随
    // 主窗 DOM 天然跟随，无任何窗口编排）
    <iframe
      ref={iframeRef}
      src={PREVIEW_HOST_URL}
      sandbox="allow-scripts"
      title={title}
      data-e2e={`preview-frame-${panelId}`}
      style={{ display: "block", width: "100%", height: "100%", border: "none" }}
    />
  );
};
