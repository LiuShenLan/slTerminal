# Stage 06 逐项验证断言（唯一真值源）

> stage-06 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-10**：`src/App.tsx` 加载页无裸 `"monospace"` 字体声明（须 Read 确认为全局字体栈完整形态 `"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace`）；说明文字色引用 DIM_FG（不再引用 INPUT_BORDER——grep INPUT_BORDER 在 App.tsx 零命中）
- **FE-11**：`src/lib/ErrorBoundary.tsx` 无裸 `"monospace"`（两处均为全局字体栈完整形态）；error-boundary 测试通过
- **FE-18**：`grep "⚠" src/` 零命中；`src/lib/icons.tsx` 新增三角告警图标导出（grep 命中）；GitShowPanel 大文件警告处引用该图标（须 Read 确认 ⚠ 已替换且颜色经 token）；gitshow-panel 测试通过
- **FE-26**：`src/ipc/window.ts`（或 App.tsx——按实际落点）unlisten Promise 链尾含 `.catch` 兜底（须 Read 确认 reject 被吞）；新增 unlisten reject 不抛用例且通过
- **FE-27**：`ExplorerPanel.tsx` 文件树容器无 `outline: "none"`（grep 零命中）；`tabIndex={-1}` 保留；explorer-focus 测试增断言且通过

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
