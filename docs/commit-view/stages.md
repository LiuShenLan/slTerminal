# Commit 侧栏视图 Stage 划分

> 清单真值源：`docs/commit-view/checklist.md`。每 Stage 一个 commit，串行执行不跳阶段。
> Stage 内并行 agent **文件零重叠**（硬性）；Stage 间允许碰同一文件（串行 + 每 Stage commit）。
> 跨边界契约以 checklist「跨边界契约」表为准，已写死于各脚本头部注释。

## Stage 总览

| Stage | 名称 | 改动项 | 并行 agent | 门禁命令 | commit message |
|-------|------|--------|-----------|---------|----------------|
| 01 | 后端 git 能力 + IPC 契约 | CV-BE-01~04 | 1 | tsc + eslint + clippy + npm test + cargo test | `feat: git_file_at_head 命令 + git_status 递归未跟踪 + oldPath 字段` |
| 02 | 前端基础设施 | CV-FE-01/02 | 2 | tsc + eslint + npm test | `feat: titleManager 标题后缀支持 + commit 按钮默认归属` |
| 03 | gitshow 只读面板 | CV-FE-03~05 | 1 | tsc + eslint + npm test | `feat: gitshow 只读面板（HEAD 版本查看）` |
| 04 | diff 双栏面板 | CV-FE-06~10 | 2 | tsc + eslint + npm test | `feat: diff 双栏面板（占位对齐 + 双侧 gutter + 滚动同步）` |
| 05 | commit 侧栏视图 | CV-FE-11~15 | 1 | tsc + eslint + npm test | `feat: commit 侧栏视图（Changes/Unversioned 列表 + 双击分派）` |
| 06 | E2E 用例 | CV-TE-01/02 | 1 | npm test + npm run e2e | `test: commit 视图 E2E——列表渲染 + diff 页签打开 2 用例` |
| 07 | 文档同步 | CV-DOC-01~03 | 1 | tsc + eslint + npm test | `docs: commit 视图模块文档同步 + 用例数更新` |

---

## Stage 01：后端 git 能力 + IPC 契约

**改动项**：CV-BE-01、CV-BE-02、CV-BE-03、CV-BE-04

**agent 分工**（1 agent——同一契约链，前后端字段强耦合不宜拆）：

| label | 负责项 | 文件 |
|-------|--------|------|
| git-backend | CV-BE-01/02/03/04 | `src-tauri/src/git/mod.rs`、`src-tauri/src/lib.rs`、`src/types/git.ts`、`src/ipc/git.ts`、`src/__tests__/ipc-contract.test.ts` |

**实现要点**：
- `recurse_untracked_dirs(true)` 追加到现有 `StatusOptions` 链，不动其他选项
- rename 检测前提：`StatusOptions` 链追加 `.renames_head_to_index(true).renames_index_to_workdir(true)`——git2 默认不做 rename 检测，不开则 renamed 状态位运行时永不置位、oldPath 恒 null
- oldPath：renamed 检测看 `INDEX_RENAMED | WT_RENAMED`；旧路径从 `entry.head_to_index()` / `index_to_workdir()` delta 的 `old_file().path()` 取，workdir join + `\\`→`/`（与 path 同格式）。L1 renamed 测试用 `git mv` 真实构造（blob 完全相同必被识别），禁止直接构造状态位
- `git_file_at_head`：沙箱校验 → `get_or_open_repo(file_path)` → `spawn_blocking`：`head()` → `peel_to_tree()` → `tree.get_path(rel)` → blob → `String::from_utf8_lossy`；UnbornBranch 与 NotFound 统一 `AppError::Git(format!("文件在 HEAD 中不存在: {rel}"))`；rel = file_path strip `dunce::simplified(workdir)` + `\\`→`/`
- workdir/短名坑：照 `compute_diff_hunks` 的 `dunce::simplified` 模式；测试用 `init_temp_repo`（已 canonicalize）
- `lib.rs` 注册照 `git::git_status,` 行格式追加
- 前端 `gitFileAtHead` wrapper 照 `gitDiff` 同文件格式；ipc-contract 测试照现有四维模式
- 只做编译级检查（`cargo test --no-run`），真实测试由全量测试 agent 单点跑

**验证项**（详见 `docs/workflows/verify/stage-01.md`）：recurse 选项存在且测试新增；oldPath serde camelCase；命令注册 + 错误消息含"HEAD 中不存在"；wrapper 命令名/参数精确匹配；全部门禁绿。

**commit**：`feat: git_file_at_head 命令 + git_status 递归未跟踪 + oldPath 字段`

---

## Stage 02：前端基础设施

**改动项**：CV-FE-01、CV-FE-02

> **划分理由**（豁免「每 Stage 3-15 项」）：仅 2 项且两项不强耦合——但基础设施（titleManager suffix、DEFAULT_ZONES）必须先于 Stage 03/05 就绪；并入任何相邻 Stage 都会引入文件重叠（titleManager.ts 归 Stage 05 消费链、sideBarState.ts 与 Stage 03/04 面板文件无交集），故独立成 Stage。

**agent 分工**（2 agent 并行，文件零重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| title-suffix | CV-FE-01 | `src/workspace/titleManager.ts`、`src/__tests__/titleManager.test.ts` |
| sidebar-default | CV-FE-02 | `src/features/sideViews/sideBarState.ts`、`src/__tests__/sideBarState.test.ts`、`src/__tests__/sideBar.test.ts`、`src/__tests__/workspace-sideviews.test.tsx`（以 grep `DEFAULT_ZONES` 实际命中为准） |

**实现要点**：
- title-suffix：suffix 语义严格按契约表——`findExistingEditor(pageId, filePath, suffix?)` 不传时仅匹配无 suffix 条目（ExplorerPanel 现有调用点零改动、行为不变）；`getFileEditorTitle` 与 `recomputeTitles` 都在末尾拼 suffix；`handleSaveAs` 不动
- sidebar-default：先 grep `DEFAULT_ZONES` 全部消费方再动笔；默认值断言逐处更新为 `["projects", "explorer", "commit"]`

**验证项**（详见 `docs/workflows/verify/stage-02.md`）：suffix 标题/冲突重算/去重隔离测试存在且通过；DEFAULT_ZONES 含 commit 且全仓默认值断言无残留旧值。

**commit**：`feat: titleManager 标题后缀支持 + commit 按钮默认归属`

---

## Stage 03：gitshow 只读面板

**改动项**：CV-FE-03、CV-FE-04、CV-FE-05

**agent 分工**（1 agent——panelRegistry.ts 单文件，不可并行）：

| label | 负责项 | 文件 |
|-------|--------|------|
| gitshow-panel | CV-FE-03/04/05 | 新建 `src/panels/gitshow/`（index.ts、GitShowPanel.tsx）、`src/panels/index.ts`、`src/panelRegistry.ts`、`src/panels/editor/useCodeMirror.ts`（仅加 export）、`src/__tests__/panel-registry.test.ts`、`src/__tests__/workspace-file-panel-types.test.ts`、新建 `src/__tests__/gitshow-panel.test.tsx` |

**实现要点**：
- 面板组件结构照 `EditorPanel.tsx` 模式（container ref → hook）；只读 = `EditorState.readOnly.of(true)` + `EditorView.editable.of(false)`
- 语言扩展 `getLanguageExtension` 从 `useCodeMirror.ts` import（已导出）；阈值常量 `MAX_FILE_SIZE_BYTES`/`LARGE_FILE_WARN_BYTES` 从 useCodeMirror 导出复用（需在该文件加 export，禁止另造数值）
- HEAD 取内容路径参数：`oldPath ?? filePath`；repoPath 来自 params
- 错误占位："该文件在 HEAD 中不存在"（三态 loading/content/error，照 HtmlPanel 三态模式）
- `PANEL_GIT_SHOW = "gitshow"`；`PANEL_TYPES` 追加；`FILE_PANEL_TYPES` 追加 gitshow 与 diff（先 grep 消费方确认）；`workspace-file-panel-types.test.ts` 的 size 断言 2→4 同步更新
- `isAlwaysRenderPanel` 决策：**改为显式白名单**（`terminal` + `htmlviewer`），gitshow/diff 不 always。理由：always 的动机是保 PTY 存活 / iframe browsing context，CM 重建无视觉闪屏；gitshow/diff 与 editor 同样承载大文件 CM，always 挂载与 editor 排除理由（大文件内存开销）相悖。行为对齐 editor；Stage 05 openCommitFile 经 `isAlwaysRenderPanel` 自然不 always，无需特判

**验证项**（详见 `docs/workflows/verify/stage-03.md`）：面板注册 + PANEL_TYPES 四类型（diff Stage 04 才注册）；只读配置存在；oldPath 优先调用；错误占位文案；阈值常量复用（无新数值字面量）。

**commit**：`feat: gitshow 只读面板（HEAD 版本查看）`

---

## Stage 04：diff 双栏面板

**改动项**：CV-FE-06、CV-FE-07、CV-FE-08、CV-FE-09、CV-FE-10

**agent 分工**（2 agent 并行，文件零重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| diff-panel | CV-FE-07/08/09/10（alignment+面板测试部分） | 新建 `src/panels/diff/`（index.ts、DiffPanel.tsx、alignment.ts、必要 hooks）、`src/panels/index.ts`、`src/panelRegistry.ts`、`src/__tests__/panel-registry.test.ts`、新建 `src/__tests__/diff-panel.test.tsx`、`src/__tests__/diff-alignment.test.ts` |
| gutter-head | CV-FE-06（含其测试） | `src/panels/editor/gitGutter.ts`、`src/__tests__/gitGutter.test.ts` |

**跨 agent 契约**（写死）：gutter-head 导出名 `buildHeadRangeSet(state: EditorState, hunks: DiffHunk[]): RangeSet<GutterMarker>`——diff-panel 按此签名 import 使用，不各自改名。

**实现要点**：
- gutter-head：HEAD 侧映射——modified hunk 的 old 行（oldStart..oldStart+min(old,new)-1）标 ModifiedMarker；纯删除（newLines=0）的 old 行标 DeletedMarker；old>new 的剩余 old 行标 DeletedMarker；纯新增无标记。复用现有 Marker 类，不新造色
- diff-panel：
  - 双栏 flex 50/50；左只读 CM（同 gitshow 配置）+ HEAD gutter + 占位 widget；右可编辑 CM（扩展集照 useCodeMirror 的基础子集：basicSetup/oneDark/高度 theme/字体/语言/diffGutter）+ workdir gutter + 占位 widget
  - `alignment.ts` 纯函数 `computeAlignment(hunks)` 按 checklist 规则；widget 用 `Decoration.widget({ widget: new PlaceholderWidget(lineHeight), side: ... })` block 级装饰，高度=行高、不可选中
  - 滚动同步：两 `.cm-scroller` 各挂 scroll 监听，`syncingRef` 防循环；只同步 scrollTop
  - 右侧保存：`usePanelFocus("editor", container, activate, deactivate)` + actions `{ save }`；save = writeFile → `gitDiff(repoPath, filePath)` → `updateDiffGutter` + 重算 alignment dispatch；左栏 .git fs-event 重取 HEAD
  - 右侧 fs-event 外部修改：净自动重载 / 脏 confirm（照 useCodeMirror 语义）；大文件阈值复用
  - `PANEL_DIFF = "diff"` 注册 + PANEL_TYPES 追加
  - renderer 策略照 Stage 03 决策：`isAlwaysRenderPanel` 已为显式白名单（terminal + htmlviewer），diff 不 always（行为对齐 editor），本 Stage 确认即可、不再改动该函数
- data-e2e 预留：`diff-panel` / `diff-left` / `diff-right`

**验证项**（详见 `docs/workflows/verify/stage-04.md`）：computeAlignment 全分支测试；buildHeadRangeSet 签名与映射测试；滚动同步防循环；保存后重 gitDiff 断言；注册五类型齐全；无双 side 硬编码颜色。

**人工验证点**（占位对齐与滚动同步的真实渲染行为 jsdom/E2E 无法自动验证，列入 Step 6 收尾实测）：
1. 占位行高与文本行高一致，未改动行双侧视觉平齐
2. 一侧垂直滚动时对侧 scrollTop 跟随；水平滚动各自独立
3. 真实负载（大文件、连续保存刷新）下渲染无撕裂错位

**commit**：`feat: diff 双栏面板（占位对齐 + 双侧 gutter + 滚动同步）`

---

## Stage 05：commit 侧栏视图

**改动项**：CV-FE-11、CV-FE-12、CV-FE-13、CV-FE-14、CV-FE-15

**agent 分工**（1 agent——sideViewDefs.ts + commit 目录同一功能链）：

| label | 负责项 | 文件 |
|-------|--------|------|
| commit-view | CV-FE-11/12/13/14/15 | 新建 `src/features/commit/`（index.ts、CommitView.tsx、CommitFileList.tsx、useCommitStatus.ts、openCommitFile.ts）、`src/features/sideViews/sideViewDefs.ts`、新建 `src/__tests__/commit-view.test.tsx`（可拆分） |

**实现要点**：
- 状态机优先级严格按 FR-2 表（无 rootPath > loading > error > 正常）；空仓库由 gitStatus 正常返回自然落到"正常"
- 列表框：标题 `Changes (N)` / `Unversioned Files (N)`；折叠箭头照 FileTree 三角样式；默认展开；空态"无变更文件"
- 行渲染：`GIT_FILE_COLORS[status] ?? EXPLORER_COLORS.fg`（照 FileTree 防御模式）；父目录相对路径灰色（`INPUT_BORDER` token）后缀；`relativePath`/`basename` 用 `src/lib/path`
- 分派映射表（openCommitFile.ts 顶部常量导出）：
  ```
  added → { panelType: "editor", suffix: "(git add)" }
  untracked → { panelType: "editor", suffix: "(git not add)" }
  deleted → { panelType: "gitshow", suffix: "(git delete)" }
  modified|renamed|conflict → { panelType: "diff", suffix: "(git diff)" }
  ```
- 分派流程照 ExplorerPanel.handleOpenFile：dedupe(findExistingEditor 带 suffix) → focus / addPanel + registerEditor(suffix) + recomputeTitles；renamed 传 `oldPath`；params 含 `repoPath`
- 视图 props：SideViewComponentProps（switchToPage/onDeletePage）用不到，箭头包装忽略（照 explorer 注册模式）
- 测试：mock `../ipc/git`、`../ipc/notify`；真实 stores 种子（照 explorer 测试模式，`helpers/workspace-setup.ts` 可复用）；`window.__dockviewApi` mock addPanel/getPanel

**验证项**（详见 `docs/workflows/verify/stage-05.md`）：状态机 4 态文案精确；映射表四类状态齐全且与契约一致；data-e2e 属性存在；颜色引用 token 无硬编码；sideViewDefs 含 commit 注册。

**commit**：`feat: commit 侧栏视图（Changes/Unversioned 列表 + 双击分派）`

---

## Stage 06：E2E 用例

**改动项**：CV-TE-01、CV-TE-02

**agent 分工**（1 agent）：

| label | 负责项 | 文件 |
|-------|--------|------|
| e2e-commit | CV-TE-01/02 | `e2e-tests/test.e2e.ts`、新建 `e2e-tests/gitScaffold.ts`（可选） |

**实现要点**：
- Node 侧 `child_process.execSync` git CLI：tempdir → `git init` → `git config user.email/name` → commit 基线 → 修改 tracked 文件 + 新建 untracked 文件（+可选 deleted）
- 用例 1：`__slterm_e2e_createProject(repoPath)` → `__slterm_e2e_toggleSideView("commit")` → 等待 `[data-e2e="commit-changes"]` → 断言列表项文本含修改文件名 + unversioned 含新文件名
- 用例 2：双击（`dblClick` 或 evaluate dispatch）modified 项 → 断言 `__dockviewApi` 面板 title 含 `(git diff)` 且页面存在 `[data-e2e="diff-left"]` 与 `[data-e2e="diff-right"]`
- 选择器一律 `data-e2e`；helpers 已有 `__slterm_e2e_toggleSideView`，无需新 helper（CV-TE-02 仅 Node 侧脚手架）
- **门禁**：`npm run e2e`（= build:e2e + wdio，VITE_E2E=1 必须）+ `npm test` 防前端回归；e2e-tests 不在根 tsconfig include，构建级门禁由 build:e2e 承担

**验证项**（详见 `docs/workflows/verify/stage-06.md`）：2 用例存在且 wdio 通过；脚手架隔离清理；无 CSS 内联样式选择器。

**commit**：`test: commit 视图 E2E——列表渲染 + diff 页签打开 2 用例`

---

## Stage 07：文档同步

**改动项**：CV-DOC-01、CV-DOC-02、CV-DOC-03

**agent 分工**（1 agent——文档一致性需单一视角）：

| label | 负责项 | 文件 |
|-------|--------|------|
| docs-sync | CV-DOC-01/02/03 | 新建 `src/features/commit/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/ipc/CLAUDE.md`、`src-tauri/src/git/CLAUDE.md`、`src/features/sideViews/CLAUDE.md`、`src/workspace/CLAUDE.md`、`.claude/CLAUDE.md`、`.claude/test-inventory.md` |

**实现要点**：
- 先 `git log --oneline -10` + `git diff HEAD~7 --stat` 读实际改动再动笔，禁凭记忆
- 各 CLAUDE.md 只更新所属模块段落，遵守渐进式披露（根文件不展开细节）
- test-inventory 用例数取数口径：L2 对照本 Stage 全量测试的 `npm test` 统计行；L1/L3/L4 对照源文件静态计数（L1: grep `#[test]`；L3/L4: grep `it(`），不依赖 cargo/wdio 实跑输出

**验证项**（详见 `docs/workflows/verify/stage-07.md`）：根 CLAUDE.md 模块表含 features/commit 行；panels CLAUDE.md 含 gitshow/diff；git CLAUDE.md 含 git_file_at_head；文档描述与代码一致（抽查断言）。

**commit**：`docs: commit 视图模块文档同步 + 用例数更新`

---

## Step 5 执行规则（主 agent 消费）

1. 逐 Stage 串行：`Workflow({ scriptPath: "docs/workflows/stage-NN-*.js" })` → 检查 `verifyResult.allFixed`
2. `false` → `Workflow({ scriptPath: "docs/workflows/fix-loop.js", args: { stage, failedItems, fixContext, verifyFile, constraints } })`，最多 3 轮；超过人工介入
3. no-return 分流：verify 阶段 → inline spawn 单 verify agent（复用脚本 prompt + verify 文件 + testResult）；重构/测试阶段 → `TaskStop` + `resumeFromRunId`
4. commit：`git add` 路径限定（`src/ src-tauri/ e2e-tests/ test/ .claude/CLAUDE.md .claude/test-inventory.md docs/`），commit 前 `git status` 甄别预期外改动；message 以本文档各 Stage 原文为准
5. 时间盒：后台运行每 15-20 分钟查 /workflows；单 Stage 超 60 分钟无进展 → `TaskStop` + 报告用户
6. 脚本调用前必须已语法预检（本次产出时已做，记录于 `docs/commit-view/execution-plan.md`）

## Step 6 收尾

1. 最终全量验证：`npx tsc --noEmit` + `npx eslint src/` + `cargo clippy -D warnings` + `npm test` + `cargo test --test-threads=1` + `npm run test:l3` + `npm run e2e`
2. **人工实测 Stage 04 标注的验证点**（占位行高=文本行高、未改动行双侧平齐、垂直滚动跟随/水平独立、真实负载无撕裂）——jsdom/E2E 无法覆盖，必须真实 app 实测
3. 抽查根 CLAUDE.md 与最终代码一致性
4. 归档 commit：docs/ 计划文档一并提交（若未随 Stage 提交）
5. 收尾报告：Stage commit 列表、最终用例数、未修复项（若有）
