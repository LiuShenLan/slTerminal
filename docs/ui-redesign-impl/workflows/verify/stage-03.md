# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。

## 断言清单

- **IC-01**：grep `lucide-react` 命中 `package.json`（dependencies）；`src/lib/icons.tsx` 存在且导出脚本头契约清单（IconNav/IconFiles/IconCommit/IconConfig/IconChevronRight/IconChevronDown/IconRefresh/IconSearch/IconHistory/IconClose/IconMin/IconMax/IconCloseWin/IconPlus/IconFolder/IconEmptyBox——名称允许微调但须 Read 确认 16 个导出齐全、统一 size/strokeWidth/currentColor 封装）
- **IC-02**：`src/lib/StatusDot.tsx` 存在；Read 确认四态色映射（working/attention/done/error）且色值经 theme/colors.ts token 引用、无描边/光晕/动画
- **IC-03**：grep `STATUS_EMOJI` 在 `src/` 零命中；grep `tabStatus` 命中 `src/panels/terminal/TerminalPanel.tsx` 与 `src/workspace/PageDockviewHost.tsx`；grep `⚡\|🟡\|✅\|❌` 在 `src/`（`--include=*.ts --include=*.tsx`，不含 `src/__tests__/`）零命中；`src/lib/agentStatus.ts` 保留 status 类型与 eventToStatus 委托（Read 确认 F3 映射逻辑未动）
- **IC-04**：Read `src/features/explorer/FileIcon.tsx` 确认 SVG 实现、六色盘色值（grep `#7fa8e8\|#93b573\|#b48ce0\|#6fbfc4` 命中）、无 emoji 字面量；gitStatus 着色逻辑保留
- **IC-05**：grep `▶\|▼\|⏳` 在 `src/`（`--include=*.ts --include=*.tsx`，不含 `src/__tests__/`）零命中
- **IC-06**：grep `📋\|📁\|🔀\|🤖` 在 `src/features/sideViews/sideViewDefs.ts` 零命中；Read 确认 icon 字段为组件形态且 ActivityBar 渲染适配
- **IC-07**：grep `CATEGORY_EMOJI` 在 `src/` 零命中；grep `🔐` 在 `src/features/notifications/` 零命中
- **IC-08**：grep `✗` 在 `src/`（`--include=*.ts --include=*.tsx`，不含 `src/__tests__/`）零命中
- **IC-09**：`src/__tests__/emoji-scan.test.ts` 存在；`npm test` 通过（含本守卫，依测试 agent 结果）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
6. `npm run test:l3`
