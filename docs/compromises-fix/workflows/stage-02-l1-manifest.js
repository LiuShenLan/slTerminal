// =====================================================================
// Stage 02 L1 测试基建（CP-040 embed-manifest 单项 Stage）
// fix-loop constraints 取值唯一真值源 = execution-plan.md「fix-loop 调用规范」表 S02 行
// =====================================================================

export const meta = {
  name: 'stage-02-l1-manifest',
  description: 'S02 L1 manifest 改 embed-manifest 通道，拆 TQ-COV-06 红线（CP-040）',
  phases: [
    { title: '修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md 对应 CP 条目（先读再动手，照抄步骤不另起方向）。`

// === Phase 1: 修复（单 agent；回退预案写死于 checklist CP-040 步骤 3.7）===
phase('修复')
const fixResult = await agent(`${PREAMBLE}

你负责 CP-040（默认 lib test target 0xc0000139 —— embed-manifest 注入 + 拆 [lib] test=false 重组 + TQ-COV-06 红线）：
照抄 checklist.md CP-040 步骤 3.1-3.7：
1. 方案已定案 embed-manifest（.cargo/config.toml rustflags 通道已评估拒绝，理由在条目内，不另选型）。
2. src-tauri/Cargo.toml [dev-dependencies] 加 embed-manifest = "1"（含 CP-040 注释块）。
3. src/lib.rs 注入块（#[cfg(all(windows, test))] embed_manifest::embed_manifest_file!("tests-comctl6.manifest");，crate 根 item 位，紧邻 #[cfg(test)] 区前；manifest 沿用现 tests-comctl6.manifest 不新建）。
4. Cargo.toml 拆 [lib] test=false 与 :17-21 F12 注释块、删 [[test]] lib_tests 块，替换为条目给定的一句话注释。
5. build.rs 的 rustc-link-arg-tests 两行保留，:2-10 注释更新口径。
6. 拆红线：根 .claude/CLAUDE.md:77 TQ-COV-06 bullet 整条删；.claude/test-exemptions.md:29 删行 + 脚注区补销记；tests-comctl6.manifest 头注补双通道口径；src-tauri/src/git/CLAUDE.md:74 补半句。
7. 验证为先（计数等价）：改造前先跑 cargo test -- --test-threads=1 记录全量计数 N1；改造后 N2 == N1 且全绿。若 cargo test --lib 仍 0xc0000139 → 停止回退（逆操作恢复重组与红线），CP-040 转登记，不得带病前进。
完成后报告：修改文件清单 + N1/N2 计数 + 是否走了回退预案。`, { label: 'cp040-embed-manifest' })

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo test --lib -- --test-threads=1
2. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npx tauri build --debug --no-bundle
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 02 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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

return { fixResult, testResult, verifyResult }
