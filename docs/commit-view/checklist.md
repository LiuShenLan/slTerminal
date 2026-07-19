# Commit 侧栏视图开发清单

> 需求真值源：`docs/commit-view-requirements.md`（FR-1~FR-11、B1~B14）。
> ID 编号：`CV-BE`（后端）/ `CV-FE`（前端）/ `CV-TE`（E2E）/ `CV-DOC`（文档）。
> 每项含位置、实现要点、测试要求。Stage 划分按 ID 引用，见 `docs/commit-view/stages.md`。
> 优先级组织：本清单为 feature 开发，实施优先级由 Stage 依赖顺序表达（BE → FE 基础 → 面板 → 视图 → E2E → 文档），不逐项标 P 级。

## 跨边界契约（写死，所有 agent 不各自推断）

| 契约 | 值 |
|------|-----|
| 新 IPC 命令 | Rust `git_file_at_head(repo_path: String, file_path: String) -> Result<String, AppError>`；TS wrapper `gitFileAtHead(repoPath: string, filePath: string): Promise<string>`（`src/ipc/git.ts`） |
| HEAD 不存在错误 | 后端 UnbornBranch / 文件不在 HEAD tree → `AppError::Git`，消息含 `"HEAD 中不存在"`；前端 **catch 任意错误** → 占位文案"该文件在 HEAD 中不存在"（不解析错误内容） |
| GitStatusEntry | Rust `{ path, status, old_path: Option<String> }` → serde camelCase → TS `{ path: string; status: string; oldPath: string | null }`（非 renamed 恒 null） |
| 新面板类型 | `"gitshow"`、`"diff"`；params 均为 `{ panelId: string; filePath: string; oldPath?: string; repoPath: string }` |
| 侧栏视图注册 | `{ id: "commit", title: "Commit", icon: "🔀" }`（sideViewDefs.ts） |
| 页签后缀 | `(git add)` / `(git not add)` / `(git delete)` / `(git diff)`——直接拼接无空格：`a.ts(git add)` |
| 状态→后缀映射 | added→`(git add)`；untracked→`(git not add)`；deleted→`(git delete)`；modified/renamed/conflict→`(git diff)` |
| 去重语义 | `findExistingEditor(pageId, filePath, suffix?)`：`suffix` 传入时仅匹配同 suffix 条目；**不传时仅匹配无 suffix 条目**（保证 ExplorerPanel 普通编辑器与 git 页签互不误聚焦，B10） |

## 后端（CV-BE）

### CV-BE-01：git_status 递归列出未跟踪文件

- **位置**：`src-tauri/src/git/mod.rs` `git_status()`
- **要点**：`StatusOptions` 追加 `.recurse_untracked_dirs(true)`（FR-4）。未跟踪目录逐文件返回，不再是 `foo/` 单条目。已知副作用（FR-4）：explorer 新目录下子文件逐个标红（现状仅目录行着色）——agent 需 grep explorer git 着色相关测试断言并按新语义同步更新
- **测试**：L1 新增——含多文件的未跟踪目录 → 每个文件单独出现在 statuses（非目录条目）

### CV-BE-02：GitStatusEntry 新增 oldPath 字段

- **位置**：`src-tauri/src/git/mod.rs`（`GitStatusEntry` + status 收集循环）；`src/types/git.ts`
- **前提**：`git_status` 的 `StatusOptions` 链追加 `.renames_head_to_index(true).renames_index_to_workdir(true)`——git2 默认不做 rename 检测，不开则 `INDEX_RENAMED`/`WT_RENAMED` 状态位运行时永不置位（重命名表现为 deleted + untracked 两条独立条目），oldPath 恒 null，`status_to_str` 的 renamed 分支为死代码
- **要点**：renamed 条目（`INDEX_RENAMED` / `WT_RENAMED`）从对应 delta 的 `old_file()` 取旧路径，拼接为与 `path` 同格式的绝对路径（workdir join + `\\`→`/`）；非 renamed → `None`（序列化 `oldPath: null`）（FR-7 renamed、B14）
- **测试**：L1——renamed 场景用 `git mv` 真实构造（blob 完全相同必被 rename 检测识别），断言 oldPath 为旧绝对路径，**禁止直接构造 git2::Status 状态位**（直接构造测试绿但行为不存在）；非 renamed 为 null；serde camelCase（`"oldPath"` 存在）

### CV-BE-03：新命令 git_file_at_head

- **位置**：`src-tauri/src/git/mod.rs`；`src-tauri/src/lib.rs` `generate_handler!` 注册
- **要点**：① `validate_path_within_root` 校验 file_path；② 复用 `get_or_open_repo`（搜 file_path）；③ `spawn_blocking` 内：`repo.head()` → `peel_to_tree` → 按 workdir 相对路径取 blob → `String::from_utf8_lossy`；UnbornBranch / 路径不在 tree → `AppError::Git("...HEAD 中不存在...")`（契约消息）；④ workdir strip 用 `dunce::simplified`（8.3 短名坑，见 git/CLAUDE.md）
- **测试**：L1——有 commit 读取内容正确；无 commit（UnbornBranch）报错含"HEAD 中不存在"；文件不在 HEAD 报错；子目录文件相对路径正确

### CV-BE-04：IPC wrapper gitFileAtHead

- **位置**：`src/ipc/git.ts`；`src/__tests__/ipc-contract.test.ts`
- **要点**：`gitFileAtHead(repoPath, filePath)` → `invoke("git_file_at_head", { repoPath, filePath })`（硬约束 #1）
- **测试**：L2 ipc-contract 四维——命令名 / 参数结构 / 正常返回透传 / 异常传播

## 前端基础设施（CV-FE-01/02）

### CV-FE-01：titleManager 标题后缀支持

- **位置**：`src/workspace/titleManager.ts`；`src/__tests__/titleManager.test.ts`
- **要点**：① `EditorEntry` 增加 `suffix?: string`；② `registerEditor(pageId, panelId, filePath?, suffix?)`；③ `getFileEditorTitle(..., suffix?)` 返回标题拼接 suffix；④ `recomputeTitles` 重算后保留 suffix（FR-10、B11）；⑤ `findExistingEditor` 加可选 `suffix` 参数，语义见契约表（B9/B10）
- **测试**：L2——suffix 标题生成；冲突重算保留后缀（`a/index.ts(git diff)`）；findExistingEditor 带/不带 suffix 的匹配隔离

### CV-FE-02：commit 按钮默认归属

- **位置**：`src/features/sideViews/sideBarState.ts` `DEFAULT_ZONES`；受影响测试
- **要点**：`top: ["projects", "explorer", "commit"]`（FR-1；新用户默认值，老用户由 R9 reconcile 自动获得）。先 grep `DEFAULT_ZONES` 全部消费方/断言再动笔
- **测试**：L2 既有断言更新——sideBarState.test.ts / sideBar.test.ts / workspace-sideviews.test.tsx 等默认值断言

## gitshow 面板（CV-FE-03~05）

### CV-FE-03：GitShowPanel 实现

- **位置**：新建 `src/panels/gitshow/`（`index.ts`、`GitShowPanel.tsx`）；`src/panels/editor/useCodeMirror.ts`（仅给 `MAX_FILE_SIZE_BYTES` / `LARGE_FILE_WARN_BYTES` 加 export，不改逻辑）
- **要点**：① params `{ panelId, filePath, oldPath?, repoPath }`；② `gitFileAtHead(repoPath, oldPath ?? filePath)` 取内容；③ CM6 只读（`EditorState.readOnly.of(true)` + `EditorView.editable.of(false)`）+ `getLanguageExtension`（从 useCodeMirror 导出复用）+ oneDark + 字体主题（editorFontSize）；④ 任意错误 → 占位文案"该文件在 HEAD 中不存在"（B5）；⑤ 大文件：HEAD 内容 length 超阈值拒绝/警告——**复用 editor 阈值常量**（从 useCodeMirror 导出 `MAX_FILE_SIZE_BYTES` / `LARGE_FILE_WARN_BYTES`，禁止新造）（FR-11、B12）；⑥ 二进制按文本显示（B13）
- **测试**：随 CV-FE-05

### CV-FE-04：注册 gitshow 面板类型

- **位置**：`src/panels/index.ts`、`src/panelRegistry.ts`；`src/__tests__/panel-registry.test.ts`、`src/__tests__/workspace-file-panel-types.test.ts`
- **要点**：`PANEL_GIT_SHOW = "gitshow"`；`panelRegistry.gitshow`；`PANEL_TYPES` 追加；`FILE_PANEL_TYPES` 追加 gitshow 与 diff（参与标题冲突重算，FR-10）——先查 `FILE_PANEL_TYPES` 全部消费方确认语义
- **测试**：既有注册表测试更新（三面板→四面板断言；`workspace-file-panel-types.test.ts` 的 size 断言 2→4 + has 断言更新）

### CV-FE-05：gitshow L2 测试

- **位置**：`src/__tests__/gitshow-panel.test.tsx`
- **要点**：mock `../ipc/git` `gitFileAtHead`；三态（loading / 内容渲染 / 错误占位文案）；readOnly 断言；oldPath 优先于 filePath 调用

## diff 面板（CV-FE-06~10）

### CV-FE-06：HEAD 侧（左侧）gutter 映射

- **位置**：`src/panels/editor/gitGutter.ts`；`src/__tests__/gitGutter.test.ts`
- **要点**：新增 HEAD 侧 RangeSet 构建（参数化 `buildRangeSet` 或新增 `buildHeadRangeSet`）：DiffHunk → **old 行号**映射——modified hunk 的 old 行标 ModifiedMarker；纯删除的 old 行标 DeletedMarker；纯新增在左侧**无标记**（FR-7 左侧标记删除/修改旧行）。独立导出便于 diff 面板与测试使用
- **测试**：L2——old 行号映射正确性（单行修改/连续删除/混合 hunk/纯新增无标记）

### CV-FE-07：DiffPanel 实现

- **位置**：新建 `src/panels/diff/`（`index.ts`、`DiffPanel.tsx`、必要 hooks）
- **要点**：① params `{ panelId, filePath, oldPath?, repoPath }`；② 纵向均分两栏（flex 50/50）：左 = HEAD 内容只读 CM（同 gitshow 只读配置）+ HEAD gutter（CV-FE-06）+ 占位行；右 = 磁盘文件可编辑 CM + workdir gutter（复用 `diffGutter`）+ 占位行；③ 占位对齐：`computeAlignment`（CV-FE-08）结果 → `Decoration.widget` 空白行（行高一致、不可选中）（FR-7）；④ 垂直滚动同步（一侧 scroll → 另一侧 scrollTop 跟随，防循环标记），水平滚动独立；⑤ 右侧 Ctrl+S：`usePanelFocus("editor")` + `setActiveEditor({ save })`——save = `fs.writeFile` 写回 + 重新 `gitDiff` + 刷新 gutter 与占位对齐（FR-7）；⑥ 左侧刷新：`onFsEvent` 检测 `.git` 路径 → 重取 HEAD 内容；⑦ 右侧外部修改：净自动重载 / 脏弹窗（同 editor 语义）；⑧ 大文件阈值复用（B12）；⑨ `data-e2e="diff-panel"` / `data-e2e="diff-left"` / `data-e2e="diff-right"` 预留
- **测试**：随 CV-FE-10

### CV-FE-08：占位对齐纯函数

- **位置**：`src/panels/diff/alignment.ts`
- **要点**：`computeAlignment(hunks: DiffHunk[]): { left: Map<number, number>; right: Map<number, number> }`——key = 在该行**之后**插入占位，value = 占位行数。规则：纯新增（oldLines=0）→ 左侧插 newLines 个；纯删除（newLines=0）→ 右侧插 oldLines 个；modified 行数不等 → 少的一侧插差值。纯函数零 DOM（照 dropTarget 模式）
- **测试**：全分支单测（纯增/纯删/等行修改/多删少/多增少/多 hunk/空）

### CV-FE-09：注册 diff 面板类型

- **位置**：`src/panels/index.ts`、`src/panelRegistry.ts`；`src/__tests__/panel-registry.test.ts`
- **要点**：`PANEL_DIFF = "diff"`；注册 + `PANEL_TYPES` 追加（Stage 3 已动同文件，Stage 串行无冲突）
- **测试**：注册表断言更新

### CV-FE-10：diff 面板 L2 测试

- **位置**：`src/__tests__/diff-panel.test.tsx`、`src/__tests__/diff-alignment.test.ts`
- **要点**：alignment 纯函数全分支（CV-FE-08）；滚动同步逻辑（模拟 scroll 事件断言对侧 scrollTop）；保存后刷新链（mock `gitDiff` 断言重调 + gutter 更新）；HEAD gutter 映射测试归 CV-FE-06

## commit 视图（CV-FE-11~15）

### CV-FE-11：CommitView 组件

- **位置**：新建 `src/features/commit/`（`index.ts`、`CommitView.tsx`、`CommitFileList.tsx`）
- **要点**：① 状态机按优先级：无 rootPath → "选择一个项目以查看变更"；loading → "加载中…"；gitStatus 任意失败 → "当前项目并非 git 项目"；正常 → 列表（FR-2、B1/B2/B3/B4）；② 标题栏 `COMMIT`（28px 大写，样式同 ExplorerPanel 标题栏）；③ 两个可折叠列表框：`Changes (N)` / `Unversioned Files (N)`，默认展开，折叠 state 不持久化，空态"无变更文件"（FR-3/4/5）；④ 行渲染：文件名色 = `GIT_FILE_COLORS[status]`（硬约束 #6，禁硬编码）+ 灰色父目录相对路径后缀；按完整相对路径字母序（FR-3）；⑤ `data-e2e="commit-view"` / `data-e2e="commit-changes"` / `data-e2e="commit-unversioned"` / 列表项 `data-e2e="commit-file-item"`
- **测试**：随 CV-FE-15

### CV-FE-12：数据加载与刷新 hook

- **位置**：`src/features/commit/useCommitStatus.ts`
- **要点**：① rootPath 推导同 ExplorerPanel（activePageId → project）；② `gitStatus(rootPath)` 加载一次；③ `onFsEvent` + 200ms debounce 自动刷新（FR-9）；④ rootPath 变化重载（参考 useFileTree generation 取消模式，防旧项目残留）
- **测试**：随 CV-FE-15

### CV-FE-13：双击分派 openCommitFile

- **位置**：`src/features/commit/openCommitFile.ts`
- **要点**：① **状态→面板映射表独立导出**（策略模式，需求 §5 设计模式要求）：added→`editor`+`(git add)`；untracked→`editor`+`(git not add)`；deleted→`gitshow`+`(git delete)`；modified/renamed/conflict→`diff`+`(git diff)`；② 去重：`titleManager.findExistingEditor(pageId, filePath, suffix)` 命中 → `panel.focus()`（B9）；③ 未命中 → `window.__dockviewApi.addPanel`（title 经 `getFileEditorTitle(..., suffix)`，params 含 repoPath，renamed 传 oldPath）+ `registerEditor(..., suffix)` + `recomputeTitles`（照 ExplorerPanel.handleOpenFile 流程）；④ addPanel 抛错不注册 titleManager
- **测试**：随 CV-FE-15

### CV-FE-14：注册 commit 侧栏视图

- **位置**：`src/features/sideViews/sideViewDefs.ts`
- **要点**：追加 `sideViewRegistry.register({ id: "commit", title: "Commit", icon: "🔀", component: () => React.createElement(CommitView) })`（FR-1）
- **测试**：随 CV-FE-15（注册断言）

### CV-FE-15：commit 视图 L2 测试

- **位置**：`src/__tests__/commit-view.test.tsx`（可按需拆分）
- **要点**：mock `../ipc/git`（gitStatus）+ `../ipc/notify`（onFsEvent）；状态机 4 态；列表渲染（颜色 token / 计数 / 排序 / 空态）；折叠交互；双击分派 4 类状态（mock dockApi + 真实 titleManager）；去重聚焦（B9/B10）；fs-event 刷新（fake timers 200ms）；rootPath 切换清空重载

## E2E（CV-TE）

### CV-TE-01：commit 视图 E2E 用例

- **位置**：`e2e-tests/test.e2e.ts` 新 describe
- **要点**：2 条用例——① Node 侧 git CLI 搭建真实仓库（init + commit + 修改 tracked + 新建 untracked）→ `__slterm_e2e_createProject` → `__slterm_e2e_toggleSideView("commit")` → 断言 Changes/Unversioned 列表条目数与文件名；② 双击 modified 文件 → 断言 `__dockviewApi` 出现 title 含 `(git diff)` 的面板且存在 `data-e2e="diff-left"`/`diff-right`
- **门禁**：`npm run e2e`（含 VITE_E2E=1 构建）

### CV-TE-02：E2E git 仓库脚手架

- **位置**：`e2e-tests/test.e2e.ts` spec 内 util（或独立 `e2e-tests/gitScaffold.ts`）
- **要点**：Node `child_process` 封装：建 tempdir → `git init` → config user → commit 基线文件 → 产出 modified/untracked/deleted 场景；用例间目录隔离，after 清理

## 文档（CV-DOC）

### CV-DOC-01：新模块文档

- **位置**：新建 `src/features/commit/CLAUDE.md`；更新 `src/panels/CLAUDE.md`
- **要点**：commit 视图（职责/状态机/分派映射表/测试模式）；panels 文档加 gitshow/diff 决策（只读 CM、占位对齐、滚动同步、双侧 gutter）+ 文件表 + 测试模式

### CV-DOC-02：既有模块文档更新

- **位置**：`src/ipc/CLAUDE.md`、`src-tauri/src/git/CLAUDE.md`、`src/features/sideViews/CLAUDE.md`、`src/workspace/CLAUDE.md`
- **要点**：gitFileAtHead wrapper；git_file_at_head 命令 + oldPath + recurse_untracked_dirs + 新测试数；第三条视图注册；titleManager suffix 语义
- **纪律**：先读 `git log`/`git diff` 再动笔，禁凭记忆写文档

### CV-DOC-03：根文档更新

- **位置**：`.claude/CLAUDE.md`、`.claude/test-inventory.md`
- **要点**：模块表加 `src/features/commit` 行 + panels 行描述更新（5 面板）；test-inventory 各层级用例数同步
