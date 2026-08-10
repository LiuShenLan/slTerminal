# 历史会话 — claude 定制优化盘点

只读现状盘点（2026-08-08）。覆盖后端 `src-tauri/src/claude_history/`、前端 `src/features/claudeHistory/`、IPC/DTO 层及两份模块文档。专属程度三档：**硬编码 claude**（显式 claude 字样或假设其行为）/ **通用机制但 claude 触发**（机制通用，当前仅被 claude 数据格式激活）/ **完全通用**（与 claude 无关）。

## 相关文件

后端（`src-tauri/src/claude_history/`）：
- `mod.rs` — HistorySession 七字段 DTO、TitleSource 五变体、`is_uuid_filename()`
- `scan.rs` — `resolve_projects_root()` 扫描根单点（SEC-02/BE-06）、`claude_history_scan` 命令、排除规则、降级条目
- `jsonl.rs` — 头部 512KB + 尾部 64KB 双窗口轻量解析（`parse_head`/`parse_tail_title`）、标题回退链（`resolve_title`）
- `ops.rs` — `validate_session_id()`（SEC-05）、`claude_history_delete`
- `CLAUDE.md` — 模块决策文档（SEC-02/05、BE-02~07、决策 22–28）

前端（`src/features/claudeHistory/`）：
- `restoreSession.ts` — 四步恢复编排（FE-06）：项目入列→页面保障→页面切换→终端注入 `claude --resume <id>`
- `useClaudeHistory.ts` — 数据 hook（FE-04）：状态机、generation 防竞、TerminalRegistry 订阅四态同源
- `historyModel.ts` — 纯函数模型（FE-05）：`isCurrentProject`/`groupByCwd`/`matchesSearch`/`formatRelativeTime`/`deriveActiveSessionStatuses`
- `historyContextMenu.ts` — 右键菜单策略（FE-07）：`buildResumeCommand` + 操作矩阵禁用态
- `HistorySessionList.tsx` — 列表（current 平铺 / all 分组折叠）、双击三分派、右键菜单调用方、`findPanelForSession` 反查
- `HistorySessionRow.tsx` — 双行式行：四态 emoji + ✗ 孤儿标记 + CLI 品牌 logo
- `SessionActionDialog.tsx` — 动作弹窗（问题 5，自绘模态）
- `ClaudeHistorySections.tsx` — 三下拉框组合件（受控组件，FE-07/FE-09）
- `CLAUDE.md` — 模块决策文档（问题 1–7 修复、四态同源、恢复编排、操作矩阵）

IPC/DTO：
- `src/ipc/claudeHistory.ts` — `scanHistory`/`deleteHistorySession` wrapper（`claude_history_scan`/`claude_history_delete` 两命令唯一 invoke 位置）
- `src/types/claudeHistory.ts` — `HistorySession`/`TitleSource` 双边 DTO

测试侧（L4 E2E）：
- `e2e-tests/history.e2e.ts` — 历史会话 spec（8 条 active：fixture 展示/标题回退链/搜索/复制恢复命令/孤儿/删除/四态/恢复编排）
- `e2e-tests/fixtures/claude-projects/` — 7 形态会话 fixture 副本（`SLTERM_CLAUDE_PROJECTS_DIR` 指向；编码目录名/UUID 与排除规则的同步关系见 fixture README，E2E-13③）
- `e2e-tests/run-wdio.cjs` — fixture 隔离注入点（E2E env 生效，任何用例不触碰用户真实 `~/.claude/projects/`）

跨模块关联：`src/features/agentStatus/AgentStatusView.tsx`（活跃区标题覆盖，问题 6 修复，宿主本领域组件）。

## 优化项清单

| # | 优化 | 位置(file:line) | 机制 | 触发点（claude 哪个行为） | 专属程度 |
|---|------|----------------|------|--------------------------|----------|
| 1 | 扫描根单点 `resolve_projects_root()`（SEC-02/BE-06） | scan.rs:21-26 | 扫描根解析：`SLTERM_CLAUDE_PROJECTS_DIR` env 非空优先，否则 `dirs::home_dir()/.claude/projects`；每次调用读 env 不缓存（E2E fixture 隔离） | claude 历史数据固定存于 `~/.claude/projects/`（外部假设，仓库内不可证——代码仅硬编码该路径） | 硬编码 claude |
| 2 | 存储布局假设：cwd 编码目录 + `<uuidv4>.jsonl` 会话文件（禁止反解码） | mod.rs:9-10；scan.rs:51-67；jsonl.rs:48 | 遍历扫描根一级子目录的直属文件；一级目录名 = cwd 的有损编码（仅作遍历容器，cwd 一律从 JSONL 内容解析） | claude 的 projects 目录布局：目录名编码 cwd、文件名主干即 sessionId | 硬编码 claude |
| 3 | 排除规则：`agent-*.jsonl` 平铺 / 非 UUID 主干 / subagents 子目录不递归 | scan.rs:72-80；scan.rs:56 | `is_session_jsonl`：扩展名 jsonl + 主干 UUID 形态（**其中 UUID 形态检查为通用部分，见 #4**）+ 非 `agent-` 前缀；只扫直属文件不递归子目录 | claude 子代理 transcript 以 `agent-*.jsonl` 平铺或 `<id>/subagents/` 子目录形态存在 | 硬编码 claude |
| 4 | `is_uuid_filename` UUID 形态校验 | mod.rs:59-74 | 纯函数：36 长度 + 连字符位置 + ascii hex 全检；scan 排除非会话文件与 ops 校验（SEC-05）复用 | claude 会话文件名主干 = UUID v4 | 通用机制但 claude 触发 |
| 5 | 头部 512KB + 尾部 64KB 双窗口轻量解析（BE-03/BE-04） | jsonl.rs:16-19（常量）；50-128（parse_head）；160-212（parse_tail_title） | 禁止整文件读取：头部顺序扫描（命中首条可见 prompt 提前结束，超 512KB 停止）+ 尾部 64KB 逆行扫描标题（中途起始跳首行，照 hooks/usage.rs `TRANSCRIPT_TAIL_BYTES` 先例） | claude transcript 文件从几 KB 到 20+MB，整读不可接受（性能约束 3.4）；标题（custom-title/ai-title）由官方写入文件尾部 | 通用机制但 claude 触发 |
| 6 | 可见 prompt 判定 `visible_prompt` | jsonl.rs:134-148 | 首条可见 user prompt：跳过 `isMeta:true` 行、content 为数组行（tool_result 载体）、trim 后以 `<` 开头（`<command-name>`/`<local-command-caveat>` 占位符）、空白行；截断至 200 字符 | claude transcript 的 user 行结构：`isMeta` 元消息、tool_result 数组 content、本地命令占位符形态 | 硬编码 claude |
| 7 | 标题回退链 `resolve_title`：custom-title > ai-title > summary > 首条 prompt（决策 22） | jsonl.rs:217-237；88-115；178-211 | 尾部扫描结果（物理最新）优先于头部候选；同类型 last-wins；`custom-title` 类型恒优先于 `ai-title` | claude transcript 的 `custom-title`/`ai-title`/`summary` 行类型；官方 `/rename` 写 custom-title（本机真实数据实证） | 硬编码 claude |
| 8 | 降级条目契约 + 扫描根缺失空数组（BE-02） | scan.rs:44-46；86-109 | 单文件解析任何失败 → 仅 sessionId + mtime_ms 条目（其余 None/none/false），不阻塞整体、不返回 Err；扫描根不存在 → 空 Vec | claude transcript 损坏/截断（运行中会话末尾不完整行）属常态；新机无 claude 数据属正常 | 通用机制但 claude 触发 |
| 9 | cwd 从 JSONL 内容解析 + `cwd_exists` 孤儿判定 + `mtime_ms` 口径（决策 26） | scan.rs:97-99；112-121；mod.rs:42,51-53 | cwd 取首个含非空 cwd 字段的行（目录名禁止反解码）；`cwd_exists` = cwd 目录当前存在（孤儿会话判定）；mtime = 文件 `metadata().modified()` 毫秒，失败 → 0 | claude 目录名是 cwd 的有损编码不可逆推（事实约束 3.1，仓库内可证）；transcript 文件 mtime 代表会话最后活动时间（外部假设，仓库内不可证） | 硬编码 claude |
| 10 | SEC-05：sessionId 严格校验 + 定位不信托前端 + 绕过 project_root 沙箱 | ops.rs:22-29；40-56；ops.rs:7-10 | 入参仅接受 UUID 形态（天然拒绝 `..`/路径分隔符/空串）；文件定位 = 遍历扫描根一级子目录精确匹配，前端不传任何路径；命令写用户 home 目录、绕过沙箱，入参即攻击面 | 删除目标是 claude home 目录下的 transcript 文件，需在沙箱外访问 | 通用机制但 claude 触发 |
| 11 | delete 删除范围：jsonl + 同名 `<id>/` 目录（含 subagents 附属数据） | ops.rs:78-86 | 删 `<id>.jsonl` 后，同名 `<id>/` 目录存在则 `remove_dir_all`；jsonl 不存在 → Err（「会话不存在」语义） | claude 会话附属数据（subagents 子代理 transcript）存于同名 `<id>/` 目录 | 硬编码 claude |
| 12 | HistorySession 七字段 DTO + TitleSource 五变体 | mod.rs:23-54 | IPC 契约 DTO（serde camelCase，硬约束 #4）：sessionId/cwd/title/titleSource/firstPrompt/mtimeMs/cwdExists + 标题来源五变体 | 字段语义全部来自 claude 会话元数据（transcript 文件名/内容字段） | 硬编码 claude |
| 13 | `claude --resume <id>` 命令注入（决策 25）+ 面板标题 "claude" | restoreSession.ts:122-140 | 终端恢复：轮询 API → `addPanel({ title: "claude", params: { panelId, cwd } })` → 轮询 TerminalRegistry → `pty.write` 注入 `claude --resume <id>`（fork 追加 ` --fork-session`） | claude CLI 的 `--resume <sessionId>` / `--fork-session` 恢复与分支参数（外部假设，仓库内不可证——代码仅按此构造命令） | 硬编码 claude |
| 14 | 四步恢复编排框架（FE-06，决策 6/25） | restoreSession.ts:71-141 | 项目入列（rootPath 与 cwd 规范化相等则复用，否则 addProject）→ 页面保障（无页 addPage 空布局）→ 页面切换（`switchToPageShared`，setProjectRoot 前置 await）→ 终端恢复注入；防重入 + 失败 toast | 编排框架复用既有原语零后端改动；第 4 步注入内容与目标（claude 会话）为 claude 专有 | 通用机制但 claude 触发 |
| 15 | `buildResumeCommand`：`cd '<dir>' && claude --resume <id>` | historyContextMenu.ts:51-54 | 复制恢复命令构造：有 cwd → 带单引号路径 cd 前缀，无 cwd → 仅 `claude --resume <id>` | claude CLI 恢复命令形态 | 硬编码 claude |
| 16 | 操作矩阵禁用态 + 双击三分派 + 动作弹窗（问题 5） | historyContextMenu.ts:63-78；HistorySessionList.tsx:298-314,436-451；SessionActionDialog.tsx:1-8 | 三项操作（复制/分支恢复/删除）禁用态矩阵：孤儿（cwd 目录已删除）与无 cwd 禁用分支恢复、运行中（四态 status 非 null）禁用删除；双击：普通→恢复、孤儿/无 cwd→无操作、运行中→自绘弹窗（Tauri 原生 dialog 仅两按钮） | 孤儿/运行中语义来自 claude 会话：cwd 目录删除后 transcript 成为孤儿；运行中 transcript 文件句柄占用 + 外部进程续写幽灵文件 | 硬编码 claude |
| 17 | 四态同源 `deriveActiveSessionStatuses`（问题 2 修复） | historyModel.ts:123-137；useClaudeHistory.ts:72-77 | 历史区行状态与活跃区同源：TerminalRegistry 条目 → `Map<sessionId, ClaudeStatus>`（sessionId 优先，回退 transcriptPath basename 去 `.jsonl` 兼容旧数据；matchedCommand-only 无定位键——文档化局限）；subscribe 实时跟随不重扫 | claude 会话运行状态由 hook 事件写入 `claudeSession`（sessionId/transcriptPath 字段） | 硬编码 claude |
| 18 | 双行式行：四态 emoji + ✗ 孤儿标记 + CLI 品牌 logo（F9） | HistorySessionRow.tsx:49-54,84-114 | 行1 = 四态 emoji（`STATUS_EMOJI`）+ 16×16 CLI logo（`cliIconRegistry.getSrc("claude")`，仅随 status emoji）+ 粗体标题（null 时 sessionId 前 8 位）+ 相对时间；行2 = 首条 prompt 预览；orphan → ✗（noCwd 不显示） | 四态来自 claude hook 事件状态机；孤儿判定基于 claude transcript cwd；显式 `getSrc("claude")`（当前会话恒为 claude） | 硬编码 claude |
| 19 | `findPanelForSession` 反查运行中面板 + `switchToPageAndFocus` | HistorySessionList.tsx:192-204,281-296 | 双击运行中行 → 「切换到该会话操作页面」：claudeSession.sessionId 精确匹配（回退 transcriptPath basename）→ `parseTerminalPageId` → `switchToPageAndFocus`；反查不到 → toast | claudeSession 是 claude 会话的运行态模型（sessionId/transcriptPath 字段） | 硬编码 claude |
| 20 | 删除流程：ask 确认 → `deleteHistorySession` → `removeLocal` | HistorySessionList.tsx:334-351；useClaudeHistory.ts:66-68 | ask 弹窗确认 → 删除 IPC（后端 SEC-05 校验）→ 成功后 `removeLocal` 纯本地即时移除（不重扫）；失败 console.error | 删除对象为 claude transcript（claude_history_delete 命令） | 通用机制但 claude 触发 |
| 21 | 三下拉框结构 + 首次展开懒加载 scan + 组默认收起（问题 3） | ClaudeHistorySections.tsx:133-235；HistorySessionList.tsx:253-268 | 受控组件（useClaudeHistory 上提 AgentStatusView 单实例）：搜索框 + 刷新按钮 + 当前项目/全部项目两历史区；首次展开触发 scan()（仅一次）；all 区 expandedGroups 白名单默认收起 + basename + (N) 计数 | UI 结构本身通用，展示的数据是 claude 历史会话（scan 由 claude_history_scan 驱动） | 通用机制但 claude 触发 |
| 22 | 展示派生纯函数：`isCurrentProject`/`groupByCwd`/`matchesSearch`/`formatRelativeTime` | historyModel.ts:21-111 | normalizePath + 忽略大小写精确相等（决策 24）；按 cwd 分组（未知目录归 null 组，组内/组间 mtime 降序）；标题+首条 prompt 大小写不敏感搜索；六档相对时间（刚刚/分钟/小时/天/同年 MM-DD/跨年 YYYY-MM-DD，mtime≤0 → 「-」） | 无——纯展示派生底层能力，不依赖任何 claude 概念（输入字段来自 claude DTO 但函数本身无 claude 假设） | 完全通用 |
| 23 | 活跃区标题覆盖（问题 6 修复） | AgentStatusView.tsx:126-141；claudeHistory/CLAUDE.md「活跃区标题覆盖」段 | 活跃区渲染行前经 `titleBySessionId`（scan 结果中 title 非 null 者）覆盖 hook 事件行的 resolveTitle 值；无匹配或 title null → 回退行原标题 | `/rename` 写 transcript custom-title 后点刷新，scan 结果即为新标题，活跃区自动同步（对齐官方行为） | 硬编码 claude |
| 24 | IPC 命令名 `claude_history_scan`/`claude_history_delete` + DTO 双边契约 | src/ipc/claudeHistory.ts:13-24；src/types/claudeHistory.ts:6-29 | wrapper 为两命令唯一 invoke 位置；`HistorySession`/`TitleSource` TS 类型与后端 serde 逐字对应（硬约束 #4） | 命令名与字段语义均以 claude 历史会话为对象（rename 已随功能整体移除，官方 `/rename` 是 custom-title 唯一写入方） | 硬编码 claude |

## 详细机制描述

### 1. 扫描根单点 `resolve_projects_root()`（SEC-02/BE-06）

`scan.rs:21-26`。解析顺序：`SLTERM_CLAUDE_PROJECTS_DIR` env 非空 → 用之；否则 `dirs::home_dir()/.claude/projects`。每次调用时读取 env（不缓存）——E2E 子进程继承 env 即可生效，测试经 `e2e-tests/run-wdio.cjs` 注入 fixture 副本路径，防止任何用例触碰用户真实 `~/.claude/projects/`（`scan.rs:17-20` 注释；模块 CLAUDE.md「扫描根单点」段）。**专属程度：硬编码 claude**——扫描根路径本身是 claude 数据目录（`.claude/projects`），env 覆盖机制是通用的测试隔离手段。

### 2. 存储布局假设：cwd 编码目录 + `<uuidv4>.jsonl` 会话文件（禁止反解码）

`mod.rs:9-10` 数据源事实约束：存储根一级目录名 = cwd 的有损编码（**禁止反解码**），会话文件 = `<uuidv4>.jsonl`（文件名主干即 sessionId）。`scan.rs:51-67` 只遍历一级子目录的直属文件；`jsonl.rs:48` 注释重申「cwd 一律从 JSONL 内容解析（目录名只是 cwd 的有损编码，禁止反解码）」。**专属程度：硬编码 claude**——整个遍历策略建立在 claude 的目录/文件命名约定上（外部假设，仓库内不可证——代码仅按此布局约定遍历）。

### 3. 排除规则：`agent-*.jsonl` 平铺 / 非 UUID 主干 / subagents 子目录不递归

`scan.rs:72-80` `is_session_jsonl`：扩展名 jsonl + 文件名主干 UUID 形态 + 非 `agent-` 前缀；`scan.rs:56` 只扫一级子目录直属文件、不递归——`<id>/subagents/` 子目录天然不命中。对应 claude 子代理 transcript 两种形态（`agent-*.jsonl` 平铺 / `<id>/subagents/` 子目录），均须排除（模块 CLAUDE.md「容错与降级契约」段）。**专属程度：硬编码 claude**——`agent-*` 前缀是 claude 子代理 transcript 命名（外部假设，仓库内不可证——仓库内证据仅为代码注释与测试构造）。

### 4. `is_uuid_filename` UUID 形态校验

`mod.rs:59-74`。纯函数：36 长度 + 连字符位置（8/13/18/23）+ ascii hex 全检，大小写不敏感。scan 侧用于排除非会话文件（`scan.rs:79`），ops 侧用于 SEC-05 删除校验（`ops.rs:23`）。**专属程度：通用机制但 claude 触发**——UUID 形态校验本身完全通用，其在本模块的触发点是 claude 用 UUID v4 命名会话文件（文件名主干即 sessionId）。

### 5. 头部 512KB + 尾部 64KB 双窗口轻量解析（BE-03/BE-04）

`jsonl.rs:16-19` 常量 `HEAD_SCAN_LIMIT_BYTES = 512KB`、`TAIL_SCAN_BYTES = 64KB`（照 `hooks/usage.rs` `TRANSCRIPT_TAIL_BYTES` 先例）。性能约束（规格 3.4）：claude transcript 文件从几 KB 到 20+MB，**禁止整文件读取**（`jsonl.rs:1-8` 模块头注释）。

- **头部窗口**（`parse_head`，`jsonl.rs:50-128`）：顺序逐行扫描，收集 cwd（首个含非空 cwd 字段的行）、custom-title/ai-title/summary 候选（同类型 last-wins）、首条可见 user prompt（命中即提前结束）；累计读取超 512KB 或单行 JSON 解析失败（EOF 截断/损坏）即停止，不报错。
- **尾部窗口**（`parse_tail_title`，`jsonl.rs:160-212`）：从文件尾部读 ≤64KB，从中途起始跳过首行（截断行）；逆序扫描——`custom-title` 类型恒优先（遇到即返回），全程无则返回最后一条 `ai-title`（覆写式 last wins）。

大文件头尾协同有专门测试（`jsonl.rs:451-519`）：>512KB 中部标题收不到、尾部 64KB 内标题命中。**专属程度：通用机制但 claude 触发**——双窗口避免整读是通用性能手法，其参数与激活场景（20+MB transcript）来自 claude 数据格式；窗口内解析的具体字段（custom-title/ai-title/summary/user）是 claude 专有，见 #6/#7。**外部假设标注**：「标题（custom-title/ai-title）由官方写入文件尾部」为外部假设——代码仅证明「尾部 64KB 窗口扫描 custom-title/ai-title 行」存在，官方将标题写入文件尾部的位置特性仓库内不可证（对比 #7 有本机真实数据实证、#23 有模块 CLAUDE.md 记载背书）。

### 6. 可见 prompt 判定 `visible_prompt`

`jsonl.rs:134-148`。首条可见 user prompt 判定：`type=="user"` 且 `message.content` 为**字符串**（数组 = tool_result 载体跳过）、`isMeta != true`（元消息跳过）、trim 后以 `<` 开头跳过（`<command-name>`/`<local-command-caveat>`/`<local-command-stdout>` 等本地命令占位符）、trim 后为空跳过；`truncate_prompt` 按字符截断至 200 字符（UTF-8 安全，`jsonl.rs:151-153`）。**专属程度：硬编码 claude**——`isMeta`、tool_result 数组 content、本地命令占位符均为 claude transcript 专有结构（外部假设，仓库内不可证——jsonl.rs:43-46 注释与测试构造（jsonl.rs:309-348）为唯一仓库内证据，无本机真实数据实证）。

### 7. 标题回退链 `resolve_title`（决策 22）

`jsonl.rs:217-237`。合成顺序：**custom-title > ai-title > summary > 首条 prompt**；四路皆空 → `(None, None)`（titleSource=none）。尾部扫描结果（物理最新）优先于头部任何候选（`resolve_tail_beats_head_candidates` 测试，`jsonl.rs:646-659`）。头部候选收集在同类型内 last-wins（`jsonl.rs:88-115`）。决策背景（模块 CLAUDE.md「标题回退链」段）：重命名写 custom-title 是对齐官方 `/rename`——本机真实数据证实官方写 custom-title，推翻了早期 ai-title 决策。**专属程度：硬编码 claude**——`custom-title`/`ai-title`/`summary` 行类型与官方 `/rename` 写盘行为均是 claude 专有。

### 8. 降级条目契约 + 扫描根缺失空数组（BE-02）

`scan.rs:86-109` `parse_session_file`：任何解析失败不返回 Err——降级为仅 sessionId + mtime_ms 的条目，其余字段 None / titleSource=none / cwdExists=false（损坏 JSONL、空文件、不可读路径均有对应测试，`scan.rs:311-379`）。`scan.rs:44-46` 扫描根不存在 → 空 Vec（注释明确「新机无 claude 数据属正常」）。**专属程度：通用机制但 claude 触发**——容错降级是通用工程契约，其语义背景（claude transcript 运行中末尾截断、新机无 claude 数据）来自 claude。

### 9. cwd 内容解析 + `cwd_exists` 孤儿判定 + `mtime_ms` 口径（决策 26）

`scan.rs:97-99`：cwd 取头部窗口解析结果（首个含非空 cwd 字段的行），`cwd_exists` = cwd 非 null 且目录当前存在——孤儿会话判定（前端 ✗ 标记与恢复禁用依赖此字段）。`scan.rs:112-121` `file_mtime_ms`：`metadata().modified()` 转 Unix 毫秒，失败 → 0（决策 26，时间口径 = 文件 mtime）。**专属程度：硬编码 claude**——cwd 必须从 JSONL 内容解析而非反解码目录名（**禁止反解码约束见 #2，同一约束**），是 claude 目录命名「有损编码」事实约束的直接产物；孤儿判定语义（cwd 目录被删除的 transcript）也是 claude 会话生命周期特性。mtime 读取本身通用。

### 10. SEC-05：sessionId 严格校验 + 定位不信托前端 + 绕过 project_root 沙箱

`ops.rs:22-29` `validate_session_id`：复用 `is_uuid_filename`（36 长度 + 连字符位置 + ascii hex 全检），天然拒绝含 `..`、路径分隔符、空串、超长的一切非 UUID 输入 → `AppError::Validation`。`ops.rs:40-56` `locate_session_jsonl`：遍历扫描根一级子目录精确匹配 `<session_id>.jsonl`，**前端不传任何路径**。`ops.rs:7-10` 安全模型注释：命令写用户 home 目录文件、绕过 project_root 沙箱（照 `hooks/config.rs` user 层先例），入参即攻击面——sessionId 严格校验是唯一防线。越界防护有哨兵文件测试（`ops.rs:261-284`）。**专属程度：通用机制但 claude 触发**——「严格校验入参 + 不信托前端路径」是通用安全模式，其适用对象（claude home 数据、沙箱外访问）由 claude 数据位置决定。**（注：ops.rs 代码注释 8 处内部编号为 SEC-01——文件头 :1/:4/:7、`validate_session_id` doc :16、`locate_session_jsonl` doc :36、`claude_history_delete` doc :60、测试区 :127/:138——滞后于需求登记表 SEC-05，追溯时以登记表为准。）**

### 11. delete 删除范围：jsonl + 同名 `<id>/` 目录（含 subagents 附属数据）

`ops.rs:78-86` `delete_session`：删 `<id>.jsonl` 后，同名 `<id>/` 目录存在则 `remove_dir_all`（含 subagents 附属数据）；jsonl 不存在 → `session_not_found`（Validation 变体，消息含「不存在」语义，`ops.rs:32-34`）。删除范围对应 claude 会话的完整数据足迹（规格 4.4）。**专属程度：硬编码 claude**——`<id>/` 目录存 subagents 子代理数据是 claude 存储形态。

### 12. HistorySession 七字段 DTO + TitleSource 五变体

`mod.rs:23-54`。`HistorySession` 七字段（serde camelCase）：sessionId（文件名主干 = UUID）、cwd（JSONL 内容解析）、title（回退链合成结果）、titleSource、firstPrompt（≤200 字符）、mtimeMs、cwdExists。`TitleSource` 五变体：customTitle/aiTitle/summary/firstPrompt/none。前端 `src/types/claudeHistory.ts:6-29` 逐字对应（硬约束 #4）。**专属程度：硬编码 claude**——DTO 的每个字段语义都来自 claude transcript 格式（文件名 UUID、JSONL 行字段、custom-title/ai-title 标题源）。

### 13. `claude --resume <id>` 命令注入（决策 25）+ 面板标题 "claude"

`restoreSession.ts:122-140`。四步恢复的最后一步：轮询 `getPageApi` 就绪（100ms×50，照 `openHooksConfigPanel` 模式）→ `addPanel({ id: "terminal-{pageId}-{Date.now()}", component: "terminal", title: "claude", params: { panelId, cwd }, renderer: "always" })` → 轮询 `TerminalRegistry.get(panelId)` → `pty.write` 注入 `claude --resume {sessionId}` + `\r`（fork 追加 ` --fork-session`）。注释（`restoreSession.ts:136`）：OSC 133 / hooks 全链路随终端自然生效，零后端改动。**专属程度：硬编码 claude**——注入命令、fork 参数、面板标题均显式 claude 字样。

### 14. 四步恢复编排框架（FE-06，决策 6/25）

`restoreSession.ts:71-141` `doRestore` 四步：① 项目入列——`useProjects` 查 rootPath 与 session.cwd 规范化（normalizePath + 忽略大小写）精确相等则复用，无则 `addProject`（字段形状照 `SidebarTree.handleAddProject`，`restoreSession.ts:79-95`）；② 页面保障——项目 pages 为空则 `addPage`（`页面-${Date.now()%10000}` + `makeEmptyLayout` 空布局——**时间戳取模非递增序号**（「N」为递增计数是 terminal-N 的语义，此处勿按递增计数器理解），照 `handleNewPage` 模式（`SidebarTree.tsx:380` 同款），`restoreSession.ts:97-112`）；③ 页面切换——`switchToPageShared`（setProjectRoot 前置 await 由其内部保证，DBG-5，`restoreSession.ts:114-115`）；④ 终端恢复（见 #13）。防重入：模块级 `restoring` 标记（`restoreSession.ts:42,54`）；失败：`sendToastNotification` + console.error（`restoreSession.ts:58-65`，场景 10）。**专属程度：通用机制但 claude 触发**——四步编排框架（项目入列/页面保障/切换/终端注入）是通用工作区状态恢复编排，全部复用既有原语零后端改动；其注入内容（claude 命令）与触发对象（claude 会话）为 claude 专有（见 #13）。

### 15. `buildResumeCommand`：`cd '<dir>' && claude --resume <id>`

`historyContextMenu.ts:51-54`。复制恢复命令构造：有 cwd → `cd '<cwd>' && claude --resume <id>`（带单引号路径）；无 cwd → 仅 `claude --resume <id>`。复制动作经 `writeText` 写剪贴板（`HistorySessionList.tsx:326-328`）。**已知限制**：cwd 含单引号时命令被破坏（PowerShell 单引号内 `'` 需 `''` 转义，`historyContextMenu.ts:53`）——fixture 与常规 Windows 路径无此形态，E2E 未覆盖，当前为已知限制不修；抽象恢复命令构造器时引号处理需平台/Shell 感知。**专属程度：硬编码 claude**——命令本体是 claude CLI 恢复命令。

### 16. 操作矩阵禁用态 + 双击三分派 + 动作弹窗（问题 5）

`historyContextMenu.ts:63-78` `getHistoryContextMenuItems` 操作矩阵：复制恢复命令全行可用（含孤儿/运行中）；分支恢复孤儿（`orphan`）/无 cwd（`noCwd`）禁用；删除运行中（`active`）禁用——原因（`historyContextMenu.ts:18` 注释）：运行中文件句柄占用删除失败 + 外部进程续写幽灵文件。`HistorySessionList.tsx:298-314` 双击三分派：运行中（status 非 null）→ `SessionActionDialog` 弹窗（「切换到该会话操作页面」/取消；分支恢复仅保留在右键菜单）——**隐含机理假设**：同一 sessionId 二次 `claude --resume` 会与现有会话冲突——**普通恢复（非 fork）与运行中会话冲突，仅 fork 变体可用**（`HistorySessionList.tsx:439` 弹窗文案「该会话已在运行中，恢复会与现有会话冲突。」；「二次 resume 冲突」本身为**外部假设，仓库内不可证**——仅可证弹窗文案与禁用矩阵（`historyContextMenu.ts:76` `disabled: opts.active`））；与删除禁用的「运行中文件句柄占用删除失败 + 外部进程续写幽灵文件」注释（`historyContextMenu.ts:18`）形成对照——该假设决定运行中会话的恢复类操作分派策略是否按 CLI 语义差异化；孤儿/无 cwd → 无操作（恢复失败概率高，禁用优于报错）；普通 → `restoreHistorySession`。弹窗自绘原因（`SessionActionDialog.tsx:1-8`）：Tauri 原生 dialog 仅两按钮，无法表达多选项。**（反查与切换机制唯一入口见 #19——本条为分派矩阵视角）**。**专属程度：硬编码 claude**——孤儿语义（cwd 目录已删除的 transcript）、运行中语义（claudeSession 四态）、句柄占用/幽灵文件问题均为 claude 会话生命周期特性。

### 17. 四态同源 `deriveActiveSessionStatuses`（问题 2 修复）

`historyModel.ts:123-137`。从 `TerminalRegistry.getAll()` 派生 `Map<sessionId, ClaudeStatus>`：sessionId 优先 `claudeSession.sessionId`（hook 事件 payload 精确值），回退 transcriptPath basename 去 `.jsonl` 后缀（兼容旧数据）；两者皆无（matchedCommand-only 会话）不产出键——文档化局限；status 为 null/undefined 不产出键（历史区无标记，与活跃区 null 无图标语义一致）。`useClaudeHistory.ts:72-77`：`TerminalRegistry.subscribe` 订阅 register/remove/sessionChange 任一事件重算，不重扫；卸载清理。**专属程度：硬编码 claude**——`claudeSession`（sessionId/transcriptPath/status 字段）与四态 `ClaudeStatus` 均为 claude 会话机制（hook 事件写入）。

### 18. 双行式行：四态 emoji + ✗ 孤儿标记 + CLI 品牌 logo（F9）

`HistorySessionRow.tsx:49-54`：title 为 null（**无标题，含降级条目**——custom/ai/summary/prompt 四路皆空同样 null 且 titleSource=none，不必然降级）→ 显示 sessionId 前 8 位；`statusIcon = STATUS_EMOJI[status]`；`logoSrc = cliIconRegistry.getSrc("claude")`——注释（`HistorySessionRow.tsx:52-53`）：「当前历史区会话均为 claude（HistorySession 无 CLI 字段），直接取注册表 claude 条目；未来新增编码 CLI 时按行 CLI 标识扩展」。行1（`HistorySessionRow.tsx:84-114`）：CLI logo 仅随 status emoji 渲染（status 为 null / 孤儿 ✗ 行不加图）；orphan → ✗（11px 灰）；右上角相对时间。行2：首条 prompt 预览单行截断（null 不渲染）。**专属程度：硬编码 claude**——显式 `getSrc("claude")` 字样 + 四态 emoji 来自 claude hook 事件状态机（`src/lib/claudeStatus.ts`）。

### 19. `findPanelForSession` 反查运行中面板 + `switchToPageAndFocus`

`HistorySessionList.tsx:192-204` `findPanelForSession`：遍历 `TerminalRegistry.getAll()`，`claudeSession.sessionId` 精确匹配，回退 transcriptPath basename 去 `.jsonl`（旧数据兼容；**与 03-17 同回退语义——代码层抽象时可提取共享纯函数**）；未命中 → undefined。`HistorySessionList.tsx:281-296` `handleSwitchToSession`：反查不到 → toast「未找到运行中的会话」；命中 → `parseTerminalPageId(panelId)` → `switchToPageAndFocus(pageId, panelId)`（内部：activePageId 相同则直接聚焦，不同则先切页）。**专属程度：硬编码 claude**——定位键（claudeSession.sessionId/transcriptPath）是 claude 会话运行态模型字段。

### 20. 删除流程：ask 确认 → `deleteHistorySession` → `removeLocal`

`HistorySessionList.tsx:334-351`：ask 弹窗确认（warning 类型）→ `deleteHistorySession(session.sessionId)`（后端 SEC-05 校验，见 #10）→ 成功后 `removeLocal` 即时局部刷新（不重扫）；失败 console.error。`useClaudeHistory.ts:66-68` `removeLocal`：`setSessions` 过滤移除，不触发重扫（删除 IPC 由调用方先执行）。**专属程度：通用机制但 claude 触发**——确认→删除→局部刷新的流程通用，删除对象与后端命令是 claude 的。

### 21. 三下拉框结构 + 首次展开懒加载 scan + 组默认收起（问题 3）

`ClaudeHistorySections.tsx:133-235`：受控组件（useClaudeHistory 上提至 AgentStatusView 单实例，问题 6 修复）——**口径说明：「三下拉框」= AgentStatusView 三个可展开区块（活跃会话 + 当前项目历史 + 全部项目历史），活跃区行建模属 04 领域（04-13），本条机制描述只覆盖两历史区**——搜索框（`agent-history-search`）+ 刷新按钮（`agent-history-refresh`）+ 当前项目历史会话/全部项目历史会话两区（受控展开）；空态/提示文案共 5 种（「无活跃项目」「该项目暂无历史会话」「暂无历史会话」「无匹配的会话」+ **「扫描中…」loading**，`ClaudeHistorySections.tsx:190-191,216-217`，FE-09）；历史区首次展开触发 `scan()`（仅首次，之后靠刷新按钮，`ClaudeHistorySections.tsx:149-156`）。`HistorySessionList.tsx:253-268`：all 区二级折叠用 expandedGroups 白名单（初始空 = 默认收起，问题 3 修复——黑名单模型已废弃），组标题 = basename + (N) 计数（未知目录组「(未知目录)」）。**专属程度：通用机制但 claude 触发**——下拉框/懒加载/白名单折叠均为通用 UI 模式，数据源与 scan 命令是 claude 历史会话。

### 22. 展示派生纯函数（完全通用）

`historyModel.ts:21-111` 四个纯函数：`isCurrentProject`（normalizePath 反斜杠→正斜杠 + toLowerCase 后精确相等，任一侧 null/空串 → false，决策 24）；`groupByCwd`（分组键 = 规范化 cwd，null 归 `UNKNOWN_CWD_KEY` 组，组内 mtimeMs 降序、组间按组内最大 mtimeMs 降序；**组展示 cwd 取组内最大 mtime 会话的原始写法**（`historyModel.ts:65` `list[0].cwd`，不规范化——与分组键的规范化形成对照）；**组键漂移现状**：`HistorySessionList.tsx:402` 的 expandedGroups 键 = `group.cwd`（原始写法），同目录不同写法归一组但组键随组内最大 mtime 会话/搜索过滤漂移，展开状态可能丢失（完全通用，非 claude 专属，抽象分组折叠状态持久化时注意））；`matchesSearch`（标题 + firstPrompt 大小写不敏感 includes，query 空白恒 true）；`formatRelativeTime`（六档：刚刚 <1min / N 分钟前 / N 小时前 / N 天前 <7d / 同年 MM-DD / 跨年 YYYY-MM-DD，mtimeMs ≤ 0 → 「-」，含时钟偏差容错）。**历史区相对时间无 ticker**：`HistorySessionRow.tsx:50` `formatRelativeTime(session.mtimeMs, Date.now())` 仅渲染时计算，无活跃区 60s ticker（04-16）驱动——历史区相对时间文本在无重渲染时冻结，与活跃区行为不一致（通用展示差异，非 claude 专属；抽象统一时间组件时注意两区行为不一致）。**专属程度：完全通用**——四个函数均为零 claude 概念依赖的纯展示派生底层能力（输入字段来自 claude DTO，但函数本体无任何 claude 假设，任何会话/文件列表可直接复用）。

### 23. 活跃区标题覆盖（问题 6 修复）

`AgentStatusView.tsx:126-141`：活跃区渲染行前经 `titleBySessionId`（sessions 中 title 非 null 者）覆盖 `row.title`；无匹配 sessionId 或标题为 null → 回退行原标题（dockview 面板标题）。出处：`src/features/claudeHistory/CLAUDE.md`「活跃区标题覆盖」段——`/rename` 写 transcript custom-title 后点刷新，scan 结果即为新标题，活跃区自动同步；hook 事件 setRows 的 resolveTitle 值被视图层覆盖。**专属程度：硬编码 claude**——覆盖机制对齐 claude 官方 `/rename` 写 custom-title 的行为（title 数据源 = claude transcript 扫描结果）。

### 24. IPC 命令名 + DTO 双边契约

`src/ipc/claudeHistory.ts:13-24`：`scanHistory()` → `invoke("claude_history_scan")`（无参）、`deleteHistorySession(sessionId)` → `invoke("claude_history_delete", { sessionId })`（前端不传路径，后端 SEC-05 校验）；文件头注释「本文件是两条命令的唯一 invoke 位置（硬约束 #1）」。`src/types/claudeHistory.ts:6-29`：`TitleSource` 五变体 + `HistorySession` 七字段 TS 接口，与 `mod.rs` serde 输出逐字对应（硬约束 #4，防字段漂移）。**专属程度：硬编码 claude**——命令名（`claude_history_*`）与字段语义均以 claude 历史会话为对象；契约测试（`src/__tests__/ipc-claude-history-contract.test.ts`，8 用例）守护 JS 侧形状（命令名/参数结构/返回透传/异常传播四维）。
