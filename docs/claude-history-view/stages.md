# Claude Code 历史会话查询与恢复 — Stage 划分（stages）

> 配套：`checklist.md`（项定义真值源）、`execution-plan.md`（编排参数）、`workflows/verify/stage-NN.md`（断言真值源）
> 规则：Stage 串行执行、每 Stage commit；Stage 内并行 agent 文件零重叠（分工表即证明）；门禁命令见各 Stage「全量测试」

## 跨 Stage 契约（写死，各 agent 不各自推断）

### IPC 契约（Stage 01/02 后端 ↔ Stage 03 前端）

```
命令：claude_history_scan() → HistorySession[]
      claude_history_delete(sessionId) → ()              invoke 参数 camelCase：{ sessionId }
      claude_history_rename(sessionId, newTitle) → ()    invoke 参数 camelCase：{ sessionId, newTitle }

HistorySession（TS camelCase ↔ Rust snake_case）：
  sessionId: string                      session_id
  cwd: string | null                     cwd
  title: string | null                   title
  titleSource: "customTitle" | "aiTitle" | "summary" | "firstPrompt" | "none"
                                         title_source（serde camelCase 枚举）
  firstPrompt: string | null             first_prompt  （≤200 字符，后端截断）
  mtimeMs: number                        mtime_ms
  cwdExists: boolean                     cwd_exists    （cwd 为 null 时恒 false）

delete/rename 定位：后端遍历扫描根定位 <sessionId>.jsonl，前端不传任何路径（SEC-01）
扫描根：resolve_projects_root() —— env SLTERM_CLAUDE_PROJECTS_DIR 优先，缺省 ~/.claude/projects
```

### Stage 05 双 agent 组件契约（写死）

```ts
// HistorySessionRow（agent A 产出，agent B 消费）
interface HistorySessionRowProps {
  session: HistorySession;
  active: boolean;      // ⚡
  orphan: boolean;      // ✗（cwd≠null && !cwdExists）
  noCwd: boolean;       // cwd===null（恢复类禁用，不显示 ✗）
  selected: boolean;
  onSelect(id: string): void;
  onDoubleClick(session: HistorySession): void;
  onContextMenu(session: HistorySession, pos: { x: number; y: number }): void;
}
// InputDialog（agent A 产出，agent B 消费）
interface InputDialogProps {
  title: string;
  initialValue: string;
  onSubmit(value: string): void;
  onCancel(): void;
}
// historyContextMenu（agent B 产出）
getHistoryContextMenuItems(
  session: HistorySession,
  opts: { active: boolean; orphan: boolean; noCwd: boolean }
): { label: string; disabled?: boolean; action(): void }[]
// useClaudeHistory（Stage 04 agent A 产出，Stage 05 消费）
{ state: "idle"|"loading"|"ready"|"error"; sessions: HistorySession[];
  activeIds: Set<string>; rootPath: string | null;
  scan(): Promise<void>; removeLocal(id: string): void; updateLocalTitle(id: string, t: string): void }
// restoreSession（Stage 04 agent B 产出，Stage 05 消费）
restoreHistorySession(session: HistorySession, opts: { fork: boolean }): Promise<void>
```

### E2E 兼容红线（Stage 05 硬约束，实证消费方）

AgentStatusView 改造必须保留（`e2e-tests/test.e2e.ts` 用例 1/2a 断言原文）：
- 根容器 `data-e2e="agent-status-view"`（test.e2e.ts:1589）
- 活跃会话行 `data-e2e="agent-status-row"`（多处）
- 标题栏文本 "AGENT STATUS"（test.e2e.ts:1598）
- 空态文案「选择一个项目」「无运行中的 claude 会话」（test.e2e.ts:1603）

### 人工验证点（无法自动化验证，执行收尾实测）

1. 真实 `claude --resume <id>` 跨项目恢复成功进入会话（E2E 只断言注入与编排，fixture id 非真实会话）
2. pty.write 注入在 shell 慢启动下的可靠性（ConPTY stdin 缓冲回放）
3. ⚡ 实时性：本应用 spawn 的 claude 会话历史行标记出现/消除
4. 重命名 custom-title 后官方 picker / `claude --resume <name>` 可见新标题
5. 大文件（20MB+）与数百会话规模扫描耗时与加载态
6. 窄侧栏（250px）双行式行/分组/搜索框视觉

---

## Stage 01 后端：历史会话扫描命令

**项**：SEC-02、BE-01、BE-02、BE-03、BE-04、BE-05、BE-06、BE-09（8 项）

**分工表**（1 agent）：

| label | 负责项 | 文件（触碰全集） |
|-------|--------|-----------------|
| backend-scan | SEC-02、BE-01..06、BE-09 | `src-tauri/src/claude_history/mod.rs`（新）、`src-tauri/src/claude_history/scan.rs`（新）、`src-tauri/src/claude_history/jsonl.rs`（新）、`src-tauri/src/lib.rs`（加 `pub mod claude_history;` + 注册 `claude_history_scan`） |

**实现要点**：
- 照 hooks 模块先例：home 目录绕过 project_root 沙箱、阻塞 I/O 全 `spawn_blocking`、DTO serde camelCase。
- `jsonl.rs` 纯函数化（`parse_head` / `parse_tail_title`），头部上限 `HEAD_SCAN_LIMIT = 512 * 1024`，尾部复用 64KB 常量与「中途起始跳首行」策略（hooks/usage.rs 先例）。
- 标题回退链 custom-title > ai-title > summary > 首条 prompt（决策 22）；prompt 截断 200 字符。
- 排除：`agent-*.jsonl`、非 UUID 文件名主干；不展开子目录。
- 单文件失败 → 降级条目（title=null, titleSource="none"）；扫描根不存在 → 空数组。
- env 测试依赖 `--test-threads=1` 门禁（串行无污染），测试后恢复 env。
- tempdir + `dunce::canonicalize`（8.3 短名坑，git/CLAUDE.md 实证）。
- **禁区**：`compute_conpty_flags` 固定 0x7，禁改 ConPTY flags 及其守卫测试。

**验证项**（详见 `workflows/verify/stage-01.md`）：DTO 字段 serde camelCase 双向；回退链 5 态测试存在且通过；排除 3 类；env 覆盖；降级条目；lib.rs 注册；clippy/fmt 零警告。

**Commit**：`feat(claude-history): 后端历史会话扫描命令（轻量解析+标题回退链+env 覆盖）`

---

## Stage 02 后端：删除/重命名命令

**项**：SEC-01、BE-07、BE-08、BE-10（4 项）

**分工表**（1 agent）：

| label | 负责项 | 文件（触碰全集） |
|-------|--------|-----------------|
| backend-ops | SEC-01、BE-07/08、BE-10 | `src-tauri/src/claude_history/ops.rs`（新）、`src-tauri/src/claude_history/mod.rs`（加 ops 模块声明 + 导出）、`src-tauri/src/lib.rs`（注册 `claude_history_delete` / `claude_history_rename`） |

**实现要点**：
- `validate_session_id()`：UUID 形态正则（checklist SEC-01 原文），非法 → `AppError::Validation`。
- 定位 = 遍历 `resolve_projects_root()` 一级子目录找 `<id>.jsonl`（复用 Stage 01 的遍历逻辑，抽共享私有函数）；找不到 → `AppError::Validation`（「会话不存在」语义）。
- delete：`remove_file` + 同名 `<id>/` 目录存在则 `remove_dir_all`。
- rename：newTitle trim 非空、≤200；`OpenOptions.append` 写 `{"type":"custom-title","customTitle":...,"sessionId":...}`（serde_json 序列化）+ `\n`。
- 两命令均 `spawn_blocking`。
- **禁区**：同 Stage 01。

**验证项**（详见 `workflows/verify/stage-02.md`）：校验 4 类非法拒绝测试；delete 范围测试（jsonl+目录）；rename 追加行反序列化断言；lib.rs 三命令注册齐全；越界无写入。

**Commit**：`feat(claude-history): 后端删除/重命名命令（sessionId 校验+custom-title 追加写）`

---

## Stage 03 前端：DTO + IPC 封装

**项**：FE-01、FE-02、FE-03（3 项）

**分工表**（1 agent）：

| label | 负责项 | 文件（触碰全集） |
|-------|--------|-----------------|
| frontend-ipc | FE-01..03 | `src/types/claudeHistory.ts`（新）、`src/types/index.ts`（加 export）、`src/ipc/claudeHistory.ts`（新）、`src/ipc/index.ts`（barrel 登记）、`src/__tests__/ipc-claude-history-contract.test.ts`（新） |

**实现要点**：
- DTO 字段与「跨 Stage 契约」逐字一致（硬约束 #4）；先读 Stage 01 落盘的 Rust DTO 核实。
- 契约测试照 `src/__tests__/ipc-hooks-config-contract.test.ts` 模式（mockIPC 四维）。
- **禁区**：同 Stage 01。

**验证项**（详见 `workflows/verify/stage-03.md`）：三命令名/参数结构 camelCase；异常传播不吞；barrel 导出；tsc/eslint 通过。

**Commit**：`feat(claude-history): 前端 DTO + IPC 封装与契约测试`

---

## Stage 04 前端：数据层 + 恢复编排

**项**：FE-04、FE-05、FE-06（3 项，2 agents 并行）

**分工表**：

| label | 负责项 | 文件（触碰全集） |
|-------|--------|-----------------|
| history-data | FE-04、FE-05 + 测试 | `src/features/claudeHistory/historyModel.ts`（新）、`src/features/claudeHistory/useClaudeHistory.ts`（新）、`src/__tests__/claude-history-model.test.ts`（新）、`src/__tests__/claude-history-hook.test.tsx`（新） |
| history-restore | FE-06 + 测试 | `src/features/claudeHistory/restoreSession.ts`（新）、`src/__tests__/claude-history-restore.test.ts`（新） |

> 本 Stage **不建 `index.ts` barrel**（避免两 agent 冲突；barrel 归 Stage 05 agent B）。两 agent 文件零重叠。接口契约见「跨 Stage 契约」段（useClaudeHistory 返回形状、restoreHistorySession 签名——写死）。

**实现要点**：
- history-data：纯函数零 React 依赖（historyModel.ts）；hook 内 TerminalRegistry.subscribe 卸载清理；rootPath 推导照 `useCommitStatus`（`src/features/commit/useCommitStatus.ts`）；normalizePath/basename 复用 `src/lib/path.ts`。
- history-restore：四步编排原语——`useProjects.getState()`（addProject/addPage/createProjectId/createPageId 字段照 `src/features/sidebar/SidebarTree.tsx` 的 handleAddProject/handleNewPage 现值）、`switchToPageShared`/`getPageApi`（`src/workspace/pageApis.ts`）、`TerminalRegistry`（`src/panels/terminal/TerminalRegistry.ts`）、`pty.write`（`src/ipc/pty.ts`）；轮询 100ms×50 照 `openHooksConfigPanel`；addPanel 参数 `{ id, component: "terminal", title: "claude", params: { panelId: id, cwd }, renderer: "always" }`（照 PageDockviewHost 模式）；panelId = `terminal-{pageId}-{Date.now()}`（`src/lib/panelId.ts` parseTerminalPageId 兼容：≥3 段 + 首段 terminal + 末段全数字）。
- 测试 mock 边界：`claude-history-restore.test.ts` mock `../../stores/projects`、`../../workspace/pageApis`、`../../ipc/pty`、`../../panels/terminal/TerminalRegistry`、`../../ipc/notification`——**注意 mock 只守 JS 侧形状**，真实编排由 Stage 06 E2E 兜底。
- **禁区**：同 Stage 01。

**验证项**（详见 `workflows/verify/stage-04.md`）：纯函数全分支测试存在且通过；hook 状态机/局部刷新/订阅清理；restore 四步调用顺序断言（addProject→addPage→switch→addPanel→write）、write payload 内容（`\r` 结尾、fork 追加）、防重入、失败 toast、两接口与契约段逐字一致。

**Commit**：`feat(claude-history): 历史数据层（分组/搜索/⚡派生）+ 四步恢复编排`

---

## Stage 05 前端：历史区 UI + AgentStatusView 改造

**项**：FE-07、FE-08、FE-09、FE-11、FE-12（5 项，2 agents 并行）

**分工表**：

| label | 负责项 | 文件（触碰全集） |
|-------|--------|-----------------|
| history-widgets | FE-07（Row/InputDialog 部分）、FE-11、FE-12（本 agent 文件内）+ 测试 | `src/features/claudeHistory/HistorySessionRow.tsx`（新）、`src/features/claudeHistory/InputDialog.tsx`（新）、`src/__tests__/claude-history-row.test.tsx`（新）、`src/__tests__/claude-history-input-dialog.test.tsx`（新） |
| history-view | FE-07（Sections/List/contextMenu 部分）、FE-08、FE-09、FE-11、FE-12（本 agent 文件内）+ 测试 | `src/features/claudeHistory/ClaudeHistorySections.tsx`（新）、`src/features/claudeHistory/HistorySessionList.tsx`（新）、`src/features/claudeHistory/historyContextMenu.ts`（新）、`src/features/claudeHistory/index.ts`（新 barrel）、`src/features/agentStatus/AgentStatusView.tsx`（改）、`src/__tests__/claude-history-view.test.tsx`（新）、`src/__tests__/agent-status-view.test.tsx`（**既有，同步更新**） |

> 组件契约见「跨 Stage 契约」段（HistorySessionRowProps / InputDialogProps / getHistoryContextMenuItems——写死，两 agent 照此实现与消费）。

**实现要点**：
- **E2E 兼容红线**（见上段）：agent-status-view / agent-status-row / "AGENT STATUS" / 两条空态文案必须保留——history-view agent 的 AgentStatusView 改造逐条核对。
- 颜色一律 `theme/colors.ts` token；选中高亮 `EXPLORER_SELECTION_BG`；禁止硬编码色值（硬约束 #6）。
- 菜单策略照 `src/features/commit/commitContextMenu.ts` 模式：ContextMenu 纯渲染组件可复用其结构（position:fixed、zIndex:1000、mousedown 外点击关闭），菜单项由 `getHistoryContextMenuItems` 返回；`historyContextMenu.ts` 不 import 组件、组件不 import git/pty IPC（策略层经 `src/ipc/claudeHistory`、`src/ipc/clipboard`、`src/ipc/dialog`、`./restoreSession`）。
- 双击分派：孤儿/无 cwd → 无操作；⚡ → `dialog.ask`「该会话已在运行中」（okLabel「分支恢复」）→ 确认走 fork；普通 → `restoreHistorySession(session, {fork:false})`。
- 复制恢复命令：有 cwd → `cd '<cwd>' && claude --resume <id>`；无 cwd → `claude --resume <id>`；`writeText`（`src/ipc/clipboard.ts`）。
- 默认展开态：活跃展开、两历史区收起；历史区首次展开触发 `scan()`；整视图滚动容器。
- data-e2e 属性按 FE-12 清单。
- **禁区**：同 Stage 01。

**验证项**（详见 `workflows/verify/stage-05.md`）：E2E 红线四件 grep；三下拉框结构与默认态；展开触发 scan；搜索/空态文案；菜单矩阵（普通/孤儿/⚡/无 cwd × 4 操作）；配色无硬编码（grep `#[0-9a-fA-F]{3,8}` 于新文件仅命中注释/token 文件）；data-e2e 清单；组件契约逐字一致；agent-status-view.test.tsx 同步更新且通过。

**Commit**：`feat(claude-history): 历史区 UI（双行式/搜索/右键菜单）+ AgentStatusView 三下拉框`

---

## Stage 06 E2E：fixture 与关键路径用例

**项**：TE-01、TE-02、TE-03、TE-04（4 项，1 agent）

**分工表**：

| label | 负责项 | 文件（触碰全集） |
|-------|--------|-----------------|
| e2e-history | TE-01..04 | `e2e-tests/fixtures/claude-projects/**`（新）、`e2e-tests/run-wdio.cjs`（改：副本 + env 注入）、`e2e-tests/test.e2e.ts`（新 describe）、`.gitignore`（加 tmp 目录）、`.claude/test-inventory.md`（E2E 用例数） |

**实现要点**：
- **安全红线**：删除/重命名用例只动 `e2e-tests/.tmp-claude-projects/` 副本；脚本与用例中**禁止出现**指向 `~/.claude/projects` 真实目录的写操作（SEC-02）。
- run-wdio.cjs：每次运行重建副本（fixtures → .tmp-claude-projects，占位符替换真实临时路径）+ `process.env.SLTERM_CLAUDE_PROJECTS_DIR`；沿用既有 settings.json 备份机制（FIX-TE-04）不动。
- 恢复编排用例断言到「终端缓冲含注入命令 + 项目/页面编排正确」为止；不断言 claude 成功进入会话。
- ask 弹窗处理：先 Grep 既有 E2E 是否已有原生 dialog 处理先例；无则用「菜单 action 内 dialog.ask 被 mock 拦截」降级方案并在用例注释写明（execute 期决策点，verify 备注）。
- 新用例的 `data-e2e` 选择器与 Stage 05 落盘属性逐一对应（FE-12 清单）。
- 门禁补 `npm run e2e` 实跑（本 Stage 专属；耗时长属预期）。`e2e-tests/run-wdio.cjs`、`test.e2e.ts` 不在根 tsconfig include——构建级兜底 = `npm run build:e2e`（含 vite build）。
- **禁区**：同 Stage 01。

**验证项**（详见 `workflows/verify/stage-06.md`）：fixtures 覆盖 7 形态；run-wdio.cjs env+副本逻辑 grep；`.gitignore` 条目；新 describe 用例全过（wdio 实跑输出）；既有 26 用例不回归（尤其 agent-status 5 用例——E2E 红线实证兜底）；test-inventory E2E 计数与 describe 实数一致；全文无真实 `~/.claude/projects` 写路径。

**Commit**：`test(claude-history): E2E fixture 与关键路径用例`

---

## Stage 07 文档同步

**项**：DOC-01、DOC-02、DOC-03、DOC-04、DOC-05、DOC-06（6 项，1 agent）

**分工表**：

| label | 负责项 | 文件（触碰全集） |
|-------|--------|-----------------|
| docs-sync | DOC-01..06 | `docs/claude-history-view/README.md`（v1.1）、`src-tauri/src/claude_history/CLAUDE.md`（新）、`src/features/claudeHistory/CLAUDE.md`（新）、`src/ipc/CLAUDE.md`（映射表加行）、`src/features/sideViews/CLAUDE.md`（AgentStatusView 行）、`.claude/CLAUDE.md`（模块索引 2 行 + F7 行）、`.claude/test-inventory.md`（计数核对） |

**实现要点**：
- 文档描述须对照当前代码核实，不撒谎（逐段 Grep/Read 验证）。
- README v1.1：决策 22–26 追加 + 决策 10 标「已被决策 22 推翻」+ 4.2/4.4/4.3.2 三处修订 + 版本头。
- test-inventory：L1/L2/L4 用例数与实跑输出一致。
- **禁区**：同 Stage 01。

**验证项**（详见 `workflows/verify/stage-07.md`）：README 决策表 26 行 + v1.1 头；两新 CLAUDE.md 四段式齐全；根索引两行 + F7 行；ipc 映射表 claudeHistory 行；sideViews 描述更新；test-inventory 三类计数与实跑一致；文档内路径/命令名 grep 全部命中真实代码。

**Commit**：`docs(claude-history): 决策 22-26 回写 + 模块文档登记 + 用例清单同步`

---

## 偏离豁免记录

- Stage 04 为 3 项下限（FE-04/05/06），达「每 Stage 3–15 项」下限，不拆——数据层与恢复编排文件零重叠可并行，合并执行减少一次全量测试开销。
- Stage 02/03 文件少但跨边界契约（写操作安全校验 / IPC 契约）要求独立验证与独立 commit，不合并。
- 文档 Stage 含 `.claude/CLAUDE.md` 修改——属 gitAddPaths 白名单内精确文件，合规。
