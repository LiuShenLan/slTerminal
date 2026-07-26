/** Context usage DTO（契约 C5，对应 Rust hooks::usage::ContextUsage） */
export interface ContextUsage {
  /** 输入 token 数 */
  inputTokens: number;
  /** 输出 token 数 */
  outputTokens: number;
}
