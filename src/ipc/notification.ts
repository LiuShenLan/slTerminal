// 通知 IPC 封装 — 桌面 toast 发送与权限管理
//
// 策略：
//   - 权限检查/请求：委托 @tauri-apps/plugin-notification（Tauri 原生 API，无需额外配置）
//   - 可点击 toast：使用 Web Notification API（new Notification()），原生支持 onclick 回调
//   - 不可点击通知：回退 Tauri sendNotification（静默通知，不阻塞主线程）
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
 * 发送可点击的桌面 toast 通知
 *
 * 使用 Web Notification API 构造函数创建通知，原生支持 onclick 回调。
 * 与 Tauri 插件 sendNotification 不同——后者不支持前端 onclick 处理。
 * WebView2 环境下 Notification API 委托 OS 原生通知中心。
 *
 * @param title   通知标题
 * @param options 通知选项（body: 通知正文）
 * @param onClick 用户点击通知时的回调（聚焦窗口 + 路由到面板）
 * @returns Notification 实例（成功路径）；null（catch 回退 Tauri sendNotification 路径）
 */
export function sendClickableNotification(
  title: string,
  options: { body: string },
  onClick: () => void,
): Notification | null {
  try {
    // Web Notification API — WebView2 原生支持，委托 OS 通知中心
    const notification = new Notification(title, {
      body: options.body,
      // 不设 icon — 使用 OS 默认应用图标
    });
    notification.onclick = () => {
      onClick();
      notification.close();
    };
    return notification;
  } catch {
    // Web Notification API 不可用时（权限未授予等），回退 Tauri 原生通知
    // 注意：此路径无点击回调能力
    sendNotification({ title, body: options.body });
    return null;
  }
}
