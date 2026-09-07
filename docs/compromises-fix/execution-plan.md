# 妥协修复编排参数（execution-plan）

> 只写本任务特定编排参数；通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作纪律）单一真值源在 `/systematic-changes-execute`，本文不复制。
> 配套：`checklist.md`（修复步骤真值源）、`stages.md`（分工表与实现要点真值源）、`workflows/verify/stage-NN.md`（断言真值源，stage 脚本与 fix-loop 共用）。

## Stage 编排表

| Stage | 脚本 | verify 断言 | 依赖 | commit message |
|---|---|---|---|---|
| S01 | `docs/compromises-fix/workflows/stage-01-cleanup-registries.js` | `docs/compromises-fix/workflows/verify/stage-01.md` | — | `refactor(compromises): S01 低风险清理与登记硬化——删 sidebar/navTree 迁移/启动色生成物/overrides 契约/TS7 机检（CP-001/025/026/021/027/038/032）` |
| S02 | `docs/compromises-fix/workflows/stage-02-l1-manifest.js` | `docs/compromises-fix/workflows/verify/stage-02.md` | S01 | `fix(compromises): S02 L1 manifest 改 embed-manifest 通道，拆 TQ-COV-06 红线（CP-040）` |
| S03 | `docs/compromises-fix/workflows/stage-03-e2e-infra.js` | `docs/compromises-fix/workflows/verify/stage-03.md` | S02 | `fix(compromises): S03 e2e 基建族——删启动器下载通道/真实屋键级断言/焦点与展开探针/mockcli history provider（CP-003/045/046/029/030/028/041）` |
| S04 | `docs/compromises-fix/workflows/stage-04-backend-primitives.js` | `docs/compromises-fix/workflows/verify/stage-04.md` | S03 | `fix(compromises): S04 后端基础原语——parking_lot 换装/git 死分支/conpty 状态暴露/pty kill 超时+删 ring/句柄级路径比对/statusline 确认注入（CP-005/008/010/011/034/014/043）` |
| S05 | `docs/compromises-fix/workflows/stage-05-ts-rs.js` | `docs/compromises-fix/workflows/verify/stage-05.md` | S04 | `refactor(compromises): S05 DTO ts-rs 单源化——Rust derive 生成 TS 类型，硬约束 #4 结构化（CP-024）` |
| S06 | `docs/compromises-fix/workflows/stage-06-backend-contracts.js` | `docs/compromises-fix/workflows/verify/stage-06.md` | S05 | `refactor(compromises): S06 后端契约重设计——fs_read_dir 游标分页/claude_history 缓存层实测处置（CP-006/007）` |
| S07 | `docs/compromises-fix/workflows/stage-07-panel-state.js` | `docs/compromises-fix/workflows/verify/stage-07.md` | S06 | `fix(compromises): S07 面板状态解耦——设置页常渲染/批量关页守卫/openSettingsPanel 事件驱动/侧栏状态上移/CM 保活/PTY spawn 尺寸首帧（CP-016/017/036/042/037/019）` |
| S08 | `docs/compromises-fix/workflows/stage-08-terminal-theme.js` | `docs/compromises-fix/workflows/verify/stage-08.md` | S07 | `fix(compromises): S08 终端与主题体验——Ctrl+C 中断事件源/SwiftShader 提示/编辑器主题槽+自绘 JSON lint/ConPTY 开关配置化（CP-020/018/039/002/009）` |
| S09 | `docs/compromises-fix/workflows/stage-09-large-file.js` | `docs/compromises-fix/workflows/verify/stage-09.md` | S08 | `feat(compromises): S09 编辑器大文件只读分片浏览——fs_read_file_range+虚拟化行窗口（CP-022）` |
| S10 | `docs/compromises-fix/workflows/stage-10-preview-webview.js` | `docs/compromises-fix/workflows/verify/stage-10.md` | S09 | `fix(compromises): S10 预览通道根治——迁独立 webview，主窗口回收 unsafe-inline/data: 放宽（CP-012/013/031/044/033/035）` |
| S11 | `docs/compromises-fix/workflows/stage-11-dockview-redesign.js` | `docs/compromises-fix/workflows/verify/stage-11.md` | S10 | `refactor(compromises): S11 Dockview 共享宿主+页组模型，MAX_PAGES 消亡（CP-004）` |
| S12 | `docs/compromises-fix/workflows/stage-12-finalize.js` | `docs/compromises-fix/workflows/verify/stage-12.md` | S11 | `test(compromises): S12 收尾——pty 覆盖率口径改生产代码+compromises.md 销项总扫+全量回归（CP-023）` |

commit 前缀集：`fix:` / `refactor:` / `docs:` / `test:` / `feat:`（S09 为唯一 feat——CP-022 属用户裁决纳入的新功能，此处登记扩展）。

## git add 路径枚举（按 Stage）

commit 时只允许 add 下列路径（该 Stage 触碰面）；`docs/compromises-fix/` 各 Stage 均可附加（执行期不留痕改动除外——计划产物本身已在执行前 commit）。`docs/compromises.md` 各 Stage 均可附加（销项勾选由主 agent 统一收口，见 stages.md 全局约定）。

| Stage | git add 路径 |
|---|---|
| S01 | `src/ test/ scripts/ package.json package-lock.json knip.json e2e-tests/CLAUDE.md .claude/adr.md .claude/skills/systematic-changes-plan/config.json .claude/skills/systematic-changes-execute/config.json` |
| S02 | `src-tauri/ .claude/CLAUDE.md .claude/test-exemptions.md` |
| S03 | `e2e-tests/ package.json package-lock.json src-tauri/src/agent_history/ .claude/test-exemptions.md` |
| S04 | `src-tauri/ src/ test/ .claude/adr.md .claude/test-exemptions.md` |
| S05 | `src-tauri/ src/types/ test/`（58 文件 import 零改动为设计前提；若执行期破了此前提，add `src/` 并在 commit body 留痕原因） |
| S06 | `src-tauri/ src/ test/` |
| S07 | `src/ test/` |
| S08 | `src/ src-tauri/ test/ package.json package-lock.json vitest.config.ts vitest.l3.config.ts .claude/adr.md`（vitest 配置文件名以 Glob 实查为准） |
| S09 | `src-tauri/ src/ test/` |
| S10 | `src-tauri/ src/ test/ e2e-tests/ .claude/adr.md` |
| S11 | `src/ test/ e2e-tests/` |
| S12 | `src-tauri/ .claude/test-exemptions.md docs/compromises.md` |

## fix-loop 调用规范

脚本：`docs/compromises-fix/workflows/fix-loop.js`（由模板 `templates/fix-workflow.js` 生成，args 强制校验在脚本内）。

调用形态（主 agent 在 Stage verify 出现未通过项时发起，每 Stage 最多 3 轮——上限值以 config.json `workflow.fixMaxRetries` 为唯一真值源，本文不复制）：

```
Workflow({
  scriptPath: 'docs/compromises-fix/workflows/fix-loop.js',
  args: {
    stage: <Stage 编号，数字 1-12>,
    failedItems: [<未通过项 ID，来自 verify agent 输出>],
    fixContext: <verify agent 的 details 证据原文>,
    verifyFile: 'docs/compromises-fix/workflows/verify/stage-<NN>.md',
    constraints: <Stage 特殊纪律，值见对应 Stage 脚本头注释；无则省略>
  }
})
```

各 Stage 的 `constraints` 取值（单处定义于此，脚本头注释引用本表）：

| Stage | constraints |
|---|---|
| S02 | 若 embed-manifest 未生效，走回退预案（逆操作恢复 [lib] 重组），不硬修 |
| S05 | src/types/ 为生成物，只改 Rust 侧 derive 与导出测试，不手改生成物 |
| S06 | CP-007 的删/修分支由 benchmark 实测数字决定，不两个分支同时做 |
| S07 | 串行链 017→036→042 顺序不可换 |
| S08 | compute_conpty_flags 禁区仅对 CP-009 解除；其余项不得触碰 ConPTY flags |
| S10 | spike no-go 时不启动 ②③④，转休眠登记而非强行修复 |
| S11 | panelId 页前缀协议先行落地+测试，再改消费点 |
| 其余 | （无） |

## 进度跟踪表

执行期由主 agent 维护（每 Stage 完成更新一行）。

| Stage | 状态 | commit | 完成日期 | 备注 |
|---|---|---|---|---|
| S01 | 已完成 | c6758e5 | 2026-09-07 | 7 项全绿（fix-loop 1 轮修注释字面断言）；发现基线红：cargo test lib_tests read_resource_without_root_rejected 1 failed（cfg!(test) root 豁免与用例语义矛盾，非 S01 引入，S02 前须定夺） |
| S02 | 回退预案 | 无 commit（零实现） | 2026-09-07 | CP-040 通道失实：embed-manifest 为 bins-only build.rs 库够不到 test target（E0433 实测）；cargo 1.96 --lib 0xc0000139 复现根因未自愈 → 维持 test=false+[[test]]+红线，L1 定向命令**维持旧形态**（全册影响，S03 起 verify 按旧形态执行） |
| S03 | 已完成 | 0c840df | 2026-09-07 | 7 项全绿（fix-loop 1 轮：CP-028 helper 逐层推进、CP-030 双确定性缺陷真修、CP-029 收口）；人工点登记：CP-003 Node 26 全量 e2e（.temp/node22.bak 待删）、CP-046 负向验证、CP-029 定责复核 |
| S04 | 已完成 | ce1acb1 | 2026-09-08 | 7 项全绿（fix-loop 1 轮：CP-014 AEL reparse 条目身份回归修复 + CP-005 测试名去 poison + CP-010/043 fmt 与 mock）；人工点登记：CP-010 Win10 toast 实测 |
| S05 | 已完成 | 1e97cc1 | 2026-09-08 | 6 项断言全绿（verify agent 测试结果缺口由主 agent 补跑闭环：cargo 785/L2 3163/L3 142 全绿）；执行期破「import 零改动」前提（12 处 import 源改指顶层）已留痕 commit body |
| S06 | 已完成 | 872e2ee | 2026-09-08 | CP-007 门槛结论：实测中位 180.33ms 越 50ms → 指纹分支（数字入 commit body）；fix-loop 1 轮（CP-006-b 三 L2 契约缺口 + CP-007-a 基准防 flaky 加固）；人工点登记：CP-007 数字合理性确认 |
| S07 | 已完成 | dee4aee | 2026-09-08 | 6 项全绿（fix-loop 1 轮：CP-019 jsdom RO mock 装配适配 35 例 + CP-016 restore 竞态渲染落定队列修复 2 例）；页删除显式不守卫复核结论登记 workspace/CLAUDE.md |
| S08 | 未开始 | — | — | |
| S09 | 未开始 | — | — | |
| S10 | 未开始 | — | — | spike go/no-go 记此 |
| S11 | 未开始 | — | — | |
| S12 | 未开始 | — | — | 人工验证点总收清单见 stages.md S12 |
