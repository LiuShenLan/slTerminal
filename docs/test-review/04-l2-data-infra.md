# L2 域C（数据层/基建域）测试 Review 报告

## 问题清单

### [中] `editorSyntaxHighlight` 测试未覆盖 syntax 映射与层叠顺序

- **维度**：覆盖率
- **证据**：`src/theme/CLAUDE.md:31,48`、`src/__tests__/overrides.test.ts:187-197`
- **证据类型**：静态推断
- **问题**：`editorSyntaxHighlight()` 负责把 `editor.overrides.syntax` 9 组 tag → color 映射转为 CM6 `HighlightStyle`，且 ACC-05 指出 syntax 映射只能靠数组顺序决胜、不得后置 `editorTheme`。但当前测试仅验证「函数存在」和「返回扩展可被 `EditorState.create` 消费而不抛异常」，未断言 9 个 syntax 色值是否生效、也未验证挂载顺序下 syntax 规则能否层叠胜出。若 `syntax` 值漂移或被 oneDark 覆盖，测试无法发现。
- **建议**：补充用例读取 `themeRules(editorSyntaxHighlight())`，断言 9 个 syntax 颜色（`propertyName/string/number/keyword/function/variableName/typeName/operator/punctuation/comment`）均出现且源自 active 方案；同时按 ACC-05 验证 syntax 规则在 `editorTheme` 之后挂载时仍能胜出。

### [中] `ipc-agent-history-contract` 未覆盖 SEC-05 sessionId 非法值

- **维度**：覆盖率
- **证据**：`src-tauri/src/agent_history/provider.rs:29-35`、`src/__tests__/ipc-agent-history-contract.test.ts:120-153,155-188`
- **证据类型**：静态推断
- **问题**：后端 `CliHistoryProvider` 契约明确 `validate_session_id` 是 `delete` / `read_title` 的强制前置（SEC-05）。但前端 IPC 契约测试只验证正常 UUID 形态的 `{ cliId, sessionId }` 与异常传播，未覆盖空串、包含路径穿越字符、非 UUID 格式等非法 sessionId 样例，无法在前端侧锁死后端拒绝语义。
- **建议**：为 `deleteHistorySession` 与 `readHistoryTitle` 各增加异常维度用例：使用 `mockThrow` 模拟后端对非法 `sessionId` 返回 `Validation` 错误，断言 wrapper 将异常透传至调用方；同步更新 `.claude/test-inventory.md` 用例计数。

### [低] theme 文档与测试对标量计数解释不一致

- **维度**：断言有效性
- **证据**：`src/theme/CLAUDE.md:77`、`src/__tests__/scheme-registry.test.ts:153-158`
- **证据类型**：静态推断
- **问题**：`CLAUDE.md` 写明 linear.ui 为「6 组 + 26 标量」，而 `scheme-registry.test.ts` 注释按「23 既有 + 4 新增 = 27 标量」解释，并断言 `scalarCount === 27`。文档与测试对同一事实的计数口径冲突，后续新增/删除 ui 标量时无法判断应同步文档还是改断言。
- **建议**：以 `linear.ts` 实际值为准统一口径（当前为 27 标量），并更新 `CLAUDE.md` 与测试注释；测试注释宜列出 4 个新增标量名称或引用 `linear.ts` 文件头契约，避免再次漂移。

### [低] `no-claude-literals` 扫描路径为硬编码白名单，新增通用目录不会自动纳入

- **维度**：覆盖率
- **证据**：`src/__tests__/no-claude-literals.test.ts:43-52`
- **证据类型**：静态推断
- **问题**：AC-5 守卫的 `SCAN_DIRS` 硬编码八条通用路径（`src/lib`、`src/ipc`、`src/types`、`src/panels/terminal` 及四个 `features` 子目录）。若后续新增通用模块目录（如 `src/services`、`src/hooks` 等）且未同步追加到 `SCAN_DIRS`，该目录下的 `"claude"` 字符串字面量、claude 事件名或 `~/.claude` 路径将逃脱扫描。
- **建议**：将扫描范围改为递归扫描整个 `src`（仅排除 `src/features/cliProfiles/profiles/claude` 等豁免目录），或在 CI 中增加校验脚本，比较 `src` 顶层目录与 `SCAN_DIRS` 是否同步。

## 审查覆盖声明

- **审阅文件**：47/47。核心模块：`src/features/agentHistory/`、`src/features/agentStatus/`、`src/features/hooksConfig/`、`src/features/cliProfiles/`、`src/ipc/`、`src/theme/`、`src/stores/`、`src/lib/`。
- **执行命令与结果**：全量 L2 基线 `npm test` 通过，154 个测试文件、2635 个用例全部通过。
- **重跑验证记录**：对 `statusline-bridge-behavior.test.ts`、`hooks-config-sync.test.tsx`、`agent-history-restore.test.ts`、`no-claude-literals.test.ts`、`e2e-build-config.test.ts` 各重跑 3 次，结果均为 `Test Files 5 passed / Tests 52 passed`，未发现新的不稳定用例。
- **未覆盖到的潜在风险**：mockIPC 无法验证 camelCase/snake_case 真实转换、Channel 序列化与 Uint8Array 处理，相关真实序列化由 L4 E2E 守卫；本次未逐行精读全部 store 与 IPC 辅助文件细节，但已覆盖关键契约点。
