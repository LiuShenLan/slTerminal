// =====================================================================
// Stage 05 E2E（E2E-01 ~ E2E-03）
// =====================================================================
// 跨边界契约（checklist.md「跨边界契约」节，双端不各自推断）：
// - 设置页 id "backgroundTasks"；选择器系列：settings-nav-backgroundTasks / settings-background-tasks-page /
//   settings-background-tasks-interval-planBalance / settings-background-tasks-error-planBalance 等
// - 落盘断言形态：root.backgroundTasks?.planBalance?.intervalSec === 120（settings.json backgroundTasks 段 planBalance 子键）
// - 红字文案：10–3600 秒 / 2–300 秒（DTO 无 default 字段，无「默认」字样）
// - 定时刷新：sessionRefresh 默认 enabled=true interval=3s；E2E 写 SLTERM_CLAUDE_PROJECTS_DIR jsonl → 等 tick → nav-history-node 计数 +1
// - 退役选择器 settings-plan-balance-* / settings-nav-planBalance 在 e2e-tests/ 内零命中
// fix-loop constraints（execution-plan.md）：本 Stage 传「只改 e2e-tests/ 下文件，不改 src/ 与 src-tauri/ 生产代码；若失败根因在生产代码，报告并停止」
export const meta = {
  name: 'stage05-e2e',
  description: 'F12 E2E：settings.e2e 适配新页 + background-tasks.e2e 新建（页操作链路 + 定时刷新端到端）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复要点详见 checklist 对应 ID 条目（先读 docs/background-tasks/checklist.md 再动手）。
补充纪律：本 Stage 只改 e2e-tests/ 下文件，禁止改 src/ 与 src-tauri/ 生产代码（若发现根因在生产代码，报告并停止，不自行修改）；helper 复用 settings.e2e.ts / history.e2e.ts 既有模式（假 env 注入 / waitForSettingsFile / SLTERM_CLAUDE_PROJECTS_DIR 写 jsonl / nav-history-node 计数断言），不另造轮子；e2e-tests 不在 tsc/eslint 覆盖内——本 Stage 无静态检查门禁，wdio 实跑为人工验证点，脚本不内嵌。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'e2e',
    prompt: `你负责 E2E-01 / E2E-02 / E2E-03，先读 docs/background-tasks/checklist.md 中三个 ID 的完整条目，逐条实现：

【E2E-01】适配 e2e-tests/settings.e2e.ts（11 用例，planBalance 页选择器/页 id 全面替换）：
- 3.1 全文替换：页 id 实参 "planBalance" → "backgroundTasks"；settings-nav-planBalance → settings-nav-backgroundTasks；settings-plan-balance-page → settings-background-tasks-page；settings-plan-balance-input → settings-background-tasks-interval-planBalance；settings-plan-balance-error → settings-background-tasks-error-planBalance。
- 3.2 用例④落盘断言改 root.backgroundTasks?.planBalance?.intervalSec === 120（waitForSettingsFile 判定函数同步改）；注释「planBalance 段」改「backgroundTasks.planBalance 子键」；余量刷新闭环节（假 env 注入 + plan-balance-row 断言）保留不变。
- 3.3 用例⑤红字文案断言由「10–3600 秒，默认 60」改「10–3600 秒」（DTO 无 default 字段，提示只写范围）。
- 3.4 用例①注释「order 10 < planBalance order 20」改 backgroundTasks 口径。
- 3.5 用例⑧两处 selectedPage).toBe("planBalance") 改 "backgroundTasks"。
- 用例数不变（11）。

【E2E-02】新建 e2e-tests/background-tasks.e2e.ts（用例 A-D；先读 settings.e2e.ts 与 helpers.ts/specUtils.ts 提取可复用 helper，不另造轮子；suite 级快照还原 exe 同级 settings.json 照 settings.e2e.ts :216-221 先例）：
- 用例 A「页渲染与两行齐备」：配置钮 → 设置面板 → 切「后台定时任务」页 → 断言 settings-background-tasks-row-planBalance 与 -row-sessionRefresh 均存在、勾选默认 true、频率输入默认 10 / 3。
- 用例 B「改频率端到端生效」：planBalance 行频率改 15 失焦 → waitForSettingsFile 断言 backgroundTasks.planBalance.intervalSec === 15 → 输入框规范化回显 15、无红字。
- 用例 C「勾选禁用 planBalance → footer 隐藏；重新启用 → footer 重显」：前置假 env 注入 + 手动刷新使 plan-balance-row 出现 → 取消勾选 → 断言 row 消失（事件驱动隐藏）+ 磁盘 enabled === false → 重新勾选 → row 重显（最后快照保留——不重拉也显）+ 磁盘 enabled 回 true。
- 用例 D「非法频率行内红字不落盘」：sessionRefresh 行输 1（< 2）失焦 → 红字「2–300 秒」+ 磁盘无 sessionRefresh 子键（文件未变）。
- 若 e2e-tests/wdio.conf.ts specs 为 glob 则免登记；显式清单则追加 background-tasks.e2e.ts（先读确认）。

【E2E-03】同文件追加用例 E/F（真实 tick 生效断言）：
- 用例 E「定时刷新自动出现新会话」：创建 E2E 项目（cwd = SLTERM_E2E_PROJECT_DIR）→ 打开导航树确认历史计数 N → 设置中心把 sessionRefresh 频率改 2s（磁盘断言落盘）→ Node 侧往 SLTERM_CLAUDE_PROJECTS_DIR/<编码目录>/ 写一个归属本项目的新会话 jsonl（照 history.e2e.ts fixture 形态：summary 首行 + user 行 cwd=项目路径）→ 等待（2×interval + 余量约 5s）→ 断言历史节点计数 pill 变 N+1（全程无手动刷新点击）。
- 用例 F「禁用 sessionRefresh → 新会话不自动出现；启用 → 出现」：禁用勾选 → 再写一个 jsonl → 等 2×interval → 计数不变；重新勾选 → 等 tick → 计数 +1。
- 用例 G（tick 失败静默）降级豁免——spec 内不写（无故障注入通道；兜底 L2 + 人工；豁免登记由 Stage 06 DOC-02 完成）。

完成后报告：每个 ID 的修改摘要 + 文件清单。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（回归防线——e2e-tests 改动无 tsc/eslint 覆盖，wdio 实跑为人工验证点）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源：docs/background-tasks/workflows/verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/background-tasks/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。wdio 实跑为人工验证点，不在此判定。
以下为测试 agent 的全量测试执行结果，回归防线类断言据此判定（无需重跑）：
---
${testResult ?? '（测试 agent 未返回——回归防线断言全部判 not_fixed）'}
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
