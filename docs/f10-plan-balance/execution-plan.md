# F10 编码套餐余量展示 — 执行编排参数（execution-plan）

> 通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作 / fix-loop 触发）单一真值源在 `/systematic-changes-execute`，本文档只写任务特定参数。

## Stage 表

| Stage | 范围 | agent 编排 | 脚本 | verify 文件 | commit message |
|-------|------|-----------|------|------------|----------------|
| 01 后端 | PB-BE-01~08 | 单 agent 顺序 | `workflows/stage-01-backend.js` | `workflows/verify/stage-01.md` | `feat(plan-balance): F10 后端套餐余量模块（来源/查询注册表 + 轮询推送 + 双命令）` |
| 02 前端 | PB-FE-01~06 | pipeline 串行 2 agent（fe-data → fe-ui） | `workflows/stage-02-frontend.js` | `workflows/verify/stage-02.md` | `feat(plan-balance): F10 前端余量 footer（DTO/ipc/纯函数/hook/组件 + L2 测试）` |
| 03 文档 | PB-DOC-01~04 | 单 agent | `workflows/stage-03-docs.js` | `workflows/verify/stage-03.md` | `docs(plan-balance): F10 模块文档与用例清单登记` |

commit 粒度：commitPerStage（每 Stage verify 全绿后单独 commit）。

## git add 路径

沿用 config.json `workflow.gitAddPaths`：`["src/", "src-tauri/", "e2e-tests/", "test/", ".claude/CLAUDE.md", ".claude/test-inventory.md", "docs/"]`。

**例外**：PB-DOC-04 若触发 CONTEXT.md 修订（CONTEXT.md 在仓根、不在列表内），Stage 03 commit 前临时补 `git add CONTEXT.md`；不修订则不补。

## fix-loop args 规范

```json
{
  "stage": <NN 数字>,
  "failedItems": ["<verify 判 partial/fail 的断言 ID>"],
  "fixContext": "<verify agent 给出的失败详情>",
  "verifyFile": "docs/f10-plan-balance/workflows/verify/stage-NN.md",
  "constraints": ""
}
```

各 Stage 无特殊纪律（constraints 空）；唯一约束来源 = 脚本头 PREAMBLE 禁区（ConPTY flags 固定 0x7 禁改）。

## 门禁命令（config.json fullCheck）

```
npx tsc --noEmit && npx eslint src/ && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings && npm test && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

三 Stage 同门禁（Stage 03 文档 Stage 门禁仅跑 `npm test` + `cargo test` 取数登记，见 stage-03 脚本）。

## 进度跟踪表

| Stage | 状态 | verify 结果 | fix 轮次 | commit hash |
|-------|------|------------|---------|-------------|
| 01 后端 | 未开始 | — | 0 | — |
| 02 前端 | 未开始 | — | 0 | — |
| 03 文档 | 未开始 | — | 0 | — |

## 收尾

全部 Stage 完成后 → stages.md「收尾人工实测」5 条由用户执行（真实账号双套餐一轮、视觉、断网容错、kimi 字段实证、ureq TLS 双机）。
