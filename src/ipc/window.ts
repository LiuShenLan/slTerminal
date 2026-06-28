// 窗口管理 IPC 封装 — 前端调用 Tauri Window API 的唯一入口
import { getCurrentWindow } from "@tauri-apps/api/window";

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
