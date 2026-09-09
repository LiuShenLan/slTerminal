// FS IPC 封装 — 前端调用文件读/写/目录命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke, Channel } from "@tauri-apps/api/core";
import type { FsMetadata, FsReadDirPage } from "../types/fs";

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

/**
 * 读取资源文件（任意二进制）并返回 base64 串
 *
 * docViewer 预览的本地相对资源（图片等）通道：后端按 256KB 原字节块 base64
 * 编码经 onChunk Channel 推送，此处累积拼接为完整 base64 串后 resolve
 * （UTF-8 安全，不做文本解码）；沙箱外/超 10MB 等后端校验失败直接 reject。
 * MIME 推断由调用方按扩展名白名单完成——本 wrapper 只取字节。
 */
export async function readResourceBase64(path: string): Promise<string> {
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
  await invoke("fs_read_resource", { path, onChunk });
  return content;
}

/**
 * 分块读文件指定字节区间（CP-022 大文件只读浏览）——返回区间内完整 UTF-8 文本
 *
 * 不经 10MB 全量上限（后端按 [offsetBytes, offsetBytes+lengthBytes) 区间读,钳制到
 * EOF;返回文本头尾对齐字符边界——头回溯含跨区间整字符、尾裁到完整字符边界）。
 * 消费方（LargeFileViewer）按 256KB 块序递增请求,响应拼接即原文（契约见后端实现注释）。
 */
export function readFileRange(
  filePath: string,
  offsetBytes: number,
  lengthBytes: number,
): Promise<string> {
  return invoke("fs_read_file_range", { filePath, offsetBytes, lengthBytes });
}

/** 文件元数据（fs_stat）——真实字节数 + 修改时间；沙箱校验同 readFile */
export function statFile(path: string): Promise<FsMetadata> {
  return invoke<FsMetadata>("fs_stat", { path });
}

/** 写入文件内容（覆盖模式，UTF-8） */
export async function writeFile(
  path: string,
  content: string,
): Promise<void> {
  await invoke("fs_write_file", { path, content });
}

/**
 * 分页读取目录内容（CP-006 游标契约，fs_read_dir）
 *
 * - path：目录路径（沙箱校验）；
 * - cursor：上一页返回的 nextCursor（首帧省略）——opaque，只回传不解读；
 * - limit：单页上限 [1, 1000]，缺省 500（越界由后端钳制）。
 *
 * 返回本页条目 + 下一页游标（null = 末页）。后端过滤（.git）与排序（文件夹→文件、
 * 小写名称序）在整表完成后切片——跨页整体序稳定，消费方按页顺序拼接即全量。
 */
export async function readDirPage(
  path: string,
  cursor?: string,
  limit?: number,
): Promise<FsReadDirPage> {
  // 只发送提供的键：cursor/limit 未提供时不入 payload（undefined 不入 payload
  // 是本仓约定，ipc-contract 测试对键集合做精确断言，缺省调用仅含 path）
  const payload: Record<string, unknown> = { path };
  if (cursor !== undefined) payload.cursor = cursor;
  if (limit !== undefined) payload.limit = limit;
  return invoke<FsReadDirPage>("fs_read_dir", payload);
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
