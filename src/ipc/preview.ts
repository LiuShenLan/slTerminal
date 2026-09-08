// 预览渲染独立 webview IPC（S10-② 迁移，ADR-0019）
//
// docViewer 面板（htmlviewer/markdownviewer）预览内容渲染于独立 Tauri
// WebviewWindow（label = preview-<panelId>，spike 驱动契约），本层承载主窗侧
// 对该窗口的全部系统调用：
//   - previewSync：创建/同步窗口几何显隐（CSS 视口坐标 → 后端换算物理屏幕坐标；
//     轮询与窗口移动监听驱动——面板隐藏/显示即窗口 hide/show，保活语义
//     = 隐藏不销毁，CP-037 复核结论）；
//   - previewRender：推送渲染内容（后端存储 + 定向通知宿主页拉取）；
//   - previewClose：销毁窗口并清内容（面板卸载）；
//   - 消息桥事件订阅（上行 zoom/scroll/nav 转发 + 宿主状态；下行 emit 广播 +
//     label 过滤——常量与载荷类型见 previewMessages.ts 单点登记）。
//
// 宿主页（自定义协议域内）只消费 preview_pull 与 event API，不经本层——其页内
// 无打包模块，raw __TAURI_INTERNALS__ 直连（capabilities/preview.json 最小权限）。

import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";

// ── 跨窗口消息桥事件名单点（S10-② 通道退役分支，ADR-0019）──
// 宿主页桥（Rust 侧内联页/自定义协议域）与 Rust 侧事件名与此处一致——
// 变更须三处同步（本文件 / src-tauri/src/preview.rs 宿主页桥脚本 / 测试守卫）。

/** 预览宿主页 → 主窗：iframe 文档上行（zoom/scroll/nav）转发事件
 *  载荷 = { label, type, nonce, zoom?/ratio?/href? }——主窗按 label 过滤后
 *  走与旧 iframe 通道同款校验（type 白名单 + nonce + 数值守卫） */
export const PREVIEW_UPLINK_EVENT = "preview:uplink";

/** 主窗 → 预览宿主页：下行控制事件（reset/zoom_set/scroll_set）
 *  载荷 = { label, type, nonce, zoom?/ratio? }——宿主页按自身 label 过滤后
 *  原样 postMessage 注入 iframe（iframe 侧 source===parent + nonce 校验不变） */
export const PREVIEW_DOWNLINK_EVENT = "preview:downlink";

/** 预览宿主页 → 主窗：宿主页状态事件（非文档消息）
 *  载荷 = { label, status }；status = iframe-loaded（iframe 每次加载完成——
 *  主窗据此执行「重建归 1 + keepZoom/keepScrollRatio 下行恢复」） */
export const PREVIEW_HOST_STATUS_EVENT = "preview:host-status";

/** 宿主页状态值：iframe 文档加载完成 */
export const PREVIEW_STATUS_IFRAME_LOADED = "iframe-loaded";

/** 预览窗口 label 前缀（与 src-tauri/src/preview.rs PREVIEW_LABEL_PREFIX 对应） */
export const PREVIEW_LABEL_PREFIX = "preview-";

/** 面板 panelId → 预览窗口 label（label 即 WDIO 驱动句柄，e2e 契约） */
export function makePreviewLabel(panelId: string): string {
  return `${PREVIEW_LABEL_PREFIX}${panelId}`;
}

/** 创建/更新预览窗口几何与显隐（幂等——窗口不存在则创建；x/y/w/h 为面板内容
 *  区在 CSS 视口坐标；后端换算物理屏幕坐标并驱动位置/尺寸/显隐）。
 *  token = 挂载期随机串（PreviewFrame 每轮生命周期 effect 生成）——后端会话
 *  守卫凭它拒绝「close 后同 token 迟到 sync」的重建（销毁后复活僵尸根因，
 *  2026-09-08 归因）；sync/close 必须同传同值 */
export function previewSync(
  label: string,
  x: number,
  y: number,
  width: number,
  height: number,
  visible: boolean,
  token: string,
): Promise<void> {
  return invoke("preview_sync", { label, x, y, width, height, visible, token });
}

/** 销毁预览窗口并清空其渲染内容（面板卸载/预览形态退出时）；token 同上——
 *  与最近一次 previewSync 同传（异 token 的迟到 close 后端拒绝受理） */
export function previewClose(label: string, token: string): Promise<void> {
  return invoke("preview_close", { label, token });
}

/** 推送渲染内容（最终注入产物文档——injectScript 装配后的完整 srcdoc 串）；
 *  后端存储后定向通知宿主页拉取（宿主未就绪时由宿主加载即拉兜底） */
export function previewRender(
  label: string,
  html: string,
  bg?: string,
): Promise<void> {
  return invoke("preview_render", { label, html, bg });
}

/** 主窗 → 预览：下行控制消息（reset/zoom_set/scroll_set——终态控制集合，
 *  previewMessages 白名单守卫锁死）；广播 + 宿主页按 label 过滤 */
export function emitPreviewDownlink(payload: {
  label: string;
  type: string;
  nonce: string;
  zoom?: number;
  ratio?: number;
}): Promise<void> {
  return emit(PREVIEW_DOWNLINK_EVENT, payload);
}

/** 预览宿主页 → 主窗：iframe 文档上行消息（zoom/scroll/nav 转发事件）——
 *  订阅后按 label 过滤并校验（type 白名单 + nonce + 数值守卫在 PreviewFrame） */
export function onPreviewUplink(
  cb: (msg: {
    label: string;
    type: string;
    nonce?: unknown;
    zoom?: unknown;
    ratio?: unknown;
    href?: unknown;
  }) => void,
): () => void {
  const unlisten = listen<{
    label: string;
    type: string;
    nonce?: unknown;
    zoom?: unknown;
    ratio?: unknown;
    href?: unknown;
  }>(PREVIEW_UPLINK_EVENT, (event) => cb(event.payload));
  return () => {
    unlisten.then((fn) => fn()).catch(() => {
      /* 卸载期监听已失效——静默 */
    });
  };
}

/** 预览宿主页 → 主窗：宿主状态事件（iframe 加载完成等） */
export function onPreviewHostStatus(
  cb: (msg: { label: string; status: string }) => void,
): () => void {
  const unlisten = listen<{ label: string; status: string }>(
    PREVIEW_HOST_STATUS_EVENT,
    (event) => cb(event.payload),
  );
  return () => {
    unlisten.then((fn) => fn()).catch(() => {
      /* 卸载期监听已失效——静默 */
    });
  };
}
