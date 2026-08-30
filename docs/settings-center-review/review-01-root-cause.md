# 设置中心 F11 后「重启项目丢失」根因定位报告

## 摘要

**物证结论**：`src-tauri/target/debug/slterminal-projects.json` 文件完整且为合法 JSON，项目数据（`proj-1788058695715-1`）并未从磁盘消失。重启后导航树为空，说明**启动时项目数据未成功加载进 `useProjects` store**，而非保存阶段把文件写空。

**根因方向**：F11 未改动 `loadAllProjects` / `load_projects` / `save_projects` 的实现，但引入了设置中心面板，并使设置面板持久化到布局中。实测数据文件显示：关闭前布局中保存了 `settings` 面板且 `selectedPage="planBalance"`。当前代码下，设置面板挂载期的自动关闭/持久化链路**不会**清空 `projects` store；真正导致空 store 的是**启动加载路径的静默失败**——失败被多层 `try/catch` 吞掉，应用继续以空状态运行。

由于仅通过静态代码与文件物证无法 100% 复现运行时 IPC/文件系统错误，本报告将**已排除的嫌疑**、**最可能根因**与**区分验证方法**一并给出。

---

## 1. 根因结论

### 1.1 已确证的因果链

1. **磁盘数据完整**  
   物证 `src-tauri/target/debug/slterminal-projects.json:1-78` 包含完整项目：`projects["proj-1788058695715-1"]` 存在，`pages[0]` 存在，`layout` 同时保存了 `terminal` 与 `settings` 两个面板，且 `settings` 面板 `params.selectedPage="planBalance"`。`.bak` 文件（`...10:59:09`）与主文件（`...11:00:37`）均完整，version 分别为 12 / 14。
2. **空 store 的入口唯一**  
   导航树 `src/features/navTree/useNavTree.ts:97` 直接订阅 `useProjects((s) => s.projects)`；`tree.length === 0` 当且仅当 store 中的 `projects` 为空。因此「项目列表空了」等价于 `useProjects` store 在启动后未获得磁盘数据。
3. **加载失败被静默吞掉**  
   - `src/stores/projects.ts:233-256`：`loadFromDisk` 内部 `try/catch` 吞掉所有异常，失败后仅 `console.warn`，store 保持 `{}`。  
   - `src/App.tsx:85-91`：`loadAllProjects()` 的 `catch` 同样静默，随后无条件调用 `markPersistenceReady()`。  
   这意味着只要 `load_projects` IPC 调用、JSON 解析或 `set` 任何一环失败，应用都会以空项目状态继续启动，用户无任何提示。
4. **持久化门控在失败后被打开**  
   `src/stores/projects.ts:295-297,316-322`：`markPersistenceReady()` 将 `initialized` 置 `true`；此后任何 store 变更都会触发 2s debounce 保存。如果后续某个 effect 触发了 `projects` 的变更（如 `updatePageLayout`），空状态会被保存到磁盘，形成「数据自杀链」。

### 1.2 F11 与现象的关联

F11 本身**没有**修改 `projects.rs`、`app_dir.rs`、`src/stores/projects.ts`、`src/App.tsx` 的加载逻辑（`git diff 2154493..825ed56` 证实）。F11 引入的设置中心面板通过以下方式**间接参与**了现场：

- 关闭前布局中保存了 `settings` 面板（`slterminal-projects.json:49-58`），说明用户确实使用过设置中心。
- 设置中心壳 `SettingsPanel.tsx` 的 `persistParams` 会调用 `handleLayoutPersist` → `useProjects.getState().updatePageLayout`，导致 `version` 递增（文件 version=14，说明关闭前有多轮布局保存）。

但**设置面板挂载期不会主动清空 `projects` store**：
- `persistParams` / `handlePageSelect` / `handlePageParamsChange` 只在用户交互（点击导航、页内参数变更）时调用，不在 mount 时调用（`src/panels/settings/SettingsPanel.tsx:240-316`）。
- SC-FE-08 自动关闭 effect（`src/panels/settings/SettingsPanel.tsx:350-398`）在启动恢复场景下：`activePageId` 与 `ownPageId` 同属一个项目时直接返回，不会 `api.close()`；即使 close，也仅触发 Dockview `onDidRemovePanel`，不会修改 `projects` store。

因此，**F11 不是根因，但 F11 使布局 JSON 变大、保存频率变高，可能暴露了原本就存在的「加载静默失败」问题**。

### 1.3 最可能的根因：启动加载路径静默失败

结合文件完整、代码未变、现象为空 store，最可能根因是：

> 在重启瞬间，`load_projects` IPC 调用或后端 `read_to_string` 因文件锁、句柄占用、ConPTY/窗口关闭时的资源竞争等原因失败，前端 `loadFromDisk` / `App.tsx` 的 `catch` 静默吞错，导致 store 保持 `{}`，随后 `markPersistenceReady()` 打开门控，应用以空项目状态渲染。

该根因能解释所有现象：
- 侧栏可见（Workspace、SideBarArea 不依赖项目数据即可渲染）。
- 主区空白（`allPages` 为空，无 `PageDockview` 渲染）。
- 导航树项目列表为空（`projects` store 为空）。
- `slterminal-projects.json` 仍然完整（失败发生在读取阶段，尚未触发空写）。

---

## 2. 修复建议

### 高：加载失败必须阻断启动并提示用户

**问题**：当前加载失败后仍 `markPersistenceReady()` 并渲染 Workspace，用户面对空项目无从感知是加载失败还是真的无项目。  
**修复**：
1. `src/stores/projects.ts:253-256` 的 `catch` 不应静默，应将错误向上抛出，由 `loadAllProjects` 调用方决定。  
2. `src/App.tsx:87-91` 在 `loadAllProjects()` catch 后应：
   - 不调用 `markPersistenceReady()`（避免后续空写自杀）；
   - 向用户显示明确错误（toast 或错误页），并提供「重试加载」/「以空状态继续」选项。  
3. 若决定允许用户以空状态继续，再调用 `markPersistenceReady()`，且应在用户确认后执行。

### 高：启动加载增加一次显式验证

**问题**：`loadFromDisk` 只验证 JSON 格式，不验证 `projects` 是否为对象。极端情况下（如 IPC 返回异常结构）可能写入非对象值。  
**修复**：在 `src/stores/projects.ts:244-252` 的 `set` 之前增加结构校验：

```ts
if (!data || typeof data.projects !== "object") {
  throw new Error("项目数据格式异常");
}
```

### 中：关闭保存前检查 store 是否为空

**问题**：如果加载失败后 `initialized` 已置 `true`，关闭时的 `saveAllProjects()` 会把空 `{}` 写回磁盘，覆盖原有数据（自杀链）。  
**修复**：在 `saveToDisk`（`src/stores/projects.ts:259-264`）或 `saveAllProjects` 中增加守卫：

```ts
if (Object.keys(projects).length === 0 && !用户显式清空) {
  // 拒绝保存空项目，避免覆盖有效历史数据
  console.warn("拒绝保存空项目数据，防止覆盖历史文件");
  return;
}
```

### 中：SettingsPanel `persistParams` 的幂等与错误处理

**问题**：`src/panels/settings/SettingsPanel.tsx:240-246` 的 `persistParams` 同时做 `updateParameters` + `handleLayoutPersist(saveLayout(...))`。如果 `saveLayout` 在容器未就绪时返回异常结构，可能触发不必要的 `updatePageLayout`。  
**修复**：在 `handleLayoutPersist`（`src/panels/settings/SettingsPanel.tsx:222-236`）中校验 `layout` 是否为对象，且查找 `pageId` 失败时显式警告，避免把空/异常布局写入 store。

### 低：加载失败增加审计日志

**问题**：当前仅 `console.warn`，生产环境难以排查。  
**修复**：在 `src-tauri/src/projects.rs:135-141` 的 `load_projects` 命令中，对 `app_data_dir()` 失败、`read_to_string` 失败、`.bak` 恢复等情况输出 `tracing::error!` 带路径信息，便于后端日志定位。

---

## 3. 附带发现

### 3.1 已确认问题

1. **SC-FE-08 自动关闭在极端时序下可能误关（中）**  
   `src/panels/settings/SettingsPanel.tsx:350-398` 依赖 `useLayout((s) => s.activePageId)` 和 `useProjects((s) => s.projects)`。在启动恢复时，如果 `projects` 尚未水合但 `activePageId` 已设置，`findProjectId(activePageId)` 返回 `null`，effect 直接返回；但随后 `projects` 水合会触发 effect 重跑。虽然当前逻辑下不会误关，但该 effect 对 store 加载时序敏感，建议增加 `loaded` 或 `ready` 门控，确保在启动恢复完成前不评估关闭逻辑。

2. **`loadFromDisk` 与 `markPersistenceReady` 之间缺少原子性（高）**  
   `src/App.tsx:85-91` 中 `loadAllProjects()` 失败或跳过（E2E 分支）后都会调用 `markPersistenceReady()`。这导致「未加载完成」与「加载失败」两种状态都进入可写模式，是数据自杀链的根源。

3. **SettingsPanel 页组件 mount 后若调用 `onPageParamsChange` 会触发保存（低）**  
   当前三个配置页（`PlanBalancePage`、`HooksSettingsPage`、`KeybindingsPage`）均未在 mount 时调用 `onPageParamsChange`，符合设计。但未来新增页组件若未遵守此约定，会在启动恢复期触发 `persistParams` → `updatePageLayout`，加剧保存压力。建议在 `SettingsPageProps` 文档或类型中显式标注「mount 期禁止调用」。

### 3.2 疑点（证据不足，需运行时验证）

1. **文件锁/句柄占用**：无法从静态代码确认重启时是否有残留句柄锁定 `slterminal-projects.json`。需用 Process Monitor 复现。  
2. **IPC 时序**：无法确认 `load_projects` 调用是否因 Tauri 启动时序而失败。需在失败时抓取 WebView2 console 与后端 tracing 日志。

---

## 4. 候选根因（按可能性排序）与区分验证方法

由于物证无法 100% 锁定唯一运行时根因，按可能性排序如下：

### 候选 1：启动时 `load_projects` 因文件锁/IPC 失败，静默吞错（最可能）

**依据**：
- 文件完整合法，排除保存写空。
- 多层 `catch` 吞掉错误，与「无提示、空状态」现象吻合。
- F11 使布局 JSON 变大、保存频繁，可能增加重启时文件锁竞争概率。

**验证方法**：
1. 在 `src/stores/projects.ts:253-256` 的 `catch` 中临时改为 `console.error` 并 `throw err`，在 `src/App.tsx:87-90` 中改为 `toast.show("error", ...)`，重新构建复现，观察报错。  
2. 复现时打开 Process Monitor，过滤 `slterminal-projects.json`，查看新进程启动瞬间是否有 `SHARING_VIOLATION` 或 `ACCESS_DENIED`。  
3. 在 `src-tauri/src/projects.rs:135-141` 增加 `tracing::error!`，复现后查看 `%APPDATA%` 或 Tauri 日志中的错误信息。

### 候选 2：应用读取了错误的数据目录

**依据**：
- `app_data_dir()` 依赖 `std::env::current_exe()` 的父目录。如果用户通过快捷方式、另一个副本或 package 后的目录启动，`current_exe()` 指向的目录可能不是 `src-tauri/target/debug/`，从而读取到另一个空的 `slterminal-projects.json`。

**验证方法**：
1. 在 `src-tauri/src/projects.rs:135-141` 的 `load_projects` 中临时 `tracing::info!` 输出 `app_dir` 与文件是否存在。  
2. 复现 bug 后，检查进程实际加载的 `slterminal.exe` 路径（任务管理器 → 详细信息 → 命令行），与 `D:\data\learn\code\slTerminal\src-tauri\target\debug` 对比。  
3. 在目标目录搜索所有 `slterminal-projects.json`，确认是否有多个副本。

### 候选 3：SettingsPanel SC-FE-08 在异常时序下触发 `api.close()`，间接导致布局保存异常

**依据**：
- 数据文件中保存了 `settings` 面板，且为 `activeView`。
- 代码上已排除该路径会清空 `projects`，但需运行时确认没有 race condition。

**验证方法**：
1. 在 `src/panels/settings/SettingsPanel.tsx:368-372`（firstRun close 分支）和 `src/panels/settings/SettingsPanel.tsx:374-397`（dirty close 分支）添加 `console.warn` 记录关闭原因。  
2. 复现 bug，检查 console 中是否有 SettingsPanel 关闭日志。  
3. 临时注释掉 SC-FE-08 effect（`src/panels/settings/SettingsPanel.tsx:350-398`），重新构建复现。若 bug 消失，则根因在此；当前静态分析判断该候选概率低。

### 候选 4：Zustand store 订阅/水合竞态

**依据**：
- `useProjects.subscribe` 在模块加载时注册，而 `loadFromDisk` 异步调用 `set`。如果 `set` 在订阅注册前发生（理论上不可能，因为 `loadFromDisk` 在 App mount 后调用），状态会丢失。

**验证方法**：
1. 在 `src/stores/projects.ts:79`（create 完成）和 `src/stores/projects.ts:245`（set 调用）添加 `console.log` 时序标记。  
2. 确认 `set` 时订阅已注册且 `initialized=false`。  
3. 该候选概率极低，仅作兜底排查。

---

## 5. 结论一句话摘要

**重启后项目未恢复的根本原因是：启动加载 `slterminal-projects.json` 的路径在失败时被多层 `try/catch` 静默吞掉，导致 `useProjects` store 保持空状态并继续渲染；磁盘文件本身完整，问题不在保存阶段。F11 设置中心面板出现在保存的布局中，但并未直接清空 store，最可能是通过增加布局保存/恢复复杂度间接暴露了既有加载失败的静默处理问题。**

**产出文件路径**：`D:\data\learn\code\slTerminal\docs\settings-center-review\review-01-root-cause.md`
