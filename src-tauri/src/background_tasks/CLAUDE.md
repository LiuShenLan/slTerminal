# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/background_tasks` 是 F12 后台定时任务骨架：任务注册表（元数据单点）+ poller 驱动 + 配置单写通道命令。骨架独立于 plan_balance 的理由：套餐余量查询之后新增第二个后台任务（session 历史刷新），两任务机制同构（间隔内存原子量/首轮立即执行/每轮末 sleep/读盘钳制初始化），机制若继续藏在 plan_balance 内，第三个任务起即重复造轮子；且两任务共用 settings.json 一段配置（`backgroundTasks`），settings 浅合并是顶层键粒度，并发段写会互覆子键——配置必须收口单写通道统一落盘。「为什么这样设计」（U2 静态切片/顺序写死/锁序/单写通道/事件感知）无法从代码自证，必须文档化。

## 关键约束与决策

### 注册表形态 = 静态切片（U2，偏离硬约束 #13 可变单例）

`registry.rs` 的 `TASKS`/`RUNTIMES` 为静态切片（照 `plan_balance/source.rs` SOURCES/QUERIES 先例）。偏离 #13 可变单例的理由：Rust 无 side-effect import（#13 先例全为前端模块）。锁死手段：`tasks_registry_key_set_locked` 精确断言键集与逐任务六字段（边界表）+ `runtimes_same_length_as_tasks` 等长守卫。新增任务 = `TASKS` 追加一行 + `RUNTIMES` 追加一项 + 前端 `BACKGROUND_TASK_IDS` 加一项（taskId 值集双边字面量测试锁死，硬约束 #4 先例：HooksLayer ↔ `Layer` 枚举）。

### 顺序写死「校验 → 落盘 → 内存」

`set_config_core` 执行顺序固定：taskId 白名单校验 → 边界校验（越界 → Validation，磁盘/内存均不变）→ 读-改-写 `backgroundTasks` 段子键合并 → 落盘 → 落盘成功才更新内存原子量。落盘失败内存不变，磁盘/内存恒一致（`set_config_disk_memory_consistent` 锁死）。启动时 `resolve_task_config` 读盘钳制初始化（缺失/损坏/类型错/越界 → 逐字段独立回退默认，照 plan_balance `resolve_poll_interval` 口径提升为逐任务）。

### 单写通道 = 复用 settings.rs，禁止第二通道；锁序单向

落盘复用 `settings.rs` 的 `save_settings_blocking` 同步写通道（白名单/浅合并/原子写/.bak/SETTINGS_SAVE_LOCK 全套），**禁止自建第二写通道**（防 SPE-06 并发写竞态回归）；前端消费型 `save_settings` 段写不适用于本段（顶层键浅合并会互覆任务子键）。并发 set_config 的读-改-写跨子键合并必须互斥（否则后写覆盖前写的其他任务子键）——`CONFIG_WRITE_LOCK` 持锁串行化整个 core，其内部再入 `SETTINGS_SAVE_LOCK`（save_settings_blocking 内）。**锁序单向：CONFIG_WRITE_LOCK → SETTINGS_SAVE_LOCK，无环。**

### 配置变更感知 = emit 事件（后端单写通道真值源）

`background_tasks_set_config` 成功后 emit `background-tasks-updated`（payload = 完整 `BackgroundTaskInfo[]`）——前端 footer/设置页订阅即知；`background_tasks_list` 只作读通道，不 emit。enabled 停/启语义：运行期禁用 → poller 轮首检查退出循环（running 置 false，**快照保留**）；重新启用 → 命令包装层重 spawn（enabled false→true 且执行体 Some 且循环未在跑）。

### 前端任务 executor=None 仅代管

`executor: Option<TaskExecutor>`——None = 前端任务（sessionRefresh 先例）：后端不 spawn 循环，仅代管配置读写与元数据（id/标题/边界/默认值），执行体在前端调度器（src/features/backgroundTasks）。DTO `BackgroundTaskInfo` 六键**无 default 字段**（FR-2 写死）——默认值单点在后端注册表，前端不复制边界/默认值，行内提示只写范围不写默认值（serde 键集合精确断言锁死）。

## 外部坑/红线

- **`save_settings_blocking` 是唯二消费点**：全仓唯一 settings.json 写通道，消费方 = settings.rs `save_settings` 命令 + 本模块 `set_config_core` 两处；改动它必须先查两处调用（`background_tasks::SETTINGS_KEY` 为 settings.rs 白名单第 5 键引用，防字面量漂移）。
- **spawn/emit 包装层 L1 豁免**：`background_tasks_set_config` 命令包装层（spawn_blocking / 重 spawn / emit）与 `spawn_poller` 循环本体需 AppHandle/tauri runtime，无法 L1 直测——既定豁免登记（见下）。
- **锁内不做可能 panic 的工作**：Mutex 中毒不可达纪律（照 src-tauri/src/CLAUDE.md）。

## 测试模式

- `registry.rs`：`tasks_registry_key_set_locked` / `runtimes_same_length_as_tasks` / `find_hit_and_miss` / `resolve_task_config_*` 6 例（AppDataDirGuard 注入 tempdir，逐字段独立钳制）。
- `mod.rs`：`background_task_info_serde_key_set`（六键精确）/ `list_returns_registry_order_with_defaults` / `set_config_core` 7 例（校验/落盘/合并/一致性）——每例首行 `reset_runtimes_for_test()` 重置内存值（--test-threads=1 门禁保证无并发干扰），current_thread runtime block_on 驱动 async 命令。
- 键集前后端双边锁：本模块 serde 断言 ↔ `src/__tests__/ipc-background-tasks-contract.test.ts`（键集合 + 值集）↔ `src/__tests__/background-tasks-scheduler.test.ts`（值集断言）。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| spawn_poller 循环本体与命令包装层（emit/重 spawn） | 需 AppHandle/tauri runtime，L1 无法直测 | L4 勾选启停端到端 + 人工实测——登记于 test-inventory（F12） |
