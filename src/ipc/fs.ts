// FS IPC 封装 — 前端调用文件读/写/目录命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke, Channel } from "@tauri-apps/api/core";
import type { DirEntry } from "../types/fs";

/** fs_read_file 分块推送的单块载荷（BE-03）——后端按 256KB 块经 onChunk Channel 推送 */
export interface FsReadChunk {
  /** 本块文本（UTF-8，后端保证落在字符边界） */
  data: string;
  /** 是否终态块：true 时 data 恒为空串，后续无更多块 */
  done: boolean;
}

/**
 * 读取文件内容（UTF-8 文本）
 *
 * 后端经 onChunk Channel 分块推送（256KB/块，终态 { data:"", done:true }），
 * 此处累积拼接为完整字符串后 resolve；invoke 失败（沙箱外/超 10MB 等）直接 reject。
 * 签名保持 Promise<string> 不变，消费方零适配。
 */
export async function readFile(path: string): Promise<string> {
  const onChunk = new Channel<FsReadChunk>();
  const chunks: string[] = [];
  const content = new Promise<string>((resolve) => {
    onChunk.onmessage = (chunk) => {
      if (chunk.done) {
        // 终态：拼接全部数据块后 resolve（终态块 data 恒为空串，不入累积）
        resolve(chunks.join(""));
      } else {
        chunks.push(chunk.data);
      }
    };
  });

  // 先 await invoke：后端校验失败时异常直接传播给调用方
  await invoke("fs_read_file", { path, onChunk });
  return content;
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

/** 设置项目根路径（路径沙箱边界，后端据此校验文件操作） */
export async function setProjectRoot(path: string): Promise<void> {
  await invoke("set_project_root", { path });
}
