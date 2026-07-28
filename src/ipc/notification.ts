// 通知 IPC 封装 — 桌面 toast 发送与权限管理
//
// 策略：
//   - 权限检查/请求：委托 @tauri-apps/plugin-notification（Tauri 原生 API，无需额外配置）
//   - toast：Tauri 原生 sendNotification（Web Notification API 在未打包 Win32 WebView2 下：
//     无 AUMID → banner 抑制 + onclick 不路由 + shim 无 close + 构造不抛→catch 回退永不触发，
//     探针实测 {"created":true,"permission":"granted","thrown":"TypeError: n.close is not a function"}）
//
// 架构硬约束 #1：invoke 调用只出现在本目录；外部通过本模块消费通知能力

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

// re-export 官方插件函数，供外部直接消费
export { isPermissionGranted, requestPermission, sendNotification };

/** 确保通知权限已授予。返回 true 表示可发送通知 */
export async function ensureNotificationPermission(): Promise<boolean> {
  if (await isPermissionGranted()) {
    return true;
  }
  const result = await requestPermission();
  return result === "granted";
}

/**
 * 发送桌面 toast 通知（Tauri 原生通道，无点击路由能力）
 *
 * 使用 Tauri sendNotification 发送系统通知。未打包 Win32 WebView2 无 AUMID——
 * banner 可能被抑制，仅通知中心条目 + 任务栏闪烁作为回窗引导。
 *
 * @param title   通知标题
 * @param options 通知选项（body: 通知正文）
 */
export function sendToastNotification(
  title: string,
  options: { body: string },
): void {
  try {
    sendNotification({ title, body: options.body });
  } catch (err) {
    console.error("sendToastNotification 失败:", err);
  }
}
