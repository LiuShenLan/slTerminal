# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

Claude Code 历史会话查询——扫描 `~/.claude/projects/` 下全部会话 transcript（JSONL）并暴露两命令：`claude_history_scan`（元数据扫描）、`claude_history_delete`（删除会话）。**`claude_history_rename` 已随前端重命名功能整体移除**（用户决策，问题 7 修复——前端不再提供重命名入口，后端命令与测试全链路删除；官方 `/rename` 仍是 custom-title 的唯一写入方）。需求规格见 `docs/claude-history-view/README.md`（v1.2，决策 22–28）。

数据源事实约束（规格 3.1）：存储根 `~/.claude/projects/`，一级目录名 = cwd 的有损编码（**禁止反解码**），会话文件 = `<uuidv4>.jsonl`（文件名主干即 sessionId），子代理 transcript 为 `agent-*.jsonl` 平铺或 `<id>/subagents/` 子目录形态（均须排除）。

## 架构决策

### 扫描根单点 `resolve_projects_root()`（SEC-02/BE-06）

解析顺序：`SLTERM_CLAUDE_PROJECTS_DIR` env 非空 → 用之；否则 `dirs::home_dir()/.claude/projects`。**每次调用时读取 env（不缓存）**——E2E 子进程继承 env 即可生效。

> **生产不设置此 env，仅测试用途**（E2E fixture 隔离——删除/重命名用例只动 `e2e-tests/.tmp-claude-projects/` 副本，任何用例不得触碰用户真实 `~/.claude/projects/`，见 `e2e-tests/run-wdio.cjs` 注入点）。

### 轻量解析：头部 512KB + 尾部 64KB 双窗口（BE-03/BE-04）

性能约束（规格 3.4，文件从几 KB 到 20+MB）——**禁止整文件读取**，每文件只读两个窗口：

- **头部 ≤512KB**（`HEAD_SCAN_LIMIT_BYTES`）：顺序逐行扫描，收集 cwd（首个含非空 cwd 字段的行）、custom-title / ai-title / summary 候选（同类型 last-wins）、首条可见 user prompt（命中即提前结束）。可见 prompt 判定：`type=="user"` 且 `message.content` 为**字符串**（数组 = tool_result 载体跳过）、`isMeta != true`、trim 后非空且不以 `<` 开头（跳过 `<command-name>` 等占位符）；截断至 200 字符（`PROMPT_MAX_CHARS`）。单行 JSON 解析失败（EOF 截断/损坏）即停止，不报错。
- **尾部 ≤64KB**（`TAIL_SCAN_BYTES`，照 `hooks/usage.rs` 的 `TRANSCRIPT_TAIL_BYTES` 同款常量与「中途起始跳首行」策略）：逆行扫描，遇 `custom-title` 立即返回（类型恒优先），全程无则返回最后一条 `ai-title`（覆写式 last wins）。

### 标题回退链（决策 22）

合成顺序：**custom-title > ai-title > summary > 首条 prompt**；四路皆空 → `title=null` / `titleSource="none"`。尾部扫描结果（物理最新）优先于头部候选（`resolve_title`）。重命名写 custom-title 是对齐官方 `/rename`（本机真实数据证实官方写 custom-title，推翻早期 ai-title 决策）。

### SEC-01：sessionId 校验 + 定位不信托前端

`claude_history_delete` 入参 `session_id` 严格校验 UUID 形态（复用 `is_uuid_filename`：36 长度 + 连字符位置 + ascii hex 全检，天然拒绝含 `..`、路径分隔符、空串的一切输入）→ `AppError::Validation`。文件定位 = 遍历扫描根一级子目录找 `<session_id>.jsonl`（`locate_session_jsonl`），**前端不传任何路径**。命令写用户 home 目录文件、绕过 project_root 沙箱（照 `hooks/config.rs` user 层先例），入参即攻击面。

### 容错与降级契约（BE-02）

- 单文件解析任何失败 → **降级条目**（仅 sessionId + mtimeMs，其余 None / titleSource=none / cwdExists=false），不阻塞整体、不返回 Err
- 扫描根本身不存在 → 返回空数组（非 Err，新机无 claude 数据属正常）
- 排除规则：`agent-*.jsonl` 平铺形态、文件名主干非 UUID、非 jsonl 扩展名；不递归子目录（`<id>/subagents/` 天然不命中）
- 时间口径（决策 26）：`mtime_ms` = `metadata().modified()` 转 Unix 毫秒；metadata 失败 → 0。`cwd_exists` = cwd 非 null 且目录存在

### 写操作（BE-07，ops.rs）

- **delete**：删 `<id>.jsonl` + 同名 `<id>/` 目录存在则 `remove_dir_all`（含 subagents 附属数据）；jsonl 不存在 → `AppError::Validation`（「会话不存在」语义）

### 命令与并发约束

两命令均在 `lib.rs` 的 `generate_handler!` 注册（`claude_history::scan::claude_history_scan` / `claude_history::ops::claude_history_delete`）；阻塞 I/O 全部 `spawn_blocking`（硬约束 #3）；DTO 双边对应 `HistorySession` / `TitleSource`（serde camelCase，硬约束 #4）。扫描顺序无契约（不排序——排序/分组/过滤是前端 `src/features/claudeHistory/historyModel.ts` 的职责）。

## 文件

| 文件 | 职责 |
|------|------|
| `mod.rs` | 模块入口：DTO 定义（`HistorySession` 七字段 + `TitleSource` 五变体，serde camelCase）+ `is_uuid_filename()` 纯函数（scan 排除 + ops 校验复用） |
| `scan.rs` | 扫描根单点 `resolve_projects_root()`（SEC-02/BE-06）+ `claude_history_scan` 命令（BE-02）+ 遍历/降级/mtime/cwdExists（BE-05） |
| `jsonl.rs` | JSONL 轻量解析纯函数：`parse_head()`（BE-03，头部 512KB）、`parse_tail_title()`（BE-04，尾部 64KB 逆行）、`resolve_title()`（回退链合成，决策 22）。常量 `HEAD_SCAN_LIMIT_BYTES` / `TAIL_SCAN_BYTES` |
| `ops.rs` | 写操作：`validate_session_id()`（SEC-01）+ `claude_history_delete`（BE-07） |

## 测试模式

Rust 测试 4 个位置（均为 `#[cfg(test)] mod tests` 嵌入源文件底部），共 56 用例（grep `#[test]` 口径，与 `.claude/test-inventory.md` 一致）：

| 位置 | 用例数 | 覆盖范围 |
|------|--------|---------|
| `jsonl.rs` | 28 | parse_head（cwd 收集/首条可见 prompt 跳过 4 类/未知 type/EOF 截断/200 字符截断/头部标题 last-wins）、大文件头尾窗口协同（>512KB 中部标题收不到/尾部 64KB 命中）、parse_tail_title（custom 恒优先/ai 兜底/截断行/空文件）、resolve_title 回退链 5 态 + tail 优先 |
| `scan.rs` | 14 | resolve_projects_root（env 覆盖/空 env 回退/默认）、排除 3 类（agent-*/非 UUID/subagents）、多目录多会话收集、扫描根缺失空数组、损坏/空文件降级条目、完整字段（回退链落位 summary）、cwd_exists（真/假）、env 端到端、mtime（存在/缺失）、尾部 custom-title 覆盖头部 summary |
| `ops.rs` | 7 | validate_session_id（UUID 双形态接受/5 类非法拒绝）、delete（jsonl+同名目录范围/仅 jsonl/不存在 Err/非法 id 端到端）、越界防护（扫描根外哨兵文件不被触碰） |
| `mod.rs` | 7 | HistorySession serde camelCase 七键集合精确匹配/反序列化/roundtrip、TitleSource 五变体序列化+反序列化、is_uuid_filename（合法/非法/agent 形态） |

### 测试模式要点

- **env 测试必须 `--test-threads=1`**：`std::env::set_var("SLTERM_CLAUDE_PROJECTS_DIR", ...)` 全局可变，并行测试互相污染——测试内设/测毕恢复（`set_scan_root`/`unset_scan_root` helper），依赖 L1 的 `--test-threads=1` 门禁
- **tempdir 隔离**：`make_scan_root()` 创建扫描根 + 编码目录（`C--Users-test-app`），路径经 `dunce::canonicalize` 统一长名（8.3 短名坑，照 `git/CLAUDE.md` 先例）
- **JSON 构造用 serde_json**：测试写入的 transcript 行经 `serde_json::json!` 序列化（Windows 路径含反斜杠，手拼字符串转义易错）
- **纯逻辑函数直测**：命令包装（async + spawn_blocking）不直测，直接调纯 I/O 逻辑（`scan_sessions` / `delete_session` / `parse_head` / `parse_tail_title` / `resolve_title`）；命令注册与端到端由 L4 E2E 验收

### 运行

```bash
# 运行全部 claude_history 测试
cargo test --manifest-path src-tauri/Cargo.toml claude_history -- --test-threads=1

# 运行单个文件测试
cargo test --manifest-path src-tauri/Cargo.toml claude_history::jsonl -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml claude_history::scan -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml claude_history::ops -- --test-threads=1
```

## 修改注意事项

1. 修改 DTO 字段（`HistorySession` / `TitleSource`）后同步更新 `src/types/claudeHistory.ts`（前端接口）与 `src/ipc/claudeHistory.ts`（IPC wrapper），跑 mod.rs serde 测试 + `src/__tests__/ipc-claude-history-contract.test.ts`
2. 修改 `resolve_projects_root` / 排除规则 / 降级逻辑后跑 `scan.rs` 全部 14 条测试
3. 修改 `parse_head` / `parse_tail_title` / `resolve_title` / 两个窗口常量后跑 `jsonl.rs` 全部 28 条测试（尤其大文件头尾协同两用例）
4. 修改 `validate_session_id` / delete 后跑 `ops.rs` 全部 7 条测试；**勿削弱 SEC-01 校验**（定位只接受 sessionId，前端不传路径）
5. env 测试依赖 `--test-threads=1` 门禁——勿在测试中引入并行 env 操作
6. 新增 Tauri 命令后在 `lib.rs` 的 `generate_handler!` 注册（本模块两命令已注册）
