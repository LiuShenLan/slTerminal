// =====================================================================
// Stage 01 数据防线（BE-01 / BE-02 / FE-01）
// =====================================================================
// fix-loop 调用约定：args.constraints 无需传值（无特殊纪律）
// 跨 agent 契约：无（本 Stage 三 agent 文件零重叠，各自独立）

export const meta = {
  name: 'stage01-data-defense',
  description: 'Stage 01 数据防线：SLTERM_DATA_DIR 隔离 + projects tracing + loadSucceeded 空写守卫',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：禁止改 src-tauri/src/pty/ 任何 ConPTY flags（compute_conpty_flags 固定 0x7，含其 4 条守卫测试——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入）；禁止前端 src/ipc/ 外出现 invoke；禁止硬编码颜色（经 theme/colors.ts token）；禁止 npm run tauri dev 验证；禁止写入真实凭据值（SEC-18，仅允许 sk-test 假值占位符）。
背景：修复要点详见 docs/settings-center-fixes/checklist.md 对应 ID 条目（先读再动手）。
测试纪律：你只做编译级检查（cargo 系用 cargo test --no-run / cargo check），禁止各自跑 cargo test / npm test——真实执行统一由后续全量测试 agent 单点跑。`

// === Phase 1: 并行修复（agent 间文件零重叠）===
phase('并行修复')
const parallelAgents = [
  {
    label: 'be-01-appdir',
    prompt: `你负责 BE-01（照抄 docs/settings-center-fixes/checklist.md 的 BE-01 条目）：
【BE-01】src-tauri/src/app_dir.rs 增加 SLTERM_DATA_DIR 环境变量覆盖
- 位置：src-tauri/src/app_dir.rs 的 app_data_dir()（约 :72-81）
- 步骤：①文件顶部常量区新增 \`const DATA_DIR_ENV: &str = "SLTERM_DATA_DIR";\`；②app_data_dir() 在 \`#[cfg(test)]\` guard 块之后、\`resolve_app_data_dir(std::env::current_exe())\` 之前插入：
\`\`\`rust
// E2E 隔离：环境变量显式指定数据目录（空串视为未设置）
if let Some(dir) = std::env::var_os(DATA_DIR_ENV).filter(|v| !v.is_empty()) {
    return Ok(PathBuf::from(dir));
}
\`\`\`
优先级语义：测试 guard > 环境变量 > exe 同级推导。
- 测试同步：同文件测试模块新增 3 例：①SLTERM_DATA_DIR 生效返回指定目录；②空串被忽略回落 exe 推导；③测试 guard 优先于 env（guard 与 env 同设时返回 guard 值）。每例结束 std::env::remove_var(DATA_DIR_ENV)（Cargo edition 2021，set_var 非 unsafe；L1 强制 --test-threads=1 无竞态）。
- 你只改 src-tauri/src/app_dir.rs。`,
  },
  {
    label: 'be-02-tracing',
    prompt: `你负责 BE-02（照抄 docs/settings-center-fixes/checklist.md 的 BE-02 条目）：
【BE-02】src-tauri/src/projects.rs 命令层补 tracing 打点
- 位置：src-tauri/src/projects.rs 的 load_from_dir（约 :57-88）
- 步骤：纯打点零行为变化——读失败非 NotFound 分支加 \`tracing::warn!(error = %e, path = %path.display(), "projects 读取失败，尝试 .bak")\`；JSON 非法分支加 tracing::warn!；损坏且 .bak 未命中分支加 tracing::error!。.bak 双保险返回语义一字不动。
- 测试同步：无新增（行为不变）。
- 你只改 src-tauri/src/projects.rs。`,
  },
  {
    label: 'fe-01-store-defense',
    prompt: `你负责 FE-01（照抄 docs/settings-center-fixes/checklist.md 的 FE-01 条目）：
【FE-01】src/stores/projects.ts 数据防线（loadSucceeded + 空写守卫 + 上抛 + 结构校验）
- 位置：src/stores/projects.ts 的 loadFromDisk（约 :233-257）、saveToDisk（约 :259-264）、loadAllProjects（约 :272-279）、模块标志区（约 :291-322）
- 步骤（可照抄代码见 checklist FE-01 条目，逐项落实）：
  ① 模块标志区（\`let initialized = false;\` 附近）新增 \`let loadSucceeded = false;\`
  ② loadFromDisk：删 try/catch 让异常上抛；JSON.parse 后、set 前插结构校验（projects 字段存在但非对象/为 null/为数组即 throw new Error("项目数据格式异常：projects 字段不是对象")）；set(...) 成功后 \`loadSucceeded = true;\`
  ③ saveToDisk 首行后插空写守卫：\`!loadSucceeded && Object.keys(projects).length === 0\` 时 console.warn("[slTerminal] 拒绝空写：项目数据未成功加载且当前为空（防覆盖磁盘数据)") + return
  ④ loadAllProjects：删 catch 直传 \`await useProjects.getState().loadFromDisk();\`
  ⑤ 新增导出：\`export function markLoadSucceeded(): void { loadSucceeded = true; }\`
  ⑥ _resetPersistence 内加 \`loadSucceeded = false;\`
- 测试同步：src/__tests__/projects.test.ts 新增 5 例（beforeEach 已调 _resetPersistence）：①loadFromDisk IPC reject → 异常上抛且后续空写被拒；②结构校验：返回 {projects: 1} → throw 格式异常；③loadFromDisk 成功 → saveAllProjects 空状态正常写盘；④未加载时空写被拒且磁盘无写入调用；⑤markLoadSucceeded() 后空写放行。用例命名中文描述语义。
- 你只改 src/stores/projects.ts 与 src/__tests__/projects.test.ts。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证（断言清单唯一真值源：verify/stage-01.md）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/settings-center-fixes/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
