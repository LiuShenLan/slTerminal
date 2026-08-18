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
    // FE-26：窗口已销毁时 unlisten Promise reject——兜底吞掉，避免清理期未处理 rejection
    unlisten.then((fn) => fn()).catch(() => {});
  };
}

/**
 * 最小化窗口
 *
 * 供自绘标题栏最小化钮调用（TB-03）。
 */
export async function minimizeWindow(): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.minimize();
}

/**
 * 切换最大化/还原
 *
 * 供自绘标题栏最大化钮与中段双击调用（TB-03/TB-04）。
 */
export async function toggleMaximizeWindow(): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.toggleMaximize();
}

/**
 * 关闭窗口
 *
 * 走 getCurrentWindow().close()——触发 onCloseRequested 事件，复用
 * registerCloseHandler 注册的 P1-19 关窗链路（遍历 TerminalRegistry 杀 PTY
 * + 后端 Job Object KILL_ON_JOB_CLOSE 兜底），保证子进程清理。
 * 禁止用 process.exit 或 destroy 绕过该链路（P1-19 依据）。
 */
export async function closeWindow(): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.close();
}
