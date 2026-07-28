/** Context usage DTO（契约 C12，对应 Rust hooks::usage::ContextUsage） */
export interface ContextUsage {
  /** 输入 token 数 */
  inputTokens: number;
  /** 输出 token 数 */
  outputTokens: number;
  /** 缓存读取输入 token 数（serde default 兼容旧 transcript 缺失，缺省 0） */
  cacheReadInputTokens: number;
  /** 缓存创建输入 token 数（serde default 兼容旧 transcript 缺失，缺省 0） */
  cacheCreationInputTokens: number;
}
