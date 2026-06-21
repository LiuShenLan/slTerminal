// Git 相关类型定义 — 与 Rust git/worktree.rs 中 DTO 一一对应（camelCase ↔ snake_case）

/** 工作树信息 */
export interface WorktreeInfo {
  path: string;
  branch: string;
  head: string;
  isBare: boolean;
  isDetached: boolean;
  isMain: boolean;
}

/** 工作树绑定（页面 ↔ 工作树） */
export interface WorktreeBinding {
  worktreePath: string;
  branchName: string;
}
