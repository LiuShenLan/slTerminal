// PreviewFrame.tsx — docViewer 共享预览宿主（主窗侧，S10-② 迁独立 webview）
//
// 由原 PreviewFrame（主窗口内 sandbox iframe + srcDoc + postMessage 总线）迁出：
// 预览内容现在渲染于【独立 Tauri WebviewWindow】（label = preview-<panelId>，
// 专用宿主页 + 宿主页内 sandbox iframe srcdoc，ADR-0019）——本组件是主窗侧
// 对预览窗口的编排层：
//
// 职责：
//   - 窗口生命周期：面板内容区锚点（anchor）几何 → 经 ipc.preview 同步窗口
//     位置/尺寸/显隐（轮询 + 主窗移动监听；面板隐藏/页面切换 → 窗口隐藏不
//     销毁——缩放/滚动态保活，CP-037 复核语义）；卸载 → 销毁窗口
//   - 内容装配：injectScript(html, buildInjectedScript(nonce, segments)) 原样
//     装配（注入机制迁入预览 CSP 域——自定义协议宿主页，域内无全局 CSP，
//     无差别字符串级转义已消亡，CP-031）；产物经 preview_render
//     推送（后端存储 + 定向通知宿主拉取）
//   - 消息桥（Tauri event，CP-044 通道退役分支）：上行 slterm_zoom /
//     slterm_scroll / slterm_nav（经宿主桥转发）按 label 过滤 + nonce 校验 +
//     类型白名单（UPLINK_MSG_TYPES，CP-013 终态集合——无按键重放分支，
//     未知类型静默丢弃）；下行 reset/zoom_set/scroll_set 经事件注入宿主 →
//     iframe（iframe 侧 source===parent + nonce 校验不变）
//   - ref 命令接口（PreviewFrameHandle.resetZoom）：悬浮区重置按钮下行复位
//   - keepZoom/keepScrollRatio：iframe 加载完成（宿主状态事件）后按父侧镜像
//     下行恢复（md 开；html 保持「重建归 100%」现状语义关）
//
// 键盘语义：预览窗口 focusable=false——键盘焦点恒在主窗口 ShortcutRegistry
// 域，全局快捷键（Ctrl+W 等）在预览聚焦时仍可用（CP-013 步骤 4 口径）；
// 代价 = 预览文档内表单键入/原生复制快捷键不可达，登记已知行为
// （docViewer/CLAUDE.md）。
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
} from "react";
import { injectScript } from "../../lib";
import {
  emitPreviewDownlink,
  makePreviewLabel,
  onPreviewHostStatus,
  onPreviewUplink,
  previewClose,
  previewRender,
  previewSync,
} from "../../ipc/preview";
import { buildInjectedScript, InjectedSegment, INJECTED_MARKER } from "./buildInjectedScript";
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
import { onMainWindowMoved } from "../../ipc/window";

/** PreviewFrame 命令接口（悬浮区重置按钮经 ref 调用下行复位） */
export interface PreviewFrameHandle {
  /** 下行 slterm_reset + 镜像归 1（iframe 内归 1 的回声上行由外层等值忽略） */
  resetZoom: () => void;
}

/** PreviewFrame 接收的面板参数 */
export interface PreviewFrameProps {
  /** 面板 panelId（预览窗口 label = preview-<panelId>——WDIO 驱动句柄契约） */
  panelId: string;
  /** 注入前的原始 HTML 文档字符串（PreviewFrame 负责 injectScript + nonce 装配） */
  html: string;
  /** iframe title（保留——宿主/未来调试定位用） */
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

/** 几何轮询间隔（ms）：面板显隐/尺寸变化（页面 CSS 切换无事件源）检测 */
const GEOMETRY_POLL_MS = 200;

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
  segments,
  keepZoom = false,
  keepScrollRatio = false,
  iframeBg,
  onZoomChange,
  onZoomReset,
  onNav,
  ref,
}) => {
  // 预览窗口 label（WDIO 驱动句柄契约：句柄 = label）
  const label = makePreviewLabel(panelId);

  /** 锚点容器（面板内容区——预览窗口几何 = 其视口矩形） */
  const anchorRef = useRef<HTMLDivElement | null>(null);
  // SEC-04：面板生命周期绑定的随机 nonce——挂载期生成一次（惰性初始化 ref，
  // StrictMode 双渲染不重复生成），拼入注入脚本 + 上下行消息校验。
  const nonceRef = useRef<string | null>(null);
  if (nonceRef.current === null) {
    nonceRef.current = createNonce();
  }
  const nonce = nonceRef.current;

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

  /** 下行复位：事件 → 宿主 → iframe 内 zoom 归 1 并回声上报；镜像归 1 */
  const resetZoom = useCallback(() => {
    void emitPreviewDownlink({
      label,
      type: buildResetRequest(nonce).type,
      nonce,
    });
    zoomRef.current = 1;
  }, [label, nonce]);
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

  // ── 内容推送：装配产物变化 → preview_render（后端存储 + 定向通知宿主拉取；
  //    窗口尚未创建/宿主未就绪时由宿主加载后主动拉取兜底）──
  useEffect(() => {
    void previewRender(label, srcDoc, iframeBg).catch(() => {
      /* 后端拒绝/窗口域异常——预览不可用但不阻断面板（可观测性优先记录） */
      console.warn("[slTerminal] 预览内容推送失败:", label);
    });
  }, [label, srcDoc, iframeBg]);

  // ── 窗口生命周期 + 几何同步（轮询 + 主窗移动即时触发）──
  useEffect(() => {
    let disposed = false;
    let lastX = NaN;
    let lastY = NaN;
    let lastW = NaN;
    let lastH = NaN;
    let lastVis: boolean | null = null;

    /** 测量锚点矩形 → 变化时 preview_sync（CSS 视口坐标，后端换算物理屏幕坐标） */
    const syncNow = () => {
      if (disposed) return;
      const el = anchorRef.current;
      if (!el) return;
      const r = el.getBoundingClientRect();
      const w = r.width;
      const h = r.height;
      const visible = w >= 1 && h >= 1;
      // 亚像素抖动抑制：半像素粒度比较
      const round2 = (v: number) => Math.round(v * 2) / 2;
      const x = round2(r.x);
      const y = round2(r.y);
      const rw = round2(w);
      const rh = round2(h);
      if (x === lastX && y === lastY && rw === lastW && rh === lastH && visible === lastVis) {
        return;
      }
      lastX = x;
      lastY = y;
      lastW = rw;
      lastH = rh;
      lastVis = visible;
      void previewSync(label, x, y, rw, rh, visible).catch(() => {
        /* 窗口域异常——轮询下轮自愈 */
      });
    };

    syncNow();
    const timer = window.setInterval(syncNow, GEOMETRY_POLL_MS);
    // 主窗拖动/移动：几何换算基准变化——即时同步（轮询兜底防丢）
    const offMove = onMainWindowMoved(() => {
      // 移动事件高频——合并到下一帧/宏任务（防 invoke 风暴）
      window.setTimeout(syncNow, 0);
    });

    return () => {
      disposed = true;
      window.clearInterval(timer);
      offMove();
      // 面板卸载 → 销毁预览窗口（缩放/滚动态随窗口销毁——与旧 iframe 关页签销毁同语义）
      void previewClose(label).catch(() => {
        /* 窗口已不存在——幂等 */
      });
    };
  }, [label]);

  // ── 上行消息处理（Tauri event 桥；校验链 = 旧 iframe 通道同款：
  //    label 归属 → 类型白名单 → nonce → 数值守卫）──
  useEffect(() => {
    const handleUplink = (msg: {
      label: string;
      type: string;
      nonce?: unknown;
      zoom?: unknown;
      ratio?: unknown;
      href?: unknown;
    }) => {
      // 载荷守卫：非对象/缺 label 静默丢弃（事件通道载荷不可信，防异常上行）
      if (!msg || typeof msg.label !== "string") return;
      // label 归属：仅接受本面板预览窗口（多预览面板并存互不干扰）
      if (msg.label !== label) return;
      const type = msg.type;
      // ── 缩放上行：父侧镜像更新 + 上报外层（HUD 显示在悬浮区）──
      if (type === ZOOM_MSG_TYPE) {
        // SEC-04：nonce 校验（预览内容与注入脚本同文档——内容可提取伪造，
        // 伪造 zoom 上报后果仅 HUD 数值误导（低危），威胁模型见 buildInjectedScript）
        if (typeof msg.nonce !== "string" || msg.nonce !== nonceRef.current) return;
        const zoom = msg.zoom;
        if (!isFiniteZoom(zoom)) return;
        // 等值消息（复位回声等）：镜像已同值——不重复上报外层
        if (zoom === zoomRef.current) return;
        zoomRef.current = zoom;
        onZoomChangeRef.current?.(zoom);
        return;
      }
      // ── 滚动上行：父侧镜像（keepScrollRatio 重建恢复的取值源）──
      if (type === SCROLL_MSG_TYPE) {
        if (typeof msg.nonce !== "string" || msg.nonce !== nonceRef.current) return;
        const ratio = msg.ratio;
        if (!isFiniteRatio(ratio)) return;
        lastScrollRatioRef.current = ratio;
        return;
      }
      // ── 链接点击上行：校验后透传面板（分类/打开在面板侧 linkPolicy）──
      if (type === NAV_MSG_TYPE) {
        if (typeof msg.nonce !== "string" || msg.nonce !== nonceRef.current) return;
        if (typeof msg.href !== "string" || msg.href.length === 0) return;
        onNavRef.current?.(msg.href);
        return;
      }
      // 其余类型（含已退役的键转发类型与未知类型）静默丢弃——上行终态集合
      // = 渲染态白名单（CP-013/044，UPLINK_MSG_TYPES 守卫测试锁死）
    };
    const offUplink = onPreviewUplink(handleUplink);
    return offUplink;
  }, [label]);

  // ── 宿主状态：iframe 加载完成 = 内容重建完成（旧 onLoad 语义随迁）──
  useEffect(() => {
    const handleHostStatus = (msg: { label: string; status: string }) => {
      if (msg.label !== label) return;
      if (msg.status !== "iframe-loaded") return;
      // 内容重建后的处理（新文档内 zoom/滚动已归零）：
      // - 镜像归 1 + 上报外层隐藏 HUD（防旧百分比误导至超时；html 现状语义）
      // - keepZoom：镜像非 1 则下行 slterm_zoom_set 恢复（iframe 侧钳制后
      //   应用并上行——恢复即反馈）
      // - keepScrollRatio：按比例下行恢复（60ms 延时等布局收敛，近似语义）
      const prevZoom = zoomRef.current;
      const prevRatio = lastScrollRatioRef.current;
      zoomRef.current = 1;
      onZoomResetRef.current?.();
      if (keepZoom && prevZoom !== 1) {
        void emitPreviewDownlink({
          label,
          type: buildZoomSetRequest(nonce, prevZoom).type,
          nonce,
          zoom: prevZoom,
        });
      }
      if (keepScrollRatio && prevRatio > 0) {
        void emitPreviewDownlink({
          label,
          type: buildScrollSetRequest(nonce, prevRatio).type,
          nonce,
          ratio: prevRatio,
        });
      }
    };
    const offStatus = onPreviewHostStatus(handleHostStatus);
    return offStatus;
    // keepZoom/keepScrollRatio 语义随 props 变化即时生效（re-render 重建 effect）
  }, [label, nonce, keepZoom, keepScrollRatio]);

  return (
    // 锚点容器：面板内容区几何（预览窗口 = 覆盖本矩形的 owned 无边框窗口；
    // 主窗口本区域不再有可见内容——FloatingArea 恒位于其上方工具条带内）
    <div
      ref={anchorRef}
      style={{ position: "relative", width: "100%", height: "100%" }}
    />
  );
};
