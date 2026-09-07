// DTO 手写残面（CP-024 单源化收容）——本文件为唯一手写类型残面之一,
// 与 hooksConfigGui.ts 并列;其余 9 域文件均由 ts-rs 从 Rust derive 生成,禁手改。
//
// 收容理由(逐项,详见 src/types/CLAUDE.md 残面清单):
// - TitleSource:开放字符串别名(Rust 为 String,不生成独立类型)
// - HooksConfigJson:Record 组合别名——Rust 侧 HooksSubtree 用 serde flatten 承载,
//   ts-rs 10.1 无法为 flatten map 生成顶层 Record 别名,故引用生成物 MatcherGroupJson 组合
// - ContextUsageSignal:前端窄视图(usedPercentage 字段已生成于 AgentEventPayload 内)
// - BACKGROUND_TASK_IDS 常量族:前端专有常量,后端无对应导出

import type { MatcherGroupJson } from "./hooksConfig";

/** 标题来源（开放字符串——claude 值集 customTitle/aiTitle/summary/firstPrompt/none 不变；UI 不消费具体值） */
export type TitleSource = string;

/** settings.json 的 hooks 子树：事件名 → matcher 组数组（组合别名,元素类型引用生成物） */
export type HooksConfigJson = Record<string, MatcherGroupJson[]>;

/** context 用量信号 DTO（对应 Rust AgentEventPayload.usedPercentage——官方口径
 *  used_percentage 0–100 float；百分比取整/钳位由 profile 能力域策略完成，本类型不加工） */
export interface ContextUsageSignal {
  /** 官方 context 用量百分比（claude statusline `context_window.used_percentage`） */
  usedPercentage: number;
}

/** taskId 合法值集（与后端 registry TASKS 键集同步，双侧字面量测试锁死——照
    HooksLayer ↔ Layer 先例，硬约束 #4；新增任务 = 后端 TASKS 一行 + 本数组一项） */
export const BACKGROUND_TASK_IDS = ["planBalance", "sessionRefresh"] as const;
export type BackgroundTaskId = (typeof BACKGROUND_TASK_IDS)[number];

/** planBalance 任务 id 常量（footer/usePlanBalance 消费——通用层禁写字面量，照 CLAUDE_CLI_ID 先例） */
export const PLAN_BALANCE_TASK_ID: BackgroundTaskId = "planBalance";
/** sessionRefresh 任务 id 常量（调度器订阅/applyConfig 消费） */
export const SESSION_REFRESH_TASK_ID: BackgroundTaskId = "sessionRefresh";
