# 设置中心（F11）开发清单

- **输入**：`docs/settings-center-requirements.md`（需求规格，已冻结）
- **编号**：`SC-BE/FE/E2E/DOC-NN`——阶段作用域代号（MC-\*/HKC-\* 先例），免入根表
- **优先级**：由 Stage 依赖顺序表达（见 `stages.md`），不用 P0-P4
- **执行**：`/systematic-changes-execute` 按 `execution-plan.md` 编排；本清单是唯一修复依据，执行 agent 逐 ID 照做

## 需求规格的 3 处落地修订（计划期代码实证回弹，SC-DOC-01 回写）

| # | 规格原文 | 修订 | 证据 |
|---|---------|------|------|
| R1 | §4.1「无项目也可打开」 | 无项目点配置钮 → toast「请先创建项目」 | Workspace.tsx 主区 `allPages.map`——无项目=无页面=无 Dockview 宿主（用户已拍板 A） |
| R2 | §5.2「AppState 原子量」 | plan_balance **模块级 static 原子量** | 读写双方同在 plan_balance 模块（无跨模块共享）；`SNAPSHOT` 模块级 static 先例（mod.rs:66）；`State<AppState>` 注入使 L1 无法直调命令 fn（现有命令测试全为直调） |
| R3 | §4.2「项目组禁用+提示」 | 删除（不可达） | 无项目开不了面板（R1 toast 拦截）；项目删除→页面销毁→面板随之销毁。hooks 页 `rootPath=null` 防御（useHooksConfig 既有）已足够 |

## 事实基线（取证凝固，清单引用为 FB-NN）

- **FB-01** plan_balance/mod.rs:66 `static SNAPSHOT: Mutex<Option<Vec<PlanBalanceInfo>>> = Mutex::new(None);`（注释：照 hooks/mod.rs WATCHER 先例，不入 AppState 避免循环依赖）；:68-71 `#[cfg(test)] pub(crate) fn reset_snapshot_for_test()`
- **FB-02** plan_balance/mod.rs:167-169 `const DEFAULT_INTERVAL_SEC: u64 = 60; MIN_INTERVAL_SEC = 10; MAX_INTERVAL_SEC = 3600;`；:171-182 `resolve_poll_interval()` 读 settings.json `planBalance.intervalSec`，越界/缺失/损坏回退 60（读取处为字面量 `root.get("planBalance")?.get("intervalSec")`）
- **FB-03** plan_balance/mod.rs:186-200 `start_plan_balance_poller`：`tokio::time::interval(interval)` 首 tick 立即，loop 内 `spawn_blocking(poll_once_production)` → `apply_snapshot`
- **FB-04** plan_balance/mod.rs:204-207 `get_plan_balance`；:210-220 `refresh_plan_balance`（恒 Ok）；:222-234 测试模块 `block_on` helper（current_thread runtime，照 hooks/mod.rs:443 先例）
- **FB-05** settings.rs:18-24 `SETTINGS_ALLOWED_KEYS: [&str; 5] = ["fontSize","keybindings","sideBar","colorScheme","planBalance"]`；:32 `SETTINGS_SAVE_LOCK`；:68 `save_settings` 是 `#[tauri::command] pub async fn`（可直接 await 调用复用写通道：白名单/浅合并/原子写/.bak）
- **FB-06** lib.rs:137-138 generate_handler! 尾部 `plan_balance::get_plan_balance, plan_balance::refresh_plan_balance,`；:99 setup 中 `plan_balance::start_plan_balance_poller`
- **FB-07** build.rs:52-53 尾部 `"get_plan_balance", "refresh_plan_balance",`（注释「当前 36 条」）
- **FB-08** capabilities/default.json:52-53 尾部 `"allow-get-plan-balance", "allow-refresh-plan-balance"`（:53 为数组末位无逗号——新增行须先给 :53 补逗号）
- **FB-09** HooksConfigPanel.tsx（hub，全 291 行）：props :38-45 `{api, containerApi, params?:{panelId?, selectedCli?}}`；`persistSelectedCli` :97-106（updateParameters + 显式 onLayoutChange(saveLayout)）；selectedCli init :123-127；dirty 守卫 :139-152（dirtyRef/askGuardRef/ASK_GUARD_MS=500）；`handleLayoutPersist` :156-170（panelId `hooksConfig-` 前缀 slice → 查 projects → updatePageLayout）；`handleCliSelect` :174-209（confirmDialog + finally setTimeout 复位 askGuard）；hub JSX :232-287（选择行 data-e2e 形如 hooks-cli-<profileId>、编辑器槽 key={selectedProfile.id}、根 data-e2e="hooks-config-panel"）
- **FB-10** pageApis.ts:130-156 `openHooksConfigPanel(pageId)`：panelId=`` `hooksConfig-${pageId}` ``，100ms×50 轮询 getPageApi，命中 `focus?.()` 否则 `addPanel({id:panelId, component:"hooksConfig", title:"Hooks 配置", params:{panelId}})`；:11 注释引用 features/hooksConfig/CLAUDE.md（SC-FE-06 同步改）
- **FB-11** panelRegistry.ts：:12 import HooksConfigPanel（直文件 `./panels/hooksConfig/HooksConfigPanel`，非 barrel）；:39-48 FE-22 惰性 displayName getter（TDZ 循环：HooksConfigPanel → layoutSerde → panelRegistry）；:67-69 `hooksConfig: withPanelBoundary(HooksConfigPanel as React.FC<{params:{panelId:string}}>)`；:73-80 PANEL_TYPES = [PANEL_TERMINAL, PANEL_EDITOR, PANEL_HTML_VIEWER, "gitshow", "diff", "hooksConfig"]（hooksConfig 末位）；:101-103 `isAlwaysRenderPanel` 仅 terminal+htmlviewer
- **FB-12** features/hooksConfig/openHooksConfig.ts:28-58 `openHooksConfigFromActivityBar()`：无项目 → 静默 return；活跃页面所属项目优先兜底第一个；pages[0] 否则 addPage 新建空布局页；`await switchToPageShared(pageId); await openHooksConfigPanel(pageId);`
- **FB-13** ActivityBar.tsx:26 `import { openHooksConfigFromActivityBar } from "../hooksConfig/openHooksConfig";`；:248-267 配置钮（`data-e2e="activity-btn-config"`，title="配置"，:258-260 `onClick={() => { void openHooksConfigFromActivityBar(); }}`）
- **FB-14** ShortcutRegistry.ts：setOverrides :135-138；resolve :145-148；listCommands :162-172；`private effectiveKeystroke(cmd)` :181-199（overrides 有键 → null 解绑/非法回退默认/isReserved 回退默认/合法用之；无覆盖 → defaultKey）；handleKeyDown :252-258（window capture）；_reset :273-280；getShortcutRegistry 惰性单例 :287-292。keystroke.ts `formatKeystroke/parseKeystroke`；reserved.ts `isReserved`；COMMAND_CATALOG 9 条；`useKeybindings`（setBinding/clearBinding/debounce 2s）；confirmDialog lib/ConfirmDialog.tsx:80
- **FB-15** PageDockviewHost.tsx:458 DefaultTab × 钮 `onClick={(e) => { e.stopPropagation(); api.close(); }}`；:296 右键「关闭」`action: () => params.panel.api.close()`（两处置拦截点）
- **FB-16** layoutSerde.ts:86-94 loadLayout 白名单过滤（`isValidPanelType(id)` 不通过 `delete panels[key]`）
- **FB-17** ipc/planBalance.ts（全 30 行）：getPlanBalance/refreshPlanBalance/onPlanBalanceUpdated 三 wrapper
- **FB-18** e2e-tests/helpers.ts：:265-283 installSettingsHelpers（`__slterm_e2e_resetSettings`）；:357-361 installHookHelpers；:372-390 installHooksConfigHelpers（`__slterm_e2e_setHooksConfigJson` 经 `EditorView.findFromDOM` 定位 `[data-e2e="hooks-json-editor"]` 容器——JsonMode 挂载点 data-e2e 属性随组件迁移保留，选择器不变）
- **FB-19** e2e 消费面：hooks.e2e.ts :320-324 程序化 `addPanel({component:"hooksConfig"})`、:309-315/:462-469/:489-493 清理模式 `if (p.component === "hooksConfig") p.api.close()`、:332/:347/:357 选择器 `[data-e2e="hooks-config-panel"]`；mockcli.e2e.ts :312-407 同模式（CS-3 用例② hub 分派 + MockCliConfigEditor 桩 helpers.ts:401-414）
- **FB-20** claude profile：cliProfiles/profiles/claude/index.ts :16 import `../../../panels/hooksConfig/ClaudeHooksConfigEditor`、:52-69 capabilities.hooks（hasConfigEditor:true、configEditor :61、configLayers 三层）、:78-79 side-effect 注册；cliProfiles/CLAUDE.md:52 KZ-1 依赖方向合法化声明（迁移后重写）
- **FB-21** 编辑器迁移文件清单（10 文件 + schema/）：ClaudeHooksConfigEditor.tsx / useHooksConfig.ts / GuiMode.tsx / JsonMode.tsx / EventTree.tsx / HandlerForm.tsx / MatcherTester.tsx / configModel.ts / eventsCatalog.ts / matcherEngine.ts + features/hooksConfig/schema/（index.ts + claude-code-settings.json）。`panels/hooksConfig/index.ts`（barrel 单行 re-export）与 `HooksConfigPanel.tsx`（hub）**不迁**——hub 改造为 HooksSettingsPage，barrel 删除（唯一消费方 hooks-config-sync.test.tsx:92）
- **FB-22** 测试波及面 13 文件（SC-FE-05/06 测试同步的完整枚举，逐文件行号见条目）：
  - 路径更新（8）：hooks-config-catalog.test.ts:20 / hooks-config-matcher.test.ts:9 / hooks-config-model.test.ts:12 / hooks-config-handlerform.test.tsx:11-13 / hooks-config-gui.test.tsx:13-16 / hooks-config-jsonmode.test.tsx:105-108 / hooks-config-schema.test.ts:12 / hooks-config-sync.test.tsx:87-88,92-93
  - 路径更新（+3）：cli-profile-claude.test.ts:34 / mock-cli-profile.test.tsx:46,256 / no-claude-literals.test.ts:50（EXEMPT_DIRS 删 `"src/panels/hooksConfig"` 条目——configEditor 已被既有 `:46` 条目 `"src/features/cliProfiles/profiles/claude"` 前缀豁免覆盖，无需新增）
  - 改名改造（1）：hooks-config-panel.test.tsx → settings-hooks-page.test.tsx
  - 删除（2，归 SC-FE-06）：open-hooks-config.test.ts / open-hooks-config-panel.test.ts
  - mock 路径改（1，归 SC-FE-06）：activityBar.test.tsx:36
  - **不受影响**：ipc-hooks-config-contract.test.ts（后端命令不动）

---

## SC-BE-01 plan_balance 模块级间隔原子量 + 动态轮询循环（Stage 01）

1. **位置**：`src-tauri/src/plan_balance/mod.rs:66`（SNAPSHOT 旁）、`:186-200`（poller）
2. **现状**：FB-01 / FB-03 verbatim
3. **修复步骤**：
   1. SNAPSHOT 旁新增（import `std::sync::atomic::{AtomicU64, Ordering}`）：
   ```rust
   /// 当前轮询间隔秒数（F11：set_interval 命令写入、poller 每轮读取——运行期可改）
   static POLL_INTERVAL_SEC: AtomicU64 = AtomicU64::new(DEFAULT_INTERVAL_SEC);
   ```
   2. poller 改造为「启动初始化内存值 + 每轮末按当前内存值 sleep」（保留 D8 首轮立即语义；interval period 不可变故弃 ticker）：
   ```rust
   pub fn start_plan_balance_poller(app_handle: tauri::AppHandle) {
       // 启动时从磁盘初始化内存间隔（resolve_poll_interval 钳制兜底：越界/损坏 → 60）
       POLL_INTERVAL_SEC.store(resolve_poll_interval().as_secs(), Ordering::Relaxed);
       tauri::async_runtime::spawn(async move {
           loop {
               // 首轮立即执行（D8 语义保留）；此后每轮末按当前内存间隔 sleep——
               // set_interval 命令改值后下一轮即按新间隔（F11 立即生效）
               let now = unix_now();
               let handle = app_handle.clone();
               match tokio::task::spawn_blocking(move || poll_once_production(now)).await {
                   Ok(new) => apply_snapshot(&handle, new),
                   Err(e) => tracing::warn!(error = %e, "套餐余量轮询任务异常"),
               }
               let secs = POLL_INTERVAL_SEC.load(Ordering::Relaxed);
               tokio::time::sleep(Duration::from_secs(secs)).await;
           }
       });
   }
   ```
4. **测试同步**：mod.rs 测试模块新增 `poll_interval_memory_default_is_60`（`POLL_INTERVAL_SEC.load() == 60`——测试隔离：用例内 store 回 60 防串扰，照 reset_snapshot_for_test 先例加 `#[cfg(test)] pub(crate) fn reset_poll_interval_for_test()`）；既有 4 例 `resolve_poll_interval_*` 不动（磁盘读取函数保留）
5. **文档同步**：plan_balance/CLAUDE.md 轮询间隔段（Stage 07 / SC-DOC-04 统一）
6. **验证**：`grep -n "POLL_INTERVAL_SEC" src-tauri/src/plan_balance/mod.rs` ≥3 命中；`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零警告；`cargo test --manifest-path src-tauri/Cargo.toml plan_balance -- --test-threads=1` 绿

## SC-BE-02 plan_balance_set_interval 专用命令（Stage 01）

1. **位置**：`src-tauri/src/plan_balance/mod.rs:220`（refresh 命令后）
2. **现状**：FB-04（get/refresh 两命令 verbatim）
3. **修复步骤**：新增命令（顺序写死：校验 → 落盘 → 内存——落盘失败内存不变，磁盘/内存恒一致）：
```rust
/// 设置轮询间隔（F11 后端消费型配置写通道：校验 → 复用 settings 写通道落盘 → 更新内存值）
/// 越界 → Validation 拒绝且磁盘/内存均不变
#[tauri::command]
pub async fn plan_balance_set_interval(interval_sec: u64) -> Result<(), AppError> {
    if !(MIN_INTERVAL_SEC..=MAX_INTERVAL_SEC).contains(&interval_sec) {
        return Err(AppError::Validation(format!(
            "设置轮询间隔失败: 须为 {MIN_INTERVAL_SEC}–{MAX_INTERVAL_SEC} 秒，实际 {interval_sec}"
        )));
    }
    // 复用 settings.rs 写通道（白名单/浅合并/原子写/.bak/SETTINGS_SAVE_LOCK）——禁止自建第二写通道
    crate::settings::save_settings(serde_json::json!({
        SETTINGS_KEY: { INTERVAL_SEC_KEY: interval_sec }
    }))
    .await?;
    POLL_INTERVAL_SEC.store(interval_sec, Ordering::Relaxed);
    Ok(())
}
```
4. **测试同步**：mod.rs 测试模块新增 4 例（AppDataDirGuard 注入 tempdir，直调命令 fn）：`set_interval_valid_persists_and_updates_memory`（120 → load_settings 回读 `planBalance.intervalSec==120` 且 `POLL_INTERVAL_SEC==120`）；`set_interval_below_min_rejected`（5 → Validation、磁盘无文件、内存不变）；`set_interval_above_max_rejected`（9999 → 同）；`set_interval_disk_memory_consistent`（合法写后磁盘值==内存值）。每例首行 `reset_poll_interval_for_test()`
5. **文档同步**：plan_balance/CLAUDE.md（Stage 07 / SC-DOC-04 统一）
6. **验证**：4 用例绿；`grep -n "plan_balance_set_interval" src-tauri/src/plan_balance/mod.rs` 命中

## SC-BE-03 三处注册（Stage 01）

1. **位置**：`src-tauri/src/lib.rs:137` 后；`src-tauri/build.rs:53` 后；`src-tauri/capabilities/default.json:53` 后
2. **现状**：FB-06 / FB-07 / FB-08（尾部均为 plan_balance 两命令；default.json:53 末位无逗号）
3. **修复步骤**：① lib.rs `plan_balance::refresh_plan_balance,` 后加 `plan_balance::plan_balance_set_interval,`；② build.rs `"refresh_plan_balance",` 后加 `"plan_balance_set_interval",`；③ default.json 先给 `"allow-refresh-plan-balance"` 行末补逗号，再加 `"allow-plan-balance-set-interval"`
4. **测试同步**：无 L1（注册宏）。**mock 边界盲区认知**：三处缺失时 invoke reject 被前端 `.catch` 吞 = 测试全绿但运行时静默失败——由 Stage 06 L4 频率用例真实 invoke 兜底（stages 标注）
5. **文档同步**：无
6. **验证**：`grep -c "plan_balance_set_interval" src-tauri/src/lib.rs src-tauri/build.rs` 各 1；`grep -c "allow-plan-balance-set-interval" src-tauri/capabilities/default.json` =1

## SC-BE-04 域键名常量归域（Stage 01）

1. **位置**：`src-tauri/src/plan_balance/mod.rs:167-182`；`src-tauri/src/settings.rs:18-24`
2. **现状**：FB-02 / FB-05（白名单 5 字面量；resolve_poll_interval 内 `"planBalance"`/`"intervalSec"` 字面量）
3. **修复步骤**：
   1. mod.rs 常量区加 `pub(crate) const SETTINGS_KEY: &str = "planBalance";` + `const INTERVAL_SEC_KEY: &str = "intervalSec";`；`resolve_poll_interval` 的 `root.get("planBalance")?.get("intervalSec")` 改 `root.get(SETTINGS_KEY)?.get(INTERVAL_SEC_KEY)`
   2. settings.rs 白名单 `"planBalance"` 改 `crate::plan_balance::SETTINGS_KEY`（数组仍 5 项）；**决策写死**：前端消费型四键（fontSize/keybindings/sideBar/colorScheme）保留字面量 + 注释更新为「前端消费型域无后端模块可归，键名集中于此；后端消费型域键名归域模块（plan_balance::SETTINGS_KEY 先例）」
4. **测试同步**：mod.rs 新增 `settings_key_constants_value`（`SETTINGS_KEY=="planBalance"`、`INTERVAL_SEC_KEY=="intervalSec"` 防字面量漂移）；settings.rs 既有 `save_accepts_plan_balance_key` 锁死行为不变
5. **文档同步**：src-tauri/src/CLAUDE.md 白名单段（Stage 07 / SC-DOC-04 统一）
6. **验证**：`grep -n "SETTINGS_KEY" src-tauri/src/settings.rs` 命中；`cargo test --manifest-path src-tauri/Cargo.toml settings -- --test-threads=1` + `cargo test --manifest-path src-tauri/Cargo.toml plan_balance -- --test-threads=1` 绿

## SC-FE-01 SettingsPageRegistry + 类型（Stage 02，agent A）

1. **位置**：新建 `src/features/settingsCenter/{types.ts, SettingsPageRegistry.ts, index.ts}`
2. **现状**：无（参照 getShortcutRegistry 惰性单例 FB-14 :287-292 + 硬约束 #13 家族契约）
3. **修复步骤**：
   1. types.ts：
   ```typescript
   /** 配置页分组（F11）：global=应用级单例 / project=需项目上下文 */
   export type SettingsPageGroup = "global" | "project";
   /** 壳透传给配置页的 props——dirty 上报 + 页内状态持久化通道（壳是 params 持久化单点） */
   export interface SettingsPageProps {
     onDirtyChange?: (dirty: boolean) => void;
     pageParams?: Record<string, unknown>;
     onPageParamsChange?: (patch: Record<string, unknown>) => void;
   }
   /** 配置页注册项 */
   export interface SettingsPage {
     id: string;
     title: string;
     group: SettingsPageGroup;
     component: React.FC<SettingsPageProps>;
     order?: number;
   }
   ```
   2. SettingsPageRegistry.ts：模块级单例 class（`register(page)` 同 id 幂等覆盖 / `getAll(group?)` 按 `order ?? 注册序` / `get(id)` / `_reset()` 仅测试）+ `getSettingsPageRegistry()` 惰性导出
   3. index.ts barrel
4. **测试同步**：新建 `src/__tests__/settings-page-registry.test.ts`（注册/getAll 分组过滤/order 排序缺省注册序/重复 id 覆盖/_reset 隔离）
5. **文档同步**：features/settingsCenter/CLAUDE.md 新建（Stage 07 / SC-DOC-04）
6. **验证**：测试文件存在且绿；`npx tsc --noEmit` 零错

## SC-FE-02 openSettings 编排 + openSettingsPanel（Stage 02，agent A）

1. **位置**：新建 `src/features/settingsCenter/openSettings.ts`；`src/workspace/pageApis.ts:156` 后新增
2. **现状**：FB-10（openHooksConfigPanel verbatim）；FB-12（openHooksConfigFromActivityBar：无项目静默 return / 活跃项目优先 / pages[0] / 新建页 / 先切页后开面板）
3. **修复步骤**：
   1. pageApis.ts 新增 `openSettingsPanel(pageId: string, settingsPageId?: string): Promise<boolean>`——照 openHooksConfigPanel 模式：panelId=`` `settings-${pageId}` ``；getPanel 命中 → `existing.focus?.()` 返回 true；否则 `api.addPanel({ id: panelId, component: "settings", title: "设置", params: { panelId, ...(settingsPageId ? { selectedPage: settingsPageId } : {}) } })`；100ms×50 轮询 + console.warn 降级
   2. openSettings.ts 新增 `openSettings(settingsPageId?: string): Promise<void>`——**无项目 → `toast.show("warning", "请先创建项目")` + return（R1 修订，取代现静默 return）**；其余照搬 openHooksConfigFromActivityBar 编排（活跃项目优先/兜底第一个/pages[0]/makeEmptyLayout 新建页/switchToPageShared → openSettingsPanel(pageId, settingsPageId)）
4. **测试同步**：新建 `src/__tests__/open-settings.test.ts`（无项目 toast 且不切页 / 活跃项目优先 / 兜底第一个项目 / 切页先于开面板 invocationCallOrder）+ `src/__tests__/open-settings-panel.test.ts`（addPanel 参数精确 `{id:"settings-page-a",component:"settings",title:"设置",params:{panelId:"settings-page-a"}}` / 单例 focus 不新建 / pageId 跟随 / 深链 settingsPageId 注入 selectedPage / 5s 超时降级）
5. **文档同步**：无（Stage 07 统一）
6. **验证**：两测试文件绿

## SC-FE-03 SettingsPanel 壳（Stage 02，agent B）

1. **位置**：新建 `src/panels/settings/{SettingsPanel.tsx, index.ts}`
2. **现状**：无（hub 先例 FB-09 各段）
3. **修复步骤**：
   1. props `{ api: DockviewPanelApi; containerApi: DockviewApi; params?: { panelId?: string; selectedPage?: string; pageParams?: Record<string, Record<string, unknown>> } }`
   2. 顶部 `import "../../features/settingsCenter/pages";`（side-effect import 注册触发点，登记模块 CLAUDE.md）
   3. selectedPage state：params.selectedPage 命中注册表 → 用之；否则全局组第一页；注册表空 → 空态「暂无配置页」
   4. 结构：左导航固定 180px（组序 global→project，组标题 + 页项 data-e2e 形如 settings-nav-<pageId> + dirty 圆点槽）+ 右槽位 `key={selectedPage}` 强制重挂载（ADR-0001 先例）渲染 `page.component`，透传 `{ onDirtyChange, pageParams: params?.pageParams?.[selectedPage], onPageParamsChange }`
   5. **壳是 params 持久化单点**：`persistParams(patch)` = `api.updateParameters({...params, ...patch})` + 显式 `onLayoutChange(saveLayout(containerApi))` + 按 panelId `settings-` 前缀解析 pageId → `updatePageLayout`（照 FB-09 :97-106/:156-170 先例改前缀）；选中切换与 `onPageParamsChange(patch)`（`pageParams: {...params?.pageParams, [selectedPage]: patch}`）均经此通道
   6. `onDidParametersChange` 订阅外部 selectedPage 变化 → setState（扁平事件结构红线：回调直接是 Parameters）
   7. corrupted 警示条：挂载 `loadSettings()` → `corrupted===true` → 顶部警示条「设置文件已损坏，已从备份/默认值恢复」（× 可关，`data-e2e="settings-corrupted-banner"`，不阻塞）
   8. 配色全走 theme/colors.ts token（硬约束 #6）
4. **测试同步**：新建 `src/__tests__/settings-panel.test.tsx`（导航组序 global 在前 / 选中渲染对应页 / 切换 persist（updateParameters+toJSON）/ params.selectedPage 失效回退全局组第一页 / corrupted 警示条渲染与关闭 / pageParams 透传与持久化 / 注册表空 → 空态）
5. **文档同步**：panels/CLAUDE.md settings 节（Stage 07 / SC-DOC-04）
6. **验证**：测试绿

## SC-FE-04 频率页 + ipc wrapper（Stage 02，agent B）

1. **位置**：`src/ipc/planBalance.ts:30` 后；新建 `src/panels/settings/pages/PlanBalancePage.tsx`；新建 `src/features/settingsCenter/pages.ts`
2. **现状**：FB-17（ipc/planBalance.ts 三 wrapper verbatim）
3. **修复步骤**：
   1. ipc/planBalance.ts 加：
   ```typescript
   /** 设置轮询间隔秒（F11）：后端校验 10–3600 → 落盘 + 更新内存值，立即生效 */
   export function setPlanBalanceInterval(intervalSec: number): Promise<void> {
     return invoke("plan_balance_set_interval", { intervalSec });
   }
   ```
   2. pages.ts（注册触发点，Stage 02 仅注册本页；Stage 03/04 追加）：
   ```typescript
   import { getSettingsPageRegistry } from "./SettingsPageRegistry";
   import PlanBalancePage from "../../panels/settings/pages/PlanBalancePage";
   getSettingsPageRegistry().register({ id: "planBalance", title: "套餐余量", group: "global", component: PlanBalancePage, order: 20 });
   ```
   3. PlanBalancePage：挂载 `loadSettings()` → `data?.planBalance?.intervalSec` 为有限数且 10–3600 → 显示之，否则显示 60；输入框失焦/Enter 提交：trim → `Number` 解析 + 整数 + 10–3600 → 非法 → 行内红字「10–3600 秒，默认 60」不提交不 toast；合法 → `setPlanBalanceInterval(v)` → 成功后 `refreshPlanBalance().catch((e) => console.error(...))`（生效反馈闭环）→ Err → `toast.show("warning", ...)` + 输入框保留用户值
4. **测试同步**：新建 `src/__tests__/settings-plan-balance.test.tsx`（缺失/越界显示 60 / 合法提交调命令且 refresh / 非法行内红字不提交 / 命令 Err → toast+保留输入）；`src/__tests__/ipc-plan-balance-contract.test.ts`（已存在，Glob 实证）加 setPlanBalanceInterval 四维契约（命令名逐字 / payload 键集合精确 `{intervalSec:120}` / 正常返回 / 异常传播）
5. **文档同步**：ipc/CLAUDE.md planBalance 节（Stage 07 / SC-DOC-04）
6. **验证**：测试绿；`grep -n "setPlanBalanceInterval" src/ipc/planBalance.ts` 命中

## SC-FE-05 hooks 页迁入 + 编辑器归域（Stage 03，agent A=迁移 / agent B=HooksSettingsPage）

1. **位置**：新建 `src/panels/settings/pages/HooksSettingsPage.tsx`；移动 `src/panels/hooksConfig/` 10 编辑器文件 → `src/features/cliProfiles/profiles/claude/configEditor/`；移动 `src/features/hooksConfig/schema/` → `src/features/cliProfiles/profiles/claude/configEditor/schema/`；删除 `src/panels/hooksConfig/{HooksConfigPanel.tsx, index.ts}`；`src/features/cliProfiles/profiles/claude/index.ts:16`
2. **现状**：FB-09（hub verbatim）；FB-20（configEditor 指向 panels/hooksConfig——panels/CLAUDE.md「编辑器在 features/cliProfiles」失实漂移，本项坐实归域）；FB-21（10 文件清单 + barrel 处置）
3. **修复步骤**：
   1. git mv 编辑器 10 文件 + schema/ 至 configEditor/（schema/index.ts 自包含零改行——仅 import json-schema-library + `./claude-code-settings.json`，已实读核实）；删除 `HooksConfigPanel.tsx` + `index.ts`（barrel）；`src/features/hooksConfig/` 整目录随迁移清空（openHooksConfig.ts 由 SC-FE-06 删、CLAUDE.md 由 SC-DOC-04 删）
   2. 迁移后跨目录 import 改造（`./` 同目录 import 全部不变；深度 2→5，`../../` → `../../../../../`）：

   | 文件:行 | 现状 | 改为 |
   |---|---|---|
   | useHooksConfig.ts:26 | `../../ipc/hooksConfig` | `../../../../../ipc/hooksConfig` |
   | useHooksConfig.ts:28 | `../../lib` | `../../../../../lib` |
   | useHooksConfig.ts:29 | `../../stores/projects` | `../../../../../stores/projects` |
   | useHooksConfig.ts:30 | `../../stores/layout` | `../../../../../stores/layout` |
   | useHooksConfig.ts:31 | `../../types/hooksConfig` | `../../../../../types/hooksConfig` |
   | useHooksConfig.ts:32 | `../../features/hooksConfig/schema` | `./schema` |
   | JsonMode.tsx:7（注释） | `src/features/hooksConfig/schema` | `./schema`（同目录） |
   | JsonMode.tsx:30 | `../../features/hooksConfig/schema` | `./schema` |
   | JsonMode.tsx:44 | `../../theme` | `../../../../../theme` |
   | MatcherTester.tsx:11 | `../../theme` | `../../../../../theme` |
   | EventTree.tsx:22 | `../../theme` | `../../../../../theme` |
   | HandlerForm.tsx:35 | `../../theme` | `../../../../../theme` |
   | GuiMode.tsx:29 | `../../theme` | `../../../../../theme` |
   | ClaudeHooksConfigEditor.tsx:37 | `../../ipc/agentHooks` | `../../../../../ipc/agentHooks` |
   | ClaudeHooksConfigEditor.tsx:38 | `../../types/agent` | `../../../../../types/agent` |
   | ClaudeHooksConfigEditor.tsx:40 | `../../types/hooksConfig` | `../../../../../types/hooksConfig` |
   | ClaudeHooksConfigEditor.tsx:41 | `../../features/cliProfiles`（import type，type-only 擦除无运行时循环） | `../../..`（cliProfiles barrel） |
   | ClaudeHooksConfigEditor.tsx:48 | `../../theme` | `../../../../../theme` |
   | eventsCatalog.ts:4（注释） | `src/features/hooksConfig/CLAUDE.md` | `src/features/cliProfiles/CLAUDE.md`（SC-DOC-04 收编后口径） |

   3. claude/index.ts:16 import 改 `./configEditor/ClaudeHooksConfigEditor`；头部「依赖方向合法化」注释更新（不再跨 panels 引用）
   4. 新建 HooksSettingsPage = hub 改造：CLI 选择行/编辑器槽分派/dirty 守卫（askGuard+confirmDialog 切 CLI）照搬 FB-09 :232-287/:139-209；**props 改 `SettingsPageProps`**——编辑器 dirty 经 `onDirtyChange` 直传壳；`selectedCli` 改读 `pageParams?.selectedCli`、写经 `onPageParamsChange({ selectedCli })`（壳单点持久化，不再自己 handleLayoutPersist）；**根容器保留 `data-e2e="hooks-config-panel"`**（选择器语义继承，最小化 E2E 适配面）；pages.ts 追加注册 `{ id: "hooks", title: "Hooks 配置", group: "project", order: 100 }`
4. **测试同步**（FB-22 全枚举，agent A 负责路径更新、agent B 负责 hub 语义改造）：
   - 路径更新（8 文件）：hooks-config-catalog.test.ts:20 / hooks-config-matcher.test.ts:9 / hooks-config-model.test.ts:12 / hooks-config-handlerform.test.tsx:11-13 / hooks-config-gui.test.tsx:13-16 / hooks-config-jsonmode.test.tsx:105-108（含 schema）/ hooks-config-schema.test.ts:12（schema）——`../panels/hooksConfig/X` → `../features/cliProfiles/profiles/claude/configEditor/X`，schema 路径同步
   - hooks-config-sync.test.tsx：:87-88 mock 路径同步；:92 barrel import `../panels/hooksConfig` → `../panels/settings/pages/HooksSettingsPage`；:93 useHooksConfig 路径同步；mockApi/mockContainerApi（hub panel props 形态 :57-67）→ SettingsPageProps 形态（onDirtyChange/pageParams/onPageParamsChange），renderLoadedPanel 辅助同步改造
   - cli-profile-claude.test.ts:34 / mock-cli-profile.test.tsx:46,256（import HooksConfigPanel → HooksSettingsPage + mock JsonMode 路径同步）
   - no-claude-literals.test.ts:50：EXEMPT_DIRS 删 `"src/panels/hooksConfig"` 条目（目录消失；configEditor 已被 :46 父目录豁免覆盖，无需新增）
   - hooks-config-panel.test.tsx（hub 43 例）→ 改名 `settings-hooks-page.test.tsx` 并改造 renderPanel helper（panel props 形态 → SettingsPageProps 形态；updateParameters/toJSON 断言改断 onPageParamsChange 回调）
5. **文档同步**：cliProfiles/CLAUDE.md:52 KZ-1 重写（Stage 07 / SC-DOC-04）
6. **验证**：`grep -rn "panels/hooksConfig" src/ src-tauri/ e2e-tests/` 零命中；`grep -rn "features/hooksConfig" src/` 零命中（测试路径同步后）；`npm test` 绿

## SC-FE-06 panelRegistry 原子切换 + 入口切换（Stage 03，agent C）

1. **位置**：`src/panelRegistry.ts:12,:67-80`；`src/features/sideViews/ActivityBar.tsx:26,:258`；`src/workspace/pageApis.ts:11（注释）,:130-156（删 openHooksConfigPanel）`；删 `src/features/hooksConfig/openHooksConfig.ts`
2. **现状**：FB-11 / FB-13 / FB-10 / FB-12 verbatim
3. **修复步骤**：
   1. panelRegistry.ts：:12 import 改 `SettingsPanel`（from `./panels/settings`）；:67-69 registry 键 `hooksConfig: withPanelBoundary(HooksConfigPanel...)` 改 `settings: withPanelBoundary(SettingsPanel as React.FC<{ params: { panelId: string } }>)`；:73-80 PANEL_TYPES `"hooksConfig"` → `"settings"`（原位替换保持末位——既有测试断言末位）；:39-48 FE-22 惰性 getter 注释中 HooksConfigPanel 提及改 SettingsPanel（同形 TDZ 循环：SettingsPanel → pages.ts → ipc/stores，layoutSerde 链路同构，getter 已兼容）
   2. **isAlwaysRenderPanel 不加入 settings（决策写死）**：同 editor/gitshow/diff——重建无视觉闪屏，状态在 params/store；未保存 dirty 随卸载丢失与现 hooksConfig 行为一致继承，不新增 always 内存开销
   3. ActivityBar.tsx:26 import 改 `openSettings`（from `../settingsCenter/openSettings`）；:258 调用改 `void openSettings();`
   4. pageApis.ts：删 openHooksConfigPanel 函数；:11 注释引用 features/hooksConfig/CLAUDE.md 改指 features/settingsCenter/CLAUDE.md
   5. 删 openHooksConfig.ts 文件
4. **测试同步**：panel-registry.test.ts（六键含 settings / PANEL_TYPES toEqual 末位 settings / length 6）；layout-serde.test.ts 9a（c1 改 settings）+ **新增「旧 hooksConfig 面板被白名单过滤」用例**（验收断言：loadLayout 含 hooksConfig 面板 → fromJSON 参数中该面板已删除）；workspace-file-panel-types.test.ts（`hooksConfig` 断言行改 `settings`、size 4 不变——该集合本就不含 hooksConfig，核实现状断言后适配）；activityBar.test.tsx:36 mock 路径 `../features/hooksConfig/openHooksConfig` → `../features/settingsCenter/openSettings`（mock 导出名同步 openSettings）；删 open-hooks-config.test.ts / open-hooks-config-panel.test.ts（SC-FE-02 新测试已取代）
5. **文档同步**：sideViews/CLAUDE.md（Stage 07 / SC-DOC-04）
6. **验证**：上述测试绿；`grep -rn "openHooksConfig" src/` 零命中；`grep -n "hooksConfig" src/panelRegistry.ts` 零命中

## SC-FE-07 dirty 汇聚守卫（Stage 03，agent B）

1. **位置**：`src/panels/settings/SettingsPanel.tsx`（壳增强）；新建 `src/features/settingsCenter/dirtyRegistry.ts`；`src/workspace/PageDockviewHost.tsx:458`
2. **现状**：FB-09 :139-152,:174-209（hub dirty 守卫 verbatim）；FB-15（× 关闭链）
3. **修复步骤**：
   1. dirtyRegistry.ts：`Map<panelId, boolean>` + `setSettingsDirty(panelId, dirty)` / `isSettingsDirty(panelId)` / `clearSettingsDirty(panelId)`（壳挂载注册、卸载 clear；DefaultTab 拦截与壳共享同一真值源）
   2. 壳：dirtyMap state（pageId→dirty）；页 `onDirtyChange` → setState + 同步 dirtyRegistry（仅当前页 dirty 值）；导航项 dirty 圆点（7px 中性色 token——不用 F3 四态色，防语义混淆）
   3. 切配置页守卫：当前页 dirty → askGuard 前置 + `confirmDialog({ title: "未保存的修改", message: "当前配置页有未保存的修改，切换将丢弃这些修改。", kind: "warning" })`（照 FB-09 :174-209 先例含 finally setTimeout 复位）→ 取消不切换；确认 → 清 dirty + 切换
   4. × 关闭守卫（PageDockviewHost.tsx:458）：onClick 包 async——`panel.view.contentComponent === "settings"`（`panel.component` 不存在红线）且 `isSettingsDirty(panel.id)` → `confirmDialog` 确认才 `api.close()`，否则直关
4. **测试同步**：新建 `src/__tests__/settings-panel-dirty.test.tsx`（切页 confirm 确认/取消/非 dirty 直切/圆点显隐）；`src/__tests__/settings-dirty-registry.test.ts`（set/is/clear）；`src/__tests__/workspace-defaulttab.test.tsx` 加 × 拦截用例（settings dirty → confirm 取消不关、确认关；非 settings 面板不经守卫）
5. **文档同步**：workspace/CLAUDE.md × 拦截登记（Stage 07 / SC-DOC-04）
6. **验证**：测试绿

## SC-FE-09 快捷键页 + 录制屏蔽 API（Stage 04）

1. **位置**：`src/features/shortcuts/ShortcutRegistry.ts:181-199`（effectiveKeystroke 旁）；新建 `src/panels/settings/pages/KeybindingsPage.tsx`；`src/features/settingsCenter/pages.ts` 追加注册
2. **现状**：FB-14（effectiveKeystroke/listCommands/setOverrides verbatim；COMMAND_CATALOG 9 条；useKeybindings setBinding/clearBinding）
3. **修复步骤**：
   1. ShortcutRegistry 新增两公共 API（写死）：
   ```typescript
   /** 录制态屏蔽（F11）：true 时 handleKeyDown/resolve 不消费任何按键——
       快捷键设置页录制期间置位，防录制键触发命令（如录 Ctrl+Shift+C 真执行复制） */
   private captureSuspended = false;
   setCaptureSuspended(suspended: boolean): void { this.captureSuspended = suspended; }
   // handleKeyDown 与 resolve 起始加：if (this.captureSuspended) return false;
   /** 生效键查询（设置页显示与运行期同源，防显示/运行漂移）：null=解绑或无默认键 */
   getEffectiveKeystroke(id: string): string | null {
     const cmd = this.commands.get(id);
     if (!cmd) return null;
     const ks = this.effectiveKeystroke(cmd);
     return ks ? formatKeystroke(ks) : null;
   }
   ```
   2. KeybindingsPage：`listCommands()` 按 category 分组（目录序 global/terminal/editor/explorer）；行 = title + 生效键（`hasOwnProperty(overrides,id)` → 高亮 + ↺ + 默认键小字；`getEffectiveKeystroke(id)===null` → 「未绑定」占位）
   3. 录制态：行点击进入（行高亮「按下新键位…Esc 取消」）；录制中挂 window keydown capture 监听：`isComposing` 跳过 / Escape 取消 / Backspace|Delete → `setBinding(id, null)` 解绑 / 纯修饰键（code 为 Control\*/Shift\*/Alt\*/Meta\*）忽略 / 其余构造 KeyStroke → `isReserved(ks, cmd.context)` → 行内红字拒绝 / `findConflict`（同 context 他命令 `getEffectiveKeystroke` 相同）→ 警告「与 XX 冲突，生效按优先级派发」但允许写入 / 合法 → `setBinding(id, formatKeystroke(ks))`；录制开始 `setCaptureSuspended(true)`，结束/取消/卸载 `false`
   4. `findConflict(commands, getEffective, id, keystroke)` 页内纯函数导出（单测）
   5. pages.ts 注册 `{ id: "keybindings", title: "快捷键", group: "global", order: 10 }`
4. **测试同步**：新建 `src/__tests__/settings-keybindings.test.tsx`（分组渲染 / override 高亮+默认小字 / 未绑定占位 / 录制 Esc 取消 / Backspace 解绑 / 纯修饰键忽略 / 保留键红字不写入 / 冲突警告放行 / 合法写入 setBinding / ↺ clearBinding / 卸载清 suspended）；`src/__tests__/shortcuts.test.ts` 加 2 例（suspended 时 handleKeyDown 不消费 / resolve 返回 false）；`src/__tests__/command-catalog.test.ts` 不动（9 条无增删）
5. **文档同步**：shortcuts/CLAUDE.md（Stage 07 / SC-DOC-04：可视化 UI 落地 + 两 API 登记）
6. **验证**：测试绿

## SC-FE-08 切项目自动关闭（Stage 05）

1. **位置**：`src/panels/settings/SettingsPanel.tsx`（壳 effect）
2. **现状**：无（`api.close()` 可用：dockview-react 8.1.0）
3. **修复步骤**：壳 effect（订阅 `useLayout(s=>s.activePageId)` + `useProjects`）：
   1. ownPageId = params.panelId 去 `settings-` 前缀；ownProjectId = projects 反查
   2. activeProjectId = activePageId 所属项目；`activePageId === null` → 不动（删除末页/启动瞬态，防连锁误关）
   3. activeProjectId 与 ownProjectId 均非空且不同 → dirty 守卫（isSettingsDirty → confirmDialog，取消则不关——面板暂留非活跃项目，尊重用户选择，文档注明）→ `api.close()`
   4. **初始评估**：挂载时 activeProjectId 已定且不一致（布局恢复场景）→ 直接 `api.close()` 静默（新挂载不可能 dirty）
4. **测试同步**：新建 `src/__tests__/settings-panel-autoclose.test.tsx`（切项目 → api.close 调用 / 同项目切页 → 不关 / 初始不一致 → 挂载即静默关 / activePageId null → 不关 / dirty confirm 取消 → 不关）
5. **文档同步**：panels/CLAUDE.md（Stage 07 / SC-DOC-04）
6. **验证**：测试绿

## SC-E2E-01 helpers 扩展（Stage 06，agent A）

1. **位置**：`e2e-tests/helpers.ts:265-283`（installSettingsHelpers 旁）
2. **现状**：FB-18（helpers 清单 verbatim）；无设置中心 helper
3. **修复步骤**：新增 `installSettingsPanelHelpers()`：`__slterm_e2e_openSettings(): Promise<void>`（调 openSettings）；`__slterm_e2e_getSettingsPanelState(): { selectedPage: string | null } | null`（经 `__dockviewApi` 查 `settings-` 前缀面板读 params）；`__slterm_e2e_getSettingsPanelCount(): number`（全部页面 api 计数）；`__slterm_e2e_switchSettingsPage(id: string): boolean`（DOM 点击 `settings-nav-${id}`）；installAllE2eHelpers 接线
4. **测试同步**：`src/__tests__/app.test.tsx` helper 契约测试适配（新 helper 存在性）；e2e-gating 测试适配
5. **文档同步**：e2e-tests/CLAUDE.md helper 清单（Stage 07 / SC-DOC-04）
6. **验证**：L2 helper 契约测试绿；`npx vite build` 打包图验证（helpers.ts 在根 tsconfig include 外，构建级门禁兜底）

## SC-E2E-02 settings.e2e.ts + hooks/mockcli 适配（Stage 06，agent B）

1. **位置**：新建 `e2e-tests/settings.e2e.ts`；`e2e-tests/hooks.e2e.ts`、`e2e-tests/mockcli.e2e.ts` 适配
2. **现状**：FB-19（两 spec 消费面 verbatim：程序化 addPanel `component:"hooksConfig"` + `p.component === "hooksConfig"` 清理 + `[data-e2e="hooks-config-panel"]` 选择器）；TE-17 合成键盘先例
3. **修复步骤**：
   1. settings.e2e.ts 用例（完整 L4 决策）：① 配置钮打开（设置面板存在 + 默认全局组第一页）；② 再点 → 单例（count=1）；③ 切页 → params.selectedPage 持久化（helper 读）；④ 频率页 120 失焦 → 真实后端落盘（loadSettings 读段 120）+ 余量刷新；⑤ 频率页 5 → 行内红字 + 文件未变；⑥ 快捷键录制（dispatch 合成 KeyboardEvent Ctrl+Alt+KeyC）→ 生效键更新 + 2s debounce 后落盘断言；⑦ 切项目 → 老面板关闭（count=0）→ 新项目配置钮 → pages[0] 打开；⑧ 同项目切页 → 保留；⑨ hooks 页迁入冒烟（设置中心内 CLI 选择行渲染）；⑩ hooks 页 dirty → 切配置页 → confirm 弹窗 → 取消 → 未切换。**corrupted 警示条 = L2 覆盖（loadSettings mock），L4 豁免登记**（写坏文件需沙箱外写，无命令通道）
   2. hooks.e2e.ts / mockcli.e2e.ts 适配：程序化 addPanel 形态改 `{ id: "settings-e2e-...", component: "settings", title: "设置", params: { panelId, selectedPage: "hooks" } }`；清理分支 `p.component === "hooksConfig"` 改 `"settings"`；`[data-e2e="hooks-config-panel"]` 选择器不变（SC-FE-05 决策：HooksSettingsPage 根容器保留该 data-e2e）；`__slterm_e2e_setHooksConfigJson` 选择器 `[data-e2e="hooks-json-editor"]` 不变（JsonMode 挂载点 data-e2e 随组件迁移保留，FB-18 实证）
4. **测试同步**：本项即测试
5. **文档同步**：test-inventory.md L4 段（Stage 07 / SC-DOC-05）
6. **验证**：`npm run e2e` 绿（build:e2e + wdio 串行；VITE_E2E=1 门控）

## SC-DOC-01 需求规格回写（Stage 07，agent A）

1. **位置**：`docs/settings-center-requirements.md` §4.1/§4.2/§4.7/§5.2/§7
2. **现状**：R1/R2/R3 三处原文（见本清单顶部修订表）
3. **修复步骤**：§4.1「无项目也可打开…」改「无项目 → toast『请先创建项目』」；§4.2 项目组禁用条目删除；§4.7 表同步；§5.2「AppState 原子量」改「plan_balance 模块级 static 原子量（读写双方同模块，SNAPSHOT 先例，State 注入不可直调单测）」；§7 验收 2 改 toast 分支；决策记录补 R1–R3 三行
4. **测试同步**：无
5. **文档同步**：本项即文档
6. **验证**：`grep -n "请先创建项目\|模块级 static" docs/settings-center-requirements.md` 命中；旧措辞零残留

## SC-DOC-02 CONTEXT.md + 根 CLAUDE.md（Stage 07，agent A）

1. **位置**：`CONTEXT.md` 面板系统/侧栏/Hooks 宿主节；`.claude/CLAUDE.md` 需求索引/模块索引
2. **现状**：CONTEXT.md:26-27（面板类型 6 种列举含 hooksConfig）、:102（活动栏配置钮=hooks 配置面板入口）、:227-228（双模式面板）；根 CLAUDE.md 模块索引含 `src/features/hooksConfig` 行
3. **修复步骤**：CONTEXT.md 新增术语（设置中心/配置页/全局组/项目组/前端消费型/后端消费型——按规格 §2）；面板类型列举 hooksConfig→settings；活动栏配置钮描述改「设置中心唯一入口（无项目 toast）」；双模式面板条目指向设置中心 hooks 页。根 CLAUDE.md：需求索引加 F11 行；模块索引 `-src/features/hooksConfig` 行删除、`+src/features/settingsCenter` 行新增
4. **测试同步**：无
5. **文档同步**：本项即文档
6. **验证**：`grep -n "F11\|settingsCenter" .claude/CLAUDE.md` 命中；`grep -n "hooksConfig" CONTEXT.md` 仅余历史语境（布局过滤说明处可无）

## SC-DOC-03 ADR-0012（Stage 07，agent A）

1. **位置**：`.claude/adr.md` 末尾
2. **现状**：末条 0011
3. **修复步骤**：追加 `## 0012 设置中心（统一配置入口 + 配置页注册表 + 后端轻量收口）`——上下文（配置钮直达单一面板无法承载两类配置）/ 决策（Dockview 面板左导航+SettingsPageRegistry；后端三段式；写入通道二分；切项目自动关闭+全局单例；无项目 toast）/ 被否决（模态/独立窗口/侧栏视图/完整后端注册表/inventory 自注册/Ctrl+, 键盘入口）/ 后果（新增配置页=注册一条；hooksConfig 类型退役；F10 豁免口径更新）
4. **测试同步**：无
5. **文档同步**：本项即文档
6. **验证**：`grep -n "## 0012" .claude/adr.md` 命中

## SC-DOC-04 模块 CLAUDE.md 群（Stage 07，agent B）

1. **位置**：`src/panels/CLAUDE.md`（hooksConfig 节→settings 节重写：壳+注册表分派+dirty 汇聚+自动关闭+isAlwaysRenderPanel 排除决策；「添加新面板类型的步骤」第 2 步失实修正——无 src/panels/index.ts，直接 panelRegistry 注册）；新建 `src/features/settingsCenter/CLAUDE.md`（存在理由/家族契约/触发点/openSettings 编排/无项目 toast）；删 `src/features/hooksConfig/CLAUDE.md`（schema 单点 MC-223/P3-FE-07/TE-09/TE-15 并入 cliProfiles/CLAUDE.md）；`src/features/cliProfiles/CLAUDE.md:52`（KZ-1 重写：编辑器归域 configEditor/，不再跨 panels 引用）；`src/features/shortcuts/CLAUDE.md`（可视化 UI 落地 + setCaptureSuspended/getEffectiveKeystroke 登记）；`src/features/sideViews/CLAUDE.md`（配置钮→openSettings+toast）；`src/ipc/CLAUDE.md`（planBalance 四命令）；`src-tauri/src/CLAUDE.md`（白名单聚合：前端消费型四键集中 + 后端消费型归域先例）；`src-tauri/src/plan_balance/CLAUDE.md`（动态间隔+新命令）；`src/workspace/CLAUDE.md`（openSettingsPanel+× 拦截）；`src/__tests__/CLAUDE.md`（测试文件迁移清单登记）
2. **现状**：各文件现状见取证（panels/CLAUDE.md hooksConfig 节、shortcuts「后续 feature」、ipc planBalance 三命令、cliProfiles/CLAUDE.md:52 KZ-1 跨 panels 声明等）
3. **修复步骤**：按上述逐文件改写/新建/删除（ADR-0011 代码自证原则：只记 why/红线/登记，不复述职责文件表）
4. **测试同步**：无
5. **文档同步**：本项即文档
6. **验证**：`grep -n "hooksConfig" src/panels/CLAUDE.md src/features/shortcuts/CLAUDE.md src/ipc/CLAUDE.md` 仅历史语境；新文件存在且经模板（存在理由→约束决策→红线→测试模式）

## SC-DOC-05 test-inventory.md 校准（Stage 07，agent B）

1. **位置**：`.claude/test-inventory.md` 表头/豁免表/L1/L2/L4 段
2. **现状**：全量计数（Rust + 前端 + L3 + E2E 40）；F10 豁免行（poller 本体豁免）
3. **修复步骤**：实跑四级测试按登记纪律校准三处计数（表头/段头/段小计一致）；F10 豁免行口径更新（「轮询任务本体」扩注动态间隔内存读取）；新增豁免行「settings.json corrupted 警示条 L4」（无沙箱外写坏文件通道，L2 覆盖）；L4 段加 settings.e2e.ts 行；L2 段：删 open-hooks-config 两行、hooks-config-panel 改 settings-hooks-page、新增 7 文件行（settings-page-registry / open-settings / open-settings-panel / settings-panel / settings-plan-balance / settings-hooks-page / settings-panel-dirty / settings-dirty-registry / settings-keybindings / settings-panel-autoclose 按实际新增登记）
4. **测试同步**：无
5. **文档同步**：本项即文档
6. **验证**：表头总数==各段小计之和；`grep -n "settings.e2e" .claude/test-inventory.md` 命中
