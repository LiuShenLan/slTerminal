# Phase 2 Fix — Stage 划分

> 清单：`docs/hooks-dev/phase2-fix/checklist.md`（22 项 PF2-）。
> **跨边界契约不在本文档复制取值**——唯一真值源 = checklist.md「跨边界契约」段（契约 1 claudeSession / 契约 2 match 首 token / 契约 3 ContextUsage 四字段 / 契约 4 sendToastNotification / 契约 5 行生命周期）。各脚本 PREAMBLE 引用契约编号。
> 执行产物：`docs/hooks-dev/phase2-fix/workflows/stage-01..05.js` + `workflows/verify/stage-01..05.md` + `workflows/fix-loop.js`。

## Stage 01 — F5 行建模重设计

**改动项**：PF2-FE-01、PF2-FE-02、PF2-FE-03、PF2-FE-04、PF2-FE-05、PF2-FE-06、PF2-FE-07、PF2-TE-01、PF2-TE-02（行建模适配部分）、PF2-TE-03、PF2-TE-05、PF2-TE-08

### agent 分工表（pipeline：A → B ∥ C → D）

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A: registry 契约 | PF2-FE-01、PF2-FE-02 | `src/panels/terminal/TerminalRegistry.ts`、`src/panels/terminal/TabTitleRegistry.ts`、`src/panels/terminal/tabRules.ts`（注释同步） |
| B: 终端侧写 session | PF2-FE-03、PF2-FE-04 | `src/panels/terminal/useCommandDetection.ts`、`src/panels/terminal/useXterm.ts` |
| C: 视图侧行建模 | PF2-FE-05、PF2-FE-06、PF2-FE-07 | `src/features/agentStatus/useAgentStatus.ts`、`src/features/agentStatus/AgentStatusView.tsx`（预期零改动——核对确认） |
| D: L2 测试重写 | PF2-TE-01、PF2-TE-02（行建模适配）、PF2-TE-03、PF2-TE-05、PF2-TE-08 | `src/__tests__/agent-status-hook.test.ts`、`src/__tests__/agent-status-view.test.tsx`、`src/__tests__/tab-rules.test.ts`、`src/__tests__/tab-title-registry.test.ts`、`src/__tests__/terminal-registry.test.ts`、`src/__tests__/terminal-registry-subscribe.test.ts`、`src/__tests__/use-xterm-lifecycle.test.ts` |

**无重叠证明**：A/B/C/D 四组文件两两不相交（上表逐文件枚举 = 各 agent prompt 触碰文件全集，含 tabRules.ts 注释微改）。D 虽与 A/B/C 改同一批**被测对象**，但 D 只碰 `src/__tests__/` 文件，且串行在 A/B/C 完成后执行。

### 实现要点

- **A**：按契约 1/2。`ClaudeSessionInfo` 二态模型（存在即运行中）；`setClaudeSession` merge 语义 + `sessionChange` 裸 panelId 事件；`register` 幂等覆盖保留旧 session；`match` 内部取首 token（调用方不改）。既有 stub 工厂编译不炸是可选字段设计的验收点（`terminal-registry.test.ts:13-27` 四字段字面量）。
- **B**：依赖 A 产出的 `setClaudeSession`。`useCommandDetection` 签名加 `panelId`（调用点 `useXterm.ts:205` 传现成值）；OSC 133 C 命中写 `{ matchedCommand: rule.command }`（无 transcriptPath——feature-plan 边界 5 未注入 hooks 会话语义）；OSC 133 D（isCommandRunning）写 null。useXterm hook 订阅（:349-357）追加：非 SessionEnd/Exit → merge `{ transcriptPath: payload.transcriptPath ?? undefined }`；SessionEnd/Exit → null。与页签 emoji 逻辑正交，不动 eventToStatus/onTabStateChange 现有链路。
- **C**：依赖 A 的 `claudeSession` 字段 + `sessionChange` 事件。按契约 5 重设计行生命周期；**#5 竞态双保险**——ref 稳定订阅（照 rowsRef :69-70 模式，effect deps `[]`）+ reconcile 对账；初始扫描只建活会话行 + 携 transcriptPath 主动 `contextUsage` 拉一次（问题 2b）；`AgentSessionRow.usage` 类型改引用 `ContextUsage`（此时仍 2 字段，Stage 03 扩 4 字段）；:169-171 静默 catch 补 `console.error`。
- **D**：A+B+C 完成后按其**最终真实形态**重写测试（禁凭计划想象 API）。mock TerminalRegistry 工厂增 `setClaudeSession` + `sessionChange` 通知能力。语义反转重灾区：T1 初始扫描（纯 shell 无行）、FE-03 register（不建行）、tab-rules:43-46（`claude update` 命中）。
- **接力说明**：Stage 01 时 `ContextUsage` 仍为 2 字段——D 写的 usage 字面量按 2 字段；Stage 03 扩 4 字段必填后由 Stage 03 agent B 同步补字面量 + 重算口径断言（checklist PF2-FE-11 波及面）。

### 验证项（断言全文 → `workflows/verify/stage-01.md`）

1. `TabTitleRegistry.match` 语义：Read 源码确认首 token 提取（`trim().split(/\s+/)[0]`）后再查表；`match("claude --resume x")` 命中、`match("  claude")` 命中、`match("claudeX")` 不命中。
2. `RegisteredTerminal.claudeSession` 为可选字段；`setClaudeSession` 五分支（merge/null 清空/no-op 不 notify/自动 lastEventAt/undefined 不覆盖）Read 源码逐条确认。
3. `RegistryEvent.type` 含 `"sessionChange"`，notify payload 不带 session 数据（语义式：payload 仅 type+panelId，不限写法）。
4. `useCommandDetection` 签名含 panelId；OSC 133 C 命中路径调 `setClaudeSession`（matchedCommand）；OSC 133 D（isCommandRunning）调 `setClaudeSession(panelId, null)`。
5. `useXterm` hook 订阅：SessionEnd/Exit → null；其他事件 → merge transcriptPath（`?? undefined` 或等价空值转换，不限写法）。
6. `useAgentStatus`：建行/删行通道与契约 5 逐条对应；registry subscribe effect deps 为 `[]`（语义式：deps 不含 rows/projectPageIds 等易变值，listener 经 ref 读最新状态）；初始扫描仅建 claudeSession 非 null 行且携 transcriptPath 时调 `contextUsage`；存在 reconcile 对账路径（语义式，不限函数名）。
7. `AgentStatusView.tsx:94` 空态文案 =「当前项目无运行中的 claude 会话」。
8. `useAgentStatus.ts` 原 :169-171 静默 catch 已补 `console.error`。
9. 测试重写完成：`tab-rules.test.ts` 无 `match("claude update")).toBeNull()` 旧断言；`agent-status-hook.test.ts` 覆盖纯 shell 无行/sessionChange 建行/remove 删行/初始扫描拉 usage/reconcile。
10. 门禁全绿：`npx tsc --noEmit`、`npx eslint src/`、`npm test`。

### commit message

```
fix: F5 行建模重设计——claudeSession 契约 + 双通道建行/三通道删行 + match 首 token

- TerminalRegistry 增 claudeSession（二态模型）+ setClaudeSession + sessionChange 事件
- TabTitleRegistry.match 改首 token 匹配，修复 claude --resume 不识别
- useCommandDetection/useXterm 双通道写入会话状态（OSC 133 C/D + hook 事件）
- useAgentStatus 行=运行中 claude 会话：初始扫描只建活会话+携 transcriptPath 拉 usage
- ref 稳定订阅 deps [] + reconcile 对账，根治关页签删行竞态（R4）
- 测试 7 文件按新语义重写（T1/FE-03/tab-rules 断言反转）

不符合项 #1/#3/#5 + 问题 1/2/3
```

### 人工验证点

无（本 Stage 全部可自动化；真实 claude 走查在收尾统一进行）。

---

## Stage 02 — toast 改设计·最小

**改动项**：PF2-FE-08、PF2-FE-09、PF2-FE-10、PF2-TE-04

### agent 分工表（单 agent）

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A: toast 全链 | PF2-FE-08、PF2-FE-09、PF2-FE-10、PF2-TE-04 | `src/ipc/notification.ts`、`src/features/notifications/useClaudeNotifications.ts`、`src/__tests__/notifications.test.ts` |

**无重叠证明**：单 agent，天然无重叠。划分豁免说明：本 Stage 仅 3 文件强耦合（ipc wrapper → 消费 hook → 其测试），并行拆分反而制造接口推断成本，故 1 agent。

### 实现要点

- 按契约 4：`sendToastNotification(title, { body })`——Tauri 原生 `sendNotification` 主路径，删 Web Notification 路径与 onclick；`ensureNotificationPermission` 保留；失败 catch 补 `console.error`。`:33` 的「委托 OS 原生通知中心」注释替换为 AUMID 平台限制结论（未打包 Win32 WebView2：banner 抑制/onclick 不路由/shim 无 close/构造不抛→catch 回退永不触发，探针实测证据）。
- `useClaudeNotifications`：删 routeToPanel/findPanelTitle/onClick 绑定 + 相关 import（setFocus/switchToPageAndFocus/getPageApi 如无其他使用一并清理）；**三类事件（permission/done/error）均触发任务栏闪烁**——决策：toast 失去点击能力后，任务栏闪烁是唯一回窗引导通道；失焦门控/60s 去重/classifyEvent 保留；:119-121 flashTaskbar 静默 catch 补 `console.error`。
- 测试：mock 换 `sendToastNotification`；删「toast onClick 路由」describe 整块（6 用例）；「任务栏闪烁细分」Stop/StopFailure/PostToolUseFailure 反转为触发闪烁；保留失焦门控/去重/classifyEvent 过滤/正文文案断言（新文案去面板标题）。

### 验证项（断言全文 → `workflows/verify/stage-02.md`）

1. `src/ipc/notification.ts` 导出 `sendToastNotification(title, options)` 且签名无第三参数（语义式：无任何形式的 onClick 参数）；实现内无 `new Notification(`。
2. `useClaudeNotifications.ts` 无 routeToPanel/findPanelTitle 定义与调用（语义式：不存在"toast 点击后路由到面板"的代码路径，不限函数名）；无 `sendClickableNotification` 引用。
3. 三类事件（permission/done/error 对应 classifyEvent 全分类）路径均调用 `requestUserAttention`（Read 源码确认闪烁调用不被事件类别条件排除）。
4. 失焦门控（`__slterm_windowFocused !== false` 才发送）与 60s 去重逻辑保留。
5. flashTaskbar 的 catch 补 `console.error`。
6. `notifications.test.ts` 无 onClick 路由 describe；闪烁细分三类均断言触发；门禁全绿（tsc/eslint/npm test）。

### commit message

```
fix: toast 改设计·最小——Tauri 原生 sendNotification + 去路由化 + 三类均闪烁

- sendClickableNotification → sendToastNotification：删 Web Notification 路径
  （未打包 Win32 WebView2 无 AUMID：banner 抑制/onclick 不路由/shim 无 close）
- useClaudeNotifications 去路由化：删 routeToPanel/findPanelTitle/onClick
- 三类事件（permission/done/error）均触发任务栏闪烁——toast 失去点击能力后
  闪烁是唯一回窗引导
- flashTaskbar/sendToastNotification 静默 catch 补 console.error

不符合项 #2 + 问题 4
```

### 人工验证点

**Stage 02 完成后（Stage 03 开始前不必阻塞，收尾前必须完成）**：Win11 真实环境实测 Tauri 原生 `sendNotification` 的 banner 可见性——真实 claude 触发权限请求（或 Stop），Alt+Tab 失焦，观察是否弹 banner。无法自动化（AUMID 限制对 Tauri 原生通道同样存在的可能性无法静态排除）。**不弹则 toast 退化为通知中心条目 + 任务栏闪烁主职，接受**（决策基线 1），结果记录在 Stage 05 文档同步中。

---

## Stage 03 — ContextUsage cache tokens 契约

**改动项**：PF2-BE-01、PF2-FE-11、PF2-DOC-01、PF2-TE-06、PF2-TE-07、PF2-TE-02（cache 口径部分）

### agent 分工表（A ∥ B 并行）

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A: Rust 契约 | PF2-BE-01、PF2-TE-06 | `src-tauri/src/hooks/usage.rs`（实现 + 同文件 `#[cfg(test)]`） |
| B: 前端+文档 | PF2-FE-11、PF2-DOC-01、PF2-TE-07、PF2-TE-02（cache 口径） | `src/types/hooks.ts`、`src/features/agentStatus/AgentStatusRow.tsx`、`docs/hooks-dev/contract.md`、`src/__tests__/ipc-hooks-contract.test.ts`、`src/__tests__/agent-status-view.test.tsx` |

**无重叠证明**：A 仅碰 Rust 单文件，B 仅碰前端/文档/测试文件，两集合不相交。A/B 共享的契约（四字段名 + camelCase + 口径）已写死在 checklist 契约 3，两边照抄不各自推断。

### 实现要点

- **A**：按契约 3——`ContextUsage` 增 `cache_read_input_tokens`/`cache_creation_input_tokens`（serde camelCase + `#[serde(default)]`）；`parse_usage_line` 缺失 `unwrap_or(0)`，`input_tokens` 缺失仍整行 None。**serde 两测试（:255-273）字面量同步**（缺新字段编译错）；`parse_extra_fields_ignored`（:143-151）不受影响（实证，勿动）。新 L1：cache 提取/缺省 0/旧格式兼容/显式 0/serde 四字段 camelCase。**cargo 系命令与 B 无共享**（B 不跑 cargo），无 target 锁冲突。
- **B**：`types/hooks.ts` 增两必填字段；`AgentStatusRow.tsx:31-35` total 改 `inputTokens + cacheReadInputTokens + cacheCreationInputTokens`；contract.md:124 C12 回填四字段 + 口径；ipc-hooks-contract `:310` 字面量补字段 + 新增键集合精确匹配守卫（照 :240-268 HookEventPayload 先例）；agent-status-view `:266,341,351,362` 字面量补字段 + 75%/low/medium/high 按新口径重算（接力 Stage 01 的 2 字段字面量）。
- **跨边界一致性**：A 的 serde 字段名（`cacheReadInputTokens`/`cacheCreationInputTokens`）与 B 的 TS 字段名必须逐字符一致——契约 3 已写死。
- **agent-status-hook.test.ts 的 T7 字面量**（Stage 01 D 产物）：四字段必填后编译错——由 B 一并补（grep `inputTokens:` 全仓测试文件兜底，含 `agent-status-hook.test.ts`）。B 的文件清单追加 `src/__tests__/agent-status-hook.test.ts`（仅字面量补字段微改）。

### 验证项（断言全文 → `workflows/verify/stage-03.md`）

1. `usage.rs` `ContextUsage` 含四字段且 serde camelCase（grep `cache_read_input_tokens` + `cacheReadInputTokens` 断言串均存在于文件）；`#[serde(default)]` 作用于 cache 两字段（语义式：缺字段反序列化不报错，不限属性写法）。
2. `parse_usage_line`：cache 缺失默认 0；`input_tokens` 缺失整行 None（沿用）。
3. `types/hooks.ts` `ContextUsage` 四字段必填；TS 字段名与 Rust serde 名逐字符一致。
4. `AgentStatusRow` total 口径 = input + cacheRead + cacheCreation（Read 源码确认 `outputTokens` 不在总占用求和内）。
5. `contract.md` C12 段含四字段定义与口径说明。
6. ipc-hooks-contract 含 ContextUsage 键集合精确匹配断言（`Object.keys(...).sort()` 四字段）。
7. agent-status-view 用量断言按新口径（75% 用例 total = input+cacheRead+cacheCreation 推导值与断言一致）。
8. grep `inputTokens:` 测试文件无 2 字段字面量残留（全量补字段）。
9. 门禁全绿：clippy（`-D warnings`）、`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`、tsc、eslint、npm test。

### commit message

```
fix: ContextUsage 增 cache tokens 四字段——用量口径对齐真实占用

- usage.rs ContextUsage 增 cache_read/cache_creation_input_tokens（serde default 0）
- 口径 = (input + cacheRead + cacheCreation) / 200_000；output 保留为信息字段
- 铁证：真实 transcript input 2745 + cache_read 196096 → 旧算法 1.4% vs 实际 99.4%
- contract.md C12 回填；L1/L2 测试同步 + ContextUsage 键集合守卫（DBG-4 模式）

不符合项 #4
```

### 人工验证点

无（真实 transcript 铁证已纳入 L1 用例数据）。

---

## Stage 04 — L4 防复发用例

**改动项**：PF2-TE-09

### agent 分工表（单 agent）

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A: L4 用例 | PF2-TE-09 | `e2e-tests/test.e2e.ts` |

**无重叠证明**：单 agent 单文件。划分豁免说明：L4 用例必须在 Stage 01/03 的新语义最终形态上编写，独立成 Stage 串行最后。

### 实现要点

- 用例 2a 语义反转改写（纯 shell 无行）；用例 2b 删「等待静态行出现」步骤（首个 PreToolUse 信号即建行）。
- 新增常驻 3 条：R2 变体（假 transcript JSONL 四字段 → 建行拉 usage → 切项目往返用量保持）、R3 变体（SessionEnd 删行 → 切项目不复活）、R4 变体（hook 建行 → `removePanel` 关页签 → 行消失）。
- 信号文件照现有先例：Node 端原子写（`.tmp → rename .json`）到 `~/.slterminal/hooks-events/`；R4 关页签用 `__dockviewApi.removePanel(panel)`（R4 原始探针教训：`panel?.close is not a function`）。
- R2 变体的假 transcript 必须是合法 JSONL 且含 `message.usage` 四字段行——后端 `hooks_context_usage` 真实解析（非 mock），同时覆盖 cache 口径全链路（L4 级）。
- **门禁特殊性**：`e2e-tests/test.e2e.ts` 不在根 tsconfig include 内（tsc/eslint 不覆盖）——本 Stage 门禁 = `npm run build:e2e` + `npm run wdio` 实跑（构建级 + 行为级双覆盖），无静态检查门禁属正常。

### 验证项（断言全文 → `workflows/verify/stage-04.md`）

1. `test.e2e.ts` 用例 2a 断言反转：创建终端后 `agent-status-row` 不出现（纯 shell 无行）；不存在「初始扫描生成 🟡 行」旧断言。
2. 用例 2b 无「等待静态行出现」前置步骤；首个信号文件到达后行出现且含 ⚡。
3. 新增 3 条常驻用例存在（语义式：切项目用量保持 / SessionEnd 删行后切项目不复活 / 关页签删行，用例名不限）。
4. R2 变体使用真实 transcript JSONL 文件（Node 端写盘）+ 信号文件 transcriptPath 指向它（非 mock contextUsage）。
5. R4 变体关页签用 `__dockviewApi.removePanel`（非 `panel.close()`）。
6. `npm run build:e2e` 成功 + `npm run wdio` 全量绿（含新增 3 条）。

### commit message

```
test: L4 防复发——R2/R3/R4 变体常驻用例 + 静态行语义反转

- 用例 2a 反转：纯 shell 终端无 agent-status 行
- 用例 2b 流程适配：hook 事件建行（首个信号即建行）
- 新增：切项目用量保持（真实 transcript JSONL 全链路）
- 新增：SessionEnd 删行 + 切项目不复活
- 新增：会话终端关页签删行（remove 事件稳定订阅）

E2E 防复发（决策基线 3）
```

### 人工验证点

无（wdio 实跑即验收）。

---

## Stage 05 — 文档同步

**改动项**：PF2-DOC-01（已由 Stage 03 完成的 contract.md 除外——本 Stage 仅核对）、PF2-DOC-02、PF2-DOC-03、PF2-DOC-04

### agent 分工表（单 agent）

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| A: 文档对账 | PF2-DOC-02、PF2-DOC-03、PF2-DOC-04 | `src/features/sideViews/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/ipc/CLAUDE.md`、`e2e-tests/CLAUDE.md`、`.claude/test-inventory.md`、`src-tauri/src/hooks/CLAUDE.md` |

**无重叠证明**：单 agent 纯文档。文档必须反映所有代码 Stage 完成后的最终状态，固定最后执行。

### 实现要点

- sideViews/CLAUDE.md：agentStatus 行语义（契约 1/5）+ AgentStatusRow 四字段口径。
- panels/CLAUDE.md：TerminalRegistry claudeSession/setClaudeSession/sessionChange；TabTitleRegistry match 首 token；**修正测试模式表 `TabTitleRegistry.test.ts` 驼峰误写 → kebab-case**（实证）。
- ipc/CLAUDE.md：notification 行——`sendToastNotification(title, {body})` Tauri 原生通道无 onClick + AUMID 平台限制结论。
- e2e-tests/CLAUDE.md：L4 用例表 agent-status 段（静态行反转 + 动态四态流程 + 新增 3 条）。
- test-inventory.md：以 Stage 01-04 后 `npm test` / `npm run wdio` **实际输出**对账（:183-189 / :45 / :57-64 / :257-259 段）。
- src-tauri/src/hooks/CLAUDE.md：问题 5 结论（hook 脚本 36-44ms/次，hooks 贡献 ~0.1s 非启动慢主因，接受现状）。
- Stage 02 人工验证点（banner 实测）结果一并写入 ipc/CLAUDE.md（若已实测）。

### 验证项（断言全文 → `workflows/verify/stage-05.md`）

1. 四 CLAUDE.md + test-inventory 描述与最终代码一致（语义式：逐段 Read 对照代码核实不撒谎——TerminalRegistry 文件行/claudeSession、match 首 token、sendToastNotification、L4 用例表新语义）。
2. `panels/CLAUDE.md` 无 `TabTitleRegistry.test.ts` 驼峰残留（grep）。
3. test-inventory 用例数与 `npm test` 实际输出一致（重写的 7 文件 + 新增）。
4. src-tauri/src/hooks/CLAUDE.md 含 hook 脚本性能实测结论（36-44ms/次）。
5. 全量回归绿：tsc、eslint、clippy、cargo test、npm test、test:l3。

### commit message

```
docs: Phase 2 fix 文档同步——CLAUDE.md/test-inventory 对账最终代码

- sideViews/panels/ipc/e2e-tests CLAUDE.md 反映行建模/toast/cache 新语义
- panels/CLAUDE.md 修正 TabTitleRegistry.test.ts 驼峰误写（kebab-case 实证）
- test-inventory.md 按 npm test/wdio 实际输出对账
- hooks/CLAUDE.md 记录问题 5 结论（hooks ~0.1s 非启动慢主因，接受现状）
```

### 人工验证点

无（文档一致性由 verify agent 语义式核对）。

---

## 收尾人工实测（全部 Stage 完成后，交付前）

1. **Stage 02 banner 实测**（若 Stage 期间未完成）：Win11 真实 claude 权限请求 → 失焦 → 观察 Tauri 原生通知 banner；不弹则接受退化（决策基线 1）并记录。
2. **真实 claude 走查**（对照 review-findings 5 问题逐项）：
   - 问题 1：终端执行 `claude --resume <id>` → 页签改名 + agent 行出现。
   - 问题 2：agent 行含量化用量 → 切项目往返 → 用量保持。
   - 问题 3 三场景：B 项目退 claude / A 项目退 claude（切项目往返不复活）/ 关页签 → 行均消失。
   - 问题 4：失焦触发权限请求 → 任务栏闪烁 + 通知（banner 或通知中心条目，按实测形态接受）。
   - 问题 5：不回归即可（启动 1-3s 为 claude 自身成本，文档已记录）。
