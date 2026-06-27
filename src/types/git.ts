// Git 相关类型定义 — 与 Rust git/mod.rs 命令参数一一对应

/** 文件 git 状态条目 */
export interface GitStatusEntry {
  /** 文件绝对路径（与 fs_read_dir 的 DirEntry.path 格式一致） */
  path: string;
  /** git 状态：modified | added | deleted | renamed | untracked | conflict | ignored */
  status: string;
}

/** diff hunk 信息（old = HEAD, new = 工作区） */
export interface DiffHunk {
  /** HEAD 侧起始行号（1-based） */
  oldStart: number;
  /** HEAD 侧行数 */
  oldLines: number;
  /** 工作区侧起始行号（1-based） */
  newStart: number;
  /** 工作区侧行数 */
  newLines: number;
}
