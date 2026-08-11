# review-06 测试质量

> 维度：断言有效性 / 防复发 / mock 诚实性 / 隔离 / 守卫有效性 / 清单一致性 / 覆盖空洞。只写问题。

## 问题条目

### CS-1（P1）no-claude-literals 守卫可被模板字符串拼接绕过
- 位置：`src/__tests__/no-claude-literals.test.ts:extractStringLiterals`（模板字符串分支）
- 问题：词法器遇到含 `${}` 的模板字符串直接跳过，不检查其字面量片段。生产代码可用 `const x = \`cl${''}aude\`` 拼出运行时值 `"claude"`，守卫不会报违规。
- 后果：通用层可通过模板字符串拼接绕过 AC-5 "claude" 字面量禁令，multi-cli 抽象泄漏无法被自动化守卫捕获。
- 修复建议：对含表达式的模板字符串也做基础扫描——若字面量片段拼接后组成 `"claude"`、claude 事件名或 `"~/.claude"` 路径，应报违规或至少标记可疑供人工复核。
- 来源：独立发现

### CS-2（P1）no-claude-literals 守卫扫描范围遗漏 cliProfiles 根目录
- 位置：`src/__tests__/no-claude-literals.test.ts:SCAN_DIRS`
- 问题：扫描范围七路径未包含 `src/features/cliProfiles`，而该目录下的 `cliProfileRegistry.ts`、`types.ts` 等文件属于通用层，同样应遵守 AC-5 不写 "claude" 字符串字面量的约束。
- 后果：若 cliProfileRegistry 或 types 文件中意外写死 `"claude"` 字符串字面量，守卫无法发现，AC-5 覆盖不完整。
- 修复建议：将 `src/features/cliProfiles` 加入 `SCAN_DIRS`，同时豁免 `src/features/cliProfiles/profiles/claude/` 子目录（claude 合法领地）。
- 校注（汇总核实）：该目录当前生产代码（types.ts / cliProfileRegistry.ts / index.ts / profiles/index.ts）grep 无 "claude" 字面量——本条为守卫覆盖空洞，非现存违规。
- 来源：独立发现

### CS-3（P2）L4 mockcli profile 覆盖仅到 OSC 133 命中
- 位置：`e2e-tests/mockcli.e2e.ts`
- 问题：`mockcli.e2e.ts` 仅覆盖 AC-4①（OSC 133 C 命中页签/logo + D 恢复），AC-4②~⑤（hooks 能力真实调用、历史聚合 UI、hub 选择行、恢复注入）没有 L4 用例。
- 后果：mock profile 的 hooks/history 能力与 hub 集成仅在 L2 mock 环境验证，真实二进制中 E2E helper 注册逻辑、IPC 泛化命令序列化、hub 面板交互等路径缺乏端到端守卫。
- 修复建议：在 L4 补充 mockcli 关键路径用例：注入 mockcli agent-event 后验证页签 emoji/通知调度；创建 `cliId="mockcli"` 历史条目并验证历史区展示与行 logo；打开 hooksConfig 面板验证 hub 选择行渲染与保存链路携带 mockcli cliId；双击 mock 历史行验证恢复注入 `mockcli --resume`。
- 来源：独立发现
