// previewMessages.ts — docViewer 预览消息协议常量单点 + 纯函数守卫
//
// 原 panels/html/zoomMath.ts 迁入（htmlviewer Ctrl+滚轮缩放协议），扩展承接
// docViewer 预览家族（htmlviewer/markdownviewer）的消息协议：
//
// 【S10-② 迁独立 webview 后的双层通道（ADR-0019）】
//  1. 文档层（iframe ↔ 宿主页，仍为 window.postMessage）：预览内容渲染于独立
//     webview 宿主页内的 sandbox iframe（srcdoc），注入脚本（zoomRuntime/
//     scrollRuntime/linkRouter 段）运行于该 iframe——上行 slterm_zoom /
//     slterm_scroll / slterm_nav、下行 slterm_reset / slterm_zoom_set /
//     slterm_scroll_set，校验语义原样（opaque origin + nonce + 白名单）。
//     targetOrigin "*" 语义同前（iframe opaque；宿主页四层校验兜底）。
//  2. 窗口层（宿主页 ↔ 主窗口 PreviewFrame，Tauri event）：跨独立 WebviewWindow
//     无 window.postMessage 通道（spike 实证）——宿主页桥把 iframe 消息转发为
//     上行事件，主窗下行事件经桥注入 iframe；事件名单点登记于 src/ipc/preview.ts
//     （通信层），本文仅登记文档层消息类型与上/下行类型白名单。
//
// 消息载荷与既有 slterm_* 平铺结构同构（{type, nonce, ...}），不引入包装层。
// 上行类型白名单 = 渲染态集合（zoom/scroll/nav）——无命令/按键重放通道
// （CP-013：键转发上行与信任标记随 webview 迁移整体退役，旧类型名零残留；
// 终态集合由守卫测试锁死）。下行类型白名单 = 控制集合（reset/zoom_set/
// scroll_set）（CP-044 守卫）。

/** iframe → 宿主页：缩放变更上报消息类型（上行，经桥转发主窗） */
export const ZOOM_MSG_TYPE = "slterm_zoom";

/** 主窗 → iframe：复位请求消息类型（下行，经桥注入） */
export const RESET_MSG_TYPE = "slterm_reset";

/** 主窗 → iframe：设值请求消息类型（下行，iframe 重建后恢复缩放——keepZoom） */
export const ZOOM_SET_MSG_TYPE = "slterm_zoom_set";

/** iframe → 宿主页：滚动比例上报消息类型（上行，scrollReport 段节流，经桥转发） */
export const SCROLL_MSG_TYPE = "slterm_scroll";

/** 主窗 → iframe：滚动比例恢复消息类型（下行，keepScrollRatio） */
export const SCROLL_SET_MSG_TYPE = "slterm_scroll_set";

/** iframe → 宿主页：链接点击消息类型（上行，linkRouter 段——http(s)/本地路径
 *  分类在面板侧做：外部 → 系统浏览器，本地 → 应用内打开） */
export const NAV_MSG_TYPE = "slterm_nav";

// ── 窗口层事件通道（S10-②：独立 webview 消息桥 = Tauri event，CP-044 通道退役）──
// 事件名与载荷形态登记于 src/ipc/preview.ts（跨窗口通道属通信层，单点定义，
// 本文不重复）——上行 = iframe 文档消息（本文件类型白名单）+ 宿主状态事件。

/** 上行消息类型白名单（终态，CP-013 写死）：恰好为渲染态集合 */
export const UPLINK_MSG_TYPES = [
  ZOOM_MSG_TYPE,
  SCROLL_MSG_TYPE,
  NAV_MSG_TYPE,
] as const;

/** 下行消息类型白名单（终态，CP-044 守卫）：恰好为控制集合 */
export const DOWNLINK_MSG_TYPES = [
  RESET_MSG_TYPE,
  ZOOM_SET_MSG_TYPE,
  SCROLL_SET_MSG_TYPE,
] as const;

/** 上行类型守卫：仅白名单渲染态通过（其余静默丢弃） */
export function isUplinkType(t: unknown): t is (typeof UPLINK_MSG_TYPES)[number] {
  return typeof t === "string" && (UPLINK_MSG_TYPES as readonly string[]).includes(t);
}

/** 下行类型守卫：仅白名单控制集通过 */
export function isDownlinkType(t: unknown): t is (typeof DOWNLINK_MSG_TYPES)[number] {
  return typeof t === "string" && (DOWNLINK_MSG_TYPES as readonly string[]).includes(t);
}

/** 缩放下限（25%） */
export const ZOOM_MIN = 0.25;

/** 缩放上限（400%） */
export const ZOOM_MAX = 4;

/** 每格等比步进系数（×/÷ 1.1 ≈ ±10% 档） */
export const ZOOM_STEP = 1.1;

/** wheel 步进阈值：deltaY 累计像素（≈1 格滚轮刻度） */
export const WHEEL_STEP_PX = 100;

/** 数值整理精度（round6）：浮点尾串收敛，保证同值全等比较稳定 */
export const ZOOM_ROUND = 1e6;

/** 夹紧缩放值到 [ZOOM_MIN, ZOOM_MAX] */
export function clampZoom(z: number): number {
  if (z < ZOOM_MIN) return ZOOM_MIN;
  if (z > ZOOM_MAX) return ZOOM_MAX;
  return z;
}

/** 缩放值 → 显示百分比字符串（取整），如 1.331 → "133%" */
export function formatPercent(z: number): string {
  return `${Math.round(z * 100)}%`;
}

/** 上行缩放上报消息（iframe 文档层） */
export interface ZoomReport {
  type: typeof ZOOM_MSG_TYPE;
  nonce: string;
  zoom: number;
}

/** 下行复位请求消息（iframe 文档层） */
export interface ResetRequest {
  type: typeof RESET_MSG_TYPE;
  nonce: string;
}

/** 下行设值请求消息（iframe 文档层，keepZoom 恢复） */
export interface ZoomSetRequest {
  type: typeof ZOOM_SET_MSG_TYPE;
  nonce: string;
  zoom: number;
}

/** 上行滚动上报消息（iframe 文档层） */
export interface ScrollReport {
  type: typeof SCROLL_MSG_TYPE;
  nonce: string;
  /** 滚动比例 [0,1]（scrollTop / (scrollHeight - clientHeight)） */
  ratio: number;
}

/** 下行滚动恢复消息（iframe 文档层） */
export interface ScrollSetRequest {
  type: typeof SCROLL_SET_MSG_TYPE;
  nonce: string;
  ratio: number;
}

/** 上行链接点击消息（iframe 文档层） */
export interface NavReport {
  type: typeof NAV_MSG_TYPE;
  nonce: string;
  href: string;
}

/** 构造上行缩放上报消息 */
export function buildZoomReport(nonce: string, zoom: number): ZoomReport {
  return { type: ZOOM_MSG_TYPE, nonce, zoom };
}

/** 构造下行复位请求消息 */
export function buildResetRequest(nonce: string): ResetRequest {
  return { type: RESET_MSG_TYPE, nonce };
}

/** 构造下行设值请求消息（iframe 重建后按父侧镜像恢复缩放） */
export function buildZoomSetRequest(nonce: string, zoom: number): ZoomSetRequest {
  return { type: ZOOM_SET_MSG_TYPE, nonce, zoom };
}

/** 构造上行滚动上报消息 */
export function buildScrollReport(nonce: string, ratio: number): ScrollReport {
  return { type: SCROLL_MSG_TYPE, nonce, ratio };
}

/** 构造下行滚动恢复消息 */
export function buildScrollSetRequest(nonce: string, ratio: number): ScrollSetRequest {
  return { type: SCROLL_SET_MSG_TYPE, nonce, ratio };
}

/** 构造上行链接点击消息 */
export function buildNavReport(nonce: string, href: string): NavReport {
  return { type: NAV_MSG_TYPE, nonce, href };
}

/** 滚动比例守卫：仅接受 [0,1] 有限 number */
export function isFiniteRatio(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= 1;
}

/** 消息数值守卫：仅接受有限 number（拒绝 NaN/Infinity/非 number） */
export function isFiniteZoom(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v);
}
