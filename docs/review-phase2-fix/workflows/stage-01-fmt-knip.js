// =====================================================================
// Stage 01：基线 fmt + knip 零误报（TE-16、TE-12、TE-13）
// 编排：pipeline 串行 A(fmt) → B(knip)（knip 迭代依赖 fmt 后稳定基线，B 收尾跑全门禁）
// verify 真值源：docs/review-phase2-fix/workflows/verify/stage-01.md
// =====================================================================

export const meta = {
  name: 'stage01-fmt-knip',
  description: 'S01 基线 fmt 修复 + knip 零误报配置（TE-16/12/13）',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 \`docs/review-phase2-fix/checklist.md\` 对应 ID 条目（先读再动手——条目内含精确 file:line、现状描述与处置规则）。`

// === Phase 1: 串行重构 ===
phase('串行重构')
const sequentialAgents = [
  {
    label: 'A-fmt',
    prompt: `你负责【TE-16】cargo fmt 基线修复。先读 \`docs/review-phase2-fix/checklist.md\` 第 1 节 TE-16 条目。

步骤：
1. 仓库根执行 \`cargo fmt --manifest-path src-tauri/Cargo.toml\`——仅格式化，零逻辑改动，禁止顺手改任何代码
2. 执行 \`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check\` 确认退出码 0
3. 用 \`git diff --stat\` 确认仅 src-tauri/src/pty/shell.rs 与 src-tauri/src/pty/spawn.rs 两文件变化

完成后报告：格式化的文件清单 + fmt --check 退出码。`,
  },
  {
    label: 'B-knip',
    prompt: `你负责【TE-12】knip 零误报 +【TE-13】CI 门禁核对。先读 \`docs/review-phase2-fix/checklist.md\` 第 1 节 TE-12/TE-13 条目（含迭代法五步与分类处置规则）。

【TE-12】迭代法（每轮改一处跑一次）：
1. 跑 \`npx knip --production\` 收集输出，按 unused files / unused exports / unused dependencies 分类
2. unused files → 入 knip.json 的 entry：本项目注册表家族（硬约束 #13）靠 side-effect import 触发注册，触发点对 knip 不可见（已知形态：src/features/sideViews/sideViewDefs.ts、src/features/cliProfiles/profiles/**/index.ts、src/features/fileViewers/ 注册触发点、src/theme/schemes/ 注册触发点——以实际输出逐一核对）
3. unused exports → 入 ignoreExports：仅限文件内有「测试专用」注释的导出（已知：src/ipc/index.ts 的 ping、src/features/agentHistory/restoreSession.ts 的 waitFor）；无注释的 unused export 不得直接 ignore——判断是否真死代码，真死代码删除之并在报告中备注
4. unused dependencies → 判断：真实未用从 package.json 移除；工具链隐式使用（如 @wdio/* 先例）入 ignoreDependencies
5. 重复 1-4 至 \`npx knip --production\` 退出码 0

【TE-13】核对 \`.github/workflows/ci.yml\` 含 \`run: npx knip --production\`（应已存在，零改动，仅确认并报告行号）。

纪律：knip.json 为主要改动目标（除确认真死代码的删除）；不窄化 CI 口径。
完成后报告：knip.json 最终各类条目数与理由 + 删除的真死代码清单（若有）+ 最终 knip 退出码 + ci.yml 核对结果。`,
  },
]
const sequentialResults = []
for (const a of sequentialAgents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break // 前序失败短路，不跑下游
  sequentialResults.push(r)
}

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
2. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
3. npx tsc --noEmit
4. npx eslint src/
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
6. npm test
7. npx knip --production
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 1 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-phase2-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

// agent() 未返回（被跳过/API 错误）时返回 null——必须兜底
const verifyResult = rawVerify ?? { allFixed: false, failedItems: ['verify-agent-no-return'], details: { 'verify-agent-no-return': { status: 'not_fixed', evidence: 'verify agent 未返回（被跳过或 API 错误）' } } }

return { sequentialResults, testResult, verifyResult }
