# Commit 侧栏视图需求文档

> 参考 JetBrains IntelliJ IDEA 的 Commit 页面交互。本文档只定义需求（What & Why），不含开发计划（How）。
> 每个决策点记录**已排除的备选方案及理由**，避免后续返工时重新争论。

## 1. 背景与目标

活动栏现有两个按钮：📋 项目列表（`projects`）、📁 文件浏览器（`explorer`），均由侧栏视图系统（`src/features/sideViews/`）管理。现新增第三个按钮 **commit**，点击后在侧栏区显示 commit 视图：

- 展示当前活跃项目的 git 变更文件（Changes / Unversioned Files 两个列表）
- 双击文件在操作页面打开带 git 语义后缀的页签（普通编辑 / 只读 HEAD 版本 / 双栏 diff）

**本期范围：浏览 + 打开。**

### 已排除：提交操作（commit message 输入、文件勾选、Commit 按钮）

- **排除理由**：① 需求描述完全未涉及；② 提交可通过内置终端执行 `git commit`，与终端模拟器定位一致；③ 保持本期范围最小，后续可作为独立 feature 追加。

## 2. 术语

| 术语 | 定义 |
|------|------|
| Changes | git 已追踪文件中发生变动的文件（status ∈ added / modified / deleted / renamed / conflict） |
| Unversioned Files | git 未追踪的新文件（status = untracked） |
| staged | 已 `git add`（git2 的 `INDEX_*` 状态） |
| unstaged | 未 `git add`（git2 的 `WT_*` 状态） |
| HEAD 版本 | 文件在最近一次 commit（HEAD）中的内容 |
| 占位对齐 | diff 视图中用空白装饰行使两侧未改动行视觉平齐（IDEA 风格） |

### 已排除：区分 staged / unstaged 分组展示

- **结论**：Changes 为**单一列表，不区分 staged/unstaged**。
- **排除理由**：① 需求描述"git 已追踪的文件中发生变动的文件"未区分两者；② IDEA 默认也不分组；③ 后端 `status_to_str`（`src-tauri/src/git/mod.rs`）现状已将 `INDEX_MODIFIED` 与 `WT_MODIFIED` 合并映射为 `"modified"`，同一文件只出现一次，直接沿用零改动。
- **推论（须知晓）**：`git add` 过的新文件（INDEX_NEW）显示绿色、页签名 `(git add)`；staged 后又本地改动的新文件（INDEX_NEW + WT_MODIFIED）按现有映射优先级仍归 added（绿色）。

## 3. 现状盘点（实现时可复用的既有能力）

| 能力 | 位置 | 说明 |
|------|------|------|
| 文件名 git 状态色 token | `src/theme/colors.ts` `GIT_FILE_COLORS` | added `#629755` / modified `#6897BB` / untracked `#D1675A` / deleted `#6C6C6C` / renamed `#3A8484` / conflict `#D5756C` |
| 行内 diff gutter 色 token | `src/theme/colors.ts` `GIT_GUTTER_COLORS` | modified / added / deleted / whitespaceOnly |
| git 状态查询 | `git_status(repoPath)` → `GitStatusEntry[]{path,status}` | status ∈ modified/added/deleted/renamed/untracked/conflict/ignored；非 git 项目返回 error |
| 行级 diff | `git_diff(repoPath, filePath)` → `DiffHunk[]{oldStart,oldLines,newStart,newLines}` | HEAD ↔ 工作区 |
| diff gutter 扩展 | `src/panels/editor/gitGutter.ts` | DiffHunk → RangeSet 映射、gutter marker |
| 编辑器面板 | `src/panels/editor/` | 读盘、Ctrl+S 保存、脏状态、外部修改监听、大文件策略（>10MB 拒绝、>1MB 警告） |
| 侧栏视图注册 | `src/features/sideViews/sideViewDefs.ts` | 加一行 `sideViewRegistry.register(...)` 即完成按钮/开关/拖拽/槽位/持久化 |
| 面板注册 | `src/panelRegistry.ts` | 新增面板类型的标准入口 |
| fs-event 监听 | `src/ipc/notify.ts` `onFsEvent` | 后端 watcher 广播，前端 200ms debounce 惯例 |
| 页签标题管理 | `src/workspace/titleManager.ts` | 同名冲突相对路径化 |

### 后端能力缺口（需新增，仅列需求不含设计）

1. **读取 HEAD 版本文件内容**——`(git delete)` 只读页签与 `(git diff)` 左栏均依赖，当前无此命令。
2. **`git_status` 递归列出未跟踪文件**——需 `recurse_untracked_dirs(true)`（见 FR-4）。

## 4. 功能需求

### FR-1 活动栏按钮

- 图标：🔀；归属：上区（top zone）末尾，即 `top: ["projects", "explorer", "commit"]`
- 默认**不打开**（默认打开视图保持 `projects` 不变）
- 老用户持久化数据经 `reconcileZones`（R9：缺失的注册 id 追加到上区末尾）自动获得该按钮，无需迁移
- 按钮的开关、拖拽换区、排序由侧栏视图框架自动处理，无特殊逻辑

**已排除**：加入下区（bottom zone）——无明显收益，与现有两按钮归于一区更一致；用户可在运行时自行拖拽换区。

### FR-2 视图整体状态机

视图按以下优先级展示其一（自上而下）：

| # | 状态 | 展示内容 |
|---|------|---------|
| 1 | 无活跃项目/页面（`activePageId` 为 null） | 占位文案"选择一个项目以查看变更"（与 ExplorerPanel 无项目时"选择一个项目以浏览文件"风格一致） |
| 2 | `git_status` 加载中 | 占位文案"加载中…"（防止先闪现"无变更文件"再跳变为实际列表） |
| 3 | `git_status` 调用失败 | 占位文案"当前项目并非 git 项目" |
| 4 | 正常（含空仓库） | 标题栏 `COMMIT` + 两个可折叠列表框 |

- 标题栏 `COMMIT`：28px 高、大写、字距 1px，样式与 ExplorerPanel 的"文件浏览器"标题栏一致。
- **空 git 仓库**（已 `git init` 无 commit，HEAD 为 UnbornBranch）按 git 项目正常处理：此时只可能有 added/untracked 文件，不涉及 diff 视图的 HEAD 依赖。"是否 git 项目"判定 = `git_status` 成功（discover 到 `.git` 即算），不依赖 HEAD 存在。

**已排除**：
- **区分 `git_status` 失败原因**（非 git vs 权限/仓库损坏）——discover 失败占绝对多数，区分收益极低；`.git` 损坏时"并非 git 项目"文案也基本符合用户认知。
- **标题栏文案用中文"提交"**——用户全程使用 commit 一词，且与视图功能名一致。

### FR-3 Changes 列表框

- 标题：`Changes (N)`（N = 文件数）
- 内容：git 已追踪且发生变动的文件（status ∈ added / modified / deleted / renamed / conflict），不区分 staged/unstaged
- 展示形式：**平铺列表**。每行 = 着色文件名 + 灰色父目录相对路径后缀（如 `Button.tsx  src/components`）；按完整相对路径字母序排序
- 文件名颜色（复用 `GIT_FILE_COLORS`，与文件浏览器同源，"同一配置"原则的落实）：

| 状态 | 颜色 | 说明 |
|------|------|------|
| added | 绿 `#629755` | 已 `git add` 的新文件（含 INDEX_NEW + WT_MODIFIED） |
| modified | 蓝 `#6897BB` | 内容改动（INDEX_MODIFIED / WT_MODIFIED） |
| deleted | 灰 `#6C6C6C` | 已删除（INDEX_DELETED / WT_DELETED） |
| renamed | 青 `#3A8484` | 重命名 |
| conflict | 红 `#D5756C` | 合并冲突 |

- 空列表：标题仍显示（计数 0），展开时内容区显示占位文案"无变更文件"——不隐藏整个框，避免列表跳动。

**renamed / conflict 归类决策**：
- **结论**：两者归入 Changes，分别用 renamed 青 / conflict 红 token 展示；双击均按"改动文件"对待，打开 `(git diff)` 双栏页签。
- **已排除**：① 单独分组或隐藏——语义上两者都是"已追踪文件发生变动"，属于 Changes；② 自造新色——token 现成，与文件浏览器保持一致是需求原文要求。

### FR-4 Unversioned Files 列表框

- 标题：`Unversioned Files (N)`；空态规则同 FR-3
- 内容：untracked 文件，文件名红色 `#D1675A`
- 展示形式：平铺 + 路径排序，同 FR-3
- **粒度**：**逐文件列出**。后端 `git_status` 需加 `recurse_untracked_dirs(true)`，否则一个含 10 个新文件的目录只显示为单条目 `foo/`，双击无合理打开目标
  - 已知副作用：文件浏览器中新目录下每个文件都会标红（现状仅目录行着色、子文件不着色——属缺陷修正，预期接受）

**已排除**：
- **保持现状、目录条目 `foo/` 双击不响应**——列表出现无法交互的条目，且与 IDEA 行为不一致。
- **树形展示**（IDEA 的可选模式）——需求原文为"文件列表框"；平铺实现简单且路径后缀足以区分同名文件。

### FR-5 折叠交互

- 两个列表框**默认展开**
- 折叠状态**不持久化**：仅组件内 state，视图关闭/重建后重置为展开

**已排除**：折叠状态写入 `settings.json` 持久化——低频偏好，收益不抵 store 与 settings schema 的改动成本。

### FR-6 双击打开页签规则

双击列表中的文件，在**当前活跃操作页面**（`window.__dockviewApi` 指向的页面）打开页签：

| 文件状态 | 页签名 | 面板类型 | 可编辑性 | 内容来源 |
|---------|--------|---------|---------|---------|
| added | `文件名(git add)` | 复用现有 `editor` | 可编辑（与文件浏览器打开完全一致：编辑/Ctrl+S/脏状态/外部修改监听） | 磁盘 |
| untracked | `文件名(git not add)` | 复用现有 `editor` | 可编辑（同上） | 磁盘 |
| deleted | `文件名(git delete)` | **新增 `gitshow` 面板** | 只读 | HEAD 版本 |
| modified / renamed / conflict | `文件名(git diff)` | **新增 `diff` 面板** | 左只读 / 右可编辑 | 左 = HEAD，右 = 磁盘 |

**面板类型策略（已确认的备选分析）**：

- `(git add)` / `(git not add)` 复用 `editor`：磁盘文件存在，现有逻辑全部适用，仅页签标题不同——零改动。
- `(git delete)` 新增 `gitshow` 面板而非给 editor 加 `readOnly` + `contentOverride` 参数：deleted 文件磁盘上不存在，editor 的"读磁盘 + fs-event 监听 + 保存"逻辑全部不适用，硬塞参数会让 editor 复杂化。
- `(git diff)` 新增 `diff` 面板：双栏结构是全新 UI，无法用 editor 表达。
- **设计模式要求**：新增面板类型与视图须使用合适的设计模式（如策略模式 / 注册表模式，与项目现有 `FileViewerRegistry` / `SideViewRegistry` / `TabTitleRegistry` 一致），保证**高内聚、低耦合、易扩展**；新增面板走硬约束 #5 标准流程（`panels/<type>/` 目录 + `panelRegistry.ts` 注册 + `PANEL_TYPES` 追加）。

**去重策略**：

- 同文件 + 同类型页签 → 聚焦已有面板，不重复打开（语义同现有 `findExistingEditor`）
- 同文件 + 不同类型页签 → 允许共存（普通编辑器、`a.ts(git add)`、`a.ts(git diff)` 可同时存在，互不影响）

### FR-7 `(git diff)` 双栏页签

- **布局**：纵向均分两栏。左栏 = HEAD 版本内容（只读），右栏 = 本地工作区内容（可编辑，编辑/保存/外部修改监听逻辑与文件浏览器打开的编辑器一致）
- **行对齐：IDEA 风格占位对齐**——删除的行在右侧对应位置显示空白占位行，新增的行在左侧对应位置显示空白占位行；未改动行两侧视觉平齐
  - **已排除：仅滚动同步不做占位**——与参考对象 IDEA 行为不一致，且占位方案下 gutter 位置天然正确
- **gutter 标记：左右两侧均显示**——左侧标记删除/修改的旧行，右侧标记新增/修改的新行；复用现有 `gitGutter` 扩展与 `GIT_GUTTER_COLORS` token
  - **已排除：仅右侧显示**（需求原文）——对称展示才完整表达 diff 语义；同一扩展两个实例，成本几乎为零（用户确认后修订原文）
- **滚动**：垂直滚动同步（保持对齐行平齐），水平滚动各自独立（长行互不干扰）
  - **已排除：水平也同步**——IDEA 行为为水平独立
- **保存**：右侧 Ctrl+S 写回磁盘原文件（同现有 editor）；保存后重新 `git_diff`，刷新 gutter 标记与占位对齐（对齐关系可能变化）；**左栏不随保存刷新**（HEAD 未变）
- **左栏 HEAD 内容刷新时机**：仅当 fs-event 检测到 `.git` 变化（commit 类操作）时重取
- **右栏外部修改处理**：与现有 editor 一致——净状态自动重载，脏状态弹窗选择（重载丢弃 vs 保留本地）
- **renamed 文件**：HEAD 侧内容取自**旧路径**，由后端负责正确匹配（git2 pathspec 细节为实现问题）
- **conflict 文件**：按普通文本 diff 处理，不做三方合并视图

### FR-8 `(git delete)` 只读页签

- 展示 HEAD 版本内容，只读
- **边界**：staged 新增后又从磁盘删除的文件（INDEX_NEW + WT_DELETED）按 `status_to_str` 优先级归为 deleted（灰），但它从未 commit 过，HEAD 中不存在 → 页签显示占位文案"该文件在 HEAD 中不存在"（与文件读取失败的错误展示样式一致），不崩溃、不空白

### FR-9 列表刷新机制

- 打开视图时加载一次 `git_status`
- 之后由 **fs-event 驱动自动刷新**（复用 `onFsEvent` + 200ms debounce，与 ExplorerPanel 同模式）；`git add` 等操作会写 `.git/index`，项目根递归 watcher 可捕获
- **已排除**：
  - **手动刷新按钮**——fs-event 已覆盖主要场景，按钮是冗余 UI
  - **定时轮询**——浪费 IO，与项目现有事件驱动模式相悖

### FR-10 页签标题管理

- 后缀 `(git add)` / `(git not add)` / `(git delete)` / `(git diff)` **始终保留**
- 同名冲突重算（titleManager 现有机制）只作用于文件名部分：如 `a/index.ts` 与 `b/index.ts` 同时以 diff 打开 → 显示 `a/index.ts(git diff)`、`b/index.ts(git diff)`；git 页签与普通编辑器页签参与**同一套**冲突检测（`index.ts` 普通编辑器 + `index.ts(git diff)` 同名时同样触发相对路径化）
- **标题是打开时刻的状态快照**：文件状态后续变化（如 `git checkout --` 还原、`git rm --cached` unstage）**不更新标题、不自动关闭页签**；diff 内容随 fs-event 刷新（还原后两侧一致、gutter 与占位清空）
  - **已排除：标题追踪状态迁移 / 自动关闭失效页签**——标题与内容状态耦合会引入持续的同步复杂度；页签保留符合用户对"打开的文件不消失"的预期

### FR-11 大文件与二进制

- 大文件：`(git diff)` / `(git delete)` 页签沿用现有 editor 策略（>10MB 拒绝、>1MB 弹窗警告），**复用同一阈值常量**（`MAX_FILE_SIZE_BYTES` / `LARGE_FILE_WARN_BYTES`），不新造
- 二进制：本期不特殊处理——按文本读取，行为同文件浏览器双击二进制（乱码但可控）；图片预览属 `fileViewers` 扩展范畴，不在本期

## 5. 非功能需求

- **配色单点（硬约束 #6）**：文件名色、行内 diff 色一律引用 `theme/colors.ts` 现有 token，禁止新增硬编码色值
- **IPC 约束（硬约束 #1）**：前端不直接 `invoke`，新增后端能力（读取 HEAD 文件内容）须经 `src/ipc/git.ts` 封装
- **DTO 双边对应（硬约束 #4）**：新增 IPC 命令的 DTO 遵守 Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边
- **面板封闭（硬约束 #5）**：见 FR-6 面板类型策略
- **测试**：遵循项目四级测试金字塔（L1 Rust / L2 Vitest / L3 headless / L4 E2E），L1/L2 覆盖所有 PR

## 6. 明确不做（Out of Scope）

| 项 | 理由 |
|----|------|
| commit message 输入、文件勾选、Commit 按钮 | 见 §1 |
| staged / unstaged 分组 | 见 §2 |
| renamed / conflict 专属视图 | 按 modified 对待（FR-3 / FR-6） |
| 树形文件列表 | 见 FR-4 |
| 折叠状态 / 视图状态跨会话持久化 | 见 FR-5 |
| 手动刷新按钮 / 轮询 | 见 FR-9 |
| 标题追踪文件状态迁移、自动关闭失效页签 | 见 FR-10 |
| 二进制 / 图片预览 | 见 FR-11 |
| `git_status` 失败原因区分 | 见 FR-2 |

## 7. 边界场景汇总

| # | 场景 | 行为 |
|---|------|------|
| B1 | 无活跃项目/页面 | 显示"选择一个项目以查看变更" |
| B2 | 加载中 | 显示"加载中…" |
| B3 | 非 git 项目 / `git_status` 任意失败 | 显示"当前项目并非 git 项目" |
| B4 | 空仓库（无 commit） | 按 git 项目正常展示（仅 added/untracked） |
| B5 | staged 新增后又磁盘删除（INDEX_NEW + WT_DELETED） | 归 deleted（灰）；双击页签显示"该文件在 HEAD 中不存在" |
| B6 | staged 后又本地改动（INDEX_NEW + WT_MODIFIED） | 归 added（绿），打开 `(git add)` |
| B7 | 未跟踪目录 | 逐文件列出（后端 `recurse_untracked_dirs(true)`） |
| B8 | 已打开页签的文件被还原 / unstage | 页签保留、标题不变，内容随 fs-event 刷新 |
| B9 | 同文件重复双击同类型 | 聚焦已有面板 |
| B10 | 同文件打开不同类型页签 | 允许共存 |
| B11 | 同名文件冲突 | 相对路径化 + 后缀保留 |
| B12 | 大文件 >10MB / >1MB | 拒绝 / 警告（复用 editor 阈值） |
| B13 | 二进制文件 | 按文本读取，不特殊处理 |
| B14 | renamed 文件 diff | HEAD 侧取旧路径内容（后端负责） |
