// =====================================================================
// stage-17-hooks-validate.js — S17 hooks 写入校验（SEC-05/12/13）
// =====================================================================
// 跨边界契约（写死）：
//   agent_hooks_config_write 后端语义校验规则——事件名 ∈ HOOK_EVENTS（10 事件
//   白名单）、handler type == "command"、command 为非空字符串；
//   校验失败返回 AppError::Validation。
//   user 层写入时前端 confirmDialog 二次确认（D9）；project/local 层不确认。
// fix-loop 调用约定：args.testCommands 省略（默认基础五条）；constraints 传空。
// =====================================================================

export const meta = {
  name: 'stage17-hooks-validate',
  description: 'Stage 17: hooks 写入语义校验 + user 层二次确认 + statusline 审查 warn + 脚本哈希比对（SEC-05/12/13，D9）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/review-fix/checklist.md 对应 ID 条目 + docs/review-fix/stages.md S17 节（先读再动手）。
测试纪律：本阶段禁止跑资源共享型全量测试（cargo test / npm test / npm run test:l3）——只做编译级检查（cargo check / npx tsc --noEmit），全量执行由独立测试 agent 单点跑（跨进程并发会死锁，cargo 排队属正常）。
本 Stage 契约：写入校验三规则 = 事件名 ∈ HOOK_EVENTS 10 事件白名单 + handler type=="command" + command 非空字符串，失败 AppError::Validation；user 层写入前端 confirmDialog 二次确认，project/local 不确认。`

// === Phase 1: 并行重构（agent 间文件零重叠；不跑资源共享型测试）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'hooks-backend',
    prompt: `你负责 SEC-05、SEC-12、SEC-13，只许改 src-tauri/src/hooks/claude/config.rs、src-tauri/src/hooks/claude/inject.rs（SHA-256 如需新依赖则含 src-tauri/Cargo.toml）：
【SEC-05】agent_hooks_config_write（config.rs:174-195）当前仅 JSON 对象类型校验。按契约加语义校验：事件名 ∈ HOOK_EVENTS（10 事件白名单，复用模块既有常量）、handler type=="command"、command 非空字符串；失败 AppError::Validation。复用 S10/BE-18 所建 Layer 枚举与 hooks 子树结构体。补 L1 测试：非法事件名/非法 type/空 command 拒绝、合法写入放行。
【SEC-12】statusline 桥接脚本透传执行用户原 statusline 命令（slterm-statusline.js）——注入/重注入时对原命令做可疑模式审查（curl/wget/Invoke-Expression 等），命中 tracing::warn! 告警；仅记录不阻断（命令来自用户自身配置，信任边界登记在 S19）。补 L1 测试（可疑模式命中 warn——可测部分抽纯函数）。
【SEC-13】脚本版本检测依赖首行文本（inject.rs），磁盘脚本可被替换为首行匹配的恶意文件。修复：include_str! 内嵌脚本模板，状态检测时对磁盘脚本计算 SHA-256 与模板哈希比对，不一致 → Outdated。sha2 优先检查 Cargo.lock 现有依赖树可用性，无则加 sha2 直接依赖。补 L1 测试：篡改脚本（首行保留）被检出 Outdated。
只做 cargo check 编译级检查，禁止 cargo test。`
  },
  {
    label: 'hooks-frontend',
    prompt: `你负责 SEC-05（前端侧），只许改 src/panels/hooksConfig/ 下文件（以 grep agent_hooks_config_write / writeHooksConfig 调用点为准）及 src/__tests__/ 下对应测试：
按 D9：user 层（layer==="user"）写入时 confirmDialog 二次确认（confirmDialog 从 src/lib 导入，文案示例：「确认写入用户级 hooks 配置？hooks 可执行任意命令」）；project/local 层不确认直接写。确认取消则不写。
先 grep 写入调用点定位（useHooksConfig.ts / ClaudeHooksConfigEditor.tsx），在保存链路加确认。
补 L2 测试：user 层弹确认（确认/取消两分支）、project/local 不弹。
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
逐项检查 Stage 17 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/review-fix/workflows/verify/stage-17.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
