# Stage 3 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **DBG-11-ipc**：`src/ipc/CLAUDE.md` 中 `pty.ts` 相关描述含 `panelId`（grep `panelId` 命中），且与实际签名一致（对照 `src/ipc/pty.ts` 当前代码确认文档不撒谎）。
- **DBG-11-panels**：`src/panels/CLAUDE.md` 中 `pty.write`/`pty.resize`/`pty.kill` 提及处与新签名一致（grep 逐命中确认无"只传 sessionId"的旧描述残留）。
- **DBG-11-workspace**：`src/workspace/CLAUDE.md` 的「页面切换流」含 `setProjectRoot` 前置描述（grep `setProjectRoot` 命中）；「架构决策」节有对应条目并说明 SEC-01 effect 保留兜底。
- **DBG-11-e2e**：`e2e-tests/CLAUDE.md` 全局对象表中 `__slterm_e2e_createProject`/`__slterm_e2e_switchToPage` 标注 async/Promise（grep `async\|Promise` 命中相关行）。
- **DBG-11-fs**：`src-tauri/src/fs/CLAUDE.md` 含 sandbox 时序要求记录（grep `project_root` 命中，且语义为"消费方须保证 project_root 已设置"）。
- **DBG-11-inventory**：`.claude/test-inventory.md` 含新增用例记录（契约守卫/时序/竞态用例），计数与 `src/__tests__/` 实际新用例数一致（Read 测试文件核实真实数量）。
- **DBG-11-scope**（语义式）：本次 commit 仅含文档改动——`git show --stat HEAD` 确认无 `src/`、`src-tauri/src/`、`e2e-tests/*.ts`（helpers 除外，不应出现）生产/测试代码文件。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
