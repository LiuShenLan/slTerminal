// tasks.ts —— 后台任务注册触发点（硬约束 #13 side-effect import）：
// 消费方（useAgentHistory / BackgroundTasksPage）import 本文件即完成全部任务注册，
// 禁止隐式初始化；新增任务 = 下方追加一条 import。
import "./sessionRefreshTask";
