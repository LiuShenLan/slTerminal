// 后台定时任务 DTO（F12）——与 src-tauri/src/background_tasks/mod.rs 双边对应（硬约束 #4）
// Rust snake_case ↔ TS camelCase 由 Tauri 自动转换

/** 单个后台任务（元数据 + 当前生效配置；六键契约，无 default 字段——默认值单点在后端注册表） */
export interface BackgroundTaskInfo {
  taskId: string;
  title: string;
  enabled: boolean;
  intervalSec: number;
  intervalMin: number;
  intervalMax: number;
}

/** taskId 合法值集（与后端 registry TASKS 键集同步，双侧字面量测试锁死——照
    HooksLayer ↔ Layer 先例，硬约束 #4；新增任务 = 后端 TASKS 一行 + 本数组一项） */
export const BACKGROUND_TASK_IDS = ["planBalance", "sessionRefresh"] as const;
export type BackgroundTaskId = (typeof BACKGROUND_TASK_IDS)[number];

/** planBalance 任务 id 常量（footer/usePlanBalance 消费——通用层禁写字面量，照 CLAUDE_CLI_ID 先例） */
export const PLAN_BALANCE_TASK_ID: BackgroundTaskId = "planBalance";
/** sessionRefresh 任务 id 常量（调度器订阅/applyConfig 消费） */
export const SESSION_REFRESH_TASK_ID: BackgroundTaskId = "sessionRefresh";
