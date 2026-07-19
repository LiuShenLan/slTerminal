# Commit 侧栏视图执行计划（Step 5 主 agent 编排）

> 配套：`docs/commit-view/checklist.md`（清单真值源）、`docs/commit-view/stages.md`（Stage 定义 + 执行规则）。
> 脚本：`docs/commit-view/workflows/stage-01-*.js` ~ `stage-07-*.js` + `docs/commit-view/workflows/fix-loop.js`（均已语法预检 ✓）。
> 断言：`docs/commit-view/workflows/verify/stage-01.md` ~ `stage-07.md`。

## 进度跟踪表

| Stage | 脚本 | 状态 | verify 结果 | commit | 备注 |
|-------|------|------|------------|--------|------|
| 01 后端 git 能力 | `docs/commit-view/workflows/stage-01-git-backend.js` | ✅ 已 commit | allFixed=true, 5/5 门禁绿 | `b01cff7` | 1 agent |
| 02 前端基础设施 | `docs/commit-view/workflows/stage-02-fe-infra.js` | ✅ 已 commit | allFixed=true (fix-loop 1轮), 1303 tests | `eca166a` | 2 agent 并行 |
| 03 gitshow 面板 | `docs/commit-view/workflows/stage-03-gitshow-panel.js` | ✅ 已 commit | allFixed=true (fix-loop 2轮), 1323 tests | `9dbb09b` | 1 agent |
| 04 diff 面板 | `docs/commit-view/workflows/stage-04-diff-panel.js` | ✅ 已 commit | allFixed=true (fix-loop 1轮), 1359 tests | `84169a8` | 2 agent 并行 |
| 05 commit 视图 | `docs/commit-view/workflows/stage-05-commit-view.js` | ✅ 已 commit | allFixed=true, 1387 tests | `a61adf1` | 1 agent |
| 06 E2E | `docs/commit-view/workflows/stage-06-e2e.js` | ✅ 已 commit | allFixed=true (fix-loop 2轮), 16 E2E 全绿 | `e73e837` | 1 agent |
| 07 文档同步 | `docs/commit-view/workflows/stage-07-docs.js` | ⬜ 未开始 | — | — | 1 agent |

状态图例：⬜ 未开始 / 🔄 进行中 / ✅ 已 commit / ❌ 修复循环超限（人工介入）

## 执行循环（每 Stage）

```
1. Workflow({ scriptPath: "docs/commit-view/workflows/stage-NN-*.js" })
2. 检查返回 verifyResult.allFixed：
   - true  → git status 甄别 → git add（路径限定）→ git commit（message 取 stages.md 原文）→ 更新本表 → 下一 Stage
   - false → Workflow({ scriptPath: "docs/commit-view/workflows/fix-loop.js", args: {...} }) 循环，最多 3 轮
3. 修复循环 3 轮仍 false → 停，报告用户
```

## fix-loop args 规范

```js
{
  stage: <NN>,
  failedItems: <verifyResult.failedItems>,
  fixContext: <verifyResult.details 摘要>,
  verifyFile: "docs/commit-view/workflows/verify/stage-NN.md",
  // constraints 按 Stage 取值，与各 Stage 脚本头注释 + fix-loop.js 注释一致：
  //   Stage 06 → "本 Stage 只改 e2e-tests/ 下文件，禁止改 src/ 与 src-tauri/ 生产代码"
  //   Stage 07 → "本 Stage 只改 *.md 文档，禁止改代码"
  //   其余 Stage → ""
  constraints: ""
}
```

## git add 路径限定（每 Stage commit）

```
git add src/ src-tauri/ e2e-tests/ test/ .claude/CLAUDE.md .claude/test-inventory.md docs/
```

commit 前 `git status` 检查预期外改动；不用 `git add -A`。

## 故障降级备忘

- verify agent no-return → inline spawn 单 verify agent（复用脚本 prompt + schema + verify 文件 + testResult），不用 resume
- 重构/测试 agent no-return → `TaskStop` + `Workflow({ scriptPath, resumeFromRunId })`
- Bash 分类器故障 → 文件移动用 `mcp__filesystem__move_file`；批量替换用 PowerShell `Get-Content -Raw` + `Set-Content -NoNewline`
- 含中文文件一律内置 Write/Edit，不用 filesystem MCP 写

## 时间盒

- 后台运行期间每 15-20 分钟查 /workflows
- 单 Stage 超 60 分钟无进展 → `TaskStop` + 报告用户

## 恢复

中断后从本表首个 ⬜/🔄 Stage 继续；其前 Stage commit 已落盘无需重做。
