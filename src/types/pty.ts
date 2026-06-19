// PTY 相关类型定义 — 与 Rust pty/spawn.rs 中 DTO 一一对应（camelCase ↔ snake_case）

/** PTY 输出事件 */
export type PtyEvent =
  | { type: "Output"; data: { bytes: number[] } }
  | { type: "Exit"; data: { code: number | null } };

/** PTY spawn 请求参数 */
export interface SpawnRequest {
  panelId: string;
  cols: number;
  rows: number;
  cwd?: string;
  shell?: string;
}
