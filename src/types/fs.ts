// 文件系统相关类型定义 — 与 Rust fs/mod.rs 命令参数一一对应

/** 目录条目 */
export interface DirEntry {
  /** 文件/目录名 */
  name: string;
  /** 完整路径 */
  path: string;
  /** 是否为目录 */
  isDir: boolean;
  /** 文件大小（字节），仅文件时有值 */
  size?: number;
  /** 最后修改时间（Unix 毫秒），仅文件时有值 */
  modified?: number;
}
