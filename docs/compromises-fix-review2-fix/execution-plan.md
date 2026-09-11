# compromises-fix-review2 修复执行计划（编排参数）

> 通用执行规则单一真值源 = `/systematic-changes-execute` skill，本文档只写任务特定编排参数。
> 清单：`docs/compromises-fix-review2-fix/checklist.md`；Stage 划分：`docs/compromises-fix-review2-fix/stages.md`。

## 编排纪律（本任务特定）

1. **所有 agent 严格串行**（用户并发限制约束）：Stage 脚本内 for 循环顺序 `await agent(...)`，不用 `parallel()`；主 agent 调用 Workflow 也逐 Stage 串行，前一个 Stage commit 完成后再起下一个。
2. **全量测试命令逐条串行执行**（CP-007 基准负载敏感）：测试 agent prompt 已写死串行措辞。
3. **Stage commit 提交即登记本 Stage 行**（DOC-05 落地的新纪律，本计划执行期即生效）：进度跟踪表本 Stage 行在该 Stage commit 完成后、下一 Stage 启动前登记；登记更新可并入紧随提交，禁止跨 Stage 捎带。
4. **历史档注记纪律**：S06/S07 对 `docs/compromises-fix-review-fix/` 历史档一律注记追加，原文保留（见 stages.md 通用约定 3）。

## Stage 表

| Stage | 主题 | 项 | 脚本 |
|---|---|---|---|
| 01 | 工具链与 e2e 设施 | TE-01/02/03/04、FE-06 | `workflows/stage-01-toolchain-e2e.js` |
| 02 | 预览链路 | SEC-01、SEC-04 | `workflows/stage-02-preview-chain.js` |
| 03 | 后端加固 | BE-01/02/03/05/06 | `workflows/stage-03-backend-hardening.js` |
| 04 | explorer 状态机 | FE-01/02/03/04 | `workflows/stage-04-explorer-state.js` |
| 05 | 大文件链路 | FE-07/08/09/10 | `workflows/stage-05-large-file-chain.js` |
| 06 | 历史执行档注记 | SEC-02/03、BE-04、FE-05、DOC-03/04 | `workflows/stage-06-history-annotations.js` |
| 07 | 文档登记收口 | DOC-01/02/05、TE-05 | `workflows/stage-07-docs-registry.js` |

## commit message 表（逐 Stage 原文）

| Stage | message |
|---|---|
| 01 | `fix(review2-fix): S01 工具链与 e2e 设施——ts7 日志标签/vitest exclude 死配置/WARN 跨 chunk 计数/fallback 退出码/knip ignore 收窄（TE-01~04+FE-06）` |
| 02 | `fix(review2-fix): S02 预览链路——CSP 测试补 script/style 断言/sync 失败注释文案失实修正（SEC-01/04）` |
| 03 | `fix(review2-fix): S03 后端加固——notify panic 回传+Drop 委托/游标解码三分支补测/spawn 注释撞号/豁免计数（BE-01/02/03/05/06）` |
| 04 | `fix(review2-fix): S04 explorer 状态机——无快照双提交收口/抑制超时兜底/恰好一次断言/注释解除点补齐（FE-01~04）` |
| 05 | `fix(review2-fix): S05 大文件链路——inflight 失效清除/重载 stat 预检/首挂基线竞态封闭/抽样指纹比对（FE-07~10）` |
| 06 | `docs(review2-fix): S06 历史执行档注记——checklist/stages/verify/execution-plan 失实断言更正（SEC-02/03+BE-04+FE-05+DOC-03/04）` |
| 07 | `docs(review2-fix): S07 文档登记收口——「无 CSP」失实表述 5 处/adr+exemptions 注记/skill 登记纪律/workspace 签名（DOC-01/02/05+TE-05）` |

## git add 路径枚举（本任务）

`src/`、`src-tauri/`、`e2e-tests/`、`scripts/`、`vitest.config.ts`、`knip.json`、`.claude/adr.md`、`.claude/test-exemptions.md`、`.claude/skills/systematic-changes-execute/SKILL.md`、`.claude/CLAUDE.md`（兜底列备）、`docs/`

## fix-loop args 规范

```
Workflow({
  scriptPath: "docs/compromises-fix-review2-fix/workflows/fix-loop.js",
  args: {
    stage: <N>,
    failedItems: [<verify 返回的未通过项 ID>],
    fixContext: <verify agent details 证据原文>,
    verifyFile: "docs/compromises-fix-review2-fix/workflows/verify/stage-0N.md",
    constraints: ""  // S01-S05 传空串；S06/S07 取值见 workflows/fix-loop.js 头注释（单一真值源，此处不复制）
  }
})
```

## 进度跟踪表

| Stage | 状态 | commit | 结果摘要 | 备注 |
|---|---|---|---|---|
| 01 | 已完成 | dca9820 | TE-01~04+FE-06 全绿；门禁 6 条全过（npm test 199 文件/3261 例不缩水、knip exit 0） | 无修复循环 |
| 02 | 已完成 | 0fd4fa7 | SEC-01/04 全绿；门禁 6 条全过（含 L1 定向 preview 12 passed） | 无修复循环 |
| 03 | 已完成 | 2d2576a | BE-01/02/03/05/06 全绿；门禁 5 条全过（L1 全量 835 passed） | BE-05 附 3 处同号消解（spawn.rs:1530/1559/1632，为零命中断言所必需）；全量 cargo test 二跑 scan_bench 一度红（CP-007 负载敏感），隔离复跑两次绿定性 flaky |
| 04 | 已完成 | 8d4f604 | FE-01~04 全绿；门禁 4 条全过（npm test 199 文件/3263 例） | 修复循环 1 轮：FE-03 守卫滞留污染 explorer-delete 全局 timer 计数（1→2）→ 加 releaseSuppression 原语收口守卫生命周期（守卫存在 ⟺ 抑制窗口开启）+ 补 FE-03-2 防复发例；改后该用例原样转绿 |
| 05 | 未开始 | | | |
| 06 | 未开始 | | | |
| 07 | 未开始 | | | |

## 收尾（全 Stage 完成后主 agent 执行）

1. **全量回归兜底**（逐条串行，禁并行）：`npx tsc --noEmit` → `npx eslint src/` → `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` → `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` → `npm test` → `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` → `npx knip --production`。长输出命令日志落盘文件再提取摘要（禁 `cmd 2>&1 | tail` 裸管道）。
2. **人工验证点移交用户**（用户统一实机验证）：
   - S05 FE-09/FE-10：实机「外部修改浏览中的大文件」抽查（含同 size 同 mtime 改写——内容变但大小不变的指纹触发路径）；
   - S04 FE-02/FE-03：实机确认展开态注册表槽行为（review2 待验证清单 R2-FE-02 收口——挂载/切页/槽位回填的上呼次数与时机）；
   - S01 TE-03：修复后首轮全量 e2e WARN 计数基线复核（跨 chunk 修复可能使计数微升——此前漏计部分被计入，填记注明）；
   - 计划前轮遗留项照旧（review2 summary.md 待人工验证清单的计划内 6 组不受本轮影响）。
3. 归档 commit：本计划文档随首个 Stage 前单独提交（`docs(review2-fix): 修复计划——7 Stage 30 项`）。
4. 输出收尾报告：Stage commit 列表、未修复项（若有，含原因）。
