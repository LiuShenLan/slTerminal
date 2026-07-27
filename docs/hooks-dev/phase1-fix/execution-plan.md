# Phase 1 review 修复 — 执行编排参数（phase1-fix）

> 通用执行规则见 `/systematic-changes-execute`（单一真值源），本文件只写任务特定编排参数。

## Stage 表

| Stage | 名称 | 项 | 脚本 | verify | commit message |
|-------|------|-----|------|--------|----------------|
| 01 | 文档对账与行为文档化 | PF-DOC-01 ~ 06 | `docs/hooks-dev/phase1-fix/workflows/stage-01-docs.js` | `docs/hooks-dev/phase1-fix/workflows/verify/stage-01.md` | `docs: Phase 1 review 修复——用例计数对账 + 中断行为/信号瞬态文档化` |

## git add 路径枚举（Stage commit 限定）

- `.claude/test-inventory.md`
- `.claude/skills/systematic-changes-plan/config.json` — **注意**：config.json `workflow.gitAddPaths` 未含 `.claude/skills/`，本任务需显式追加此精确文件路径
- `src-tauri/src/hooks/CLAUDE.md`
- `src/panels/CLAUDE.md`
- `src/lib/claudeStatus.ts`
- `docs/hooks-dev/phase1-fix/`

明确排除：`docs/hooks-dev/phase1/`（决策 4：历史文档不回改）。

## fix-loop args 规范

```
Workflow({
  scriptPath: "docs/hooks-dev/phase1-fix/workflows/fix-loop.js",
  args: {
    stage: 1,
    failedItems: [<verify 未通过项 ID>],
    fixContext: "<verify details 证据原文>",
    verifyFile: "docs/hooks-dev/phase1-fix/workflows/verify/stage-01.md",
    constraints: <见 stage-01-docs.js 头注释的「args.constraints 应传值」——单一真值源，引用不复制>
  }
})
```

最多 3 轮（config.json `workflow.fixMaxRetries`）。

## 进度跟踪表

| Stage | 状态 | commit | 备注 |
|-------|------|--------|------|
| 01 | 未开始 | — | — |
