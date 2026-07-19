// =====================================================================
// Stage 01: 后端 git 能力 + IPC 契约（CV-BE-01~04）
// =====================================================================
// 跨边界契约（写死，agent 不各自推断）：
//   - 命令: git_file_at_head(repo_path: String, file_path: String) -> Result<String, AppError>
//   - TS wrapper: gitFileAtHead(repoPath: string, filePath: string): Promise<string>（src/ipc/git.ts）
//   - HEAD 不存在错误: AppError::Git，消息含 "HEAD 中不存在"
//   - GitStatusEntry: { path, status, old_path: Option<String> } → camelCase → { path, status, oldPath: string | null }
// fix-loop 调用时 args.constraints 传 ""（本 Stage 无特殊纪律）
// =====================================================================

export const meta = {
  name: 'stage01-git-backend',
  description: 'Stage 01: git_file_at_head 命令 + git_status 递归未跟踪 + oldPath 字段 + IPC wrapper（CV-BE-01~04）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:/data/learn/code/slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/commit-view/checklist.md 对应 ID 条目（先读再动手）。
本 Stage 为单 agent 执行全部 4 项（同一契约链，前后端字段强耦合不拆分）。
只允许做编译级检查（cargo test --no-run / npx tsc --noEmit），禁止跑含 PTY spawn 的真实测试（资源共享冲突），真实测试由全量测试 agent 单点跑。

跨边界契约（写死，不得偏离）：
- 命令签名: git_file_at_head(repo_path: String, file_path: String) -> Result<String, AppError>
- TS wrapper: gitFileAtHead(repoPath: string, filePath: string): Promise<string>，invoke 参数 { repoPath, filePath }
- HEAD 不存在/无 commit（UnbornBranch）: AppError::Git，消息含 "HEAD 中不存在"
- GitStatusEntry 序列化: { path, status, oldPath }（camelCase；非 renamed 为 null）`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: "git-backend",
    prompt: `你负责 CV-BE-01 / CV-BE-02 / CV-BE-03 / CV-BE-04 全部 4 项：

【CV-BE-01】git_status 递归列出未跟踪文件
- 文件：src-tauri/src/git/mod.rs 的 git_status()
- 改动：StatusOptions 链追加 .recurse_untracked_dirs(true)，不动其他选项
- 已知副作用（FR-4）：explorer 新目录下子文件逐个标红（现状仅目录行着色）——grep explorer git 着色相关测试断言，按新语义同步更新
- L1 测试（mod.rs 内 tests 模块追加）：新建含多文件的未跟踪目录 → statuses 中每个文件单独出现（非目录单条目 foo/）

【CV-BE-02】GitStatusEntry 新增 oldPath 字段
- 文件：src-tauri/src/git/mod.rs（GitStatusEntry + status 收集循环）；src/types/git.ts
- 前提（必须先做）：git_status 的 StatusOptions 链追加 .renames_head_to_index(true).renames_index_to_workdir(true)，不动其他选项——git2 默认不做 rename 检测，不开则 INDEX_RENAMED/WT_RENAMED 状态位运行时永不置位（重命名表现为 deleted + untracked 两条），oldPath 恒 null
- 改动：Rust 侧加 pub old_path: Option<String>（serde camelCase 自动转 oldPath）；renamed 条目（INDEX_RENAMED | WT_RENAMED）从对应 delta（head_to_index / index_to_workdir）的 old_file().path() 取旧路径，workdir join + \\\\→/（与 path 同格式）；非 renamed → None
- TS 侧：GitStatusEntry 接口加 oldPath: string | null（含注释说明 renamed 时为旧绝对路径）
- L1 测试：renamed 场景用 git mv 真实构造（blob 完全相同必被 rename 检测识别），断言 oldPath 为旧绝对路径——禁止直接构造 git2::Status 状态位（直接构造测试绿但行为不存在）；非 renamed 为 null；serde JSON 含 "oldPath"（camelCase）

【CV-BE-03】新命令 git_file_at_head
- 文件：src-tauri/src/git/mod.rs；src-tauri/src/lib.rs（generate_handler! 注册，照 git::git_status 行格式追加）
- 签名：pub async fn git_file_at_head(repo_path: String, file_path: String, state: State<'_, AppState>) -> Result<String, AppError>
- 实现：validate_path_within_root 校验 → get_or_open_repo 复用 → spawn_blocking 内：repo.head() → peel_to_tree → 按 workdir 相对路径（dunce::simplified 处理，\\\\→/）tree.get_path(rel) → find_blob → String::from_utf8_lossy；UnbornBranch 与 NotFound 统一 AppError::Git(format!("文件在 HEAD 中不存在: {rel}"))
- L1 测试：有 commit 读取内容正确；无 commit 报错含"HEAD 中不存在"；文件不在 HEAD 报错；子目录文件相对路径正确

【CV-BE-04】IPC wrapper gitFileAtHead
- 文件：src/ipc/git.ts（照 gitDiff 同文件格式）；src/__tests__/ipc-contract.test.ts
- wrapper：export async function gitFileAtHead(repoPath: string, filePath: string): Promise<string> → invoke("git_file_at_head", { repoPath, filePath })
- contract 测试四维：命令名 git_file_at_head / 参数 { repoPath, filePath } 精确匹配 / 正常返回透传 / 异常传播`,
  },
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
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

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:/data/learn/code/slTerminal）。
先读 docs/commit-view/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
