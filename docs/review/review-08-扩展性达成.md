# review-08 扩展性达成

> 维度：新增 CLI 接入成本演习（codex 假想接入逐触点核对）。只写问题。

## 问题条目

### KZ-1（P0）hub 配置编辑器硬编码为 ClaudeHooksConfigEditor，新 CLI 无法挂载自有编辑器
- 位置：`src/panels/hooksConfig/HooksConfigPanel.tsx:32`（import ClaudeHooksConfigEditor）/`:254`（编辑器槽无条件渲染）——汇总核实修正行号
- 问题：选择行过滤 `hasConfigEditor===true` 后，下方编辑器槽无条件渲染 `<ClaudeHooksConfigEditor>`，没有任何按 cliId 分派编辑器的逻辑；profile 接口也未提供 `configEditorComponent` 或类似字段。
- 阻碍场景：codex 若声明 `hasConfigEditor: true`，hub 会显示 codex 按钮，但点击后仍加载 claude 专属编辑器——其事件树（eventsCatalog）、schema 校验、matcher 引擎、注入提示全部面向 claude hooks 协议，codex 配置语义无法正确呈现，实质上 forced to modify core UI。
- 修复建议：在 profile 上增加编辑器分派字段（如 `capabilities.hooks.configEditor: React.FC<{ profile: CodingCliProfile }>`），或建立 `HooksConfigEditorRegistry`；`HooksConfigPanel` 按选中 cliId 查表渲染对应编辑器。 codex 提供自己的编辑器组件并独立测试。
- 来源：独立发现

### KZ-2（P1）AgentEventPayload 强依赖 `transcriptPath` 字段，语义对 codex 不中立
- 位置：`src/types/agent.ts:27`；`src-tauri/src/hooks/signal.rs:31`
- 问题：事件负载 DTO 中 `transcriptPath: string` 为必填字段，字段名与注释均指向 claude 的 transcript 文件；codex 若不存在 transcript 概念，reporter 必须伪造路径或写空串，后端 `context_usage` 也会收到无意义路径。
- 阻碍场景：codex reporter 实现时需额外约定“无 transcript 时填空串/占位路径”，并确保 `context_usage` 能容错；该字段的强存在把 codex 会话模型强行拉向 claude 形态。
- 修复建议：将 `transcriptPath` 改为可选（`string | null`），reporter 无对应概念时传 `null`；消费方（用量查询、会话存储）将 null 视为“该 CLI 无 transcript”。
- 来源：独立发现

### KZ-3（P1）CliHooksProvider trait 参数命名暴露 claude transcript 概念
- 位置：`src-tauri/src/hooks/provider.rs:33`
- 问题：`context_usage(&self, transcript_path: &str)` 的参数名把“transcript”写入跨 CLI trait 签名；codex provider 实现时方法签名语义上即声明自己在处理 transcript。
- 阻碍场景：codex 实现 trait 时被迫把其自身 artifact（如 conversation log、checkpoint）解释为 transcript_path，增加理解成本与文档噪音，且trait 文档的中立性受损。
- 修复建议：参数更名为 `usage_source_path: &str` 或 `artifact_path: &str`，并在 trait 文档说明“由具体 CLI 解释路径语义”。
- 来源：独立发现

### KZ-4（P1）hooks 配置层级被 claude 的 user/project/local 三层锁死
- 位置：`src/types/hooksConfig.ts:8`；`src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx:58-62`；`src/panels/hooksConfig/useHooksConfig.ts`（layer 状态及 read/write 透传）
- 问题：`HooksLayer` 类型硬编码为 `"user" | "project" | "local"`，编辑器 LAYERS 常量、禁用逻辑、保存/读取全部围绕这三层；后端 trait 的 `config_read(layer, ...)` / `config_write(layer, ...)` 也收到同一字符串集合。
- 阻碍场景：若 codex 的配置分层模型不同（如仅 global/project 两层，或完全不同命名），现有 UI 与 IPC 契约无法表达；codex 要么伪装成 user/project/local，要么需要改核心类型与编辑器。
- 修复建议：将 layer 抽象为 profile 可声明的字符串集合（如 `HooksCapability.layers: { id: string; label: string }[]`），编辑器按 profile 提供的层列表渲染；缺省回退 claude 三层。
- 来源：独立发现

### KZ-5（P2）ClaudeHooksConfigEditor 错误提示硬编码 claude settings.json 路径
- 位置：`src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx:269`（注入失败提示）/`:285`（卸载失败提示）——汇总核实修正行号
- 问题：错误文案直接写 `"~/.claude/settings.json"`，且 `src/panels/hooksConfig/` 不在 AC-5 守卫扫描范围，导致该 claude 路径字面量残留。
- 阻碍场景：codex 接入后，用户点击“注入 Hooks”失败时，弹窗提示检查 `~/.claude/settings.json`，与实际 codex 配置路径不符，造成误导。
- 修复建议：错误提示改为 profile 驱动（如增加 `capabilities.hooks.configHint` 或从 provider 返回），或统一使用中性文案“CLI 配置文件”。
- 校注（汇总核实）：与 KZ-1 同源——该字面量位于已声明的 claude 专属编辑器内（MC-223 决策 2 合法领地），KZ-1 编辑器分派落地后 codex 使用自有编辑器，本条自然消解；是否仍需中性化文案随 KZ-1 修复形态定夺。
- 来源：独立发现

### KZ-6（P2）cliProfiles/CLAUDE.md「新增 CLI 步骤」遗漏关键扩展点
- 位置：`src/features/cliProfiles/CLAUDE.md:88-90`
- 问题：文档列出的新增步骤仅包含 `public/cli-icons/<id>.png`、 `profiles/<cli>/` 定义、`profiles/index.ts` 追加 import、对应 cli-profile 测试；遗漏后端 hooks provider 注册（`src-tauri/src/hooks/provider.rs` REGISTRY）、后端 history provider 注册（`src-tauri/src/agent_history/provider.rs` REGISTRY）、`test-inventory.md` 用例数同步，以及当 `hasConfigEditor=true` 时必须新增编辑器组件并修改 hub 分派的步骤。
- 阻碍场景：按当前文档接入 codex，开发者会漏掉后端 provider 注册导致命令返回「未知 cliId」；会漏掉 test-inventory 更新导致审查/CI 口径不一致；更会漏掉编辑器分派导致 KZ-1 的 bug 无人知晓。
- 修复建议：补充上述四步到「新增 CLI 步骤」；若保留 hub 编辑器硬编码，则应显式注明“当前仅 claude 支持 hasConfigEditor”。
- 来源：独立发现

### KZ-7（P2）mock-cli-profile 测试用 mock 编辑器掩盖了 hub 编辑器无法泛化的问题
- 位置：`src/__tests__/mock-cli-profile.test.tsx:83-86`（`vi.mock("../panels/hooksConfig/JsonMode", ...)` 及 `mockJsonMode`）；`src/__tests__/helpers/mockCliProfile.ts:36`（`hasConfigEditor: true`）
- 问题：AC-4④ 验证 hub 选择行与切换时，用 `mockJsonMode` 替换了真实 `JsonMode`，因此从未真正渲染过非 claude 编辑器的完整 UI；mockCliProfile 的 `hasConfigEditor: true` 并未触发真实 codex-like 编辑器的挂载，导致 KZ-1 未被测试捕获。
- 阻碍场景：新增 codex 时若照搬 mockCliProfile 的测试路径，会误以为 hasConfigEditor 已验证通过，实则在生产环境崩溃或显示错误编辑器。
- 修复建议：AC-4④ 增加一条回归用例——mockCliProfile 被选中时，hub 实际渲染的是 mockCli 自己的编辑器桩（而非 ClaudeHooksConfigEditor 中的 claude 事件树/GUI 表单），或至少断言未渲染 claude 专属事件目录节点。
- 来源：独立发现

## 已检查范围

1. profile 定义：`profiles/index.ts` 纯追加即可，Workspace.tsx 已 side-effect import，无问题。
2. HooksCapability 接口自足，但 `classifyNotification` 消费 `AgentEventPayload` 受 KZ-2 影响。
3. 消费方泛化：useXterm/useAgentStatus/useAgentNotifications/AgentStatusRow/HistorySessionRow/restoreSession/historyContextMenu/useCommandDetection 均经 registry 消费，无直接 import claude 策略函数，无问题。
4. UI 挂载点：hooksConfig hub 选择行、agentStatus/agentHistory 行 logo 均按 registry 自动出现；但编辑器渲染见 KZ-1。
5. 缺能力降级：useXterm/useAgentStatus/notifications 三处未知/无 hooks 能力均 `console.warn + return`，不崩溃，无问题。
6. 后端 provider 注册：hooks/history 的 `REGISTRY` 静态数组新增条目即可，resolve_provider 错误语义自动生效，无问题。
7. 后端 trait 中立性：`context_usage` 参数名见 KZ-3；`config_read/write` 的 layer 参数受 KZ-4 影响。
8. reporter 机制：`~/.slterminal/hooks-events/` 单目录共用 + `payload.cliId` 路由，支持多 CLI 并存，无问题。
9. agent-event 广播：单事件多 CLI 共用，前端按 cliId 分发成立，无问题。
10. 测试基建：mockCliProfile 夹具路径已趟平，但 AC-4④ 对编辑器的验证不完整，见 KZ-7。
11. 文档指引：「新增 CLI 步骤」遗漏后端注册、test-inventory 与编辑器分派，见 KZ-6。
