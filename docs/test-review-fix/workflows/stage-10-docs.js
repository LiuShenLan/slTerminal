// Stage 10：文档/inventory 收尾（TQ-CI-01, TQ-CI-02, TQ-CI-05, TQ-COV-02, TQ-E-10, TQ-L1-02, TQ-L1-04, TQ-L1-06 + 翻案留痕 TQ-CI-04/TQ-E-07 + 收尾验收）
// fix-loop 调用时 args.constraints 传空
export const meta = {
  name: 'stage-10-docs',
  description: 'Stage 10：文档/inventory 收尾 + 全量复跑验收（10 项 + 统一动作）',
  phases: [
    { title: '并行文档' },
    { title: '全量复跑' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；文档用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复细节先读 docs/test-review-fix/checklist.md 对应 ID 的六段式条目再动手。
本 Stage 只改文档（.claude/test-inventory.md 与各模块 CLAUDE.md），禁止改代码。`

phase('并行文档')
const parallelAgents = [
  { label: 'sync-inventory', prompt: '你负责 TQ-CI-01 + TQ-CI-02 + TQ-CI-05 + TQ-COV-02 + TQ-E-10 + TQ-L1-02：.claude/test-inventory.md 全量校准——表头/段头/段小计三处以当时实跑数为准（Stage 01-08 净增用例后非 726/2635；L1 实跑数取 cargo test 输出、L2 取 npm test 输出、L3 取 npm run test:l3 输出、L4 取 e2e 报告，跑不动时以最近一次全量测试报告为准并注明日期）；TQ-CI-02 新增/变动用例逐条登记；TQ-CI-05 基线指标段更新；豁免清单更新——SEC-17 翻案（已由 TQ-COV-05 tracing-test 锁死）、PTY 残余豁免登记（TQ-COV-03 容量 kill I/O 段）、lib.rs setup 细化（TQ-COV-02）、L4 按键条目细化（TQ-E-05 改名后职责）、L3 职责边界（TQ-E-10）、L1 条件跳过用例登记（TQ-L1-02：signal.rs/watcher.rs/notify/ops.rs 五处有效覆盖依赖 runner 权限）。翻案留痕：TQ-CI-04（tempfile 生产在用，不移 dev-dependencies）与 TQ-E-07（embedded driver 已直连，无 msedgedriver 下载）按 checklist 留痕格式写入。另 src/__tests__/CLAUDE.md 补 helpers/keyboard.ts 登记（若 Stage 04 已补则核对即可）。触碰：.claude/test-inventory.md, src/__tests__/CLAUDE.md。' },
  { label: 'sync-module-docs', prompt: '你负责 TQ-L1-04 + TQ-L1-06 + 模块 CLAUDE.md 同步：src-tauri/src/pty/CLAUDE.md——Mutex 中毒分支无回归用例的豁免登记（TQ-L1-04）+ write_if_size_differs 大小判定假设登记（TQ-L1-06，conpty_api.rs:152 注释已有，文档化）；src-tauri/src/git/CLAUDE.md——TQ-COV-06 死函数清理结果留痕；src-tauri/src/hooks/CLAUDE.md——SEC-17 已由 tracing-test 锁死更新；src/features/explorer/CLAUDE.md——FileTree data-testid 登记；src/features/fileViewers/CLAUDE.md——registerDefaultViewers 导出登记；src/panels/CLAUDE.md——oscHandlers.ts/keyEventHandler.ts 新文件登记；e2e-tests/CLAUDE.md——resetSettings helper + retries=0 + restoreAll 恢复报告登记。各文件遵循渐进式披露原则补句，不展开细节。触碰：上述 7 个 CLAUDE.md。' },
]
const docResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

phase('全量复跑')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行收尾全量复跑（7 条门禁）。前端三命令并行启动；cargo 系串行（共享 target 锁）；L3/L4 随后：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test（报告总用例数——inventory 校准用）
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1（必须单线程；报告总用例数）
6. npm run test:l3（报告总用例数）
7. npm run e2e（全量，耗时长勿中止；报告 9 spec 通过/失败计数）
再补 coverage 复测：npm run test:coverage（前端行覆盖率，对照目标 ≥94.5%）；Rust 侧如 cargo llvm-cov 可用则跑（目标行 ≥90%），不可用则注明。
逐条报告：每命令一行 exit code + 通过/失败 + 关键计数；失败时附前 50 行错误摘要，勿贴完整输出。
`, { label: 'full test suite' })

phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 10 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/test-review-fix/workflows/verify/stage-10.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由；文档断言须核对文档描述与当前代码一致（不撒谎），不一致判 partial。
inventory 三处一致断言：表头总数 = 各级段头之和 = 段小计之和 = 实跑数（以测试 agent 报告的实跑数对照）。
收尾验收断言：全量 7 门禁全绿 + coverage 对照（前端行 ≥94.5% / Rust 行 ≥90% 或重点文件未达标已逐条登记豁免）——以测试报告为据。
以下为测试 agent 的全量复跑结果，测试类断言据此判定（无需重跑）：
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

return { docResults, testResult, verifyResult }
