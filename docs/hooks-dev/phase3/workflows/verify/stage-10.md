# Stage 10 逐项验证断言

> stage-10 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-DOC-01**：`src/panels/CLAUDE.md` 已更新，包含 `hooksConfig` 面板描述、文件清单、双模式与三层配置说明。
- **P3-DOC-02**：`src/ipc/CLAUDE.md` 模块映射表追加 `src/ipc/hooksConfig.ts` 与 `hooks_config_read`/`hooks_config_write`。
- **P3-DOC-03**：`src/stores/CLAUDE.md` Store 清单追加 `hooksConfig.ts` 与 `disabledHooks` 段说明。
- **P3-DOC-04**：`src/features/shortcuts/CLAUDE.md` 命令目录追加 `global.openHooksConfig`。
- **P3-DOC-05**：`.claude/test-inventory.md` 已新增 Phase 3 测试文件与用例数；全量用例总数已更新。
- **P3-DOC-05b**：文档中的 IPC 命令名、面板类型名、store 字段名与代码一致（须 Read 代码核对）。

## 全量测试

1. 文档语法检查（无 markdown linter 则手动抽查）
2. `grep` 确认 `.claude/test-inventory.md` 中 Phase 3 新增文件数与实际 `src/__tests__/` 下新增测试文件数一致
