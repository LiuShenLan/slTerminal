// =====================================================================
// Fix Workflow 模板 — 阶段验证不通过时的修复循环
// =====================================================================
// 用法: 读取此文件，替换 __PLACEHOLDER__ 后传给 Workflow({ script: ..., args: ... })
//
// 占位符:
//   __STAGE_NAME__        - Workflow 名称 (e.g. "stage1-fix")
//   __STAGE_DESCRIPTION__ - 修复描述
//   __STAGE_NUMBER__      - 阶段编号
//   __PROJECT_ROOT__      - 项目根路径
//   __TEST_COMMANDS__     - 全量测试命令, 逗号分隔
// =====================================================================

export const meta = {
  name: '__STAGE_NAME__',
  description: '__STAGE_DESCRIPTION__',
  phases: [
    { title: '修复问题' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

// 从 args 中接收上一轮验证失败的项列表
const failedItems = args.failedItems || []
const stageNumber = args.stage || '__STAGE_NUMBER__'
const fixPrompt = args.fixPrompt || ''

// === Phase 1: 修复问题 ===
phase('修复问题')
const fixResult = await agent(`
  针对 Stage${stageNumber} 验证失败的以下问题逐一修复（项目路径 __PROJECT_ROOT__）：
  ${JSON.stringify(failedItems)}

  ${fixPrompt}

  请阅读相关文件，找到问题根源，执行修复。
  确保修复不引入新问题（不改变其他已验证通过项的代码）。
`, { label: 'fix remaining issues' })

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
  在 __PROJECT_ROOT__ 执行全量验证：
  __TEST_COMMANDS__
  报告所有结果。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const verifyResult = await agent(`
  重新逐项检查之前失败的 ${JSON.stringify(failedItems)} 项是否已修复。
  返回: { "allFixed": true/false, "failedItems": [...], "details": {...} }
`, { label: 're-verify failed items', schema: {
  type: 'object',
  properties: {
    allFixed: { type: 'boolean' },
    failedItems: { type: 'array', items: { type: 'string' } },
    details: { type: 'object' }
  },
  required: ['allFixed', 'failedItems']
}})

return { fixResult, testResult, verifyResult }


// =====================================================================
// 主 agent 调用方式:
//   Workflow({
//     script: (替换后的脚本),
//     args: { stage: N, failedItems: ["ID1", "ID2"], fixPrompt: "..." }
//   })
// =====================================================================
