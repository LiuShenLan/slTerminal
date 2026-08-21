// =====================================================================
// Stage 04：root 竞态 Mutex（SEC-16）
// 编排：单 agent（豁免：P1 核心修复强耦合单项独立 Stage——AppState 结构 + impl 签名 + Cargo feature 三处联动）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-04.md
// 人工验证点：A→B 快速连切页面沙箱不串（旧项目文件操作被拒）
// =====================================================================

export const meta = {
  name: 'stage04-root-mutex',
  description: 'S04 set_project_root tokio::Mutex 串行化（SEC-16）',
  phases: [
    { title: '重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line 与可照抄的代码块）。
测试纪律：本阶段禁止跑 cargo test（资源共享型，全量测试 agent 单点跑）；编译级自查用 \`cargo check --manifest-path src-tauri/Cargo.toml\`。`

// === Phase 1: 重构（单 agent）===
phase('重构')
const refactorResults = await parallel(
  [{
    label: 'A-mutex',
    prompt: `你负责【SEC-16】set_project_root tokio::Mutex 串行化（D17 决策）。先读 \`docs/review-phase2-fix/checklist.md\` 第 4 节 SEC-16 条目（六步，含可照抄代码块）。

触碰文件：\`src-tauri/Cargo.toml\`、\`src-tauri/src/state.rs\`、\`src-tauri/src/CLAUDE.md\`

步骤：
1. \`src-tauri/Cargo.toml\` 第 43 行 tokio features 改 \`["rt", "sync"]\`
2. \`src-tauri/src/state.rs\` AppState（约 :132-140）加字段 \`pub project_root_lock: tokio::sync::Mutex<()>\`（含 checklist 条目中的文档注释）；\`AppState::new\`（约 :148-157）对应初始化
3. \`set_project_root_impl\` 签名加 \`lock: &tokio::sync::Mutex<()>\` 参数；函数体首行 \`let _guard = lock.lock().await;\`——持锁至函数尾（canonicalize+apply 全程互斥）
4. 命令层 \`set_project_root\`（约 :249-251）调用改传 \`&state.project_root_lock\`
5. 测试适配：state.rs 现有直测 \`set_project_root_impl\` 的用例调用点补传 \`&tokio::sync::Mutex::new(())\`
6. 新增 L1 用例 \`set_project_root_serializes_concurrent_calls\`：tokio runtime 下并发 \`join!\` 两个 impl 调用（两个 tempdir 路径），断言：两调用均 Ok、最终 root 为其中之一且非 None、再顺序调用 B 后 root == B
7. \`src-tauri/src/CLAUDE.md\` state.rs 节补一句（AppState 字段清单加 project_root_lock + SEC-16 串行化语义）
8. \`cargo check --manifest-path src-tauri/Cargo.toml\` 编译通过

完成后报告：改动摘要 + 适配的既有用例清单 + 新增用例名。`,
  }].map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
3. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 4 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-04.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
