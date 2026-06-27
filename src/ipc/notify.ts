// 文件系统监听 IPC — 向后端通知启动/切换文件监听
import { invoke } from "@tauri-apps/api/core";

/** 启动对指定路径的递归文件监听（后端 300ms 去抖 → fs-event） */
export function startWatch(path: string): Promise<void> {
  return invoke("fs_watch", { path });
}
