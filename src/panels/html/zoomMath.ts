// zoomMath.ts — htmlviewer Ctrl+滚轮缩放的协议常量单点 + 纯数学函数
//
// 服务三方：注入脚本代码生成（zoomRuntime 插值）、HtmlPanel 消息处理、L2 测试。
// 消息载荷与既有 slterm_key 平铺结构同构（{type, nonce, ...}），不引入包装层；
// 上行 slterm_zoom 复用父窗口四层校验链，下行 slterm_reset 由 iframe 侧
// （source===parent + nonce + type）校验。

/** iframe → 父窗口：缩放变更上报消息类型（上行） */
export const ZOOM_MSG_TYPE = "slterm_zoom";

/** 父窗口 → iframe：复位请求消息类型（下行） */
export const RESET_MSG_TYPE = "slterm_reset";

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

/** 构造上行缩放上报消息 */
export function buildZoomReport(nonce: string, zoom: number): ZoomReport {
  return { type: ZOOM_MSG_TYPE, nonce, zoom };
}

/** 构造下行复位请求消息 */
export function buildResetRequest(nonce: string): ResetRequest {
  return { type: RESET_MSG_TYPE, nonce };
}

/** 消息数值守卫：仅接受有限 number（拒绝 NaN/Infinity/非 number） */
export function isFiniteZoom(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v);
}
