# review-fix 修复 — Stage 划分

> 清单真值源：`docs/review-fix/checklist.md`（逐 ID，30 条）。编排参数：`docs/review-fix/execution-plan.md`。
>
> **划分原则**：Stage 内文件不重叠（硬性，pipeline 串行块除外）；Stage 间允许重复碰同一文件（串行 + 每 Stage commit）；并行 agent ≤ 5；文档同步固定末位 Stage。
>
> **依赖序**：安全族 + 守卫强化先行（CS-2 扩扫描范围后守护后续全部 Stage 的字面量纪律）→ 前端正确性 → transcript 中性化（跨前后端契约链）→ hub 编辑器分派 + 层抽象（KZ-7/CS-3 依赖其产物）→ mockcli 验收强化 → 文档终验。
>
> **统一门禁（每 Stage 全量；L4 按需——决策 4）**：
> ```
> npx tsc --noEmit
> npx eslint src/
> cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
> cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
> cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1   # L1
> npm test                                                            # L2
> npm run test:l3                                                     # L3
> ```
> **L4（`npm run e2e` = build:e2e + wdio 串行，禁拆分/禁并行）仅 Stage 01/03/05 + Stage 06 收尾**：
> - Stage 01：`e2e-tests/run-wdio.cjs` 无任何静态门禁覆盖（纯 Node 脚本，tsc include 为 `["src","test/**/*"]`，eslint 仅扫 src/）——补 `node --check e2e-tests/run-wdio.cjs` + L4 实跑兜底
> - Stage 03：更名波及 `e2e-tests/*.e2e.ts`（wdio 运行时才编译，无静态门禁）——L4 实跑兜底
> - Stage 05：CS-3 新增 L4 用例 + `e2e-tests/helpers.ts`（vite 打包图覆盖语法级、类型级无门禁）——L4 实跑
> - Stage 06 收尾：全量复跑含 L4 + L1（**先关闭运行中的 slterminal.exe**——review-00 基线门禁遗留：进程占用致链接器 os error 5）
>
> ⚠️ L4 必须用 `npm run e2e`——config.json 的 `e2eBuild`（`npx tauri build --debug --no-bundle`）**不带 `VITE_E2E=1`**，直接用会 tree-shake 掉 E2E helper，wdio 全卡「Workspace 未就绪」。

## 跨边界契约（写死——各 Stage 脚本头部重复此段，agent 不各自推断）

### 契约 1：`usageSourcePath` 全链路更名（Stage 03，决策 1）

```
后端 DTO（signal.rs）：
  pub usage_source_path: Option<String>   // #[serde(default)]，rename_all camelCase → JS 键 usageSourcePath
  —— 不加 serde alias：信号文件瞬态（亚秒~3s 存活），旧键信号降级 None（仅丢该事件用量拉取）；
     reporter SCRIPT_VERSION 递增后版本门控引导重新注入（决策 7 先例）
trait（provider.rs）：
  fn context_usage(&self, usage_source_path: &str) -> Result<Option<ContextUsage>, AppError>
  // trait 文档注明：路径语义由具体 CLI 解释（claude = transcript JSONL）
命令（mod.rs）：agent_context_usage(cli_id: String, usage_source_path: String)
  —— JS invoke 键 { cliId, usageSourcePath }（Tauri camelCase 双边）
reporter（slterm-hook-reporter.js）：
  payload 键 transcriptPath → usageSourcePath: data.transcript_path || null
  —— data.transcript_path 是 claude hook stdin 协议字段（snake_case），更名不动
  —— SCRIPT_VERSION 2 → 3（已注入用户显示「版本过旧」需重新注入，测试锁死此形态）
前端 DTO（types/agent.ts）：usageSourcePath?: string | null（原 transcriptPath: string 必填 → 可选）
前端内部状态（TerminalRegistry.ts AgentSessionInfo）：transcriptPath → usageSourcePath（可选语义不变）
wrapper（agentHooks.ts）：contextUsage(cliId, usageSourcePath)
```

**更名豁免（不动）**：`transcript_path` snake_case 形态仅出现于 claude hook stdin 协议模拟（hooks.e2e.ts:170）与 claude provider 内部（scan_transcript_usage 函数名/JSONL 解析语义——claude 合法领地知识）；文档中「transcript JSONL」作为 claude 概念名词可保留（CONTEXT.md 术语表除外——术语条目更名）。

**消费方全量清单**（grep 实查 2026-08-11，含测试/文档，禁凭记忆）：

| 侧 | 文件 |
|----|------|
| 后端 | `hooks/signal.rs`（DTO + 11 处测试 JSON）、`hooks/provider.rs:33`、`hooks/mod.rs:162/165/225/227` + 测试 `:342/364/431/439/523`、`hooks/claude/mod.rs:128-129`、`hooks/claude/usage.rs:344/346/362`（测试）、`hooks/claude/slterm-hook-reporter.js:50`、`hooks/CLAUDE.md:36/158/160/198/243/316` |
| 前端契约 | `src/types/agent.ts:26`、`src/ipc/agentHooks.ts:42/44`、`src/__tests__/ipc-agent-hooks-contract.test.ts:158-165/226/273/288`、`src/ipc/CLAUDE.md:26`、`src/types/CLAUDE.md`（agent.ts 行） |
| 前端消费 | `src/panels/terminal/TerminalRegistry.ts:16/95`、`src/panels/terminal/useXterm.ts:384`、`src/features/agentStatus/useAgentStatus.ts:43/179-180/196/207-208/264/272/340/346`、`src/features/agentHistory/historyModel.ts:121-135`、`src/features/agentHistory/HistorySessionList.tsx:201-203` |
| 前端测试 | `src/__tests__/agent-status-hook.test.ts`（:6/81-84/236/248/260/269/446-470/543-561/906-994 共 18 处）、`terminal-registry.test.ts:106-162/305/317`、`mock-cli-profile.test.tsx:283/508/511`、`notifications.test.ts:132`、`cli-profile-claude.test.ts:353`、`agent-history-hook.test.tsx:130`、`agent-history-model.test.ts:251/303/319/326` |
| L3 | `test/terminal/production-osc.test.ts:106`（注释） |
| E2E | `e2e-tests/agent.e2e.ts`（:235/249/263/284/304/315/349/355/385/446/461/539）、`e2e-tests/hooks.e2e.ts:107/126`（:170 snake_case stdin 不动）、`e2e-tests/history.e2e.ts:488/497` |
| 文档 | `CONTEXT.md:229`、`src/features/agentStatus/CLAUDE.md:27`、`src/features/agentHistory/CLAUDE.md:43`、`src/panels/CLAUDE.md:277`、`.claude/test-inventory.md:282` |

### 契约 2：`configEditor` + `configLayers` 入 profile（Stage 04，形态决策已定）

```ts
// src/features/cliProfiles/types.ts（HooksCapability 追加两字段；React 仅类型 import）
/** hub 配置编辑器组件 props（泛化自 ClaudeHooksConfigEditorProps） */
export interface HooksConfigEditorProps {
  profile: CodingCliProfile;
  onDirtyChange?: (dirty: boolean) => void;
  askGuardRef?: React.MutableRefObject<boolean>;
}
interface HooksCapability {
  // ……现有五字段（eventToStatus/classifyNotification/contextLimit/restartHint/hasConfigEditor）不动
  /** hub 配置编辑器组件（hasConfigEditor=true 时必填；缺失 → hub 空态防御） */
  configEditor?: React.ComponentType<HooksConfigEditorProps>;
  /** hooks 配置分层声明（hasConfigEditor=true 时必填；claude = user/project/local 三层现值） */
  configLayers?: { id: string; label: string; hint: string }[];
}

// src/types/hooksConfig.ts
export type HooksLayer = string;  // 值集由 profile.capabilities.hooks.configLayers 声明（claude = "user"|"project"|"local"）
```

- hub 分派：`const Editor = selectedProfile?.capabilities.hooks?.configEditor` → `<Editor key={selectedProfile.id} profile={selectedProfile} onDirtyChange={...} askGuardRef={...} />`；缺失 → 空态占位（不渲染 claude 编辑器）
- 选择行过滤条件不变（`hasConfigEditor === true`）
- claude profile 挂载：`configEditor: ClaudeHooksConfigEditor`（profiles/claude/index.ts import `panels/hooksConfig/ClaudeHooksConfigEditor`——**新增 features→panels 依赖方向**，合法化理由：profiles/claude/ 是 claude 合法领地，编辑器组件是 claude 专属资产；就近文档注明）
- `configLayers` claude 值 = 现 LAYERS 三层（含 hint 文案）；ClaudeHooksConfigEditor 的 LAYERS 常量退役、PRIORITY_HINT 由 layers 派生或随编辑器内部保留（claude 领地内可硬编码，执行期定）；useHooksConfig 初始层 = `configLayers[0]`
- 后端零改动（trait `config_read/write` 的 layer 本为字符串；`parse_layer` 三层校验是 claude provider 内部知识）

### 契约 3：`keyOf` 复合键单点（Stage 02，agent 内部契约）

```ts
// src/features/agentHistory/historyModel.ts 导出（FE-05 纯函数模型单点）
export function keyOf(cliId: string | null | undefined, sessionId: string): string
// 内部：(cliId ?? CLAUDE_CLI_ID) 回退 + 两侧 `replaceAll("|", "\\|")` 转义 → `${a}|${b}`
// 消费方比较/查键一律经 keyOf——生产消费同函数即口径一致
```

### 契约 4：`resolvePayloadCliId` 单点（Stage 02，agent 内部契约）

```ts
// 新建 src/panels/terminal/resolvePayloadCliId.ts（与 TerminalRegistry 同域；三消费方均有 import panels/terminal 先例）
export function resolvePayloadCliId(payload: AgentEventPayload): string
// payload.cliId?.trim() || TerminalRegistry.get(payload.panelId)?.agentSession?.cliId || CLAUDE_CLI_ID
// ——空串/仅空白/null/undefined 同等回退（ZQ-2），MC-205 三级解析语义不变
```

### 禁区（PREAMBLE 必含）

1. `compute_conpty_flags` 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 `exit(0)`、不写 stderr——Stage 03 改 payload 键时勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——Stage 01 改 ops.rs 时勿削弱（is_symlink 是加防御不是松校验）
5. E2E 不得触碰用户真实 `~/.claude/`——AQ-4 正是强化此防线，改 run-wdio.cjs 时勿引入新降级路径
6. `E2E_ENABLED` 保持内联 `import.meta.env` 字面量形态（rolldown DCE 红线）——Stage 05 改 helpers.ts 时勿动

---

## Stage 01 — 安全族 + 字面量守卫强化

**条目**：AQ-1、AQ-2、AQ-3、AQ-4、ZQ-5、CS-1、CS-2（7 条）

**agent 分工表**（5 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| backend-guard | AQ-2（信号大小限制）、AQ-3（符号链接拒跟随） | 改 `src-tauri/src/hooks/signal.rs`（含 #[cfg(test)] 新用例）、`src-tauri/src/agent_history/claude/ops.rs`（含 #[cfg(test)] 新用例）；就近同步 `src-tauri/src/agent_history/CLAUDE.md`（**不动 hooks/CLAUDE.md**——见下方裁决） |
| backend-config | ZQ-5（null 视作 {}） | 改 `src-tauri/src/hooks/claude/config.rs`（含 #[cfg(test)] 新用例）；就近同步 `src-tauri/src/hooks/CLAUDE.md`（write 语义段 + 代 backend-guard 补 AQ-2 大小限制一句——单点负责防同文件并发冲突） |
| frontend-strategy | AQ-1（cwd 单引号转义） | 改 `src/features/cliProfiles/profiles/claude/strategies.ts`、`src/__tests__/cli-profile-claude.test.ts`（逐字断言更新 + 单引号回归用例）；就近同步 `src/features/cliProfiles/CLAUDE.md`（strategies 行「逐字一致」表述）——**不动 test-inventory**（归 literal-guard 代同步） |
| e2e-launcher | AQ-4（fixture 缺失终止） | 改 `e2e-tests/run-wdio.cjs`；就近同步 `e2e-tests/CLAUDE.md`（fixture 缺失行为描述） |
| literal-guard | CS-1（模板字符串拼接检测）、CS-2（扫描范围扩八路径） | 改 `src/__tests__/no-claude-literals.test.ts`；就近同步 `src/__tests__/CLAUDE.md`（守卫描述）、`.claude/test-inventory.md`（**单点负责**：守卫条目 + 代 frontend-strategy 同步 cli-profile-claude 用例数 +1） |

> backend-guard 与 backend-config 都就近同步 `src-tauri/src/hooks/CLAUDE.md`——**文件重叠**：backend-guard 只动「信号文件管道」相关段落，backend-config 只动「write 语义」段落？不行，同文件并发改仍有冲突风险。裁决：hooks/CLAUDE.md 的就近同步**归 backend-config 单点负责**（汇总 AQ-2 大小限制一句 + ZQ-5 write 语义），backend-guard 只改 signal.rs/ops.rs + agent_history/CLAUDE.md，不动 hooks/CLAUDE.md。
>
> 同理裁决（脚本生成期补查）：`.claude/test-inventory.md` 在 frontend-strategy / literal-guard 间重叠——**归 literal-guard 单点负责**（含代同步 AQ-1 用例数 +1），frontend-strategy 不动。

**实现要点**：

1. **AQ-2**：`process_signal_file_with` 读取前 `path.metadata()` 判定，超 `MAX_SIGNAL_FILE_BYTES = 1024 * 1024`（1MB）→ `tracing::warn!` + 删除文件返回（与「解析失败仍删」容错语义一致）；metadata 失败（如文件已消失）走既有读失败分支。L1 新用例：构造 >1MB 信号文件 → emit 闭包零调用 + 文件已删。
2. **AQ-3**：三处补符号链接拒绝——`locate_session_jsonl` 的 `dir_path.is_dir()` 后补 `!dir_path.is_symlink()`（:50）、`candidate.is_file()` 后补 `!candidate.is_symlink()`（:54）、`delete_session` 同名目录 `session_dir.is_dir()` 后补 `!session_dir.is_symlink()`（:79）。L1 新用例：symlink 形态注入（`std::os::windows::fs::symlink_dir`/`symlink_file`；创建失败如权限不足则测试内跳过并注释说明——CI runner 开发者模式/权限差异）。**SEC-05 语义不变**：校验与定位流程不动，仅加拒绝分支。
3. **ZQ-5**：`config_write_sync` 入口校验改为「`hooks.is_null()` → 替换为空对象 `{}`；非 null 且非 object → Validation」；`write_hooks_subtree` 的 `is_object` 闸门保持不变（收到的恒为 object）。L1 新用例：`hooks=Value::Null` 写入 → 文件 hooks 键 = `{}`（merge 保留其他字段）。语义 = 清空该层 hooks。
4. **AQ-1**：`buildResumeCommand` 的 cwd 单引号转义 `session.cwd.replace(/'/g, "''")`（PowerShell 单引号字符串内 `''` = 字面单引号）；:106-107 注释改为「单引号按 PowerShell 规则转义为 `''`（AQ-1 修复）」。`cli-profile-claude.test.ts` 的「逐字一致」断言更新 + 新增 `C:\Bob's Project` 形态用例。**buildRestoreInput 不动**（注入 PTY 的命令不经 PowerShell 解析单引号——PTY 直接写 stdin，shell 自己解析；现状行为不变）。🚨 执行期核实：`buildRestoreInput` 不含 cwd（现状仅 `claude --resume <id>`），恢复编排的 cwd 由 addPanel params.cwd 承担——无注入面。
5. **AQ-4**：else 分支（:148-152）改为 `console.error` 明确文案（含「fixtures/claude-projects 缺失，E2E 终止——防止回落真实 ~/.claude/projects」语义）+ `process.exit(1)`；位置在 wdio 启动前（现状即是前置检查段）。
6. **CS-1**：词法器模板字符串分支——含 `${}` 时提取各字面量片段（表达式外的静态文本）**拼接成单值**后 push（走既有三类判定）；新增自检用例：构造含 `` `cl${''}aude` `` 形态的样例源码字符串，断言 extractStringLiterals/扫描判定报违规。
7. **CS-2**：`SCAN_DIRS` 追加 `src/features/cliProfiles`；豁免实现 = 扫描循环按相对路径前缀跳过 `src/features/cliProfiles/profiles/claude/`（normalize 正斜杠后比较）；完整性断言同步八路径；头部注释「七路径」→「八路径」与豁免规则说明随行。豁免路径拼写错会静默空扫——补断言：`profiles/claude` 目录存在（Glob/fs existsSync）且被豁免（该目录文件不参与违规收集）。

**验证项**（详表落盘 `verify/stage-01.md`）：

1. AQ-1：strategies.ts 存在单引号转义（语义式——Read 确认 cwd 中 `'` 被替换为 `''` 的转义逻辑）；「原样保留/未转义」自述注释零残留；L2 单引号回归用例存在且 npm test 全绿
2. AQ-2：signal.rs 存在文件大小上限常量与超限拒绝路径（语义式——Read 确认读取前有大小判定、超限 warn + 删除 + 不 emit）；L1 超限用例存在且 cargo test 全绿
3. AQ-3：ops.rs 定位与删除链三处符号链接拒绝（语义式——Read 确认一级子目录/命中文件/同名目录三处均不跟随符号链接）；L1 符号链接用例存在（含权限不足跳过说明）
4. AQ-4：run-wdio.cjs fixture 缺失分支为非零退出（Read 确认 `process.exit(1)` 或等价 throw 在 wdio 启动前）；`node --check` 通过；L4 全绿（fixture 存在正常路径回归）
5. ZQ-5：config.rs 写路径 null → {}（语义式——Read 确认 null 入参不再 Validation 而是视作空对象 merge）；L1 null 用例存在
6. CS-1：词法器含表达式模板字符串的拼接检测（语义式）；自检用例存在（`cl${''}aude` 必报违规）
7. CS-2：SCAN_DIRS 含 `src/features/cliProfiles` 且 `profiles/claude/` 目录级豁免存在；豁免目录存在性断言存在；守卫全绿（八路径扫描零违规——含 cliProfiles 根目录现状零命中间接证实）
8. 全量门禁绿（含本 Stage L4）

**commit message**：`fix(security): 安全族修复 + AC-5 守卫强化（AQ-1~4/ZQ-5/CS-1/2）`

**人工验证点**：① fixture 缺失终止实测——`fixtures/claude-projects` 临时改名 → `node e2e-tests/run-wdio.cjs` 非零退出且文案明确 → 恢复（无法自动化：修复失效时会继续启动 wdio，自动化验证有触碰真实 home 风险）；② 含单引号 cwd 的恢复命令粘贴 PowerShell 实测可执行（L2 已断言输出形态，人工抽验）。

---

## Stage 02 — 前端正确性族

**条目**：ZQ-1、ZQ-2、ZQ-3、ZQ-4、ZQ-6、ZQ-7（6 条）

**agent 分工表**（3 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| composite-key | ZQ-1、ZQ-7（keyOf 单点 + 五处统一） | 改 `src/features/agentHistory/historyModel.ts`（keyOf 导出 + deriveActiveSessionStatuses 改用）、`src/features/agentHistory/HistorySessionList.tsx`（rowFlags:278 + findPanelForSession:196/206 改用）、`src/features/agentStatus/AgentStatusView.tsx`（titleBySessionId:133 + displayRows:144 改用）；测试 `src/__tests__/agent-history-model.test.ts`（keyOf 新用例）、`src/__tests__/agent-history-view.test.tsx`（随行）；就近同步 `src/features/agentHistory/CLAUDE.md`（复合键段）、`.claude/test-inventory.md`（**单点负责**：本 Stage 全部新增用例登记——含代 event-pipeline 登记空串回退 ×3/null 建行/Exit 清图标、代 restore-id 登记同毫秒相异） |
| event-pipeline | ZQ-2（resolvePayloadCliId 单点）、ZQ-3（null 建行 status null）、ZQ-6（EXIT_EVENT 清图标） | 新建 `src/panels/terminal/resolvePayloadCliId.ts`；改 `src/panels/terminal/useXterm.ts`（:358-361 三级解析 → helper + :390-392 清图标条件）、`src/features/agentStatus/useAgentStatus.ts`（:134-137 → helper + :187-199 建行 status null）、`src/features/notifications/useAgentNotifications.ts`（:65-68 → helper）；测试 `src/__tests__/agent-status-hook.test.ts`、`src/__tests__/use-xterm-lifecycle.test.ts`、`src/__tests__/notifications.test.ts`（各加空串回退/新语义用例）；就近同步 `src/features/agentStatus/CLAUDE.md`、`src/features/notifications/CLAUDE.md`、`src/panels/CLAUDE.md`（useXterm 行）——**不动 test-inventory / agentHistory/CLAUDE.md**（归 composite-key） |
| restore-id | ZQ-4（panelId 防碰撞） | 改 `src/features/agentHistory/restoreSession.ts`（:133 加模块级自增序号）；测试 `src/__tests__/agent-history-restore.test.ts`（同毫秒两次恢复 panelId 相异用例）——**不动 test-inventory / agentHistory/CLAUDE.md**（归 composite-key） |

> 裁决（脚本生成期补查）：`.claude/test-inventory.md` 三 agent 均需登记新用例——**归 composite-key 单点负责**（prompt 内写明代登记项），event-pipeline / restore-id 不动；`agentHistory/CLAUDE.md` 归 composite-key（restoreSession 属内部实现细节，文档零改动则无需碰）。

**实现要点**：

1. **keyOf 落点与形态**：`historyModel.ts` 导出（契约 3）。`HistorySessionList.tsx:278` 的 `activeStatuses.get(...)` 键与 :196/:206 的 findPanelForSession 比较键统一经 keyOf——入参 cliId 为 null/undefined 时回退在 keyOf 内部完成，消费方不再各自 `?? CLAUDE_CLI_ID`。**转义对既有键的影响**：现状 cliId/sessionId 均不含 `|`（UUID + profile id），转义对存量键零变化（纯防御未来）。
2. **resolvePayloadCliId 落点**：新建 `src/panels/terminal/resolvePayloadCliId.ts`（契约 4）。三处 `??` 链改 `||` 链 + trim——useAgentNotifications.ts 的 `classifyEvent` 是纯函数导出，helper import 无循环（TerminalRegistry 不 import notifications）。
3. **ZQ-3 形态**：建行 `status: newStatus ?? "attention"` → `status: newStatus`（AgentSessionRow.status 类型随行放宽含 null）；更新已有行 :177 的 null 不覆盖逻辑**不动**；AgentStatusRow 渲染 null = 无图标（现状「icon 为空仍渲染空列占位」已兼容，核实不撒谎）；deriveActiveSessionStatuses「status null 不产出键」语义不变（活跃区 null 行 ↔ 历史区无标记，语义一致）。注释注明决策：「null 映射事件建行但 status null——感知存活（SessionStart 丢失场景）且不误标 attention（ZQ-3 决策 2）」。
4. **ZQ-6**：:391 条件改 `payload.event === SESSION_END_EVENT || payload.event === EXIT_EVENT`——与 :376 删 agentSession 的双事件判定对齐。
5. **ZQ-4**：模块级 `let restoreSeq = 0`，panelId = `` `terminal-${targetPageId}-${Date.now()}-${++restoreSeq}` ``；测试 mock Date.now 同值两次调用断言 panelId 相异。
6. **L3 复刻段核对**：本 Stage 改动段（agent-event 订阅、建行逻辑）不在 `production-osc.test.ts` 复刻范围（OSC 52/133/8）——执行 agent 确认零波及，verify 语义核对。

**验证项**（详表落盘 `verify/stage-02.md`）：

1. keyOf 单点存在且五处调用点全部经 keyOf（语义式——Read 确认 historyModel/HistorySessionList/AgentStatusView 不再存在 `${...}|${...}` 裸拼接；不限变量名）；keyOf 内部含回退 + 转义；L2 新用例（含 `|` 转义一致性、cliId 缺省回退）存在且全绿
2. resolvePayloadCliId 单点存在且三处消费方改经 helper（语义式——三文件不再存在 `payload.cliId ??` 链）；空串/空白 cliId 回退用例 ×3 存在且全绿
3. ZQ-3：useAgentStatus 建行路径不存在 `?? "attention"` 兜底（语义式）；「null 映射事件首达 → 建行 status null 无图标」L2 用例存在且全绿
4. ZQ-6：清图标条件含 EXIT_EVENT（Read 确认）；Exit 事件清图标 L2 用例存在
5. ZQ-4：panelId 含自增序号（语义式）；同毫秒相异用例存在
6. 全量门禁绿（静态 + L1/L2/L3，无 L4）

**commit message**：`fix(agent-status): 前端正确性族——keyOf/resolvePayloadCliId 单点 + null 建行语义（ZQ-1~4/6/7）`

**人工验证点**：null 映射事件（如普通 Notification）首达时侧栏出现**无图标行**的视觉确认（SessionStart 丢失场景模拟——无法自动化验证视觉效果）。

---

## Stage 03 — transcript 概念中性化全链路

**条目**：KZ-2、KZ-3（2 条）

> **划分豁免**（每 Stage 3-15 项规则的例外）：本 Stage 仅 2 项——单一跨前后端契约链（事件 DTO + trait + 命令 + reporter + 全消费方更名）强耦合不可拆，符合「强耦合大项（跨前后端的单一任务）可 2-3 项独立成 Stage」。

**agent 分工表**（3 并行，文件无重叠；契约 1 写死于脚本头部）：

| label | 负责项 | 文件 |
|-------|--------|------|
| backend | KZ-3 trait/命令更名 + KZ-2 后端 DTO/reporter | 改 `src-tauri/src/hooks/signal.rs`（DTO + 11 处测试 JSON）、`src-tauri/src/hooks/provider.rs:33`、`src-tauri/src/hooks/mod.rs`（:162/165/225/227 + 测试 :342/364/431/439/523）、`src-tauri/src/hooks/claude/mod.rs:128-129`、`src-tauri/src/hooks/claude/usage.rs:344/346/362`、`src-tauri/src/hooks/claude/slterm-hook-reporter.js:50`（+SCRIPT_VERSION 3）、`src-tauri/src/hooks/claude/inject.rs:389`（版本断言 2→3 随行）；就近同步 `src-tauri/src/hooks/CLAUDE.md`（:36/158/160/198/243/316）——**不动 test-inventory**（L1 测试名更名归 frontend-consumers 代同步） |
| frontend-contract | KZ-2 前端 DTO + KZ-3 wrapper | 改 `src/types/agent.ts:26`、`src/ipc/agentHooks.ts:42/44`、`src/__tests__/ipc-agent-hooks-contract.test.ts`（:158-165/226/273/288）；就近同步 `src/ipc/CLAUDE.md:26`、`src/types/CLAUDE.md`（agent.ts 行） |
| frontend-consumers | KZ-2 前端内部状态 + 全消费方 + L3/E2E 随行 | 改 `src/panels/terminal/TerminalRegistry.ts:16/95`、`src/panels/terminal/useXterm.ts:384`、`src/features/agentStatus/useAgentStatus.ts`（:43/179-180/196/207-208/264/272/340/346）、`src/features/agentHistory/historyModel.ts:121-135`、`src/features/agentHistory/HistorySessionList.tsx:201-203`；测试 `src/__tests__/agent-status-hook.test.ts`、`terminal-registry.test.ts`、`mock-cli-profile.test.tsx:283/508/511`、`notifications.test.ts:132`、`cli-profile-claude.test.ts:353`、`agent-history-hook.test.tsx:130`、`agent-history-model.test.ts:251/303/319/326`；L3 `test/terminal/production-osc.test.ts:106`；E2E `e2e-tests/agent.e2e.ts`、`hooks.e2e.ts:107/126`（:170 stdin snake_case 不动）、`history.e2e.ts:488/497`；就近同步 `src/features/agentStatus/CLAUDE.md:27`、`src/features/agentHistory/CLAUDE.md:43`、`src/panels/CLAUDE.md:277`、`CONTEXT.md:229`、`.claude/test-inventory.md:282`（**单点负责**：282 行更名 + 代 backend 同步 L1 测试名更名 `context_usage_passes_transcript_path` → `_passes_usage_source_path`） |

> 裁决（脚本生成期补查）：`.claude/test-inventory.md` 在 backend（L1 测试名更名）与 frontend-consumers（:282 更名）间重叠——**归 frontend-consumers 单点负责**，backend 不动。

**实现要点**：

1. **机械更名、逐文件核对**：上表消费方全量清单（grep 实查）逐文件打勾；完成后全仓 grep `transcriptPath|transcript_path` 残留仅豁免形态（claude hook stdin 协议、claude provider 内部语义名词、文档迁移溯源）。
2. **后端 DTO**：`usage_source_path: Option<String>` + `#[serde(default)]`——serde 键集合测试（mod.rs:364 区域）同步为 9 键含 `usageSourcePath`；「无 cliId 旧信号兼容」用例形态扩展「无 usageSourcePath 信号 → None」。
3. **reporter**：payload 键更名 + `|| null`（Option 对应）；SCRIPT_VERSION 2→3；模板内嵌校验测试（inject.rs）断言新版本。
4. **前端内部状态**：`AgentSessionInfo.transcriptPath` → `usageSourcePath`（TerminalRegistry merge 语义注释随行）；basename 回退（historyModel:133-135、HistorySessionList:201-203）逻辑不变仅字段更名。
5. **E2E 信号构造**：camelCase `transcriptPath` → `usageSourcePath`（agent.e2e.ts 12 处、hooks.e2e.ts:107/126、history.e2e.ts:497）；`transcript_path`（hooks.e2e.ts:170 stdin 模拟）**不动**。
6. **L1 命令层用例**：usage.rs:346 `context_usage_passes_transcript_path` 测试名随行更名（如 `_passes_usage_source_path`）——测试名更名同步 test-inventory 就近核对。

**验证项**（详表落盘 `verify/stage-03.md`）：

1. 全仓 grep `transcriptPath` 零残留（豁免：`docs/` 历史文档、CHANGELOG 类溯源注释）——语义式：豁免项须 Read 确认属「claude hook stdin 协议模拟 / claude provider 内部语义 / 迁移溯源」三类之一
2. `transcript_path` 残留仅于：hooks.e2e.ts:170（stdin 模拟）、claude provider 内部（scan_transcript_usage 函数名等 claude 领地）、reporter 的 `data.transcript_path` 读取——逐处 Read 确认
3. 后端 DTO `usage_source_path: Option<String>` + serde default；serde 键集合测试含 `usageSourcePath` 键；L1 全绿
4. trait 参数 `usage_source_path` + 命令参数 camelCase `usageSourcePath`；契约测试 expectExactKeys = `["cliId", "usageSourcePath"]`；L2 全绿
5. reporter payload 键 `usageSourcePath` + SCRIPT_VERSION=3；inject.rs 模板内嵌校验断言新版本
6. E2E 三 spec camelCase 键更名完成；L4 全绿（本 Stage 门禁含 L4）
7. 全量门禁绿

**commit message**：`refactor(hooks): transcript 概念中性化——usageSourcePath 全链路更名（KZ-2/KZ-3）`

**人工验证点**：已注入用户升级路径复测——旧版本注入状态显示「版本过旧」（SCRIPT_VERSION 3 检测）→ 重新注入恢复 → 信号文件驱动用量条更新（决策 7 先例路径）。

---

## Stage 04 — hub 编辑器分派 + 配置层抽象

**条目**：KZ-1、KZ-4、KZ-5（消解验证，3 条）

**agent 分工表**（pipeline 串行 2 agent——`types.ts` / `profiles/claude/index.ts` / 测试文件共享，前序产出供后序使用；契约 2 写死于脚本头部）：

| label | 负责项 | 文件 |
|-------|--------|------|
| editor-dispatch | KZ-1（configEditor 入 profile + hub 分派） | 改 `src/features/cliProfiles/types.ts`（HooksConfigEditorProps + configEditor 字段）、`src/features/cliProfiles/profiles/claude/index.ts`（挂载 configEditor）、`src/panels/hooksConfig/HooksConfigPanel.tsx`（:32 import 移除 + :253-260 分派渲染 + 缺失防御空态）；测试 `src/__tests__/cli-profile-claude.test.ts`（configEditor 挂载断言）、`src/__tests__/hooks-config-panel.test.tsx`（hub 分派用例）；就近同步 `src/features/cliProfiles/CLAUDE.md`（含 features→panels 依赖方向合法化说明）、`src/panels/CLAUDE.md`（hub 段 + 文件表） |
| layers | KZ-4（configLayers 入 profile + HooksLayer 泛化） | 改 `src/features/cliProfiles/types.ts`（configLayers 字段）、`src/types/hooksConfig.ts:8`（HooksLayer = string）、`src/features/cliProfiles/profiles/claude/index.ts`（configLayers 三层值）、`src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx`（LAYERS 常量退役 → 按 profile.configLayers 渲染 + PRIORITY_HINT 处置）、`src/panels/hooksConfig/useHooksConfig.ts`（layer 状态类型 + 初始层随行）；测试 `src/__tests__/cli-profile-claude.test.ts`（configLayers 断言）、`src/__tests__/hooks-config-panel.test.tsx`（层渲染用例随行）、`src/__tests__/hooks-config-sync.test.tsx`（若涉层类型则随行）；就近同步 `src/types/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/features/hooksConfig/CLAUDE.md`、`.claude/test-inventory.md`（**单点负责**——串行后序拿最终态，editor-dispatch 不动） |

> 裁决（脚本生成期补查）：`.claude/test-inventory.md` 归串行后序 layers 单点负责（editor-dispatch 的用例变化由 layers 在最终态一并登记）。

**实现要点**：

1. **KZ-1 分派形态**：hub 编辑器槽 `const Editor = selectedProfile?.capabilities.hooks?.configEditor`——`Editor ? <Editor key={selectedProfile.id} profile={selectedProfile} onDirtyChange={handleDirtyChange} askGuardRef={askGuardRef} /> : 空态占位`；`import ClaudeHooksConfigEditor` 从 HooksConfigPanel 移除（唯一直接引用消亡）；选择行过滤条件不变。
2. **依赖方向合法化**：`profiles/claude/index.ts` import `src/panels/hooksConfig/ClaudeHooksConfigEditor`——features/cliProfiles → panels/hooksConfig 新方向，理由写入 cliProfiles/CLAUDE.md（claude 合法领地引用 claude 专属资产；types.ts 仅类型 import 不构成运行循环）。🚨 循环依赖核查：`ClaudeHooksConfigEditor.tsx` import `features/cliProfiles/types`（CodingCliProfile——类型 import，运行期擦除）；`profiles/claude/index.ts` import 编辑器组件（运行期）——`profiles/index.ts` ← `Workspace.tsx` 注册链不 import hooksConfig 面板模块之外的循环；执行 agent 须 `npx tsc --noEmit` + vite build 验证无循环报错。
3. **KZ-4 层驱动**：编辑器层切换器数据源 = `profile.capabilities.hooks.configLayers`（LAYERS 常量退役）；useHooksConfig 的 layer state 初始值 = `configLayers?.[0]?.id ?? "user"`（防御缺省）；PRIORITY_HINT 与禁用逻辑（rootPath 空时 project/local 禁用）——禁用判定按 layer id 是否 user 之外？claude 语义「project/local 需 rootPath」是 claude 知识——保留在 claude 编辑器内部（合法领地），执行期定形态（如 configLayers 项加 `requiresProject?: boolean` 或编辑器内部判定），verify 语义式断言「层列表数据源来自 profile 而非模块级常量」。
4. **KZ-5 消解**：不改 ClaudeHooksConfigEditor 内部文案——分派落地后该文件整文件属 claude 专属编辑器（MC-223 合法领地）；verify 断言 hub 不存在对 ClaudeHooksConfigEditor 的无条件引用即闭环。
5. **mock 夹具波及提示**：`mockCliProfile.ts` / `e2e-tests/helpers.ts` 的 mockcli profile 声明在 Stage 05 才补 configEditor/configLayers——本 Stage 不动；tsc 对缺字段不报错（字段可选）✓ 中间态合法。

**验证项**（详表落盘 `verify/stage-04.md`）：

1. HooksConfigPanel.tsx 不存在对 ClaudeHooksConfigEditor 的 import/无条件渲染（语义式——编辑器槽渲染来源须为 profile 的 configEditor 字段）；hasConfigEditor=true 但 configEditor 缺失 → 空态占位用例存在
2. claude profile 声明 configEditor = ClaudeHooksConfigEditor + configLayers = 三层现值（cli-profile-claude.test.ts 断言存在）
3. 编辑器层切换器数据源 = profile.configLayers（语义式——ClaudeHooksConfigEditor 内不存在模块级 LAYERS 常量硬编码三层）；HooksLayer = string 泛化（types/hooksConfig.ts）
4. tsc/vite build 无循环依赖报错；L2 全绿（含 hub 分派用例）
5. KZ-5 消解：ClaudeHooksConfigEditor 的 `~/.claude/settings.json` 文案保留但整文件仅经 profile.configEditor 引用（grep HooksConfigPanel 零直接引用）
6. 全量门禁绿（静态 + L1/L2/L3，无 L4）

**commit message**：`refactor(cli-profiles): hub 编辑器分派 + 配置层抽象入 profile（KZ-1/KZ-4/KZ-5）`

**人工验证点**：hub 面板实测——claude 编辑器经分派正常渲染（层切换/GUI/JSON/保存/注入按钮/重启提示全链）；切换 CLI 卸载重挂载 + dirty 守卫不回归。

---

## Stage 05 — mockcli 验收强化

**条目**：KZ-7、CS-3（2 条）

> **划分豁免**：本 Stage 仅 2 项——均为 mockcli 验收强化且依赖 Stage 04 产物（configEditor 分派 + configLayers 声明），独立成 Stage 承接前后依赖。

**agent 分工表**（2 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| l2-mock-editor | KZ-7（mock 编辑器桩 + AC-4④ 双向分派断言） | 改 `src/__tests__/helpers/mockCliProfile.ts`（configEditor 桩组件 + configLayers 桩声明）、`src/__tests__/mock-cli-profile.test.tsx`（AC-4④ 段重写）；就近同步 `src/__tests__/CLAUDE.md`——**不动 test-inventory**（归 l4-mockcli 代登记） |
| l4-mockcli | CS-3（L4 两条新用例 + E2E 夹具补桩） | 改 `e2e-tests/helpers.ts`（installMockCliProfile 补 configEditor 桩 + configLayers）、`e2e-tests/mockcli.e2e.ts`（新增 agent-event 注入用例 + hub 分派/保存 cliId 透传用例）；就近同步 `e2e-tests/CLAUDE.md`、`.claude/test-inventory.md`（**单点负责**：L4 不可行豁免登记（历史条目/恢复注入两条）+ 新 L4 用例 + 代 l2-mock-editor 登记 AC-4④ 重写后 L2 用例数变化） |

> 裁决（脚本生成期补查）：`.claude/test-inventory.md` 在两 agent 间重叠——**归 l4-mockcli 单点负责**，l2-mock-editor 不动。

**实现要点**：

1. **KZ-7 桩组件**：mockCliProfile.capabilities.hooks.configEditor = 桩组件（渲染可识别标记 `data-e2e="mockcli-config-editor"`，props 签名 = HooksConfigEditorProps）；AC-4④ 重写为**双向分派断言**：选中 mockcli → 桩渲染标记存在 + `mockJsonMode`（claude 编辑器内部 JsonMode mock）零调用；选中 claude → JsonMode 被调用 + 桩标记不存在。
2. **CS-3 用例 ①**（agent-event 注入）：E2E helper 注册 mockcli → 终端面板 → 原子写信号文件（cliId="mockcli"、事件经桩映射 working）→ 断言页签 ⚡ + 活跃区建行（真实 watcher → agent-event → 三级解析 → 桩策略全链）。
3. **CS-3 用例 ②**（hub 分派 + 保存透传）：打开 hooksConfig 面板 → 选择行渲染 mockcli 按钮 → 点击 → 断言 mock 编辑器桩渲染（`data-e2e="mockcli-config-editor"`，helpers.ts 桩组件同标记）→ 桩内保存动作触发 `writeHooksConfig("mockcli", ...)` → 断言后端「未知 cliId: mockcli」错误透传展示（证明 cliId 全链携带；mockcli 无后端 provider，错误即透传证据）。
4. **helpers.ts 桩**：installMockCliProfile 的 mockcli 定义补 configEditor（React.createElement 桩——helpers.ts 为 .ts 无 JSX）+ configLayers 桩；E2E_ENABLED 门控与 tree-shake 形态不动（禁区 6）。
5. **L4 豁免登记**：review CS-3 建议的「历史条目展示」「双击恢复注入」两条 L4 不可行（生产二进制无 mockcli 后端 provider，历史条目由 claude provider 打标产出 cliId 恒 "claude"；为测试留生产后门代价过大）——豁免理由记入 test-inventory 豁免清单，L2 AC-4③/⑤ 为兜底层级。

**验证项**（详表落盘 `verify/stage-05.md`）：

1. mockCliProfile 声明 configEditor 桩 + configLayers；AC-4④ 双向分派断言存在（mockcli → 桩渲染 + JsonMode 零调用；claude → JsonMode 调用）且 npm test 全绿
2. helpers.ts mockcli 定义补 configEditor/configLayers（E2E_ENABLED 内联形态不动——grep `import.meta.env` 字面量形态确认）
3. mockcli.e2e.ts 新增两用例存在（信号注入 emoji/建行 + hub 分派/保存 cliId 透传）；L4 全绿
4. test-inventory 登记新用例 + L4 豁免两条（历史条目/恢复注入，含理由与兜底层级）
5. 全量门禁绿（含本 Stage L4）

**commit message**：`test(cli-profiles): mockcli 编辑器分派双向断言 + L4 关键路径补全（KZ-7/CS-3）`

**人工验证点**：无（L2/L4 自动覆盖）。

---

## Stage 06 — 文档同步终验

**条目**：YS-1、YS-2、YS-3、YS-4、YS-5、WD-1、WD-2、WD-3、WD-4、KZ-6（10 条）

**agent 分工表**（4 并行，文件无重叠）：

| label | 负责项 | 文件 |
|-------|--------|------|
| panels-doc | YS-1、WD-2 | 改 `src/panels/CLAUDE.md`（:249 退役 API 描述 + :152 补全字样）、`src/features/hooksConfig/CLAUDE.md:21`、`src/features/hooksConfig/schema/index.ts:15` |
| registry-doc | YS-3、KZ-6 | 改 `src/features/sideViews/CLAUDE.md:5/28/46/125`、`src/theme/CLAUDE.md:19`、`src/theme/schemeRegistry.ts:4-5`、`src/features/cliProfiles/CLAUDE.md`（:17/:29 先例 + :88-90 新增 CLI 步骤补四步） |
| code-comments | YS-2、YS-4、YS-5、WD-4 | 改 `src/__tests__/terminal.test.tsx:292`、`src/__tests__/use-xterm-lifecycle.test.ts:1363`、`src/theme/schemes/types.ts:46`、`src/features/agentStatus/AgentStatusRow.tsx:55`、`src/features/agentHistory/AgentHistorySections.tsx:49` |
| root-doc | WD-1、WD-3 + 终态核对 | 改 `.claude/CLAUDE.md`（F9 登记）、`CONTEXT.md:75-76`；终态核对：全仓 grep 退役 API/旧命名零残留兜底（TabTitleRegistry 先例残留/CliIconRegistry/claudeHistory/transcriptPath——逐处甄别豁免形态）、`.claude/test-inventory.md` 用例数终态对齐（L1/L2/L3/L4 实跑计数回写） |

**实现要点**：

1. **YS-1**：:249 改与 :275 一致描述（`cliProfileRegistry.matchByCommand` 命中 → `logo = profile.iconSrc`、`icon = STATUS_EMOJI.attention`）——同一文件矛盾消除；改前 Read :275 现文照齐。
2. **YS-3 逐处甄别**：7 处先例引用改指现存注册表（CliProfileRegistry/ShortcutRegistry 就近取语义近者）或删先例只留自身机制描述；**合法迁移溯源两处保留**（panels/CLAUDE.md:275、cliProfiles/CLAUDE.md:29 后半「两份拷贝收敛于此」）——勿误删。
3. **KZ-6 四步补全**（按 Stage 03/04 终态撰写）：后端 hooks provider 注册（`hooks/provider.rs` REGISTRY）、后端 history provider 注册（`agent_history/provider.rs` REGISTRY）、test-inventory 用例数同步、`hasConfigEditor=true` 时新增编辑器组件 + profile 挂载 `configEditor`/`configLayers`。
4. **WD-2 三处统一**：「补全/悬停/波浪线」→「悬停/波浪线」（JsonMode.tsx:6-8 注释为准——无自动补全是 2026-08-01 验收后决策）。
5. **终态核对**（root-doc）：grep 全仓零残留断言 + test-inventory 实跑计数回写（本 Stage 全量复跑产出）。
6. **文档描述对照代码核实**：每条修改 Read 对应代码确认不撒谎（如 YS-1 改后与 useCommandDetection.ts 现行实现一致）。

**验证项**（详表落盘 `verify/stage-06.md`）：

1. YS-1：panels/CLAUDE.md 无 `cliIconRegistry.match` 描述（:249 与 :275 一致——Read 对照）；全仓 grep `CliIconRegistry` 仅豁免形态（迁移溯源）
2. YS-2/4/5、WD-4：四处代码注释更新（Read 逐处确认新表述与现行代码一致）
3. YS-3：7 处 TabTitleRegistry 先例引用更新 + 2 处合法溯源保留（Read 逐处）；全仓 grep `TabTitleRegistry` 仅溯源形态
4. WD-1：根 CLAUDE.md 需求编号索引含 F9 行；WD-3：CONTEXT.md 后端注册表描述与 provider.rs 实际一致（Read 对照代码）
5. WD-2：三处「补全」字样移除（grep `补全` 于三文件零命中或仅剩「无自动补全」否定表述）
6. KZ-6：cliProfiles/CLAUDE.md 新增 CLI 步骤含后端双注册 + test-inventory + configEditor/configLayers 四要素（Read 确认与 Stage 03/04 终态一致）
7. test-inventory 用例数 = 实跑计数（本 Stage 全量复跑回写）
8. **收尾全量门禁绿（含 L4 + L1）——先关闭运行中的 slterminal.exe**

**commit message**：`docs(review-fix): 文档一致性修复 + 终态核对（YS-1~5/WD-1~4/KZ-6）`

**人工验证点**：终验人工走查——真实 claude 全功能回归（照 multi-cli Stage 08 先例：页签四态/用量条/hooks 注入三态/历史区/恢复编排/hub 面板）；汇总前序 Stage 人工验证点统一确认。

---

## 人工验证点汇总（收尾实测项）

| Stage | 验证点 |
|-------|--------|
| 01 | fixture 缺失终止实测（rename → 非零退出 → 恢复）；含单引号 cwd 恢复命令粘贴实测 |
| 02 | null 映射事件首达的无图标行视觉确认（SessionStart 丢失模拟） |
| 03 | 「版本过旧」→ 重新注入 → 用量条链路实测（SCRIPT_VERSION 3 决策 7 路径） |
| 04 | hub 面板 claude 编辑器分派渲染全链实测 |
| 05 | 无（自动化覆盖） |
| 06 | 终验人工走查：真实 claude 全功能回归 + 前序验证点统一确认 |
