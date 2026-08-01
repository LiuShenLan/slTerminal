# Claude Code 历史会话查询与恢复 — 执行编排参数（execution-plan）

> 通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）单一真值源在 `/systematic-changes-execute`，本文只写任务特定参数。
> 命令/路径/禁区值**单处定义**于 `.claude/skills/systematic-changes-plan/config.json`，执行期从 config 读取，本文不复制值。

## 一、Stage 表

| Stage | Workflow 脚本 | Verify 断言 | 门禁（全量测试命令） | Commit message |
|-------|--------------|-------------|---------------------|----------------|
| 01 后端扫描 | `docs/claude-history-view/workflows/stage-01-backend-scan.js` | `docs/claude-history-view/workflows/verify/stage-01.md` | fullCheck（config.json `commands.fullCheck`） | `feat(claude-history): 后端历史会话扫描命令（轻量解析+标题回退链+env 覆盖）` |
| 02 后端写操作 | `docs/claude-history-view/workflows/stage-02-backend-ops.js` | `docs/claude-history-view/workflows/verify/stage-02.md` | fullCheck | `feat(claude-history): 后端删除/重命名命令（sessionId 校验+custom-title 追加写）` |
| 03 前端 IPC | `docs/claude-history-view/workflows/stage-03-frontend-ipc.js` | `docs/claude-history-view/workflows/verify/stage-03.md` | fullCheck | `feat(claude-history): 前端 DTO + IPC 封装与契约测试` |
| 04 数据层+编排 | `docs/claude-history-view/workflows/stage-04-frontend-data.js` | `docs/claude-history-view/workflows/verify/stage-04.md` | fullCheck | `feat(claude-history): 历史数据层（分组/搜索/⚡派生）+ 四步恢复编排` |
| 05 历史区 UI | `docs/claude-history-view/workflows/stage-05-frontend-ui.js` | `docs/claude-history-view/workflows/verify/stage-05.md` | fullCheck | `feat(claude-history): 历史区 UI（双行式/搜索/右键菜单）+ AgentStatusView 三下拉框` |
| 06 E2E | `docs/claude-history-view/workflows/stage-06-e2e.js` | `docs/claude-history-view/workflows/verify/stage-06.md` | fullCheck + `npm run build:e2e` + `npm run wdio`（本 Stage 专属实跑） | `test(claude-history): E2E fixture 与关键路径用例` |
| 07 文档 | `docs/claude-history-view/workflows/stage-07-docs.js` | `docs/claude-history-view/workflows/verify/stage-07.md` | fullCheck（防御性，代码零改动） | `docs(claude-history): 决策 22-26 回写 + 模块文档登记 + 用例清单同步` |

> Stage 06 门禁特例：`e2e-tests/run-wdio.cjs`、`test.e2e.ts` 不在根 tsconfig include，构建级兜底 = `npm run build:e2e`（含 vite build）+ `npm run wdio` 实跑；全量测试 agent 依次执行，耗时长属预期（cargo 锁排队勿中止）。

## 二、git add 路径（每 Stage commit 限定枚举）

从 config.json `workflow.gitAddPaths` 读取：`src/`、`src-tauri/`、`e2e-tests/`、`test/`、`.claude/CLAUDE.md`、`.claude/test-inventory.md`、`docs/`。

本任务各 Stage 预期落盘均在该枚举内；`.gitignore`（Stage 06）为根目录文件，补 `git add .gitignore` 单文件（白名单外精确文件，执行期逐 Stage 声明）。

## 三、fix-loop 调用规范

脚本：`docs/claude-history-view/workflows/fix-loop.js`（模板 fix-workflow.js 填充）。

```js
Workflow({
  scriptPath: "docs/claude-history-view/workflows/fix-loop.js",
  args: {
    stage: <Stage 编号 1-7>,
    failedItems: [<未通过项 ID 列表，来自该 Stage verifyResult>],
    fixContext: "<verify agent details 证据原文>",
    verifyFile: "docs/claude-history-view/workflows/verify/stage-0<N>.md",
    constraints: ""   // 恒空串，除三例外（与各 Stage 脚本头注释一致，见下）
  }
})
```

- `constraints` 三例外（取值单处定义在对应 Stage 脚本头注释，fix-loop.js 头注释同步）：
  - Stage 04 → `"本 Stage 不建 src/features/claudeHistory/index.ts（barrel 归 Stage 05）"`
  - Stage 06 → `"安全红线：任何用例不得触碰用户真实 ~/.claude/projects/，写操作只允许作用于 .tmp-claude-projects 副本"`
  - Stage 07 → `"纯文档 Stage：禁改任何 .ts/.tsx/.rs 代码文件"`
- **Stage 06 fix-loop 特例**：fix-loop 全量测试仅 5 条通用命令（不含 build:e2e/wdio）——Stage 06 的 fix-loop 全部通过后，主 agent 须补跑 `npm run build:e2e` + `npm run wdio` 实证 E2E 全绿方可 commit。
- `__CHECKLIST_FILE__` = `docs/claude-history-view/checklist.md`
- `__STAGES_FILE__` = `docs/claude-history-view/stages.md`
- 每 Stage 最多 3 轮 fix-loop（config.json `workflow.fixMaxRetries`）；仍不通过 → 上报用户决策。
- PREAMBLE 禁区文本：从 config.json `workflow.forbiddenZones` 读取（compute_conpty_flags 固定 0x7 禁改），各脚本已写入 PREAMBLE，fix-loop 同。

## 四、并行与测试纪律

- 并行 agent ≤5（config.json `workflow.maxParallelAgents`）；本任务最大并行 2（Stage 04/05）。
- 并行 agent 不跑资源共享型测试（PTY/端口/全局锁/cargo target 锁）——重构阶段只做编译级检查，真实执行统一由全量测试 agent 单点跑。
- L1 恒 `--test-threads=1`（ConPTY 死锁红线；BE-09 env 测试亦依赖串行）。
- Stage 06 wdio 单实例串行（wdio.conf.ts `maxInstances: 1` 现状，不改）。

## 五、进度跟踪表（执行期逐格填）

| Stage | 执行 | 全量测试 | verify | fix-loop 轮数 | commit | 状态 |
|-------|------|---------|--------|--------------|--------|------|
| 01 | ✓ | ✓（重跑 npm test 绿；bootstrap.test.ts 预存 flaky，非本 Stage 回归） | ✓ | 0 | 67573da | 完成 |
| 02 | ✓ | ✓ | ✓ | 0 | 3a58463 | 完成 |
| 03 | - | - | - | 0 | - | 待执行 |
| 04 | - | - | - | 0 | - | 待执行 |
| 05 | - | - | - | 0 | - | 待执行 |
| 06 | - | - | - | 0 | - | 待执行 |
| 07 | - | - | - | 0 | - | 待执行 |

## 六、收尾人工实测清单（stages.md「人工验证点」段同步）

全部 Stage 完成后，逐项人工实测并记录结果：
1. 真实 `claude --resume <id>` 跨项目恢复成功进入会话
2. pty.write 注入在 shell 慢启动下的可靠性
3. ⚡ 实时性（本应用 spawn 的会话标记出现/消除）
4. 重命名 custom-title 后官方 picker / `claude --resume <name>` 可见
5. 大文件（20MB+）与数百会话规模扫描耗时
6. 窄侧栏（250px）视觉效果
