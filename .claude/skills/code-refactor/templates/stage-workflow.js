// =====================================================================
// Stage Workflow 模板（实战版）
// =====================================================================
// 用法: 读取此文件，替换 __PLACEHOLDER__，落盘 workflows/stage-NN-*.js，
//       语法预检（SKILL.md 5.1）后传给 Workflow({ scriptPath })
//
// 生成纪律（违反曾致脚本与 checklist 大面积错位）:
//   1. 逐 ID 对照 checklist 原文填写各 agent prompt，禁止凭记忆/凭摘要
//   2. 跨边界契约（IPC 命令名+签名、token 命名、数据结构字段）写死于
//      脚本头部注释，并行 agent 不各自推断
//   3. 若本 Stage 有特殊纪律（如"只改测试"），写进 PREAMBLE_EXTRA，
//      并在头部注释注明 fix-loop 调用时 args.constraints 应传的值
//
// 占位符:
//   __STAGE_NAME__        - Workflow 名称 (e.g. "stage1-xxx")
//   __STAGE_DESCRIPTION__ - 阶段描述
//   __STAGE_NUMBER__      - 阶段编号
//   __PROJECT_ROOT__      - 项目根路径
//   __FORBIDDEN_ZONES__   - 项目禁区文本（config.json forbiddenZones）
//   __PREAMBLE_EXTRA__    - Stage 特殊纪律（无可留空行）
//   __PARALLEL_AGENTS__   - 并行 agent 数组, JSON: [{label, prompt}, ...]
//   __SEQUENTIAL_AGENTS__ - 串行 pipeline 阶段（无则见文末替换说明）
//   __TEST_COMMANDS__     - 全量测试命令清单（config.json commands）
//   __VERIFY_FILE__       - 断言文件路径（workflows/verify/stage-NN.md）
// =====================================================================

export const meta = {
  name: '__STAGE_NAME__',
  description: '__STAGE_DESCRIPTION__',
  phases: [
    { title: '并行重构' },
    __SEQUENTIAL_PHASE__
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 __PROJECT_ROOT__。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：__FORBIDDEN_ZONES__
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。__PREAMBLE_EXTRA__`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试，见文末）===
phase('并行重构')
const parallelAgents = __PARALLEL_AGENTS__;
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

__SEQUENTIAL_PHASE_BLOCK__

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 __PROJECT_ROOT__ 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
__TEST_COMMANDS__
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：__VERIFY_FILE__）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage__STAGE_NUMBER__ 的改动是否实际生效（项目根 __PROJECT_ROOT__）。
先读 __VERIFY_FILE__ 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——测试类断言全部判 not_fixed）'}
---
返回 JSON：{ "allFixed": true/false, "failedItems": ["未通过项ID"], "details": { "<ID>": { "status": "fixed|not_fixed|partial", "evidence": "..." } } }
`, { label: 'verify all items', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems', 'details']
}})

// agent() 未返回（被跳过/API 错误）时返回 null——必须兜底，否则主 agent 拿到 undefined
const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, __SEQUENTIAL_RESULT__ testResult, verifyResult }


// =====================================================================
// 替换说明
// =====================================================================
//
// __SEQUENTIAL_PHASE__: 如果 __SEQUENTIAL_AGENTS__ 非空，替换为:
//   { title: '串行重构' },
//   否则删除该行（含占位符行）。
//
// __SEQUENTIAL_PHASE_BLOCK__: 如果 __SEQUENTIAL_AGENTS__ 非空，替换为:
//   // === Phase 2: 串行重构（共享文件依赖，前序 agent 的产出供后序使用）===
//   phase('串行重构')
//   const sequentialResults = []
//   for (const a of __SEQUENTIAL_AGENTS__) {
//     const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
//     if (!r) break  // 前序失败短路，不跑下游
//     sequentialResults.push(r)
//   }
//   否则删除该行（含占位符行）。
//
// __SEQUENTIAL_RESULT__: 有串行阶段加 "sequentialResults, "，否则删除。
//
// __PARALLEL_AGENTS__ 示例:
//   [
//     { label: "backend-agent", prompt: "你负责 ID-01/ID-02：\n【ID-01】..." },
//     { label: "frontend-agent", prompt: "你负责 ID-03：\n【ID-03】..." }
//   ]
//
// __TEST_COMMANDS__ 示例（逐行编号，与 verify 文件尾部一致）:
//   1. npx tsc --noEmit
//   2. npx eslint src/
//   3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
//   4. npm test
//   5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
//
// 并行 agent 测试纪律（重要）:
//   并行 agent 的 prompt 中禁止让其各自跑资源共享型测试（含 PTY spawn、
//   端口监听、全局锁的测试）——跨进程并发会死锁。重构阶段只做编译级
//   检查（如 cargo test --no-run），真实执行统一由全量测试 agent 单点跑。
// =====================================================================
