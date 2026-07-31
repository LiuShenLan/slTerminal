# Stage 10 逐项验证断言

> stage-10 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **P3-DOC-01**：`src/panels/CLAUDE.md` 已更新，包含 `hooksConfig` 面板描述、文件清单、双模式与 hooks 子树三层配置说明、注入段保护、F2 并入。
- **P3-DOC-02**：`src/ipc/CLAUDE.md` 模块映射表追加 `src/ipc/hooksConfig.ts` 与 `hooks_config_read`/`hooks_config_write`（含 hooks 子树 + merge 语义、与 `hooks.ts` 的区分说明）。
- **P3-DOC-03**：`src/stores/CLAUDE.md` Store 清单追加 `hooksConfig.ts` 与 `disabledHooks` 段说明（含面板挂载时加载、不在 App init）。
- **P3-DOC-04**：`src/features/shortcuts/CLAUDE.md` 命令目录追加 `global.openHooksConfig`（含同页单例语义）。
- **P3-DOC-05**：`.claude/test-inventory.md` 已新增 Phase 3 测试文件与用例数；全量用例总数已更新（与实际 grep 计数一致，非照抄计划数字）。
- **P3-DOC-05 一致性**：文档中的 IPC 命令名、面板类型名、store 字段名与代码一致（须 Read 代码核对：`hooks_config_read`/`hooks_config_write`/`hooksConfig`/`disabledHooks`）。
- **P3-DOC-06**：契约回查已执行——对照 `docs/hooks-dev/contract.md` C13 逐项核实最终实现（命令签名/字段矩阵/事件目录/面板 id 规则），回查结论落盘（一致项打勾、偏差项记录）；`src-tauri/src/hooks/CLAUDE.md` 已追加 `config.rs` 文件行与两条命令说明。

## 全量测试

1. 文档语法检查（无 markdown linter 则手动抽查）
2. `grep` 确认 `.claude/test-inventory.md` 中 Phase 3 新增文件数与实际 `src/__tests__/` 下新增测试文件数一致
3. `grep` 核对文档中 IPC 命令名/面板类型名/store 字段名与代码一致
