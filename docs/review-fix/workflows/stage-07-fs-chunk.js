// =====================================================================
// stage-07-fs-chunk.js — S07 fs_read_file 分块（BE-03）
// =====================================================================
// 跨边界契约（写死）：
//   后端：fs_read_file(path: String, onChunk: Channel<FsReadChunk>) -> Result<(), AppError>
//   FsReadChunk { data: String, done: bool }；块 256KB；先 metadata 校验 ≤10MB
//   （超限 Err，行为同现状）；发送序列 = 若干 {data, done:false} + 终态 {data:"", done:true}。
//   前端：readFile(path: string): Promise<string> 签名不变——src/ipc/fs.ts 内部
//   Channel 监听拼接 resolve 完整字符串；消费方（DiffPanel/HtmlPanel/useCodeMirror）零适配。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage07-fs-chunk',
  description: 'Stage 07: fs_read_file 改 Channel 分块推送削大文件内存/IPC 峰值（BE-03，D3）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S07 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：fs_read_file(path, onChunk: Channel<FsReadChunk>)；FsReadChunk { data, done }；块 256KB；≤10MB 上限不变；发送序列 = 若干 done:false + 终态 {data:"", done:true}；前端 readFile(path): Promise<string> 签名不变（内部拼接，消费方零适配）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'fs-backend',
    prompt: `你负责 BE-03（后端侧），只许改 src-tauri/src/fs/mod.rs：
fs_read_file（:64）当前 read_to_string 一次性读入，大文件内存/IPC 峰值高。按契约改 Channel 分块推送：
- 签名：fs_read_file(path: String, onChunk: Channel<FsReadChunk>) -> Result<(), AppError>（tauri::ipc::Channel，参照 pty 模块 Channel 用法）
- FsReadChunk { data: String, done: bool }（serde camelCase 与前端对齐）
- 先 metadata 校验大小 ≤10MB（超限 Err，文案与现状一致）
- 按 256KB 分块读取发送；【UTF-8 边界红线】按字节读 256KB 后必须回退到 char boundary 再转 String（多字节字符跨块禁止切散），发送序列 = 若干 {data, done:false} + 终态 {data:"", done:true}
补 L1 测试：多块文件拼接还原一致（含多字节字符跨界用例）、超限拒绝、空文件直接终态。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'fs-frontend',
    prompt: `你负责 BE-03（前端侧），只许改 src/ipc/fs.ts、src/__tests__/ipc-contract.test.ts 及 src/__tests__/ 下相关测试：
按契约适配：readFile(path: string): Promise<string> 签名不变——内部 new Channel<FsReadChunk>() 随 invoke 传入，onmessage 累积 data，done:true 时 resolve 拼接后的完整字符串；invoke 失败/错误 reject。FsReadChunk 类型加入 src/ipc/fs.ts 或 types（遵循既有 DTO 位置惯例）。
ipc-contract.test.ts 更新为新 payload 形态（mock Channel 回调驱动 done 序列）；readFile 相关 L2 用例适配。
【红线】DiffPanel.tsx / HtmlPanel.tsx / useCodeMirror.ts 三消费方零改动（签名不变保证）。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 07 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-07.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { refactorResults, testResult, verifyResult }
