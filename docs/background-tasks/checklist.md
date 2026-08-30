# F12 后台定时任务——开发清单

- 真值源：`docs/background-tasks-spec.md`（已澄清，2026-08-30 grilling 收口）
- 编号：BE=后端 / FE=前端 / E2E=端到端 / DOC=文档；**不用 P0-P4，优先级由 stages.md 的 Stage 依赖顺序表达**
- 现状代码均经计划期实读核验（行号以 2026-08-30 工作区为准）

## 用户决策记录（grilling 收口）

1. **footer enabled 感知通道** = 后端 `set_config` 成功后 emit `background-tasks-updated` 事件（携带完整 `BackgroundTaskInfo[]`）；footer 挂载读 `background_tasks_list` + 订阅该事件。
2. **useAgentHistory 公开面** = `scan(force)` 退役，改名 `triggerNow()`（NavTree 刷新钮改调）。
3. **L4 范围** = 全量：settings.e2e.ts 既有用例适配 + 新增页操作链路 + 定时刷新端到端（真实 tick 等待断言）。

## 跨边界契约（写死，双端不各自推断）

- IPC 命令：`background_tasks_list() -> Vec<BackgroundTaskInfo>`（无参）；`background_tasks_set_config(task_id: String, enabled: Option<bool>, interval_sec: Option<u64>) -> Vec<BackgroundTaskInfo>`（带 `app_handle`，serde 参数 JS 侧 camelCase：`{ taskId, enabled?, intervalSec? }`，缺省键不发送）。
- 事件：`background-tasks-updated`，payload = `BackgroundTaskInfo[]`（set_config 成功后 emit；list 不 emit）。
- DTO `BackgroundTaskInfo` 六键（serde camelCase）：`taskId / title / enabled / intervalSec / intervalMin / intervalMax`——**无 default 字段**（规格 FR-2 写死六键），前端行内提示文案只写范围不写默认值。
- taskId 合法值集 = `["planBalance", "sessionRefresh"]`：后端 `TASKS` 静态切片键集 ↔ 前端 `BACKGROUND_TASK_IDS` 常量，两侧各自字面量测试锁死（照 HooksLayer ↔ Layer 先例，硬约束 #4）。
- 任务元数据（后端注册表单点，前端不复制）：

  | taskId | title | enabled 默认 | intervalSec 默认 | 合法区间 | 执行体 |
  |---|---|---|---|---|---|
  | planBalance | 套餐余量查询 | true | 10 | 10–3600 | Some（后端 poller 驱动） |
  | sessionRefresh | 会话历史刷新 | true | 3 | 2–300 | None（前端调度器驱动） |

- settings.json 段：顶层键 `"backgroundTasks"`（`background_tasks::SETTINGS_KEY`，settings.rs 白名单第 5 键引用），子键 per taskId：`{ "enabled": bool, "intervalSec": u64 }`。
- 前端调度器快照形状：`TaskSnapshot<T> = { state: "idle"|"loading"|"ready"|"error", data: T | undefined }`；sessionRefresh 的 T = `AgentHistorySession[]`（扁平聚合）。
- 任务定义：`{ id: string, run(source: TriggerSource, prev: T | undefined): Promise<T> }`，`TriggerSource = "manual" | "tick"`。
- data-e2e 系列：`settings-background-tasks-page` / `settings-background-tasks-row-{taskId}` / `settings-background-tasks-enabled-{taskId}` / `settings-background-tasks-interval-{taskId}` / `settings-background-tasks-error-{taskId}`；footer 既有 `plan-balance-footer` / `plan-balance-row` 不动。
- 退役：`plan_balance_set_interval` 命令 / `setPlanBalanceInterval` wrapper / settings 页 id `planBalance` / `settings-plan-balance-*` 选择器 / settings.json `planBalance` 键（白名单移除，旧文件残留不迁移）。

---

## BE-01 新建 background_tasks/registry.rs（注册表 + 配置钳制）

1. **位置**：新建 `src-tauri/src/background_tasks/registry.rs`。
2. **现状**：无此文件。先例：`plan_balance/source.rs` 的 `SOURCES` 静态切片（U2 形态，偏离 #13 可变单例——Rust 无 side-effect import）。
3. **修复步骤**（照抄级）：

   - 3.1 写完整文件：

   ```rust
   //! 任务注册表（元数据单点）+ 运行时内存配置 + 配置读取钳制
   //!
   //! 形态 = 静态切片（照 plan_balance SOURCES/QUERIES 先例 U2）：新增任务 = TASKS 追加
   //! 一行 + RUNTIMES 追加一项（等长守卫测试锁死）。taskId 合法值集 ↔ 前端
   //! BACKGROUND_TASK_IDS 常量双边锁死（硬约束 #4）。

   use std::sync::atomic::{AtomicBool, AtomicU64};

   use crate::app_dir::app_data_dir;

   /// 执行体签名：poller 骨架每轮在 spawn_blocking 内调用；AppHandle 供 emit
   pub type TaskExecutor = fn(tauri::AppHandle);

   /// 任务元数据（静态切片条目）
   pub struct TaskDef {
       pub task_id: &'static str,
       pub title: &'static str,
       pub interval_min: u64,
       pub interval_max: u64,
       pub interval_default: u64,
       pub enabled_default: bool,
       /// None = 前端任务（后端仅代管配置与元数据，不 spawn 循环）
       pub executor: Option<TaskExecutor>,
   }

   /// 任务运行时内存配置（与 TASKS 同序对齐；启动时读盘初始化，set_config 写）
   pub struct TaskRuntime {
       pub enabled: AtomicBool,
       pub interval_sec: AtomicU64,
       /// poller 循环是否在跑（停循环后置 false；重启用时据此判定重 spawn）
       pub running: AtomicBool,
   }

   /// settings.json 顶层键（settings.rs 白名单第 5 键引用此常量，防字面量漂移）
   pub(crate) const SETTINGS_KEY: &str = "backgroundTasks";

   /// 任务注册表（元数据单点）——键集与前端 BACKGROUND_TASK_IDS 同步
   pub static TASKS: &[TaskDef] = &[
       TaskDef {
           task_id: "planBalance",
           title: "套餐余量查询",
           interval_min: 10,
           interval_max: 3600,
           interval_default: 10,
           enabled_default: true,
           executor: Some(crate::plan_balance::poll_once_executor),
       },
       TaskDef {
           task_id: "sessionRefresh",
           title: "会话历史刷新",
           interval_min: 2,
           interval_max: 300,
           interval_default: 3,
           enabled_default: true,
           executor: None,
       },
   ];

   /// 运行时内存配置（与 TASKS 同序——静态初值 = 默认值，启动时 resolve_task_config 覆盖）
   pub static RUNTIMES: [TaskRuntime; 2] = [
       TaskRuntime {
           enabled: AtomicBool::new(true),
           interval_sec: AtomicU64::new(10),
           running: AtomicBool::new(false),
       },
       TaskRuntime {
           enabled: AtomicBool::new(true),
           interval_sec: AtomicU64::new(3),
           running: AtomicBool::new(false),
       },
   ];

   /// 按 taskId 查（元数据 + 运行时）——未知 taskId → None（命令层转 Validation）
   pub(crate) fn find(task_id: &str) -> Option<(&'static TaskDef, &'static TaskRuntime)> {
       TASKS.iter().zip(RUNTIMES.iter()).find(|(d, _)| d.task_id == task_id)
   }

   /// 单任务生效配置（读盘钳制：缺失/损坏/类型错/越界 → 逐字段独立回退默认，
   /// 照 plan_balance resolve_poll_interval 口径提升为逐任务）
   pub(crate) struct ResolvedConfig {
       pub enabled: bool,
       pub interval_sec: u64,
   }

   pub(crate) fn resolve_task_config(def: &TaskDef) -> ResolvedConfig {
       let read = || -> Option<serde_json::Value> {
           let path = app_data_dir().ok()?.join("settings.json");
           let content = std::fs::read_to_string(path).ok()?;
           let root: serde_json::Value = serde_json::from_str(&content).ok()?;
           root.get(SETTINGS_KEY)?.get(def.task_id).cloned()
       };
       let mut cfg = ResolvedConfig {
           enabled: def.enabled_default,
           interval_sec: def.interval_default,
       };
       if let Some(v) = read() {
           if let Some(b) = v.get("enabled").and_then(|x| x.as_bool()) {
               cfg.enabled = b;
           }
           if let Some(n) = v.get("intervalSec").and_then(|x| x.as_u64()) {
               if (def.interval_min..=def.interval_max).contains(&n) {
                   cfg.interval_sec = n;
               }
           }
       }
       cfg
   }
   ```

   - 3.2 同文件 `#[cfg(test)] mod tests`（L1，AppDataDirGuard 注入 tempdir，照 plan_balance 先例）：
     - `tasks_registry_key_set_locked`：TASKS 键集精确 == `["planBalance", "sessionRefresh"]`，且逐条断言两任务六字段（title/min/max/default/enabled_default）——边界表锁死。
     - `runtimes_same_length_as_tasks`：TASKS.len() == RUNTIMES.len()（等长守卫）；RUNTIMES 静态初值 == 对应 def 默认值。
     - `find_hit_and_miss`：命中返回元数据+运行时；`"nope"` → None。
     - `resolve_task_config_*`（6 例，每例 tempdir + AppDataDirGuard）：无文件 → 默认；合法值采用；intervalSec 越界回退默认但 enabled 合法值保留（逐字段独立）；enabled 非 bool 回退默认；段非对象（`"backgroundTasks": 5`）回退默认；损坏 JSON 回退默认。
4. **测试同步**：即 3.2（同文件 cfg(test)）；用例登记 test-inventory（DOC-02）。
5. **文档同步**：新建 `src-tauri/src/background_tasks/CLAUDE.md`（DOC-01）。
6. **验证**：`cargo test --manifest-path src-tauri/Cargo.toml background_tasks -- --test-threads=1` 全绿；grep `pub static TASKS` 命中 registry.rs；TASKS/RUNTIMES 等长。

## BE-02 新建 background_tasks/mod.rs（DTO + poller 骨架 + 命令）

1. **位置**：新建 `src-tauri/src/background_tasks/mod.rs`。
2. **现状**：无此文件。轮询通用件现状在 `plan_balance/mod.rs:203-220`（`start_plan_balance_poller`：启动读盘初始化内存原子量 → 首轮立即执行 → 每轮末按当前内存间隔 sleep）。
3. **修复步骤**：

   - 3.1 写完整文件：

   ```rust
   //! 后台定时任务骨架（F12）——任务注册表（registry.rs）+ poller 驱动 + 配置命令
   //!
   //! 轮询通用件自 plan_balance 上提：内存配置（enabled/intervalSec 原子量）/首轮立即
   //! 执行/每轮末按当前内存间隔 sleep。套餐语义（resolve/fetch/merge/emit 口径）下沉
   //! plan_balance 执行体 poll_once_executor，行为不变。
   //! 配置单写通道：set_config = 校验 → 复用 settings.rs 写通道落盘（读-改-写子键合并，
   //! 禁止自建第二写通道）→ 更新内存 → emit 配置变更事件 → 返回完整清单。

   pub mod registry;

   use std::sync::atomic::Ordering;
   use std::time::Duration;
   use tauri::Emitter;

   use crate::error::AppError;
   use registry::{TaskDef, TaskRuntime, TASKS, RUNTIMES};

   // ── DTO（serde camelCase ↔ src/types/backgroundTasks.ts，硬约束 #4） ──

   #[derive(Debug, Clone, PartialEq, serde::Serialize)]
   #[serde(rename_all = "camelCase")]
   pub struct BackgroundTaskInfo {
       pub task_id: String,
       pub title: String,
       pub enabled: bool,
       pub interval_sec: u64,
       pub interval_min: u64,
       pub interval_max: u64,
   }

   fn current_list() -> Vec<BackgroundTaskInfo> {
       TASKS
           .iter()
           .zip(RUNTIMES.iter())
           .map(|(def, rt)| BackgroundTaskInfo {
               task_id: def.task_id.into(),
               title: def.title.into(),
               enabled: rt.enabled.load(Ordering::Relaxed),
               interval_sec: rt.interval_sec.load(Ordering::Relaxed),
               interval_min: def.interval_min,
               interval_max: def.interval_max,
           })
           .collect()
   }

   // ── poller 驱动 ──

   /// 单任务轮询循环（enabled=true 才由调用方 spawn）：首轮立即执行（D8 语义保留），
   /// 每轮末按当前内存间隔 sleep——set_config 改值下一轮即生效；轮首检查 enabled，
   /// 运行期禁用 → 退出循环（running 置 false，快照保留，重启用由 set_config 重 spawn）
   fn spawn_poller(app: tauri::AppHandle, def: &'static TaskDef, rt: &'static TaskRuntime) {
       let Some(executor) = def.executor else { return };
       rt.running.store(true, Ordering::Relaxed);
       tauri::async_runtime::spawn(async move {
           loop {
               if !rt.enabled.load(Ordering::Relaxed) {
                   break;
               }
               let handle = app.clone();
               if let Err(e) = tokio::task::spawn_blocking(move || executor(handle)).await {
                   tracing::warn!(task = def.task_id, error = %e, "后台任务执行体异常");
               }
               let secs = rt.interval_sec.load(Ordering::Relaxed);
               tokio::time::sleep(Duration::from_secs(secs)).await;
           }
           rt.running.store(false, Ordering::Relaxed);
       });
   }

   /// lib.rs setup 调用：逐任务读盘初始化内存配置（钳制回退）→ enabled 且有执行体才 spawn
   pub fn start_background_tasks(app: tauri::AppHandle) {
       for (def, rt) in TASKS.iter().zip(RUNTIMES.iter()) {
           let cfg = registry::resolve_task_config(def);
           rt.enabled.store(cfg.enabled, Ordering::Relaxed);
           rt.interval_sec.store(cfg.interval_sec, Ordering::Relaxed);
           if cfg.enabled && def.executor.is_some() {
               spawn_poller(app.clone(), def, rt);
           }
       }
   }

   // ── 配置命令（三处注册：lib.rs + build.rs + capabilities） ──

   /// set_config 读-改-写串行化（SETTINGS_SAVE_LOCK 先例）：并发 set_config 的
   /// 读-改-写跨子键合并必须互斥，否则后写覆盖前写的其他子键。
   /// 锁序单向：CONFIG_WRITE_LOCK → SETTINGS_SAVE_LOCK（save_settings_blocking 内部），无环。
   static CONFIG_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

   /// set_config 核心（同步，spawn_blocking 内执行；L1 直测）——顺序写死：
   /// taskId 白名单校验 → 边界校验（越界 → Validation，磁盘/内存均不变）→
   /// 读-改-写 backgroundTasks 段（子键合并，settings.rs 写通道落盘）→ 更新内存值。
   /// 返回完整清单。spawn/emit 在命令包装层（需 AppHandle，L1 豁免登记）。
   pub(crate) fn set_config_core(
       task_id: &str,
       enabled: Option<bool>,
       interval_sec: Option<u64>,
   ) -> Result<Vec<BackgroundTaskInfo>, AppError> {
       let (def, rt) = registry::find(task_id).ok_or_else(|| {
           AppError::Validation(format!("设置后台任务失败: 未知任务 {task_id}"))
       })?;
       if enabled.is_none() && interval_sec.is_none() {
           return Err(AppError::Validation(
               "设置后台任务失败: 未提供任何配置项".into(),
           ));
       }
       if let Some(sec) = interval_sec {
           if !(def.interval_min..=def.interval_max).contains(&sec) {
               return Err(AppError::Validation(format!(
                   "设置后台任务失败: {} 频率须为 {}–{} 秒，实际 {sec}",
                   def.title, def.interval_min, def.interval_max
               )));
           }
       }
       let _guard = CONFIG_WRITE_LOCK
           .lock()
           .map_err(|_| AppError::Unknown("后台任务配置锁中毒".into()))?;
       // 读现有 backgroundTasks 段（损坏/非对象视作空段——照 save_settings 合并读法口径）
       let section = std::fs::read_to_string(
           crate::app_dir::app_data_dir()?.join("settings.json"),
       )
       .ok()
       .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
       .and_then(|root| root.get(registry::SETTINGS_KEY).cloned())
       .filter(|v| v.is_object())
       .unwrap_or_else(|| serde_json::json!({}));
       // 子键合并：只改本任务子对象，其他任务子键原样保留
       let mut task_cfg = section
           .get(task_id)
           .cloned()
           .filter(|v| v.is_object())
           .unwrap_or_else(|| serde_json::json!({}));
       let obj = task_cfg.as_object_mut().unwrap();
       if let Some(b) = enabled {
           obj.insert("enabled".into(), serde_json::json!(b));
       }
       if let Some(sec) = interval_sec {
           obj.insert("intervalSec".into(), serde_json::json!(sec));
       }
       let mut section = section;
       section
           .as_object_mut()
           .unwrap()
           .insert(task_id.to_string(), task_cfg);
       crate::settings::save_settings_blocking(serde_json::json!({
           registry::SETTINGS_KEY: section
       }))?;
       // 落盘成功才更新内存（磁盘/内存恒一致）
       if let Some(b) = enabled {
           rt.enabled.store(b, Ordering::Relaxed);
       }
       if let Some(sec) = interval_sec {
           rt.interval_sec.store(sec, Ordering::Relaxed);
       }
       Ok(current_list())
   }

   /// 全部任务元数据 + 当前生效配置（内存值）——设置页与前端调度器共用此读通道
   #[tauri::command]
   pub async fn background_tasks_list() -> Result<Vec<BackgroundTaskInfo>, AppError> {
       Ok(current_list())
   }

   /// 设置任务配置：core（校验→落盘→内存）→ enabled false→true 且执行体 Some 且循环
   /// 未在跑 → 重新 spawn → emit background-tasks-updated（前端 footer/调度器感知通道）
   /// → 返回完整清单
   #[tauri::command]
   pub async fn background_tasks_set_config(
       app_handle: tauri::AppHandle,
       task_id: String,
       enabled: Option<bool>,
       interval_sec: Option<u64>,
   ) -> Result<Vec<BackgroundTaskInfo>, AppError> {
       let id = task_id.clone();
       let list = tokio::task::spawn_blocking(move || set_config_core(&id, enabled, interval_sec))
           .await
           .map_err(AppError::from)??;
       if enabled == Some(true) {
           if let Some((def, rt)) = registry::find(&task_id) {
               if def.executor.is_some() && !rt.running.load(Ordering::Relaxed) {
                   spawn_poller(app_handle.clone(), def, rt);
               }
           }
       }
       let _ = app_handle.emit("background-tasks-updated", &list);
       Ok(list)
   }

   /// 测试隔离：内存值重置为注册表默认（--test-threads=1 门禁保证无并发干扰）
   #[cfg(test)]
   pub(crate) fn reset_runtimes_for_test() {
       for (def, rt) in TASKS.iter().zip(RUNTIMES.iter()) {
           rt.enabled.store(def.enabled_default, Ordering::Relaxed);
           rt.interval_sec.store(def.interval_default, Ordering::Relaxed);
           rt.running.store(false, Ordering::Relaxed);
       }
   }
   ```

   - 3.2 同文件 `#[cfg(test)] mod tests`（L1；AppDataDirGuard + current_thread block_on 照 plan_balance 先例；每个 set_config 用例首行 `reset_runtimes_for_test()`）：
     - `background_task_info_serde_key_set`：六键精确 `["enabled","intervalMax","intervalMin","intervalSec","taskId","title"]`。
     - `list_returns_registry_order_with_defaults`：tempdir 无文件 + reset → block_on(background_tasks_list()) 返回两条（注册表序），enabled/intervalSec = 默认。
     - `set_config_valid_interval_persists_and_updates_memory`：`set_config_core("planBalance", None, Some(120))` → 磁盘 `backgroundTasks.planBalance.intervalSec == 120` + RUNTIMES[0] == 120 + 返回清单两条。
     - `set_config_valid_enabled_persists`：`("sessionRefresh", Some(false), None)` → 磁盘子键 enabled=false + 内存 false。
     - `set_config_subkey_merge_preserves_sibling`：先写 planBalance 120，再 set sessionRefresh enabled=false → 磁盘 planBalance.intervalSec 仍为 120（子键不互踩）。
     - `set_config_out_of_range_rejected`：intervalSec=5 → Validation（消息含「套餐余量查询」+「10–3600」）；磁盘无文件 + 内存不变。
     - `set_config_unknown_task_rejected`：`"nope"` → Validation 含「未知任务」；磁盘/内存不变。
     - `set_config_no_fields_rejected`：双 None → Validation 含「未提供任何配置项」。
     - `set_config_disk_memory_consistent`：合法写后磁盘值 == 内存值。
4. **测试同步**：即 3.2。spawn_poller 循环本体与命令包装层（emit/重 spawn）无法 L1 直测（需 AppHandle/tauri runtime）——登记 test-inventory 豁免（兜底层级：L4 勾选启停端到端 + 人工实测）。
5. **文档同步**：新建模块 CLAUDE.md 写明「spawn/emit 包装层 L1 豁免」既定豁免表（DOC-01）。
6. **验证**：`cargo test ... background_tasks` 全绿；grep `background_tasks_set_config` 命中 mod.rs 命令定义；clippy 零警告。

## BE-03 plan_balance 改造（通用件上提，套餐语义下沉为执行体）

1. **位置**：`src-tauri/src/plan_balance/mod.rs`。
2. **现状**（实读）：`POLL_INTERVAL_SEC`（:70）、`resolve_poll_interval`（:188-199）、`SETTINGS_KEY = "planBalance"`（:180）、`INTERVAL_SEC_KEY`（:182）、`DEFAULT/MIN/MAX_INTERVAL_SEC`（:184-186）、`start_plan_balance_poller`（:203-220）、`plan_balance_set_interval`（:245-258）、`reset_poll_interval_for_test`（:78-80）。
3. **修复步骤**：
   - 3.1 删除：`POLL_INTERVAL_SEC`、`reset_poll_interval_for_test`、`resolve_poll_interval`、`SETTINGS_KEY`、`INTERVAL_SEC_KEY`、`DEFAULT_INTERVAL_SEC`、`MIN_INTERVAL_SEC`、`MAX_INTERVAL_SEC`、`start_plan_balance_poller`、`plan_balance_set_interval` 命令；同步删 `use std::sync::atomic::{AtomicU64, Ordering}`（atomic 不再需要——`std::time::Duration` 与 `app_data_dir` import 若无其他消费一并删）。
   - 3.2 新增执行体（放 `apply_snapshot` 之后）：

   ```rust
   /// 后台任务执行体（F12）：background_tasks poller 骨架每轮 spawn_blocking 调用——
   /// 一轮拉取 + 快照应用（行为不变：resolve/fetch/merge/emit 口径含 updated_at 比较）
   pub fn poll_once_executor(app_handle: tauri::AppHandle) {
       let new = poll_once_production(unix_now());
       apply_snapshot(&app_handle, new);
   }
   ```

   - 3.3 `poll_once_production` 由 `fn` 改 `pub(crate) fn` 维持私有即可——执行体在同模块，无需改可见性。
   - 3.4 头注释更新：模块职责段删「轮询间隔」描述，注明「轮询编排骨架已上提 background_tasks（F12），本模块保留套餐语义执行体与快照存储」。
4. **测试同步**：删 10 例——`resolve_poll_interval_*` 4 例（:530-576）、`set_interval_*` 4 例（:601-687）、`poll_interval_memory_default_is_60`（:593-596）、`settings_key_constants_value`（:693-696）；保留 14 例（serde 4 + merge_slot 4 + poll_once 5 + get_plan_balance 1）零改动。
5. **文档同步**：`src-tauri/src/plan_balance/CLAUDE.md`「轮询间隔」节改写（DOC-01）。
6. **验证**：`cargo test ... plan_balance` 14 例全绿；`grep -n "SETTINGS_KEY\|resolve_poll_interval\|start_plan_balance_poller\|plan_balance_set_interval\|POLL_INTERVAL_SEC" src-tauri/src/plan_balance/mod.rs` 零命中；`poll_once_executor` 存在且被 registry.rs 引用。

## BE-04 settings.rs 白名单换键 + save_settings_blocking 抽取

1. **位置**：`src-tauri/src/settings.rs`（白名单 :18-24；save_settings :67-117）。
2. **现状**：白名单第 5 键 `crate::plan_balance::SETTINGS_KEY`；`save_settings` async 命令内嵌 spawn_blocking 闭包（校验在闭包前、写逻辑在闭包内）。background_tasks 的 set_config_core 是同步函数（spawn_blocking 内），跨 await 持 std MutexGuard 会致 future !Send 编译失败——必须抽取同步写通道供其复用（规格 FR-2「复用 settings.rs 写通道，禁止自建第二写通道」）。
3. **修复步骤**：
   - 3.1 白名单第 5 键改 `crate::background_tasks::SETTINGS_KEY`（数组仍 5 项）；头注释 SEC-11 行与「后端消费型域键名归域模块（plan_balance::SETTINGS_KEY 先例）」注释改写为 background_tasks 口径。
   - 3.2 抽取同步写通道：

   ```rust
   /// 同步写通道（F12 抽取）：校验（白名单 + 大小上限）→ 浅合并 → 原子写 + .bak。
   /// 供 async save_settings 与 background_tasks::set_config_core（spawn_blocking 内）共用——
   /// 全仓唯一 settings.json 写通道，禁止另建。
   pub(crate) fn save_settings_blocking(settings: serde_json::Value) -> Result<(), AppError> {
       validate_settings_input(&settings)?;
       let app_dir = app_data_dir()?;
       let settings_path = app_dir.join("settings.json");
       // 以下为原 save_settings spawn_blocking 闭包本体，逐行平移（SETTINGS_SAVE_LOCK
       // 持锁串行化读-合并-写全程，语义不变）
       let _guard = SETTINGS_SAVE_LOCK
           .lock()
           .map_err(|_| AppError::Unknown("settings 保存锁中毒".into()))?;
       std::fs::create_dir_all(&app_dir).map_err(|e| io_error("保存设置", &app_dir, e))?;
       let existing = std::fs::read_to_string(&settings_path)
           .ok()
           .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
           .unwrap_or(serde_json::Value::Null);
       let merged = merge_settings(existing, settings);
       let json = serde_json::to_string_pretty(&merged)?;
       if json.len() > MAX_PERSIST_BYTES {
           return Err(AppError::Validation("保存设置失败: 数据超过 1MB 上限".into()));
       }
       let mut tmp =
           NamedTempFile::new_in(&app_dir).map_err(|e| io_error("保存设置", &app_dir, e))?;
       tmp.write_all(json.as_bytes())
           .map_err(|e| io_error("保存设置", &settings_path, e))?;
       tmp.flush().map_err(|e| io_error("保存设置", &settings_path, e))?;
       if settings_path.exists() {
           let bak = app_dir.join("settings.json.bak");
           if let Err(e) = std::fs::copy(&settings_path, &bak) {
               tracing::warn!(error = %e, path = %settings_path.display(), "settings .bak 备份失败");
           }
       }
       tmp.persist(&settings_path)
           .map_err(|e| io_error("保存设置", &settings_path, e.error))?;
       Ok(())
   }
   ```

   - 3.3 `save_settings` async 命令瘦身为包装：

   ```rust
   #[tauri::command]
   pub async fn save_settings(settings: serde_json::Value) -> Result<(), AppError> {
       match tokio::task::spawn_blocking(move || save_settings_blocking(settings)).await {
           Ok(inner) => inner,
           Err(e) => Err(AppError::TaskJoin(e.to_string())),
       }
   }
   ```

   （注：validate 从「spawn_blocking 前快速失败」移入 blocking 内——行为等价（同 Err 返回），头注释相应行删除「spawn_blocking 前快速失败」表述。）
4. **测试同步**：既有 26 例零改动全保留（命令层行为不变）；`save_accepts_plan_balance_key`（:568-578）改名 `save_accepts_background_tasks_key`，payload 改 `serde_json::json!({ "backgroundTasks": { "planBalance": { "enabled": true, "intervalSec": 120 } } })`，断言文案同步；新增 `save_rejects_plan_balance_key`：`{"planBalance": {...}}` → Validation 含「白名单」且不落盘（旧键退役防回归）。
5. **文档同步**：`src-tauri/src/CLAUDE.md` settings.rs 节白名单口径改写（DOC-01）。
6. **验证**：`cargo test ... settings` 全绿；grep `plan_balance::SETTINGS_KEY` 全仓零命中；grep `background_tasks::SETTINGS_KEY` 命中 settings.rs:18-24 白名单区；`save_settings_blocking` 被 background_tasks/mod.rs 与 settings.rs 两处引用。

## BE-05 三处注册（lib.rs + build.rs + capabilities）

1. **位置**：`src-tauri/src/lib.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/default.json`。
2. **现状**（实读）：lib.rs:8 `mod plan_balance;`、:99 `plan_balance::start_plan_balance_poller(app.handle().clone());`、:137-139 三条 plan_balance 命令注册；build.rs:16 注释「当前 36 条」、:52-54 三条 plan_balance 命令名；capabilities:52-54 三条 allow-plan-balance-*。
3. **修复步骤**：
   - 3.1 lib.rs：`mod app_dir;` 后插入 `mod background_tasks;`（保字母序）；setup 中 `plan_balance::start_plan_balance_poller(app.handle().clone()); // F10 套餐余量轮询` 整行替换为 `background_tasks::start_background_tasks(app.handle().clone()); // F12 后台定时任务骨架（含套餐余量 poller）`；generate_handler 中删 `plan_balance::plan_balance_set_interval,`，在 `agent_history::agent_history_read_title,` 后插入 `background_tasks::background_tasks_list,` 与 `background_tasks::background_tasks_set_config,`。
   - 3.2 build.rs：commands 数组删 `"plan_balance_set_interval"`，`"agent_history_read_title",` 后插入 `"background_tasks_list",` `"background_tasks_set_config",`；:16 注释「当前 36 条」改「当前 37 条」。
   - 3.3 capabilities/default.json：`"allow-agent-history-read-title",` 后插入 `"allow-background-tasks-list",` `"allow-background-tasks-set-config",`；删 `"allow-plan-balance-set-interval"`。
4. **测试同步**：无独立用例；命令可达性由 L2 契约测试（FE-02）+ L4 守卫。build.rs 计数注释由 verify grep 断言。
5. **文档同步**：无（三处注册纪律已在 src-tauri/src/CLAUDE.md 红线节，命令清单代码自证）。
6. **验证**：grep `plan_balance_set_interval` 在 lib.rs/build.rs/capabilities/default.json 零命中；grep `background_tasks_list` 三处各命中一次；build.rs 注释含「37 条」；`cargo clippy` 零警告（编译期证明 generate_handler 注册路径存在）。

---

## FE-01 types/backgroundTasks.ts（DTO + taskId 常量）

1. **位置**：新建 `src/types/backgroundTasks.ts`。
2. **现状**：无此文件。`src/types/planBalance.ts` 为 DTO 双边对应先例。
3. **修复步骤**——写完整文件：

   ```ts
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
   ```

4. **测试同步**：无独立文件；DTO 键集合断言并入 `ipc-background-tasks-contract.test.ts`（FE-02），taskId 值集断言并入调度器测试（FE-03）。
5. **文档同步**：`src/types/CLAUDE.md` 对照表加 `backgroundTasks.ts ↔ src-tauri/src/background_tasks/mod.rs` 行（DOC-01）。
6. **验证**：`npx tsc --noEmit` 通过；grep `BACKGROUND_TASK_IDS` 命中本文件。

## FE-02 ipc/backgroundTasks.ts + planBalance.ts 删 setPlanBalanceInterval + 契约测试

1. **位置**：新建 `src/ipc/backgroundTasks.ts`；改 `src/ipc/index.ts`、`src/ipc/planBalance.ts`；新建 `src/__tests__/ipc-background-tasks-contract.test.ts`；改 `src/__tests__/ipc-plan-balance-contract.test.ts`。
2. **现状**（实读）：`ipc/planBalance.ts:31-34` `setPlanBalanceInterval`；`ipc/index.ts:17` `export * as planBalance`；契约测试 `ipc-plan-balance-contract.test.ts:144-177` setPlanBalanceInterval 四维段。
3. **修复步骤**：
   - 3.1 新建 `src/ipc/backgroundTasks.ts`（照抄）：

   ```ts
   // 后台定时任务 IPC（F12）——清单读取 / 配置写通道 / 配置变更事件订阅
   // 本文件是 background_tasks_list / background_tasks_set_config 的唯一 invoke 位置（硬约束 #1）
   import { invoke } from "@tauri-apps/api/core";
   import { listen } from "@tauri-apps/api/event";
   import type { BackgroundTaskInfo } from "../types/backgroundTasks";

   /** 全部任务元数据 + 当前生效配置（后端内存值） */
   export function listBackgroundTasks(): Promise<BackgroundTaskInfo[]> {
     return invoke("background_tasks_list");
   }

   /**
    * 设置任务配置（写通道：后端校验 → 落盘 → 内存 → emit 变更事件 → 返回完整清单）。
    * enabled/intervalSec 至少提供其一（均缺省 → 后端 Validation）；只发送提供的键
    * （undefined 不入 payload，契约键集合精确断言依赖此行为）。
    */
   export function setBackgroundTaskConfig(
     taskId: string,
     config: { enabled?: boolean; intervalSec?: number },
   ): Promise<BackgroundTaskInfo[]> {
     const args: Record<string, unknown> = { taskId };
     if (config.enabled !== undefined) args.enabled = config.enabled;
     if (config.intervalSec !== undefined) args.intervalSec = config.intervalSec;
     return invoke("background_tasks_set_config", args);
   }

   /** 订阅配置变更（set_config 成功后后端推送完整清单）；返回 unsubscribe */
   export function onBackgroundTasksUpdated(
     callback: (payload: BackgroundTaskInfo[]) => void,
   ): () => void {
     const unlisten = listen<BackgroundTaskInfo[]>(
       "background-tasks-updated",
       (event) => callback(event.payload),
     );
     return () => {
       unlisten.then((fn) => fn());
     };
   }
   ```

   - 3.2 `src/ipc/index.ts:17` 后插入 `export * as backgroundTasks from "./backgroundTasks";`（planBalance 行保留——get/refresh wrapper 仍在用）。
   - 3.3 `src/ipc/planBalance.ts`：删 `setPlanBalanceInterval`（:31-34）及其上方注释行。
   - 3.4 新建 `src/__tests__/ipc-background-tasks-contract.test.ts`（照 ipc-plan-balance-contract.test.ts 模式；setup.ts 无 ../ipc/backgroundTasks 全局 mock，直接真实导入；mock `@tauri-apps/api/event` listen）：
     - `listBackgroundTasks` 四维（命令名 / 无参 payload={} / 透传两任务清单 / 异常传播）。
     - `setBackgroundTaskConfig` 四维（命令名 / payload 键集合精确——`{taskId, intervalSec: 120}` 与 `{taskId, enabled: false}` 两形态各一例 `expectExactKeys` / 返回清单透传 / 异常传播）。
     - `onBackgroundTasksUpdated` 手写模拟驱动两例（照 plan-balance 契约 :187-230 先例：解包 payload + unsubscribe 调 listen 清理函数）。
     - `BackgroundTaskInfo` 键集合六键精确匹配（与后端 serde 测试互为双边锁）。
     - `BACKGROUND_TASK_IDS` 值集精确 == `["planBalance","sessionRefresh"]`（前后端字面量锁死的前端半）。
   - 3.5 `ipc-plan-balance-contract.test.ts`：删 `setPlanBalanceInterval 合约` describe 段（:144-177），文件头注释「三命令」改「两命令」。
   - 3.6 `src/__tests__/setup.ts` 新增全局 mock（照 planBalance 先例）——下游 Stage 03 的 nav-tree 测试经真实 useAgentHistory → 调度器 activate 会触达 listBackgroundTasks，全局 mock 必须先于消费到位：

   ```ts
   // ../ipc/backgroundTasks（F12）：清单返回两任务默认配置，set resolve []，事件订阅 no-op
   vi.mock("../ipc/backgroundTasks", () => ({
     listBackgroundTasks: vi.fn().mockResolvedValue([
       { taskId: "planBalance", title: "套餐余量查询", enabled: true, intervalSec: 10, intervalMin: 10, intervalMax: 3600 },
       { taskId: "sessionRefresh", title: "会话历史刷新", enabled: true, intervalSec: 3, intervalMin: 2, intervalMax: 300 },
     ]),
     setBackgroundTaskConfig: vi.fn().mockResolvedValue([]),
     onBackgroundTasksUpdated: vi.fn(() => () => {}),
   }));
   ```

4. **测试同步**：即 3.4/3.5/3.6。
5. **文档同步**：`src/ipc/CLAUDE.md` planBalance 命令段改写 + 新增 backgroundTasks 命令段（DOC-01）；`src/__tests__/CLAUDE.md` 全局 mock 清单节追加 `../ipc/backgroundTasks` 行（DOC-01）。
6. **验证**：`npm test -- ipc-background-tasks ipc-plan-balance` 全绿；grep `setPlanBalanceInterval` 在 src/ 零命中；`background_tasks_set_config` 仅出现于 ipc/backgroundTasks.ts。

## FE-03 前端调度器（features/backgroundTasks 模块 + 测试）

1. **位置**：新建 `src/features/backgroundTasks/`：`types.ts`、`scheduler.ts`、`sessionRefreshTask.ts`、`tasks.ts`、`index.ts`；新建 `src/__tests__/background-tasks-scheduler.test.ts`、`src/__tests__/background-tasks-session-refresh.test.ts`。
2. **现状**：无此模块。注册表家族契约（硬约束 #13）先例：SettingsPageRegistry / cliProfileRegistry。
3. **修复步骤**：
   - 3.1 `types.ts`（照抄）：

   ```ts
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
   ```

   - 3.2 `scheduler.ts`（照抄——语义逐行即规格 FR-1）：

   ```ts
   // scheduler.ts —— 后台定时任务调度器（F12，注册表家族契约 #13 模块级单例）
   //
   // 生命周期：首个订阅者出现 → 读配置（background_tasks_list）→ enabled 则立即执行
   // 一轮（接管「挂载即扫」语义）+ 启动 interval；最后订阅者退订 → 停 interval
   // （无订阅者不空转）。tick 防重入：上一轮未结束跳过本 tick；triggerNow 与 tick
   // 互斥同一闸门。失败处理（规格 §7）：tick 失败静默（快照不变）；manual 失败置 error
   // 态（保留旧 data）。applyConfig 运行期改配立即生效（设置页直调）。

   import { listBackgroundTasks } from "../../ipc/backgroundTasks";
   import type { BackgroundTaskDef, TaskSnapshot, TriggerSource } from "./types";

   interface TaskRuntime {
     def: BackgroundTaskDef;
     listeners: Set<(snapshot: TaskSnapshot) => void>;
     timer: ReturnType<typeof setInterval> | null;
     /** 防重入闸门（tick 与 manual 共用） */
     running: boolean;
     enabled: boolean;
     intervalSec: number;
     /** 配置是否已成功从后端读取（list 失败 → 首轮仍执行但不启动 interval） */
     configReady: boolean;
     snapshot: TaskSnapshot;
   }

   class BackgroundTaskScheduler {
     private tasks = new Map<string, TaskRuntime>();

     /** 注册任务（同 id 覆盖旧条目——运行时状态随条目重建清零） */
     register<T>(def: BackgroundTaskDef<T>): void {
       this.tasks.set(def.id, {
         def: def as BackgroundTaskDef,
         listeners: new Set(),
         timer: null,
         running: false,
         enabled: true,
         intervalSec: 0,
         configReady: false,
         snapshot: { state: "idle", data: undefined },
       });
     }

     /** 全部任务定义，按注册序 */
     getAll(): BackgroundTaskDef[] {
       return [...this.tasks.values()].map((t) => t.def);
     }

     /** 清空全部任务（仅测试用——停全部 timer） */
     _reset(): void {
       for (const rt of this.tasks.values()) this.stopTimer(rt);
       this.tasks.clear();
     }

     /** 订阅任务快照：立即回调当前快照；首个订阅者触发激活（读配置 → 立即一轮 + interval） */
     subscribe<T>(id: string, listener: (snapshot: TaskSnapshot<T>) => void): () => void {
       const rt = this.tasks.get(id);
       if (!rt) {
         console.error(`[slTerminal] 后台任务未注册: ${id}`);
         return () => {};
       }
       const l = listener as (snapshot: TaskSnapshot) => void;
       rt.listeners.add(l);
       l(rt.snapshot);
       if (rt.listeners.size === 1) this.activate(rt);
       return () => {
         rt.listeners.delete(l);
         if (rt.listeners.size === 0) this.stopTimer(rt); // 最后退订停 interval（在途轮继续完成）
       };
     }

     /** 手动触发（刷新钮）：与 tick 共用同一执行体与防重入闸门，仅 source 不同 */
     async triggerNow(id: string): Promise<void> {
       const rt = this.tasks.get(id);
       if (!rt) return;
       await this.runOnce(rt, "manual");
     }

     /** 运行期改配（设置页 set_config 成功后直调）：启停/改频率立即生效 */
     applyConfig(id: string, cfg: { enabled: boolean; intervalSec: number }): void {
       const rt = this.tasks.get(id);
       if (!rt) return;
       const wasRunningTimer = rt.timer !== null;
       rt.enabled = cfg.enabled;
       rt.intervalSec = cfg.intervalSec;
       rt.configReady = true;
       if (!cfg.enabled) {
         this.stopTimer(rt);
         return;
       }
       if (rt.listeners.size === 0) return; // 无订阅者不空转（配置已记，订阅时生效）
       if (!wasRunningTimer) void this.runOnce(rt, "tick"); // 禁用→启用：立即一轮
       this.restartTimer(rt);
     }

     /** 本地变更透传（removeLocal 语义：删除会话后本地移除列表项不重扫） */
     applyLocal<T>(id: string, updater: (prev: T | undefined) => T): void {
       const rt = this.tasks.get(id);
       if (!rt) return;
       rt.snapshot = { ...rt.snapshot, data: updater(rt.snapshot.data as T | undefined) };
       this.broadcast(rt);
     }

     /** 首个订阅者激活：配置未读 → 先读配置再启动；已读 → 直接启动（切回立即一轮） */
     private activate(rt: TaskRuntime): void {
       if (rt.configReady) {
         this.startIfEnabled(rt);
         return;
       }
       void (async () => {
         try {
           const list = await listBackgroundTasks();
           const cfg = list.find((t) => t.taskId === rt.def.id);
           if (cfg) {
             rt.enabled = cfg.enabled;
             rt.intervalSec = cfg.intervalSec;
           }
           rt.configReady = true;
         } catch (e) {
           // 配置读取失败：保住首轮执行（数据可见），不启动 interval（无元数据第二来源）
           console.error(`[slTerminal] 后台任务配置读取失败（${rt.def.id}），仅执行首轮不启动定时`, e);
         }
         this.startIfEnabled(rt);
       })();
     }

     private startIfEnabled(rt: TaskRuntime): void {
       if (!rt.enabled) return; // 禁用：不执行首轮不启动定时
       void this.runOnce(rt, "tick");
       if (rt.intervalSec > 0 && rt.timer === null) this.startTimer(rt);
     }

     private startTimer(rt: TaskRuntime): void {
       rt.timer = setInterval(() => void this.runOnce(rt, "tick"), rt.intervalSec * 1000);
     }

     private stopTimer(rt: TaskRuntime): void {
       if (rt.timer !== null) {
         clearInterval(rt.timer);
         rt.timer = null;
       }
     }

     private restartTimer(rt: TaskRuntime): void {
       this.stopTimer(rt);
       if (rt.enabled && rt.intervalSec > 0) this.startTimer(rt);
     }

     /** 执行一轮（防重入：进行中直接返回）。状态机：开始 loading → 成功 ready /
         tick 失败静默（快照不变）/ manual 失败 error（保留旧 data） */
     private async runOnce(rt: TaskRuntime, source: TriggerSource): Promise<void> {
       if (rt.running) return;
       rt.running = true;
       rt.snapshot = { ...rt.snapshot, state: "loading" };
       this.broadcast(rt);
       try {
         const data = await rt.def.run(source, rt.snapshot.data);
         rt.snapshot = { state: "ready", data };
         this.broadcast(rt);
       } catch (e) {
         if (source === "manual") {
           rt.snapshot = { ...rt.snapshot, state: "error" };
           this.broadcast(rt);
         }
         console.error(`[slTerminal] 后台任务执行失败（${rt.def.id}, ${source}）:`, e);
       } finally {
         rt.running = false;
       }
     }

     private broadcast(rt: TaskRuntime): void {
       for (const l of rt.listeners) l(rt.snapshot);
     }
   }

   /** 模块级单例 */
   export const backgroundTaskScheduler = new BackgroundTaskScheduler();
   ```

   - 3.3 `sessionRefreshTask.ts`（照抄——规格 FR-4 扫描执行体）：

   ```ts
   // sessionRefreshTask.ts —— sessionRefresh 任务执行体（F12，规格 FR-4）
   //
   // 扫描执行体 = 历史会话扫描唯一执行路径：遍历 cliProfileRegistry 中声明 history
   // 能力的 profile 逐个 scanAgentHistory(cliId, true) 聚合为扁平列表（恒 force=true——
   // 后端 (目录 mtime, 文件数) 缓存对进行中会话不敏感，手动与定时同，规格 §8）。
   // 多 provider 失败隔离：单 provider 失败 → 该 provider 保留旧数据、其余采用新值；
   // 全部失败 → throw（调度器按触发来源走规格 §7）。

   import { cliProfileRegistry } from "../cliProfiles/cliProfileRegistry";
   import { scanAgentHistory } from "../../ipc/agentHistory";
   import { SESSION_REFRESH_TASK_ID } from "../../types/backgroundTasks";
   import type { AgentHistorySession } from "../../types/agentHistory";
   import type { TriggerSource } from "./types";
   import { backgroundTaskScheduler } from "./scheduler";

   async function runSessionRefresh(
     _source: TriggerSource,
     prev: AgentHistorySession[] | undefined,
   ): Promise<AgentHistorySession[]> {
     const prevSessions = prev ?? [];
     const profiles = cliProfileRegistry
       .getAll()
       .filter((p) => p.capabilities.history !== undefined);
     const results = await Promise.allSettled(
       profiles.map((p) => scanAgentHistory(p.id, true)),
     );
     const merged: AgentHistorySession[] = [];
     let failed = 0;
     results.forEach((r, i) => {
       const cliId = profiles[i].id;
       if (r.status === "fulfilled") {
         merged.push(...(Array.isArray(r.value) ? r.value : []));
       } else {
         failed++;
         console.error(`[slTerminal] 历史扫描失败（${cliId}）:`, r.reason);
         // 失败 provider 保留旧数据（按 cliId 过滤 prev）
         merged.push(...prevSessions.filter((s) => s.cliId === cliId));
       }
     });
     if (failed > 0 && failed === results.length) {
       throw new Error(`全部 history provider 扫描失败（${failed} 个）`);
     }
     return merged;
   }

   backgroundTaskScheduler.register({
     id: SESSION_REFRESH_TASK_ID,
     run: runSessionRefresh,
   });
   ```

   - 3.4 `tasks.ts`（注册触发点集中文件）：

   ```ts
   // tasks.ts —— 后台任务注册触发点（硬约束 #13 side-effect import）：
   // 消费方（useAgentHistory / BackgroundTasksPage）import 本文件即完成全部任务注册，
   // 禁止隐式初始化；新增任务 = 下方追加一条 import。
   import "./sessionRefreshTask";
   ```

   - 3.5 `index.ts`（barrel，不触发注册）：

   ```ts
   // backgroundTasks barrel —— 公共 API 出口（不触发任务注册；注册触发点在 ./tasks.ts）
   export { backgroundTaskScheduler } from "./scheduler";
   export type { BackgroundTaskDef, TaskRunState, TaskSnapshot, TriggerSource } from "./types";
   ```

   - 3.6 新建 `src/__tests__/background-tasks-scheduler.test.ts`：
     - mock `../ipc/backgroundTasks`（listBackgroundTasks hoisted 可控）；
     - 每用例 beforeEach `backgroundTaskScheduler._reset()`；
     - 注册表契约：register/getAll 注册序/同 id 覆盖/_reset 清空；
     - subscribe 立即回调当前快照（idle）；首个订阅者 → listBackgroundTasks 被调 + 立即执行一轮（enabled=true）；enabled=false 配置 → 不执行不启动定时；list 失败 → 仍执行首轮 + console.error + 不启动 interval；
     - 订阅者计数启停：fake timers 断言 interval 按 intervalSec 触发；全部退订 → advanceTimers 不再触发；重订阅 → 立即一轮（不重复读配置——listBackgroundTasks 仍 1 次）；
     - tick 防重入：run 挂起时 tick/manual 均被闸门跳过（run 调用次数不增）；
     - 失败处理：tick 失败 → 快照不变（state 保持 ready，data 保留）+ console.error；manual 失败 → state=error + data 保留；
     - applyConfig：运行期改频率 → timer 重启（新间隔生效）；禁用 → 停 timer；启用 → 立即一轮 + 启动 timer；无订阅者时 applyConfig 不启动；
     - applyLocal：updater 变换 data + 广播（state 不变）。
   - 3.7 新建 `src/__tests__/background-tasks-session-refresh.test.ts`：
     - mock `../ipc/agentHistory`（scanAgentHistory hoisted 可控）+ mock `../ipc/backgroundTasks`（listBackgroundTasks 恒返回 sessionRefresh 配置，防 activate 真实 invoke）；
     - beforeEach：`backgroundTaskScheduler._reset(); cliProfileRegistry._reset();` + 文件顶部 `import "../features/backgroundTasks/tasks";` + 注册桩 profile（history 能力有无两种）；
     - 遍历聚合：两 history profile → scanAgentHistory 各调一次（force=true）→ 聚合扁平列表；
     - 无 history 能力 profile 被跳过（不调 scanAgentHistory）；
     - 部分失败隔离：provider A resolve 新值、B reject → 结果 = A 新值 + B 旧值（先建立 prev：首轮全成功 → 第二轮 B 失败）；
     - 全部失败：tick → 快照不变；manual → state=error；
     - force 恒 true：断言每次调用第二参 true。
4. **测试同步**：即 3.6/3.7。
5. **文档同步**：新建 `src/features/backgroundTasks/CLAUDE.md`（含注册触发点登记：useAgentHistory.ts 与 BackgroundTasksPage.tsx 顶部 import ./tasks）（DOC-01）。
6. **验证**：`npm test -- background-tasks` 全绿；grep `backgroundTaskScheduler` 命中 scheduler.ts + tasks.ts；`npx tsc --noEmit` 通过。

## FE-04 useAgentHistory 订阅改造（scan 退役 → triggerNow）

1. **位置**：`src/features/agentHistory/useAgentHistory.ts`；重写 `src/__tests__/agent-history-hook.test.tsx`。
2. **现状**（实读 :55-69）：`scan(force?)` 调 `scanAgentHistory(CLAUDE_CLI_ID, force)` + genRef 防竞 + setState 直管 sessions/state；返回面 `{ state, sessions, activeStatuses, rootPath, scan, removeLocal }`。
3. **修复步骤**——重写 useAgentHistory.ts（照抄）：

   ```ts
   // useAgentHistory.ts — agent 历史会话数据 hook（FE-04；F12 订阅化改造）
   //
   // 返回形状契约（跨 Stage 契约写死）：
   //   { state, sessions, activeStatuses, rootPath, triggerNow, removeLocal }
   //
   // 设计要点：
   // - sessions/state 真值源上移 backgroundTaskScheduler（F12）：本 hook 订阅
   //   sessionRefresh 任务快照（状态机 idle|loading|ready|error 语义不变）；
   //   首个订阅者出现 → 立即执行一轮（接管「挂载即扫」语义）+ 按配置频率定时刷新；
   //   最后订阅者退订 → 停 interval（调度器全局单例与 UI 解耦，NavTree 卸载无碍，ADR-0001）
   // - triggerNow() = 手动刷新（刷新钮）——与 tick 共用同一扫描执行体（规格 §1 单一执行体）
   // - removeLocal 经调度器 applyLocal 透传（删除会话后本地移除列表项不重扫）
   // - activeStatuses 实时跟随 TerminalRegistry（register/remove/sessionChange），
   //   不重扫；rootPath 推导保持 hook 本地不变（activePageId → 所属 project）

   import { useState, useEffect, useCallback } from "react";
   import { backgroundTaskScheduler } from "../backgroundTasks";
   import "../backgroundTasks/tasks"; // side-effect：任务注册触发点之一（硬约束 #13）
   import type { TaskSnapshot } from "../backgroundTasks";
   import { SESSION_REFRESH_TASK_ID } from "../../types/backgroundTasks";
   import { useProjects } from "../../stores/projects";
   import { useLayout } from "../../stores/layout";
   import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
   import { deriveActiveSessionStatuses } from "./historyModel";
   import type { AgentHistorySession } from "../../types/agentHistory";
   import type { AgentStatus } from "../../lib/agentStatus";

   /** 加载状态机：idle 初始未扫描 / loading 扫描中 / ready 成功 / error 失败 */
   export type AgentHistoryState = "idle" | "loading" | "ready" | "error";

   export function useAgentHistory() {
     const projects = useProjects((s) => s.projects);
     const activePageId = useLayout((s) => s.activePageId);

     // rootPath 推导：activePageId → 所属 project（照 useCommitStatus 先例）
     let rootPath: string | null = null;
     if (activePageId) {
       for (const [, proj] of Object.entries(projects)) {
         const activePage = proj.pages.find((p) => p.pageId === activePageId);
         if (activePage) {
           rootPath = activePage.cwd || proj.rootPath;
           break;
         }
       }
     }

     // 订阅调度器快照（首个订阅者 → 立即执行一轮 + 启动定时刷新；卸载退订）
     const [snapshot, setSnapshot] = useState<TaskSnapshot<AgentHistorySession[]>>({
       state: "idle",
       data: undefined,
     });
     useEffect(
       () =>
         backgroundTaskScheduler.subscribe<AgentHistorySession[]>(
           SESSION_REFRESH_TASK_ID,
           setSnapshot,
         ),
       [],
     );

     const [activeStatuses, setActiveStatuses] = useState<Map<string, AgentStatus>>(
       () => deriveActiveSessionStatuses(),
     );

     /** 手动刷新（刷新钮）——与 tick 同一执行体，仅触发来源不同（manual 失败置 error 态） */
     const triggerNow = useCallback(() => {
       void backgroundTaskScheduler.triggerNow(SESSION_REFRESH_TASK_ID);
     }, []);

     /** 局部删除：调用方已执行删除 IPC，此处仅即时移除列表项（不重扫） */
     const removeLocal = useCallback((sessionId: string) => {
       backgroundTaskScheduler.applyLocal<AgentHistorySession[]>(
         SESSION_REFRESH_TASK_ID,
         (prev) => (prev ?? []).filter((s) => s.sessionId !== sessionId),
       );
     }, []);

     // 订阅 TerminalRegistry：register/remove/sessionChange 任一事件 → 重算四态映射
     useEffect(() => {
       const unsubscribe = TerminalRegistry.subscribe(() => {
         setActiveStatuses(deriveActiveSessionStatuses());
       });
       return unsubscribe;
     }, []);

     return {
       state: snapshot.state,
       sessions: snapshot.data ?? [],
       activeStatuses,
       rootPath,
       triggerNow,
       removeLocal,
     };
   }
   ```

   （删除：scanAgentHistory/CLAUDE_CLI_ID import、genRef、scan；useRef import 删除。）
4. **测试同步**：`agent-history-hook.test.tsx` 重写——mock 面：`../ipc/agentHistory`（scanAgentHistory）+ `../ipc/backgroundTasks`（listBackgroundTasks 返回 `[{ taskId: "sessionRefresh", title: "会话历史刷新", enabled: true, intervalSec: 300, intervalMin: 2, intervalMax: 300 }]`，setBackgroundTaskConfig/onBackgroundTasksUpdated 防御 stub）+ TerminalRegistry（同现状）；beforeEach `backgroundTaskScheduler._reset(); cliProfileRegistry._reset(); cliProfileRegistry.register(claudeProfile);`（claudeProfile 自 `../features/cliProfiles/profiles/claude` 导入）；文件顶部 `import "../features/backgroundTasks/tasks";`。用例：初始 idle / 订阅后自动执行一轮 ready+sessions（挂载即扫语义，断言 scanAgentHistory("claude", true)）/ manual 失败 → error（triggerNow 后 scanAgentHistory reject）/ removeLocal 不重扫 / activeStatuses 订阅（保留现效用例）/ rootPath 推导（保留）。
5. **文档同步**：`src/features/agentHistory/CLAUDE.md`「数据流与刷新时机」节改写（DOC-01）。
6. **验证**：`npm test -- agent-history-hook` 全绿；grep `\.scan(` 在 src/features/ 零命中（historyModel 等无关处除外则 Read 确认）；useAgentHistory 返回面无 `scan` 有 `triggerNow`。

## FE-05 useNavTree 适配 + nav-tree 测试适配

1. **位置**：`src/features/navTree/useNavTree.ts`；`src/__tests__/nav-tree.test.tsx`、`src/__tests__/nav-tree-history.test.tsx`。
2. **现状**（实读）：useNavTree.ts:198-200 挂载即扫 `useEffect(() => { void history.scan(); }, [history.scan])`；:262-264 `refresh = useCallback(() => { void history.scan(true); }, [history.scan])`；接口注释 :86-89。测试现状：nav-tree.test.tsx:53-55 与 nav-tree-history.test.tsx:32-34 mock `../ipc/agentHistory` 的 scanAgentHistory（真实 useAgentHistory 链路）；nav-tree-history.test.tsx:481-540 FE-19 两例（挂载即扫一次/展开不重复/刷新钮重扫）。
3. **修复步骤**：
   - 3.1 useNavTree.ts：删挂载即扫 useEffect（:198-200——订阅语义接管，首个订阅者立即一轮）；refresh 改：

   ```ts
   // 刷新钮 = 手动触发（与定时 tick 同一扫描执行体；force=true 在执行体内恒定，
   // 绕过后端缓存——空结果永久命中场景必须 bypass）
   refresh: useCallback(() => {
     history.triggerNow();
   }, [history.triggerNow]),
   ```

   接口注释 :86-89 同步改写；`useEffect` import 若无其他消费则删。返回面字段名 `refresh` 不变（NavTree.tsx:598 零改动）。
   - 3.2 nav-tree.test.tsx / nav-tree-history.test.tsx 适配：
     - 追加 mock `../ipc/backgroundTasks`（listBackgroundTasks 返回 sessionRefresh enabled=true intervalSec=300 配置——大间隔防 tick 干扰断言；setBackgroundTaskConfig/onBackgroundTasksUpdated 防御 stub）；
     - beforeEach 增加 `backgroundTaskScheduler._reset(); cliProfileRegistry._reset(); cliProfileRegistry.register(claudeProfile);`（import 自 `../features/backgroundTasks` 与 `../features/cliProfiles/profiles/claude`），文件顶部 `import "../features/backgroundTasks/tasks";`；
     - 「挂载即 scan」类断言语义保留（订阅首轮即调 scanAgentHistory）——断言调用参数由 `("claude", undefined)`/`("claude")` 变 `("claude", true)`（force 恒 true）；
     - nav-tree-history.test.tsx FE-19 两例保留语义：「挂载即扫一次」（订阅首轮）/「展开不重复 scan」/「刷新钮显式重扫」改断言点击刷新钮后 scanAgentHistory 第二次调用且 force=true——用例名与注释更新为 triggerNow/定时刷新语义。
4. **测试同步**：即 3.2（两文件适配）；用例计数变化登记 test-inventory（DOC-02）。
5. **文档同步**：`src/features/navTree/CLAUDE.md` FE-19 节改写（DOC-01）。
6. **验证**：`npm test -- nav-tree` 全绿；grep `void history.scan` 零命中 useNavTree.ts；refresh 钮链路 = nav.refresh → history.triggerNow（Read 确认）。

## FE-06 footer enabled 感知（usePlanBalance + PlanBalanceFooter）

1. **位置**：`src/features/navTree/usePlanBalance.ts`、`src/features/navTree/PlanBalanceFooter.tsx`；`src/__tests__/plan-balance-footer.test.tsx`。
2. **现状**（实读）：usePlanBalance.ts 全文 36 行（挂载 getPlanBalance + onPlanBalanceUpdated 订阅 + 5s 节流 refresh）；PlanBalanceFooter.tsx:45 `if (items.length === 0) return null;`；`../ipc/backgroundTasks` 全局 mock 已在 FE-02 加入 setup.ts。
3. **修复步骤**：
   - 3.1 usePlanBalance.ts：
     - import 追加 `import { listBackgroundTasks, onBackgroundTasksUpdated } from "../../ipc/backgroundTasks";` 与 `import { PLAN_BALANCE_TASK_ID } from "../../types/backgroundTasks";`
     - 新增 state：`const [enabled, setEnabled] = useState<boolean | null>(null);`（null = 配置未加载——footer 不渲染，防「先渲染后隐藏」闪烁）。
     - 挂载 effect 内追加（与首拉并行）：

     ```ts
     listBackgroundTasks()
       .then((list) => {
         if (cancelled) return;
         setEnabled(
           list.find((t) => t.taskId === PLAN_BALANCE_TASK_ID)?.enabled ?? true,
         );
       })
       .catch((e) => {
         // 配置读取失败回退启用（宁可显示，与现行行为一致）
         console.error("background_tasks_list 读取失败，按启用处理", e);
         if (!cancelled) setEnabled(true);
       });
     const unlistenConfig = onBackgroundTasksUpdated((list) => {
       setEnabled(
         list.find((t) => t.taskId === PLAN_BALANCE_TASK_ID)?.enabled ?? true,
       );
     });
     ```

     - cleanup 改 `return () => { cancelled = true; unlisten(); unlistenConfig(); };`；返回面改 `return { items, refresh, enabled };`。
     - 头注释更新（F12：enabled 感知通道 = list 读 + background-tasks-updated 事件订阅）。
   - 3.2 PlanBalanceFooter.tsx:44-45 改：

   ```tsx
   const { items, refresh, enabled } = usePlanBalance();
   // 禁用即整块不渲染（F12 enabled 语义：禁用即不关注，快照保留——重启用即重显最后快照）
   if (enabled !== true || items.length === 0) return null;
   ```

   - 3.3 plan-balance-footer.test.tsx：vi.mock 追加 `../ipc/backgroundTasks`（hoisted：listBackgroundTasks 默认 resolve 含 planBalance enabled=true 清单，onBackgroundTasksUpdated 捕获回调供 `triggerConfigUpdate` 测试辅助——照 triggerUpdate 先例）；新增用例：
     - `enabled=false → 整块不渲染（有快照也隐藏）`（list 返回 enabled=false + getPlanBalance 返回行数据）；
     - `事件推送 enabled=false → 已渲染 footer 隐藏；再推 enabled=true → 重显最后快照`（triggerConfigUpdate 两连发）；
     - `list 失败 → 按启用处理（footer 正常渲染）`（reject + console.error spy）。
4. **测试同步**：即 3.3；`src/__tests__/CLAUDE.md` 全局 mock 清单节同步（DOC-01，mock 本体在 FE-02）。
5. **文档同步**：`src/features/navTree/CLAUDE.md` 余量 footer 行改写（DOC-01）。
6. **验证**：`npm test -- plan-balance-footer` 全绿；grep `PLAN_BALANCE_TASK_ID` 命中 usePlanBalance.ts；`enabled !== true` 守卫存在于 PlanBalanceFooter.tsx；setup.ts 全局 mock 生效（npm test 全量绿即证无文件真实 invoke）。

## FE-07 设置中心「后台定时任务」页替换

1. **位置**：`src/features/settingsCenter/pages.ts`；新建 `src/panels/settings/pages/BackgroundTasksPage.tsx`；删除 `src/panels/settings/pages/PlanBalancePage.tsx`；新建 `src/__tests__/settings-background-tasks.test.tsx`；删除 `src/__tests__/settings-plan-balance.test.tsx`；改 `src/__tests__/settings-pages-registration.test.ts`。
2. **现状**（实读）：pages.ts:9 import PlanBalancePage、:22-28 注册 planBalance 页（global order 20）；PlanBalancePage.tsx 153 行（单频率输入页，失焦/回车提交，非法行内红字，Err toast + 保留输入）。
3. **修复步骤**：
   - 3.1 pages.ts：删 :9 import PlanBalancePage 与 :22-28 注册段，替换为：

   ```ts
   import BackgroundTasksPage from "../../panels/settings/pages/BackgroundTasksPage";

   // 后台定时任务页（F12：套餐余量/会话刷新统一配置）——global 组（应用级单例）
   getSettingsPageRegistry().register({
     id: "backgroundTasks",
     title: "后台定时任务",
     group: "global",
     component: BackgroundTasksPage,
     order: 20,
   });
   ```

   pages.ts 头注释「Stage 02 仅注册 planBalance 一页」过时表述同步改写为现行三页口径。
   - 3.2 新建 BackgroundTasksPage.tsx（照抄）：

   ```tsx
   // BackgroundTasksPage — 设置中心「后台定时任务」配置页（F12）
   //
   // 读取：挂载经 background_tasks_list() 拿任务清单 + 生效配置（通用行组件纯渲染——
   //       新增任务自动出现，页零改动）。
   // 提交（立即提交型，无 dirty 暂存，照 PlanBalancePage 先例）：
   //   勾选切换立即提交 set_config(taskId, {enabled})；
   //   频率失焦/回车提交，非法（非数/非整数/越界）→ 行内红字提示，不提交不 toast；
   //   后端拒绝 → toast + 保留用户输入。
   // 生效闭环：set_config 返回完整清单 → 更新行；sessionRefresh 直调调度器 applyConfig
   //   即时生效；planBalance 由后端内存值即时生效（footer 经 background-tasks-updated
   //   事件感知），并调一次 refreshPlanBalance() 拉取最新余量（照 F11 反馈闭环先例）。

   import React, { useCallback, useEffect, useState } from "react";
   import {
     listBackgroundTasks,
     setBackgroundTaskConfig,
   } from "../../../ipc/backgroundTasks";
   import { refreshPlanBalance } from "../../../ipc/planBalance";
   import {
     PLAN_BALANCE_TASK_ID,
     SESSION_REFRESH_TASK_ID,
     type BackgroundTaskInfo,
   } from "../../../types/backgroundTasks";
   import { backgroundTaskScheduler } from "../../../features/backgroundTasks";
   import "../../../features/backgroundTasks/tasks"; // side-effect：applyConfig 目标注册保障
   import { toast, getErrorMessage } from "../../../lib";
   import {
     PANEL_BG,
     SIDEBAR_FG,
     DIM_FG,
     INPUT_BG,
     INPUT_BORDER,
     FOCUS_BORDER,
     ERROR_FG,
   } from "../../../theme";
   import type { SettingsPageProps } from "../../../features/settingsCenter/types";

   /** 行内非法提示（范围提示——DTO 无 default 字段，不写默认值） */
   function rangeHint(task: BackgroundTaskInfo): string {
     return `${task.intervalMin}–${task.intervalMax} 秒`;
   }

   /** 通用任务行（纯渲染——新增任务自动出现） */
   function TaskRow(props: {
     task: BackgroundTaskInfo;
     input: string;
     error: string | null;
     onToggle(enabled: boolean): void;
     onInput(v: string): void;
     onCommitInterval(): void;
   }) {
     const { task, input, error, onToggle, onInput, onCommitInterval } = props;
     const inputId = `settings-background-tasks-interval-${task.taskId}`;
     return (
       <div data-e2e={`settings-background-tasks-row-${task.taskId}`} style={{ marginBottom: 16 }}>
         <div
           style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: SIDEBAR_FG }}
         >
           <input
             type="checkbox"
             checked={task.enabled}
             data-e2e={`settings-background-tasks-enabled-${task.taskId}`}
             onChange={(e) => onToggle(e.target.checked)}
           />
           <label htmlFor={inputId}>{task.title}频率（秒）</label>
           <input
             id={inputId}
             type="text"
             inputMode="numeric"
             value={input}
             data-e2e={`settings-background-tasks-interval-${task.taskId}`}
             onChange={(e) => onInput(e.target.value)}
             onFocus={(e) => {
               e.currentTarget.style.borderColor = FOCUS_BORDER;
             }}
             onBlur={(e) => {
               e.currentTarget.style.borderColor = INPUT_BORDER;
               onCommitInterval();
             }}
             onKeyDown={(e) => {
               if (e.key === "Enter") onCommitInterval();
             }}
             style={{
               width: 90,
               padding: "4px 8px",
               fontSize: 13,
               color: SIDEBAR_FG,
               background: INPUT_BG,
               border: `1px solid ${INPUT_BORDER}`,
               borderRadius: 4,
               outline: "none",
             }}
           />
           <span style={{ fontSize: 12, color: DIM_FG }}>{rangeHint(task)}</span>
         </div>
         {error && (
           <div
             data-e2e={`settings-background-tasks-error-${task.taskId}`}
             style={{ marginTop: 8, fontSize: 12, color: ERROR_FG }}
           >
             {error}
           </div>
         )}
       </div>
     );
   }

   const BackgroundTasksPage: React.FC<SettingsPageProps> = () => {
     /** null = 加载中（行区空态） */
     const [tasks, setTasks] = useState<BackgroundTaskInfo[] | null>(null);
     const [inputs, setInputs] = useState<Record<string, string>>({});
     const [errors, setErrors] = useState<Record<string, string | null>>({});

     useEffect(() => {
       let mounted = true;
       listBackgroundTasks()
         .then((list) => {
           if (!mounted) return;
           setTasks(list);
           setInputs(Object.fromEntries(list.map((t) => [t.taskId, String(t.intervalSec)])));
         })
         .catch((e) => {
           console.error("加载后台任务清单失败", e);
           if (mounted) setTasks([]);
         });
       return () => {
         mounted = false;
       };
     }, []);

     /** 提交成功公共收尾：返回清单更新行 + 前端调度器即时生效 + planBalance 反馈闭环 */
     const afterCommitted = useCallback((list: BackgroundTaskInfo[]) => {
       setTasks(list);
       for (const t of list) {
         if (t.taskId === SESSION_REFRESH_TASK_ID) {
           backgroundTaskScheduler.applyConfig(t.taskId, {
             enabled: t.enabled,
             intervalSec: t.intervalSec,
           });
         }
       }
       if (list.some((t) => t.taskId === PLAN_BALANCE_TASK_ID)) {
         refreshPlanBalance().catch((e) => console.error("刷新套餐余量失败", e));
       }
     }, []);

     /** 勾选切换立即提交（不做乐观更新——本地命令往返快，失败时 UI 保持原值） */
     const handleToggle = useCallback(
       (task: BackgroundTaskInfo, enabled: boolean) => {
         setBackgroundTaskConfig(task.taskId, { enabled })
           .then(afterCommitted)
           .catch((e) => toast.show("warning", `设置失败：${getErrorMessage(e)}`));
       },
       [afterCommitted],
     );

     /** 频率失焦/回车提交：非法 → 行内红字不提交不 toast；合法 → 命令 + 规范化回显 */
     const handleCommitInterval = useCallback(
       (task: BackgroundTaskInfo) => {
         const trimmed = (inputs[task.taskId] ?? "").trim();
         const v = Number(trimmed);
         if (
           trimmed === "" ||
           !Number.isFinite(v) ||
           !Number.isInteger(v) ||
           v < task.intervalMin ||
           v > task.intervalMax
         ) {
           setErrors((prev) => ({ ...prev, [task.taskId]: rangeHint(task) }));
           return;
         }
         setErrors((prev) => ({ ...prev, [task.taskId]: null }));
         setBackgroundTaskConfig(task.taskId, { intervalSec: v })
           .then((list) => {
             afterCommitted(list);
             setInputs((prev) => ({ ...prev, [task.taskId]: String(v) }));
           })
           .catch((e) => toast.show("warning", `设置失败：${getErrorMessage(e)}`));
       },
       [inputs, afterCommitted],
     );

     return (
       <div style={{ width: "100%", height: "100%", background: PANEL_BG }} data-e2e="settings-background-tasks-page">
         <div style={{ padding: "16px 20px" }}>
           {(tasks ?? []).map((task) => (
             <TaskRow
               key={task.taskId}
               task={task}
               input={inputs[task.taskId] ?? ""}
               error={errors[task.taskId] ?? null}
               onToggle={(enabled) => handleToggle(task, enabled)}
               onInput={(v) => setInputs((prev) => ({ ...prev, [task.taskId]: v }))}
               onCommitInterval={() => handleCommitInterval(task)}
             />
           ))}
         </div>
       </div>
     );
   };

   export default BackgroundTasksPage;
   ```

   - 3.3 删除 PlanBalancePage.tsx 与 settings-plan-balance.test.tsx（git rm）。
   - 3.4 settings-pages-registration.test.ts：mock 路径 `../panels/settings/pages/PlanBalancePage` 改 `../panels/settings/pages/BackgroundTasksPage`；断言数组 `{ id: "planBalance", group: "global", order: 20 }` 改 `{ id: "backgroundTasks", group: "global", order: 20 }`。
   - 3.5 新建 settings-background-tasks.test.tsx（照 settings-plan-balance.test.tsx 模式）：
     - mock `../ipc/backgroundTasks`（list/set hoisted 可控）+ `../ipc/planBalance`（refreshPlanBalance）+ `../lib` 仅替换 toast + mock `../features/backgroundTasks`（backgroundTaskScheduler.applyConfig hoisted spy——避免真实调度器；`../features/backgroundTasks/tasks` side-effect 因 barrel mock 不触达，无需处理）；
     - 用例：挂载渲染两行（任务标题 + 勾选态 + 频率输入回显 intervalSec + 范围提示文案「10–3600 秒」「2–300 秒」）；list 失败 → 空态不崩 + console.error；勾选 planBalance → setBackgroundTaskConfig("planBalance", { enabled: false }) + 成功后行勾选态更新（用返回清单）+ refreshPlanBalance 被调；勾选 sessionRefresh → applyConfig 被调且参数为返回清单新值；频率非法（非数/小数/越界/空串）→ 行内红字 = 范围文案 + 不提交不 toast + 输入保留；频率合法 → setBackgroundTaskConfig + 规范化回显；set reject → toast warning + 输入保留。
4. **测试同步**：即 3.4/3.5；registration 断言计数不变（3 条）。
5. **文档同步**：`src/ipc/CLAUDE.md` 命令段更新（DOC-01）；settingsCenter/panels CLAUDE.md 零改动（只记机制不列页，实读确认）。
6. **验证**：`npm test -- settings-background-tasks settings-pages-registration` 全绿；grep `PlanBalancePage` 在 src/ 零命中；grep `settings-plan-balance` 在 src/ 零命中（e2e-tests 由 E2E-01 处理，本项断言限 src/）；pages.ts 注册三条含 backgroundTasks order 20。

## FE-08 深链失配兜底（验证项）

1. **位置**：`src/panels/settings/SettingsPanel.tsx:181-185`。
2. **现状**（实读）：useState 初始化 `if (saved && registry.get(saved)) return saved; return registry.getAll("global")[0]?.id ?? null;`——旧布局持久化 `selectedPage: "planBalance"` 失配 → 回退 global 组第一页，**现状代码已满足**；settings-panel.test.tsx:144-148 已有「selectedPage 失效（注册表无此页）→ 回退全局组第一页」用例（stub 注册表 + "nonexistent" id，语义等价覆盖）。
3. **修复步骤**：零代码改动——纯验证项。
4. **测试同步**：既有用例保留即覆盖；不新增。
5. **文档同步**：无。
6. **验证**：`npm test -- settings-panel` 全绿（含既有回退用例）；Read SettingsPanel.tsx 确认回退逻辑仍在。

---

## E2E-01 settings.e2e.ts 既有用例适配

1. **位置**：`e2e-tests/settings.e2e.ts`。
2. **现状**（实读）：11 用例，涉 planBalance 页的选择器/页 id 分布——② :269 `settings-nav-planBalance` 断言；③ :330-338 `switchSettingsPage("planBalance")` + `settings-plan-balance-page`；④ :346-397 频率页 120 落盘闭环（`settings-plan-balance-input` + `root.planBalance.intervalSec === 120` + 假 env 注入 + 余量行断言）；⑤ :401-447 非法 5 红字（文案含「默认 60」）；⑧ :591-614 selectedPage 持久化（planBalance）；⑩ :702-708 切配置页目标 `settings-nav-planBalance`。
3. **修复步骤**：
   - 3.1 全文选择器/页 id 替换：`"planBalance"`（页 id 实参）→ `"backgroundTasks"`；`settings-nav-planBalance` → `settings-nav-backgroundTasks`；`settings-plan-balance-page` → `settings-background-tasks-page`；`settings-plan-balance-input` → `settings-background-tasks-interval-planBalance`；`settings-plan-balance-error` → `settings-background-tasks-error-planBalance`。
   - 3.2 用例④：落盘断言改 `root.backgroundTasks?.planBalance?.intervalSec === 120`（waitForSettingsFile 判定函数同步改）；注释「planBalance 段」改「backgroundTasks.planBalance 子键」；余量刷新闭环节（假 env 注入 + plan-balance-row 断言）保留不变。
   - 3.3 用例⑤：红字文案断言由 `10–3600 秒，默认 60` 改 `10–3600 秒`（DTO 无 default 字段，提示只写范围）。
   - 3.4 用例① :263 注释「order 10 < planBalance order 20」改 backgroundTasks 口径。
   - 3.5 用例⑧：两处 `selectedPage).toBe("planBalance")` 改 `"backgroundTasks"`。
4. **测试同步**：本项即测试改造；用例数不变（11）。
5. **文档同步**：无。
6. **验证**：grep `settings-plan-balance|settings-nav-planBalance` 在 e2e-tests/ 零命中；grep `"planBalance"` 在 settings.e2e.ts 仅出现于 taskId 上下文（`backgroundTasks.planBalance` 断言 / interval 选择器后缀），逐处 Read 确认无语义残留。

## E2E-02 background-tasks.e2e.ts 新建（页操作链路 + 勾选启停 footer 联动）

1. **位置**：新建 `e2e-tests/background-tasks.e2e.ts`；`e2e-tests/wdio.conf.ts` specs 登记（若 specs 为 glob 则免登记——执行 agent 先读 wdio.conf.ts 确认）。
2. **现状**：无此文件。settings.e2e.ts 已有可复用 helper 链（openSettingsCenter / switchSettingsPage / setInputValue / blurInput / waitForSettingsFile / writeFakePlanEnv / ensureNavOpen / clickConfigButton 等——执行 agent 先读 settings.e2e.ts 与 helpers.ts/specUtils.ts 提取复用，不另造轮子）。
3. **修复步骤**——新建 spec（用例清单，实现照 settings.e2e.ts 模式）：
   - 用例 A「页渲染与两行齐备」：配置钮 → 设置面板 → 切「后台定时任务」页 → 断言 `settings-background-tasks-row-planBalance` 与 `-row-sessionRefresh` 均存在、勾选默认 true、频率输入默认 10 / 3。
   - 用例 B「改频率端到端生效」：planBalance 行频率改 15 失焦 → waitForSettingsFile 断言 `backgroundTasks.planBalance.intervalSec === 15` → 输入框规范化回显 15、无红字。
   - 用例 C「勾选禁用 planBalance → footer 隐藏；重新启用 → footer 重显」：前置假 env 注入 + 手动刷新使 `plan-balance-row` 出现 → 取消勾选 → 断言 row 消失（事件驱动隐藏）+ 磁盘 `enabled === false` → 重新勾选 → row 重显（最后快照保留——不重拉也显）+ 磁盘 enabled 回 true。
   - 用例 D「非法频率行内红字不落盘」：sessionRefresh 行输 1（< 2）失焦 → 红字「2–300 秒」+ 磁盘无 sessionRefresh 子键（文件未变）。
   - suite 级快照还原 exe 同级 settings.json（照 settings.e2e.ts :216-221 先例）。
4. **测试同步**：本项即测试；登记 test-inventory（DOC-02）。wdio 实际执行由用户收尾人工跑（`npm run e2e` 全量或定向 spec——构建重，门禁不内嵌）。
5. **文档同步**：e2e-tests/CLAUDE.md 零改动（无新机制——helper 复用既有）。
6. **验证**：文件存在且四用例 describe/it 结构完整；grep `data-e2e` 选择器全部为新系列；**人工验证点：wdio 实跑四用例全绿**。

## E2E-03 定时刷新端到端（真实 tick 生效断言）

1. **位置**：`e2e-tests/background-tasks.e2e.ts`（与 E2E-02 同文件追加）。
2. **现状**：`run-wdio.cjs:226` 注入 `SLTERM_CLAUDE_PROJECTS_DIR` 指向 fixtures 副本 tmp 目录；`history.e2e.ts:294` 起已有「导航树历史节点归属会话展示」断言模式（`nav-history-node` 计数 pill + 行）可照抄；fixture 会话 jsonl 写法见 fixtures/claude-projects/ 与 history.e2e.ts 文件头注释（cwd 占位符替换为 SLTERM_E2E_PROJECT_DIR 真实路径）。
3. **修复步骤**——同文件追加用例：
   - 用例 E「定时刷新自动出现新会话」：创建 E2E 项目（cwd = SLTERM_E2E_PROJECT_DIR）→ 打开导航树确认历史计数 N → 设置中心把 sessionRefresh 频率改 2s（磁盘断言落盘）→ Node 侧往 `SLTERM_CLAUDE_PROJECTS_DIR/<编码目录>/` 写一个归属本项目的新会话 jsonl（照 history.e2e.ts fixture 形态：summary 首行 + user 行 cwd=项目路径）→ 等待（2×interval + 余量，约 5s）→ 断言历史节点计数 pill 变 N+1（全程无手动刷新点击）。
   - 用例 F「禁用 sessionRefresh → 新会话不自动出现；启用 → 出现」：禁用勾选 → 再写一个 jsonl → 等 2×interval → 计数不变；重新勾选 → 等 tick → 计数 +1。
   - 用例 G「tick 失败静默」：无可控故障注入通道——**降级为 L2 覆盖 + 人工观察**，spec 内不写；在 test-inventory 登记豁免（原因：tick 失败需后端扫描故障注入，无沙箱内通道；兜底 = 调度器 L2 用例 + 人工）。
4. **测试同步**：本项即测试；登记 test-inventory（DOC-02）。
5. **文档同步**：无。
6. **验证**：**人工验证点：wdio 实跑 E/F 两例全绿**（E2E 默认配置注意：sessionRefresh 默认 enabled=true interval=3s——E2E 全程定时刷新在跑，既有 history.e2e.ts/settings.e2e.ts 用例须连带复跑确认无干扰）。

---

## DOC-01 CLAUDE.md 系列 + CONTEXT.md 同步

1. **位置**：`.claude/CLAUDE.md`、`src-tauri/src/background_tasks/CLAUDE.md`（新建）、`src/features/backgroundTasks/CLAUDE.md`（新建）、`src-tauri/src/CLAUDE.md`、`src-tauri/src/plan_balance/CLAUDE.md`、`src/features/agentHistory/CLAUDE.md`、`src/features/navTree/CLAUDE.md`、`src/ipc/CLAUDE.md`、`src/types/CLAUDE.md`、`src/__tests__/CLAUDE.md`、`CONTEXT.md`。
2. **现状**：各文件相关段落见各项「现状」节引用。
3. **修复步骤**：
   - 3.1 根 `.claude/CLAUDE.md` 模块索引表加两行：`src/features/backgroundTasks | ../src/features/backgroundTasks/CLAUDE.md`、`src-tauri/src/background_tasks | ../src-tauri/src/background_tasks/CLAUDE.md`（插位照现有分组序）。
   - 3.2 新建 `src-tauri/src/background_tasks/CLAUDE.md`（模板：存在理由——为何骨架独立于 plan_balance（多任务复用/配置单写通道）→ 关键约束：静态切片注册表 U2 形态及理由 / 顺序写死「校验→落盘→内存」/ 单写通道复用 settings.rs 禁第二通道 / 锁序 CONFIG_WRITE_LOCK→SETTINGS_SAVE_LOCK / emit 事件感知通道 / 前端任务 executor=None 仅代管 → 外部坑：save_settings_blocking 是唯二消费点 / spawn-emit 包装层 L1 豁免 → 测试模式 + 既定豁免表）。
   - 3.3 新建 `src/features/backgroundTasks/CLAUDE.md`（存在理由——双端抽象前端半；关键约束：注册表家族契约 #13 / 订阅生命周期（首个订阅者读配置+立即一轮+interval，末退订停）/ 防重入闸门 tick+manual 共用 / 失败策略 tick 静默 vs manual error / applyConfig 运行期生效与无订阅者不空转 / 扫描执行体 force 恒 true 理由（mtime 缓存不敏感）/ 注册触发点 = useAgentHistory.ts 与 BackgroundTasksPage.tsx 顶部 import ./tasks；测试模式：fake timers + hoisted mock 清单）。
   - 3.4 `src-tauri/src/CLAUDE.md` settings.rs 节：白名单行改写为 `[..., background_tasks::SETTINGS_KEY]` 口径；「planBalance 段 = F10 轮询间隔」与「plan_balance_set_interval 专用命令通道」句改写为 backgroundTasks 段 + background_tasks_set_config 通道；新增「save_settings_blocking 同步写通道（F12 抽取，供 background_tasks set_config_core 复用）」句。
   - 3.5 `src-tauri/src/plan_balance/CLAUDE.md`：「轮询间隔：动态内存原子量」节整节改写——通用件上提 background_tasks（内存配置/首轮立即/每轮末 sleep 由骨架承载）；`plan_balance_set_interval` 退役，配置走 `background_tasks_set_config`；enabled=false 停轮询 + 快照保留 + 前端 footer 隐藏语义；默认间隔 60→10s（骨架注册表）；本模块保留执行体 `poll_once_executor` 与快照/merge/emit 口径不变。
   - 3.6 `src/features/agentHistory/CLAUDE.md`「数据流与刷新时机（FE-04/FE-19/NAV-10）」节改写：sessions/state 订阅调度器快照；触发时机 = 首个订阅者立即一轮 + 配置频率定时 + triggerNow 手动；force 恒 true；scan 退役；removeLocal 经 applyLocal。
   - 3.7 `src/features/navTree/CLAUDE.md`：「FE-19 历史扫描时机」三条改写（挂载即扫 → 订阅首轮；刷新钮 = triggerNow）；余量 footer 行追加「enabled=false 不渲染（F12）」；测试模式行追加 background-tasks 两测试文件。
   - 3.8 `src/ipc/CLAUDE.md`：planBalance 命令段删 `setPlanBalanceInterval` 条目；新增「backgroundTasks 命令（F12）」段（list 读通道 / set_config 写通道+校验顺序 / onBackgroundTasksUpdated 事件）；「测试模式」节契约文件清单加 `ipc-background-tasks-contract.test.ts`。
   - 3.9 `src/types/CLAUDE.md` 对照表加 `backgroundTasks.ts ↔ src-tauri/src/background_tasks/mod.rs`。
   - 3.10 `src/__tests__/CLAUDE.md`：全局 mock 清单节加 `../ipc/backgroundTasks` 行；F11 测试文件迁移映射表后追加 F12 行（settings-plan-balance.test.tsx 退役 → settings-background-tasks.test.tsx 新增）。
   - 3.11 `CONTEXT.md`：「全局组」行「（快捷键、套餐余量查询频率）」改「（快捷键、后台定时任务）」；新增术语三条——**后台定时任务**（应用级周期性任务单元，元数据单点在后端注册表，配置统一 settings.json backgroundTasks 段）/ **扫描执行体**（历史会话扫描唯一执行路径，遍历全部已注册 history provider 聚合）/ **触发来源**（manual/tick，仅影响失败处理策略）。
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：grep `plan_balance_set_interval` 在全部 CLAUDE.md 零命中（历史叙事句除外则改写）；模块索引两行存在；两个新模块 CLAUDE.md 存在且含注册触发点/豁免表；CONTEXT.md 三术语存在。

## DOC-02 test-inventory.md 同步

1. **位置**：`.claude/test-inventory.md`。
2. **现状**（实读）：:89 settings.rs 26 例、:99 plan_balance/mod.rs 24 例、:132 ipc-plan-balance-contract 16 例、:247/248 plan-balance 模型/footer、:297 settings-plan-balance 16 例、:450/452 变更日志行。
3. **修复步骤**：
   - 3.1 settings.rs 行：26 → 27（删 1 改名 + 新增 save_rejects_plan_balance_key 1 例，净 +1——以执行后实跑统计为准改写）。
   - 3.2 plan_balance/mod.rs 行：24 → 14（删 10 例）。
   - 3.3 新增 background_tasks 模块行（mod.rs + registry.rs 用例数按实跑统计）。
   - 3.4 ipc-plan-balance-contract 行：16 → 12（删 setPlanBalanceInterval 四维 4 例）；新增 ipc-background-tasks-contract 行（按实跑）。
   - 3.5 settings-plan-balance 行删除；新增 settings-background-tasks / background-tasks-scheduler / background-tasks-session-refresh 行；agent-history-hook / nav-tree 两文件计数按实跑更新。
   - 3.6 E2E 区：settings.e2e.ts 11 不变；新增 background-tasks.e2e.ts（6 例：A-F）。
   - 3.7 豁免登记：background_tasks spawn/emit 包装层（L1 不可测，兜底 L4+人工）；tick 失败静默 E2E 豁免（E2E-03 用例 G，兜底 L2+人工）。
   - 3.8 变更日志加 2026-08-31 F12 行（全量计数按实跑汇总）。
4. **测试同步**：本项即清单同步（硬约束 #11）。
5. **文档同步**：本项即文档。
6. **验证**：计数与实跑输出一致（cargo test / npm test / wdio 统计行）；豁免两条例外登记在案。

## DOC-03 ADR-0013 补写（F12 双端抽象决策）

1. **位置**：`.claude/adr.md`。
2. **现状**（实读）：adr.md 无 ADR-0013（grep 零命中）——根 CLAUDE.md F12 行已引用「决策见 ADR-0013」，属预登记引用，必须补写否则断链。
3. **修复步骤**：按 adr.md 既有格式追加 ADR-0013，决策点：
   - 双端各自抽象（后端 poller 骨架 + 前端调度器）而非单端统一——执行体天然双栖（套餐查询必须后端触网、session 扫描编排在前端订阅生命周期内）；
   - 任务元数据单点在后端注册表（含前端任务）——配置读通道统一 `background_tasks_list`，前端不复制边界/默认值（DTO 无 default 字段的直接后果）；
   - 前端调度器订阅者计数生命周期（无订阅者不空转扫盘）——sessionRefresh 空转即读盘，必须随 NavTree 卸载暂停（规格 §8）；
   - 配置变更前端感知经 emit 事件（background-tasks-updated）而非前端总线——后端单写通道真值源。
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：grep `ADR-0013` 命中 adr.md；根 CLAUDE.md F12 行引用不再断链。
