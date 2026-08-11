// Agent 事件/用量/注入状态 DTO（决策 3 更名，契约 C1/C6/C12，对应 Rust hooks 模块）
//
// 原 src/types/hooks.ts 的 ContextUsage + src/ipc/hooks.ts 内嵌 DTO 合并于此（MC-212）：
// 事件负载 → AgentEventPayload；注入状态 DTO → AgentHookInjectionStatus；
// 新增 AgentInjectionStatus（状态枚举，对应 Rust AgentInjectionStatus）；ContextUsage 保留名。
// DTO 双边对应（硬约束 #4）：Rust snake_case ↔ JS camelCase，改一边必须改另一边。

/** Context usage DTO（契约 C12，对应 Rust hooks::claude::usage::ContextUsage）——决策 3 保留名 */
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

/** Agent 事件负载 DTO（契约 C1 八字段 + 可选 cliId，对应 Rust AgentEventPayload） */
export interface AgentEventPayload {
  panelId: string;
  event: string;
  timestamp: number;
  sessionId: string;
  /** 用量来源路径（中性名，KZ-2 决策 1——claude 解释为 transcript JSONL；可选——旧信号无此字段，serde default → null，对应后端 Option<String>） */
  usageSourcePath?: string | null;
  cwd: string;
  toolName: string | null;
  notificationType: string | null;
  /** 来源 CLI 标识（MC-205 三级解析显式分支；旧信号无此字段——serde default，缺省按 CLAUDE_CLI_ID 兼容） */
  cliId?: string;
}

/** 注入状态枚举（契约 C6，对应 Rust AgentInjectionStatus） */
export type AgentInjectionStatus = "injected" | "notInjected" | "outdated";

/** 注入状态 DTO（契约 C6，对应 Rust AgentHookInjectionStatus，camelCase 三态契约不变） */
export interface AgentHookInjectionStatus {
  status: AgentInjectionStatus;
  version: number | null;
}
