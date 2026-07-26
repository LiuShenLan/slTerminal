// Hooks IPC — 注入/卸载/状态查询 + hook-event 事件订阅
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Hook 注入状态 DTO（契约 C6） */
export interface HookInjectionStatus {
  status: "injected" | "notInjected" | "outdated";
  version: number | null;
}

/** Hook 事件负载 DTO（契约 C1 八字段 + 契约 C6） */
export interface HookEventPayload {
  panelId: string;
  event: string;
  timestamp: number;
  sessionId: string;
  transcriptPath: string;
  cwd: string;
  toolName: string | null;
  notificationType: string | null;
}

/** 注入 hook 脚本到 claude settings.json，返回注入后状态 */
export async function inject(): Promise<HookInjectionStatus> {
  return invoke("hooks_inject");
}

/** 卸载 hook：移除配置段 + 删脚本目录 + 清信号目录 */
export async function uninstall(): Promise<void> {
  return invoke("hooks_uninstall");
}

/** 查询当前注入状态（面板/入口显示用） */
export async function getInjectionStatus(): Promise<HookInjectionStatus> {
  return invoke("hooks_injection_status");
}

/**
 * 订阅后端 hook-event 事件（照 onFsEvent 模式）
 *
 * 返回取消监听的清理函数。
 */
export function onHookEvent(
  callback: (payload: HookEventPayload) => void,
): () => void {
  const unlisten = listen<HookEventPayload>("hook-event", (event) =>
    callback(event.payload),
  );
  return () => {
    unlisten.then((fn) => fn());
  };
}
