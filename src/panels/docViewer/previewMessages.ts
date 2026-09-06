// previewMessages.ts — docViewer 预览框 iframe 消息协议常量单点 + 纯函数守卫
//
// 原 panels/html/zoomMath.ts 迁入（htmlviewer Ctrl+滚轮缩放协议），扩展承接
// docViewer 预览家族（htmlviewer/markdownviewer）的 iframe ↔ 父窗口消息类型：
// 上行 slterm_key / slterm_zoom /（slterm_scroll），下行 slterm_reset /
// （slterm_zoom_set / slterm_scroll_set）。服务三方：注入脚本代码生成
// （zoomRuntime/scrollRuntime 插值）、PreviewFrame 消息处理、L2 测试。
// 消息载荷与既有 slterm_key 平铺结构同构（{type, nonce, ...}），不引入包装层；
// 上行复用父窗口四层校验链，下行由 iframe 侧（source===parent + nonce + type）校验。

/** iframe → 父窗口：缩放变更上报消息类型（上行） */
export const ZOOM_MSG_TYPE = "slterm_zoom";

/** 父窗口 → iframe：复位请求消息类型（下行） */
export const RESET_MSG_TYPE = "slterm_reset";

/** 父窗口 → iframe：设值请求消息类型（下行，iframe 重建后恢复缩放——keepZoom） */
export const ZOOM_SET_MSG_TYPE = "slterm_zoom_set";

/** iframe → 父窗口：滚动比例上报消息类型（上行，scrollReport 段节流） */
export const SCROLL_MSG_TYPE = "slterm_scroll";

/** 父窗口 → iframe：滚动比例恢复消息类型（下行，keepScrollRatio） */
export const SCROLL_SET_MSG_TYPE = "slterm_scroll_set";

/** iframe → 父窗口：链接点击消息类型（上行，linkRouter 段——http(s)/本地路径
 *  分类在父侧面板做：外部 → 系统浏览器，本地 → 应用内打开） */
export const NAV_MSG_TYPE = "slterm_nav";

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

/** 上行缩放上报消息 */
export interface ZoomReport {
  type: typeof ZOOM_MSG_TYPE;
  nonce: string;
  zoom: number;
}

/** 下行复位请求消息 */
export interface ResetRequest {
  type: typeof RESET_MSG_TYPE;
  nonce: string;
}

/** 下行设值请求消息（keepZoom 恢复） */
export interface ZoomSetRequest {
  type: typeof ZOOM_SET_MSG_TYPE;
  nonce: string;
  zoom: number;
}

/** 上行滚动上报消息 */
export interface ScrollReport {
  type: typeof SCROLL_MSG_TYPE;
  nonce: string;
  /** 滚动比例 [0,1]（scrollTop / (scrollHeight - clientHeight)） */
  ratio: number;
}

/** 下行滚动恢复消息 */
export interface ScrollSetRequest {
  type: typeof SCROLL_SET_MSG_TYPE;
  nonce: string;
  ratio: number;
}

/** 上行链接点击消息 */
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
