// =====================================================================
// Stage 03 e2e 基建族
// 波次① CP-003+045+046（run-wdio 合并体）→ 波次② [CP-029 ∥ CP-030 ∥ CP-028]
// → CP-041（串行尾位，接 mockcli.e2e.ts 与 run-wdio.cjs）
// fix-loop constraints：本 Stage 无特殊纪律（省略 args.constraints）
// =====================================================================

export const meta = {
  name: 'stage-03-e2e-infra',
  description: 'S03 e2e 基建族（CP-003/045/046/029/030/028/041）',
  phases: [
    { title: '波次① run-wdio 合并体' },
    { title: '波次② 并行修复' },
    { title: '串行尾位 CP-041' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md 对应 CP 条目（先读再动手，照抄步骤不另起方向）；大代码块按条目内「照抄 review-NN 步骤 X.Y」指针读 docs/compromises-fix/ 下对应 review 文件。
本 Stage 起 L1 定向测试命令 = cargo test <filter> -- --test-threads=1（S02 已拆 TQ-COV-06 红线，旧 --test lib_tests 形态失效）。`

// === Phase 1: 波次① run-wdio 合并体（单 agent 先行，② 在新工具链上定责）===
phase('波次① run-wdio 合并体')
const wave1Result = await agent(`${PREAMBLE}

你负责 CP-003+CP-045+CP-046（run-wdio.cjs 合并体，单 agent）：
【CP-003】照抄 checklist 步骤 1-8：门禁实证先行（.temp/node22 临时改名隔离、node --version ≥26 确认）；run-wdio.cjs:246-288 整块替换为条目给定新块（显式预置 >1MB 判活，删自动下载；旧 else fallback 语义已并入新块末尾 fallback();，勿残留）；删 :27 https require；头注 :1-4 替换；package.json 插 "engines": { "node": ">=22" }（scripts 块之后 dependencies 之前）；e2e-tests/CLAUDE.md :23-25 节替换 + :42 理由换锚（隔离链口径）；wdio.conf.ts:5 注释替换；.claude/test-exemptions.md 追加启动器豁免行。失败分支（tauri-service/undici 类 Node 26 证据）→ 回滚全部改动并登记，本条转休眠。
【CP-045】纯文档销项：run-wdio.cjs:13 头注改 per-pid 唯一名口径；e2e-tests/CLAUDE.md:48 固定名表述改写；docs/compromises.md CP-045 勾选 + 注记；清理逻辑零改动。
【CP-046】run-wdio.cjs:66-139 改写：SETTINGS_SENTINEL_KEYS = ["hooks", "statusLine"] + snapSettingsSentinels（照抄 checklist 代码块）；snapshotUserHome 的 claudeSettings 改键级快照；verifyRealHomeUnchanged 改哨兵键级校验（照抄代码块）；statuslineBackup/hooksDir/hooksEvents 三面保留原形态；snapFile 无其它调用方则删（snapDir 内部保留）。e2e-tests/CLAUDE.md:50/:54 改写哨兵键口径；run-wdio.cjs:17-21 头注同步。
人工验证点（报告中标出待用户执行）：Node 26 全量 e2e 实跑；CP-046 负向验证（注入 hooks 键 → exit 1）。
自查：grep -n "nodejs.org\|https.get\|require('https')" e2e-tests/run-wdio.cjs 零命中；npm pkg get engines 输出 {"node":">=22"}；node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts exit 0。
完成后报告修改文件清单 + 人工验证点待办。`, { label: 'cp003-045-046-run-wdio' })

// === Phase 2: 波次② 并行（CP-029 定责 ∥ CP-030 焦点探针 ∥ CP-028 展开确定性）===
phase('波次② 并行修复')
const wave2Agents = [
  {
    label: 'cp029-triage',
    prompt: `你负责 CP-029（editor.e2e.ts auto-reload 二分定责——只取证不预判）：
照抄 checklist CP-029 步骤 1-6：基线复现（WDIO_RETRIES=0 单 spec，确认唯一失败用例）；探针 A（fs-event 投递链，临时 spec probe-fsevent.e2e.ts，登记 wdio.conf 跑完即删）；探针 B（notify_watch 手动注册）；探针 C（环境消融，不经 run-wdio 手动起 slterminal.exe）。
定责收尾两出口写死：环境出口 → 修 run-wdio.cjs 注入方式【注意：run-wdio.cjs 串行尾位还有 CP-041 改动，你的 run-wdio 修复须报告改动原文但不直接写盘，由主 agent 并入尾位序列——test-exemptions.md:24 归因改写你可以直接执行】；产品出口 → 修 useCodeMirror.ts:426-504 或 Workspace.tsx:238-269 + 删豁免行 + L2 回归用例。
editor.e2e.ts:283 用例保持启用零改动；探针取证完删除不入库。
修后复跑 WDIO_RETRIES=0 连续 3 轮全绿。
完成后报告：定责结论（环境/产品）+ 实测证据 + 修复面 + run-wdio.cjs 待并改动原文（若环境出口）。`,
  },
  {
    label: 'cp030-focus-probe',
    prompt: `你负责 CP-030（beforeSuite 聚焦探针 fast-fail + cli-aliases 真实手势回归）：
照抄 checklist CP-030 步骤 1-3：
1. wdio.conf.ts beforeSuite（:70 双 reset 的 browser.execute 之前）插入 TQ-E-10 探针块（照抄代码块，失焦即 throw）。
2. cli-aliases.e2e.ts 四处真实手势替换（:202-204 setValue、:205-207 添加钮 click、:260-262 删除钮 click；chip 出现等待保持 execute 轮询不动）+ 恢复交互时序断言（添加后输入框清空，照抄代码块）。
3. 不 fork tauri-service、不改 node_modules；其余 spec 不动。
文档：e2e-tests/CLAUDE.md 外部坑节 focusCommands 条目补 TQ-E-10 口径；「合成 JS click 无焦点语义」节补 alias 链例外。
自查：WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec cli-aliases.e2e.ts 1 轮 exit 0；npx tsc --noEmit 绿。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp028-aria-expanded',
    prompt: `你负责 CP-028（NavTree aria-expanded 探针 + 两处 6 轮循环收敛）：
照抄 checklist CP-028 步骤 1-4：
1. 三处行根加 aria-expanded={expanded}：NavProjectRow.tsx:47-49、NavPageRow.tsx:69-71、NavHistoryNode.tsx:38-40（props 已有，直接引用）。
2. agent.e2e.ts ensureTreeExpanded 整体替换为单次确定性版（完整代码块照抄 review-05 CP-028 步骤 3.2）；原函数体与「展开态判定」长注释删除。
3. mockcli.e2e.ts 用例①内联 6 轮循环（206-255）替换为同构单次版（含「当前」pill 容器操作域）。
4. 顺手项（附注登记）：history.e2e.ts:123-148 ensureAllProjectsExpanded 第三处同构循环可同改，不改不算未完成。
测试：nav-tree.test.tsx/nav-tree-history.test.tsx 各补 aria-expanded 初始 false/点击后 true 断言（建议新增）。
文档：src/features/navTree/CLAUDE.md 数据属性契约节补 aria-expanded 登记。
自查：grep -c "aria-expanded" 三组件文件各 ≥1；grep -c "for (let i = 0; i < 6" agent/mockcli e2e 各 = 0；npx tsc --noEmit 绿。
完成后报告修改文件清单（含顺手项是否执行）。`,
  },
]
const wave2Results = await parallel(
  wave2Agents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 串行尾位 CP-041（接 mockcli.e2e.ts 与 run-wdio.cjs，须在 028/029 后）===
phase('串行尾位 CP-041')
const tailResult = await agent(`${PREAMBLE}

你负责 CP-041（mockcli L4 专用 history provider，env 门控注册）+ 并入 CP-029 环境出口的 run-wdio.cjs 待改项（若有）：
【CP-041】照抄 checklist 步骤 1-6：
1. 新建 src-tauri/src/agent_history/mock.rs（骨架照抄 review-05 CP-041 步骤 3.1，两处 todo!() 填充，ScanRootGuard 误引行删除；复用 claude::jsonl 解析助手，可见性不足则 pub(crate) 化，禁止复刻第二份解析）。
2. provider.rs:46-62 注册表 env 门控改写（BASE_REGISTRY/E2E_REGISTRY/registry()，照抄 review-05 步骤 3.2）；mod.rs:114-133 的 REGISTRY.iter() 改 registry().iter()；mod.rs:26 use 同步。
3. mod.rs 加 mod mock;（crate 内私有）。
4. run-wdio.cjs 在 claude fixture 块（:211-228）之后追加 mockcli fixture 块（照抄 review-05 步骤 3.4；fixtures/mockcli-projects 缺失即 exit(1) 红线扩列）。
5. 新建 e2e-tests/fixtures/mockcli-projects/（形态照 claude-projects，README 一行 + ≥1 条会话）。
6. mockcli.e2e.ts 新增第三 describe「mockcli 历史链路（CP-041 L4：展示 + 双击恢复注入）」两用例（展开辅助复用 CP-028 单次确定性形态）。
【并入】CP-029 agent 报告（下方）若含 run-wdio.cjs 待改原文 → 在本 agent 内落到 run-wdio.cjs（同文件串行收口）。
---
CP-029 报告：${wave2Results[0] ?? '（未返回或无 run-wdio 改动）'}
---
测试：L1 新增 registry_env_absent_returns_base_only / registry_present_e2e_env_includes_mockcli（env 修改成对 set/remove）+ mock.rs mock_provider_tests 组；豁免销项：删 .claude/test-exemptions.md:21-22 两行。
文档：agent_history/CLAUDE.md 注册表节补 env 门控扩展；e2e-tests/CLAUDE.md fixture 通道节补 mockcli；mockcli.e2e.ts 头注补第三 describe 数据隔离语义。
自查：cargo test mock -- --test-threads=1 绿；cargo test -- --test-threads=1 全量绿（计数 = 基线 + 新增）；WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts 三轮 exit 0；grep -c "mockcli" .claude/test-exemptions.md = 0。
完成后报告修改文件清单。`, { label: 'cp041-mockcli-history' })

// === Phase 4: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. npm test
4. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
5. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
6. npx vite build
7. npm run e2e
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 5: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 03 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-03.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { wave1Result, wave2Results, tailResult, testResult, verifyResult }
