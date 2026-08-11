// =====================================================================
// Stage 01 — 安全族 + 字面量守卫强化（AQ-1/AQ-2/AQ-3/AQ-4/ZQ-5/CS-1/CS-2）
// =====================================================================
// 真值源: docs/review-fix/checklist.md（逐 ID 修复要点）+ docs/review-fix/stages.md（Stage 01 节）
// 断言清单: docs/review-fix/workflows/verify/stage-01.md（本脚本与 fix-loop 共用同一真值源）
// 跨边界契约: 本 Stage 无跨 agent 共享接口契约（各 ID 文件零重叠，分工表见 stages.md Stage 01）
// fix-loop args: { stage: 1, failedItems, fixContext,
//   verifyFile: 'docs/review-fix/workflows/verify/stage-01.md',
//   constraints: stages.md「禁区」六条原样,
//   testCommands: 本脚本 TEST_COMMANDS 数组原样（失败项涉 AQ-4 的 L4 断言时必传） }
// =====================================================================

export const meta = {
  name: 'stage01-security',
  description: 'Stage 01: 安全族修复 + AC-5 守卫强化（AQ-1~4/ZQ-5/CS-1/CS-2）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区（不可违背）：
1. compute_conpty_flags 固定 0x7 勿动（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮，无法自动化守卫
2. C10 契约：reporter 任何路径 exit(0)、不写 stderr——改 payload 键时勿削弱
3. watcher 轮询补漏（3s）勿削弱——win10 实证防线
4. SEC-05 等价：sessionId 校验 + 定位不信托前端——改 ops.rs 时勿削弱（is_symlink 是加防御不是松校验）
5. E2E 不得触碰用户真实 ~/.claude/——AQ-4 正是强化此防线，改 run-wdio.cjs 时勿引入新降级路径
6. E2E_ENABLED 保持内联 import.meta.env 字面量形态（rolldown DCE 红线）——改 helpers.ts 时勿动
背景：先读 docs/review-fix/checklist.md 中你负责 ID 的条目原文 + docs/review-fix/stages.md Stage 01 节的实现要点，再动手。
本 Stage 纪律：
- 并行期间禁止跑资源共享型测试（cargo test 必须单线程全量由专门 agent 跑；npm run e2e 有 slterminal.exe 占用冲突）——后端 agent 只做 cargo check --manifest-path src-tauri/Cargo.toml；前端 agent 允许跑自己改动的单文件 vitest（npx vitest run <文件>，纯 jsdom 无共享资源）；e2e-launcher 允许 node --check e2e-tests/run-wdio.cjs
- 测试通过与否以全量测试 agent 为准，你的单文件自查不替代门禁`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-guard',
    prompt: `你负责 AQ-2、AQ-3（后端文件防护强化）：

【AQ-2】信号文件大小限制——src-tauri/src/hooks/signal.rs：
- 位置：process_signal_file_with（现 :72-78，读取前无大小限制）。
- 修复：读取前 path.metadata() 判定，超 MAX_SIGNAL_FILE_BYTES = 1024 * 1024（1MB，模块内常量）→ tracing::warn! 记录 + 删除该信号文件后直接返回（与既有「解析失败仍删」容错语义一致）；metadata 失败（如文件已消失）走既有读失败分支。
- L1 新用例（同文件 #[cfg(test)]）：构造 >1MB 信号文件 → emit 闭包零调用 + 文件已被删除。

【AQ-3】符号链接拒跟随——src-tauri/src/agent_history/claude/ops.rs：
- 位置：locate_session_jsonl（现 :43-56）与 delete_session 同名目录删除（现 :77-81）。
- 修复三处：一级子目录 dir_path.is_dir() 后补 !dir_path.is_symlink()（:50）；命中文件 candidate.is_file() 后补 !candidate.is_symlink()（:54）；delete_session 同名 <id>/ 目录 is_dir() 后补 !session_dir.is_symlink()（:79）。
- SEC-05 语义不变：sessionId 校验与定位流程不动，仅加拒绝分支（加防御不是松校验）。
- L1 新用例：symlink 指向外部目录 → 定位不命中 / 删除被拒绝；用 std::os::windows::fs::symlink_dir / symlink_file 创建，创建失败（权限不足，CI runner 差异）时测试内跳过并注释说明。

就近同步：src-tauri/src/agent_history/CLAUDE.md（符号链接拒跟随一句）。
禁止触碰 src-tauri/src/hooks/CLAUDE.md——该文件归 backend-config 单点负责（AQ-2 大小限制的文档句由它代写）。
禁止触碰 .claude/test-inventory.md——归 literal-guard 单点负责。
自查：cargo check --manifest-path src-tauri/Cargo.toml 通过；cargo fmt --manifest-path src-tauri/Cargo.toml -- --check 通过。`,
  },
  {
    label: 'backend-config',
    prompt: `你负责 ZQ-5（hooks 配置写路径 null 视作 {}）：

- 位置：src-tauri/src/hooks/claude/config.rs——write_hooks_subtree（现 :109-112）与 config_write_sync（现 :179-181）。
- 决策 3（用户拍板）：写路径 hooks 入参 null 视作空对象 {} 进行 merge——与 read 返回 null 对称，与既有「原文件内容为 null 视作空对象」语义一致；语义 = 清空该层 hooks。
- 修复：config_write_sync 入口校验改为「hooks.is_null() → 替换为空对象 {}；非 null 且非 object → Validation 错误」；write_hooks_subtree 的 is_object 闸门保持不变（收到的恒为 object）。
- L1 新用例（同文件 #[cfg(test)]）：hooks = Value::Null 写入 → 文件 hooks 键 = {}（merge 保留原文件其他字段）。

就近同步 src-tauri/src/hooks/CLAUDE.md（本文件由你单点负责）：
1. write 语义段更新：hooks 入参 null 视作 {}（语义 = 清空该层 hooks），非 object 仍 Validation
2. 代 backend-guard 补 AQ-2 一句：信号文件读取前大小限制（>1MB 超限 → warn + 删除不处理，常量 MAX_SIGNAL_FILE_BYTES 见 signal.rs）
禁止触碰 .claude/test-inventory.md——归 literal-guard 单点负责。
自查：cargo check --manifest-path src-tauri/Cargo.toml 通过；cargo fmt --manifest-path src-tauri/Cargo.toml -- --check 通过。`,
  },
  {
    label: 'frontend-strategy',
    prompt: `你负责 AQ-1（buildResumeCommand cwd 单引号转义）：

- 位置：src/features/cliProfiles/profiles/claude/strategies.ts:109-112（buildResumeCommand）。
- 问题：恢复命令经 PowerShell 解析，cwd 含单引号（如 C:\\Bob's Project）会破坏单引号字符串边界。
- 修复：cwd 单引号按 PowerShell 规则转义为两个单引号——session.cwd.replace(/'/g, "''")；:106-107「原样保留」自述注释删除，改为「单引号按 PowerShell 规则转义为 ''（AQ-1 修复）」说明。
- buildRestoreInput 不动（不含 cwd；恢复编排的 cwd 由 addPanel params.cwd 承担，PTY 直接写 stdin 不经 PowerShell 解析，无注入面）。
- 测试 src/__tests__/cli-profile-claude.test.ts：「输出与迁出源逐字一致」断言同步更新为转义后形态 + 新增 cwd 含单引号回归用例（C:\\Bob's Project → 命令中该段为 C:\\Bob''s Project，整体包裹于单引号内）。

就近同步：src/features/cliProfiles/CLAUDE.md（strategies 行「逐字一致」表述随行——输出与迁出源差异点 = cwd 单引号转义）。
禁止触碰 .claude/test-inventory.md——归 literal-guard 单点负责（你的用例数 +1 由它代登记）。
自查：npx tsc --noEmit 通过；npx vitest run src/__tests__/cli-profile-claude.test.ts 通过。`,
  },
  {
    label: 'e2e-launcher',
    prompt: `你负责 AQ-4（E2E fixture 缺失终止不降级）：

- 位置：e2e-tests/run-wdio.cjs:148-152（fixture 缺失 else 分支，现仅 console.warn 继续启动 wdio）。
- 问题：fixture（fixtures/claude-projects）缺失时继续启动 → HOME 回落真实用户目录，E2E 有触碰真实 ~/.claude/ 风险。
- 修复：else 分支改为 console.error 明确文案（须含「fixtures/claude-projects 缺失，E2E 终止——防止回落真实 ~/.claude/projects」语义）+ process.exit(1)；位置保持在 wdio 启动前（现状即是前置检查段）。
- 禁区 5：本项正是强化「E2E 不得触碰真实 ~/.claude/」防线——禁止引入任何新降级路径（如自动创建空 fixture、改用临时目录兜底）。

就近同步：e2e-tests/CLAUDE.md（fixture 缺失行为描述：终止而非降级）。
自查：node --check e2e-tests/run-wdio.cjs 通过（该文件无任何静态门禁覆盖——tsc include 不含 e2e-tests/、eslint 仅扫 src/，node --check 是你唯一的语法级自查）。`,
  },
  {
    label: 'literal-guard',
    prompt: `你负责 CS-1、CS-2（no-claude-literals 守卫强化）——只改 src/__tests__/no-claude-literals.test.ts + 就近文档：

【CS-1】模板字符串拼接绕过检测（现 :129-140 模板分支含 \${} 即整体跳过）：
- 问题：生产代码可用 \`cl\${''}aude\` 拼出运行时值 "claude"，守卫不报违规。
- 修复：含表达式的模板字符串不再整体跳过——提取全部字面量片段（表达式外的静态文本）拼接成单值后 push 进字面量集合，走既有三类判定（claude 精确 / 10 事件名 / ~/.claude 路径），命中即报违规。
- 新增自检用例：构造含 \`cl\${''}aude\` 形态的样例源码字符串，断言守卫判定报违规。

【CS-2】扫描范围遗漏 cliProfiles 根目录（现 :39-47 SCAN_DIRS 七路径）：
- 修复：SCAN_DIRS 追加 src/features/cliProfiles（第八路径）；目录级豁免 profiles/claude/ 子目录（claude 合法领地）——扫描循环按相对路径前缀（统一 normalize 为正斜杠后比较）跳过 src/features/cliProfiles/profiles/claude/。
- 扫描范围完整性断言同步（七 → 八路径）；describe 标题与头部注释「七路径」表述随行改为八路径 + 豁免规则说明。
- 豁免路径拼写错会静默空扫——补断言：profiles/claude 目录存在（fs.existsSync）且被豁免（构造该目录下的样例路径确认不参与违规收集）。

就近同步：
1. src/__tests__/CLAUDE.md（守卫描述：模板拼接检测 + 八路径 + profiles/claude 豁免）
2. .claude/test-inventory.md（由你单点负责）：守卫条目更新（八路径 + 模板拼接检测 + 自检用例）+ 代 frontend-strategy 登记 cli-profile-claude 用例数 +1（AQ-1 新增单引号回归用例）
自查：npx vitest run src/__tests__/no-claude-literals.test.ts 通过。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（1-8 并行；L4 单独最后串行——exe 占用冲突）===
phase('全量测试')
const TEST_COMMANDS = [
  'npx tsc --noEmit',
  'npx eslint src/',
  'cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings',
  'cargo fmt --manifest-path src-tauri/Cargo.toml -- --check',
  'cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1',
  'npm test',
  'npm run test:l3',
  'node --check e2e-tests/run-wdio.cjs',
  'npm run e2e',
]
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。
执行前确认：无运行中的 slterminal.exe（Windows 文件锁会致 cargo 链接 os error 5）。
以下命令 1-8 相互独立，并行启动执行，收集全部结果；第 9 条 npm run e2e（= build:e2e + wdio）与 cargo 系存在 slterminal.exe 文件占用冲突——必须等 1-8 全部完成后单独串行执行：
${TEST_COMMANDS.map((c, i) => `${i + 1}. ${c}`).join('\n')}
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
