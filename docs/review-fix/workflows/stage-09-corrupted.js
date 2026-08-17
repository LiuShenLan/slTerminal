// =====================================================================
// stage-09-corrupted.js — S09 corrupted 契约 + 持久化加固（BE-14/16、FE-11、SEC-11）
// =====================================================================
// 跨边界契约（写死）：
//   load_settings() -> { data: Value | null, corrupted: bool }
//     ——无文件 = data:null, corrupted:false；解析失败回退 = data:默认, corrupted:true；
//       .bak 命中也算 corrupted:true。
//   load_projects() -> { data: String, corrupted: bool }（data 为 JSON 字符串，形态同现状）。
//   前端 wrapper：loadSettings(): Promise<{ data: Record<string, unknown> | null;
//   corrupted: boolean }>；loadProjects(): Promise<{ data: string; corrupted: boolean }>。
//   保存侧：大小上限 1MB；settings 顶层键白名单 = fontSize | keybindings | sideBar | colorScheme。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage09-corrupted',
  description: 'Stage 09: load 返回 corrupted 标志 + app_dir 模块上提 + 保存大小/schema 校验（BE-14/16、FE-11、SEC-11，D11）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S09 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：load_settings() -> { data: Value|null, corrupted: bool }（无文件 data:null/corrupted:false；解析失败回退 data:默认/corrupted:true；.bak 命中 corrupted:true）；load_projects() -> { data: String, corrupted: bool }；前端 loadSettings(): Promise<{ data: Record<string, unknown>|null; corrupted: boolean }>、loadProjects(): Promise<{ data: string; corrupted: boolean }>；保存侧 1MB 上限 + settings 顶层键白名单 fontSize/keybindings/sideBar/colorScheme。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'persist-backend',
    prompt: `你负责 BE-14、BE-16、SEC-11，只许改 src-tauri/src/settings.rs、src-tauri/src/projects.rs、新建 src-tauri/src/app_dir.rs、改 src-tauri/src/lib.rs：
【BE-16】app_data_dir/resolve_app_data_dir 从 settings.rs 上提到新顶层模块 src-tauri/src/app_dir.rs（projects.rs 直接导入 settings:: 违反约束 #2）；settings/projects 均从 app_dir 导入；lib.rs 加 mod app_dir;。
【BE-14】load_settings/load_projects 按契约返回 { data, corrupted } 结构（serde camelCase）：无文件 data 默认/corrupted:false；JSON 解析失败回退默认值 corrupted:true；.bak 兜底逻辑保留且 bak 命中也 corrupted:true。
【SEC-11】save_settings/save_projects 加大小上限 1MB + settings 顶层键白名单校验（fontSize/keybindings/sideBar/colorScheme）+ projects 结构校验（须为 JSON 对象），超限/非法返回 AppError::Validation。
补 L1 测试：corrupted 三态（无文件/损坏/bak 命中）、大小上限拒绝、非法顶层键拒绝、app_dir 导入路径正确。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'persist-frontend',
    prompt: `你负责 FE-11，只许改 src/ipc/settings.ts、src/ipc/projects.ts、src/stores/projects.ts、src/stores/fontSize.ts、src/stores/keybindings.ts、src/stores/sideBar.ts、src/main.tsx 及 src/__tests__/ 下对应测试：
按契约适配 wrapper 返回结构：loadSettings(): Promise<{ data: Record<string, unknown>|null; corrupted: boolean }>；loadProjects(): Promise<{ data: string; corrupted: boolean }>。
四 store（projects/fontSize/keybindings/sideBar）loadFromDisk 消费 corrupted → toast.show("warning", "配置已损坏，已回退默认值")（toast 从 src/lib 导入）。
main.tsx:38 早期 loadSettings 调用适配新返回结构（启动早期 toast 未挂载，corrupted 时 console.warn 带模块名）。
grep 两 wrapper 全部消费方（含 src/__tests__/）逐一适配。补 L2 测试：corrupted:true 时各 store toast 断言、data 正常消费。
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
逐项检查 Stage 09 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
