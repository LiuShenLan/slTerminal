# F11 设置中心 Review 修复清单（settings-center-fixes）

> 输入：`docs/settings-center-review/` 三份分报告 + review-00-summary.md（15 条）+ 根因修复链新增项。
> 组织方式：不用 P0-P4，优先级由 stages.md 的 Stage 依赖顺序表达。
> 编号：BE（后端）/ FE（前端）/ TE（测试）/ DOC（文档）。全部修复点现状均已计划期实读原文（非凭 review 报告转述）。

## 根因终版（两层）

1. **直接根因（翻案结论）**：当前 `target/debug/slterminal.exe` 是 E2E 构建（F11 收尾 04:52 `npm run e2e` 产物覆盖，`dist/assets/helpers-rvpJaFqp.js` 含 `__slterm_e2e_workspaceReady` 实锤——普通构建 tree-shake 掉 helpers chunk）。用户日常跑此 exe → `VITE_E2E === "1"` → `App.tsx:84` 跳过 `loadAllProjects` → store 空 + `:91` 无条件 `markPersistenceReady()` → 关闭链 `saveAllProjects` 空写覆盖磁盘。物证时间线：exe 04:52:15 → projects.json 12:27:52 被空写为 118B（.bak 11:34:19 留存完整 version 17 数据，已恢复）。
2. **深层缺陷**：`loadFromDisk`/`loadAllProjects` 双层静默 catch + 无条件 `markPersistenceReady` + 无空写守卫 = 任何加载失败路径都会自杀式清空磁盘数据。无论触发点为何都必须修。

## 清单 ↔ review 问题对照（合并留痕）

| 清单 ID | review-00 问题 # |
|---------|------------------|
| FE-01 + FE-02 + BE-02 | #1（根因链拆三处落点）+ 附带发现 1 |
| FE-03 | #3（附带发现 2 同源） |
| DOC-04 | #4 |
| DOC-01 | #2 |
| DOC-02 | #5 |
| TE-04 | #6 |
| TE-05 + TE-06 | #7（短路用例归 TE-05，合并/不可变归 TE-06） |
| TE-06 | #8 |
| TE-03（用例⑪） | #9 |
| TE-03（用例⑧） | #10 |
| TE-05 | #11 |
| TE-03（路径）+ TE-02 | #12（SLTERM_DATA_DIR 一并消解备份集合不一致） |
| DOC-05 | #13 |
| DOC-03 | #14 |
| FE-04（注释半） | #15 |
| BE-01 | 根因修复链新增（E2E/日常数据隔离） |

---

## BE-01 app_dir.rs 增加 SLTERM_DATA_DIR 环境变量覆盖

1. **位置**：`src-tauri/src/app_dir.rs:72-81`（`app_data_dir()`）
2. **现状**：
   ```rust
   pub(crate) fn app_data_dir() -> Result<PathBuf, AppError> {
       #[cfg(test)]
       {
           if let Some(dir) = APP_DATA_DIR_OVERRIDE.lock().unwrap().clone() {
               return Ok(dir);
           }
       }
       resolve_app_data_dir(std::env::current_exe())
   }
   ```
3. **修复步骤**：
   - 文件顶部常量区新增：`const DATA_DIR_ENV: &str = "SLTERM_DATA_DIR";`
   - `app_data_dir()` 在 `#[cfg(test)]` 块之后、`resolve_app_data_dir` 之前插入：
     ```rust
     // E2E 隔离：环境变量显式指定数据目录（空串视为未设置）
     if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|v| !v.is_empty()) {
         return Ok(PathBuf::from(dir));
     }
     ```
   - 优先级语义写死：测试 guard（`cfg(test)`，生产零编译）> 环境变量 > exe 同级推导。
4. **测试同步**：同文件测试模块新增 3 例：① `SLTERM_DATA_DIR` 生效返回指定目录；② 空串被忽略回落 exe 推导；③ 测试 guard 优先于 env（guard 与 env 同设时返回 guard 值）。每例结束 `std::env::remove_var(DATA_DIR_ENV)`（Cargo edition 2021，`set_var` 非 unsafe；L1 强制 `--test-threads=1` 无竞态）。
5. **文档同步**：`src-tauri/src/CLAUDE.md` app_dir 段补「数据目录三级来源：测试 guard > `SLTERM_DATA_DIR` env > exe 同级」（DOC-06 合并执行）。
6. **验证**：`cargo test --manifest-path src-tauri/Cargo.toml app_dir -- --test-threads=1` 全绿；`grep -rn "app_data_dir()" src-tauri/src --include="*.rs"` 确认消费方仅 settings.rs×2 / projects.rs×2 / plan_balance/mod.rs:190（隔离后 E2E 不再污染 exe 同级 settings.json 与 slterminal-projects.json；plan_balance 读 E2E 目录不存在的 settings.json 走原有 fallback，无害）。

## BE-02 projects.rs 命令层补 tracing 打点

1. **位置**：`src-tauri/src/projects.rs:57-88`（`load_from_dir`）、`:121-141`（命令层）
2. **现状**：`load_from_dir` 四分支（含 .bak 双保险）静默返回默认值；命令层无 tracing。
3. **修复步骤**：纯打点零行为变化——读失败非 NotFound 分支加 `tracing::warn!(error = %e, path = %path.display(), "projects 读取失败，尝试 .bak")`；JSON 非法分支 warn；损坏且 .bak 未命中分支 `tracing::error!`。
4. **测试同步**：无新增（行为不变）；既有 `cargo test projects` 保持绿。
5. **文档同步**：无。
6. **验证**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零告警；`grep -c "tracing::" src-tauri/src/projects.rs` ≥ 3。

## FE-01 stores/projects.ts 数据防线（loadSucceeded + 空写守卫 + 上抛 + 结构校验）

1. **位置**：`src/stores/projects.ts:233-257`（loadFromDisk）、`:259-264`（saveToDisk）、`:272-279`（loadAllProjects）、`:291-322`（模块标志区）
2. **现状**：`loadFromDisk` try/catch 吞异常仅 console.warn；`loadAllProjects` 第二层静默 catch；`saveToDisk` 无任何守卫。
3. **修复步骤**（可照抄代码）：
   - `:292` 后新增模块标志：`let loadSucceeded = false;`
   - `loadFromDisk`：删 try/catch 让异常上抛；`JSON.parse` 后、set 前插结构校验：
     ```ts
     // 结构校验：projects 字段存在时必须是对象（防损坏数据进 store）
     if (data.projects !== undefined &&
         (typeof data.projects !== "object" || data.projects === null || Array.isArray(data.projects))) {
       throw new Error("项目数据格式异常：projects 字段不是对象");
     }
     ```
     set(...) 成功后 `loadSucceeded = true;`
   - `saveToDisk` 首行后插守卫：
     ```ts
     // 空写守卫：未成功加载且当前为空时拒写，防加载失败路径空写覆盖磁盘数据
     if (!loadSucceeded && Object.keys(projects).length === 0) {
       console.warn("[slTerminal] 拒绝空写：项目数据未成功加载且当前为空（防覆盖磁盘数据）");
       return;
     }
     ```
   - `loadAllProjects`：删 catch 直传 `await useProjects.getState().loadFromDisk();`
   - 新增导出（供 E2E 分支与「以空状态继续」显式放行）：`export function markLoadSucceeded(): void { loadSucceeded = true; }`
   - `_resetPersistence` 内加 `loadSucceeded = false;`
   - 放行语义：loadFromDisk 成功自动置位；E2E 分支/「以空状态继续」经 `markLoadSucceeded()` 显式放行；删除最后一个项目时 loadSucceeded 已为 true 自然放行（无需 allowEmpty 参数）。
4. **测试同步**：`src/__tests__/projects.test.ts` 新增 5 例（beforeEach 已调 `_resetPersistence`）：① loadFromDisk IPC reject → 异常上抛且 loadSucceeded 未置位（后续空写被拒）；② 结构校验：返回 `{projects: 1}` → throw 格式异常；③ loadFromDisk 成功 → saveAllProjects 空状态正常写盘；④ 未加载时空写被拒且磁盘无写入调用；⑤ `markLoadSucceeded()` 后空写放行。
5. **文档同步**：`src/stores/CLAUDE.md` 补 loadSucceeded 口径（DOC-06 合并执行）。
6. **验证**：`npm test -- projects.test` 全绿；`grep -n "markLoadSucceeded" src/stores/projects.ts` 命中定义与导出；`grep -c "loadSucceeded" src/stores/projects.ts` ≥ 5。

## FE-02 App.tsx 启动链阻断 + 错误页（重试 / 以空状态继续）

1. **位置**：`src/App.tsx:75-91`（init 加载段）、`:257-277`（`if (!ready)` 加载页）
2. **现状**：`:75-91` `if (import.meta.env.VITE_E2E !== "1") { await loadAllProjects(); }` catch 仅 console.warn，`:91` 无条件 `markPersistenceReady()`；`:257-277` 加载页为纯「slTerminal 启动中…」文本（startup-restore 用例 4/8 断言该文本与 style，须保持）。
3. **修复步骤**（可照抄代码）：
   - state 区新增：`const [projectsLoadError, setProjectsLoadError] = useState<string | null>(null);`
   - 抽取 `loadProjectsAndRestore()` async 函数：E2E 分支（`VITE_E2E === "1"`）改调 `markLoadSucceeded()`；成功路径 `markPersistenceReady()` + lastPage 恢复逻辑原样保留（DBG-6 顺序不动：先 await setProjectRoot 再 setActivePage）。
   - init 的 catch 改为：`console.error("[App] 加载项目数据失败:", err); setProjectsLoadError(err instanceof Error ? err.message : String(err)); return;`（**不** setReady、**不** markPersistenceReady——阻断写门控）。
   - 新增 `retryProjectsLoad`：`setProjectsLoadError(null)` → 重跑 `loadProjectsAndRestore()` → 成功 `setReady(true)`。
   - 新增 `continueWithEmptyProjects`：`markLoadSucceeded(); markPersistenceReady(); setReady(true);`（用户显式选择空状态，放行写盘）。
   - 加载页 JSX 改条件渲染：`projectsLoadError === null` 时原「启动中…」分支逐字不变（文本节点与 div 样式不动）；否则渲染错误页：
     - 容器 `data-e2e="projects-load-error"`，错误文案「项目数据加载失败」+ 错误详情 + 说明「可选择重试，或以空项目状态继续（磁盘上的项目数据不会被覆盖）」
     - 两按钮：`data-e2e="projects-load-retry"`（文案「重试」）/ `data-e2e="projects-load-continue-empty"`（文案「以空状态继续」）
     - 样式沿用主题 token：`PANEL_BG` 底 + `SECONDARY_BG` 按钮底 + `SEPARATOR_BG` 1px 边框 + `DIM_FG` 13px（theme/colors.ts 均已导出，禁止硬编码）。
4. **测试同步**：`src/__tests__/startup-restore.test.ts`（TE-01 执行）：用例 3 语义反转（mockRejectedValueOnce → 断言不 ready / markPersistenceReady 未调 / console.error 被调 / 错误页渲染）；新增 3 例：错误页两按钮存在、点重试成功进 ready、点「以空状态继续」进 ready 且写盘放行。
5. **文档同步**：无（行为恢复属修复）。
6. **验证**：`npm test -- startup-restore` 全绿；`grep -c 'data-e2e="projects-load-' src/App.tsx` ≥ 3；「启动中…」文本节点仍在 `projectsLoadError === null` 分支（Read 确认）。

## FE-03 SettingsPanel SC-FE-08 effect 水合门控

1. **位置**：`src/panels/settings/SettingsPanel.tsx:350-398`（自动关闭 effect）
2. **现状**：effect 对 store 水合时序敏感——projects 未水合（`{}`）时首轮评估可能把 firstRun 误消费为「变化触发」走 dirty 弹窗分支。
3. **修复步骤**：`:360` 后、`:363` firstRun 消费前插入：
   ```tsx
   // ownProjectId===null && projects 空 = 未水合，保留 firstRun 待重跑
   // （防误消费为「变化触发」致 dirty 分支误弹窗）
   if (ownProjectId === null && Object.keys(projects).length === 0) return;
   ```
   判据语义写死：`ownProjectId===null` 且 projects 非空 = 项目已删，维持现状逻辑 return 不拦截。
4. **测试同步**：`src/__tests__/settings-panel-autoclose.test.tsx` 新增用例：空 projects 首轮评估不关闭不消费 firstRun；水合后重跑走初始评估静默关闭。
5. **文档同步**：无。
6. **验证**：`npm test -- settings-panel-autoclose` 全绿；`grep -n "未水合" src/panels/settings/SettingsPanel.tsx` 命中。

## FE-04 PageDockviewHost × 按钮 data-e2e + 注释修正

1. **位置**：`src/workspace/PageDockviewHost.tsx:459-479`（× button）、`:422-425`（filePath 判据注释）
2. **现状**：× button 无 data-e2e（SC-FE-07 dirty 守卫在此，L4 用例⑪无法定位）；`:423` 注释残留已退役的 `hooksConfig` 字样。
3. **修复步骤**：× button 加 data-e2e 属性（照抄代码块）：
   ```tsx
   data-e2e={tabParams?.panelId ? `tab-close-${tabParams.panelId}` : "tab-close"}
   ```
   注释「terminal/hooksConfig 恒不设置」改为「terminal/settings 恒不设置」。两处同文件合并一项。
4. **测试同步**：既有 `workspace-defaulttab.test.tsx` 保持绿（仅加属性不改行为）。
5. **文档同步**：无（注释修正即文档）。
6. **验证**：`grep -n "tab-close-" src/workspace/PageDockviewHost.tsx` 命中；`grep -n "hooksConfig" src/workspace/PageDockviewHost.tsx` 零命中；`npm test -- workspace-defaulttab` 全绿。

## TE-01 启动链测试适配（语义反转 + 五文件 mock 补全）

1. **位置**：`src/__tests__/startup-restore.test.ts:120-123`（mock 工厂）、`:186-205`（用例 3）；`src/__tests__/startup-store-fail-warn.test.tsx:96-99`、`close-handler.test.ts:172-175`、`error-boundary.test.tsx:22-24`、`e2e-clipboard-helper.test.ts:44-46`（四处 mock 工厂）
2. **现状**：五处 `vi.mock` projects store 工厂未含新导出 `markLoadSucceeded`（FE-01 新增后 mock 缺导出会运行时 undefined 报错）；startup-restore 用例 3 断言「静默降级仍 ready」——与 FE-02 新语义相反。
3. **修复步骤**：
   - 五处 mock 工厂各补 `markLoadSucceeded: vi.fn()`（startup-restore 的在 :120-123 工厂对象内）。
   - startup-restore 用例 3 语义反转：mockRejectedValueOnce 后断言——`ready` 不置位 / `markPersistenceReady` 未被调用 / `console.error` 被调 / 渲染出 `projects-load-error` 错误页。
   - 新增 3 例（与 FE-02 测试同步同一组，本项执行）：错误页两按钮渲染、重试成功路径、以空状态继续路径。
4. **测试同步**：本项即测试改动本身。
5. **文档同步**：无。
6. **验证**：`npm test -- startup-restore startup-store-fail-warn close-handler error-boundary e2e-clipboard-helper` 五文件全绿；`grep -rn "markLoadSucceeded" src/__tests__/` 命中 ≥ 6 处（五 mock + projects.test.ts 用例）。

## TE-02 run-wdio.cjs SLTERM_DATA_DIR 隔离 + 清理失实备份

1. **位置**：`e2e-tests/run-wdio.cjs:70-71`（`~/.slterminal/settings.json` 备份——失实对象）、`:104-121`（E2E-16 projects.json+.bak 备份/清空段）、`:135-151`（还原段）、`:152-155`（settings 还原段）
2. **现状**：备份 ~/.slterminal/settings.json（BE-16 便携化后应用读写 exe 同级，后端 grep 实证无此路径写入点）；projects.json 备份三件套在 SLTERM_DATA_DIR 隔离后冗余。
3. **修复步骤**：
   - 文件头常量区新增：
     ```js
     // E2E 数据目录隔离：应用全部数据写入（settings.json / slterminal-projects.json）
     // 经 SLTERM_DATA_DIR 指向临时目录，与日常使用数据完全隔离（BE-01）
     const e2eDataDir = path.join(os.tmpdir(), 'slterm-e2e-data');
     fs.rmSync(e2eDataDir, { recursive: true, force: true });
     fs.mkdirSync(e2eDataDir, { recursive: true });
     process.env.SLTERM_DATA_DIR = e2eDataDir;
     ```
     位置：`fallback()` spawn wdio 之前（env 链式继承：run-wdio → npx wdio → tauri driver → slterminal.exe，wdio.conf.ts:26 未覆盖 env 故继承）。
   - 删除 :70-71 / :104-121 / :135-151 / :152-155 四段备份还原代码。
   - 进程退出清理 `fs.rmSync(e2eDataDir, { recursive: true, force: true })`。
   - 文件头注释同步新隔离机制。
4. **测试同步**：L4 实跑即验证（settings.e2e.ts 用例④落盘断言走 env 路径成功 = 链路通的证明）。
5. **文档同步**：`e2e-tests/CLAUDE.md` 更新备份清单与新隔离机制说明（DOC-04 执行）。
6. **验证**：`grep -n "SLTERM_DATA_DIR" e2e-tests/run-wdio.cjs` 命中；`grep -n "slterminal-projects" e2e-tests/run-wdio.cjs` 零命中（备份段已删）；settings spec 实跑全绿。

## TE-03 settings.e2e.ts + helpers.ts 补强（路径 env 化 / 用例⑧强化 / 用例⑪新增）

1. **位置**：`e2e-tests/settings.e2e.ts:64`（settingsJsonPath 硬编码）、`:575-606`（用例⑧）；`e2e-tests/helpers.ts:305-344`（installSettingsPanelHelpers）
2. **现状**：`:64` 硬编码 `join(process.cwd(), "src-tauri", "target", "debug", "settings.json")`；用例⑧只断言 DOM 面板数 + selectedPage，未断言 panelId 归属；× 关闭 dirty 守卫无 L4 覆盖；`:213-243` suite 级快照还原段保留（直跑 wdio 无 env 时兜底保护 exe 同级，路径改 env 推导后自动跟随）。
3. **修复步骤**：
   - `:64` 改为 `join(process.env.SLTERM_DATA_DIR ?? join(process.cwd(), "src-tauri", "target", "debug"), "settings.json")`。
   - helpers.ts `getSettingsPanelState` 返回值加 `panelId: panel.id`；新增后门 helper：
     ```ts
     // 测试后门：直接置设置面板 dirty 态（供 × 关闭守卫用例绕过真实编辑）
     window.__slterm_e2e_setSettingsDirty = (panelId, dirty) => { setSettingsDirty(panelId, dirty); };
     ```
     （import `setSettingsDirty` from `src/features/settingsCenter/dirtyRegistry`）。
   - 用例⑧补断言：切页前后 helper 读到的 `panelId` 恒为 `settings-${pageIdA}`。
   - 新增用例⑪ × 关闭 dirty 守卫：createProject → openSettingsCenter → 后门置 dirty → 点 `[data-e2e="tab-close-settings-{pageId}"]`（FE-04 提供）→ `confirm-dialog` 出现 → `confirm-cancel` 面板保留 → 再点 × → `confirm-ok` 面板关闭。ConfirmDialog 选择器实证：`confirm-dialog-mask`/`confirm-dialog`/`confirm-cancel`/`confirm-ok`（lib/ConfirmDialog.tsx:129/145/177/193）。
4. **测试同步**：本项即 L4 测试改动；test-inventory.md L4 用例数 50→51（DOC-07）。
5. **文档同步**：`e2e-tests/CLAUDE.md` helper 说明补 `__slterm_e2e_setSettingsDirty`（DOC-04）。
6. **验证**：`npm run build:e2e` 成功 + settings spec 实跑 11 用例全绿（e2e-tests 无 tsconfig，tsc include 外——构建级门禁强制）。

## TE-04 settings-pages-registration.test.ts 新建

1. **位置**：新建 `src/__tests__/settings-pages-registration.test.ts`；被测 `src/features/settingsCenter/pages.ts`
2. **现状**：settings-panel.test.tsx:19 把 pages mock 成 `{}`，真实注册（keybindings global:10 / planBalance global:20 / hooks project:100）在 L2 从未验证。
3. **修复步骤**：新文件——不 mock pages，mock 三个页面组件为 `() => null`（组件实现不属本测试面），import 真实 `pages.ts`（side-effect 注册），断言 `getSettingsPageRegistry().getAll()` 精确包含三条 `{id, group, order}`；`afterEach` 调 `_reset()`（硬约束 #13 注册表契约）。
4. **测试同步**：本项即新测试；test-inventory.md L2 +1。
5. **文档同步**：`src/__tests__/CLAUDE.md` 新增清单补此文件（DOC-03）。
6. **验证**：`npm test -- settings-pages-registration` 全绿；新文件内 `vi.mock` 调用不含 pages 自身路径。

## TE-05 settings-hooks-page 短路用例 + 两文件死 mock 清理

1. **位置**：`src/__tests__/settings-hooks-page.test.tsx:40-52`（死 mockApi/mockContainerApi）、`:782-796`（selectedCli 用例区）；`src/__tests__/hooks-config-sync.test.tsx:56-67`（死 mock）
2. **现状**：mockApi/mockContainerApi 定义后从未作 props 传入（Grep 实证仅 reset 使用）= 迁移遗留死代码；persistSelectedCli 丢失「点击已选中 CLI 短路」用例（HooksSettingsPage.tsx:132 已有短路 `if (cliId === selectedCliRef.current) return;`，可测）。
3. **修复步骤**：两文件删 mockApi/mockContainerApi 定义与全部 reset 引用；settings-hooks-page.test.tsx 新增用例「点击当前已选中 CLI 不触发 onPageParamsChange」（渲染 selectedCli="claude" → 点击 claude 项 → 断言 `mockOnPageParamsChange` 零调用）。
4. **测试同步**：本项即测试改动；test-inventory.md L2 +1。
5. **文档同步**：无。
6. **验证**：`npm test -- settings-hooks-page hooks-config-sync` 全绿；`grep -n "mockContainerApi" src/__tests__/settings-hooks-page.test.tsx src/__tests__/hooks-config-sync.test.tsx` 零命中。

## TE-06 settings-panel.test.tsx 补 saveLayout 落盘断言 + 不可变用例

1. **位置**：`src/__tests__/settings-panel.test.tsx:248-270`（pageParams patch 用例）
2. **现状**：只断言 `api.updateParameters` 合并参数，未断言 `containerApi.toJSON`（saveLayout 触发源）——存在「只改 params 不落盘」假绿空间。
3. **修复步骤**：pageParams patch 用例补 `expect(containerApi.toJSON).toHaveBeenCalled()`；新增用例「onPageParamsChange 合并既有 params 且不修改原对象」（renderPanel 携带初始 params → 触发 patch → 断言合并结果含原键 + 原对象引用未被改写）。
4. **测试同步**：本项即测试改动；test-inventory.md L2 +1。
5. **文档同步**：无。
6. **验证**：`npm test -- settings-panel` 全绿。

## DOC-01 需求规格 :189 schema 单点失实改写

1. **位置**：`docs/settings-center-requirements.md:189`
2. **现状**：「`src/features/hooksConfig/`：schema 单点保留（claude 专属资产，MC-223 语义不变），`openHooksConfig.ts` 删除。」——实际目录已整体删除，schema 在 `src/features/cliProfiles/profiles/claude/configEditor/schema/`。
3. **修复步骤**：改为「`src/features/hooksConfig/`：已整体删除；schema 单点迁入 `src/features/cliProfiles/profiles/claude/configEditor/schema/`（MC-223 语义不变）。」
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：`grep -n "schema 单点保留" docs/settings-center-requirements.md` 零命中。

## DOC-02 需求规格 :170 面板注册流程失实改写

1. **位置**：`docs/settings-center-requirements.md:170`
2. **现状**：五步含不存在的 `panels/index` barrel（panels/CLAUDE.md:200-204 明示无 barrel）。
3. **修复步骤**：改为「目录 → `panelRegistry.ts` 注册 → `PANEL_TYPES` 追加；如涉及新 IPC 命令再追加 capabilities」。
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：`grep -n "panels/index" docs/settings-center-requirements.md` 零命中。

## DOC-03 __tests__/CLAUDE.md 新增清单补 3+1 文件

1. **位置**：`src/__tests__/CLAUDE.md:69`
2. **现状**：F11 新增清单缺 `open-settings.test.ts`、`open-settings-panel.test.ts`、`settings-hooks-page.test.tsx`；TE-04 又新增 `settings-pages-registration.test.ts`。
3. **修复步骤**：清单补齐上述 4 文件。
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：四文件名在 `src/__tests__/CLAUDE.md` 新增清单逐一 grep 命中。

## DOC-04 report.md 偏离登记 + e2e-tests/CLAUDE.md 同步

1. **位置**：`docs/settings-center/report.md` 第四节（偏离登记）；`e2e-tests/CLAUDE.md`（helper 说明 / 备份清单 / 隔离机制）
2. **现状**：`__slterm_e2e_getSettingsPanelCount` 只计活跃页面 api 与 checklist「全部页面」约定不符，未登记；e2e-tests/CLAUDE.md 备份清单含已删除的 projects 三件套备份与失实 ~/.slterminal/settings.json 备份，无 SLTERM_DATA_DIR 机制说明，无 E2E 构建误跑警示。
3. **修复步骤**：report.md 补第 4 项偏离（helper 计数范围收窄为活跃页面，理由：设置中心全局单例功能等价，本修复计划已收口径）；e2e-tests/CLAUDE.md 更新备份清单（删三段 + 新增 SLTERM_DATA_DIR 隔离说明 + helper 清单补 `__slterm_e2e_setSettingsDirty` + 警示「target/debug 的 exe 可能是 E2E 构建，日常使用须以普通 `npx tauri build --debug --no-bundle` 覆盖」）。
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：`grep -n "getSettingsPanelCount" docs/settings-center/report.md` 命中；`grep -n "SLTERM_DATA_DIR" e2e-tests/CLAUDE.md` 命中。

## DOC-05 types.ts SettingsPageProps mount 约定标注

1. **位置**：`src/features/settingsCenter/types.ts:13-17`（SettingsPageProps）
2. **现状**：无「mount 期禁止调用 onPageParamsChange」约定标注，未来新增页组件有误触发保存风险。
3. **修复步骤**：`onPageParamsChange` 字段加注释「**约定**：组件 mount/首渲染期禁止调用（仅响应用户交互调用）——mount 期调用会误触发布局保存」（中文注释规范）。
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：`grep -n "mount" src/features/settingsCenter/types.ts` 命中。

## DOC-06 stores/CLAUDE.md + settings.rs:1 注释 + src-tauri/src/CLAUDE.md 三处口径

1. **位置**：`src/stores/CLAUDE.md`（projects store 段）；`src-tauri/src/settings.rs:1`；`src-tauri/src/CLAUDE.md`（app_dir 段）
2. **现状**：stores/CLAUDE.md 无 loadSucceeded 防线说明；settings.rs:1 注释仍写「~/.slterminal/settings.json」（BE-16 便携化后实际 exe 同级，失实）；app_dir 段无 env 覆盖说明。
3. **修复步骤**：stores/CLAUDE.md 补「projects 持久化防线：loadFromDisk 成功置 loadSucceeded；未成功加载且 store 空时 saveToDisk 拒写（防空写覆盖磁盘）；显式放行经 markLoadSucceeded()（E2E 分支/用户选择空状态继续）」；settings.rs:1 注释改为 exe 同级口径；src-tauri/src/CLAUDE.md app_dir 段补三级来源（BE-01 文档同步合并于此）。
4. **测试同步**：无。
5. **文档同步**：本项即文档。
6. **验证**：`grep -n "loadSucceeded" src/stores/CLAUDE.md` 命中；`grep -n "~/.slterminal/settings" src-tauri/src/settings.rs` 零命中；`grep -n "SLTERM_DATA_DIR" src-tauri/src/CLAUDE.md` 命中。

## DOC-07 test-inventory.md 计数收口

1. **位置**：`.claude/test-inventory.md` 表头 / L2 设置中心段 / L4 settings.e2e.ts 段
2. **现状**：全量 3846（Rust 815 + 前端 2839 + L3 142 + E2E 50）。
3. **修复步骤**：按执行期实跑校准——预测 L1 815→818（+3）、L2 2839→2852（+13：FE-01×5 + FE-02 反转组净+3 + FE-03×1 + TE-04×1 + TE-05×1 + TE-06×1 + FE-04 适配 0，以实跑为准）、L4 50→51（+用例⑪）、L3 142 不变；三处一致。
4. **测试同步**：本项即清单同步。
5. **文档同步**：本项即文档。
6. **验证**：三处计数一致且与实跑输出一致；总计 = 四项之和。
