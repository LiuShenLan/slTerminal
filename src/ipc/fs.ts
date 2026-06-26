// FS IPC 封装 — 前端调用文件读/写/目录命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke } from "@tauri-apps/api/core";
import type { DirEntry } from "../types/fs";

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

/** 读取目录内容 */
export async function readDir(path: string): Promise<DirEntry[]> {
  return invoke<DirEntry[]>("fs_read_dir", { path });
}

/** 创建目录（递归创建父目录） */
export async function createDir(path: string): Promise<void> {
  await invoke("fs_create_dir", { path });
}

/** 删除文件或目录（永久删除，不进回收站） */
export async function deleteEntry(path: string): Promise<void> {
  await invoke("fs_delete", { path });
}

/** 重命名/移动文件或目录 */
export async function rename(src: string, dst: string): Promise<void> {
  await invoke("fs_rename", { src, dst });
}
