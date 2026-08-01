// =====================================================================
// Stage 01: 后端历史会话扫描命令
// 覆盖项: SEC-02、BE-01、BE-02、BE-03、BE-04、BE-05、BE-06、BE-09
// 跨边界契约（写死，agent 不各自推断）:
//   命令: claude_history_scan() → HistorySession[]
//   HistorySession（Rust snake_case ↔ TS camelCase）:
//     session_id ↔ sessionId: string
//     cwd ↔ cwd: string | null
//     title ↔ title: string | null（后端按回退链解析；null → 前端显示 sessionId 前 8 位）
//     title_source ↔ titleSource: "customTitle"|"aiTitle"|"summary"|"firstPrompt"|"none"
//     first_prompt ↔ firstPrompt: string | null（≤200 字符，后端截断）
//     mtime_ms ↔ mtimeMs: number
//     cwd_exists ↔ cwdExists: boolean（cwd 为 null 时恒 false）
// fix-loop 调用本 Stage 时 args.constraints 传空串（无特殊纪律）
// =====================================================================

export const meta = {
  name: 'stage01-backend-scan',
  description: 'Stage 01: 后端历史会话扫描命令（轻量解析+标题回退链+env 覆盖）',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 checklist 对应 ID 条目（先读再动手）。先读 docs/claude-history-view/checklist.md 中 SEC-02、BE-01..06、BE-09 条目 + docs/claude-history-view/stages.md Stage 01 实现要点 + docs/claude-history-view/README.md 第 4.1/4.2 节（数据模型与标题回退链），再动手。本 Stage 不自行跑 cargo test 执行（编译级检查用 cargo check），真实执行由全量测试 agent 单点跑。`

// === Phase 1: 并行重构（单 agent，无文件重叠问题）===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-scan-agent',
    prompt: `你负责 SEC-02、BE-01、BE-02、BE-03、BE-04、BE-05、BE-06、BE-09，全部落在新模块 src-tauri/src/claude_history/ + src-tauri/src/lib.rs：

【BE-01 模块骨架 + DTO + lib.rs 注册】
- 新建 src-tauri/src/claude_history/ 目录，含 mod.rs / scan.rs / jsonl.rs 三文件（ops.rs 归 Stage 02，本 Stage 不建）。
- mod.rs 定义 DTO：HistorySession（七字段：session_id/cwd/title/title_source/first_prompt/mtime_ms/cwd_exists，serde camelCase 序列化）+ TitleSource 枚举（serde camelCase，五变体 CustomTitle/AiTitle/Summary/FirstPrompt/None 序列化为 "customTitle"/"aiTitle"/"summary"/"firstPrompt"/"none"）。
- 字段类型：session_id: String；cwd: Option<String>；title: Option<String>；title_source: TitleSource；first_prompt: Option<String>；mtime_ms: u64；cwd_exists: bool。
- src-tauri/src/lib.rs：mod claude_history; 声明 + claude_history_scan 进 generate_handler!（照既有命令注册模式）。

【BE-06 + SEC-02 扫描根单点】
- scan.rs 提供 resolve_projects_root() 单点函数：env SLTERM_CLAUDE_PROJECTS_DIR 非空则用之，缺省 dirs::home_dir()/.claude/projects。
- 每次 scan 调用时读取 env（不缓存），供 E2E 隔离注入；代码注释标注「生产不设置此 env，仅测试用途」（SEC-02）。

【BE-02 claude_history_scan 命令】
- 签名：async fn claude_history_scan() -> Result<Vec<HistorySession>, AppError>。
- 遍历扫描根的一级子目录 → 收集其中 *.jsonl 文件：排除文件名以 agent- 开头的平铺形态、排除非 UUID 形态的文件名（UUID 形态校验复用为独立纯函数，Stage 02 还会用）；<id>/subagents/ 下的 jsonl 天然不命中（只扫一级子目录的直属文件，不递归）。
- 阻塞 I/O 全部在 tokio::task::spawn_blocking 内（硬约束 #3）。
- 单文件解析失败 → 降级条目（sessionId 取文件名、mtime_ms 取文件 mtime、其余字段 None/titleSource=None/cwd_exists=false），不阻塞整体扫描。
- 扫描根不存在 → 返回空 Vec（非 Err）。
- 编码目录名只是 cwd 的编码形式，cwd 一律从 jsonl 内容解析，不依赖目录名解码。

【BE-03 头部解析纯函数（jsonl.rs）】
- 顺序扫描至首条可见 user prompt 或 512KB 上限（常量 HEAD_SCAN_LIMIT_BYTES = 512 * 1024）：
  - 沿途收集：cwd（首个含 cwd 字段的行）、custom-title、ai-title、summary 候选。
  - 首条可见 prompt 规则：type=="user" 且 message.content 为字符串；跳过 isMeta:true 行、content 为数组的行（tool_result 载体）、字符串以 "<" 开头的行（<command-name>/<local-command-caveat>/<local-command-stdout>）；未知 type 行忽略；文件末尾截断行容忍（JSON 解析失败即停止，不报错）。
- first_prompt 提取后截断至 ≤200 字符。

【BE-04 尾部 64KB last-wins 标题】
- 照 src-tauri/src/hooks/usage.rs 的 TRANSCRIPT_TAIL_BYTES 先例：从文件尾部读最多 64KB，从中途起始则跳过首行（截断行），逆行扫描找最后一条 custom-title/ai-title/summary 候选（last-wins）。
- 标题回退链（决策 22）：custom-title > ai-title > summary > 首条 prompt；全无所属 → title=None、title_source=None。

【BE-05 mtime + cwdExists】
- mtime_ms = 文件修改时间（决策 26，毫秒时间戳）。
- cwd_exists = cwd 目录存在性检查（cwd 为 null 时恒 false，不做 fs 调用）。

【BE-09 L1 测试】
- 测试嵌入各源文件 #[cfg(test)] mod tests（照项目惯例）。
- 覆盖：标题回退链 5 态（custom-title 优先/ai-title/summary/首条 prompt/全无）、prompt 跳过 4 类（isMeta/content 数组/< 开头/未知 type）、EOF 截断行容忍、无 cwd 会话、大文件仅头尾（构造 >512KB 文件 + 尾部标题行，验证不读全文）、扫描排除 3 类（agent-*.jsonl 平铺/非 UUID 文件名/subagents 目录不命中）、env 覆盖（SLTERM_CLAUDE_PROJECTS_DIR 指向 tempdir）、降级条目（损坏 jsonl → 仅 sessionId+mtime）。
- tempfile::tempdir() 隔离；路径比较前 dunce::canonicalize（8.3 短名坑，照 git/CLAUDE.md 先例）；env 操作测试依赖 --test-threads=1 串行（门禁已保证，测试内注释说明）。

约束：命令返回 Result<_, AppError>；DTO 七字段与脚本头契约逐字一致；零新依赖（serde/serde_json/tokio/tempfile/dirs/dunce 均已在 Cargo.toml）；不碰 project_root 沙箱（home 目录扫描照 hooks/config.rs user 层先例）。`,
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
逐项检查 Stage 1 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/claude-history-view/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
