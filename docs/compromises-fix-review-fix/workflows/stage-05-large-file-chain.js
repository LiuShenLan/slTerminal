// =====================================================================
// Stage 05 · 大文件链路（BE-05 + FE-03/04/05/08）
// =====================================================================
// 逐 ID 对照 docs/compromises-fix-review-fix/checklist.md 原文编写。
// 串行纪律：全程 agent 严格串行；本 Stage 三 agent 为**顺序依赖** A→B→C
// （editor-precheck 消费 fs-stat 的 statFile；large-viewer 消费 statFile +
// 信号形态），顺序写死勿调换。
//
// 跨边界契约（写死，agent 不各自推断）：
//   - IPC 命令：fs_stat({ path: string }) -> FsMetadata
//   - FsMetadata = { sizeBytes: number, mtimeMs: number | null }
//     （Rust derive TS，snake_case→camelCase 经 ts-rs 生成，禁手改 src/types/）
//   - 前端 wrapper：src/ipc/fs.ts 增 statFile(path: string): Promise<FsMetadata>
//   - LargeFileSignal = { filePath: string }（删 sizeBytes）
//   - LargeFileViewerProps 删 fileSizeBytes
//   - useLineIndex 签名：(filePath: string, fileRev: number)；
//     fileKey 复合 `${filePath}#${fileRev}`
//
// fix-loop 调用本 Stage 时 args.constraints 取值见 execution-plan.md
// 「fix-loop args 规范」表 Stage 05 行（值单源在彼，本注释不复制）。
// =====================================================================

export const meta = {
  name: 'stage-05-large-file-chain',
  description: 'S05 大文件链路：fs_stat 真实字节 + 行色响应式 + 块缓存失效重扫 + 10MB 预检前置',
  phases: [
    { title: '串行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
L1 定向红线（TQ-COV-06）：cargo 定向测试一律 \`cargo test --test lib_tests <filter> -- --test-threads=1\`，禁裸 filter/--lib。
跨边界契约（写死）：fs_stat({ path: string }) -> FsMetadata{ sizeBytes: number, mtimeMs: number | null }；wrapper statFile(path)；LargeFileSignal 仅 { filePath }；useLineIndex(filePath, fileRev)。
DTO 纪律：src/types/fs.ts 为 ts-rs 生成物禁手改——改 Rust 后跑 cargo test --test lib_tests export_bindings -- --test-threads=1 生成。
背景：修复要点详见 docs/compromises-fix-review-fix/checklist.md 对应 ID 条目（先读六段式原文再动手，严格照其步骤执行）。`

// === Phase 1: 串行重构（顺序依赖 A→B→C 写死）===
phase('串行重构')
const agents = [
  { label: 'fs-stat', prompt: `你负责 BE-05（先读 checklist BE-05 六段式原文）——fs_stat 命令全链：
src-tauri/src/fs/mod.rs 加 fs_stat 命令 + FsMetadata DTO（derive TS）；三处注册齐全——src-tauri/src/lib.rs generate_handler!、src-tauri/build.rs commands（42→43）、src-tauri/capabilities/default.json allow-fs-stat。
DTO 生成：cargo test --test lib_tests export_bindings -- --test-threads=1 生成 src/types/fs.ts（禁手改）；src/ipc/fs.ts 加 statFile wrapper（契约：fs_stat({ path }) -> FsMetadata{sizeBytes:number, mtimeMs:number|null}）。
测试：L1 三用例 + L2 ipc 契约用例（落点照 checklist BE-05 测试同步节）；文档：src-tauri/src/fs/CLAUDE.md + src/ipc/CLAUDE.md 按文档同步节登记。
自验：cargo test --test lib_tests fs -- --test-threads=1 绿；cargo clippy -D warnings 绿；git diff --exit-code -- src/types 绿（导出后无漂移——TE-03 守卫形态本地等价）。` },
  { label: 'editor-precheck', prompt: `你负责 FE-08 + FE-04 信号侧（先读 checklist 两条目六段式原文；fs_stat/statFile 已由前序 agent 落地，直接消费）：
【FE-08】src/panels/editor/useCodeMirror.ts:352-377 读盘前插 stat 预检——meta.sizeBytes > MAX → setLargeFile({filePath}) 零读盘返回；> WARN → confirmDialog 用真实字节文案（formatSize(meta.sizeBytes)）；if (filePathRef.current === undefined) return 判用户取消；读后保留 doc.length > MAX 复核（TOCTOU 防线）。
【FE-04 信号侧】LargeFileSignal 删 sizeBytes（仅留 filePath）；EditorPanel.tsx:47-49、GitShowPanel.tsx:304、DiffPanel.tsx:732,751 删 prop/信号字段（GitShow/Diff 的 10MB 判定保留 text.length——blob 无 stat 通道，checklist 已登记收窄）。
测试同步 + 文档（src/panels/editor/CLAUDE.md、src/panels/CLAUDE.md）按 checklist 两条目对应节。
自验：npx tsc --noEmit 绿 + npx vitest run（checklist 点名测试文件）绿。` },
  { label: 'large-viewer', prompt: `你负责 FE-03 + FE-04 viewer 侧 + FE-05（先读 checklist 三条目六段式原文；statFile 与信号形态已由前序两 agent 落地）：
【FE-03】src/panels/editor/largeFileViewer/LargeFileViewer.tsx:27 删 LINE_TEXT_COLOR 模块级常量；组件内 useState(() => schemeRegistry.getActive().editor.overrides.plainText) + useEffect onDidChange 订阅；LINE_FONT_FAMILY（:29）保留（静态规格）。
【FE-04 viewer 侧】LargeFileViewerProps 删 fileSizeBytes；组件内 fileMeta state（fs.statFile 挂载 effect）；信息条 fileMeta !== null ? formatSize(fileMeta.sizeBytes) : "…"。
【FE-05】src/panels/editor/largeFileViewer/blockCache.ts 加 fileGen Map + invalidateFile(filePath)（代际递增 + 前缀清 cache，inflight 保留靠代际比对）；readBlock task 启动记 gen、写缓存前比对；_resetBlockCache 清 fileGen；useLineIndex 签名改 (filePath, fileRev)、fileKey 复合 \`\${filePath}#\${fileRev}\`；LargeFileViewer 加 fileRev state + baseMetaRef + onFsEvent 订阅（照 src/panels/editor/useCodeMirror.ts:587-623 形态：Modify 过滤 + 路径归一化命中 → 重 stat 比对 → invalidateFile + setFileRev+1），静默重载（只读无 dirty）。
测试同步：src/__tests__/large-file-viewer.test.tsx 按 checklist 三条目测试同步节补/改用例。
自验：npx vitest run large-file-viewer 绿；npx tsc --noEmit 绿。` },
]
const refactorResults = []
for (const a of agents) {
  const r = await agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label })
  if (!r) break
  refactorResults.push(r)
}

// === Phase 2: 全量测试（逐条串行；前后端混合 Stage 全门禁）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:/data/learn/code/slTerminal 执行全量验证。以下命令**逐条串行执行**（前一条 exit 后再起下一条，禁并行——CP-007 基准负载敏感）：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
5. npm test
6. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。npm test 与 cargo test 各报告最终统计行原文。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 05 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix-review-fix/workflows/verify/stage-05.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
