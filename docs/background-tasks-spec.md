# F12 后台定时任务 需求规格

- 状态：已澄清（2026-08-30 grilling 会话收口）
- 范围：session 历史定时刷新（新特性）+ 后台定时任务框架抽象 + 套餐余量兼容性改造 + 设置中心页改造
- 非目标见 §10

## 1. 背景与目标

现状：导航树历史会话区的刷新 = 挂载一次 + 手动刷新钮（`scan(true)`）；活跃会话区由 claude hooks 事件驱动（本规格不动）。套餐余量查询是唯一的后台定时任务（后端 tokio poller）。

目标：

1. **session 定时刷新**：历史会话区按可配频率自动刷新，与手动刷新钮严格同一代码路径（单一执行体）。
2. **提前支持无 hooks 的 CLI**：历史 provider 扫描文件系统本不依赖 hooks——扫描执行体遍历全部已注册 history provider，新 CLI 注册 provider 即自动纳入定时刷新与手动刷新。
3. **框架抽象**：双端各自抽象后台定时任务骨架（高内聚、低耦合、易扩展），新增任务 ≈ 注册一条元数据 + 写执行体，框架与设置页零改动。
4. **设置中心**：「套餐余量」页升级为「后台定时任务」页，统一管各任务的启用与频率。

## 2. 术语

| 术语 | 定义 |
|------|------|
| 后台定时任务 | 应用级周期性执行的任务单元。元数据单点 = 后端任务注册表（taskId/标题/频率边界/默认值）；执行体在后端（poller 骨架驱动，如套餐余量）或前端（前端调度器驱动，如 session 刷新）；配置统一持久化于 settings.json `backgroundTasks` 段 |
| 扫描执行体 | 历史会话扫描的唯一执行路径——遍历全部已注册 history provider 逐个扫描并聚合。手动刷新与定时刷新同为它的触发器 |
| 触发来源 | `manual`（刷新钮 / triggerNow）或 `tick`（定时器）——仅影响失败处理策略（§7） |

## 3. 现状锚点（代码自证）

- 后端套餐余量 poller：`src-tauri/src/plan_balance/mod.rs`（`start_plan_balance_poller` / `POLL_INTERVAL_SEC` 原子量 / `plan_balance_set_interval` 命令 / `resolve_poll_interval` 读 `planBalance.intervalSec`，默认 60、合法 10–3600）。
- 前端历史扫描：`useAgentHistory.scan(force)` → `scanAgentHistory(CLAUDE_CLI_ID, force)`——cliId 硬编码 claude；后端按 `(目录 mtime, 文件数)` 缓存，进行中会话的 jsonl 内容更新不改目录 mtime，**故有效刷新必须 force=true**。
- 刷新钮：NavTree「导航」头 `nav.refresh()` = `history.scan(true)`。
- 设置页：`pages.ts` 注册 `planBalance` 页（global 组 order 20）→ `PlanBalancePage.tsx`（单频率输入，失焦/回车提交，非法行内红字）。
- settings.rs 顶层键白名单 5 项，含 `plan_balance::SETTINGS_KEY`（= `"planBalance"`）；`save_settings` 为顶层键**浅合并**。

## 4. 总体架构

双端各自抽象，配置模型统一：

```
设置中心「后台定时任务」页
   │  background_tasks_list()（读）/ background_tasks_set_config()（写）
   ▼
后端任务骨架（src-tauri/src/background_tasks/）
   ├─ 任务注册表（元数据单点：taskId/title/边界/默认/执行体 Option）
   │     ├─ planBalance：执行体=Some（poller 骨架驱动，现有轮询编排下沉为执行体）
   │     └─ sessionRefresh：执行体=None（前端任务，后端仅代管配置与元数据）
   └─ 配置单写通道（taskId 子键合并 → settings.rs 写通道落盘）
前端调度器（src/features/backgroundTasks/，注册表家族契约 #13）
   └─ sessionRefresh：执行体=扫描执行体（遍历 cliProfileRegistry 中
        声明 history 能力的 profile，逐个 scanAgentHistory(cliId, true) 聚合）
```

任务元数据（id/标题/边界/默认值）单点在后端注册表，前端任务同样在后端登记（执行体字段为 None）。taskId 合法值集前后端同步，测试锁死（照 HooksLayer ↔ `Layer` 枚举先例，硬约束 #4）。

## 5. 功能需求

### FR-1 前端调度器（backgroundTasks 模块）

注册表家族契约（硬约束 #13）：模块级单例、`register(task)` / `getAll()` / `_reset()`（仅测试）、side-effect import 触发注册（触发点登记模块 CLAUDE.md）。

任务定义：`{ id, run(source: TriggerSource): Promise<void> }`；调度器统一提供：

- **配置读取**：启动时经 `background_tasks_list()` 取本任务配置；`enabled=false` 不启动定时。
- **生命周期**：`subscribe(listener)` 返回退订函数；**首个订阅者出现 → 立即执行一轮（接管现行「挂载即扫」语义）+ 启动 interval**；**最后订阅者退订 → 停 interval（无订阅者不空转扫盘）**。
- **tick 防重入**：上一轮未结束跳过本 tick（照 plan_balance poller 每轮串行语义）。
- **triggerNow()**：手动触发（刷新钮）——与 tick 共用同一 `run`，仅 `source` 不同；与进行中的 tick 互斥（防重入同一闸门）。
- **运行期改配**：`applyConfig(id, cfg)`——设置页 `set_config` 成功后直调，立即生效（启停/改频率）；手改 settings.json 文件运行期不生效、重启生效（与 plan_balance 现行语义一致）。
- **结果快照与分发**：调度器持任务结果快照（含 sessions 列表），订阅者收到推送；提供本地变更透传接口（`removeLocal` 语义上移：删除会话后本地移除列表项不重扫）。
- **失败处理（§7）**：`tick` 失败静默；`manual` 失败置 error 态。

`useAgentHistory` 改造：sessions/state 改为订阅调度器快照（状态机 `idle|loading|ready|error` 语义不变，真值源上移调度器）；`scan(force)` 公开面保留为 `triggerNow()` 转发（NavTree 刷新钮改调 triggerNow）；`activeStatuses`（TerminalRegistry 订阅）与 `rootPath` 推导保持 hook 本地不变。

### FR-2 后端任务骨架（background_tasks 模块）

新模块 `src-tauri/src/background_tasks/`，从 plan_balance 上提通用件：

- **任务注册表**：静态切片（照 plan_balance `SOURCES`/`QUERIES` 先例 U2）；条目 = `{ task_id, title, interval_min, interval_max, interval_default, enabled_default, 执行体: Option<fn> }`。
- **poller 驱动**：对执行体为 Some 的任务：启动时从磁盘初始化内存配置（enabled/intervalSec）→ `enabled=true` 才 spawn 循环；每轮 = spawn_blocking 执行体 + **每轮末按当前内存间隔 sleep**（首轮立即执行语义保留）；`set_config` 改值下一轮即生效；运行期 enabled→false 停循环、→true 重新 spawn。
- **配置命令**（三处注册：lib.rs + build.rs + capabilities）：
  - `background_tasks_list() -> Vec<BackgroundTaskInfo>`：全部任务元数据 + 当前生效配置（内存值）——设置页与前端调度器共用此读通道。
  - `background_tasks_set_config(task_id, enabled?, interval_sec?) -> Vec<BackgroundTaskInfo>`：taskId 白名单校验（合法集 = 注册表键集）→ 边界校验（越界 → Validation，磁盘/内存均不变）→ **按 taskId 子键合并**（读-改-写 `backgroundTasks` 段，复用 settings.rs 写通道：白名单/原子写/.bak/SETTINGS_SAVE_LOCK，禁止自建第二写通道）→ 更新内存值 → 返回完整清单。顺序写死：校验 → 落盘 → 内存。
- **DTO**：`BackgroundTaskInfo { taskId, title, enabled, intervalSec, intervalMin, intervalMax }`（serde camelCase ↔ `src/types/backgroundTasks.ts`，硬约束 #4 双边对应）。

### FR-3 plan_balance 兼容性改造

- 轮询通用件（间隔内存原子量/每轮末 sleep/读盘初始化）上提骨架；`poll_once_production`/`apply_snapshot`/`merge_slot` 等套餐语义下沉为执行体，**行为不变**。
- `plan_balance_set_interval` 命令退役，由 `background_tasks_set_config` 取代（无迁移负担，见 §9）。
- **enabled 语义（新）**：`enabled=false` → 停轮询 + **快照保留** + 前端 footer 不渲染（禁用即不关注；数据仍可读）→ `enabled` 恢复 true 时重显最后快照并恢复轮询。**手动行点击刷新不受 enabled 影响**（`refresh_plan_balance` 保留）。
- 默认间隔 60 → **10s**（合法区间 10–3600 不变）。emit 口径（D5 含 updated_at 比较）不变。
- settings.rs 白名单：`planBalance` 键移除，替换为 `background_tasks::SETTINGS_KEY`（= `"backgroundTasks"`，仍 5 键；键归域先例——settings.rs 白名单与命令 payload 均经常量引用）。

### FR-4 session 刷新任务（sessionRefresh）

- 注册：后端注册表登记元数据（执行体 None）；前端调度器注册执行体。
- 执行体 = 扫描执行体：遍历 `cliProfileRegistry.getAll()` 中声明 history 能力的 profile，逐个 `scanAgentHistory(profile.id, true)`，聚合为扁平会话列表（归组/排序仍由 `historyModel` 消费侧负责，不动）。
- **多 provider 失败隔离**：单 provider 失败 → 该 provider 保留旧数据、其余采用新值；全部失败时按触发来源走 §7。
- 配置：默认 enabled=true，默认 3s，合法 2–300s。

### FR-5 设置中心「后台定时任务」页

- `pages.ts`：`planBalance` 页注册移除，替换为 `{ id: "backgroundTasks", title: "后台定时任务", group: "global", order: 20 }`；`PlanBalancePage.tsx` 删除，新页组件替代。
- 页组件：挂载经 `background_tasks_list()` 拿任务清单 + 生效配置，**通用行组件纯渲染**（新增任务自动出现，页零改动）：每行 = 任务标题 + 启用勾选 + 频率输入（秒）+ 范围提示。
- 提交语义（照现行 PlanBalancePage 先例）：勾选切换立即提交 `set_config(taskId, {enabled})`；频率失焦/回车提交，非法（非数/非整数/越界）→ 行内红字提示，不提交不 toast；后端拒绝 → toast + 保留用户输入。本页立即提交型，无 dirty 暂存。
- 提交成功后：planBalance 由后端内存值即时生效；sessionRefresh 由页组件直调前端调度器 `applyConfig` 即时生效。
- **深链失配兜底**：壳 `get(selectedPage)` 未命中（旧布局持久化 `planBalance` 页 id）→ 回退 global 组第一页。
- data-e2e：`settings-plan-balance-*` 系列替换为 `settings-background-tasks-*` 系列（行/勾选/输入/红字各一）。

## 6. 配置模型

settings.json 顶层段（白名单第 5 键 `"backgroundTasks"`）：

```json
{
  "backgroundTasks": {
    "planBalance":    { "enabled": true, "intervalSec": 10 },
    "sessionRefresh": { "enabled": true, "intervalSec": 3 }
  }
}
```

| taskId | enabled 默认 | intervalSec 默认 | 合法区间 | 执行端 |
|--------|------|------|------|--------|
| planBalance | true | 10 | 10–3600 | 后端 poller |
| sessionRefresh | true | 3 | 2–300 | 前端调度器 |

读取钳制：缺失/损坏/越界 → 回退默认（照 `resolve_poll_interval` 现行口径，逐任务独立）。GUI 拒绝越界写与手改越界读回退两层不矛盾。

## 7. 失败处理

| 触发来源 | 失败行为 |
|------|------|
| tick（定时） | console.error + 保留旧列表 + **不改变视图状态机**（不置 error）；无连续失败熔断（本地扫描失败率极低） |
| manual（刷新钮） | 保持现行置 `error` 态 |
| 多 provider 部分失败 | 失败 provider 保留旧数据，成功 provider 采用新值；全部失败才按来源走上行两策略 |

## 8. 关键行为边界

- **活跃会话区不动**：事件驱动（hooks/OSC 133/TerminalRegistry），无定时对账、无后端轮询通道。
- **force 语义**：扫描执行体恒 `force=true`（后端 mtime 缓存对进行中会话不敏感，见 §3），手动与定时同。
- **空转**：sessionRefresh 无订阅者暂停；planBalance 后端 poller 恒跑（启动预热语义不变，仅受 enabled 控制）。
- **窗口最小化/失焦**：不特殊处理（WebView2 默认定时器行为），登记为已知观察项。
- **面板单例/换区重建**：调度器全局单例与 UI 解耦，NavTree 卸载（ADR-0001）不影响定时刷新。

## 9. 兼容性

- 项目单用户，**不做旧配置迁移**：旧 `planBalance.intervalSec` 键直接废弃（白名单移除后不再读写；残留于文件无害）。
- 旧布局持久化的设置面板深链 `selectedPage="planBalance"` 失配 → 壳回退组内第一页（FR-5）。
- 套餐余量对外行为除「默认 10s / 可禁用 / footer 随禁用隐藏」外不变。

## 10. 非目标

- 活跃会话区的定时刷新/对账（事件驱动已实时，无可轮询的后端数据源）。
- 无 hooks CLI 的活跃区状态轮询通道（OSC 133 双态维持现状）。
- 旧配置迁移、多用户兼容。
- 套餐余量后端轮询改前端（poller 保留后端）。

## 11. 测试策略

硬约束 #11：改动可自动化部分全量自动化；用例清单同步 `.claude/test-inventory.md`。

- **L1（后端）**：任务注册表（元数据键集/边界表锁死）；`set_config`（合法合并落盘+内存、越界 Validation 双不变、taskId 白名单拒绝、磁盘/内存一致性）；`list` 命令；配置解析钳制（缺失/损坏/越界回退）；plan_balance 执行体移植后既有用例全保留（merge_slot/poll_once_with/serde 键集合等）；DTO serde 键集合精确匹配。
- **L2（前端）**：调度器（注册表契约/订阅者计数启停/首轮立即/防重入/tick 静默 vs manual error/applyConfig 运行期生效/removeLocal 透传）；useAgentHistory 订阅改造（快照消费/状态机）；扫描执行体（provider 遍历/聚合/部分失败隔离）；设置页（清单渲染/勾选提交/频率非法红字/后端拒绝 toast/深链兜底）；pages.ts 注册守卫同步更新。
- **L4**：设置中心「后台定时任务」页操作链路（改频率/勾选启用的端到端生效）；定时刷新端到端（test-inventory 登记）。
- **契约**：ipc contract 测试双命令四维权（照 ipc-plan-balance-contract 先例）。

## 12. 验收标准

1. 设置中心 global 组出现「后台定时任务」页，含套餐余量查询与 session 刷新两行，各可勾选启用、可配频率（默认 10s/3s）；新增注册任务自动出现在页内。
2. session 刷新启用时，历史会话区按配置频率自动更新（新会话出现/进行中会话标题与时间与磁盘一致），与点击刷新钮结果完全一致（同一执行体）。
3. 勾选禁用套餐余量 → 轮询停止、footer 隐藏、最后快照保留；重新启用 → 恢复。
4. 频率修改立即生效（不等重启）；越界输入行内红字不落盘。
5. 定时扫描失败不产生 toast/error 态；手动刷新失败置 error 态。
6. 切走 nav 视图（NavTree 卸载）定时刷新暂停；切回立即执行一轮。
7. 套餐余量既有展示/手动刷新/冻结态等行为回归无损。
