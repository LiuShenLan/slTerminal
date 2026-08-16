# Review 修复 Stage 划分（ui-redesign-review-fix）

> 9 Stage 全串行（每 Stage commit）。划分原则：Stage 内 agent 文件零重叠；Stage 间允许重复碰同一文件。
> 断言唯一真值源：`workflows/verify/stage-NN.md`（stage 脚本与 fix-loop 共用）。
> 跨 agent 契约（ROOT_CSS_VARS 新键名、token 名、函数签名、组件更名）写死于各脚本头部，执行 agent 不各自推断。
> 清单原文：`docs/ui-redesign-review-fix/checklist.md`（48 项：SPEC 2 / FE 29 / TE 6 / DOC 11 内含 VER 注记）。

## Stage 01 浮层收尾（FE-01/02/03/09/24）

**改动项**：FE-01、FE-02、FE-03、FE-09、FE-24

**agent 分工**（并行 3，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| editor-overlay | FE-01 | src/panels/editor/useCodeMirror.ts、src/__tests__/use-code-mirror.test.ts、src/__tests__/editor-confirm.test.ts |
| diff-overlay | FE-02 | src/panels/diff/DiffPanel.tsx、src/__tests__/diff-panel.test.tsx |
| navtree-overlay | FE-03/09/24 | src/features/navTree/NavTree.tsx、src/workspace/pageApis.ts、src/__tests__/nav-tree.test.tsx、src/__tests__/pageapis.test.ts |

**实现要点**：
- confirmDialog/toast API 契约（既有，不新增）：`confirmDialog(opts): Promise<boolean>`、`toast.show(type, message)`，import 自 `src/lib` barrel；危险确认传 `danger: true`
- FE-01 语义对照：`:265`（外部修改净文件重载确认）、`:392`（脏文件确认）confirm 确认=继续、取消=中止；`:177` 保存失败 alert 改 `toast.show("error", ...)` 纯通知
- FE-09 上提后签名写死（跨 agent 不需要——同 agent 内两文件，但 verify 断言引用）：`findPanelForSession(cliId: string, sessionId: string): string | undefined`、`findPageIdForPanelId(panelId: string): string | null`，由 `workspace/pageApis.ts` 导出；NavTree 内调用点 :325,333,340 改调；`NavTree.tsx:56` TerminalRegistry import 删除后 navTree 零引用 panels/terminal
- FE-09 行为零变化：复合键 `keyOf(cliId, sessionId)` 匹配、usageSourcePath 回退、B14 前缀匹配优先 + parseTerminalPageId 兜底——逐字搬运，只换位置
- FE-24 仅删 handleNewPage 返回值（调用方 :560 不消费）

**验证**：verify/stage-01.md。要点：`grep "window\.alert\|window\.confirm" src/` 零命中；NavTree 无 TerminalRegistry import；pageApis 两导出存在；pageapis 新用例通过。

**门禁**：1. `npx tsc --noEmit` 2. `npx eslint src/` 3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 4. `npm test` 5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

**commit**：`fix(overlay): 原生 alert/confirm 清零——统一 confirmDialog/toast + 会话反查上提 pageApis`

## Stage 02 配色单点收敛（FE-07/08/20/21）

**改动项**：FE-07、FE-08、FE-20、FE-21

**agent 分工**（并行 4，文件零重叠；token/键名契约写死脚本头）：

| label | 负责项 | 文件 |
|---|---|---|
| theme-token | FE-07/08 方案侧 | src/theme/schemes/types.ts、schemes/linear.ts、colors.ts、index.ts、src/__tests__/scheme-registry.test.ts、colors.test.ts |
| titlebar-fix | FE-07 组件侧 + FE-21 | src/features/titleBar/TitleBar.tsx、src/__tests__/title-bar.test.tsx |
| appcss-vars | FE-08 样式侧 | src/App.css |
| fileicon-git | FE-20 | src/features/explorer/FileIcon.tsx、src/__tests__/file-icon.test.tsx |

**实现要点**：
- 跨 agent 契约（脚本头写死）：① ui 段新标量 `titlebarCloseHover: "#c04747"`，facade 导出名 `TITLEBAR_CLOSE_HOVER_BG`；② ROOT_CSS_VARS 新 4 键 `--sl-focus-border`（← ui.focusBorder #6e9ff2）、`--sl-scrollbar-slider` / `--sl-scrollbar-slider-hover` / `--sl-scrollbar-slider-active`（← terminal.scrollbarSlider*，rgba(255,255,255,0.10/0.20/0.28)）——值与现硬编码逐字相同，D1 零视觉变化
- theme-token：scheme-registry.test.ts 标量计数 26→27；colors.test.ts token 集合 + ROOT_CSS_VARS 键集合 2→6；index.ts:3 注释 34→35
- titlebar-fix：FE-21  selector 拆分执行期按最小改动定（窄 selector 或派生上提），行为不变
- appcss-vars：`:59,63,66,75` 四处改 var()；`:5,72-73` 注释重写（「var() 不可行」理由消失）
- FE-20：isDir 分支 `statusColorMap[gitStatus] ?? EXPLORER_COLORS.fg`

**验证**：verify/stage-02.md。要点：grep `src/features/titleBar/TitleBar.tsx` 无 hex 字面量；App.css 无 hex/rgba 字面量（var() 全覆盖）；scheme-registry/colors 计数断言与新键集合一致。

**门禁**：同 Stage 01 五条

**commit**：`fix(theme): 硬编码色收敛——titlebarCloseHover token + ROOT_CSS_VARS 扩 4 键 + TitleBar 订阅拆分`

## Stage 03 右键菜单与树行（FE-04/05/06/15/16）

**改动项**：FE-04、FE-05、FE-06、FE-15、FE-16

**agent 分工**（并行 3，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| dockview-menu | FE-04/16 | src/workspace/PageDockviewHost.tsx、src/__tests__/workspace-header-actions.test.tsx |
| filetree-hover | FE-05 | src/features/explorer/FileTree.tsx、src/__tests__/explorer-selection.test.tsx（hover 相关用例所在文件执行期核实后就近放） |
| navctx-hover | FE-06/15 | src/features/navTree/NavContextMenu.tsx、NavPageRow.tsx |

**实现要点**：
- FE-04：`const newTerminalId = nextPanelId()` 从菜单构建体（:262）移入「新建终端」action 闭包——右键不点即不消耗编号；回归用例断言连续构建两次菜单后执行新建编号不跳
- FE-05/06：hover 改 React state 驱动（参考同仓 NavProjectRow/TabContextMenuItem 既有模式）；选中态优先于 hover 的语义不变（FileTree.tsx:204-205 注释契约）
- FE-15/16：圆角档修正（UI-306：输入框 8 / 按钮 6）

**验证**：verify/stage-03.md。要点：PageDockviewHost 菜单构建路径无 nextPanelId 调用（须 Read 确认在 action 闭包内）；FileTree/NavContextMenu 无 `onMouseEnter`/`onMouseLeave` 直改 `style.background`（语义式，须 Read 确认）；圆角值 grep。

**门禁**：同 Stage 01 五条

**commit**：`fix(menu): panelId 延迟到 action 执行 + 右键菜单 hover 改 React state + 圆角档修正`

## Stage 04 对话框与通知形态（FE-12/13/14/28/29）

**改动项**：FE-12、FE-13、FE-14、FE-28、FE-29

**agent 分工**（并行 4，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| confirm-dialog | FE-12/28 | src/lib/ConfirmDialog.tsx、src/__tests__/confirm-dialog.test.tsx |
| rename-dialog | FE-13 | src/workspace/TerminalRenameDialog.tsx、src/__tests__/terminal-rename-dialog.test.tsx |
| session-dialog | FE-14 | src/features/agentHistory/SessionActionDialog.tsx、src/__tests__/agent-history-action-dialog.test.tsx |
| toast-a11y | FE-29 | src/lib/toast.tsx、src/__tests__/toast.test.tsx |

**实现要点**：
- FE-28 焦点陷阱最小实现：挂载后确认钮 focus()；Tab/Shift+Tab 在取消/确认两钮间循环；Enter 由按钮原生提交（焦点在钮上即生效），Esc/遮罩既有语义不动；**`data-e2e="confirm-ok"/"confirm-cancel"/"confirm-dialog-mask"` 选择器不变**（history.e2e.ts:602 依赖——红线）
- FE-12：主次钮 borderRadius 4→6；删次钮 `border: 1px solid ...`（UI-803 只规定底色/字色）
- session-dialog 测试文件已核实为 `src/__tests__/agent-history-action-dialog.test.tsx`

**验证**：verify/stage-04.md。要点：三弹窗圆角 grep 断言；ConfirmDialog 含 autoFocus/focus 调用与 Tab 循环逻辑（须 Read 确认）；toast 容器 role="status" 断言；data-e2e 选择器零变更（grep 白名单）。

**门禁**：同 Stage 01 五条

**commit**：`fix(dialog): 弹窗圆角档收敛 + ConfirmDialog 焦点陷阱 + ToastHost aria-live`

## Stage 05 侧栏与活动栏（FE-17/19/22/23）

**改动项**：FE-17、FE-19、FE-22、FE-23

**agent 分工**（并行 3，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| activitybar-fix | FE-17/23 | src/features/sideViews/ActivityBar.tsx、src/__tests__/activityBar.test.tsx |
| sidebar-ratio | FE-19 | src/features/sideViews/SideBarArea.tsx、src/__tests__/sideBarArea.test.tsx |
| reconcile-pure | FE-22 | src/features/sideViews/sideBarState.ts、src/__tests__/sideBarState.test.ts |

**实现要点**：
- FE-19：仅首次进入双视图（无持久化值）或越界（出 [0.1,0.9]）时回退 0.5；单↔双切换保留用户值——effect 条件收窄，勿动 store 持久化链
- FE-23：`relatedTarget` 包含判断（`e.currentTarget.contains(e.relatedTarget)` 时不清指示线），或统一 dragend/drop 清理——执行期取最小改动
- FE-22：`top.push` 前 `[...top]` 复制；bottom 同模式自查

**验证**：verify/stage-05.md。要点：ActivityBar 无 transition（grep）；reconcileZones 入参不被 mutate（测试断言）；splitRatio 保留用例通过。

**门禁**：同 Stage 01 五条

**commit**：`fix(sidebar): splitRatio 双开保留 + reconcileZones 纯函数化 + ActivityBar 动效/指示线修正`

## Stage 06 杂项收敛（FE-10/11/18/26/27）

**改动项**：FE-10、FE-11、FE-18、FE-26、FE-27

**agent 分工**（并行 4，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| app-misc | FE-10/26 | src/App.tsx、src/ipc/window.ts、src/__tests__/app.test.tsx |
| boundary-font | FE-11 | src/lib/ErrorBoundary.tsx、src/__tests__/error-boundary.test.tsx |
| gitshow-icon | FE-18 | src/panels/gitshow/GitShowPanel.tsx、src/lib/icons.tsx、src/__tests__/gitshow-panel.test.tsx |
| explorer-outline | FE-27 | src/features/explorer/ExplorerPanel.tsx、src/__tests__/explorer-focus.test.tsx |

**实现要点**：
- 全局字体栈唯一真值：`"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace`（UI-201）
- FE-18：icons.tsx 新增导出（lucide 三角告警图标，导出名按 lucide-react 1.x 实际——`TriangleAlert`/`AlertTriangle` 执行期核实）；GitShowPanel 改 `<IconAlertTriangle size={13} />` 经 ERROR_BANNER_FG/语义色 token 着色
- FE-26：落点优先 `ipc/window.ts` 内部（unlisten Promise 链尾 `.catch(() => {})`）；App.tsx 调用侧若无需改动则不动
- FE-27：仅删 `outline:"none"`；`:focus-visible` 全局环接管（鼠标点击不匹配，视觉无变化）

**验证**：verify/stage-06.md。要点：grep `"monospace"` 残留仅全局栈字符串内；GitShowPanel 无 `⚠`（emoji-scan 守卫）；icons.tsx 导出新图标；ExplorerPanel 容器无 outline 抑制。

**门禁**：同 Stage 01 五条

**commit**：`fix(misc): 加载页/错误边界字体栈统一 + GitShow 告警图标化 + 关窗 unlisten 兜底 + 焦点环接管`

## Stage 07 退役组件删除（FE-25）

**改动项**：FE-25（单一项——豁免「每 Stage 3-15 项」：跨多文件协同删除 + 测试迁移为强耦合单一任务，拆分会制造中间态编译断裂）

**agent 分工**（并行 2，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| retire-components | FE-25 主体 | src/features/agentHistory/HistorySessionList.tsx（删）、HistorySessionRow.tsx（删）、index.ts、src/__tests__/agent-history-row.test.tsx（迁移/删）+ 孤儿 helper（执行期 grep 确认零引用后删） |
| comment-sync | FE-25 注释 | src/lib/panelId.ts、src/features/navTree/NavTree.tsx、useNavTree.ts、NavHistoryRow.tsx、NavContextMenu.tsx、src/features/agentHistory/historyContextMenu.ts |

**实现要点**：
- 迁移评估口径：HistorySessionRow 用例中仍有独立语义的（四态同源/交互回调/选中态中未被 nav-tree-history.test.tsx 覆盖者）改写为 NavHistoryRow 面向；已覆盖语义删除
- 孤儿 helper 判定：grep 全仓引用，零生产+零测试引用才删；有引用保留不动
- comment-sync 只动注释（「照 HistorySessionList」类迁移注记改写为「原 HistorySessionList（已删）」口径或就近重写），不动任何逻辑

**验证**：verify/stage-07.md。要点：两文件不存在；`grep -r "HistorySessionList\|HistorySessionRow" src/` 仅余历史注记式命中（注释含「已删」字样）；index.ts 无两导出；L2 全量绿。

**门禁**：同 Stage 01 五条

**commit**：`refactor(agentHistory): 删除退役 HistorySessionList/Row（生产零消费）+ 测试迁移 NavHistoryRow`

## Stage 08 测试质量强化（TE-01~06）

**改动项**：TE-01、TE-02、TE-03、TE-04、TE-05、TE-06

**Stage 特殊纪律（fix-loop args.constraints 传此值）**：「本 Stage 只改测试与 e2e 辅助代码（src/__tests__/、e2e-tests/），禁止改 src/ 生产代码」

**agent 分工**（并行 5，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| navtree-test | TE-01 | src/__tests__/nav-tree.test.tsx |
| navhist-test | TE-02 | src/__tests__/nav-tree-history.test.tsx |
| e2e-rename | TE-03 | e2e-tests/specUtils.ts、mockcli.e2e.ts、hooks.e2e.ts |
| misc-test | TE-04/06 | src/__tests__/sideBarState.test.ts、workspace-page-dockview.test.tsx |
| actbar-test | TE-05 | src/__tests__/activityBar.test.tsx |

**实现要点**：
- TE-01：查询词改仅命中项目名（如「项目Beta」——执行期读测试种子数据定），断言页面行仍渲染
- TE-03：更名 `waitForPanelTabIcon` → `waitForPanelTabStatus`（签名不变）；e2e-tests 不在根 tsconfig include——门禁补 `npm run e2e`（含 build:e2e，约 2-3 分钟，勿中止）
- TE-05：三处补 dropIndicator DOM 状态/事件调用次数/清理后样式实断言（执行期读现断言就近补强）

**验证**：verify/stage-08.md。要点：`grep waitForPanelTabIcon e2e-tests/` 零命中；L2 全量绿；e2e 9/9。

**门禁**：五条 + 6. `npm run e2e`

**commit**：`test(e2e+l2): waitForPanelTabStatus 更名 + 假守卫断言强化 + 测试数据口径对齐`

## Stage 09 规范修订与文档同步（SPEC-01/02 + DOC-01~11 + VER-01 注记）

**改动项**：SPEC-01、SPEC-02、DOC-01、DOC-02、DOC-03、DOC-04、DOC-05、DOC-06、DOC-07、DOC-08、DOC-09、DOC-10、DOC-11、VER-01（注记）

**Stage 特殊纪律（fix-loop args.constraints 传此值）**：「本 Stage 只改文档与代码注释（docs/、各 CLAUDE.md、.claude/test-inventory.md、linear.ts 文件头注释、App.css:9 注释），禁止改逻辑代码」

**agent 分工**（并行 5，文件零重叠）：

| label | 负责项 | 文件 |
|---|---|---|
| spec-revise | SPEC-01/02 + DOC-10 explorer 部分 | docs/ui-redesign/requirements.md、design.md、src/App.css（仅 :9 注释）、src/features/explorer/CLAUDE.md（行高 24 档登记 + FE-27 outline 注记） |
| claudemd-a | DOC-03/04/05/06 | src/stores/CLAUDE.md、src/features/shortcuts/CLAUDE.md、src/ipc/CLAUDE.md、src/panels/CLAUDE.md |
| claudemd-b | DOC-07/08 | src/theme/CLAUDE.md、src/theme/schemes/linear.ts（仅文件头注释）、src/workspace/CLAUDE.md |
| claudemd-c | DOC-09/10（explorer 部分归 spec-revise） | .claude/CLAUDE.md、src/features/agentHistory/CLAUDE.md、src/features/navTree/CLAUDE.md、src/lib/CLAUDE.md |
| inventory-sync | DOC-01/02/11 | .claude/test-inventory.md、e2e-tests/CLAUDE.md |

**实现要点**：
- SPEC-01 修订口径（写死）：UI-204 与 design.md 阶梯表「编辑器/终端 12.5–13px」改「12.5–13px 为设计基准；终端/编辑器**内容区默认 14px**（用户 Ctrl+Wheel 可调 8–32）为登记例外」；SPEC-02 口径：「树行 28」改「导航树行 28/会话行 30；**文件树（explorer）行 24px**——紧凑列表档」
- DOC-09 例外指向修正措辞：硬约束 #6 例外实登记于 theme/CLAUDE.md（fail-safe 三处 + 终端 adapter）、explorer/CLAUDE.md（六色盘）、navTree/CLAUDE.md（项目蓝）——根文件改汇总指向
- DOC-11 登记范围 = Stage 01~08 全部用例增删（各 Stage commit message 可追溯）；DOC-01/02 按 checklist 原文修
- VER-01 注记落点：workspace/CLAUDE.md 的 F8 段「panel.component 不存在」后补「`view.contentComponent` 为 dockview 公开类型成员（IDockviewPanelModel.contentComponent）」

**验证**：verify/stage-09.md。要点：三处失实文档改字（grep 新旧措辞）；requirements/design 例外登记存在；根 CLAUDE.md 例外指向不再指向 panels/CLAUDE.md；test-inventory 与本修复用例增删一致（抽查）。

**门禁**：五条（纯文档 Stage，门禁跑全量防注释改动意外破编译——linear.ts 注释变更触碰 ts 文件）

**commit**：`docs(spec): 字号 14px/文件树 24px 规范登记 + 文档失实修正 + 配色例外指向收敛`

## 人工验证点（无法自动化验证项，收尾实测兜底）

**本次修复新增**：
1. ConfirmDialog 焦点陷阱实机手感（FE-28：Tab 循环/Enter 确认/Esc 取消在真实 WebView2 中）
2. 右键菜单 hover state 化后无闪烁（FE-05/06，视觉对照）
3. GitShowPanel 大文件警告 lucide 图标视觉（FE-18）
4. 侧栏双开 splitRatio 保留交互（FE-19：调节→单开→再双开，比例保持）
5. 项目删除 confirmDialog 实机（FE-03：WebView2 下自绘弹窗替代原生阻塞弹窗）

**继承 review 未实机验证 6 项**（docs/ui-redesign-review/00-汇总.md 第六节）：
6. TB-07 标题栏拖拽/双击/三钮手感
7. NAV-11 导航树三级交互与旧 settings 启动
8. GL-04 dockview sash 拖拽热区
9. 产物对照 final-mockup.html 视觉验收
10. 断网字体回退（UI-202）
11. 中文 CJK 渲染（UI-203）
