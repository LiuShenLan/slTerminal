# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

Zustand 全局状态真值来源。每个 store 覆盖一类状态域，面板只订阅，不自存状态（硬约束 #12）。PTY 进程映射在 `panels/terminal/TerminalRegistry` 管理。

## 关键约束与决策

### Store 纯状态（硬约束 #12）

- `src/stores/` 只存状态与状态转换，不存业务逻辑（校验/映射/编排放到注册表、纯函数或上层组件）。
- 持久化一律经 `src/ipc/` 对应领域函数：settings 类（`fontSize` / `keybindings` / `sideBar` / `cliAliases`）走 `src/ipc/settings`；项目数据（`projects`）走 `src/ipc/projects`。
- 禁止在 store 内直接调用 Tauri `invoke`；禁止跨 store 隐式依赖，store 间协调在上层组件/命令中完成。

### 持久化模式

settings 类四 store 与 `projects` 均遵循同一模式：

- 启动时 `loadFromDisk()` 恢复；`loaded` 守卫防止加载阶段触发空写。
- 变更后 Zustand `subscribe` + 2s debounce 自动保存。
- `markPersistenceReady()`（projects）/ `loaded = true`（settings）在加载完成后置位。
- **projects 持久化防线**：`loadFromDisk` 成功置 `loadSucceeded`；未成功加载且 store 空时 `saveToDisk` 拒写（防空写覆盖磁盘）；显式放行经 `markLoadSucceeded()`（E2E 分支/用户选择空状态继续）。
- `cancelPendingSave()` 供关闭钩子冲刷未落盘的 timer。

`cliAliases`（CLI 别名）store 为硬约束 #12 边界的现行示例：**校验不进 store**——语法与 D3 全命名空间唯一性校验/加载净化收在 cliProfiles 域纯函数模块 `features/cliProfiles/aliasValidation.ts`（参数注入、零 store/注册表 import），store 只存 `aliases`（cliId → 数组）与增删转换；消费编排（快照 → 注册表）在 App.tsx 组合层。loadFromDisk 的 sanitize 需 profile 已注册（孤儿 cliId 判定），依赖 App 启动 import 链时序（见文件头注释）。

### 段形态契约（断链修复）

settings 类 store 保存时顶层键必须是段名（`fontSize` / `keybindings` / `sideBar`），后端 `settings.rs` 浅合并 top-level 键。`fontSize.ts` 曾因平铺 `terminalFontSize` / `editorFontSize` 顶层键被 SEC-11 白名单拒绝，导致配置静默丢失；现 payload 精确写段，双侧测试锁死。

### FE-09 / FE-11 统一消费

- **FE-09 保存失败**：`saveSettings` / `saveProjects` 失败统一 `toast.show("warning", "设置保存失败，重启后将丢失")`。
- **FE-11 corrupted**：`loadSettings` / `loadProjects` 返回 `{ data, corrupted }`，`corrupted: true` 时 toast「配置已损坏，已回退默认值」。

### 页面总数上限随多实例架构消亡（CP-004/S11）

- **页面总数上限（原 FE-01/FE-36，旧常量值 20）已删除**：多 Dockview 实例架构退役（转共享宿主 + 页组模型——`src/workspace/CLAUDE.md`「共享宿主 + 页组模型」节）后，页面不再各持一实例，容器/渲染管线共享，内存/DOM 不再随页数线性增长——上限随之消亡。
- `addPage` 拒绝条件只剩「项目不存在」；不再有「页面数已达上限」toast。
- `OperationPage.layout` 字段语义 = 该页「页组子树切片」（宿主全量 JSON 按页切分），由 `workspace/layoutSerde.ts` 单点存取（硬约束 #7）。

### FE-37 switchToPage 纯状态转换

`switchToPage` 为纯状态转换，不触 IPC。页面切换的 `setProjectRoot` 前置由 `src/workspace/pageApis.ts` 的 `switchToPageShared` 承担，符合硬约束 #12。

### sideBar 默认态

默认 `zones.top = ["nav", "explorer", "commit"]`、`open.top = "nav"`；`projects` / `agent-status` 视图随 NAV-06/08 退役。

## 测试模式

- **真实 store**：测试用 Zustand `create()` 创建真实 store，`beforeEach` 用 `.setState()` 重置。
- **`getState()` 直接操作**：不依赖 React 渲染，直接操作和断言。
- **持久化测试**：mock `src/ipc/settings` 或 `src/ipc/projects`，验证 save/load 往返 + 异常降级 + `loaded` 守卫。
- **debounce 测试**：`vi.useFakeTimers()` + `vi.advanceTimersByTime()`；projects 用 `_resetPersistence()` 辅助函数重置 `initialized` 和 timer。

## 运行

```bash
npm test
```
