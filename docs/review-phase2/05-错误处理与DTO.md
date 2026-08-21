# Phase 2 Review：05-错误处理与 DTO

## 范围

- **S08**：错误处理体系（FE-02/03/05~10、BE-13/15）
- **S09**：corrupted 契约 + 持久化加固（BE-14/16、FE-11、SEC-11）
- **S10**：DTO 契约修正（FE-12/13/14、BE-18）
- **a40ee09**：与本主题相关的收尾修正（`agent_history_scan` 必填 `cliId`/`force`）

## 1. 闭环判定表

| 编号 | 原问题摘要 | 判定 | 关键证据（file:line） |
|------|-----------|------|----------------------|
| FE-02 | 无统一 AppError 解析器与错误消息提取 | **已闭环** | `src/ipc/appError.ts:16-67` 提供 `parseAppError` / `getErrorMessage` / `APP_ERROR_VARIANTS`（11 变体，含 `configParse`）；`src/lib/index.ts:12-16` re-export；`src/__tests__/app-error.test.ts:14-98` 覆盖全 11 变体解析与非 AppError 兜底。 |
| FE-03 | 启动链多处静默吞错 | **已闭环** | `src/main.tsx:55-62` `loadSettings` 失败与 `corrupted` 均 `console.warn`；`src/App.tsx:50-82` 三 store 加载与 `loadAllProjects` 失败均带模块名 `console.warn`。 |
| FE-05 | 关闭时 `pty.kill` 失败仅 `console.error` | **已闭环** | `src/App.tsx:129-150` 收集 kill 失败至 `killFailures`，结束后统一汇总一条 `console.error`（含失败数）。 |
| FE-06 | `requestUserAttention` 静默吞错 | **已闭环** | `src/App.tsx:234-237` catch 内 `console.warn`。 |
| FE-07 | `loadDirectory` 错误伪装空目录 | **已闭环** | `src/features/explorer/useFileTree.ts:42` 新增按路径 `dirErrors`；`src/features/explorer/ExplorerPanel.tsx:449-489` 根目录加载失败渲染错误占位（消息 + 重试按钮）。 |
| FE-08 | PTY write/resize/kill/openUrl 静默；spawn 失败无提示 | **部分修复** | 关键路径已覆盖：`src/panels/terminal/useXterm.ts:195-203` write 连续失败 ≥3 次 toast；`src/panels/terminal/useXterm.ts:350-356` spawn 失败 toast；非关键路径 `console.error`。但残留 `src/panels/terminal/keyboard.ts:37` 终端粘贴 `readText()` 失败仍 `.catch(() => {})` 静默吞错，与 S08 验证断言「FE-08 相关行零 `.catch(() => {})`」冲突。 |
| FE-09 | 设置保存失败仅 console/空 | **已闭环** | `src/stores/fontSize.ts:80-84`、`src/stores/keybindings.ts:84-88`、`src/stores/sideBar.ts:142-146` 保存失败统一 `toast.show("warning", "设置保存失败，重启后将丢失")` + `getErrorMessage` 日志。 |
| FE-10 | git diff/外部修改重载失败仅 console.warn | **部分修复** | Diff 面板：`src/panels/diff/DiffPanel.tsx:174` `diffStale` 状态 + `src/panels/diff/DiffPanel.tsx:655-677`「内容可能过时」提示条（git diff 失败）。编辑器：`src/panels/editor/useCodeMirror.ts:417-438` 外部修改重载失败 toast。但 Diff 面板右栏外部修改重载失败仍只 `console.warn`（`src/panels/diff/DiffPanel.tsx:483/490`），无提示条/toast，用户可能持续看到过时工作区内容。 |
| BE-13 | `From<std::io::Error>` 丢失路径上下文 | **已闭环** | `src-tauri/src/error.rs:87-93` 新增 `io_error(action, path, e)`；`src-tauri/src/fs/mod.rs:122/131/234` 等调用点注入路径；`src-tauri/src/settings.rs:62/76-87`、`src-tauri/src/projects.rs:23/30-36` 持久化链统一使用 `io_error`。 |
| BE-15 | `Notify`/`IoKind` 异构 + 用户消息技术化；缺 `ConfigParse` | **已闭环** | `src-tauri/src/error.rs:29-31` 新增 `ConfigParse` 变体；注释约定消息业务语义化 + 技术细节进 tracing；`src-tauri/src/error.rs:99-206` 测试覆盖全部 11 变体。 |
| BE-14 | 损坏返回 Null/`"{}"` 无法区分无数据/已损坏 | **已闭环** | `src-tauri/src/app_dir.rs:22-25` 新增 `LoadResult<T> { data, corrupted }`；`src-tauri/src/settings.rs:104-147`、`src-tauri/src/projects.rs:52-88` load 命令均返回 corrupted 标志，且 `.bak` 命中算 `corrupted:true`。 |
| BE-16 | `projects` 直接导入 `settings::app_data_dir` 违反约束 #2 | **已闭环** | 新建 `src-tauri/src/app_dir.rs:72-81` `app_data_dir()`；`src-tauri/src/settings.rs:8`、`src-tauri/src/projects.rs:8` 均 `use crate::app_dir::{...}`。 |
| FE-11 | 前端无法感知持久化损坏 | **已闭环** | `src/ipc/settings.ts:11-18`、`src/ipc/projects.ts:10-15` wrapper 返回 `{ data, corrupted }`；`src/stores/fontSize.ts:52-56`、`src/stores/keybindings.ts:63-67`、`src/stores/sideBar.ts:100-104`、`src/stores/projects.ts:236-240` 均消费 `corrupted` 弹出 toast；`src/main.tsx:59-62` 启动早期仅 `console.warn`（ToastHost 未挂载，符合预期）。 |
| SEC-11 | 保存无 schema/大小校验 | **已闭环** | `src-tauri/src/settings.rs:15/18-30` settings 顶层键白名单 + 1MB 上限；`src-tauri/src/projects.rs:96-114` projects 1MB 上限 + JSON 对象校验；L1 测试覆盖非法键、超上限、非对象输入。 |
| FE-12 | `DirEntry.size/modified` 声明 `?: number` 与运行时 `null` 不符 | **已闭环** | `src/types/fs.ts:12/14` 改为 `number | null`；`src-tauri/src/fs/mod.rs:26/28` Rust 端 `Option<u64>`；grep 无 `entry.size?` / `entry.modified?` 残留。 |
| FE-13 | `FsEventPayload.detail` TS 可选但 Rust 必填 | **已闭环** | `src/types/notify.ts:6` 改为 `detail: string`；`src-tauri/src/notify/mod.rs:59` `detail: String`；grep 无 `detail?` 误用（仅 `CustomEvent.detail?.path`，不相关）。 |
| FE-14 | `HooksLayer` 任意 string；cols/rows 无范围；u64 精度 | **已闭环** | `src/types/hooksConfig.ts:10` `HooksLayer = "user" | "project" | "local"`；`src/ipc/pty.ts:8-27` `assertPtyDim` 前置校验 1..=32767；`src/types/pty.ts:11-16`、`src/types/agentHistory.ts:21`、`src/types/agent.ts:22` 均注释 u64 → number 安全整数约定。 |
| BE-18 | hooks 配置 Rust 端无 DTO | **已闭环** | `src-tauri/src/hooks/claude/config.rs:24-45` `Layer` 枚举 + `parse_layer` 返回枚举；`src-tauri/src/hooks/claude/config.rs:62-93` `HooksSubtree`/`MatcherGroup`/`HookHandler` 形态校验结构体；L1 测试覆盖 Layer serde、子树形态、handler 必填等。 |
| a40ee09 相关 | `agent_history_scan` 无参调用导致历史区恒空 | **已闭环** | `src/ipc/agentHistory.ts:16-21` `scanAgentHistory(cliId, force?)` 与后端 `agent_history_scan` 对应；`src/features/agentHistory/useAgentHistory.ts:55/59` 调 `scanAgentHistory(CLAUDE_CLI_ID, force)`；`src/__tests__/ipc-agent-history-contract.test.ts` 已更新为含 `cliId`/`force` 参数。 |

## 2. 新发现问题

| # | 问题描述 | 位置 | 级别 | 说明 |
|---|---------|------|------|------|
| N1 | 终端粘贴失败静默吞错 | `src/panels/terminal/keyboard.ts:35-38` | P2 | `readText().then(...).catch(() => {})` 完全忽略剪贴板读取失败，与 S08 FE-08「关键路径错误可感知」及 S08 验证断言「零 `.catch(() => {})`」均冲突。应改为 `console.error` 或 toast。 |
| N2 | 关窗处理器 cleanup 静默吞错 | `src/ipc/window.ts:55-58` | P2 | `unlisten.then((fn) => fn()).catch(() => {})` 虽为清理路径，但窗口已销毁等异常被完全隐藏。建议 `.catch((err) => console.warn("取消关窗监听失败:", err))`。 |
| N3 | Diff 面板右栏外部修改重载失败不可感知 | `src/panels/diff/DiffPanel.tsx:478-491` | P2 | 外部修改后右栏重载失败仍只 `console.warn`，未设置 `diffStale` 提示条，也未 toast。用户可能在不知情下继续编辑过时内容。 |
| N4 | Diff 面板保存失败提示未统一错误解析 | `src/panels/diff/DiffPanel.tsx:372-376` | P2 | `toast.show("error", \`保存失败: ${err}\`)` 直接拼接原始错误对象，未使用 `getErrorMessage(err)`，与 FE-02 统一解析契约不一致。属于 P2 中较轻路径，但破坏错误处理一致性。 |
| N5 | 编辑器保存失败提示未统一错误解析 | `src/panels/editor/useCodeMirror.ts:178-181` | P2 | 同 N4：直接使用 `${err}`，未走 `getErrorMessage`。属于 P2 中较轻路径。 |
| N6 | stores 内部 `loadFromDisk` 异常不记录 | `src/stores/fontSize.ts:64`、`src/stores/keybindings.ts:71`、`src/stores/sideBar.ts:122` | P2 | `catch {}` 空块导致 store 自身加载失败时不产生任何日志。当前仅由 `App.tsx` 外层 effect 捕获并 `console.warn`，但 store 作为独立 API 被其他调用方使用时将完全静默。属于 P2 中较轻路径。 |

## 3. 依赖人工验证

- **S09 corrupted toast**：手动损坏 `~/.slterminal/settings.json` 或 `slterminal-projects.json`，验证启动后弹出「配置已损坏，已回退默认值」警告，且默认值兜底正常。
- **FE-08 spawn 失败 toast**：构造 PTY spawn 失败场景（如非法 shell 路径），验证终端内显示重连提示的同时弹出右上角 error toast。
- **FE-14 pty 尺寸校验**：传入 cols/rows 为 0 或 32768，验证 invoke 前直接抛错、不进入后端。

## 结论

- **已闭环**：16 项
- **部分修复**：2 项（FE-08、FE-10）
- **未修复**：0 项
- **决策关闭**：0 项
- **新发现问题**：6 项（P2 ×6）

报告文件：`D:\data\learn\code\slTerminal\docs\review-phase2\05-错误处理与DTO.md`
