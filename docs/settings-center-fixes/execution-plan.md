# F11 设置中心 Review 修复——执行编排参数

> 通用执行规则见 `/systematic-changes-execute`（单一真值源），本文件只写任务特定参数。

## Stage 表

| Stage | 脚本 | verify | 负责项 |
|-------|------|--------|--------|
| 01 | `docs/settings-center-fixes/workflows/stage-01-data-defense.js` | `docs/settings-center-fixes/workflows/verify/stage-01.md` | BE-01 / BE-02 / FE-01 |
| 02 | `docs/settings-center-fixes/workflows/stage-02-startup-blocking.js` | `docs/settings-center-fixes/workflows/verify/stage-02.md` | FE-02 / FE-03 / TE-01 |
| 03 | `docs/settings-center-fixes/workflows/stage-03-e2e-isolation.js` | `docs/settings-center-fixes/workflows/verify/stage-03.md` | TE-02 / TE-03 / FE-04 |
| 04 | `docs/settings-center-fixes/workflows/stage-04-l2-test-quality.js` | `docs/settings-center-fixes/workflows/verify/stage-04.md` | TE-04 / TE-05 / TE-06 |
| 05 | `docs/settings-center-fixes/workflows/stage-05-docs-closure.js` | `docs/settings-center-fixes/workflows/verify/stage-05.md` | DOC-01..07 |

## fix-loop args 规范

- `stage`：Stage 编号（1-5）
- `failedItems`：verify agent 返回的未通过项 ID 数组（必填非空）
- `fixContext`：verify details 证据原文
- `verifyFile`：上表对应 Stage 的 verify 文件路径
- `constraints`：取值单点定义于 `workflows/fix-loop.js` 头注释与各 stage 脚本头注释，本文件不复制

## git add 路径枚举

`src-tauri/src/app_dir.rs`、`src-tauri/src/projects.rs`、`src/stores/projects.ts`、`src/App.tsx`、`src/panels/settings/`、`src/workspace/PageDockviewHost.tsx`、`src/__tests__/`、`e2e-tests/`、`docs/settings-center-requirements.md`、`docs/settings-center/report.md`、`docs/settings-center-fixes/`、`.claude/test-inventory.md`、`src/stores/CLAUDE.md`、`src-tauri/src/CLAUDE.md`、`e2e-tests/CLAUDE.md`、`src/__tests__/CLAUDE.md`、`src/features/settingsCenter/types.ts`

## 禁区（各脚本 PREAMBLE 写入）

- 禁止改 `src-tauri/src/pty/` 任何 ConPTY flags（compute_conpty_flags 固定 0x7，含其 4 条守卫测试——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入）
- 禁止前端 `src/ipc/` 外出现 invoke
- 禁止硬编码颜色（经 theme/colors.ts token）
- 禁止 `npm run tauri dev` 验证（构建用 `npx tauri build --debug --no-bundle`）
- 禁止写入真实凭据值（SEC-18，测试/文档仅允许 sk-test 假值占位符）

## 并行纪律

- 并行 agent 不跑共享资源测试（PTY/端口/全局锁类）——重构阶段只做编译级检查，真实执行统一由全量测试 agent 单点跑
- cargo 系命令共享 target 目录锁，并行时排队属正常勿中止
- Stage 03 的 wdio 实跑由该 Stage 全量测试 agent 单点执行

## 进度跟踪表

| Stage | 实现 | 测试 | verify | commit |
|-------|------|------|--------|--------|
| 01 | ☑ | ☑ | ☑ | ☑ (967d42d) |
| 02 | ☑ | ☑ | ☑ | ☑ (d053748) |
| 03 | ☐ | ☐ | ☐ | ☐ |
| 04 | ☐ | ☐ | ☐ | ☐ |
| 05 | ☐ | ☐ | ☐ | ☐ |
