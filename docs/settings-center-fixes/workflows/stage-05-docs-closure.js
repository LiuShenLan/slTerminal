// =====================================================================
// Stage 05 文档收口（DOC-01..07，固定最后）
// =====================================================================
// fix-loop 调用约定：args.constraints 传「本 Stage 只改文档/注释，禁止改代码逻辑」
// 跨 agent 契约：无（四 agent 文件零重叠）
// DOC-07 计数以本 Stage 全量四级回归实跑输出校准（预测 L1 818 / L2 2852 /
//   L3 142 / L4 51，总 3864——实跑为准）

export const meta = {
  name: 'stage05-docs-closure',
  description: 'Stage 05 文档收口：规格失实改写 + 偏离登记 + 清单计数同步 + 模块文档口径',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；文档/注释用中文；完成后报告修改的文件清单与每项改动摘要。
本 Stage 只改文档与代码注释，禁止改代码逻辑（DOC-05/DOC-06 仅加注释、DOC-06 settings.rs 仅改 :1 行注释）。
禁区：禁止改 src-tauri/src/pty/ 任何 ConPTY flags；禁止 npm run tauri dev 验证；禁止写入真实凭据值（SEC-18，仅允许 sk-test 假值占位符）。
背景：修复要点详见 docs/settings-center-fixes/checklist.md 对应 ID 条目（先读再动手）。
测试纪律：禁止各自跑任何测试命令——真实执行统一由后续全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'doc-01-02-requirements',
    prompt: `你负责 DOC-01、DOC-02（照抄 docs/settings-center-fixes/checklist.md 对应条目；同文件同 agent）：
【DOC-01】docs/settings-center-requirements.md:189 schema 单点失实改写
- 现状原文：「- src/features/hooksConfig/：schema 单点保留（claude 专属资产，MC-223 语义不变），openHooksConfig.ts 删除。」
- 改为：「- src/features/hooksConfig/：已整体删除；schema 单点迁入 src/features/cliProfiles/profiles/claude/configEditor/schema/（MC-223 语义不变）。」
【DOC-02】docs/settings-center-requirements.md:170 面板注册流程失实改写
- 现状含不存在的 panels/index barrel 五步（先 Read 确认原文）
- 改为「目录 → panelRegistry.ts 注册 → PANEL_TYPES 追加；如涉及新 IPC 命令再追加 capabilities」
- 你只改 docs/settings-center-requirements.md。`,
  },
  {
    label: 'doc-03-07-inventory',
    prompt: `你负责 DOC-03、DOC-07（照抄 docs/settings-center-fixes/checklist.md 对应条目）：
【DOC-03】src/__tests__/CLAUDE.md 新增清单补 4 文件
- 位置：src/__tests__/CLAUDE.md（约 :69 新增清单——先 Read 确认现状）
- 步骤：清单补齐 open-settings.test.ts、open-settings-panel.test.ts、settings-hooks-page.test.tsx、settings-pages-registration.test.ts（注意核对实际文件名拼写，open-settings-panel 先 Glob 确认是 .ts 还是 .tsx）
【DOC-07】.claude/test-inventory.md 计数收口
- 位置：表头 / L2 设置中心段 / L4 settings.e2e.ts 段
- 步骤：本 Stage 收尾的全量四级回归结果由主 agent 在 verify 后提供给你——若你执行时未拿到实跑计数，先把口径改为「预测 L1 818 / L2 2852 / L3 142 / L4 51，总 3864」并在条目注明「待实跑校准」；三处（表头/L2/L4 段）计数必须一致，总计 = 四项之和
- 你只改 src/__tests__/CLAUDE.md 与 .claude/test-inventory.md。`,
  },
  {
    label: 'doc-04-e2e-docs',
    prompt: `你负责 DOC-04（照抄 docs/settings-center-fixes/checklist.md 的 DOC-04 条目）：
【DOC-04】report.md 偏离登记 + e2e-tests/CLAUDE.md 同步
- 位置：docs/settings-center/report.md 第四节（偏离登记——先 Read 确认现有 3 项格式）；e2e-tests/CLAUDE.md（先 Read 全文确认备份清单/helper 说明现状）
- 步骤：
  ① report.md 补第 4 项偏离：__slterm_e2e_getSettingsPanelCount 计数范围收窄为活跃页面 api（checklist 约定「全部页面」），理由：设置中心全局单例功能等价；格式与既有 3 项一致
  ② e2e-tests/CLAUDE.md：备份清单删三段（~/.slterminal/settings.json / projects.json / .bak——已被 SLTERM_DATA_DIR 隔离取代）；新增 SLTERM_DATA_DIR 隔离机制说明（run-wdio.cjs 注入 os.tmpdir()/slterm-e2e-data，应用全部数据写入与日常隔离）；helper 清单补 __slterm_e2e_setSettingsDirty(panelId, dirty)（测试后门：直接置设置面板 dirty 态）；新增警示「target/debug 的 exe 可能是 E2E 构建（npm run e2e 覆盖产物），日常使用须以普通 npx tauri build --debug --no-bundle 覆盖」
- 你只改 docs/settings-center/report.md 与 e2e-tests/CLAUDE.md。`,
  },
  {
    label: 'doc-05-06-module-docs',
    prompt: `你负责 DOC-05、DOC-06（照抄 docs/settings-center-fixes/checklist.md 对应条目）：
【DOC-05】src/features/settingsCenter/types.ts SettingsPageProps mount 约定标注
- 位置：src/features/settingsCenter/types.ts SettingsPageProps（约 :13-17）
- 步骤：onPageParamsChange 字段加注释「**约定**：组件 mount/首渲染期禁止调用（仅响应用户交互调用）——mount 期调用会误触发布局保存」（仅加注释，不改类型定义）
【DOC-06】stores/CLAUDE.md + settings.rs:1 注释 + src-tauri/src/CLAUDE.md 三处口径
- ① src/stores/CLAUDE.md projects store 段补：「projects 持久化防线：loadFromDisk 成功置 loadSucceeded；未成功加载且 store 空时 saveToDisk 拒写（防空写覆盖磁盘）；显式放行经 markLoadSucceeded()（E2E 分支/用户选择空状态继续）」（先 Read 确认现状结构再插入）
- ② src-tauri/src/settings.rs:1 注释「~/.slterminal/settings.json」改为 exe 同级口径（BE-16 便携化后实际位置，先 Read 确认原文）
- ③ src-tauri/src/CLAUDE.md app_dir 段补「数据目录三级来源：测试 guard > SLTERM_DATA_DIR env > exe 同级」（先 Read 确认现状段落）
- 你只改 src/features/settingsCenter/types.ts、src/stores/CLAUDE.md、src-tauri/src/settings.rs、src-tauri/src/CLAUDE.md。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（全量四级回归；组内并行、组间串行——build:e2e 先于 wdio）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量四级回归。命令分两组——组内并行、组间串行：
组一（并行启动）：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
7. npm run test:l3
组二（组一完成后串行）：
8. npm run e2e
逐条报告：每命令一行 exit code + 通过/失败；各测试命令附用例计数行（如 Tests 2839 passed / test result: ok. 815 passed）；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队、npm run e2e 耗时较长，均属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-05.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center-fixes/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码/文档判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。文档类断言须对照当前真实代码核实，防文档撒谎。
以下为测试 agent 的全量测试执行结果，测试类断言据此判定（无需重跑）；DOC-07 计数以其中的用例计数行为准：
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
