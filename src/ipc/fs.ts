// FS IPC 封装 — 前端调用文件读/写命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke } from "@tauri-apps/api/core";

/** 读取文件内容（UTF-8 文本） */
export async function readFile(path: string): Promise<string> {
  return invoke<string>("fs_read_file", { path });
}

/** 写入文件内容（覆盖模式，UTF-8） */
export async function writeFile(
  path: string,
  content: string,
): Promise<void> {
  await invoke("fs_write_file", { path, content });
}
