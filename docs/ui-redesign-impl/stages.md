# UI 重设计 Stage 划分（ui-redesign-impl）

> 9 Stage 全串行（每 Stage commit）。划分原则：Stage 内 agent 文件零重叠；Stage 间允许重复碰同一文件。
> 断言唯一真值源：`workflows/verify/stage-NN.md`（stage 脚本与 fix-loop 共用）。
> 契约单点：linear 全值 = checklist.md 附录 A；组件 API 契约写死于各脚本头部。

## Stage 01 配色方案替换（TH-01~TH-11）

**改动项**：TH-01/02/03/04/05/06/07/08/09/10/11

**agent 分工**（并行 4，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| scheme-core | TH-01/02/03/04/09 | src/theme/schemes/types.ts、schemes/linear.ts（新建）、schemes/index.ts、schemes/darcula.ts（删除）、schemeRegistry.ts、theme/index.ts、theme/colors.ts |
| scheme-inject | TH-07/08 | src/theme/overrides.ts、src/panels/editor/useCodeMirror.ts、src/panels/gitshow/GitShowPanel.tsx、src/panels/diff/DiffPanel.tsx、src/panels/hooksConfig/JsonMode.tsx |
| failsafe-main | TH-05/06 | src/main.tsx、index.html、src-tauri/tauri.conf.json |
| test-sync | TH-10/11 | src/__tests__/colors.test.ts、scheme-registry.test.ts、overrides.test.ts、bootstrap.test.ts、main-bootstrap.test.tsx、explorer-git-status.test.tsx、git-gutter.test.ts、commit-context-menu-ui.test.tsx、explorer-selection.test.tsx、sideBarArea.test.tsx、test/terminal/theme-options.test.ts |

**实现要点**：
- 全值只准照抄 checklist 附录 A；`DEFAULT_SCHEME_ID` 与 main.tsx 默认 id 均改 `"linear"`
- editorSyntaxHighlight 注入 5 消费点扩展数组时**位于 editorTheme 之前**（ACC-05 reverse 层叠：先声明者排最后恒胜）；新增覆盖规则全带 `&.cm-editor` 前缀
- scheme-core 的 colors.ts facade 31→33 导出（+ACCENT_FG/SELECTION_HOVER_BG），test-sync 的 colors.test.ts 键集合断言同步 +2
- tauri.conf.json 本 Stage 仅改 backgroundColor（decorations 归 Stage 04）

**验证**：verify/stage-01.md。要点：`grep -ri "darcula" src/ test/` 零残留（注释/测试白名单外）；linear.ts 四段键数（ui 6 组+25 标量/terminal 25/editor syntax 9+3/libraries dockview 20+allotment 2）；全量测试 + L3 通过。

**门禁**：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 4. `npm test` 5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 6. `npm run test:l3` 7. `npx vite build`（index.html 在 tsc 覆盖外）

**commit**：`feat(theme): linear 配色方案替换 darcula + syntax 语法色槽位 + fail-safe 同步`

## Stage 02 字体内置（FT-01~FT-04、FT-08）

**改动项**：FT-01/02/03/04/08（FT-05/06/07 分布实施，Stage 08 兜底）

**agent 分工**（并行 2）：

| label | 负责项 | 文件 |
|---|---|---|
| font-deps | FT-01/08 | package.json、src/main.tsx（需执行 `npm install @fontsource/jetbrains-mono`） |
| font-stack | FT-02/03/04 | src/App.css、src/panels/editor/useCodeMirror.ts、src/panels/terminal/theme.ts |

**实现要点**：字体栈唯一真值 = `"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace`；main.tsx ② 阶段（setActive 后）import `@fontsource/jetbrains-mono/400.css` 与 `500.css`；错误页 font-family 同步。

**验证**：verify/stage-02.md。要点：`grep "font-family\|fontFamily" src/ index.html` 全部声明 = 规格栈（main.tsx 错误页同）；`node_modules/@fontsource/jetbrains-mono` 存在；vite build 产物 dist/assets 含 woff2。

**门禁**：同 Stage 01 七条（terminal/theme.ts 改动涉 L3）

**commit**：`feat(theme): JetBrains Mono 字体内置 + 全局字体栈统一`

## Stage 03 图标体系（IC-01~IC-09）

**改动项**：IC-01/02/03/04/05/06/07/08/09

**agent 分工**（并行 4，icon-base 产出由其余三家引用——API 契约写死脚本头，同 Stage 并行可行）：

| label | 负责项 | 文件 |
|---|---|---|
| icon-base | IC-01/02 | package.json、src/lib/icons.tsx（新建）、src/lib/StatusDot.tsx（新建） |
| file-icon | IC-04 | src/features/explorer/FileIcon.tsx、src/__tests__/file-icon.test.tsx |
| status-chain | IC-03/08（状态行部分） | src/lib/agentStatus.ts、src/panels/terminal/useCommandDetection.ts、useXterm.ts、TerminalPanel.tsx、src/workspace/PageDockviewHost.tsx（仅 DefaultTab tabIcon 分支）、src/features/agentStatus/AgentStatusRow.tsx、src/features/agentHistory/HistorySessionRow.tsx + 状态链路相关测试 |
| misc-emoji | IC-05/06/07/08/09 | src/features/sideViews/sideViewDefs.ts、src/features/sidebar/SidebarTree.tsx、src/features/explorer/FileTree.tsx、src/features/agentStatus/AgentStatusView.tsx、src/features/agentHistory/HistorySessionList.tsx、src/features/notifications/useAgentNotifications.ts、src/__tests__/emoji-scan.test.ts（新建） |

**实现要点**：
- 契约：`<StatusDot status={working|attention|done|error} size={7}/>` 色映射 working→#86bb7a/attention→#d6b25e/done→#6b675f/error→#d9706b；icons.tsx 导出图标组件清单写死脚本头
- tabIcon 链路：updateParameters 键 `tabIcon` 改 `tabStatus`（status 字符串），DefaultTab tabIcon 分支改渲染 StatusDot；tabLogo 分支不动
- STATUS_EMOJI 删除后 `src/lib/agentStatus.ts` 保留 status 类型与 eventToStatus 委托（F3 映射逻辑零改动）

**验证**：verify/stage-03.md。要点：emoji-scan 守卫通过；`grep STATUS_EMOJI src/` 零命中；`grep "⚡\|🟡\|✅\|❌" src/` 零命中（守卫白名单外）；lucide-react 在 dependencies。

**门禁**：五件 + `npm run test:l3`（useXterm 终端侧）

**commit**：`feat(icons): lucide 线性图标体系 + 状态圆点替代 emoji + FileIcon 六色盘`

## Stage 04 自绘标题栏（TB-01~TB-06）

**改动项**：TB-01/02/03/04/05/06

**agent 分工**（并行 2，wrapper 签名契约写死脚本头）：

| label | 负责项 | 文件 |
|---|---|---|
| titlebar-ui | TB-01/02/04/05 | src-tauri/tauri.conf.json、src/features/titleBar/TitleBar.tsx（新建）、src/App.tsx |
| titlebar-ipc | TB-03/06 | src/ipc/window.ts、src/__tests__/title-bar.test.tsx（新建） |

**实现要点**：关闭钮必须经 `getCurrentWindow().close()`（触发 onCloseRequested → P1-19 杀 PTY 链路）；拖拽区 `data-tauri-drag-region` 仅标题栏背景与中段，三钮区域排除；项目/页面名数据源 = projects store 活跃项（执行期 Read stores/projects 确认 selector）。

**验证**：verify/stage-04.md。要点：tauri.conf.json `"decorations": false`；TitleBar 三钮调用对应 wrapper（测试断言）；**人工验证点 TB-07**（实机拖拽/双击/三钮/关窗 PTY 清理）。

**门禁**：五件 + `npx vite build`（tauri.conf.json 在 tsc 覆盖外）

**commit**：`feat(titlebar): 自绘 34px 一体化标题栏（decorations:false + 窗口控制）`

## Stage 05 页签栏改造（TAB-01~TAB-05）

**改动项**：TAB-01/02/03/04/05

**agent 分工**（并行 2）：

| label | 负责项 | 文件 |
|---|---|---|
| tab-ui | TAB-01/02/03/04 | src/workspace/PageDockviewHost.tsx |
| tab-test | TAB-05 | src/__tests__/workspace-header-actions.test.tsx、workspace-page-dockview.test.tsx、tab 相关测试文件 |

**实现要点**：底部 2px 指示条自绘（dockview 无对应变量）；文件页签图标复用 FileIcon（按 panel 文件路径扩展名）；35px 高为库默认不动。

**验证**：verify/stage-05.md。

**门禁**：五件

**commit**：`feat(tabs): 扁平页签 + 底部 2px 指示条 + hover 关闭钮 + 圆点logo构成`

## Stage 06 侧栏 IA 重构（NAV-01~NAV-11）

**改动项**：NAV-01/02/03/04/05/06/07/08/09/10/11（本 Stage 为最大改造，高风险，标人工验证点 NAV-11）

**agent 分工**（并行 3 → 串行 2）：

| label | 负责项 | 文件 |
|---|---|---|
| navtree-new（并行） | NAV-01/02/03/04/09 | src/features/navTree/（新建目录全部文件）；只读引用 useAgentStatus/agentHistory/projects store，不改动 |
| history-migrate（并行） | NAV-08 前半 | src/features/agentHistory/HistorySessionList.tsx、HistorySessionRow.tsx（单行化+title tooltip） |
| navtree-test（并行） | NAV-10 L2 部分 | src/__tests__/nav-tree*.test.tsx（新建） |
| sidebar-switch（串行 1） | NAV-05/06/07/08 后半 | src/features/sideViews/sideViewDefs.ts、sideBarState.ts、src/stores/sideBar.ts、src/features/sideViews/ActivityBar.tsx、删除 src/features/sidebar/SidebarTree.tsx、agentStatus/AgentStatusView.tsx、AgentStatusRow.tsx、agentHistory/AgentHistorySections.tsx、「打开 Hooks 配置」公共函数新文件（src/features/hooksConfig/ 或 src/lib/，执行期定）、SidebarTree/被删组件引用清理波及的 barrel 与测试文件 |
| e2e-rewrite（串行 2） | NAV-10 E2E 部分 | e2e-tests/sidebar.e2e.ts、agent.e2e.ts、mockcli.e2e.ts、helpers.ts |

**跨边界契约（写死脚本头）**：导航树视图 id = `"nav"`；配置钮视图 id = `"config"`（点击打开 hooksConfig 面板，非侧栏视图——ActivityBar 底部独立按钮，不入注册表）；data-e2e 选择器：`nav-tree`、`nav-row-project`、`nav-row-page`、`nav-row-session`、`nav-history-node`、`activity-btn-config`；旧选择器 `agent-status-view`/`agent-status-row`/`AGENT STATUS` 文案随 E2E 重写废除。

**实现要点**：
- 活跃会话归属：agentSession panelId → 页面前缀解析（B14 的 parseTerminalPageId 成对函数，执行期 Read TerminalRegistry 确认）；历史归属：cwd 前缀匹配项目 rootPath
- NavTree 右键菜单承接 SidebarTree 项目/页面菜单（删「打开 Hooks 配置」项），菜单视觉规范 = UI-802（28px/圆角 5/hover #222227）
- settings 迁移：reconcileZones 过滤未注册 id 语义确认覆盖 open 字段；sanitizeSideBar 适配
- makeEmptyLayout/内联重命名/项目页面 CRUD 逻辑全部迁移（行为不变，ADR-0003）

**验证**：verify/stage-06.md。要点：`grep -r "agent-status\|SidebarTree" src/` 零残留（barrel/测试白名单外）；sideViewDefs 恰 3 注册；E2E 三文件重写后 `npm run e2e` 通过。

**门禁**：五件 + `npm run test:l3` + `npx vite build`（e2e-tests/helpers.ts 在 tsc include 外，实证坑）+ `npm run e2e`（L4 关键路径变更）

**commit**：`feat(sidebar): 统一导航树 IA（项目→页面→会话）+ 活动栏三槽 + 配置钮入口唯一化`

## Stage 07 浮层统一（OV-01~OV-05）

**改动项**：OV-01/02/03/04/05

**agent 分工**（并行 4；overlay-base 产出由其余三家引用——ConfirmDialog/toast API 契约写死脚本头）：

| label | 负责项 | 文件 |
|---|---|---|
| overlay-base | OV-01 | src/lib/ConfirmDialog.tsx（新建）、src/lib/toast.tsx（新建） |
| overlay-commit-history | OV-02/04 部分 | src/features/commit/commitContextMenu.ts、CommitFileList.tsx、src/features/agentHistory/HistorySessionList.tsx、historyContextMenu.ts |
| overlay-explorer | OV-02/04/05 部分 | src/features/explorer/ExplorerPanel.tsx、FileTree.tsx |
| overlay-hooks-workspace | OV-02/03/04 部分 | src/panels/hooksConfig/HooksConfigPanel.tsx、useHooksConfig.ts、src/workspace/PageDockviewHost.tsx、TerminalRenameDialog.tsx、src/features/agentHistory/SessionActionDialog.tsx、src/ipc/dialog.ts、src/__tests__/dialog-e2e-hook.test.ts |

**跨边界契约（写死脚本头）**：`confirmDialog(opts: { title?: string; message: string; kind?: "warning"|"error"|"info"; confirmText?: string; cancelText?: string; danger?: boolean }): Promise<boolean>`；`toast.show(type: "success"|"warning"|"error", message: string): void`；组件挂载点 = App.tsx 根部（overlay-base 负责，App.tsx 仅其触碰）。

**验证**：verify/stage-07.md。要点：`grep "await ask(\|void ask(" src/` 零命中；ipc/dialog.ts 无 ask 导出（open/save 保留）；9 处调用点全部改 confirmDialog/toast。

**门禁**：五件

**commit**：`feat(overlay): ConfirmDialog/toast 统一浮层 + ask() 全替换 + 菜单规范`

## Stage 08 全局收敛（GL-01~GL-06）

**改动项**：GL-01/02/03/04/05/06

**agent 分工**（串行 2——豁免：全仓字号/圆角兜底扫描的文件集合事前不可枚举，并行无法证明零重叠，接受串行时长）：

| label | 负责项 | 文件 |
|---|---|---|
| global-css | GL-01/02/06 | src/App.css、全仓 fontSize/fontWeight/分组标题残留文件（执行期 grep 枚举） |
| density-empty | GL-03/04/05 | 全仓 borderRadius/间距残留文件、src/workspace/PageDockviewHost.tsx（Watermark）、src/features/navTree/（空态）、src/features/explorer/ExplorerPanel.tsx（空态） |

**验证**：verify/stage-08.md。要点：App.css 含滚动条与 focus-visible 规则；grep `fontSize.*(?:14|15|16|20)px\|font-size.*(?:14|15|16|20)px` 阶梯外零残留（xterm/CM 行高计算等白名单外）；空态三处 + Watermark 规范。

**门禁**：五件

**commit**：`style(global): 滚动条/焦点环/圆角/密度/空态/字号字重全仓收敛`

## Stage 09 文档同步（DOC-01~DOC-06）

**改动项**：DOC-01/02/03/04/05/06

**agent 分工**（并行 3，文档间天然零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| docs-src | DOC-01 | src/theme/CLAUDE.md、src/features/sideViews/CLAUDE.md、sidebar/CLAUDE.md、agentStatus/CLAUDE.md、agentHistory/CLAUDE.md、src/workspace/CLAUDE.md、src/lib/CLAUDE.md、src/panels/CLAUDE.md、src/ipc/CLAUDE.md、新建 src/features/navTree/CLAUDE.md、src/features/titleBar/CLAUDE.md |
| docs-root | DOC-02/04/05/06 | CONTEXT.md、docs/ui-redesign/requirements.md、.claude/adr.md、.claude/CLAUDE.md |
| docs-test | DOC-03 | .claude/test-inventory.md |

**验证**：verify/stage-09.md。要点：`grep -ri "darcula" src/**/CLAUDE.md .claude/CLAUDE.md CONTEXT.md` 零残留（adr.md 历史决策记录白名单）；模块索引含 navTree/titleBar；文档描述对照当前代码核实不撒谎（语义式断言）。

**门禁**：五件（照跑）

**commit**：`docs(ui-redesign): 实施期文档同步（CLAUDE.md/CONTEXT/ADR/测试清单）`

---

## 人工验证点汇总（执行收尾实测，全部 Stage 完成后）

1. **TB-07**：标题栏拖拽/双击最大化/三钮/关窗 PTY 清理（Stage 04 后也可单独先验）
2. **NAV-11**：导航树展开折叠/会话跳转/历史恢复/搜索/配置钮/旧 settings 启动（Stage 06）
3. **GL-04 sash 热区**：dockview 分隔条拖拽手感（Stage 08）
4. **视觉对照**：`npx tauri build --debug --no-bundle` 产物对照 `docs/ui-redesign/final-mockup.html` 两页（全 Stage 后）：明度阶梯/发丝线/页签/圆点/图标/浮层/滚动条/焦点环；终端 ANSI 实跑 claude；编辑器语法色开 .ts/.md 文件
5. **断网字体**：断网启动界面不回退非等宽（UI-202 验收）
6. **中文渲染**：CJK 回退 YaHei UI 不错位（UI-203 验收）
