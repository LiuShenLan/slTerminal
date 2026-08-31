// types.ts —— 后台定时任务公共类型（F12）

/** 触发来源：manual = 刷新钮/triggerNow；tick = 定时器（仅影响失败处理策略，规格 §7） */
export type TriggerSource = "manual" | "tick";

/** 任务运行状态机（视图语义不变：idle 初始 / loading 执行中 / ready 成功 / error 手动失败） */
export type TaskRunState = "idle" | "loading" | "ready" | "error";

/** 任务快照（调度器持有并分发）：state 真值源在调度器；data 形状 per-task 约定 */
export interface TaskSnapshot<T = unknown> {
  state: TaskRunState;
  data: T | undefined;
}

/** 任务定义（注册条目）：run 为唯一执行体——手动刷新与定时刷新同为它的触发器。
    prev = 上一次成功数据（部分失败隔离用；首轮 undefined） */
export interface BackgroundTaskDef<T = unknown> {
  id: string;
  run(source: TriggerSource, prev: T | undefined): Promise<T>;
}
