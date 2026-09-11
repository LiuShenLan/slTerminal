// blockCache.ts — 大文件只读浏览读块 LRU 缓存（CP-022）
//
// 契约（与后端 fs_read_file_range 对齐,见 src-tauri/src/fs/mod.rs 命令文档注释）:
// 对同一路径按块序 0,1,2… 递增请求时,后端返回的块文本首尾相接——头回溯含跨块
// 整字符、尾裁到完整字符边界,逐块拼接即原文。本模块只管「块文本」LRU 内存缓存;
// 行结构/字节对齐语义（块文本真实起点 = 前序响应长度累计）在 useLineIndex。
//
// 缓存跨文件路径隔离（多查看器/多文件并行浏览互不污染）;同块并发请求去重（在途
// 单飞,不重复发 IPC）。读失败直接 reject——调用方按块记失败并停止扩展,不缓存错误。

import { fs } from "../../../ipc";

/** 读块大小（字节）——对齐后端 256KB 分块先例（BE-03） */
export const READ_BLOCK_BYTES = 256 * 1024;
/** 缓存块数上限（LRU） */
export const BLOCK_CACHE_LIMIT = 32;

/** 块键分隔符——NUL 不出现在任何文件路径中（含 Windows/Unix）,防路径拼接歧义 */
const KEY_SEP = "\u0000";

/** 块键——路径 + 块号 */
function blockKey(filePath: string, blockIndex: number): string {
  return `${filePath}${KEY_SEP}${blockIndex}`;
}

/** LRU 表：键 → 块文本;Map 迭代序 = 插入序,尾部最新（命中删除重插 = 提升序） */
const cache = new Map<string, string>();
/** 在途读取去重：键 → Promise（同块并发请求只发一次 IPC,返回同一文本） */
const inflight = new Map<string, Promise<string>>();

/**
 * 按需读块（命中 LRU 直接返回;未命中经 ipc/fs.readFileRange 拉取）
 *
 * 读失败时原样抛出（不缓存错误）——useLineIndex 捕获后按块记失败并停止扩展,
 * 防止同块反复重试打满 IPC。
 */
export async function readBlock(filePath: string, blockIndex: number): Promise<string> {
  const key = blockKey(filePath, blockIndex);
  const hit = cache.get(key);
  if (hit !== undefined) {
    // LRU 提升：删除后重插（Map 迭代序头部 = 最久未用,驱逐时先淘汰）
    cache.delete(key);
    cache.set(key, hit);
    return hit;
  }
  const pending = inflight.get(key);
  if (pending) return pending;

  const task = (async () => {
    const gen = fileGen.get(filePath) ?? 0;
    const text = await fs.readFileRange(
      filePath,
      blockIndex * READ_BLOCK_BYTES,
      READ_BLOCK_BYTES,
    );
    // 代际复核（FE-05）：读取途中文件已失效 → 本次结果不回填缓存（防脏写）；
    // 等待期间可能已被并发插入（读后复核,防重复块）
    if ((fileGen.get(filePath) ?? 0) === gen && !cache.has(key)) {
      cache.set(key, text);
      evictIfOverLimit();
    }
    return text;
  })();
  inflight.set(key, task);
  try {
    return await task;
  } finally {
    inflight.delete(key);
  }
}

/**
 * 同步窥视缓存块文本（无副作用、不提升 LRU 序）
 *
 * useLineIndex 组装跨块行文本用——行文本所需块是否已载入须同步可知,
 * 未命中时由调用方异步触发 readBlock 后重渲染。
 */
export function peekCachedBlock(filePath: string, blockIndex: number): string | undefined {
  return cache.get(blockKey(filePath, blockIndex));
}

/** LRU 驱逐: 超出 BLOCK_CACHE_LIMIT 时淘汰最久未用（Map 头部） */
function evictIfOverLimit(): void {
  while (cache.size > BLOCK_CACHE_LIMIT) {
    const oldest = cache.keys().next().value;
    if (oldest === undefined) break;
    cache.delete(oldest);
  }
}

/** 文件代际（失效计数）——invalidateFile 递增；readBlock 在途任务写缓存前比对，
 *  代际已推进（失效发生于读取途中）则旧代际结果不入缓存（防失效后脏回填） */
const fileGen = new Map<string, number>();

/** 失效指定文件的全部缓存块 + 在途条目 + 推进代际（FE-05/FE-07）；在途调用方仍收响应
 * （Promise 对象存活），但失效后新扫描不再复用旧代际在途结果（防陈旧文本进新行索引）；
 * 旧任务 finally 的 inflight.delete 幂等无害 */
export function invalidateFile(filePath: string): void {
  fileGen.set(filePath, (fileGen.get(filePath) ?? 0) + 1);
  const prefix = `${filePath}${KEY_SEP}`;
  for (const k of [...cache.keys()]) {
    if (k.startsWith(prefix)) cache.delete(k);
  }
  for (const k of [...inflight.keys()]) {
    if (k.startsWith(prefix)) inflight.delete(k);
  }
}

/** 测试用: 清空块缓存与在途表（L2 用例隔离;生产零消费） */
export function _resetBlockCache(): void {
  cache.clear();
  inflight.clear();
  fileGen.clear();
}
