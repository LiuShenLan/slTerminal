// =====================================================================
// stage-10-dto.js — S10 DTO 契约修正（FE-12/13/14、BE-18）
// =====================================================================
// 跨边界契约（写死）：
//   DirEntry.size: number | null; modified: number | null（运行时实为 null 非 undefined）
//   FsEventPayload.detail: string（Rust 必填，去 ?）
//   HooksLayer = "user" | "project" | "local"
//   pty_spawn wrapper 前置校验 cols/rows ∈ 1..=32767，越界抛错不 invoke
//   后端 Layer 枚举 User/Project/Local（serde snake_case），parse_layer 返回枚举
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage10-dto',
  description: 'Stage 10: DirEntry/detail DTO 对齐 Rust 真实形态 + HooksLayer 收窄 + pty 参数前置校验 + 后端 Layer 枚举（FE-12/13/14、BE-18）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S10 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：DirEntry.size/modified 为 number | null；FsEventPayload.detail 必填 string；HooksLayer = "user"|"project"|"local"；pty_spawn wrapper 校验 cols/rows ∈ 1..=32767；后端 Layer 枚举 User/Project/Local（serde snake_case）。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'dto-frontend',
    prompt: `你负责 FE-12、FE-13、FE-14，只许改 src/types/ 下文件、src/ipc/pty.ts 及 grep 发现的 DirEntry 消费方（含 src/__tests__/ 测试工厂）：
【FE-12】src/types/fs.ts:12-14 DirEntry.size/modified 声明 ?: number 但运行时实为 null——改 size: number | null; modified: number | null；grep -rn "\\.size\\b\\|\\.modified\\b" src/ 全部消费方（explorer 排序/显示、测试工厂）适配 null 语义。
【FE-13】src/types/notify.ts:5 FsEventPayload.detail 去 ? 改必填 string；消费方适配。
【FE-14】src/types/hooksConfig.ts:9 HooksLayer 收窄 "user" | "project" | "local"（当前仅 claude 三层，未来 CLI 加层再泛化——types/CLAUDE.md 登记在 S19）；src/ipc/pty.ts spawn wrapper 加 cols/rows 1..=32767 前置校验（越界抛错不 invoke）；src/types/pty.ts、agentHistory.ts、agent.ts 中 u64 对应字段加注释「Rust u64 → JS number，安全整数范围（< 2^53）约定」。
补 L2 测试：null DirEntry 渲染/排序、cols/rows 越界拒绝。
只做 npx tsc --noEmit 编译级检查，禁止 npm test。`
  },
  {
    label: 'dto-backend',
    prompt: `你负责 BE-18，只许改 src-tauri/src/hooks/claude/config.rs：
hooks 配置 Rust 端 serde_json::Value 无 DTO。补：Layer 枚举（User/Project/Local，serde rename_all snake_case），parse_layer 改返回枚举；hooks 子树结构体（serde 反序列化校验形态——事件名到 handler 数组的映射骨架，与 S17 SEC-05 语义校验共用此结构，本 Stage 只建结构不加校验规则）。
补 L1 测试：Layer 枚举序列化/反序列化（"user"/"project"/"local"）、非法 layer 拒绝、hooks 子树结构体形态校验。
只做 cargo check 编译级检查，禁止 cargo test。`
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
逐项检查 Stage 10 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-10.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
