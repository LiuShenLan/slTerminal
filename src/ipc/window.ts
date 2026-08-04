// 窗口管理 IPC 封装 — 前端调用 Tauri Window API 的唯一入口
import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";

/** 重新导出 UserAttentionType——供通知调度使用 */
export { UserAttentionType };

/**
 * 注册窗口焦点变化监听器
 *
 * 回调参数 `focused: boolean`——窗口获得焦点为 true，失去为 false。
 * 返回取消监听的清理函数。
 */
export function onFocusChanged(
  cb: (focused: boolean) => void,
): () => void {
  const appWindow = getCurrentWindow();
  const unlisten = appWindow.onFocusChanged(({ payload: focused }) => {
    cb(focused);
  });
  return () => {
    unlisten.then((fn) => fn());
  };
}

/**
 * 请求用户关注（任务栏闪烁）
 *
 * Windows 上 UserAttentionType.Critical = FLASHW_TIMERNOFG——持续闪烁直到窗口获得焦点。
 * 传入 null 停止闪烁。
 */
export async function requestUserAttention(
  type: typeof UserAttentionType.Critical | null,
): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.requestUserAttention(type);
}

/**
 * 聚焦窗口
 *
 * 预留：当前无消费方（通知回窗引导仅用 requestUserAttention 任务栏闪烁，点击路由已放弃）。
 * 未来通知点击回窗等场景启用；保留完整实现与最小契约测试（ipc-window-contract.test.ts）。
 */
export async function setFocus(): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.setFocus();
}

/**
 * 注册窗口关闭处理器
 *
 * 自动调用 event.preventDefault() 阻止立即关闭，
 * 执行回调（持久化保存）后在 finally 中销毁窗口。
 * 返回取消监听的清理函数。
 */
export function registerCloseHandler(cb: () => Promise<void>): () => void {
  const appWindow = getCurrentWindow();
  const unlisten = appWindow.onCloseRequested(async (event) => {
    event.preventDefault();
    try {
      await cb();
    } finally {
      await appWindow.destroy();
    }
  });
  return () => {
    unlisten.then((fn) => fn());
  };
}
