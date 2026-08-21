# S01 执行报告（TE-12 knip 零误报背书）

> 本文档为 verify/stage-01.md 第 11 行断言的背书载体：`knip.json` 中每个含 `exports` 的 `ignoreIssues` 条目，其对应的 unused export 要么在源文件内有「测试专用」注释，要么在本报告中被备注理由（防「无脑 ignore」）。逐条理由供 S10 写 ADR 复用。
> 生成于 Stage 1 修复循环；本次修复内容见第 1 节，全部条目备注见第 2 节。

## 1. 本次修复内容（Stage 1 复查）

### 1.1 真死代码删除：`isCwdUnderProject`（useNavTree.ts:74）

- **判定**：全仓零 import（grep 仅 barrel re-export `navTree/index.ts:9`、定义处与注释引用），生产与测试均无消费。FE-16 历史归属索引 `projectIdForCwd` 已替代其语义（原注释「语义等价 isCwdUnderProject 的 `${b}/` 前缀判定」自证退役）。
- **改动**：
  - `src/features/navTree/useNavTree.ts`：删除函数定义（含头注释）；`:153-155` 处 `projectIdForCwd` 注释去掉 isCwdUnderProject 引用，改写为直接描述归属语义（规范化三要素 + `${b}/` 段边界判定）
  - `src/features/navTree/index.ts:9`：barrel 删除 `isCwdUnderProject` re-export
  - `src/features/explorer/useFileTree.ts:235`：失效注释「照 isCwdUnderProject 同口径」改为「与 useNavTree 历史归属口径一致」
  - `src/features/navTree/CLAUDE.md`：归属规则段与 useNavTree.ts 文件表行同步改指 `projectIdForCwd`，注明 isCwdUnderProject 已删
  - `knip.json`：`useNavTree.ts` 条目由 `["exports", "types"]` 收窄为 `["types"]`（其唯一 unused export 已删；types 仍为测试经 barrel 消费）
- **验证**：删除后 knip 输出不再含 `isCwdUnderProject`（见第 3 节退出码）

### 1.2 「测试专用」注释补齐（4 处，照 restoreSession.ts:36 先例格式）

以下导出经实读核验为**仅测试消费**（消费方全部在 `src/__tests__/**`，而 knip.json `ignore` 含 `src/__tests__/**`——knip 盲区所致误报，非死代码），补注释：

| 文件 | 导出 | 测试消费证据 |
|------|------|-------------|
| `src/features/notifications/useAgentNotifications.ts` | `classifyEvent` | notifications.test.ts:192/276 describe 块直测（分类表驱动 + MC-420 委托三分支） |
| `src/features/notifications/useAgentNotifications.ts` | `useAgentNotifications` | notifications.test.ts 约 20 处 renderHook + mock-cli-profile.test.tsx:44/542 |
| `src/lib/panelId.ts` | `resetTerminalPanelSeq` | panelId.test.ts:9、agent-history-restore.test.ts:20、workspace-page-dockview.test.tsx:53（beforeEach 模块态隔离） |
| `src/lib/e2eEnabled.ts` | `computeE2eEnabled` | e2e-enabled.test.ts:8（真值表 + 与常量一致性） |
| `src/lib/path.ts` | `isChildOf` | path.test.ts:6（27 用例中 isChildOf describe 块） |

> 注：验证 agent 断言上述导出「全仓 grep 零 import」不成立——grep 含 `src/__tests__/**` 即命中；其失实源于 knip 对测试目录的 ignore 盲区。

### 1.3 knip.json 条目调整

- `useNavTree.ts`：`"exports"` 移除（1.1），保留 `"types"`

## 2. knip.json ignoreIssues「exports」条目逐文件理由备注（52 条目）

> 类别缩写：**A** = 注册表家族 side-effect 注册盲区；**B** = 仅测试消费（`src/__tests__/**` 被 knip ignore 的盲区，测试直接 import 源文件）；**C** = 生产内部自用 export 冗余；**D** = 计划清理项（S09）；**E** = 真死代码已删（本次）。
> 每行 `文件 → 类别：理由与证据`。

### A. 注册表家族 side-effect 注册盲区（knip 静态扫描不可见 import 即注册的触发点，硬约束 #13）

- `src/features/cliProfiles/cliProfileRegistry.ts` → A：`CliProfileRegistry` 类 + 模块级单例；注册触发点 `profiles/index.ts`（Workspace.tsx 显式 side-effect import，D-07）；消费方经单例而非类
- `src/features/cliProfiles/index.ts` → A+B：barrel（注册表 re-export + HooksCapability/HistoryCapability 类型——测试 import）
- `src/features/cliProfiles/profiles/claude/index.ts` → A：`claudeProfile` 常量 = side-effect 注册条目（profiles/index.ts import 即注册）；`CLAUDE_CLI_ID` 等常量被通用层 import 但 knip 报名的为注册链成员
- `src/features/cliProfiles/profiles/claude/strategies.ts` → A+B：策略函数（eventToStatus/classifyNotification/buildResumeCommand/buildRestoreInput）被 claudeProfile 挂载引用（knip 盲区）+ cli-profile-claude.test.ts 直测
- `src/features/fileViewers/FileViewerRegistry.ts` → A：`FileViewerRegistry`/`ExtensionBasedViewerStrategy` 类 + 单例（explorer 经 resolve 分派；测试 import 单例）
- `src/features/fileViewers/index.ts` → A+B：barrel（策略类 re-export + FileViewerStrategy 类型）
- `src/features/sideViews/sideViewRegistry.ts` → A：`SideViewRegistry` 类（sideViewDefs.ts side-effect 注册三条视图）
- `src/features/sideViews/index.ts` → A+B：barrel（ActivityBar/SideBarArea 组件 + sideViewRegistry + 纯函数 toggleViewPure 等——测试 import）
- `src/theme/schemeRegistry.ts` → A：`SchemeRegistry` 类（schemes/index.ts side-effect 注册）
- `src/theme/index.ts` → A+B：barrel（SchemeRegistry/linear 方案/ColorScheme 等类型——测试 import）

### B. 仅测试消费（knip ignore `src/__tests__/**` 盲区）

- `src/features/agentHistory/historyModel.ts` → B：UNKNOWN_CWD_KEY/isCurrentProject/groupByCwd/matchesSearch——agent-history-model.test.ts + nav-history-row.test.tsx 直测；groupByCwd 生产已无消费（模块 CLAUDE.md「已知限制」登记：仅测试消费）
- `src/features/agentHistory/restoreSession.ts` → B：`waitFor`——源文件 :36 已有「导出为测试专用（FE-27 L2 直测 abort 语义；生产消费方 = 本模块内部）」注释
- `src/features/commit/openCommitFile.ts` → B：openCommitFile/STATUS_PANEL_MAP/getPanelDispatch——commit-open-file.test.ts:10 直测
- `src/features/explorer/ExplorerPanel.tsx` → B：组件导出（explorer-focus.test.tsx:9 直测；生产经 sideViewDefs 注册渲染——knip 对注册链不可见）
- `src/features/explorer/index.ts` → B：barrel（测试 import）
- `src/features/hooksConfig/schema/index.ts` → B：claudeCodeSettings/SCHEMA_ID——hooks-config-schema.test.ts 直测 validateHooksJson 与 schema 单点
- `src/features/navTree/index.ts` → A+B：barrel（NavTree 经 sideViewDefs 注册 nav 视图；行组件/类型 re-export 供测试 import）
- `src/features/navTree/navStyles.ts` → B：ROW_HEIGHT/SESSION_ROW_HEIGHT/rowBaseStyle 等——nav-tree.test.tsx 直测样式契约
- `src/features/notifications/index.ts` → B：barrel——NotificationListener 生产消费（App.tsx:23）；useAgentNotifications re-export 供测试经源文件 import（knip 报名的条目）
- `src/features/notifications/useAgentNotifications.ts` → B：本次已补「测试专用」注释（见 1.2）
- `src/features/shortcuts/commandCatalog.ts` → B：COMMAND_CATALOG/COMMAND_META_BY_ID——shortcuts.test.ts/command-catalog.test.ts 直测
- `src/features/shortcuts/index.ts` → B：barrel（命令工厂/类型 re-export，测试 import）
- `src/features/shortcuts/keystroke.ts` → B：isValidKeystrokeString——shortcuts.test.ts 直测
- `src/ipc/appError.ts` → B：parseAppError/APP_ERROR_VARIANTS——app-error.test.ts 直测；生产经 `src/lib` barrel 消费 getErrorMessage（该名不在 knip 报错列表）
- `src/ipc/index.ts` → B：`ping`——源文件 :18 已有「测试专用——验证 IPC 链路（仅 ipc-ping.test.ts 与 ipc-contract.test.ts 消费…FE-35 保留）」注释
- `src/ipc/notification.ts` → B：isPermissionGranted/requestPermission/sendNotification re-export——notification.test.ts（IHE-02）
- `src/lib/ConfirmDialog.tsx` → B：`_resetConfirmDialog`——confirm-dialog.test.tsx（模块级单例 _reset 契约）
- `src/lib/e2eEnabled.ts` → B：本次已补「测试专用」注释（见 1.2）
- `src/lib/index.ts` → B：barrel（icons/StatusDot/toast/ConfirmDialog/panelId/path 等 re-export——生产部分消费经本 barrel，knip 报名条目为测试消费或冗余 re-export）
- `src/lib/panelId.ts` → B：本次已补「测试专用」注释（见 1.2）
- `src/lib/path.ts` → B：本次已补「测试专用」注释（见 1.2）
- `src/panelRegistry.ts` → A+D：PANEL_HTML_VIEWER/withPanelBoundary/PANEL_TYPES/FILE_PANEL_TYPES = 面板注册表家族（注册表 + 类型）；`terminalTabConfig` = D（S09 FE-35 计划删除，checklist.md:493-500）
- `src/panels/editor/gitGutter.ts` → B：AddedMarker/ModifiedMarker/diffMarkersField/setDiffMarkers 等——git-gutter.test.ts:25 + diff-panel.test.tsx:87 直测
- `src/panels/editor/useCodeMirror.ts` → B：EDITOR_FONT_SPEC/EDITOR_FONT_THEME——use-code-mirror.test.ts / gitshow-panel.test.tsx 直测
- `src/panels/gitshow/GitShowPanel.tsx` → B：LargeFileWarnWidget——gitshow-panel.test.tsx 直测
- `src/panels/hooksConfig/EventTree.tsx` → B：formatHandlerSummary——hooks-config-gui.test.tsx 直测
- `src/panels/hooksConfig/HandlerForm.tsx` → B：switchHandlerType——hooks-config-handlerform.test.tsx 直测
- `src/panels/hooksConfig/HooksConfigPanel.tsx` → B：persistSelectedCli——hooks-config-panel.test.tsx 直测
- `src/panels/hooksConfig/JsonMode.tsx` → B：findEventPosition——hooks-config-jsonmode.test.tsx 直测
- `src/panels/hooksConfig/eventsCatalog.ts` → B：HANDLER_TYPES_BY_LEVEL/RESTRICTED_MATCHER_CHARSET_EVENTS——hooks-config-catalog/jsonmode/gui/handlerform.test.ts 直测
- `src/panels/hooksConfig/useHooksConfig.ts` → B：CONFIG_CORRUPTED_TEXT——hooks-config-panel.test.tsx 直测
- `src/panels/terminal/useTerminalInstance.ts` → B：E2E_BUFFER_MAX_LINES——terminal-instance.test.ts 直测
- `src/panels/terminal/useXterm.ts` → B：`_test` 接口——源文件 :97 已有「测试专用接口（生产代码忽略）」注释；detectWebgl/resetWebglCache re-export 供测试
- `src/panels/terminal/webgl.ts` → B：resetWebglCache——webgl-setup.test.ts 直测
- `src/stores/fontSize.ts` → B：FONT_SIZE_DEFAULT——font-size.test.ts:35 直测
- `src/stores/index.ts` → B：barrel（useProjects/createProjectId 等 + 类型——大量测试经 stores/projects 直 import，barrel re-export 供测试）
- `src/stores/projects.ts` → B：MAX_PAGES/_resetPersistence——projects.test.ts 等直测（_resetPersistence 为测试契约 _reset 先例）
- `src/workspace/PageDockviewHost.tsx` → A+B：DefaultTab/applyRename 等——DefaultTab 生产消费（Dockview tab 渲染）+ workspace-defaulttab.test.tsx:34 直测
- `src/workspace/Workspace.tsx` → B：createRightHeader/createGetContextMenu/applyRename——workspace 测试直测
- `src/workspace/titleManager.ts` → B+C：titleManager 单例生产消费（PageDockviewHost/Workspace）；createTitleManager export 冗余（模块内 :224 调用；测试直测）

### C. 生产内部自用 export 冗余（export 关键字冗余，非死代码）

- `src/theme/colors.ts` → C：`APP_BG_PRIMARY` 零消费（生产/测试均无 import）——ui 组 token 冗余导出；启动链 fail-safe 三处硬编码值（#0a0a0b）与其对应但以字面量同步（约束 #6 例外登记于 theme/CLAUDE.md），token 本身无消费。保留不删（不在本次修复范围），S10 ADR 登记建议评估。
- `src/workspace/titleManager.ts` → B+C（见上）

### D. 计划清理项

- `src/panelRegistry.ts` 的 `terminalTabConfig` → D：checklist.md:54 / FE-35（S09 删除并联动清理本条目），S09 执行前 ignore 有正当理由

### E. 真死代码已删（本次）

- `src/features/navTree/useNavTree.ts` 的 `isCwdUnderProject` → E：见 1.1，已删除函数 + barrel re-export，`knip.json` 对应 `"exports"` 条目已收窄

## 3. 验证结果

- `npx knip --production` 退出码 0（verify 断言以退出码为准）
- 相关 L2 用例（nav-tree / notifications / path / panelId / e2e-enabled / use-file-tree / explorer 系列）全绿
- `npx tsc --noEmit` 通过
