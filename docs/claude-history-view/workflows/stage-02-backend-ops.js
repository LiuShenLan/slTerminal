// =====================================================================
// Stage 02: 后端写操作（删除/重命名）
// 覆盖项: SEC-01、BE-07、BE-08、BE-10
// 跨边界契约（写死，agent 不各自推断）:
//   claude_history_delete(sessionId) → ()        invoke 参数 camelCase: { sessionId }
//   claude_history_rename(sessionId, newTitle) → ()   { sessionId, newTitle }
//   delete/rename 定位: 后端遍历扫描根定位 <sessionId>.jsonl，不接受前端路径入参
//   重命名写入格式: 追加一行 {"type":"custom-title","customTitle":<名>,"sessionId":<id>}
// fix-loop 调用本 Stage 时 args.constraints 传空串（无特殊纪律）
// =====================================================================

export const meta = {
  name: 'stage02-backend-ops',
  description: 'Stage 02: 后端删除/重命名命令（sessionId 校验+custom-title 追加写）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。先读 docs/claude-history-view/checklist.md 中 SEC-01、BE-07/08、BE-10 条目 + docs/claude-history-view/stages.md Stage 02 实现要点 + docs/claude-history-view/README.md 第 4.4 节（删除/重命名语义），再动手。本 Stage 不自行跑 cargo test 执行（编译级检查用 cargo check），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（单 agent）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-ops-agent',
    prompt: `你负责 SEC-01、BE-07、BE-08、BE-10，落在 src-tauri/src/claude_history/{mod.rs, ops.rs}（Stage 01 已建模块骨架与 scan.rs/jsonl.rs，先读现状）+ src-tauri/src/lib.rs：

【SEC-01 sessionId 校验 + 定位不信托前端】
- ops.rs 提供 sessionId 严格校验纯函数：仅接受 UUID 形态（8-4-4-4-12 十六进制带连字符，大小写不敏感），拒绝含 ".."/路径分隔符（/ 与 \\）/空串/超长等一切非 UUID 输入，非法 → AppError::Validation。
- 文件定位：遍历 resolve_projects_root() 扫描根一级子目录找 <sessionId>.jsonl，前端不传任何路径参数。

【BE-07 claude_history_delete】
- 签名：async fn claude_history_delete(session_id: String) -> Result<(), AppError>。
- 流程：SEC-01 校验 → spawn_blocking 内定位 → 删除 <id>.jsonl 文件 + 同名 <id>/ 目录（存在则 remove_dir_all，该目录存 subagents 等附属数据）；文件不存在 → Err（AppError 合适变体，消息含「不存在」语义）。

【BE-08 claude_history_rename】
- 签名：async fn claude_history_rename(session_id: String, new_title: String) -> Result<(), AppError>。
- 流程：SEC-01 校验 + newTitle 非空（trim 后）且 ≤200 字符（非法 → AppError::Validation）→ spawn_blocking 内定位 → OpenOptions append 模式向 <id>.jsonl 尾部追加一行 JSON：{"type":"custom-title","customTitle":<名>,"sessionId":<id>}（决策 22；JSON 序列化经 serde_json 构造，禁手拼字符串防注入）；文件不存在 → Err。
- 追加前若文件非空且末字节非 \\n，先补 \\n 再写行（保证 JSONL 行完整）。

【lib.rs 注册】
- claude_history_delete、claude_history_rename 进 generate_handler!。

【BE-10 L1 测试】
- 测试嵌入 ops.rs #[cfg(test)] mod tests。
- 覆盖：delete 范围（jsonl + 同名目录一并删除；目录不存在时仅删文件不报错）、rename 追加格式与内容（追加行 JSON.parse 后三字段逐字断言：type=="custom-title"、customTitle==新名、sessionId==id；原文件内容不变）、sessionId 非法拒绝（含 ".."/含斜杠/含反斜杠/空串/非 UUID 各一条）、文件不存在 Err、newTitle 空/纯空白/>200 字符拒绝。
- tempfile::tempdir() 隔离；env SLTERM_CLAUDE_PROJECTS_DIR 指向 tempdir（扫描根定位依赖 env 覆盖）；路径比较前 dunce::canonicalize；env 操作测试依赖 --test-threads=1 串行（测试内注释说明）。

约束：两命令阻塞 I/O 均在 spawn_blocking 内（硬约束 #3）；返回 Result<(), AppError>；不碰 project_root 沙箱；零新依赖。`,
  },
]
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 3: 全量测试（命令相互独立，并行启动执行，收集全部结果）===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. npx tsc --noEmit
2. npx eslint src/
3. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
4. npm test
5. cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 4: 逐项验证（断言清单唯一真值源）===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 2 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/claude-history-view/workflows/verify/stage-02.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
