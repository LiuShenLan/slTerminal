# review-fix 修复 — 执行编排参数

> 本文件只写任务特定编排参数；通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）单一真值源在 `/systematic-changes-execute`，不复制。
> 真值源：`docs/review-fix/checklist.md`（条目）+ `docs/review-fix/stages.md`（划分与契约）。

## 1. Stage 表

| Stage | 内容 | agent 数 | L4 门禁 | commit message |
|-------|------|---------|---------|----------------|
| 01 | 安全族 + 守卫强化（AQ-1~4 + ZQ-5 + CS-1/2） | 5 并行 | ✅（run-wdio.cjs 零静态覆盖） | `fix(security): 安全族修复 + AC-5 守卫强化（AQ-1~4/ZQ-5/CS-1/2）` |
| 02 | 前端正确性族（ZQ-1/2/3/4/6/7） | 3 并行 | ❌ | `fix(agent-status): 前端正确性族——keyOf/resolvePayloadCliId 单点 + null 建行语义（ZQ-1~4/6/7）` |
| 03 | transcript 中性化全链路（KZ-2/3） | 3 并行 | ✅（e2e spec 更名无静态门禁） | `refactor(hooks): transcript 概念中性化——usageSourcePath 全链路更名（KZ-2/KZ-3）` |
| 04 | hub 编辑器分派 + 层抽象（KZ-1/4/5） | 2 串行 pipeline | ❌（hub claude 渲染列入人工验证点 + 收尾 L4 兜底） | `refactor(cli-profiles): hub 编辑器分派 + 配置层抽象入 profile（KZ-1/KZ-4/KZ-5）` |
| 05 | mockcli 验收强化（KZ-7/CS-3） | 2 并行 | ✅（CS-3 新增用例 + helpers.ts） | `test(cli-profiles): mockcli 编辑器分派双向断言 + L4 关键路径补全（KZ-7/CS-3）` |
| 06 | 文档同步终验（YS-1~5/WD-1~4/KZ-6） | 4 并行 | ✅ 收尾全量（含 L1——先关闭 slterminal.exe） | `docs(review-fix): 文档一致性修复 + 终态核对（YS-1~5/WD-1~4/KZ-6）` |

脚本路径：`docs/review-fix/workflows/stage-NN-*.js`；verify 断言：`docs/review-fix/workflows/verify/stage-NN.md`；修复循环：`docs/review-fix/workflows/fix-loop.js`。

## 2. 统一门禁命令

| 序 | 命令 | 用途 | Stage 适用范围 |
|---|------|------|---------------|
| 1 | `npx tsc --noEmit` | TS 静态检查 | 全部 |
| 2 | `npx eslint src/` | ESLint | 全部 |
| 3 | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Clippy | 全部 |
| 4 | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | rustfmt | 全部 |
| 5 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | L1（**必须单线程**） | 全部 |
| 6 | `npm test` | L2 | 全部 |
| 7 | `npm run test:l3` | L3 | 全部 |
| 8 | `node --check e2e-tests/run-wdio.cjs` | launcher 语法级（tsc/eslint 覆盖外文件） | 01 |
| 9 | `npx vite build` | 循环依赖打包图验证（KZ-1 新增 features→panels 依赖方向） | 04 |
| 10 | `npm run e2e` | L4（= build:e2e + wdio 串行；**禁拆分、禁与其他命令并行**——cargo 无法覆写被 wdio 占用的 slterminal.exe；config.json 的 `e2eBuild` 不带 `VITE_E2E=1`，直接使用会 tree-shake helper 全卡「Workspace 未就绪」） | 01/03/05/06 |

> L4 按需策略 = 决策 4（checklist.md「决策结论」表）：触碰 e2e-tests/ 的 Stage（01/03/05）必跑，纯 src/ 改动的 Stage（02/04）不跑，06 收尾全量。

## 3. git add 路径枚举（Stage commit 限定）

照 config.json `workflow.gitAddPaths` 原样，本任务无补充（不涉 public/ 资源新增）：

```
src/
src-tauri/
e2e-tests/
test/
.claude/CLAUDE.md
.claude/test-inventory.md
docs/
```

## 4. fix-loop args 规范

`fix-loop.js` 的 args 由执行期组装（模板强制校验：failedItems 非空数组 + verifyFile 非空字符串，缺失即 throw）：

| 字段 | 取值规范 |
|------|---------|
| `stage` | Stage 编号（1–6） |
| `failedItems` | verify agent 返回的 `failedItems` 原样透传（与 `verify/stage-NN.md` 同一真值源） |
| `fixContext` | Stage 脚本头部的跨边界契约段（stages.md「跨边界契约」节对应契约原文）+ 本 Stage 实现要点 |
| `verifyFile` | `docs/review-fix/workflows/verify/stage-NN.md`（与 Stage 脚本同一断言文件） |
| `constraints` | stages.md「禁区」六条原样（ConPTY 0x7 / C10 / 轮询补漏 / SEC-05 / E2E 隔离 / E2E_ENABLED 内联） |
| `testCommands` | 可选；缺省 = 统一门禁 1–7（无 L4）。Stage 01/03/05/06 且失败项涉 L4 断言时必传——取对应 Stage 脚本的 TEST_COMMANDS 数组原样（含 L4，脚本内已注明 L4 最后单独串行） |

重试上限：`maxFixRetries = 3`（config.json）。

## 5. 并行 agent 测试纪律（生成期约定，执行期提醒）

- 同一 Stage 并行 agent **不跑资源共享型测试**（PTY/端口/全局锁）——重构阶段只做编译级检查（`npx tsc --noEmit` / `cargo check --manifest-path src-tauri/Cargo.toml`），真实执行由全量测试 agent 单点跑
- cargo 系命令共享 target 目录锁，并行时排队属正常——勿中止
- L4（`npm run e2e`）与任何 cargo/前端构建命令**禁并行**（slterminal.exe 文件占用冲突）

## 6. test-inventory 就近同步纪律

`.claude/test-inventory.md` 在并行 agent 间是共享文件——**每 Stage 指定单点负责**（防同文件并发冲突，裁决已写死 stages.md 各分工表）：Stage 01 → literal-guard；Stage 02 → composite-key；Stage 03 → frontend-consumers；Stage 04 → layers（串行后序）；Stage 05 → l4-mockcli；Stage 06 → root-doc（总数对齐核对 + 实跑计数回写）。其余 agent 的用例增/删/更名由单点负责者按 prompt 写明的代登记项一并同步。Stage 间串行 + commit，跨 Stage 无冲突。

## 7. 进度跟踪表（执行期填写）

| Stage | 状态（待跑/进行中/已提交/已验证） | commit hash | verify allFixed | fix-loop 轮次 | 备注（人工验证点确认） |
|-------|------|------------|-----------------|--------------|----------------------|
| 01 | 待跑 | | | | |
| 02 | 待跑 | | | | |
| 03 | 待跑 | | | | |
| 04 | 待跑 | | | | |
| 05 | 待跑 | | | | |
| 06 | 待跑 | | | | |

## 8. 人工验证点汇总（收尾实测项）

照 stages.md 末节原样：Stage 01 fixture 缺失终止 + 单引号 cwd 命令实测；02 无图标行视觉；03 「版本过旧」重注入链路；04 hub 面板全链；06 终验走查。
