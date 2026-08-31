// 后台定时任务 IPC（F12）——清单读取 / 配置写通道 / 配置变更事件订阅
// 本文件是 background_tasks_list / background_tasks_set_config 的唯一 invoke 位置（硬约束 #1）
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { BackgroundTaskInfo } from "../types/backgroundTasks";

/** 全部任务元数据 + 当前生效配置（后端内存值） */
export function listBackgroundTasks(): Promise<BackgroundTaskInfo[]> {
  return invoke("background_tasks_list");
}

/**
 * 设置任务配置（写通道：后端校验 → 落盘 → 内存 → emit 变更事件 → 返回完整清单）。
 * enabled/intervalSec 至少提供其一（均缺省 → 后端 Validation）；只发送提供的键
 * （undefined 不入 payload，契约键集合精确断言依赖此行为）。
 */
export function setBackgroundTaskConfig(
  taskId: string,
  config: { enabled?: boolean; intervalSec?: number },
): Promise<BackgroundTaskInfo[]> {
  const args: Record<string, unknown> = { taskId };
  if (config.enabled !== undefined) args.enabled = config.enabled;
  if (config.intervalSec !== undefined) args.intervalSec = config.intervalSec;
  return invoke("background_tasks_set_config", args);
}

/** 订阅配置变更（set_config 成功后后端推送完整清单）；返回 unsubscribe */
export function onBackgroundTasksUpdated(
  callback: (payload: BackgroundTaskInfo[]) => void,
): () => void {
  const unlisten = listen<BackgroundTaskInfo[]>(
    "background-tasks-updated",
    (event) => callback(event.payload),
  );
  return () => {
    unlisten.then((fn) => fn());
  };
}
