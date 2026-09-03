// 套餐余量 DTO（F10）——与 src-tauri/src/plan_balance/mod.rs 双边对应（硬约束 #4）
// Rust snake_case ↔ TS camelCase 由 Tauri 自动转换

/** 单来源套餐余量（updatedAt=0 表示尚无成功值 → 行显 --） */
export interface PlanBalanceInfo {
  sourceId: string; // v1 恒 "claude"
  planId: string; // "deepseek" | "kimi"
  frozen: boolean; // kimi 月限额触顶
  amount: AmountInfo | null;
  windows: WindowsInfo | null;
  updatedAt: number;
}

export interface AmountInfo {
  value: string; // 原样透传 total_balance
  currency: string;
}

export interface WindowsInfo {
  fiveHour: WindowInfo;
  sevenDay: WindowInfo;
}

export interface WindowInfo {
  usedPercent: number; // 已用百分比 = used/limit×100（used 缺失经 remaining 换算回退）
  resetsAt: string | null; // ISO 字符串，可缺失
}
