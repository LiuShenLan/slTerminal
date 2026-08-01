# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-05**：`src/features/claudeHistory/historyModel.ts` 存在且为纯函数模块（grep `react`/`useState`/`useEffect` 零命中）；导出 `isCurrentProject`/`groupByCwd`/`matchesSearch`/`formatRelativeTime`/`deriveActiveSessionIds`（Read 确认）；`isCurrentProject` 经 `normalizePath` + 忽略大小写精确相等（Read 确认，复用 `src/lib/path.ts`）；`groupByCwd` 组内 mtimeMs 降序、组间按组内最大 mtimeMs 降序、无 cwd 归 null 组（Read 确认三分支）；`formatRelativeTime` 覆盖 刚刚/N 分钟前/N 小时前/N 天前/MM-DD/YYYY-MM-DD 六档（Read 确认分支）；`deriveActiveSessionIds` 仅从 `claudeSession.transcriptPath` basename 提取（Read 确认无 matchedCommand 分支）。
- **FE-04**：`src/features/claudeHistory/useClaudeHistory.ts` 存在；返回形状与 stages.md 契约段逐字一致（`state/sessions/activeIds/rootPath/scan/removeLocal/updateLocalTitle`，Read 对照）；`removeLocal`/`updateLocalTitle` 不触发 scan（Read 确认无 IPC 调用）；含 `TerminalRegistry.subscribe` 且卸载时取消订阅（Read 确认 cleanup）；rootPath 推导经 activePageId → project（Read 确认）。
- **FE-06**：`src/features/claudeHistory/restoreSession.ts` 存在；导出 `restoreHistorySession(session, opts)` 签名与 stages.md 契约段逐字一致（Read 对照）；四步顺序为 项目入列（addProject 条件调用）→ 页面保障（addPage 条件调用）→ `switchToPageShared` → 轮询 `getPageApi` + `addPanel` + 轮询 TerminalRegistry + `pty.write`（Read 确认调用顺序）；注入命令普通 = `claude --resume <id>\r`、fork = `claude --resume <id> --fork-session\r`（Read 确认字符串构造）；防重入守卫存在（模块级标记，Read 确认）；失败路径调 `sendToastNotification`（grep 命中）。
- **无 barrel（负向断言）**：`src/features/claudeHistory/index.ts` 本 Stage **不存在**（避免双 agent 冲突，barrel 归 Stage 05）。
- **测试**：`src/__tests__/claude-history-model.test.ts`、`claude-history-hook.test.tsx`、`claude-history-restore.test.ts` 三文件存在；restore 测试断言四步调用顺序与 write payload 内容（`\r` 结尾、fork 变体）、防重入、失败 toast（Read 测试文件逐一核对）；model 测试覆盖六档时间与分组排序（Read 核对）。
- **禁区**：`git diff` 本 Stage 不含 `src-tauri/src/pty/` 下任何文件改动。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
