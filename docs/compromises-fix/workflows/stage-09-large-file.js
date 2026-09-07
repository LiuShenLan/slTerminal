// =====================================================================
// Stage 09 编辑器大文件（CP-022 单项 Stage，新功能 feat）
// fix-loop constraints：本 Stage 无特殊纪律（省略 args.constraints）
// =====================================================================

export const meta = {
  name: 'stage-09-large-file',
  description: 'S09 编辑器大文件只读分片浏览（CP-022）',
  phases: [
    { title: '修复' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入。
背景：修复步骤六段式真值源 = docs/compromises-fix/checklist.md CP-022 条目（先读再动手，照抄步骤不另起方向）；本条为契约/模块级设计，代码给接口签名骨架——骨架内签名与常量值写死，实现细节按骨架落地。`

// === Phase 1: 修复（单 agent，跨前后端 + 新 IPC 命令）===
phase('修复')
const fixResult = await agent(`${PREAMBLE}

你负责 CP-022（大文件只读分片浏览；10MB 可编辑上限与 1MB 警告阈值常量不动）：
照抄 checklist CP-022 步骤 1-4：
1. 后端新命令 fs_read_file_range(path, offset_bytes, length_bytes) -> Result<String, AppError>（spawn_blocking；UTF-8 边界裁剪——裁到首个完整字符边界）；三处注册（lib.rs generate_handler! + build.rs AppManifest + capabilities/default.json）；DTO 双边（ts-rs 形态按 S05 落地约定）；src/ipc/fs.ts 增 readFileRange wrapper（照抄代码块）。
2. 新建 src/panels/editor/largeFileViewer/ 三文件：LargeFileViewer.tsx / useLineIndex.ts / blockCache.ts（接口签名骨架照抄 checklist 代码块；READ_BLOCK_BYTES=256KB / BLOCK_CACHE_LIMIT=32 / LARGE_FILE_LINE_HEIGHT=20 / overscan 20 行写死；虚拟化窗口参照 FileTree 手实现先例；顶部信息条「只读浏览(文件大小)，可编辑上限 10MB」）。不经 panelRegistry 新类型（同面板内形态切换）。
3. 超限拒绝改引导：useCodeMirror.ts:305-309 分支改返回 largeFile 信号（filePathRef.current = undefined 保留防误保存）；EditorPanel 检测 largeFile → 渲染 LargeFileViewer；GitShowPanel.tsx:177-185 超限分支改渲染（sourceLabel="git show"，1MB-10MB 警告 header 不变）；DiffPanel.tsx:245-258 任一侧超限该侧改 LargeFileViewer（对齐/滚动同步对该侧降级，占位对齐装饰跳过），双侧超限分栏各自。
测试：L1 新 fs_read_file_range_tests.rs（空区间/越界 clamp/多字节边界不截断/不存在路径 AppError）；L2 新 large-file-viewer.test.tsx（mock readFileRange：行索引扩展/窗口渲染/ LRU 驱逐）+ useLineIndex 单测；改 use-code-mirror.test.ts:1029（→ 返回 largeFile 信号且 view 不创建）、gitshow-panel.test.tsx:345、diff-panel.test.tsx:729,741；防复发 before 形态断言（拒绝文案不再出现）。
文档：editor/CLAUDE.md FE-31 节改写为四层防线口径；panels/CLAUDE.md docViewer/gitshow/diff 节补引导口径；src/ipc/CLAUDE.md 登记新 wrapper。
自查：条目「验证」段全部满足；npx tsc --noEmit；cargo test --no-run。
完成后报告修改文件清单 + 人工/L4 验证点待办（>10MB 只读浏览滚动至 EOF 抽样断言）。`, { label: 'cp022-large-file-viewer' })

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
逐项检查 Stage 09 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/compromises-fix/workflows/verify/stage-09.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
