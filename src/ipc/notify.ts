// 文件系统监听 IPC — 向后端通知启动/切换文件监听 + 前端事件订阅
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** fs-event 后端推送的文件系统事件负载 */
export interface FsEvent {
  paths: string[];
  kind: string;
  detail?: string;
}

/** 启动对指定路径的递归文件监听（后端 300ms 去抖 → fs-event） */
export function startWatch(path: string): Promise<void> {
  return invoke("notify_watch", { path });
}

/**
 * 订阅后端文件系统变更事件
 *
 * 返回取消监听的清理函数。
 * 前端通过此封装订阅，禁止直接 import @tauri-apps/api/event。
 */
export function onFsEvent(callback: (payload: FsEvent) => void): () => void {
  const unlisten = listen<FsEvent>("fs-event", (event) =>
    callback(event.payload),
  );
  return () => {
    unlisten.then((fn) => fn());
  };
}
