# Claude Code 历史 Session 信息检索总览

> 检索日期: 2026-08-01
> 目的: 为 slTerminal 新增「查询 Claude Code 历史 session + 通过 `claude --resume` 恢复」功能提供信息依据——覆盖会话存储机制、查询方式、resume 用法与限制、生态实践四个维度。

## 检索范围

- **主题边界**: 含数据格式细节（transcript JSONL 字段级结构）
- **时间范围**: 近期优先（2025-2026），版本演进时间线兼顾历史
- **语言范围**: 英文优先（官方文档/源码/issue 为主）
- **输出深度**: 复杂（8 方向，每方向 ≥8 独立来源）

## 文件索引

| 方向 | 汇总文件 | 子文件数 | 关键发现数 |
|------|---------|--------|----------|
| D1: 会话存储机制 — 存储位置与目录结构 | [D1-storage-location.md](D1-storage-location.md) | 0 | 6 |
| D2: 会话存储机制 — transcript JSONL 数据格式 | [D2-transcript-jsonl-format.md](D2-transcript-jsonl-format.md) | 0 | 16 |
| D3: 历史会话查询 — CLI 原生能力 | [D3-cli-native-query.md](D3-cli-native-query.md) | 0 | 8 |
| D4: 历史会话查询 — 文件系统扫描与第三方查询 | [D4-fs-thirdparty-query.md](D4-fs-thirdparty-query.md) | 0 | 11 |
| D5: `claude --resume` 用法 — 参数与交互行为 | [D5-resume-usage.md](D5-resume-usage.md) | 0 | 11 |
| D6: `claude --resume` 用法 — 边界与限制 | [D6-resume-limits.md](D6-resume-limits.md) | 0 | 14 |
| D7: 生态实践 — 开源 session 管理工具 | [D7-opensource-tools.md](D7-opensource-tools.md) | 0 | 6 |
| D8: 生态实践 — 终端/工作区集成 | [D8-terminal-integrations.md](D8-terminal-integrations.md) | 0 | 11 |

## 统计

- 文件总数: 9（1 README + 8 方向汇总）
- 总行数: 1040
- 关键发现总数: 83
- 信息来源: 官方文档 > 源码仓库 > 技术博客 > 社区讨论

## 跨方向关键发现

1. **存储位置统一为 `~/.claude/projects/<编码路径>/<session-id>.jsonl`**：目录名 = 启动目录绝对路径、非字母数字替换为 `-`（Windows 盘符小写参与编码），文件名即 UUID。编码有损不可逆——反查项目路径不能依赖解码，需自行维护映射（来源: D1, D2, D4, D8）
2. **官方没有原生非交互式"列出会话"命令**：唯一入口是 `claude --resume` 的交互式选择器（无参/`-r`）。社区 feature request（issue #16901 closed）与 PR #34168 均未落地（来源: D3, D5）
3. **生态集成统一范式**：只读扫描 `~/.claude/projects/` 的 JSONL → 提取 sessionId → 输出 `claude --resume <id>` / `claude -r <id>`。所有工具都不修改 Claude 自有文件（来源: D4, D7, D8）
4. **程序化恢复官方支持**：`claude -p --resume <session-id> --output-format json` 可 headless 续问；Agent SDK `listSessions()` 提供非交互列举（含 sessionId/summary/gitBranch/cwd，按 lastModified 降序）（来源: D3, D5）
5. **resume 硬限制**：session ID 查找限定当前项目目录 + git worktrees，跨目录报 "No conversation found"；`--mcp-config`/`--settings` 等标志不随恢复带回，权限模式 `plan`/`bypassPermissions` 永不恢复（来源: D5, D6）
6. **transcript 是未文档化内部格式**：官方明示版本间可变、直接解析可能随时破坏；字段信息来自社区逆向（type 枚举已观测 15 种）。解析实现需容忍格式演进（来源: D1, D2, D8）
7. **JSONL 是 parentUuid 树而非线性日志**：一条 API 消息拆多行（同 message.id），token 核算必须按 id 去重否则高估 ~2.6 倍（来源: D2, D7）
8. **默认 30 天清理窗口**：`cleanupPeriodDays` 默认 30 天、启动时删除过期 transcripts——历史查询的覆盖面上限（来源: D1, D4, D6）
9. **Windows 支持是生态短板**：明确支持 Windows 的工具仅 6 家，多为 Tauri/Go 桌面形态（CCHV 2.0k stars 已验证 Rust+Tauri 2 可行）——对 Windows 原生终端模拟器是空档与机会（来源: D4, D7）
10. **可借鉴三段式架构**：扫描（JSONL/索引）→ 展示（fzf/状态列/预览）→ 恢复（`cd <project> && claude --resume <id>`）。终端 app 相比 shell wrapper 天然免去 eval/目录持久化问题（来源: D8）
