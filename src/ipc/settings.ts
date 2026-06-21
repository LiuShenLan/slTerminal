// Settings IPC 封装 — 前端调用 Settings 命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke } from "@tauri-apps/api/core";

/** 加载持久化设置（返回 null 表示首次启动） */
export async function loadSettings(): Promise<Record<string, unknown> | null> {
  return invoke<Record<string, unknown> | null>("load_settings");
}

/** 保存设置到磁盘 */
export async function saveSettings(
  settings: Record<string, unknown>,
): Promise<void> {
  await invoke("save_settings", { settings });
}
