// PTY 相关类型定义 — 与 Rust pty/spawn.rs + conpty_api.rs 中 DTO 一一对应（camelCase ↔ snake_case）

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

/** ConPTY 后端状态（CP-010：启动 toast 数据源；双边 = Rust conpty_api.rs ConptyStatus，serde camelCase） */
export interface ConptyStatus {
  /** 是否尝试捆绑（仅 Win10 build < 21376） */
  attempted: boolean;
  /** 实际是否走捆绑 conhost */
  bundled: boolean;
  /** 回退原因（attempted && !bundled 时有值，与后端 warn 日志同源） */
  fallbackReason: string | null;
}
