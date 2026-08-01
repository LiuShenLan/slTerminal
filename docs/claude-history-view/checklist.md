# Claude Code 历史会话查询与恢复 — 开发清单（checklist）

> 版本：v1.0（2026-08-01）
> 真值源：`docs/claude-history-view/README.md` v1.0（21 轮决策 + 15 场景 + 验收清单）+ 决策 22–26（见下）
> 组织方式：按模块前缀编号（SEC/BE/FE/TE/DOC）；**优先级由 Stage 依赖顺序表达，不用 P0–P4**（见 stages.md）
> 本文只列做什么与依据，实现编排见 stages.md / execution-plan.md

## 决策 22–26（本计划期 grill 敲定，执行收尾回写 README v1.1）

| # | 决策点 | 结论 |
|---|--------|------|
| 22 | 重命名写入格式 | 写 `custom-title` 条目（对齐官方 /rename，本机真实数据证实 /rename 写 custom-title；推翻冻结决策 10 的 ai-title）。标题回退链改为：**custom-title > ai-title > summary > 首条 prompt** |
| 23 | E2E 数据隔离 | 后端扫描根支持 `SLTERM_CLAUDE_PROJECTS_DIR` env 覆盖；E2E 用 fixture 目录，**任何用例不得触碰用户真实 `~/.claude/projects/`** |
| 24 | 当前项目匹配 | session.cwd 与 rootPath 规范化（反斜杠统一为 `/`）+ 忽略大小写后**精确相等** |
| 25 | 恢复命令注入 | 前端 addPanel 建终端 → 轮询 TerminalRegistry 拿 sessionId → `pty.write("claude --resume <id>\r")`；**零后端改动**，OSC 133/hooks 全链路自然生效 |
| 26 | 时间口径 | 最后活动时间 = 文件 mtime（两区统一） |

## 跨边界契约（前后端写死，各 Stage agent 不得各自推断）

```
命令：claude_history_scan() → HistorySession[]
      claude_history_delete(sessionId) → ()              invoke 参数 camelCase：{ sessionId }
      claude_history_rename(sessionId, newTitle) → ()    invoke 参数 camelCase：{ sessionId, newTitle }

HistorySession（TS camelCase ↔ Rust snake_case）：
  sessionId: string                      session_id
  cwd: string | null                     cwd
  title: string | null                   title         （后端已按回退链解析；null → 前端显示 sessionId 前 8 位）
  titleSource: "customTitle" | "aiTitle" | "summary" | "firstPrompt" | "none"
                                         title_source（serde camelCase 枚举）
  firstPrompt: string | null             first_prompt  （≤200 字符，后端截断）
  mtimeMs: number                        mtime_ms
  cwdExists: boolean                     cwd_exists    （cwd 为 null 时恒 false）

delete/rename 定位：后端遍历扫描根定位 <sessionId>.jsonl，不接受前端任何路径入参（SEC-01）
```

## 行操作矩阵（规格 4.4 + 场景 1/2/3/4 + 决策 16/18/20）

| 操作 | 普通行 | 孤儿行 ✗ | 运行中行 ⚡ | 无 cwd 行 |
|------|--------|---------|------------|----------|
| 复制恢复命令 `cd '<dir>' && claude --resume <id>` | ✓ | ✓ | ✓ | ✓（无 cd 段，仅 `claude --resume <id>`） |
| 分支恢复（四步 + `--fork-session`） | ✓ | ✗ 禁用 | ✓ | ✗ 禁用 |
| 删除（ask 确认 → ipc → 局部刷新） | ✓ | ✓ | ✗ 禁用 | ✓ |
| 重命名（自绘 InputDialog → custom-title） | ✓ | ✓ | ✓ | ✓ |
| 双击 | 恢复四步 | 无操作 | ask「该会话已在运行中」→ 引导分支恢复 | 无操作 |

孤儿行 ✗ 判定：`cwd ≠ null && cwdExists = false`。无 cwd 行：归「(未知目录)」组，不显示 ✗，恢复类操作禁用（无目录无法编排）。

---

## SEC — 安全

### SEC-01 sessionId 校验 + 定位不信托前端
- **位置**：`src-tauri/src/claude_history/ops.rs`（新建）
- **要点**：`claude_history_delete` / `claude_history_rename` 入参 `session_id` 严格校验 UUID 形态（`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`），拒绝含 `..`、路径分隔符、空串的一切输入（`AppError::Validation`）。文件定位 = 遍历扫描根一级子目录找 `<session_id>.jsonl`，**前端不传任何路径**。两命令写用户 home 目录文件、绕过 project_root 沙箱（先例 hooks/config.rs user 层），入参即攻击面。
- **Stage**：02

### SEC-02 env 覆盖仅测试用途
- **位置**：`src-tauri/src/claude_history/scan.rs`（`resolve_projects_root()` 单点）
- **要点**：扫描根解析顺序：`SLTERM_CLAUDE_PROJECTS_DIR` env 非空 → 用之；否则 `dirs::home_dir()/.claude/projects`。读取时机 = 每次 scan 调用时（不缓存，E2E 进程继承 env 即可生效）。文档标注生产不设置（DOC-02）。
- **依据**：决策 23。
- **Stage**：01

---

## BE — 后端（新模块 `src-tauri/src/claude_history/`）

### BE-01 模块骨架 + DTO + lib.rs 注册
- **位置**：`src-tauri/src/claude_history/mod.rs`（新建）、`src-tauri/src/lib.rs`（注册）
- **要点**：`HistorySession` / `TitleSource`（serde camelCase，契约见上）；三命令签名照契约；`lib.rs` 加 `pub mod claude_history;` + `generate_handler!` 注册三命令（Stage 01 先注册 scan，Stage 02 追加 delete/rename）。零新依赖（serde_json/tempfile/dirs/dunce 均已在 Cargo.toml）。
- **Stage**：01

### BE-02 `claude_history_scan` 命令
- **位置**：`src-tauri/src/claude_history/scan.rs`
- **要点**：遍历扫描根一级子目录 → 每目录顶层 `*.jsonl`；排除：`agent-*.jsonl` 平铺形态、文件名主干非 UUID 者（`subagents/` 等子目录不展开，天然不命中）；逐文件轻量解析（BE-03/04/05）。阻塞 I/O 全在 `spawn_blocking`（硬约束 #3）。**单文件解析任何失败 → 降级条目**（仅 sessionId + mtimeMs + cwd=null + title=null + titleSource="none" + firstPrompt=null + cwdExists=false），不阻塞整体、不返回 Err。扫描根本身不存在 → 返回空数组（非 Err，新机无 claude 数据属正常）。
- **依据**：规格 3.4/3.5、4.5、场景 5/6。
- **Stage**：01

### BE-03 头部解析纯函数
- **位置**：`src-tauri/src/claude_history/jsonl.rs`
- **要点**：`parse_head(reader) -> HeadInfo`：顺序逐行扫描，上限 512KB（`HEAD_SCAN_LIMIT`），沿途收集：cwd（首个含 `cwd` 字段行）、custom-title、ai-title、summary；命中**首条可见 user prompt** 即提前结束。可见 prompt 规则（本机真实数据验证）：`type=="user"` 且 `isMeta != true`；`message.content` 为**字符串**（数组 = tool_result 载体，跳过）；字符串**不以 `<` 开头**（跳过 `<command-name>`/`<local-command-caveat>`/`<local-command-stdout>` 等）；trim 后非空。未知 type 忽略；单行 JSON 解析失败（EOF 截断/损坏）跳过该行不中止；prompt 截断至 200 字符。
- **依据**：规格 3.2/3.3 容错规则 1–5、4.2 行内容。
- **Stage**：01

### BE-04 尾部 64KB last-wins 标题
- **位置**：`src-tauri/src/claude_history/jsonl.rs`
- **要点**：`parse_tail_title(path) -> Option<(String, TitleSource)>`：读文件尾部 ≤64KB（复用 hooks/usage.rs `TRANSCRIPT_TAIL_BYTES` 同款常量与「中途起始跳首行」策略），逆序找最后一条 `custom-title`（优先）或 `ai-title`。合成标题回退链：**custom-title > ai-title > summary > 首条 prompt**（决策 22）；四路皆空 → title=null / titleSource="none"。
- **依据**：规格 3.3 规则 5、4.2 标题三路回退（v1.0）+ 决策 22 修订。
- **Stage**：01

### BE-05 mtime + cwdExists
- **位置**：`src-tauri/src/claude_history/scan.rs`
- **要点**：`mtime_ms` = `std::fs::metadata().modified()` 转 Unix 毫秒（决策 26）；metadata 失败 → 0。`cwd_exists` = cwd 非 null 且 `Path::new(cwd).is_dir()`；cwd 为 null → false。
- **Stage**：01

### BE-06 env 覆盖扫描根
- **位置**：`src-tauri/src/claude_history/scan.rs`（`resolve_projects_root()`）
- **要点**：见 SEC-02（同函数落地，SEC-02 是约束面、本条是实现面）。
- **Stage**：01

### BE-07 `claude_history_delete` 命令
- **位置**：`src-tauri/src/claude_history/ops.rs`
- **要点**：SEC-01 校验 → 遍历定位 → `remove_file(<id>.jsonl)` + 同名 `<id>/` 目录存在则 `remove_dir_all`（含 subagents 子目录，规格 4.4 删除范围）；jsonl 不存在 → `AppError::Validation`（含「会话不存在」语义）。`spawn_blocking`。ask 确认在前端（FE-07）。
- **依据**：规格 4.4 删除、场景 1/3。
- **Stage**：02

### BE-08 `claude_history_rename` 命令
- **位置**：`src-tauri/src/claude_history/ops.rs`
- **要点**：SEC-01 校验 + `new_title` trim 后非空、≤200 字符（`AppError::Validation`）→ 定位 → `OpenOptions.append` 追加一行 `{"type":"custom-title","customTitle":<名>,"sessionId":<id>}`（serde_json 序列化保证转义正确；行尾 `\n`）。不做原子重写（追加写与运行中会话写入无冲突，决策 20/22）。`spawn_blocking`。
- **依据**：规格 4.4 重命名、场景 4、决策 22。
- **Stage**：02

### BE-09 L1 测试：扫描/解析
- **位置**：`jsonl.rs` / `scan.rs` 的 `#[cfg(test)] mod tests`
- **要点**：标题回退链 5 态（custom-title 赢 ai-title、ai-title 赢 summary、summary 赢 prompt、仅 prompt、全空 none）；prompt 跳过 4 类（isMeta / 数组 content / `<` 开头 / 空白）；EOF 截断行；未知 type；无 cwd；大文件（构造 >512KB 头部 + 尾部 custom-title，验证头限+尾扫协同）；扫描排除 3 类（agent-*.jsonl / 非 UUID 文件名 / subagents 子目录）；env 覆盖（`std::env::set_var` 前后调用，测毕恢复）；损坏文件 → 降级条目；扫描根不存在 → 空数组。tempdir 隔离 + `dunce::canonicalize`（8.3 短名坑，CI 实证）。**env 测试必须与并行测试隔离——`--test-threads=1` 下安全，注明依赖该门禁**。
- **Stage**：01

### BE-10 L1 测试：写操作
- **位置**：`ops.rs` 的 `#[cfg(test)] mod tests`
- **要点**：delete 范围（jsonl + 同名目录含内容物被删；无同名目录仅删文件）；delete 不存在 → Err；rename 追加行内容与格式（读回末行反序列化断言 type/customTitle/sessionId）；rename newTitle 空/空白/>200 拒绝；sessionId 非法 4 类拒绝（含 `..`、含 `/`、含 `\`、非 UUID）；两命令均不产生越界文件（断言扫描根外无写入）。
- **Stage**：02

---

## FE — 前端

### FE-01 `src/types/claudeHistory.ts`
- **位置**：新建 + `src/types/index.ts`（追加 export）
- **要点**：`HistorySession` 接口 + `TitleSource` 联合类型，字段与契约一一对应（硬约束 #4）。
- **Stage**：03

### FE-02 `src/ipc/claudeHistory.ts`
- **位置**：新建 + `src/ipc/index.ts`（barrel 登记）
- **要点**：`scanHistory(): Promise<HistorySession[]>` / `deleteHistorySession(sessionId: string): Promise<void>` / `renameHistorySession(sessionId: string, newTitle: string): Promise<void>`；invoke 参数 camelCase（Tauri 自动转 snake_case）。invoke 单点约束（硬约束 #1）。
- **Stage**：03

### FE-03 IPC 契约测试
- **位置**：`src/__tests__/ipc-claude-history-contract.test.ts`（新建）
- **要点**：三命令 × 四维验证（命令名 / 参数结构 / 正常返回 / 异常传播），照 `ipc-hooks-config-contract.test.ts` 模式（mockIPC）。
- **Stage**：03

### FE-04 `useClaudeHistory.ts` 数据 hook
- **位置**：`src/features/claudeHistory/useClaudeHistory.ts`（新建）
- **要点**：状态机 `idle | loading | ready | error`；`scan()` 由展开下拉框与手动刷新按钮触发（规格 4.3.5）；`removeLocal(sessionId)` / `updateLocalTitle(sessionId, title)` 局部即时刷新不重扫（规格 4.3.5）；rootPath 推导（activePageId → project，照 `useCommitStatus` 模式）；`TerminalRegistry.subscribe` → `deriveActiveSessionIds()` 实时更新 ⚡ 集合（不重扫，规格 4.5）；卸载清理订阅；rootPath 变化不自动重扫（历史区数据与项目弱相关，仅影响「当前项目」过滤——重算过滤即可）。
- **Stage**：04（agent A）

### FE-05 `historyModel.ts` 纯函数
- **位置**：`src/features/claudeHistory/historyModel.ts`（新建）
- **要点**：
  - `isCurrentProject(session, rootPath)`：`normalizePath`（`src/lib/path.ts`）+ `toLowerCase()` 后精确相等（决策 24）；rootPath null / session.cwd null → false。
  - `groupByCwd(sessions)`：按规范化 cwd 分组；无 cwd 归「(未知目录)」组（key 用 `null`，展示文案由 UI 层负责）；组内 mtimeMs 降序；组间按组内最大 mtimeMs 降序（规格 4.1 项目分组排序）；返回 `[{ cwd, sessions[] }]`。
  - `matchesSearch(session, query)`：标题 + firstPrompt，大小写不敏感 `includes`（规格 4.3.4）；query 空 → true。
  - `formatRelativeTime(mtimeMs, now)`：刚刚（<1min）/ N 分钟前（<60min）/ N 小时前（<24h）/ N 天前（<7d）/ 日期 `MM-DD`，跨年 `YYYY-MM-DD`（规格 4.2 时间粒度）；mtimeMs=0 → 「-」。
  - `deriveActiveSessionIds(entries)`：TerminalRegistry.getAll() 条目 → `claudeSession?.transcriptPath` basename 去 `.jsonl` → Set<string>；无 transcriptPath（matchedCommand-only）不标记（文档化局限，规格 4.1 两区关系）。
- **Stage**：04（agent A）

### FE-06 `restoreSession.ts` 四步恢复编排
- **位置**：`src/features/claudeHistory/restoreSession.ts`（新建）
- **要点**：`restoreHistorySession(session: HistorySession, opts: { fork: boolean }): Promise<void>`：
  1. **项目入列**：`useProjects.getState()` 查 rootPath 匹配（决策 24 同款规范化比较）；无则 `addProject({ projectId: createProjectId(), name: basename(cwd), rootPath: cwd, pages: [], activePageId: null, version: 1, ... })`（字段照 `SidebarTree.handleAddProject` 现值）
  2. **页面保障**：该项目 `pages` 为空则 `addPage`（name=`页面-${Date.now() % 10000}`、layout=`makeEmptyLayout()`，照 `SidebarTree.handleNewPage`）
  3. **页面切换**：`switchToPageShared(project.pages[0].pageId)`（pageApis.ts；setProjectRoot 前置语义不变）
  4. **终端恢复**：轮询 `getPageApi(pageId)`（100ms×50，照 `openHooksConfigPanel`）→ `addPanel({ id: "terminal-{pageId}-{Date.now()}", component: "terminal", title: "claude", params: { panelId: id, cwd }, renderer: "always" })` → 轮询 `TerminalRegistry.get(panelId)`（100ms×50）→ `pty.write(sessionId, panelId, new TextEncoder().encode("claude --resume <id>" + (fork ? " --fork-session" : "") + "\r"))`
  - **防重入**：模块级 `restoring` 标记，进行中再次调用立即返回（可 toast「恢复进行中」）。
  - **失败**：任何步骤异常 → `sendToastNotification("恢复会话失败", { body: ... })`，不中断其他流程（场景 10）。
  - **前置拦截**：调用方（菜单/双击）保证孤儿/无 cwd 不进入本函数。
- **依据**：规格 4.3.2、决策 6/25。
- **Stage**：04（agent B）

### FE-07 历史区 UI 组件
- **位置**：`src/features/claudeHistory/`（`ClaudeHistorySections.tsx` / `HistorySessionList.tsx` / `HistorySessionRow.tsx` / `historyContextMenu.ts` / `InputDialog.tsx`，均新建）
- **要点**：
  - `ClaudeHistorySections`：搜索框（位于两个历史下拉框**之上**，规格 4.3.4）+ 手动刷新按钮 + 「当前项目历史会话」「全部项目历史会话」两个下拉框容器；搜索过滤两区**当前展开**列表；无结果 → 空结果提示。
  - `HistorySessionList`：当前项目区平铺（`isCurrentProject` 过滤）；全部项目区二级折叠（`groupByCwd`，组标题 = basename + 悬停 title 完整路径，空组不显示，规格 4.1）。
  - `HistorySessionRow`：双行式（行1 = 粗体标题 + 右上相对时间灰字；行2 = prompt 预览单行截断省略，规格 4.2）；⚡/✗ 标记；单击选中高亮（`EXPLORER_SELECTION_BG`，FE-11）；双击按操作矩阵分派；右键弹菜单。
  - `historyContextMenu.ts`：`getHistoryContextMenuItems(session, opts)` 策略函数（照 `commitContextMenu.ts` 模式）——复制恢复命令（`writeText`，格式 `cd '<dir>' && claude --resume <id>`，无 cwd 仅命令）/ 分支恢复（`restoreHistorySession(session, {fork:true})`）/ 删除（`dialog.ask` 确认 → `deleteHistorySession` → `removeLocal`）/ 重命名（开 `InputDialog` → `renameHistorySession` → `updateLocalTitle`）；禁用态按操作矩阵。
  - `InputDialog.tsx`：自绘输入弹窗（Tauri 原生 dialog 无输入框，规格 4.4）；受控 input + 确认/取消；Enter 提交 / Esc 取消；空输入禁确认。
- **Stage**：05（A：Row + InputDialog；B：Sections + List + contextMenu）

### FE-08 `AgentStatusView.tsx` 三下拉框改造
- **位置**：`src/features/agentStatus/AgentStatusView.tsx`（改造）
- **要点**：视图内改为三个可展开/收起下拉框——「活跃会话」（现有逻辑**零改动**，`useAgentStatus` + `AgentStatusRow` 不动）+ 两个历史区（挂载 `ClaudeHistorySections`）；默认态：活跃展开、两历史区收起；历史区首次展开触发 `scan()`；整视图滚动。**E2E 兼容红线**：保留根容器 `data-e2e="agent-status-view"`、活跃行 `data-e2e="agent-status-row"`、"AGENT STATUS" 标题栏、空态文案「无运行中的 claude 会话」「选择一个项目」（test.e2e.ts 用例 1/2a 断言原文，e2e-tests/test.e2e.ts:1598/1603）。
- **依据**：规格 4.1 视图结构、决策 1。
- **Stage**：05（agent B）

### FE-09 空态与容错文案
- **位置**：`ClaudeHistorySections.tsx` / `HistorySessionList.tsx`
- **要点**：当前项目无历史 →「该项目暂无历史会话」；全部无 →「暂无历史会话」；无活跃项目 →「无活跃项目」（场景 7）；搜索无结果 → 提示；损坏条目显示 sessionId 前 8 位 + 时间（后端降级条目天然满足，场景 5）。
- **Stage**：05（agent B）

### FE-10 L2 测试
- **位置**：`src/__tests__/claude-history-*.test.ts(x)`（新建）
- **要点**：
  - `claude-history-model.test.ts`：FE-05 全分支（匹配大小写/斜杠、分组排序与未知组、搜索大小写、时间粒度全档含跨年、mtime=0、⚡ 派生含无 transcriptPath 不标）。
  - `claude-history-hook.test.tsx`：状态机流转、scan 成功/失败、removeLocal/updateLocalTitle 不触发 scan、subscribe 驱动 ⚡ 更新、卸载清理。
  - `claude-history-restore.test.ts`：四步编排（mock `stores/projects`、`workspace/pageApis`、`ipc/pty`、`panels/terminal/TerminalRegistry`）——已开项目跳过入列、无页建页、切页、addPanel 参数（cwd/id 格式）、pty.write 内容（普通/fork）、防重入、失败 toast、孤儿/无 cwd 前置拦截不调用。
  - `claude-history-row.test.tsx`：双行渲染、⚡/✗、单击选中、双击三分派、右键回调。
  - `claude-history-input-dialog.test.tsx`：受控输入/Enter/Esc/空禁确认。
  - `claude-history-view.test.tsx`：三区结构、默认展开态、展开触发 scan、搜索过滤、空态文案、菜单可用性矩阵（普通/孤儿/⚡/无 cwd × 4 操作）。
  - `agent-status-view.test.tsx`（**既有文件同步更新**，agent B 负责）：适配三下拉框结构，保留活跃区断言。
- **Stage**：04（model/hook/restore，归 A/B）+ 05（row/dialog/view/agent-status-view，归 A/B）

### FE-11 配色单点
- **位置**：全部新 UI 文件
- **要点**：颜色一律 `theme/colors.ts` token（硬约束 #6）；选中高亮复用 `EXPLORER_SELECTION_BG`（照 explorer 选中模型）；时间/二级文本用既有灰阶 token。
- **Stage**：05

### FE-12 data-e2e 属性
- **位置**：全部新 UI 文件
- **要点**：`agent-history-search` / `agent-history-refresh` / `agent-history-section-current` / `agent-history-section-all` / `agent-history-group` / `agent-history-row` / `agent-history-menu` / `agent-history-input-dialog`（L4 选择器约定，禁 CSS 内联样式选择器）。
- **Stage**：05

---

## TE — E2E（L4）

### TE-01 fixture 目录
- **位置**：`e2e-tests/fixtures/claude-projects/`（新建，入 git）
- **要点**：合成 transcript 集——≥2 个编码目录（其一 cwd 指向 E2E 运行时临时项目目录的 fixture 用**占位符**，Node 侧复制时替换为真实路径）；覆盖：custom-title / ai-title / 无标题（回退首条 prompt）/ 无 cwd 行 / 孤儿 cwd（不存在的路径）/ `agent-*.jsonl`（应排除）/ `<id>/subagents/agent-*.jsonl`（应排除）。行内容最小化（几行 JSONL 即可）。
- **Stage**：06

### TE-02 env 注入 + 工作副本
- **位置**：`e2e-tests/run-wdio.cjs`（改）、`.gitignore`（加 `e2e-tests/.tmp-claude-projects/`）
- **要点**：run-wdio.cjs 启动 wdio 前：Node 侧复制 `fixtures/claude-projects/` → `e2e-tests/.tmp-claude-projects/`（每次运行重建，防用例间污染；占位符替换为真实临时路径）→ `process.env.SLTERM_CLAUDE_PROJECTS_DIR = <副本绝对路径>`（子进程继承，后端 scan 时读取生效）。**安全红线**：副本机制保证删除/重命名用例只动 tmp，不触碰用户真实 `~/.claude/projects/`（SEC-02）。
- **Stage**：06

### TE-03 E2E 用例
- **位置**：`e2e-tests/test.e2e.ts`（新 describe「Claude 历史会话」）
- **要点**（用例以执行期脚本分工表为准，关键路径）：
  1. 展开「全部项目历史会话」→ 列表展示（加载态 → 行出现；agent-*.jsonl 不出现）
  2. 标题回退正确（custom-title 行显 custom 名；无标题行显首条 prompt）
  3. 搜索过滤（命中 / 无结果提示）
  4. 复制恢复命令 → 剪贴板内容断言（`__slterm_e2e_writeClipboard` 同族读取路径或后端读；以执行期可用 API 为准）
  5. 重命名：右键 → InputDialog 输入 → 列表标题更新 + **副本文件尾部出现 custom-title 行**（Node 侧读文件断言）
  6. 删除：右键 → ask 确认 → 列表行消失 + **副本文件消失**（Node 侧断言）
  7. 孤儿行 ✗ 标记 + 双击无反应
  8. 恢复编排：双击普通行 → 新项目入列（`__slterm_e2e_getActivePageInfo` rootPath = fixture cwd）→ 新终端页签 → 终端缓冲含 `claude --resume <id>`（`__e2e_getTerminalText`）；**不断言 claude 成功进入会话**（fixture id 非真实会话；真实成功属人工验证）
  - ask 弹窗处理照既有 E2E 模式（原生 dialog 由 WDIO 能力或 mock 拦截，执行期核实可用路径并写 verify 备注）。
- **Stage**：06

### TE-04 用例清单同步
- **位置**：`.claude/test-inventory.md`
- **要点**：E2E 用例数 + 新 describe 登记；L1/L2 新增用例数由 DOC-06 最终核对。
- **Stage**：06

---

## DOC — 文档（固定最后 Stage）

### DOC-01 README v1.1
- **位置**：`docs/claude-history-view/README.md`
- **要点**：决策记录追加 22–26（含推翻决策 10 的说明）；同步修订：4.2 标题回退链（加 custom-title 首优先级）、4.4 重命名行（写 custom-title）、4.3.2 步骤 4（pty.write 注入表述，与决策 25 一致）；版本头 v1.1 + 日期。
- **Stage**：07

### DOC-02 `src-tauri/src/claude_history/CLAUDE.md` 新建
- **要点**：职责 → 架构决策（轻量扫描三读点、env 覆盖[生产不设置]、sessionId 校验、custom-title 追加写）→ 文件表 → 测试模式；根 `.claude/CLAUDE.md` 模块索引登记（含详情链接）。
- **Stage**：07

### DOC-03 `src/features/claudeHistory/CLAUDE.md` 新建
- **要点**：职责 → 架构决策（三下拉框、数据流、恢复编排四步、⚡ 派生局限、操作矩阵）→ 文件表 → 测试模式；根模块索引登记；`src/ipc/CLAUDE.md` 模块映射表加 `claudeHistory.ts` 行。
- **Stage**：07

### DOC-04 需求编号登记
- **位置**：`.claude/CLAUDE.md` 需求编号索引
- **要点**：加 `F7 | 特性 | claude 历史会话查询与恢复（三下拉框 + 四步恢复编排）`。
- **Stage**：07

### DOC-05 `src/features/sideViews/CLAUDE.md`
- **要点**：文件表 `AgentStatusView.tsx` 行描述更新为三下拉框结构（活跃 + 两历史区）。
- **Stage**：07

### DOC-06 test-inventory 一致性
- **位置**：`.claude/test-inventory.md`
- **要点**：L1（claude_history 模块用例数）/ L2（claude-history-* 用例数）/ L4（TE-04 已更新则复核）最终一致；计数与实跑输出一致。
- **Stage**：07
