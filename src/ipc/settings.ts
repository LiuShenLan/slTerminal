// Settings IPC 封装 — 前端调用 Settings 命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke } from "@tauri-apps/api/core";

/**
 * 加载持久化设置（FE-11/D11 契约）：
 * - data: null / corrupted: false → 无文件（首次启动）
 * - data: 默认值 / corrupted: true → 解析失败回退默认（含 .bak 命中）
 */
export async function loadSettings(): Promise<{
  data: Record<string, unknown> | null;
  corrupted: boolean;
}> {
  return invoke<{ data: Record<string, unknown> | null; corrupted: boolean }>(
    "load_settings",
  );
}

/** 保存设置到磁盘 */
export async function saveSettings(
  settings: Record<string, unknown>,
): Promise<void> {
  await invoke("save_settings", { settings });
}
