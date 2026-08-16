# UI 重设计执行编排参数（ui-redesign-impl）

> 通用执行规则见 `/systematic-changes-execute`（单一真值源），本文件只写任务特定参数。
> 计划文档族：`docs/ui-redesign-impl/checklist.md`（46 项）+ `stages.md`（9 Stage）+ 本文件 + `workflows/`。

## Stage 表

| Stage | 名称 | 脚本 | verify | commit message |
|---|---|---|---|---|
| 01 | 配色方案替换 | workflows/stage-01-theme-scheme.js | workflows/verify/stage-01.md | `feat(theme): linear 配色方案替换 darcula + syntax 语法色槽位 + fail-safe 同步` |
| 02 | 字体内置 | workflows/stage-02-font.js | workflows/verify/stage-02.md | `feat(theme): JetBrains Mono 字体内置 + 全局字体栈统一` |
| 03 | 图标体系 | workflows/stage-03-icons.js | workflows/verify/stage-03.md | `feat(icons): lucide 线性图标体系 + 状态圆点替代 emoji + FileIcon 六色盘` |
| 04 | 自绘标题栏 | workflows/stage-04-titlebar.js | workflows/verify/stage-04.md | `feat(titlebar): 自绘 34px 一体化标题栏（decorations:false + 窗口控制）` |
| 05 | 页签栏改造 | workflows/stage-05-tabs.js | workflows/verify/stage-05.md | `feat(tabs): 扁平页签 + 底部 2px 指示条 + hover 关闭钮 + 圆点logo构成` |
| 06 | 侧栏 IA 重构 | workflows/stage-06-sidebar.js | workflows/verify/stage-06.md | `feat(sidebar): 统一导航树 IA（项目→页面→会话）+ 活动栏三槽 + 配置钮入口唯一化` |
| 07 | 浮层统一 | workflows/stage-07-overlay.js | workflows/verify/stage-07.md | `feat(overlay): ConfirmDialog/toast 统一浮层 + ask() 全替换 + 菜单规范` |
| 08 | 全局收敛 | workflows/stage-08-global.js | workflows/verify/stage-08.md | `style(global): 滚动条/焦点环/圆角/密度/空态/字号字重全仓收敛` |
| 09 | 文档同步 | workflows/stage-09-docs.js | workflows/verify/stage-09.md | `docs(ui-redesign): 实施期文档同步（CLAUDE.md/CONTEXT/ADR/测试清单）` |

## git add 路径枚举（Stage commit 限定）

`src/`、`src-tauri/`、`e2e-tests/`、`test/`、`index.html`、`package.json`、`package-lock.json`、`.claude/CLAUDE.md`、`.claude/test-inventory.md`、`.claude/adr.md`、`CONTEXT.md`、`docs/`（来源：config.json workflow.gitAddPaths + 本任务新增 index.html/package*.json/CONTEXT.md/.claude/adr.md）

## fix-loop args 规范

- 脚本：`docs/ui-redesign-impl/workflows/fix-loop.js`
- `args: { stage, failedItems, fixContext, verifyFile, constraints }`
- `verifyFile` = 对应 `docs/ui-redesign-impl/workflows/verify/stage-NN.md`（与 Stage 脚本同一真值源）
- `constraints`：各 Stage 特殊纪律——取值一律以对应 Stage 脚本头部注释的「fix-loop 调用本 Stage 时 args.constraints 传」为准（单点定义，本文不复制）；无该行则传空串

## 契约单点索引（跨 agent 共享，写死于各脚本头）

1. **linear 全值**：`docs/ui-redesign-impl/checklist.md` 附录 A（Stage 01 全体 agent 只准照抄）
2. **StatusDot API**：`<StatusDot status="working"|"attention"|"done"|"error" size={7}/>`，色映射 working→#86bb7a / attention→#d6b25e / done→#6b675f / error→#d9706b（Stage 03）
3. **icons.tsx 导出清单**：ActivityNav/ActivityFiles/ActivityCommit/ActivityConfig、ChevronRight/ChevronDown、Refresh、Search、History（时钟）、Close、Min/Max/CloseWin、Plus、EmptyFolder 等（Stage 03 定稿后 Stage 04/05/06 沿用）
4. **窗口控制 wrapper**：`minimizeWindow()/toggleMaximizeWindow()/closeWindow(): Promise<void>`（Stage 04）
5. **导航树契约**：视图 id `nav`、配置钮 id `config`（不入注册表）、data-e2e 选择器 `nav-tree`/`nav-row-project`/`nav-row-page`/`nav-row-session`/`nav-history-node`/`activity-btn-config`（Stage 06）
6. **ConfirmDialog/toast API**：`confirmDialog(opts): Promise<boolean>`、`toast.show(type, message)`（Stage 07）

## 进度跟踪表（执行期逐 Stage 更新）

| Stage | 状态 | commit | 备注 |
|---|---|---|---|
| 01 | 完成 | a02aafa | fix-loop 1 轮（TH-10 测试同步） |
| 02 | 完成 | d8dd797 | |
| 03 | 完成 | b43e196 | |
| 04 | 完成 | fd544be | 补 capabilities 四权限（三钮+拖拽）；TB-07 待收尾实测 |
| 05 | 完成 | 46feead | fix-loop 1 轮（R9 jsdom 颜色归一化） |
| 06 | 未开始 | — | 人工验证 NAV-11；含 npm run e2e |
| 07 | 未开始 | — | |
| 08 | 未开始 | — | |
| 09 | 未开始 | — | |

## 收尾（全部 Stage 完成后）

1. 人工验证点 6 项实测（stages.md 末节汇总）
2. `npx tauri build --debug --no-bundle` 产物对照 `docs/ui-redesign/final-mockup.html` 视觉验收
3. 进度跟踪表全绿后向用户汇报交付
