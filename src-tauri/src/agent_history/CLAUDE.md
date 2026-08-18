# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

历史会话聚合层（Stage 04，MC-301~305）——**聚合层 + `CliHistoryProvider` trait + `claude/` provider** 三层结构，替代原 `claude_history/` 单 CLI 实现：

- **聚合层**（`mod.rs`）：`AgentHistorySession` DTO（IPC 契约八字段，serde camelCase）+ `AgentHistoryTitle` DTO（IPC 契约两字段，运行中会话标题通道）+ `is_uuid_filename` 可复用工具 + 三条泛化命令（`agent_history_scan(cliId, force)` 带缓存聚合 / `agent_history_delete(cliId, sessionId)` / `agent_history_read_title(cliId, sessionId)`）
- **`provider.rs`**：`CliHistoryProvider` trait（scan / delete / validate_session_id / **read_title** 四方法）+ cliId 键静态注册表（claude 为首个实现，行为零改动）
- **`claude/`**：claude history provider（scan/jsonl/ops 整体下沉；`TitleSource` 五态 + `ScanRootGuard` 测试守卫）

核心数据流：

```
claude 会话文件（~/.claude/projects/<cwd 编码>/<sessionId>.jsonl）
  → claude/scan.rs 扫描根一级子目录 + 头部 512KB/尾部 64KB 轻量解析
  → 聚合层 agent_history_scan 串行聚合全部已注册 provider → 前端历史区
  → agent_history_delete(cliId, sessionId)：validate_session_id 前置 → 定位 → 删除
```

## 架构决策

### 聚合语义（MC-303）

`agent_history_scan(cli_id, force)` 遍历全部已注册 provider 串行聚合；`scan()` 无 Err 通道——单 provider 失败（内部降级为空/部分结果）天然不阻塞其他 provider（照单文件降级条目契约的语义层级提升）；全部空 → 空数组（`Ok` 非 `Err`）。

### 扫描缓存 + force 通道（BE-19，S12）

`claude/scan.rs` 扫描结果按 **`ScanCacheKey = (目录 mtime, 文件数)`** 进程内缓存（`SCAN_CACHE` 静态 Mutex<Option>）——键不变命中则复用缓存结果（不重复读盘），键变化或 `force=true` → 全量重扫并回填。**目录级粗粒度失效**：新增/删除/改名一级编码目录 → mtime 或条目数变化 → 缓存失效；**目录内会话文件的增删改不影响根键**——由前端显式刷新（force=true）兜底（FE-19 联动：挂载一次扫描，展开历史节点不重复 scan，显式刷新/恢复完成时 force）。

`scan_sessions_with_force(force)` 供命令层 force 通道强制重扫；trait `scan()` 无 force 参数（注册表路径恒走 `scan_sessions()`）。L1 测试：缓存命中不重读 / 文件数变化失效 / force 绕过 / 键跟踪 mtime+文件数 / 每 root 隔离 / 命令层 force 透传 / 未知 cliId Validation / 非 claude provider force 回退 trait scan。

### CliHistoryProvider trait + 注册表（provider.rs）

```rust
pub trait CliHistoryProvider: Send + Sync + std::fmt::Debug {
    fn scan(&self) -> Vec<AgentHistorySession>;                 // 无 Err 通道
    fn delete(&self, session_id: &str) -> Result<(), AppError>;
    fn validate_session_id(&self, session_id: &str) -> Result<(), AppError>;
    fn read_title(&self, session_id: &str) -> Result<AgentHistoryTitle, AppError>;
}
```

注册表 = cliId 键静态映射（`REGISTRY`）；`resolve_provider` 是命令层分发唯一入口，未知 cliId → `AppError::Validation("未知 cliId: ...")`。实现均为同步阻塞（含 IO），命令层经 `spawn_blocking` 串行化（硬约束 #3）。

### SEC-05 等价强制（MC-304）

**`validate_session_id` 是 `delete` / `read_title` 的强制前置**（trait 契约写明，未来 provider 的等价校验强制）：`run_delete` / `run_read_title` 先经 provider 校验 sessionId，通过后才执行操作。claude provider 的校验 = UUID 形态（`is_uuid_filename`：36 长度 + 连字符位置 + ascii hex 全检）+ `locate_session_jsonl` 在扫描根一级子目录遍历定位——**定位不信托前端任何路径参数**（前端只传 sessionId，SEC-05 保留）。

**符号链接拒跟随（AQ-3）**：一级子目录、命中 jsonl 文件、同名 `<id>/` 删除目录为符号链接时一律拒绝（定位不命中 / 删除不触碰链接目标）——加防御分支，SEC-05 校验与定位流程语义不变。

**测试 cfg 豁免登记（BE-17/D5）**：`claude/ops.rs` 三条 AQ-3 symlink 测试保留 `#[cfg(windows)]`——测试调用 `std::os::windows::fs::symlink_dir/symlink_file`，该 API 仅 Windows target 编译期存在，无法改 `cfg!(windows)` 运行时分支；且创建 symlink 需 Windows 特权（管理员/开发者模式），无权限时测试内运行时跳过（照 state.rs `validate_symlink_*` 先例）。

### env 覆盖留 provider 内部（MC-305）

`SLTERM_CLAUDE_PROJECTS_DIR` env 覆盖留在 claude provider 内部（`claude/scan.rs` 的 `resolve_projects_root`，每次调用读 env 不缓存）。聚合层不假设任何 provider 的 env 命名——未来 `SLTERM_<CLI>_PROJECTS_DIR` 同款模式由各 provider 自管。**生产不设置此 env，仅测试用途**（E2E fixture 隔离，防止测试触碰真实用户数据）；测试经 `ScanRootGuard` RAII 守卫 set/unset（Drop 恢复原值，依赖 `--test-threads=1` 门禁）。

### claude provider 内部是 claude 合法领地（MC-301/302）

`claude/` 内部保留全部 claude 命名与 claude 知识：`~/.claude/projects/` 扫描根、custom-title/ai-title/summary 标题语义、`agent-*.jsonl` 平铺排除、UUID sessionId 校验。`TitleSource` 五态（customTitle/aiTitle/summary/firstPrompt/none）为 provider 内部类型，DTO `title_source` 为开放字符串（UI 不消费具体值）；产出条目 `cli_id: "claude"` 打标（provider 内部写字面量合法）。

### AgentHistorySession DTO（IPC 契约八键）

serde camelCase 八键：`sessionId`/`cwd`/`title`/`titleSource`/`firstPrompt`/`mtimeMs`/`cwdExists` + **`cliId`**（provider 打标，前端按 cliId 区分来源）。`cwd` 一律从 JSONL 内容解析（目录名只是 cwd 的有损编码，禁止反解码）。

### AgentHistoryTitle DTO（IPC 契约两键，人工验证问题 3）

serde camelCase 两键：`title`（回退链合成结果，全无时 null——前端兜底 CLI 名）/ `titleSource`（开放字符串）。供 `agent_history_read_title` 返回——运行中会话页签/导航树行标题与历史扫描**同源回退链**（custom-title > ai-title > summary > firstPrompt，`ops::read_session_title` 复用 `parse_head`/`parse_tail_title`/`resolve_title`）。**文件未定位 → `Ok(title: None)`**（运行中 jsonl 可能尚未落盘，正常条件非错误——与 delete 的「会话不存在 → Err」语义区分：读是幂等查询，删是有副作用操作）。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | 聚合层：`AgentHistorySession` DTO + `AgentHistoryTitle` DTO + `is_uuid_filename` 工具 + 三条泛化命令 `agent_history_scan`（无参聚合）/ `agent_history_delete(cliId, sessionId)`（validate 前置，`run_delete` provider 可注入测试）/ `agent_history_read_title(cliId, sessionId)`（validate 前置，`run_read_title` provider 可注入测试） |
| `provider.rs` | `CliHistoryProvider` trait（四方法）+ cliId 键静态注册表 + `resolve_provider` 分发入口 |
| `claude/mod.rs` | claude history provider：`ClaudeHistoryProvider` trait 实现 + `TitleSource` 五态 + `ScanRootGuard`（cfg test，env 守卫） |
| `claude/scan.rs` | 扫描根单点 `resolve_projects_root`（env 覆盖留 provider 内部）+ `scan_sessions` 一级子目录扫描（UUID 过滤 + `agent-` 平铺排除 + 单文件降级条目） |
| `claude/jsonl.rs` | transcript JSONL 轻量解析：头部 512KB（cwd/firstPrompt/summary）+ 尾部 64KB（custom-title/ai-title） |
| `claude/ops.rs` | 写/读操作：`validate_session_id`（SEC-05 严格校验）+ `locate_session_jsonl` 遍历定位 + `delete_session`（删 jsonl + 同名目录），符号链接拒跟随（AQ-3）+ **`read_session_title`**（运行中会话标题——回退链与 scan 同源，文件缺失 Ok None） |

## 命令

三条 Tauri 命令均在 `lib.rs` 的 `generate_handler!` 注册（旧命令名 `claude_history_*` 零残留）。

### agent_history_scan

签名：`async fn agent_history_scan(cli_id: String, force: Option<bool>) -> Result<Vec<AgentHistorySession>, AppError>`（BE-19）。遍历全部已注册 provider 串行聚合；claude provider 走缓存（`force=true` 绕过，前端 wrapper `scanAgentHistory(cliId, force?)`）；非 claude provider 无 force 概念回退 trait scan。阻塞 I/O 在 `spawn_blocking` 内（硬约束 #3）。

### agent_history_delete

签名：`async fn agent_history_delete(cli_id: String, session_id: String) -> Result<(), AppError>`。未知 cliId → `Validation`；删除前经该 provider `validate_session_id` 前置（SEC-05 等价强制）。

### agent_history_read_title

签名：`async fn agent_history_read_title(cli_id: String, session_id: String) -> Result<AgentHistoryTitle, AppError>`。未知 cliId → `Validation`；读取前经该 provider `validate_session_id` 前置（SEC-05 等价强制）；会话文件不存在 → `Ok(title: None)`（前端兜底 CLI 名）。消费方 = 前端运行中会话页签/导航树行标题（useXterm SessionStart + agent-event 5s 节流刷新）。

## 测试模式

共 92 用例（原 `claude_history/` 63 条全保留迁移 + 聚合层/provider 新增 + 人工验证问题 3 read_title 新增 + **BE-19 缓存 8 条**）：

| 位置 | 用例数 | 覆盖范围 |
|------|--------|---------|
| `mod.rs` `#[cfg(test)]` | 21 | `AgentHistorySession` serde camelCase 八键（序列化键集合/反序列化/往返）、**`AgentHistoryTitle` serde camelCase 两键 ×2**、`is_uuid_filename`（合法/非法全分支）、聚合 scan（多 provider 桩串行聚合/单 provider 失败不阻塞）、delete 命令层（validate 前置——validate 拒绝时 delete 不被调用 / 未知 cliId Validation / 包装层 spawn_blocking 透传 / IO 降级空数组）、**read_title 命令层（桩分发 + validate 前置拒绝时不读 / 未知 cliId / 包装层真实文件 summary 回退链 / 文件缺失 Ok none）**、**BE-19 命令层 force 通道（`command_scan_unknown_cli_id_returns_validation` / `command_scan_force_true_bypasses_cache` / `run_scan_force_true_on_non_claude_falls_back_to_trait_scan`）** |
| `provider.rs` `#[cfg(test)]` | 2 | resolve_provider（已知 cliId 命中/未知 cliId Validation） |
| `claude/mod.rs` `#[cfg(test)]` | 4 | TitleSource serde camelCase 五变体（序列化/反序列化/as_str 映射）、ScanRootGuard Drop 恢复原 env |
| `claude/scan.rs` `#[cfg(test)]` | 21 | resolve_projects_root（env 覆盖/缺省 home 拼接）、scan_sessions（一级子目录/UUID 过滤/`agent-` 平铺排除/降级条目/空目录）、**BE-19 缓存（`scan_cache_hit_returns_stale_without_reread` / `scan_cache_invalidated_when_file_count_changes` / `scan_force_true_bypasses_cache` / `scan_cache_key_tracks_dir_mtime_and_file_count` / `scan_cache_isolated_per_root`）** |
| `claude/jsonl.rs` `#[cfg(test)]` | 28 | 头部/尾部解析窗口、标题回退链、cwd/firstPrompt 提取、损坏行容错 |
| `claude/ops.rs` `#[cfg(test)]` | 16 | validate_session_id（UUID 形态接受/五类非法全拒）、locate_session_jsonl、delete_session（删文件 + 同名目录）、符号链接拒跟随（AQ-3 三形态：一级子目录/命中文件/同名删除目录，Windows symlink 创建失败时跳过）、**read_session_title（回退链五态/文件缺失 Ok none/非法 id Err/尾部覆盖头部/损坏容错）** |

### 运行

```bash
# 运行全部 agent_history 测试
cargo test --manifest-path src-tauri/Cargo.toml agent_history -- --test-threads=1
```

## 修改注意事项

1. 新增 Tauri 命令后在 `lib.rs` 的 `generate_handler!` 注册。
2. **SEC-05 不可削弱**：delete / read_title 必须先经 `validate_session_id`（trait 契约强制）；claude provider 的校验与定位逻辑（UUID 形态 + 遍历定位，不信托前端路径）勿改语义。
3. **BE-19 缓存键语义勿改**：`(目录 mtime, 文件数)` 是目录级粗粒度键，目录内会话文件增删不失效由前端 force 兜底——改键粒度须同步前端 `scanAgentHistory(cliId, force?)` 调用点（显式刷新/恢复完成时 force）。缓存生效测试依赖 `--test-threads=1`（`SCAN_CACHE` 静态 Mutex）。
3. 新增 provider：在 `provider.rs` 的 `REGISTRY` 注册 cliId 条目；扫描/删除/读标题逻辑保持「无 Err 通道、失败降级」语义（read_title 的文件缺失 = Ok None，非 Err）。
4. 修改 `AgentHistorySession` / `AgentHistoryTitle` DTO 字段后同步 `src/types/agentHistory.ts`（前端接口，serde camelCase 双边对应，硬约束 #4）——键集合测试（八键/两键）防漂移。
5. env 覆盖命名属 provider 内部，勿上提聚合层。
