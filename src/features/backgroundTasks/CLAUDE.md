# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

后台定时任务双端抽象（F12，ADR-0013）的**前端半**：`BackgroundTaskScheduler` 模块级单例调度器 + 任务执行体注册表。后端半（`src-tauri/src/background_tasks`）承载任务元数据注册表单点、poller 骨架与配置单写通道。前端调度器解决的是「任务执行体天然双栖」的另一半：`sessionRefresh`（历史会话扫描）必须编排在前端订阅生命周期内（无订阅者不空转扫盘），无法由后端 poller 驱动。不这样设计则需在后端复制一份订阅者计数生命周期或让前端轮询空转，两者都违背规格 §8 与「无订阅者不空转」。

## 关键约束与决策

### 注册表家族契约（硬约束 #13）

- 模块级单例 `backgroundTaskScheduler`（scheduler.ts 底部实例化导出）；`register(...)` / `getAll()`（按注册序）/ `_reset()`（仅测试用，停全部 timer 并清空）。
- **注册经 side-effect import 触发**：`tasks.ts` 是注册触发点集中文件（import `./sessionRefreshTask` 即完成全部任务注册），消费方 `useAgentHistory.ts` 与 `BackgroundTasksPage.tsx` 顶部 `import "../backgroundTasks/tasks"` 触发。禁止隐式初始化；新增任务 = `sessionRefreshTask.ts` 平行新增文件 + `tasks.ts` 追加一行 import。
- 注册触发点登记：`useAgentHistory.ts`、`BackgroundTasksPage.tsx`（两处 import `./tasks`）。
- `sessionRefreshTask.ts` 额外显式导出 `runSessionRefresh` 供测试在 `_reset()` 后重注册（注册入口仍收敛于 `tasks.ts`）。

### 订阅生命周期（规格 FR-1）

- **首个订阅者出现** → 读配置（`background_tasks_list`）→ enabled 则立即执行一轮（接管「挂载即扫」语义）+ 启动 interval。
- **最后订阅者退订** → 停 interval（在途轮继续完成）；无订阅者不空转。
- 配置读取失败：保住首轮执行（数据可见），不启动 interval（无元数据第二来源）。
- 重订阅（退订后再订阅）不重复读配置（`configReady` 已置位，直接启动）。

### 防重入闸门

`running` 标志为 tick 与 manual 共用闸门：上一轮未结束时，tick 与 triggerNow 均直接跳过（run 调用次数不增）。

### 失败策略（规格 §7，按触发来源区分）

- **tick 失败静默**：快照不变（state/data 均保留），仅 console.error。
- **manual 失败置 error 态**：`state: "error"`，旧 data 保留。
- 执行体自身语义：单 provider 失败 → 该 provider 保留旧数据（按 cliId 过滤 prev）、其余采用新值；全部失败 → throw（调度器按触发来源兜底）。

### applyConfig 运行期改配

设置页 `set_config` 成功后直调：启停/改频率**立即生效**；禁用 → 停 timer；启用 → 立即一轮 + 重启 timer；**无订阅者时 applyConfig 只记配置不启动**（订阅时生效）。`applyLocal` 为本地变更透传（removeLocal 语义：删除会话后本地移除列表项不重扫），只更新 data 不改变 state。

### 扫描执行体 force 恒 true

`runSessionRefresh` 遍历 `cliProfileRegistry` 中声明 history 能力的 profile 逐个 `scanAgentHistory(cliId, true)` 聚合为扁平列表。**恒 `force=true`**：后端 `(目录 mtime, 文件数)` 缓存对进行中会话不敏感（规格 §8）——目录内会话文件增删不影响根键，手动与定时必须同一口径绕过缓存，否则空结果永久命中场景无法恢复。

### 与后端任务的分工

- 前端只注册 `sessionRefresh` 任务（后端 `TaskDef.executor = None` 的任务，后端仅代管配置与元数据）；`planBalance` 任务执行体在后端 poller，前端调度器不注册。
- 任务元数据（taskId 合法值集 / 默认值 / 频率边界）单点在后端注册表，前端只经 `src/types/backgroundTasks.ts` 常量引用（DTO 无 default 字段），不复制边界。

## 测试模式

- **调度器行为**：fake timers 断言 interval 按 `intervalSec` 触发/退订停止/重订阅不重复读配置；`../ipc/backgroundTasks` hoisted mock（`listBackgroundTasks` 可控）驱动 activate 分支（enabled 真/假/list 失败）；每用例 `backgroundTaskScheduler._reset()`。
- **执行体聚合**：mock `../ipc/agentHistory`（`scanAgentHistory` hoisted 可控）+ `../ipc/backgroundTasks`（list 恒返回 sessionRefresh 配置防 activate 真实 invoke）；文件顶部 `import "../features/backgroundTasks/tasks"` 注册；beforeEach `_reset()` 双注册表（调度器 + cliProfileRegistry）。
- 均依赖 `setup.ts` 全局 mock 的 `../ipc/backgroundTasks`（默认两任务清单 + set resolve [] + 事件订阅 no-op）作为兜底。
