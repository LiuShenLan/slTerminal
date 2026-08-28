# 执行编排参数（conda-profile-fix）

> 通用执行规则见 `/systematic-changes-execute`，本文件只写任务特定编排参数。

- 任务目录：`docs/conda-profile-fix/`
- checklist：`docs/conda-profile-fix/checklist.md`
- stages：`docs/conda-profile-fix/stages.md`
- Stage 脚本：`docs/conda-profile-fix/workflows/stage-01-shell-profile.js`、`docs/conda-profile-fix/workflows/stage-02-docs.js`
- fix-loop：`docs/conda-profile-fix/workflows/fix-loop.js`
- verify 唯一真值源：`docs/conda-profile-fix/workflows/verify/stage-01.md`、`docs/conda-profile-fix/workflows/verify/stage-02.md`

## Stage 表

| Stage | 脚本 | 改动项 | commit message | git add |
|-------|------|--------|----------------|---------|
| 01 | `stage-01-shell-profile.js` | B17-FIX, TE-B17 | `fix(pty): 移除 spawn PowerShell 的 -NoProfile——恢复用户 profile 加载，修复 conda activate 失效（B17）` | `src-tauri/src/pty/shell.rs` |
| 02 | `stage-02-docs.js` | DOC-B17a/b/c | `docs(pty): B17 文档同步——profile 加载红线 + 编号登记 + 用例清单/豁免登记` | `src-tauri/src/pty/CLAUDE.md` `.claude/CLAUDE.md` `.claude/test-inventory.md` |

## fix-loop args 规范

`Workflow({ scriptPath: 'docs/conda-profile-fix/workflows/fix-loop.js', args: { stage, failedItems, fixContext, verifyFile, constraints } })`

| 字段 | 取值 |
|------|------|
| `stage` | 1 或 2 |
| `failedItems` | 该 Stage verify 返回的未过项 ID 列表（非空） |
| `fixContext` | verify agent details 证据原文 |
| `verifyFile` | `docs/conda-profile-fix/workflows/verify/stage-01.md` 或 `stage-02.md`（与 Stage 同一标尺） |
| `constraints` | Stage 01：空串；Stage 02：`本 Stage 只改 markdown 文档，禁止改任何代码/测试文件` |

## 进度跟踪表

| Stage | 状态 | verify 结果 | commit |
|-------|------|-------------|--------|
| 01 | 待执行 | — | — |
| 02 | 待执行 | — | — |

## 人工验证（全部 Stage 完成后，MANUAL-B17）

1. win11 本机：`npx tauri build --debug --no-bundle` → 新终端页签 `conda activate claude` → `(claude)` 前缀 + `python --version` 正常
2. win10 部署机（miniforge）：同法验证
3. 冒烟：OSC cwd 跟踪、prompt 无转义泄漏、`claude` 页签标题/图标切换正常
