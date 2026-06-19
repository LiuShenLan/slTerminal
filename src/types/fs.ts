// FS 相关类型定义 — 与 Rust fs/mod.rs 中命令参数一一对应

/** 文件读取结果 */
export type ReadResult = string;

/** 文件写入请求 */
export interface WriteRequest {
  path: string;
  content: string;
}
