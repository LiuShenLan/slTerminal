# 05 域 — hooks 配置面板（hub 面板 + CLI 选择行）

> 总规格：`docs/spec/multi-cli/00-需求规格.md`（决策 D7）。现状基线：`docs/analysis/02-hooks.md`（02-18~28）、`docs/analysis/04-ui-status.md`（面板标题现状）。

## 1. 设计形态（用户确认，决策 D7）

hub 面板：**单面板 + 顶部 CLI 选择行 + 内部切换**。同页单例（面板 id = `hooksConfig-{pageId}` 不变）；顶部一行 CLI 按钮（logo + 名称，能力过滤），点击切换下方配置编辑器；选中项 `params.selectedCli` 随布局持久化。

```
┌──────────────────────────────────────────────┐
│ [🖼 claude] [🖼 codex] …        ← CLI 选择行   │
├──────────────────────────────────────────────┤
│  选中 CLI 的配置编辑器（claude = 现有面板全部内容）│
│  层级切换 user/project/local + GUI/JSON +      │
│  注入状态条 + 保存 + 重启提示                    │
└──────────────────────────────────────────────┘
```

## 2. 需求条目

| 编号 | 需求 | 优先级 |
|------|------|--------|
| MC-501 | 面板 id `hooksConfig-{activePageId}` 同页单例语义不变（C13-7）；侧栏右键菜单「打开 Hooks 配置」保持**单一入口**（先切页 → `openHooksConfigPanel(pageId)` 查重聚焦/新建，流程零改动）；面板标题「Hooks 配置」已是 CLI 中立，不改 | 必须 |
| MC-502 | **顶部 CLI 选择行**：遍历 `CliProfileRegistry.getAll()` 过滤 `profile.capabilities.hooks?.hasConfigEditor === true` 的 profile，渲染按钮（iconSrc 16×16 logo + displayName）；选中态视觉（背景高亮，配色走 theme token，硬约束 #6）；点击 → 切换下方编辑器目标 CLI | 必须 |
| MC-503 | **选中态持久化**：`params.selectedCli` 随布局 JSON 持久化（照 F8 customTitle 先例——`api.updateParameters({...params, selectedCli})` + 显式 `onLayoutChange(saveLayout(api))`，setTitle/updateParameters 不触发 onDidLayoutChange 须显式保存）；面板挂载时读 `params.selectedCli` 恢复；**缺省/失效回退**：params 无值或指向已无 hasConfigEditor 的 CLI → 选中选择行首个有能力 CLI | 必须 |
| MC-504 | **选择行下方渲染选中 CLI 的配置编辑器**：claude = 现有 HooksConfigPanel 全部内容（层级切换 user/project/local + GUI/JSON 双模式 + 注入状态条 + 保存 + 重启提示条），作为 claude 的配置编辑器组件整体迁入 hub 容器；IPC 调用经泛化命令（`agent_hooks_config_read/write(selectedCliId, ...)`、`agent_hooks_inject/uninstall/injection_status(selectedCliId)`，见 MC-220/221） | 必须 |
| MC-505 | **切换 CLI 的状态处置**：切换选中 CLI = 卸载当前编辑器并重挂载目标编辑器（照 ADR-0001 换区重建先例——编辑器内部 dirty/选中态丢弃）；**dirty 守卫**：当前编辑器 dirty 时切换需 `dialog.ask` 确认丢弃（照切层/visibilitychange 的 ask 守卫先例，askGuard 防循环复用） | 必须 |
| MC-506 | 保存成功提示条由 `profile.hooks.restartHint` 驱动（MC-222）；`data-e2e="hooks-restart-hint"` 选择器保留；注入状态条三态（已注入/未注入/版本过旧）语义不变，数据源为 `agent_hooks_injection_status(selectedCliId)` | 必须 |
| MC-507 | **选择行空态**：注册表中无任何 hasConfigEditor 的 profile 时（理论上 claude 恒有，防御分支）选择行渲染「无可配置 CLI」占位，不渲染编辑器 | 必须 |
| MC-508 | claude 配置编辑器内部（层级切换/GUI/JSON 双模式/eventsCatalog/matcherEngine/schema/注入段保护/F2 注入按钮）**行为零改动**；claude hooks 协议知识不抽象（MC-223）；文件物理位置保留现状（`panels/hooksConfig/` + `features/hooksConfig/`），模块 CLAUDE.md 注明「claude 专属编辑器」语义 | 必须 |

## 3. 边界条件

1. **只有一个有能力 CLI**（现状：仅 claude）：选择行仍渲染（单按钮选中态），不为单 CLI 隐藏——保持 UI 结构稳定，避免「有时有行有时无」的布局跳动。
2. **selectedCli 指向已卸载/能力被移除的 CLI**：回退首个有能力 CLI（MC-503 失效回退），不报错。
3. **rootPath 为空（无活跃项目）**：claude 编辑器内 project/local 层禁用（现状保留）；选择行不受影响（user 层仍可编辑）。
4. **dirty 跨 CLI**：dirty 状态属编辑器实例不属面板——切 CLI 经 ask 确认后丢弃（MC-505），不做跨 CLI 脏状态保留（照 ADR-0001 先例，低频操作接受状态丢失）。
5. **面板 props 兼容 Dockview**：hub 容器组件与现状 HooksConfigPanel 同样不依赖 panelId 单例语义（props 兼容，C13-7 语义由 openHooksConfigPanel 入口保证）。

## 4. 测试要求

| 层级 | 用例 |
|------|------|
| L2 | 选择行渲染：能力过滤（hasConfigEditor=false 不出现）、logo+displayName、选中态高亮；点击切换 → 编辑器重挂载（目标 CLI 的 IPC 调用携新 cliId） |
| L2 | 持久化：selectedCli 写入 params + 显式 onLayoutChange 调用（照 customTitle 测试先例）；挂载恢复；失效回退首个有能力 CLI |
| L2 | dirty 守卫：dirty 切换 → ask 确认/取消两分支；非 dirty 直接切换 |
| L2 | 空态：无 hasConfigEditor profile → 占位文案 |
| L2 | claude 编辑器既有用例（hooks-config-* 11 文件）在 hub 容器内全绿（挂载路径变化，断言语义不丢）；restartHint 由 profile 驱动 |
| L4 | `hooks.e2e.ts` hooksConfig 用例（project 层保存写盘 + merge 保留其他字段）经 hub 面板全绿 |
| AC-4 | mock profile（hasConfigEditor=true + 桩编辑器）：选择行出现两枚按钮、切换渲染 mock 编辑器、selectedCli 持久化恢复 |

## 5. 迁移点（本域）

| 现状 | 目标 |
|------|------|
| `HooksConfigPanel`（面板根组件 = 层级切换器 + 模式切换 + 注入条 + 保存） | hub 容器（选择行 + 编辑器槽）+ claude 编辑器组件（现有内容整体下移一层） |
| 面板 params（无 CLI 选择概念） | `params.selectedCli` 随布局持久化（照 customTitle 先例） |
| `readHooksConfig/writeHooksConfig/inject/uninstall/getInjectionStatus`（无 cliId） | 泛化命令 + selectedCliId 首参（MC-220/221） |
| 保存提示「…重启 claude 会话生效」硬编码 | profile.hooks.restartHint 驱动 |
