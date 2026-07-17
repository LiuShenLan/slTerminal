# slTerminal 重构执行计划（Step 5 执行 + Step 6 收尾）

> 前置产物：`docs/review/`（7 份 review 报告，86 条原始发现）→ `refactoring-checklist.md`（去重后 77 项，P0-P4）→ `refactoring-stages.md`（11 个 Stage 划分）。
> 本计划只规定**如何执行**，不重复各 Stage 的改动细节（细节见 `refactoring-stages.md`）。

## 0. 执行前准备（一次性，必须先做）

1. **归档已有产物**：`docs/` 当前为未跟踪状态，先整体提交，避免混入后续 Stage commit：
   ```bash
   git add docs/ && git commit -m "docs: 重构 review 报告与执行计划归档"
   ```
2. **脚本语法预检**：12 个 workflow 脚本在首次 Workflow 调用前做语法检查。注意脚本含顶层 `return`（Workflow 运行时特性），标准 `node --check` 不适用——用 Git Bash 执行以下命令（已验证可行）：
   ```bash
   for f in docs/refacotr/workflows/*.js; do
     node -e 'const fs=require("fs");const src=fs.readFileSync(process.argv[1],"utf8").replace(/^export\s+const\s+meta/m,"const meta");try{new Function("agent","parallel","pipeline","phase","log","args","budget","workflow","meta",`"use strict"; return (async()=>{ ${src} \n})`);console.log("OK: "+process.argv[1])}catch(e){console.log("FAIL: "+process.argv[1]+" -- "+e.message)}' "$f"
   done
   ```
   任一失败先修脚本再开始 Stage 1。
3. **确认工作区干净**：`git status --porcelain` 除预期外无残留。

## 1. 执行总则

- 主 agent 逐 Stage 串行执行，**严格按 Stage 1→11 顺序**，不跳阶段。
- 每个 Stage 一个 Workflow 脚本（`workflows/stage-NN-*.js`），用 `Workflow({ scriptPath })` 直接调用，不复制脚本内容内联。
- 每个 Stage 通过后立即 commit，message 一律以 refactoring-stages.md 中该 Stage 的原文为准（Stage 11 为 `docs:` 前缀，其余为 `refactor:`）。
- 直接 main 分支操作，不用 worktree。
- 修复循环最多 **3 轮**；第 3 轮仍 `allFixed=false` → 停止执行（现场处置见 §5 规则），输出人工介入报告，等用户决策。
- 执行中**不修改任何 workflow 脚本与 verify 文件**；发现缺陷先报告用户。
- **Stage 中断续跑**：Workflow 因 API 错误等非脚本原因中断时，优先用 `Workflow({ scriptPath, resumeFromRunId })` 续跑（已完成 agent 命中缓存），不重新从头调用。重新调用仅限脚本内容已修改的场景。
- **时间盒**：Workflow 后台运行期间主 agent 每 15-20 分钟查看一次 /workflows 进度；单 Stage 超过 60 分钟无进展 → TaskStop 该 Workflow，报告用户后决定重试或人工介入。
- **全局暂停/恢复**：任何时刻可中断本计划；恢复时读 §8 进度表，从首个非"完成"行的 Stage 继续（其前 commit 已落盘，无需重做）。
- **成本预期**：单 Stage 约 30-60 分钟（全量测试为主），11 Stage + 修复余量总计约 8-14 小时墙钟。
- **回滚**：某 Stage commit 后发现严重问题（如收尾 L4 暴露 Stage 2 回归）→ `git revert` 该 Stage commit 并在收尾报告记录；后续 Stage 对其有依赖的，一并报告用户决策，不静默继续。

## 2. 公共约束（所有执行 agent 必须遵守，已写入各脚本 PREAMBLE）

| 约束 | 内容 |
|------|------|
| surgical changes | 只改分配的文件/项，不顺手改无关代码；发现无关死代码只报告不删 |
| 中文注释 | 代码注释一律中文，遵循语言注释规范 |
| 项目禁区 | `compute_conpty_flags` 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试） |
| 并行隔离 | 同一 Stage 内并行 agent 文件不重叠（分工表见 refactoring-stages.md 与各脚本头部注释） |
| 测试纪律 | L1 必须 `--test-threads=1`；前端新测试遵循 `src/__tests__/` 的 `vi.hoisted()` mock 模式；git 测试 tempdir 必须 `dunce::canonicalize` |
| 测试专用 Stage | Stage 9/10 只改测试不改生产代码；fix-loop 经 `constraints` 参数透传此纪律；测试暴露的真实 bug 只报告，交用户决策 |
| 文档同步 | 固定最后 Stage（Stage 11），反映所有代码 Stage 完成后的最终状态 |

## 3. 全量验证命令（唯一权威源）

本节是全量测试命令的**唯一权威源**；12 个脚本内的测试 agent prompt 均为本节副本。命令变更时先改本节，再同步全部脚本。

```bash
npx tsc --noEmit
npx eslint src/
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
npm test
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
# Stage 10 额外：npm run test:l3
```

各脚本已指示测试 agent **并行启动**这些命令（相互独立），收集全部结果。

## 4. 脚本与断言文件清单

| # | 文件 | 对应 Stage | 并行结构 |
|---|------|-----------|---------|
| 1 | `workflows/stage-01-sandbox.js` + `verify/stage-01.md` | Stage 1 路径沙箱启用（SEC-01/SEC-06/DOC-05c） | 2 并行（后端 / 前端，set_project_root 契约写死） |
| 2 | `workflows/stage-02-pty-async.js` + `verify/stage-02.md` | Stage 2 PTY 异步化 + shell 白名单（BE-01/02/11/12/14、SEC-02/07/08） | 串行 2 agent（shell 先行；失败短路不跑下游） |
| 3 | `workflows/stage-03-spawn-hardening.js` + `verify/stage-03.md` | Stage 3 ConPTY 加固 + reader 跨边界（BE-07~10/13/16） | 2 并行（spawn.rs / reader.rs） |
| 4 | `workflows/stage-04-backend-modules.js` + `verify/stage-04.md` | Stage 4 fs/git/settings/notify（SEC-04/05、BE-03/05/06/15、TE-13/14） | 3 并行 |
| 5 | `workflows/stage-05-frontend-critical.js` + `verify/stage-05.md` | Stage 5 编辑器竞态 + HTML 注入（FE-01/02/19、SEC-03） | 2 并行 |
| 6 | `workflows/stage-06-terminal-hooks.js` + `verify/stage-06.md` | Stage 6 会话元数据单点（FE-23/08/12/14/15/17、TE-16） | 2 并行 |
| 7 | `workflows/stage-07-frontend-mid.js` + `verify/stage-07.md` | Stage 7 前端生命周期（FE-03~07/09/10/16/18/22、DOC-10） | 5 并行 |
| 8 | `workflows/stage-08-frontend-cleanup.js` + `verify/stage-08.md` | Stage 8 配色单点 + 清理（FE-11/13/20/21/24） | 5 并行（token 命名写死于脚本头） |
| 9 | `workflows/stage-09-core-tests.js` + `verify/stage-09.md` | Stage 9 核心补测（TE-01/02/03/05/06/07） | 5 并行（只改测试） |
| 10 | `workflows/stage-10-test-quality.js` + `verify/stage-10.md` | Stage 10 测试整改（TE-04/08/09/10/11/12） | 4 并行（只改测试，含 L3） |
| 11 | `workflows/stage-11-docs-sync.js` + `verify/stage-11.md` | Stage 11 文档同步（DOC-01~09/11/12/13、TE-17） | 3 并行（只改文档） |
| 12 | `workflows/fix-loop.js` | 修复循环（任意 Stage 复用） | 单 agent，参数化 args |

**verify 断言文件是验证唯一真值源**：Stage 脚本与 fix-loop 的 verify agent 都读同一份 `verify/stage-NN.md`，保证修复循环与 Stage 验证同一标尺。断言变更时改 verify 文件，两处自动生效。`refactoring-stages.md` 中的「验证项」仅作历史参考，不回改；执行期一切以 verify/ 文件为准。

## 5. 主 agent 执行流程（伪代码）

```
for stage in 1..11:
    result = Workflow({ scriptPath: workflows/stage-NN-*.js })
    fixed = result.verifyResult.allFixed
    rounds = 0
    # no-return 分流：agent 未返回属基础设施问题（API 错误/被跳过），不是代码缺陷
    if not fixed and failedItems 全为 *-no-return:
        判断 no-return 发生的阶段：
          - verify 阶段（verify-agent-no-return）：主 agent 直接 inline spawn 单个 verify agent
            （复用该 Stage 脚本中的 verify prompt + schema + verify/stage-NN.md + testResult），
            结果并入 fixed 判断——不重启 Workflow（已完成的 run resume 会全命中缓存，无效）
          - 重构/测试阶段（如 shell-agent-no-return）：TaskStop 原 run（若仍在跑）→ resumeFromRunId 续跑
            （缓存前缀有效，仅死去的 agent 重跑）；仍失败则停止并报告用户
    while not fixed and rounds < 3:
        rounds += 1
        fixResult = Workflow({
            scriptPath: workflows/fix-loop.js,
            args: {
                stage: N,
                failedItems: <上一轮 verify 的 failedItems（剔除 *-no-return 项）>,
                fixContext: JSON.stringify(<上一轮 verify 的 details>),
                verifyFile: "docs/refacotr/workflows/verify/stage-NN.md",
                constraints: <Stage 9/10 传 "本 Stage 只改测试，禁止改生产代码"，其余传空——值见对应脚本头注释>
            }
        })
        fixed = fixResult.verifyResult.allFixed
    if not fixed:
        # 现场处置：保留工作区改动（不 stash 不丢弃），便于人工接手
        停止执行 → 输出人工介入报告（failedItems + details + 已尝试轮次 + 工作区状态）→ 等用户
    git add src/ src-tauri/ e2e-tests/ test/ .claude/CLAUDE.md .claude/test-inventory.md
    git status 检查暂存区：
      - 只有本 Stage 预期文件 → 正常 commit
      - 发现预期外改动 → 不 commit，先甄别：agent 越界改动 → git checkout -- <file> 丢弃并记入报告；
        属合理遗漏（agent 已修但未列入断言）→ 并入 fix-loop 处理
    git commit "refactor: StageN — <描述>"（rounds > 0 时末尾附 "（含 N 轮修复）"）
    更新 §8 进度跟踪表
```

要点：
- **commit 用路径限定 `git add`**（不用 `-A`）：docs/ 的进度表更新与其他杂项不混入 Stage commit。
- 脚本中断优先 `resumeFromRunId` 续跑（见 §1）。
- 每 Stage 结束后核对 `git status` 无意外残留。

## 6. 高风险点与人工验证点

| 位置 | 风险 | 处置 |
|------|------|------|
| Stage 1 | 启用真沙箱是行为变更——sandbox 外路径从此被拒 | 用户已决策启用（见 checklist 范围决策）；前后端 2 并行接入，契约写死于脚本头 |
| Stage 2 | pty 四命令异步化改动核心会话路径 | shell-agent 失败短路；Stage 内全量 L1 + 收尾 L4 实测 claude |
| Stage 5 | SEC-03 的 `e.origin === "null"` 为规范推断，无法实测 | 代码注释明确记录假设；正确性由收尾 L4 验证，失败则回滚重议 |
| Stage 6 | 约束 #8 单点化触碰 useXterm 编排层 | use-xterm 系列 108+ 条测试为门禁；收尾人工实测终端 |
| Stage 8 | 5 并行下 sidebar/filetree 引用的 token 可能暂未导出 | agent 只跑 vitest 不单独 tsc，全量 tsc 由统一测试阶段兜底 |
| Stage 11 | 文档同步依赖 git log 还原前 10 个 Stage 实况 | agent prompt 要求先读 git log 再动笔，防凭记忆写文档 |

## 7. Step 6 收尾（全部 Stage commit 后执行）

1. **最终全量构建验证**（超出每 Stage 的 L1/L2 门禁）：
   ```bash
   npx tsc --noEmit && npx eslint src/
   cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
   npm test
   cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
   npm run test:l3
   npm run e2e    # = build:e2e + wdio，必须 VITE_E2E=1
   ```
2. **根 CLAUDE.md 抽查**：对照最终代码核对全局约束与模块索引（Stage 11 已做一轮，此处抽查确认）。
3. **人工实测**（自动化无法覆盖）：`npm run tauri dev` 起真实 app → 跑真实 `claude` TUI → 验证鼠标滚轮、中文 IME、Ctrl+C 中断、页面切换终端存活（H6）。
4. **归档 commit**：进度跟踪表最终状态与执行中产生的文档变更一并提交：`git add docs/ && git commit -m "docs: 重构执行完成——进度与收尾归档"`。
5. **收尾报告**输出给用户：11 个 Stage commit 列表、各层最终测试用例数（L1/L2/L3/L4）、未修复项（若有，含原因与建议）。

## 8. 进度跟踪表（执行时逐行更新）

| Stage | Workflow | 状态 | 修复轮次 | Commit |
|-------|----------|------|---------|--------|
| 1 沙箱启用 | stage-01-sandbox.js | 未开始 | — | — |
| 2 PTY 异步化 | stage-02-pty-async.js | 未开始 | — | — |
| 3 ConPTY 加固 | stage-03-spawn-hardening.js | 未开始 | — | — |
| 4 后端模块 | stage-04-backend-modules.js | 未开始 | — | — |
| 5 前端高危 | stage-05-frontend-critical.js | 未开始 | — | — |
| 6 终端 hook | stage-06-terminal-hooks.js | 未开始 | — | — |
| 7 前端中危 | stage-07-frontend-mid.js | 未开始 | — | — |
| 8 配色+清理 | stage-08-frontend-cleanup.js | 未开始 | — | — |
| 9 核心补测 | stage-09-core-tests.js | 未开始 | — | — |
| 10 测试整改 | stage-10-test-quality.js | 未开始 | — | — |
| 11 文档同步 | stage-11-docs-sync.js | 未开始 | — | — |
| 收尾 | Step 6 | 未开始 | — | — |
