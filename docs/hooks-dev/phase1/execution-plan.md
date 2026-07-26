# Phase 1 执行计划

> 本文件只写任务特定编排参数。通用执行规则见 `/systematic-changes-execute`。

---

## Stage 表

| Stage | 脚本路径 | verify 路径 | 主要门禁命令 |
|-------|----------|-------------|--------------|
| 01 | `docs/hooks-dev/phase1/workflows/stage-01-backend.js` | `docs/hooks-dev/phase1/workflows/verify/stage-01.md` | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` + `cargo test --manifest-path src-tauri/Cargo.toml hooks -- --test-threads=1` |
| 02 | `docs/hooks-dev/phase1/workflows/stage-02-pty.js` | `docs/hooks-dev/phase1/workflows/verify/stage-02.md` | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` + `cargo test --manifest-path src-tauri/Cargo.toml pty_env -- --test-threads=1` |
| 03 | `docs/hooks-dev/phase1/workflows/stage-03-frontend-ipc.js` | `docs/hooks-dev/phase1/workflows/verify/stage-03.md` | `npx tsc --noEmit` + `npx eslint src/ipc/hooks.ts src/lib/claudeStatus.ts` + `npm test ipc-hooks-contract claude-status` |
| 04 | `docs/hooks-dev/phase1/workflows/stage-04-frontend-integration.js` | `docs/hooks-dev/phase1/workflows/verify/stage-04.md` | `npx tsc --noEmit` + `npx eslint src/panels/terminal src/workspace/PageDockviewHost.tsx` + `npm test` |
| 05 | `docs/hooks-dev/phase1/workflows/stage-05-e2e.js` | `docs/hooks-dev/phase1/workflows/verify/stage-05.md` | `npm run build:e2e` + `npm run wdio` |
| 06 | `docs/hooks-dev/phase1/workflows/stage-06-docs.js` | `docs/hooks-dev/phase1/workflows/verify/stage-06.md` | markdownlint 或 `npx tsc --noEmit`（若修改 TypeScript 类型）；纯文档 Stage 不跑测试 |

---

## Commit message 模板（按 Stage）

| Stage | message |
|-------|---------|
| 01 | `feat: 后端 hooks 模块（信号 watcher + 注入/卸载/状态命令）` |
| 02 | `feat: pty_spawn 注入 SLTERM_PANEL_ID 环境变量` |
| 03 | `feat: 前端 IPC hooks 层 + 四态状态机单点` |
| 04 | `feat: 页签四态指示（OSC 133 + hook-event 集成）` |
| 05 | `test: L4 E2E 页签图标 hook-event 流转关键路径` |
| 06 | `docs: Phase 1 hooks 模块与四态指示文档同步` |

---

## `git add` 路径枚举（每 Stage）

### Stage 01
```
src-tauri/src/hooks/mod.rs
src-tauri/src/hooks/signal.rs
src-tauri/src/hooks/watcher.rs
src-tauri/src/hooks/inject.rs
src-tauri/assets/slterm-hook-reporter.js
src-tauri/src/lib.rs
```

### Stage 02
```
src-tauri/src/pty/spawn.rs
src-tauri/tests/pty_integration_tests.rs
```

### Stage 03
```
src/ipc/hooks.ts
src/ipc/index.ts
src/lib/claudeStatus.ts
src/__tests__/ipc-hooks-contract.test.ts
src/__tests__/claude-status.test.ts
```

### Stage 04
```
src/panels/terminal/useCommandDetection.ts
src/panels/terminal/useXterm.ts
src/panels/terminal/TerminalPanel.tsx
src/panels/terminal/tabRules.ts
src/workspace/PageDockviewHost.tsx
src/__tests__/*（新增/修改的测试文件）
```

### Stage 05
```
e2e-tests/helpers.ts
e2e-tests/test.e2e.ts
```

### Stage 06
```
src-tauri/src/hooks/CLAUDE.md
src/ipc/CLAUDE.md
src/lib/CLAUDE.md
src/panels/CLAUDE.md
.claude/test-inventory.md
```

---

## fix-loop args 规范

调用 `Workflow({ scriptPath: "docs/hooks-dev/phase1/workflows/fix-loop.js", args: {...} })` 时 args 必须含：

```jsonc
{
  "stage": 1,          // 所属 Stage 编号
  "failedItems": ["P1-BE-01", "P1-BE-03"], // verify 返回的未通过项 ID 数组，非空
  "fixContext": "verify agent 给出的 details 证据原文...", // 可选
  "verifyFile": "docs/hooks-dev/phase1/workflows/verify/stage-01.md", // 必填
  "constraints": ""    // Stage 特殊纪律，如无则空串
}
```

- `stage` 用于日志与 label。
- `failedItems` 必须来自对应 verify 文件的 ID。
- `verifyFile` 必须与 Stage 脚本同一真值源。
- `constraints` 示例：Stage 05 可传 `"本 Stage 只改 e2e-tests/，禁止改生产代码"`。

---

## 进度跟踪表

| Stage | 状态 | 通过 verify | 备注 |
|-------|------|-------------|------|
| 01 | 未开始 | - | |
| 02 | 未开始 | - | 依赖 Stage 01 的 hooks 模块命令注册（仅编译级，不依赖实现细节） |
| 03 | 未开始 | - | |
| 04 | 未开始 | - | 依赖 Stage 03 的 `src/ipc/hooks.ts` 与 `src/lib/claudeStatus.ts` |
| 05 | 未开始 | - | 依赖 Stage 04 的页签集成与 Stage 01 的 watcher |
| 06 | 未开始 | - | 依赖全部代码 Stage 完成 |

---

## 待执行期确认清单（checklist 已标注）

| # | 项 | 推荐值 | 是否已在 checklist/stages 中显式决策 |
|---|----|--------|--------------------------------------|
| 1 | 信号并发策略 | 单事件单文件 + 原子 rename | 是 |
| 2 | 版本过旧比对 | 脚本内 `SCRIPT_VERSION` 常量正则提取 | 是 |
| 3 | hooks 模块文件划分 | `mod.rs`/`signal.rs`/`watcher.rs`/`inject.rs` | 是 |
| 4 | Stage 1 L4 关键路径 | 注入状态 + 页签图标流转 | 是 |
| 5 | `InjectionStatus` 序列化方案 | camelCase 字符串；实现时若 serde 默认不满足改用 untagged/自定义 | 是（实现期确认） |

---

## 关键依赖与顺序

```
Stage 01 ─┐
Stage 02 ─┤
Stage 03 ─┤
          ├──> Stage 04 ─> Stage 05
Stage 06 （最后，依赖 01-05）
```

Stage 01/02/03 之间无强依赖，可并行启动；Stage 04 依赖 03；Stage 05 依赖 01+04；Stage 06 最后。
