// Agent 事件/用量/注入状态 DTO（决策 3 更名，契约 C1/C6，对应 Rust hooks 模块）
//
// 原 src/types/hooks.ts 的 ContextUsage + src/ipc/hooks.ts 内嵌 DTO 合并于此（MC-212）：
// 事件负载 → AgentEventPayload；注入状态 DTO → AgentHookInjectionStatus；
// 新增 AgentInjectionStatus（状态枚举，对应 Rust AgentInjectionStatus）。
// transcript token 链路（ContextUsage + agent_context_usage）已整体移除——百分比改用
// claude 官方 statusline used_percentage 桥接（ContextUsageSignal，MC-214 口径变更）。
// DTO 双边对应（硬约束 #4）：Rust snake_case ↔ JS camelCase，改一边必须改另一边。

/** context 用量信号 DTO（对应 Rust AgentEventPayload.usedPercentage——官方口径
 *  used_percentage 0–100 float；百分比取整/钳位由 profile 能力域策略完成，本类型不加工） */
export interface ContextUsageSignal {
  /** 官方 context 用量百分比（claude statusline `context_window.used_percentage`） */
  usedPercentage: number;
}

/** Agent 事件负载 DTO（契约 C1 十字段 + 可选 cliId，对应 Rust AgentEventPayload） */
export interface AgentEventPayload {
  panelId: string;
  event: string;
  /** 事件时间戳（毫秒；Rust u64 → JS number，安全整数范围（< 2^53）约定） */
  timestamp: number;
  sessionId: string;
  /** 用量来源路径（中性名，KZ-2 决策 1——claude 解释为 transcript JSONL；可选——旧信号无此字段，serde default → null，对应后端 Option<String>） */
  usageSourcePath?: string | null;
  cwd: string;
  toolName: string | null;
  notificationType: string | null;
  /** 来源 CLI 标识（MC-205 三级解析显式分支；旧信号无此字段——serde default，缺省按 CLAUDE_CLI_ID 兼容） */
  cliId?: string;
  /** context 用量百分比（可选——ContextUsage 信号字段，旧信号缺省 undefined；
   *  数值语义由 CLI 解释，claude = 官方 used_percentage） */
  usedPercentage?: number | null;
}

/** 注入状态枚举（契约 C6，对应 Rust AgentInjectionStatus） */
export type AgentInjectionStatus = "injected" | "notInjected" | "outdated";

/** 注入状态 DTO（契约 C6，对应 Rust AgentHookInjectionStatus，camelCase 三态契约不变） */
export interface AgentHookInjectionStatus {
  status: AgentInjectionStatus;
  version: number | null;
}
