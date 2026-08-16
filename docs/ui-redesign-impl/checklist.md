# UI 重设计实施清单（ui-redesign-impl）

> 真值源：`docs/ui-redesign/requirements.md`（54 条）+ `design.md`（定值）+ 本清单附录 A（linear 方案全值契约）。
> 组织方式：按模块前缀分组，**优先级由 Stage 依赖顺序表达**（见 stages.md），不使用 P0-P4。
> 每项 = ID + 需求映射 + 位置 + 修复要点。全部 51 条需求（UI-405/406/407 经决策剔除，见 DOC-04）映射到 46 项。

## 决策定案（grill 已确认，全清单适用）

1. UI-405/406/407 剔除（Agent 面板/composer/状态行 = 远期新功能，非视觉重设计），需求规格补注记（DOC-04）
2. 新方案 `linear` 替换并删除 darcula；启动链未知 id 回退机制已内建（schemeRegistry.setActive），仅改默认 id
3. 自绘标题栏接受失去原生标题栏/阴影与 Snap Layouts 悬停预览（Win+方向键 Aero Snap 仍可用——OS 窗口管理，与 decorations:false 无关，2026-08 实机验证修订）
4. 活动栏底部「配置」钮 = 打开 hooks 配置面板；右键菜单 hooks 配置入口删除（入口唯一化）
5. 导航树：活跃会话挂页面下（panelId→pageId），历史折叠节点挂项目下（cwd 归属）
6. 会话行单行化：圆点+logo+标题+右侧迷你用量条（32×3+百分比 11px fg-4）；历史行 prompt 预览 → 原生 title tooltip
7. 图标 = lucide-react；文件彩色图标自绘六色盘 SVG；CLI logo PNG 保留
8. 字体 = @fontsource/jetbrains-mono（400/500 woff2）
9. 语法色 = editor 段扩 syntax 子组 + HighlightStyle；oneDark 仅作底
10. 新建应用内 toast；Tauri 原生桌面通知保留
11. 自绘 ConfirmDialog 全替换原生 ask()
12. 测试 = L2 守卫（色值单测 + emoji 扫描 + 新组件）+ L4 不回归 + 人工对照 final-mockup.html；不做截图对比
13. 派生细则：STATUS_EMOJI 删除、eventToStatus status 字符串保留（F3 映射逻辑不变，仅渲染层 emoji→圆点）；通知 CATEGORY_EMOJI 删除（标题纯文本）；sideBar settings 未知 view id 丢弃回退默认；自绘关闭钮经 getCurrentWindow().close() 复用 P1-19 关窗杀 PTY 链路；dockview 页签栏高 35px 为库默认值（--dv-tabs-and-actions-container-height，node_modules/dockview-core/dist/styles/dockview.css:40 实证）无需改

---

## TH — 配色方案替换（Stage 01）

- **TH-01**（UI-703/106/301）`src/theme/schemes/types.ts`：editor.overrides 新增 `syntax` 子组 9 键（property/string/number/keyword/function/type/operator/punctuation/comment）+ `plainText`/`lineNumber`/`lineNumberActive` 3 键；ui 段标量新增 `accentFg`、`selectionHoverBg`、`titlebarBg` 3 键（强调派生色与标题栏 chrome 底 #141416 落位，防硬编码违反 #6）。各新键带区域级消费注释（D8 规范）。`ColorScheme.id` 注释「回退 darcula」改「回退 linear」
- **TH-02**（UI-101~110、701、702、704~706）新建 `src/theme/schemes/linear.ts`：四段全值 = 附录 A（契约单点，执行 agent 只准照抄）；对象标注 `: ColorScheme`；文件头写 fail-safe 交叉引用注释（照 darcula.ts:11-15 格式，值改 #0a0a0b/#ece9e4/#d9706b）
- **TH-03** `src/theme/schemes/index.ts`：注册改 `linear`；删除 `src/theme/schemes/darcula.ts`
- **TH-04** `src/theme/schemeRegistry.ts:14`：`DEFAULT_SCHEME_ID` `"darcula"`→`"linear"`；文件头与 getActive/setActive/getDefaultId/_reset 注释中「darcula」字样同步
- **TH-05** `src/main.tsx:36-40`：默认 schemeId `"darcula"`→`"linear"`；注释同步
- **TH-06**（UI-111）fail-safe 三处同步：`index.html:10` body background、`src-tauri/tauri.conf.json:20` backgroundColor 均 → `#0a0a0b`；`src/main.tsx:28` 超时错误页 → 背景 `#0a0a0b`、文字 `#ece9e4`、强调 `#d9706b`（现为 #1e1f22/#e35f6c）
- **TH-07**（UI-703）`src/theme/overrides.ts`：新增导出 `editorSyntaxHighlight(): Extension`——`syntaxHighlighting(HighlightStyle.define(...))`，tags 映射：propertyName←syntax.property、string←string、number←number、keyword←keyword、`tags.function(tags.variableName)`←function、typeName←type、operator←operator、punctuation←punctuation、comment←comment；`editorColorOverrides()` 增正文/行号规则（`"&.cm-editor .cm-content"` color←plainText、`"&.cm-editor .cm-gutters"` backgroundColor←background + color←lineNumber + borderRight 发丝线、`"&.cm-editor .cm-lineNumbers .cm-gutterElement"` color←lineNumber、活跃行号←lineNumberActive），**全部带 `&.cm-editor` 前缀**（ACC-05：mountStyles reverse 层叠，平级选择器恒输 oneDark）
- **TH-08**（UI-703）5 处 CM 消费点在扩展数组中注入 `editorSyntaxHighlight()`，**位置在 `editorTheme` 之前**（reverse 后自定义规则排最后=恒胜，ACC-05）：`src/panels/editor/useCodeMirror.ts:289`、`src/panels/gitshow/GitShowPanel.tsx:142`、`src/panels/diff/DiffPanel.tsx:520,566`、`src/panels/hooksConfig/JsonMode.tsx:162`
- **TH-09** `src/theme/index.ts` barrel + `src/theme/colors.ts` facade：schemes 导出 darcula→linear；facade 新增 `ACCENT_FG`、`SELECTION_HOVER_BG`、`TITLEBAR_BG` 3 导出（31→34）
- **TH-10** L2 测试同步（darcula 色值断言全量改附录 A 值）：`src/__tests__/colors.test.ts`（gitFile 7/gitGutter 3/explorer/sidebar/23+2 标量/agentStatusUsage/ROOT_CSS_VARS --sl-fg-primary）、`scheme-registry.test.ts`（import darcula→linear、回退断言、四段完整性——editor 段键数变）、`overrides.test.ts`、`bootstrap.test.ts`、`main-bootstrap.test.tsx:35`（#e35f6c→#d9706b）、`explorer-git-status.test.tsx`（gitFile 值 ×2 组）、`git-gutter.test.ts:115-117`、`commit-context-menu-ui.test.tsx:246`（hover 值）、`explorer-selection.test.tsx:12,84,168`、`sideBarArea.test.tsx:461`（注释）。新增：syntax 9 键与 accentFg/selectionHoverBg 断言、`editorSyntaxHighlight` 导出存在性
- **TH-11** L3 `test/terminal/theme-options.test.ts:47-52`：ANSI 6 色断言改附录 A 值；本 Stage 门禁加入 `npm run test:l3`

## FT — 字体（Stage 02）

- **FT-01**（UI-202）`package.json` 新增 `@fontsource/jetbrains-mono`；`src/main.tsx` ② 阶段 import 其 400/500 woff2 入口（随产物打包，断网可用）
- **FT-02**（UI-201/203）`src/App.css:8` 全局栈 → `"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace`
- **FT-03**（UI-201）`src/panels/editor/useCodeMirror.ts:48,57` CM fontFamily → 同规格栈
- **FT-04**（UI-201）`src/panels/terminal/theme.ts:13` xterm fontFamily → 同规格栈
- **FT-05**（UI-204）字号阶梯收敛 11/11.5/12/12.5/13px：grep `fontSize`/`font-size` 全仓，阶梯外值逐处收敛。**分布实施**：各区域 Stage 顺手改自己文件，Stage 08 兜底全仓扫描
- **FT-06**（UI-205）字重仅 400/500：grep `fontWeight`/`font-weight` 全仓收敛（bold/600/700 → 500）。同 FT-05 分布实施
- **FT-07**（UI-206）分组标题 11px+全大写+0.08em+fg-3：各视图区块头（EXPLORER/COMMIT/AGENT STATUS 残留处/导航树「导航」）。随区域 Stage 实施，Stage 08 兜底
- **FT-08**（UI-201 附属）`src/main.tsx:28` 错误页 `font-family:monospace` → 规格栈

## IC — 图标（Stage 03）

- **IC-01**（UI-601）`package.json` 新增 `lucide-react`；新建 `src/lib/icons.tsx` 集中封装：统一 15px（紧凑 12–13px）、1.5px 描边、currentColor；导出本应用用到的图标集（活动栏 4、chevron 开合、刷新、搜索、时钟、关闭 ×、窗口控制 3、「+」、空态文件夹等），组件只准从本文件引用（单点）
- **IC-02**（UI-504）新建 `src/lib/StatusDot.tsx`：7px 圆点，绿 `#86bb7a` 运行/黄 `#d6b25e` 等待/灰 `#6b675f` 空闲，无描边/光晕/动画；props = status 字符串（working/attention/done/error → 绿/黄/绿?灰映射——done/error 视觉归灰?绿?：**写死契约**：working→绿、attention→黄、done→灰、error→红 `#d9706b`（F3 四态完整映射，设计绿黄灰三档+错误红））
- **IC-03**（UI-601）`src/lib/agentStatus.ts:20-33`：STATUS_EMOJI 常量删除；eventToStatus 返回 status 字符串不变（F3 逻辑不动）；渲染层全部改 StatusDot。链路改造：`useCommandDetection.ts:80-83`（icon→status 字段）、`useXterm.ts:409-414`、`TerminalPanel.tsx:84-102`（updateParameters tabIcon→tabStatus）、`PageDockviewHost.tsx` DefaultTab tabIcon 分支 → StatusDot（tabLogo 分支不动）
- **IC-04**（UI-602）`src/features/explorer/FileIcon.tsx` 重构：文件夹 = 描边款 SVG；文件 = 描边+小色块款 SVG，色限六色盘 `#7fa8e8/#d6b25e/#93b573/#d9706b/#b48ce0/#6fbfc4`；扩展名→色系映射表保留现有分组逻辑（ts/js/rs/py/json/md/html/css/配置/默认）；gitStatus 着色保留（gitFile token 不变）；`src/__tests__/file-icon.test.tsx` 同步
- **IC-05**（UI-601）树箭头 `▶`/`▼` → chevron SVG（12px fg-3）：`SidebarTree.tsx:174,256`、`FileTree.tsx`、`AgentStatusView.tsx` 折叠区、`HistorySessionList.tsx:441` 组标题
- **IC-06**（UI-604）`sideViewDefs.ts` 4 视图 icon 字段 emoji → lucide 组件（Stage 06 重组为三槽时沿用）；活动栏图标 15px 默认 fg-3 hover fg-1+`#222227`
- **IC-07** `src/features/notifications/useAgentNotifications.ts:41-43`：CATEGORY_EMOJI 删除（通知标题纯文本）
- **IC-08** 杂项 emoji 清除：`FileTree.tsx:211` ⏳（改 spinner 线性 SVG 或「…」）、`HistorySessionRow.tsx:108` ✗（改 lucide x-circle）、`AgentStatusRow.tsx:86`/`HistorySessionRow.tsx:51,86` 状态 emoji（并入 IC-03 链路）
- **IC-09**（UI-601 验收）新建 `src/__tests__/emoji-scan.test.ts`：读 `src/` 全部 .ts/.tsx 源文件（排除测试自身与本守卫白名单——白名单仅允许终端输出语义字符），断言无装饰 emoji 字面量（📁📂📋🤖🌿⭐🟠⚡✅❌🕐💾📄✏️🗑➕🔍⚙️🔄🖖📜🐍📝🌐🎨📦⏳🔐✗▶▼ 等集合）

## TB — 自绘标题栏（Stage 04）

- **TB-01**（UI-301）`src-tauri/tauri.conf.json` windows 段新增 `"decorations": false`
- **TB-02**（UI-301）新建 `src/features/titleBar/TitleBar.tsx`：34px、`#141416` 底 + 底部发丝线；左 app 标识（logo+slTerminal，12px）；中 `<b>项目</b> / 页面`（数据来源：projects store 活跃项目+活跃页面名）；右自绘最小化/最大化/关闭三钮（38×26，图标 lucide 12px，hover `#222227`，关闭 hover `#c04747`）
- **TB-03** `src/ipc/window.ts` 新增 wrapper：`minimizeWindow()`/`toggleMaximizeWindow()`/`closeWindow()`（getCurrentWindow 已有 import 模式）；关闭 = `close()` 触发 onCloseRequested → 复用 P1-19 关窗杀 PTY 链路（禁止直接 process.exit）
- **TB-04** 拖拽：标题栏容器 `data-tauri-drag-region`（按钮区排除）；双击切换最大化（dblclick → toggleMaximize）
- **TB-05** `src/App.tsx` 骨架：ready 后改列向 flex（TitleBar + Workspace）；启动加载页不动
- **TB-06** `src/__tests__/title-bar.test.tsx` 新建：渲染三段结构、三钮调对应 wrapper（mock ipc/window）、双击 toggleMaximize、项目/页面名显示
- **TB-07** 人工验证点（实机）：拖拽移动、双击最大化/还原、三钮功能、关闭后 PTY 清理（P1-19 链路）、窗口阴影缺失与直角接受

## TAB — 页签栏（Stage 05）

- **TAB-01**（UI-401）`PageDockviewHost.tsx` DefaultTab 扁平化：激活 = 底 `#0a0a0b`（dockview 变量已置）+ **底部 2px `#6e9ff2` 指示条**（自绘：tab 内绝对定位底条或 borderBottom）+ fg-1；未激活透明 fg-3；hover fg-1 不变底。35px 高为库默认
- **TAB-02**（UI-402）关闭 × hover 页签才显（14px、圆角 4、hover 底 `#2b2b31`；激活页签不常驻）；dockview `--dv-icon-hover-background-color` 已置 `#2b2b31`
- **TAB-03**（UI-403）页签构成：终端/agent = StatusDot + CLI logo（tabLogo 分支保留）+ 名称；文件页签 = FileIcon 彩色图标 + 名称（editor/html/gitshow/diff 页签按文件扩展名取图标）；emoji 渲染分支已随 IC-03 删除
- **TAB-04**（UI-404）RightHeader「+」钮：22px、圆角 4、fg-3、hover `#222227`
- **TAB-05** 页签相关 L2 测试同步：`workspace-header-actions.test.tsx`、`workspace-page-dockview.test.tsx`、tabIcon/tabLogo 相关断言（圆点替换 emoji 断言）

## NAV — 侧栏 IA 重构（Stage 06）

- **NAV-01**（UI-303/501/502/503）新建 `src/features/navTree/`（NavTree.tsx + 行组件 + useNavTree hook）：层级恰为 项目→页面→会话；chevron 12px fg-3 + 图标 + 名称 + 右侧 11px fg-4 元数据；行高 28（会话 30）、圆角 5、hover `#222227`；选中 `rgba(110,159,242,0.13)` 底（hover 0.22=selectionHoverBg）+ fg-1；每级左缩 15px + 1px 发丝引导线（sidebar.treeGuide token）
- **NAV-02**（UI-504/109）活跃会话行：StatusDot + CLI logo 14px + 标题 + 右侧（32×3 迷你用量条 + 百分比 11px fg-4）；数据 = useAgentStatus 行（panelId→pageId 归属页面）；点击行聚焦对应终端页签（沿用 agentStatus 现有跳转逻辑）；阈值 ≥90/≥70/≥50 逻辑不变
- **NAV-03**（UI-303/505）历史会话折叠节点挂项目下：时钟图标 +「历史」+ 计数 pill（`#1a1a1e` 底 fg-4）；展开 = 历史行（StatusDot+logo+标题+右侧相对时间）；prompt 预览 → 原生 title tooltip；数据 = agentHistory scan（cwd 归属项目）；双击恢复/右键菜单（复制恢复命令/分支恢复/删除）沿用 historyContextMenu 策略
- **NAV-04**（UI-506）搜索框：分组标题「导航」+ 刷新钮下，`#1a1a1e` 底圆角 5、12px、占位「搜索项目 / 页面 / 会话…」fg-4、focus 描边 accent；过滤项目/页面/会话名（子串不区分大小写，父节点因子命中而显示）
- **NAV-05**（UI-302）`sideViewDefs.ts` 重组三槽：`nav`（导航树）/`explorer`/`commit` + 底部「配置」钮（打开 hooksConfig 面板，复用 SidebarTree 现「打开 Hooks 配置」逻辑——执行期提取为公共函数）；`sideBarState.ts` DEFAULT_ZONES 改三槽；`ActivityBar.tsx` 46px 宽、34×34 圆角 6 钮、激活 = accent-dim 底+accent-fg 图标+左 2px accent 竖条、配置钮固定底部；agent-status 注册删除
- **NAV-06** `SidebarTree.tsx:466-503,523-544` 右键菜单「打开 Hooks 配置」项删除（项目/页面菜单其余项不变）；SidebarTree 本体随 NAV-05 退役删除（NavTree 承接项目/页面 CRUD + 右键菜单 + 内联重命名 + 空布局 makeEmptyLayout 逻辑——全部迁移，行为不变）
- **NAV-07** `src/stores/sideBar.ts`：持久化迁移——恢复时 zones/open 中未注册 id（projects/agent-status）丢弃回退默认（reconcileZones R9 已有过滤语义，确认覆盖 open 字段）；`sanitizeSideBar` 校验逻辑适配三槽
- **NAV-08** 旧组件删除：`agentStatus/AgentStatusView.tsx`、`AgentStatusRow.tsx`、`agentHistory/AgentHistorySections.tsx` 删除；`useAgentStatus.ts` 保留（数据层供 NAV-02）；`HistorySessionList/HistorySessionRow` 改造迁入导航树（单行化 + title tooltip）；E2E 兼容红线（agent-status-view/agent-status-row/"AGENT STATUS"）同步废除——E2E 用例重写见 NAV-10
- **NAV-09**（UI-505）项目行：500 字重 fg-1 + 彩色文件夹图标 + 当前项目「当前」pill（accent-dim 底 `#8fb4f5` 字 10px）；计数 pill `#1a1a1e` 底 fg-4（页面数/历史数）
- **NAV-10** 测试重写：L2 `sideBarState.test.ts`/`sideBar.test.ts`（三槽适配）、`activityBar.test.tsx`（46px/激活样式/配置钮）、`sideBarArea.test.tsx`、`workspace-sideviews.test.tsx`、agentStatus/agentHistory 相关测试迁入 navTree；E2E `sidebar.e2e.ts`/`agent.e2e.ts`/`mockcli.e2e.ts`（agent-status→nav 视图、活动栏序位、⚡断言改圆点存在性断言、helpers `__slterm_e2e_toggleSideView` 参数）
- **NAV-11** 人工验证点（实机）：三级树展开/折叠、会话行跳转聚焦、历史恢复（双击/右键）、搜索过滤、配置钮打开 hooks 面板、旧 settings.json（含 projects/agent-status）启动不崩

## OV — 浮层（Stage 07）

- **OV-01**（UI-801/803）新建 `src/lib/ConfirmDialog.tsx`：遮罩 `rgba(0,0,0,0.55)`、卡片 `#1a1a1e` 底 + `rgba(255,255,255,0.09)` 描边 + 圆角 8 + 阴影 `0 8px 32px rgba(0,0,0,0.35)`；主按钮 `#6e9ff2` 底 + `#0c1220` 字、次按钮 `#222227` 底 + `#ece9e4` 字；Promise<boolean> API（写死契约，见 stages.md）；`src/lib/toast.tsx`：右上堆叠、语义色 12% 底 + 1px 语义描边 + fg-1 + 圆角 8、自动消失（成功/警告/错误三型）（UI-804）
- **OV-02**（UI-803）ask() 9 处调用点全替换为 ConfirmDialog：`commitContextMenu.ts:53,72`、`HistorySessionList.tsx:365`、`HooksConfigPanel.tsx:184`、`useHooksConfig.ts:149,194,201`（194/201 为纯告警——改 toast 或 ConfirmDialog 单钮，执行期按语义定）、`ExplorerPanel.tsx:109`、`FileTree.tsx:307,357`；`ipc/dialog.ts` 删 ask 保留 open/save（文件对话框原生保留）；`dialog-e2e-hook.test.ts` 同步
- **OV-03**（UI-801）`TerminalRenameDialog.tsx`、`SessionActionDialog.tsx` 统一浮层规范（l3 底/0.09 描边/圆角 8/阴影/按钮规格）
- **OV-04**（UI-802）右键菜单 4 处统一（PageDockviewHost 页签菜单、FileTree、SidebarTree→NavTree 承接、CommitFileList）：项 28px、圆角 5、hover `#222227`、危险项 `#d9706b`、边框 0.09、阴影 UI-801
- **OV-05**（UI-805）`ExplorerPanel.tsx:400-434` 错误横幅：token 已 Stage 01 换值，核对结构（关闭 × 图标 lucide 化）

## GL — 全局收敛（Stage 08）

- **GL-01**（UI-807）`App.css` 新增全局 `::-webkit-scrollbar`：9px、轨道透明、滑块 `rgba(255,255,255,0.10)`/hover 0.20/拖动 0.28、圆角 5、无箭头；dockview/terminal 滚动条变量已 Stage 01 置值，本项核对实际生效
- **GL-02**（UI-808）`App.css` 新增 `:focus-visible { outline: 1px solid #6e9ff2 }`；审计 10 处组件级 `outline:"none"`（TerminalRenameDialog:142、AgentHistorySections:109 已删、ActivityBar:53、ExplorerPanel:446、SidebarTree:272 已删、FileTree:459,497,541,596,636、HandlerForm:184）——保留无键盘语义处、键盘可达处改 accent 环
- **GL-03**（UI-306）圆角收敛 4/5/6/8/pill（页签 0）：grep `borderRadius`/`border-radius` 全仓收敛
- **GL-04**（UI-304/305）密度收敛：活动栏 46（NAV-05 已做）/页签栏 35（默认）/树行 28/会话行 30；组件间距 4/8/12/16/24；dockview sash 视觉 1px 发丝线（变量已置）+ 拖拽热区 ≥4px（dockview 默认 sash 尺寸，执行期实测确认）
- **GL-05**（UI-806）空态统一：空文件树/无历史会话/无搜索结果 + Watermark（`PageDockviewHost.tsx:67-100` createWatermark）——15px 线性图标 fg-4 + 说明 fg-3 + 可选次按钮
- **GL-06** FT-05/06/07 兜底：全仓 grep fontSize/fontWeight/分组标题，阶梯外残留清零

## DOC — 文档同步（Stage 09）

- **DOC-01** 子路径 CLAUDE.md 同步：`src/theme/CLAUDE.md`（linear 方案/syntax 槽位/facade 33 导出/fail-safe 新值）、`src/features/sideViews/CLAUDE.md`（三槽+配置钮、agent-status 退役）、`src/features/sidebar/CLAUDE.md`（NavTree 取代 SidebarTree）、`src/features/agentStatus/CLAUDE.md`（视图删除、useAgentStatus 留存）、`src/features/agentHistory/CLAUDE.md`（迁移导航树）、`src/workspace/CLAUDE.md`（DefaultTab 形态）、`src/lib/CLAUDE.md`（icons/StatusDot/ConfirmDialog/toast）、`src/panels/CLAUDE.md`（页签圆点）、`src/ipc/CLAUDE.md`（dialog ask 删除+window 三 wrapper）、新建 `src/features/navTree/CLAUDE.md`、`src/features/titleBar/CLAUDE.md`
- **DOC-02** `CONTEXT.md`：术语更新（统一导航树/状态圆点已定义处核实、「Agent Status 视图」条退役标注、活动栏「配置」钮）
- **DOC-03** `.claude/test-inventory.md`：新增/修改/删除用例全量同步
- **DOC-04** `docs/ui-redesign/requirements.md`：UI-405/406/407 补「远期愿景，本期不实施（2026-08 决策）」注记
- **DOC-05** `.claude/adr.md`：ADR-0003 补充实现期决策（剔除 Agent 面板/配置钮入口唯一化/导航树挂法/lucide+fontsource 依赖）
- **DOC-06** 根 `.claude/CLAUDE.md`：模块索引增 navTree/titleBar 行、sidebar/agentStatus 行更新；编号索引核实

---

## 附录 A：linear 方案四段全值（契约单点——执行 agent 照抄，禁止自估）

> 值来源：requirements.md UI-1xx/7xx + design.md §2。types.ts 槽位 ↔ 值一一对应。

### ui 段

```ts
gitFile:    { modified:"#d6b25e", added:"#86bb7a", untracked:"#6fbfc4", deleted:"#d9706b", renamed:"#6e9ff2", conflict:"#d9706b", ignored:"#6b675f" }
gitGutter:  { modified:"#d6b25e", added:"#86bb7a", deleted:"#d9706b" }
explorer:   { bg:"#101012", fg:"#b3aea6", hover:"#222227", arrowClosed:"#8a857d", arrowOpen:"#8a857d" }
sidebar:    { bg:"#101012", fg:"#b3aea6", hover:"#222227", selected:"rgba(110,159,242,0.13)",
              border:"rgba(255,255,255,0.055)", contextMenuBorder:"rgba(255,255,255,0.09)",
              contextMenuShadow:"0 8px 32px rgba(0,0,0,0.35)", treeGuide:"rgba(255,255,255,0.055)" }
errorBanner:{ bg:"rgba(217,112,107,0.12)", border:"#d9706b", fg:"#ece9e4" }
agentStatusUsage: { low:"#86bb7a", medium:"#a9c686", high:"#d6b25e", critical:"#d9706b" }
// 标量（23 既有 + 2 新增）
panelBg:"#0a0a0b", sidebarBg:"#1a1a1e", secondaryBg:"#222227", appBg:"#0a0a0b",
appBgPrimary:"#0a0a0b", appFg:"#b3aea6", editorBg:"#0a0a0b", sidebarFg:"#ece9e4",
errorFg:"#d9706b", placeholderFg:"#6b675f", buttonFg:"#ece9e4", dimFg:"#8a857d",
inputBg:"#1a1a1e", inputBorder:"rgba(255,255,255,0.09)", focusBorder:"#6e9ff2",
activeSelectionBg:"rgba(110,159,242,0.13)", separatorBg:"rgba(255,255,255,0.055)",
contextMenuBorder:"rgba(255,255,255,0.09)", shadowMenu:"rgba(0,0,0,0.55)",
htmlPanelLoadingFg:"#8a857d", htmlPanelIframeBg:"#FFFFFF", onAccentFg:"#0c1220",
explorerSelectionBg:"rgba(110,159,242,0.13)",
accentFg:"#8fb4f5",                       // 新增：强调派生前景色（活动栏激活图标/状态行模型段）
selectionHoverBg:"rgba(110,159,242,0.22)", // 新增：选中行 hover（accent-dim-2）
titlebarBg:"#141416"                       // 新增：自绘标题栏 chrome 底（明度阶梯 l2）
```

### terminal 段（25 键）

```ts
foreground:"#cfcac1", background:"#0a0a0b", cursor:"#6e9ff2", cursorAccent:"#0a0a0b",
selectionBackground:"rgba(110,159,242,0.28)", selectionForeground:"#f0ede8",
scrollbarSliderBackground:"rgba(255,255,255,0.10)", scrollbarSliderHoverBackground:"rgba(255,255,255,0.20)",
scrollbarSliderActiveBackground:"rgba(255,255,255,0.28)",
black:"#0a0a0b", red:"#d9706b", green:"#93b573", yellow:"#d6b25e",
blue:"#7fa8e8", magenta:"#b48ce0", cyan:"#6fbfc4", white:"#cfcac1",
brightBlack:"#7d7871", brightRed:"#e2877f", brightGreen:"#a8c98d", brightYellow:"#e3c67f",
brightBlue:"#9dbfee", brightMagenta:"#c6a6e8", brightCyan:"#8dd0d4", brightWhite:"#f0ede8"
```

### editor 段

```ts
theme: oneDark, // 不变，仅作底座
overrides: {
  background: "#0a0a0b",
  lint: { error:"#d9706b", warning:"#d6b25e", info:"#6e9ff2", hint:"#8a857d",
          activeBackground:"rgba(110,159,242,0.13)", tooltipBackground:"#1a1a1e",
          tooltipBorder:"rgba(255,255,255,0.09)" },
  searchMatch: { match:"rgba(214,178,94,0.25)", matchOutline:"transparent",
                 selected:"rgba(214,178,94,0.45)", selectionMatch:"rgba(214,178,94,0.25)" },
  syntax: { property:"#d9827e", string:"#93b573", number:"#d89a66", keyword:"#b48ce0",
            function:"#7fa8e8", type:"#6fbfc4", operator:"#6fbfc4",
            punctuation:"#7d7871", comment:"#6b675f" },        // 新增 9 键
  plainText: "#b3aea6", lineNumber: "#6b675f", lineNumberActive: "#b3aea6" // 新增 3 键
}
```

### libraries 段

```ts
dockview: {
  "--dv-group-view-background-color": "#0a0a0b",
  "--dv-tabs-and-actions-container-background-color": "#101012",
  "--dv-activegroup-visiblepanel-tab-background-color": "#0a0a0b",
  "--dv-activegroup-hiddenpanel-tab-background-color": "transparent",
  "--dv-inactivegroup-visiblepanel-tab-background-color": "#0a0a0b",
  "--dv-inactivegroup-hiddenpanel-tab-background-color": "transparent",
  "--dv-tab-divider-color": "transparent",
  "--dv-separator-border": "rgba(255,255,255,0.055)",
  "--dv-paneview-header-border-color": "rgba(255,255,255,0.055)",
  "--dv-activegroup-visiblepanel-tab-color": "#ece9e4",
  "--dv-activegroup-hiddenpanel-tab-color": "#8a857d",
  "--dv-inactivegroup-visiblepanel-tab-color": "#ece9e4",
  "--dv-inactivegroup-hiddenpanel-tab-color": "#8a857d",
  "--dv-drag-over-background-color": "rgba(110,159,242,0.13)",
  "--dv-icon-hover-background-color": "#2b2b31",
  "--dv-floating-box-shadow": "0 8px 32px rgba(0,0,0,0.35)",
  "--dv-floating-border": "1px solid rgba(255,255,255,0.09)",
  "--dv-tabs-container-scrollbar-color": "rgba(255,255,255,0.20)",
  "--dv-scrollbar-background-color": "rgba(255,255,255,0.10)",
  "--dv-paneview-active-outline-color": "#6e9ff2",
},
allotment: { separatorBorder: "rgba(255,255,255,0.055)", focusBorder: "#6e9ff2" }
```
