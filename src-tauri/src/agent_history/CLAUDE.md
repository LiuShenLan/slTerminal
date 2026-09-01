# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src-tauri/src/agent_history` 聚合各 CLI 的历史会话数据（当前仅 claude），向前端暴露统一的 scan/delete/read_title 命令。扫描缓存策略、SEC-05 校验前置、运行中标题读取语义与 provider 注册表形态，属于跨 CLI 的设计决策，必须文档化。

## 关键约束与决策

### `CliHistoryProvider` trait + cliId 注册表

`provider.rs` 定义四方法 trait：
- `scan() -> Vec<AgentHistorySession>`（无 Err 通道，失败降级为空/部分结果）；
- `delete(session_id)` / `read_title(session_id)`；
- `validate_session_id(session_id)`。

注册表是 cliId 键静态映射；未知 cliId → `Validation`。

### 扫描缓存 + force 通道（BE-19）

claude provider 的扫描结果按 `ScanCacheKey = (目录 mtime, 文件数)` 进程内缓存。键不变时复用缓存；键变化或 `force=true` 时重扫。这是目录级粗粒度失效——目录内会话文件增删改不会使缓存失效，由前端显式 `force` 兜底。

### SEC-05 校验前置

`agent_history_delete` 与 `agent_history_read_title` 必须先经 provider `validate_session_id` 通过。claude provider 的校验 = UUID 形态 + `locate_session_jsonl` 遍历定位；**不信托前端任何路径参数**，前端只传 sessionId。

### 符号链接拒跟随（AQ-3）

claude provider 定位与会话目录删除时：一级子目录、命中 jsonl 文件、同名删除目录为 symlink 时一律不跟随/不触碰。

### env 覆盖留在 provider 内部（MC-305）

`SLTERM_CLAUDE_PROJECTS_DIR` 仅用于测试隔离，生产不设置。命名与解析留在 `claude/scan.rs` 的 `resolve_projects_root` 内部，不上提聚合层。

### DTO 字段

- `AgentHistorySession`：serde camelCase 八键——`sessionId`/`cwd`/`title`/`titleSource`/`firstPrompt`/`mtimeMs`/`cwdExists`/`cliId`；`cwd` 一律从 JSONL 内容解析，不反解码目录名。
- `AgentHistoryTitle`：两键 `title`/`titleSource`；文件未定位 → `Ok(title: None)`，与 delete 的「不存在 → Err」语义区分（读是幂等查询，删是有副作用操作）。

### 标题回退链

`read_session_title` 与 scan 同源回退链：customTitle > aiTitle > summary > firstPrompt。

## 外部坑/红线

- **SEC-05 不可削弱**：delete/read_title 必须先 validate_sessionId；claude provider 用 UUID 形态 + 遍历定位，禁止信任前端路径。
- **不要跟随 symlink**：定位与删除路径均显式拒绝 symlink。
- **env 命名不上提**：未来 CLI 的 projects 目录 env 由各 provider 自管。
- **DTO 字段双边同步**：改后端 `AgentHistorySession` / `AgentHistoryTitle` 须同步 `src/types/agentHistory.ts`。
- **缓存键语义勿改**：`(mtime, file_count)` 是目录级粗粒度键，依赖前端 `force` 兜底。

## 测试模式

- **`ScanRootGuard`**：RAII 注入 `SLTERM_CLAUDE_PROJECTS_DIR` env，Drop 恢复；依赖 `--test-threads=1` 门禁。
- **provider 桩注入**：`mod.rs` 用 `RecordingStub` 验证 delete/read_title 的 validate 前置。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| Windows symlink 创建测试 | 需要管理员/developer mode | 创建失败 skip，逻辑分支由非 symlink 用例覆盖 |
| 真实 claude JSONL 解析 | 依赖真实会话文件 | L4 E2E / 人工实测 |
