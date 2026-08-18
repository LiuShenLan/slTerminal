// PTY 相关类型定义 — 与 Rust pty/spawn.rs 中 DTO 一一对应（camelCase ↔ snake_case）

/** PTY 输出事件 */
export type PtyEvent =
  | { type: "output"; data: { bytes: number[] } }
  | { type: "exit"; data: { code: number | null } };

/** PTY spawn 请求参数 */
export interface SpawnRequest {
  panelId: string;
  /** 列数（Rust u16 → JS number；契约范围 1..=32767，ipc/pty.ts spawn wrapper 前置校验；
   *  u16 恒在 JS 安全整数（< 2^53）范围内——Rust 数值 → JS number 精度约定） */
  cols: number;
  /** 行数（Rust u16 → JS number；契约范围 1..=32767，ipc/pty.ts spawn wrapper 前置校验；
   *  u16 恒在 JS 安全整数（< 2^53）范围内——Rust 数值 → JS number 精度约定） */
  rows: number;
  cwd?: string;
  shell?: string;
}
