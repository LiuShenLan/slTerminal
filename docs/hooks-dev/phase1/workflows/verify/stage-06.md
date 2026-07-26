# Stage 06 Verify：文档同步

> 断言与 Stage 06 完成后的真实中间态一致。

## 新增模块文档

- [ ] `P1-DOC-01` `src-tauri/src/hooks/CLAUDE.md` 存在。
- [ ] `P1-DOC-01` 文档含模块职责、文件清单、信号目录、watcher 启动方式、三命令说明、注入配置段识别规则、版本检测说明、测试模式。

## 更新文档

- [ ] `P1-DOC-02` `src/ipc/CLAUDE.md` 模块映射表含 `hooks.ts` ↔ `src-tauri/src/hooks/`。
- [ ] `P1-DOC-03` `src/lib/CLAUDE.md` 文件表含 `claudeStatus.ts`。
- [ ] `P1-DOC-04` `src/panels/CLAUDE.md` 更新 `useCommandDetection`（OSC 133 C→🟡）。
- [ ] `P1-DOC-04` `src/panels/CLAUDE.md` 更新 `useXterm`（hook-event 订阅）。
- [ ] `P1-DOC-04` `src/panels/CLAUDE.md` 更新 `DefaultTab`（emoji 渲染）。
- [ ] `P1-DOC-04` `src/panels/CLAUDE.md` 更新 `tabRules`（图标移除）。

## 测试清单

- [ ] `P1-DOC-05` `.claude/test-inventory.md` 更新新增测试文件与用例数。
- [ ] `P1-DOC-05` L1 用例数包含 hooks 模块与 pty env 注入测试增量。
- [ ] `P1-DOC-05` L2 用例数包含 ipc-hooks-contract、claude-status、useXterm hooks 测试增量。
- [ ] `P1-DOC-05` L4 用例数包含新增 2 条 E2E 用例。

## 静态检查

- [ ] `npx tsc --noEmit` 通过（确保文档修改未破坏类型）。
- [ ] 所有修改的 markdown 文件无语法错误。
