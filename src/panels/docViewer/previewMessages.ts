// previewMessages.ts — docViewer 预览消息协议常量单点 + 纯函数守卫
//
// 原 panels/html/zoomMath.ts 迁入（htmlviewer Ctrl+滚轮缩放协议），扩展承接
// docViewer 预览家族（htmlviewer/markdownviewer）的消息协议：
//
// 【ADR-0021 回迁主窗 DOM 后的三层通道】预览渲染于主窗 DOM 内跨源沙箱
// iframe（src = 自定义协议宿主页，sandbox="allow-scripts"），宿主页内再嵌
// 内容 iframe（srcdoc）——全链同窗口树，一律 window.postMessage：
//  1. 文档层（内容 iframe ↔ 主窗，经宿主页桥 relay）：注入脚本（zoomRuntime/
//     scrollRuntime/linkRouter/keyForward 段）运行于内容 iframe——上行
//     slterm_zoom / slterm_scroll / slterm_nav / slterm_keyfwd /
//     slterm_font_probe，下行 slterm_reset / slterm_zoom_set /
//     slterm_scroll_set；opaque origin（e.origin === "null"）+ nonce +
//     类型白名单 + source 归属校验链不变。targetOrigin 一律 "*"（opaque
//     origin 序列化 "null"，无显式 targetOrigin 可用，2026-09-12 spike 实证）。
//  2. 宿主层（宿主页 ↔ 主窗）：上行 slterm_host_ready（桥就绪——主窗据此
//     推送/重推内容兜底）/ slterm_iframe_loaded（内容重建完成）；下行
//     slterm_host_content（装配产物 + 背景色直推）。
//
// 消息载荷与既有 slterm_* 平铺结构同构（{type, nonce, ...}），不引入包装层。
// 上行类型白名单 = 渲染态集合（zoom/scroll/nav）+ 收窄键转发（keyfwd，
// ADR-0021/D2：焦点非表单元素时上行按键描述，主窗经 ShortcutRegistry
// global context 解析消费——旧键转发通道 slterm_key（命令重放语义）随
// CP-013 退役零残留，本通道不复用旧名）+ E2E 专用字体探针（TE-08：宿主页
// FontFaceSet 不覆盖 iframe 文档且 opaque origin 不可读——字体真实加载锚点
// 只能自 iframe 内上行；该段仅 VITE_E2E 构建注入，生产零注入面）——无命令
// 重放通道（终态集合由守卫测试锁死）。下行类型白名单 = 控制集合
//（reset/zoom_set/scroll_set）（CP-044 守卫）。

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

/** iframe → 宿主页：字体加载探针上报（上行，E2E 专用，TE-08）——iframe 内
 *  `fonts.check('12px "KaTeX_Main"')` 结果（loaded 布尔）上行；宿主页
 *  FontFaceSet 不覆盖 iframe 文档（opaque origin 不可读），字体真实加载锚点
 *  只能自 iframe 内取。仅 VITE_E2E 构建注入该段（生产零注入面） */
export const FONT_PROBE_MSG_TYPE = "slterm_font_probe";

/** iframe → 宿主页：keydown 收窄转发（上行，ADR-0021/D2）——回迁主窗 DOM
 *  后预览可聚焦：焦点在非表单元素（input/textarea/select/contenteditable
 *  之外）时上行按键描述（code + 修饰键），主窗合成 KeyboardEvent 经
 *  ShortcutRegistry global context 解析消费（全局快捷键预览聚焦仍可用）；
 *  焦点在表单元素时不转发——表单键入/复制快捷键解禁。注入段不
 *  preventDefault（文档内默认行为保留；global 命令集修饰组合在文档内无
 *  默认行为，不双重触发）。
 *  【威胁面登记】nonce 明文内联于渲染文档——内容脚本可提取伪造本类型上行，
 *  伪造后果 = 触发主窗 global 命令（当前集合仅 global.closeTab，
 *  command-catalog 守卫锁死最小集；global 集扩充时须重估本面）。
 *  旧键转发通道（slterm_key，主窗 dispatchEvent 重放任意按键语义）随
 *  CP-013 退役零残留——本通道为收窄语义新通道，不复用旧名 */
export const KEY_FWD_MSG_TYPE = "slterm_keyfwd";

// ── 宿主层消息（宿主页 ↔ 主窗，ADR-0021）──

/** 宿主页 → 主窗：桥就绪（宿主页脚本执行完）——主窗据此推送/重推内容兜底 */
export const HOST_READY_MSG_TYPE = "slterm_host_ready";

/** 宿主页 → 主窗：内容 iframe 加载完成（= 内容重建完成——主窗据此归 1 +
 *  keepZoom/keepScrollRatio 下行恢复；旧 host-status iframe-loaded 语义随迁） */
export const HOST_IFRAME_LOADED_MSG_TYPE = "slterm_iframe_loaded";

/** 主窗 → 宿主页：内容推送（html = 装配产物完整文档串，bg = iframe 背景色） */
export const HOST_CONTENT_MSG_TYPE = "slterm_host_content";

/** 预览宿主页 URL（自定义协议域——scheme 注册于 src-tauri/src/preview.rs
 *  PREVIEW_SCHEME，host_protocol 仅服务 /preview-host.html 路径；变更须两侧同改） */
export const PREVIEW_HOST_URL = "http://slterm-preview.localhost/preview-host.html";

/** 上行消息类型白名单（文档层终态）：渲染态集合 + 收窄键转发（ADR-0021
 *  D2）+ E2E 专用字体加载探针（仅 VITE_E2E 构建注入可达——生产无该段，
 *  PreviewFrame 侧另经 E2E_ENABLED 门控收束）。宿主层上行（host_ready/
 *  iframe_loaded）为桥生命周期信号，不入本集合 */
export const UPLINK_MSG_TYPES = [
  ZOOM_MSG_TYPE,
  SCROLL_MSG_TYPE,
  NAV_MSG_TYPE,
  FONT_PROBE_MSG_TYPE,
  KEY_FWD_MSG_TYPE,
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

/** 上行 keydown 收窄转发消息（iframe 文档层，ADR-0021/D2）——仅按键描述，
 *  主窗合成 KeyboardEvent 经 ShortcutRegistry global context 解析 */
export interface KeyFwdReport {
  type: typeof KEY_FWD_MSG_TYPE;
  nonce: string;
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
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
