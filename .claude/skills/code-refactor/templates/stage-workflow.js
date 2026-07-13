// =====================================================================
// Stage Workflow 模板
// =====================================================================
// 用法: 读取此文件，替换 __PLACEHOLDER__ 后传给 Workflow({ script: ... })
//
// 占位符:
//   __STAGE_NAME__        - Workflow 名称 (e.g. "stage1-xxx")
//   __STAGE_DESCRIPTION__ - 阶段描述
//   __STAGE_NUMBER__      - 阶段编号
//   __PROJECT_ROOT__      - 项目根路径
//   __PARALLEL_AGENTS__   - 并行 agent 数组, JSON 格式: [{label, prompt}, ...]
//   __SEQUENTIAL_AGENTS__ - 串行 pipeline 阶段, JSON 格式: [{label, prompt}, ...]
//                           如无串行依赖, 替换为: []
//   __TEST_COMMANDS__     - 全量测试命令, 逗号分隔的 shell 命令字符串
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

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = __PARALLEL_AGENTS__;
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(a.prompt, { label: a.label }))
)

__SEQUENTIAL_PHASE_BLOCK__

// === Phase 3: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
  在项目根目录 __PROJECT_ROOT__ 执行全量验证：
  __TEST_COMMANDS__
  报告每个命令的通过/失败状态和详细错误信息。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证 ===
phase('逐项验证')
const verifyResult = await agent(`
  逐项检查 Stage__STAGE_NUMBER__ 的每个改动是否实际生效（项目路径 __PROJECT_ROOT__）：
  __VERIFY_ITEMS__
  返回 JSON: { "allFixed": true/false, "failedItems": ["未修复项ID列表"], "details": {"每项的检查结果"} }
`, { label: 'verify all items', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems']
}})

return { refactorResults, __SEQUENTIAL_RESULT__ testResult, verifyResult }


// =====================================================================
// 替换说明
// =====================================================================
//
// __SEQUENTIAL_PHASE__: 如果 __SEQUENTIAL_AGENTS__ 非空，替换为:
//   { title: '串行重构' },
//
// __SEQUENTIAL_PHASE_BLOCK__: 如果 __SEQUENTIAL_AGENTS__ 非空，替换为:
//   // === Phase 2: 串行重构（共享文件依赖）===
//   phase('串行重构')
//   const sequentialRefactors = await pipeline(
//     __SEQUENTIAL_AGENTS__.map(a => async () => await agent(a.prompt, { label: a.label }))
//   )
//
// __SEQUENTIAL_RESULT__: 如果有串行阶段加 "sequentialRefactors, " 否则留空
//
// __PARALLEL_AGENTS__ 示例:
//   [
//     { label: "Agent 1", prompt: "修改文件A: 具体改动描述..." },
//     { label: "Agent 2", prompt: "修改文件B: 具体改动描述..." }
//   ]
//
// __VERIFY_ITEMS__ 示例:
//   ID1: 检查某文件是否包含某内容
//   ID2: 检查某命令名是否已重命名
//   ...
//
// __TEST_COMMANDS__ 示例:
//   1. npx tsc --noEmit
//   2. npx eslint src/
//   3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
//   4. npm test
//   5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
//   报告所有结果。
// =====================================================================
