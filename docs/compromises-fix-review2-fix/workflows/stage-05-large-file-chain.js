// =====================================================================
// Stage 05 大文件链路（FE-07/09/10 → FE-08，pipeline 串行组）
// 串行形态：用户并发限制约束——agents for 循环顺序 await，不用 parallel()；
// 全量测试命令逐条串行（CP-007 基准负载敏感）。
// fix-loop 调用本 Stage 时 args.constraints 传空串（本 Stage 无特殊纪律）。
// =====================================================================

export const meta = {
  name: 'stage-05-large-file-chain',
  description: 'Stage 05 大文件链路：块缓存 inflight 前缀清除/scannedBlocks 暴露+ref 转发/外部变更指纹比对/CM 10MB 预检',
  phases: [
    { title: '串行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点唯一真值源 = docs/compromises-fix-review2-fix/checklist.md 对应 ID 条目（先读再动手）——条目为六段式（位置/现状/修复步骤含可照抄代码块/测试同步/文档同步/验证），修复步骤段的代码块照抄适配，不自行设计；每项完成后跑条目「验证」段断言自查。`

// === Phase 1: 串行修复（用户并发限制——顺序 await，禁 parallel）===
phase('串行修复')
const agentsSeq = [
  {
    label: 'largefile-viewer',
    prompt: `你负责 FE-07、FE-09、FE-10（同链路，先读 checklist 对应条目再动手）：
【FE-07】src/panels/editor/largeFileViewer/blockCache.ts invalidateFile（:95-103）补 inflight 前缀清除循环（checklist 代码块照抄）——防止失效后 inflight 竞态回填旧块。
【FE-09】src/panels/editor/largeFileViewer/useLineIndex.ts:339-344 return 对象加 scannedBlocks: wsRef.current.nextBlock（fatalError 渲染期直读先例）；src/panels/editor/largeFileViewer/LargeFileViewer.tsx scannedBlocksRef 渲染期同步转发（防闭包过期）；首挂 stat then 内 scannedBlocks>0 → invalidateFile + setFileRev（checklist 代码块照抄，fileRev+1 归零、effect deps 仅 [filePath]——无循环论证）。
【FE-10】LargeFileViewer.tsx sampleFingerprint（首/中/末各 4KB 拼接文本直接相等比对）+ baseMetaRef 扩 fingerprint 字段 + 文件变更事件比对双层（mtime/size 变→直接失效；未变→指纹复核）（checklist 代码块照抄）。
【测试同步】checklist 三项「测试同步」段补例全部落地（src/__tests__/large-file-viewer.test.tsx；既有 :284 例「mtime/size 未变不失效」夹具适配指纹取样调用——mock readFileRange 按 offset 分派）。
【文档同步】checklist 三项「文档同步」段口径统一落 src/panels/editor/CLAUDE.md 大文件节（照抄）。
触碰文件仅限：src/panels/editor/largeFileViewer/blockCache.ts、src/panels/editor/largeFileViewer/useLineIndex.ts、src/panels/editor/largeFileViewer/LargeFileViewer.tsx、src/__tests__/large-file-viewer.test.tsx、src/panels/editor/CLAUDE.md。
自查：npx vitest run large-file-viewer 绿（含新例）。`,
  },
  {
    label: 'codemirror-reload',
    prompt: `你负责 FE-08（先读 checklist 对应条目再动手；必须在 largefile-viewer 组完成后才开始——本 agent 即后续组）：
【FE-08】src/panels/editor/useCodeMirror.ts applyExternalChange（:546-602）补 statFile 预检（size > MAX → filePathRef=undefined + setLargeFile 跳大文件查看器，不走全量读入）+ 读后复核（读后 size 复校）；脏分支弹窗先于 stat 的顺序不动（checklist 代码块照抄，注意保持既有顺序语义）。
【测试同步】checklist FE-08「测试同步」段补例落地（src/__tests__/use-code-mirror-reload-error.test.ts：越界预检 / 读后复核两例）。
【文档同步】checklist FE-08「文档同步」段口径落 src/panels/editor/CLAUDE.md。
触碰文件仅限：src/panels/editor/useCodeMirror.ts、src/__tests__/use-code-mirror-reload-error.test.ts、src/panels/editor/CLAUDE.md。
自查：npx vitest run use-code-mirror 绿（含新例）。`,
  },
]
const refactorResults = []
for (const a of agentsSeq) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break // 前序失败短路——FE-08 依赖大文件链路落地，不跑下游
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行执行，禁并行——CP-007 基准负载敏感）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行，禁止并行**（CP-007 基准负载敏感，并行抢核曾致误红）：
1. npx tsc --noEmit
2. npx eslint src/
3. npx vitest run large-file-viewer
4. npx vitest run use-code-mirror
5. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review2-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { refactorResults, testResult, verifyResult }
