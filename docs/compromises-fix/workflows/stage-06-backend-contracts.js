// =====================================================================
// Stage 06 后端契约重设计（CP-006 游标分页 ∥ CP-007 实测决策）
// fix-loop constraints 取值唯一真值源 = execution-plan.md「fix-loop 调用规范」表 S06 行
//（本脚本 PREAMBLE_EXTRA 已内含分支互斥纪律，与表值同源）
// =====================================================================

export const meta = {
  name: 'stage-06-backend-contracts',
  description: 'S06 后端契约重设计（CP-006/007）',
  phases: [
    { title: '并行修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
【Stage 特殊纪律】CP-007 的删/修分支由 benchmark 实测数字决定，不两个分支同时做。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md 对应 CP 条目（先读再动手，照抄步骤不另起方向）。
前置确认：S05 ts-rs 已落仓（新契约 DTO 用 ts-rs 定义）。
并行纪律：本阶段只做编译级检查（cargo test --no-run / npx tsc --noEmit），真实测试由全量测试 agent 单点跑。`

// === Phase 1: 并行修复 ===
phase('并行修复')
const parallelAgents = [
  {
    label: 'cp006-readdir-page',
    prompt: `你负责 CP-006（fs_read_dir 游标分页 + FileTree 迁移）：
照抄 checklist CP-006 步骤 1-4：
1. 后端：fs/mod.rs 增 FsReadDirPage（ts-rs derive 导出 ../src/types/fs.ts）+ READ_DIR_PAGE_DEFAULT=500/READ_DIR_PAGE_MAX=1000；fs_read_dir 签名改 (path, cursor, limit, state)；过滤+排序全量完成后按游标切片（游标 = 排序后序号 base64，opaque）；limit 钳 [1,1000] 缺省 500；.git 过滤与排序语义零变更；不采用 Channel 推送。
2. 前端：src/ipc/fs.ts readDir 改 readDirPage(path, cursor?, limit?)；src/types/fs.ts 为生成物（禁手改）。
3. useFileTree.ts loadRoot 首帧 + 续页拼接（gen 竞态语义不变；refresh/refreshExpanded 同步走分页聚合）。
4. 红线改写：fs/CLAUDE.md BE-21 节重写 + 红线改「禁止无游标全量返回」；adr.md ADR-0009 BE-21 行改写；src/types/CLAUDE.md 补 FsReadDirPage；src/ipc/CLAUDE.md fs 通道同步。
测试：L1 六用例（first_page_has_cursor_when_overflow/last_page_null_cursor/cursor_resume_mid_list/limit_clamped_to_max/sort_order_stable_across_pages/git_filter_still_applied）；既有整表断言用例改分页遍历聚合（逐一点名）；L2 use-file-tree 续页用例 + ipc-fs-contract 键集合断言改 { path, cursor, limit }。
自查：cargo test --no-run；npx tsc --noEmit。
完成后报告修改文件清单。`,
  },
  {
    label: 'cp007-scan-cache',
    prompt: `你负责 CP-007（session 扫描缓存实测决策；人工验证点 = 门槛数字落档）：
照抄 checklist CP-007 步骤 0-3：
0. 先落基准用例 scan_bench_1000_sessions_median_under_50ms（scan.rs mod scan_bench，门槛中位 <50ms / max <200ms 写死），跑出实测中位/max 数字。
1. 机械判定：绿 → 删除分支（步骤 2）；红 → 指纹分支（步骤 3）。实测数字与分支选择写入最终报告（主 agent 落 commit body）。
2. 删除分支：删 SCAN_CACHE/ScanCacheEntry/ScanCacheKey/cache_key_of/cached_scan；scan_sessions 双函数合并单一直扫；agent_history_scan 删 force 参数；run_scan 收敛 provider.scan()；src/ipc/agentHistory.ts 删 force；sessionRefreshTask.ts:27 调用与头注释改写。
3. 指纹分支（仅不达标时）：ScanCacheKey 改目录内容指纹（file_name, mtime_ms, len 排序后 FNV-1a 64 位 + 条目数）；force 通道全保留。
测试按分支执行（删除分支：删五缓存用例 + 两 force 用例、新增 scan_reflects_deletion_immediately 与 command_scan_without_force_param、L2 三件适配；指纹分支：缓存用例改指纹口径 + 新增 session_file_modified 失效用例）；两分支共有 = 基准用例常驻。
文档按分支执行（agent_history/CLAUDE.md BE-19 节重写、红线处置、src/ipc/CLAUDE.md:64、backgroundTasks/CLAUDE.md:41、agentHistory/CLAUDE.md:32、types/CLAUDE.md）。
自查：cargo test --no-run；删除分支 rg "SCAN_CACHE|ScanCacheKey|scan_sessions_with_force" src-tauri/src/agent_history 零命中。
完成后报告：修改文件清单 + 实测中位/max + 走的分支。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
2. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
3. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
4. npx tsc --noEmit
5. npx eslint src/
6. npm test
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 06 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-06.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
总则：不仅核对字面断言，还须 Read 代码判断实现是否达成断言意图——字面通过但意图未达判 partial 并说明理由。
CP-007 分支互斥：b/c 两组断言按修复 agent 报告的实测分支只取其一，另一组标 N/A。
修复 agent 报告（CP-007 分支与实测数字）：
---
${refactorResults[1] ?? '（未返回）'}
---
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
