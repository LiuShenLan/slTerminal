// backgroundTasks barrel —— 公共 API 出口（不触发任务注册；注册触发点在 ./tasks.ts）
export { backgroundTaskScheduler } from "./scheduler";
export type { BackgroundTaskDef, TaskRunState, TaskSnapshot, TriggerSource } from "./types";
