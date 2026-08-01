// =====================================================================
// Stage 01 Workflow — 后端三层 hooks 子树读写命令
// =====================================================================
// 跨边界契约（contract.md C13-1，2026-07-31 修订）：
//   - Rust 命令名: hooks_config_read, hooks_config_write
//   - read 签名: (layer: String, project_path: Option<String>) -> Result<Value, AppError>
//     返回该层 settings.json 的 hooks 子树；文件不存在/无 hooks 键 → Null；JSON 损坏 → Err
//   - write 签名: (layer: String, hooks: Value, project_path: Option<String>) -> Result<(), AppError>
//     后端 read-modify-write merge：仅替换根对象 hooks 键，保留 permissions/env 等其他字段；损坏拒绝
//   - layer 仅允许 "user" / "project" / "local"
//   - user 层: ~/.claude/settings.json（绕过沙箱）；project 层: <projectPath>/.claude/settings.json
//   - local 层: <projectPath>/.claude/settings.local.json；project/local 层 project_path 须过沙箱
// =====================================================================

export const meta = {
  name: 'stage-01-backend',
  description: '后端 hooks 配置三层读写命令（hooks 子树级 + merge 保留）+ L1 测试',
  phases: [
    { title: '并行重构' },
    { title: '全量测试' },
    { title: '逐项验证' },
  ],
}

const PREAMBLE = `项目根目录 D:\\data\\learn\\code\\slTerminal。
纪律：只修改分配给你的文件/项，不顺手改无关代码（surgical changes）；代码注释用中文；完成后报告修改的文件清单与每项改动摘要。
禁区：compute_conpty_flags 固定 0x7，任何 agent 不得修改 ConPTY flags（含其 4 条守卫测试）——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮输入
背景：修复要点详见 docs/hooks-dev/phase3/checklist.md 对应 ID 条目（先读再动手）。`

// === Phase 1: 并行重构 ===
phase('并行重构')
const parallelAgents = [
  {
    label: 'backend-config',
    prompt: `你负责 P3-BE-01/02/03/06/07/08 与 P3-TE-01/02。

【P3-BE-01】新建 src-tauri/src/hooks/config.rs：
- 实现三层配置路径纯函数：user 层 → ~/.claude/settings.json（dirs::home_dir() 解析）；project 层 → <projectPath>/.claude/settings.json；local 层 → <projectPath>/.claude/settings.local.json。
- project/local 层的 project_path 入参须经 crate::state::validate_path_within_root 沙箱校验。
- 父目录不存在时自动 create_dir_all（仅写入路径）。

【P3-BE-02】实现 hooks_config_read(layer, project_path) -> Result<serde_json::Value, AppError>：
- 读文件 → 提取并返回 hooks 子树（非整文件）。
- 文件不存在、或文件合法但无 hooks 键 → 返回 Ok(Value::Null)。
- JSON 损坏 → 返回 Err（AppError::Validation 或 Io 系），不返回 Null——防止面板在损坏文件上编辑后 merge 丢其他字段（对齐 C9 注入的非法中止先例）。
- layer 仅允许 user/project/local，非法返回 AppError::Validation。

【P3-BE-03】实现 hooks_config_write(layer, hooks, project_path) -> Result<(), AppError>：
- 要求 hooks 为 JSON Object（hooks 子树），否则返回 AppError::Validation。
- read-modify-write：读原文件（不存在视为空对象 {}，损坏 → Err 拒绝，不覆盖用户文件）→ 将根对象 hooks 键替换为入参 → 写回。原样保留 permissions/env/$schema 等其他字段。
- 原子写：NamedTempFile::new_in -> write -> flush -> persist（照 settings.rs 先例，明确不做 .bak）。
- 所有阻塞 IO 在 spawn_blocking 内执行（硬约束 #3）。

【P3-BE-06/07】user 层命令体内不得调用 validate_path_within_root；project/local 层缺失 project_path 返回 Validation，校验失败返回 AppError::PathNotAllowed。

【P3-BE-08】非法 layer、非法 hooks、JSON 损坏统一走 AppError::Validation；IO 错误走 AppError::Io / AppError::IoKind。

【P3-TE-01/02】在 config.rs 底部写 #[cfg(test)] mod tests（tempfile::tempdir 隔离）：
- user 层：文件不存在返回 Null、无 hooks 键返回 Null、合法 hooks 子树读取、原子写后内容正确、父目录自动创建。
- merge 保留：文件预置 permissions/env 字段，write 后断言其他字段原样保留。
- 损坏拒绝：损坏 JSON 时 read 返回 Err、write 返回 Err（不被覆盖）。
- project/local 路径解析正确、沙箱校验失败分支、非 Object hooks 返回 Validation。

约束：本文件为新建，不修改阶段 1 已有的 hooks/inject.rs、hooks/signal.rs、hooks/usage.rs。`
  },
  {
    label: 'backend-register',
    prompt: `你负责 P3-BE-04/05。

【P3-BE-04】修改 src-tauri/src/hooks/mod.rs：
- 新增 pub mod config;。
- 在 mod.rs 的导出中加入 config::hooks_config_read 与 config::hooks_config_write（与既有 hooks_inject / hooks_uninstall / hooks_injection_status / hooks_context_usage 并列风格）。

【P3-BE-05】修改 src-tauri/src/lib.rs：
- 在 generate_handler! 宏中追加 hooks_config_read、hooks_config_write。

约束：只导入 config.rs 中已存在的函数（函数名以上方契约为准）；不重复实现逻辑。若 config.rs 尚未就绪（并行执行），按契约函数名先行导入即可。`
  }
];
const refactorResults = await parallel(
  parallelAgents.map(a => () => agent(`${PREAMBLE}\n\n${a.prompt}`, { label: a.label }))
)

// === Phase 2: 全量测试 ===
phase('全量测试')
const testResult = await agent(`
在项目根目录 D:\\data\\learn\\code\\slTerminal 执行全量验证。以下命令相互独立，并行启动执行，收集全部结果：
1. cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
2. cargo test --manifest-path src-tauri/Cargo.toml hooks::config -- --test-threads=1
逐条报告：每命令一行 exit code + 通过/失败；失败时附前 50 行错误摘要，勿贴完整输出。cargo 系命令共享 target 目录锁会排队，属正常勿中止。
`, { label: 'full test suite' })

// === Phase 3: 逐项验证 ===
phase('逐项验证')
const rawVerify = await agent(`
逐项检查 Stage 01 的改动是否实际生效（项目根 D:\\data\\learn\\code\\slTerminal）。
先读 docs/hooks-dev/phase3/workflows/verify/stage-01.md 获取断言清单，用 Grep/Read 逐条核实并给出证据（文件+行号）。
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
