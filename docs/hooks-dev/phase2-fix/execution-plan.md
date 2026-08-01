# Phase 2 Fix — 执行编排参数

> 通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作 / 修复循环流程）单一真值源在 `/systematic-changes-execute`，本文档不复制。
> 清单与契约真值源：`docs/hooks-dev/phase2-fix/checklist.md`；Stage 细节真值源：`docs/hooks-dev/phase2-fix/stages.md`。

## Stage 编排表

| Stage | 名称 | 脚本 | verify 断言 | commit message（前缀） |
|-------|------|------|------------|----------------------|
| 01 | F5 行建模重设计 | `docs/hooks-dev/phase2-fix/workflows/stage-01-row-model.js` | `docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md` | `fix: F5 行建模重设计——claudeSession 契约 + 双通道建行/三通道删行 + match 首 token` |
| 02 | toast 改设计·最小 | `docs/hooks-dev/phase2-fix/workflows/stage-02-toast.js` | `docs/hooks-dev/phase2-fix/workflows/verify/stage-02.md` | `fix: toast 改设计·最小——Tauri 原生 sendNotification + 去路由化 + 三类均闪烁` |
| 03 | ContextUsage cache 契约 | `docs/hooks-dev/phase2-fix/workflows/stage-03-cache-usage.js` | `docs/hooks-dev/phase2-fix/workflows/verify/stage-03.md` | `fix: ContextUsage 增 cache tokens 四字段——用量口径对齐真实占用` |
| 04 | L4 防复发用例 | `docs/hooks-dev/phase2-fix/workflows/stage-04-l4-e2e.js` | `docs/hooks-dev/phase2-fix/workflows/verify/stage-04.md` | `test: L4 防复发——R2/R3/R4 变体常驻用例 + 静态行语义反转` |
| 05 | 文档同步 | `docs/hooks-dev/phase2-fix/workflows/stage-05-docs.js` | `docs/hooks-dev/phase2-fix/workflows/verify/stage-05.md` | `docs: Phase 2 fix 文档同步——CLAUDE.md/test-inventory 对账最终代码` |

## 各 Stage 门禁命令

| Stage | 门禁命令（全量测试 agent 单点执行） |
|-------|----------------------------------|
| 01 | `npx tsc --noEmit`；`npx eslint src/`；`npm test` |
| 02 | `npx tsc --noEmit`；`npx eslint src/`；`npm test` |
| 03 | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`；`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`；`npx tsc --noEmit`；`npx eslint src/`；`npm test` |
| 04 | `npm run build:e2e`；`npm run wdio` |
| 05 | `npx tsc --noEmit`；`npx eslint src/`；`npm test`；`npm run test:l3`；`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`；`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` |

> **Stage 04 偏离 config.json 的说明**：config `commands.e2eBuild`（`npx tauri build --debug --no-bundle`）缺 `VITE_E2E=1`——E2E helper 由 `E2E_ENABLED` 门控，`tauri build` 前端恒为 production `vite build`（`DEV=false`），不设开关则 helper 被 tree-shake、wdio 全部卡"Workspace 未就绪"（项目 CLAUDE.md 测试策略硬约束）。故 Stage 04 用 `npm run build:e2e`（= `cross-env VITE_E2E=1 tauri build --debug --no-bundle`），不用 config 的 e2eBuild。

## git add 路径枚举（Stage commit 限定）

```
src/
src-tauri/
e2e-tests/
test/
.claude/CLAUDE.md
.claude/test-inventory.md
docs/
```

（与 config.json `workflow.gitAddPaths` 一致；各 Stage 脚本 git 操作按此枚举，不用 `git add -A`。）

## fix-loop args 规范

verify 未全过时调用修复循环脚本 `docs/hooks-dev/phase2-fix/workflows/fix-loop.js`，args：

```json
{
  "stageName": "stage-01-row-model",
  "failedItems": ["<verify 断言原文，逐条>","..."],
  "verifyFile": "docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md",
  "testCommands": ["<该 Stage 门禁命令，照上表逐条>"],
  "constraints": "<该 Stage 脚本 PREAMBLE 中的 Stage 特殊纪律段原文>",
  "fixContext": "<可选：verify agent details 证据原文（失败原因线索）>"
}
```

- `failedItems` 非空强制校验（脚本内建）；逐条照抄 verify agent 返回的 failedItems，不改写不概括。
- `verifyFile` 必填；与对应 Stage 脚本引用同一文件（修复与初验同一标尺）。
- `testCommands` 非空强制校验（脚本内建）；照「各 Stage 门禁命令」表逐条。
- `constraints` 从对应 Stage 脚本头复制（单一出处，禁手写第三份）。
- `fixContext` 可选；verify agent 返回的 details 原文，作失败原因线索传入。

## 进度跟踪表

| Stage | 状态 | commit | 备注 |
|-------|------|--------|------|
| 01 行建模 | 未开始 | — | pipeline A→B∥C→D |
| 02 toast | 未开始 | — | 完成后安排 banner 人工实测（不阻塞 03） |
| 03 cache | 未开始 | — | A∥B；B 接力补 Stage 01 测试字面量 |
| 04 L4 | 未开始 | — | wdio 实跑为验收 |
| 05 DOC | 未开始 | — | 收尾人工走查后交付 |
