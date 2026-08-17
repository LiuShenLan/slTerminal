# slTerminal review 修复清单（checklist）

- **来源**：`docs/review/00-汇总.md`（10 维度 130 条原始发现，去重后 105 项）+ 计划期 grill 决策（2026-08-18）
- **组织方式**：按模块前缀编号（SEC 安全 / BE 后端 / FE 前端 / TE 工具链依赖 / DOC 文档）；**不用 P0-P4 组织优先级——优先级由 stages.md 的 Stage 依赖顺序表达**，每项保留 review 原始级别标注
- **合并关系**：每项「来源」列保留汇总编号与维度报告#编号留痕
- **修复范围**：全部修复（用户确认）；少数项经决策以「登记豁免」方式关闭（理由随条目写明）

## 0. 计划期决策记录（grill 结果，全清单适用）

| # | 决策点 | 结论 | 影响条目 |
|---|--------|------|----------|
| D1 | Workspace 多 Dockview 实例（P0-4）vs H6 架构 | **保持多实例 + 页面总数上限 + 文档豁免登记**；`pty_reattach` 前后端删除（死代码与攻击面一并消除） | FE-01、SEC-03 |
| D2 | 依赖 major 升级范围 | **全部纳入本轮**（typescript 7 / dockview-react 8 / json-schema-library 11 / jsdom 30 / jest-dom 7 / @types/node 26 / cross-env 10） | TE-07~TE-10 |
| D3 | fs_read_file 大文件 | **Channel 分块推送**，前端拼接；保持 10MB 上限；CodeMirror 不做部分加载（其文档模型不支持） | BE-03 |
| D4 | CSP `unsafe-inline` | **保留 + 文档登记**（srcdoc iframe 继承父 CSP 是 W3C 行为，HTML 预览注入脚本必须内联） | SEC-09 |
| D5 | 约束 #9 测试 cfg(windows) | **改代码为主**：6 处测试 cfg 改运行时 `cfg!(windows)` 分支；约束措辞同步明确 | BE-17、DOC-02 |
| D6 | 剪贴板读权限 | **保留权限 + 登记消费点 + 守卫测试**（唯一消费点 keyboard.ts Ctrl+Shift+V 显式手势；改后端命令不缩小攻击面——前端上下文被注入时同样能 invoke） | SEC-06 |
| D7 | setProjectRoot 失败契约 | **保持降级切换**（DBG-9 契约不动），失败补 **toast 告警**可感知 | FE-04 |
| D8 | watcher 排除范围 | **仅 watcher 硬编码排除**（node_modules/target/.venv/dist 等）；fs_read_dir 不动（懒加载既定决策） | BE-02 |
| D9 | hooks user 层写入 | **后端语义校验 + user 层 confirmDialog 二次确认** | SEC-05 |
| D10 | CI 门禁 | **npm audit + knip + cargo audit 三项全纳入** | TE-13 |
| D11 | 持久化损坏 | **返回结构加 `corrupted` 标志**（`{ data, corrupted }`，DTO 双边变更） | BE-14、FE-11 |

## 1. SEC 安全（14 项）

| ID | 级别 | 位置 | 问题 | 修复要点 | 来源 |
|----|------|------|------|----------|------|
| SEC-01 | P0 | `src-tauri/src/pty/shell.rs:29-58`、`spawn.rs:979` | Shell 白名单仅比对文件名，传 `C:\project\cmd.exe` 或篡改 PATH 可绕过 → RCE（已核实：仅 `file_name` 比对） | 用户传入 shell 含路径分隔符时：canonicalize 后必须与 `which_full_path(文件名)` 解析结果一致才放行（即只信任 PATH 解析出的真实路径）；纯文件名输入维持现状。补 L1 测试（伪造路径拒绝/合法绝对路径放行） | 汇总 P0-2 / 07#1 |
| SEC-02 | P0 | `src-tauri/src/hooks/signal.rs:82-127`、`watcher.rs:155-174` | 信号目录 `.json` 未过滤符号链接，`fs::metadata`/`read_to_string` 跟随 symlink → 越界读取经 agent-event 泄露（已核实） | `process_signal_file_with` 与 `collect_signal_files` 改用 `fs::symlink_metadata` + `is_symlink()` 检查，symlink 文件仅删除不读取；补 L1 测试（Windows symlink 需管理员/developer mode——测试内创建失败则 skip 并注明） | 汇总 P0-3 / 07#2 |
| SEC-03 | P1 | `spawn.rs:1309-1367`、`lib.rs:86`、`src/ipc/pty.ts:74-82` | `pty_reattach` 无 panel_id 归属校验；且无生产调用方 | **按 D1 删除**：后端命令 + `generate_handler!` 注册 + 前端 wrapper + 关联测试（`ipc-contract.test.ts` reattach 用例、`pty_integration_tests.rs` reattach 用例）；ring buffer/channel 替换机制保留（reader 内部仍用） | 汇总 P1-21 / 07#3、01#9、01#11 |
| SEC-04 | P1 | `src/panels/html/HtmlPanel.tsx:96-125` | iframe 内任意脚本可伪造 `slterm_key` postMessage 触发全局快捷键 | 引入面板生命周期绑定的随机 nonce：面板挂载时生成 nonce 经注入脚本传入 iframe，父窗口校验消息 nonce；nonce 不符静默丢弃。补 L2 测试 | 汇总 P1-22 / 07#4 |
| SEC-05 | P1 | `src-tauri/src/hooks/claude/config.rs:174-195`、hooksConfig 面板 | `agent_hooks_config_write` 仅 JSON 对象类型校验，可写入任意 command hooks | 后端语义校验：事件名白名单（HOOK_EVENTS 10 事件）、handler `type` 白名单（`"command"`）、`command` 非空字符串审查；**user 层写入时前端 confirmDialog 二次确认**（D9）。补 L1/L2 测试 | 汇总 P1-23 / 07#5 |
| SEC-06 | P1 | `src-tauri/capabilities/default.json:15-16` | `clipboard-manager:allow-read-text` 过宽 | **按 D6 保留**：`src/ipc/CLAUDE.md` 登记唯一消费点（`src/panels/terminal/keyboard.ts` 的 terminal.paste 命令，Ctrl+Shift+V 显式手势——已核实路径）；补契约守卫测试（grep 级：readText 仅出现于 `src/ipc/clipboard.ts`、`src/panels/terminal/keyboard.ts` 与测试文件） | 汇总 P1-24 / 07#6 |
| SEC-07 | P1 | `src-tauri/build.rs`、`capabilities/default.json`、`lib.rs` | 33 条自定义命令默认对所有窗口放行 | `build.rs` 配置 `tauri_build::Attributes::new().app_manifest(AppManifest::new().commands(&[...]))`（一手证据：tauri-build 2.6.3 `acl.rs:100` 存在该 API），为每条命令生成 `allow-<cmd>` 权限；`capabilities/default.json` 逐条 allow（SEC-03 删除后为 32 条）。删 `_p0-07-note` 旧注释 | 汇总 P1-25 / 07#7 |
| SEC-08 | P1 | `src-tauri/src/notify/mod.rs:103-109` | watcher 事件路径未过滤 symlink，项目内 symlink 可致外部路径经 fs-event 泄露 | 事件循环中对事件路径做 symlink 检查（`symlink_metadata`），命中 symlink 的路径不 emit；`need_rescan` 分支不受影响（只发 watch root）。与 BE-02 同 Stage | 汇总 P1-26 / 07#8 |
| SEC-09 | P2 | `src-tauri/tauri.conf.json:25-26` | CSP 含 `script-src 'unsafe-inline'` | **按 D4 保留 + 登记**：tauri.conf.json 注释 + `.claude/adr.md` 补登记——srcdoc iframe 继承父 CSP（W3C），HTML 预览注入脚本（锚点拦截/键盘转发/nonce）必须内联，移除即破坏预览 | 07#9 |
| SEC-10 | P2 | `src/main.tsx:28-31` | fail-safe 页用 `innerHTML` 拼接（msg 为变量） | 改为 `createElement` + `textContent` + `style` 赋值 | 07#10 |
| SEC-11 | P2 | `src-tauri/src/settings.rs:79-115`、`projects.rs:65-81` | 保存仅浅合并，无 schema/大小校验 | 增加大小上限（如 1MB）+ 顶层键白名单/schema 校验（settings 顶层键 = fontSize/keybindings/sideBar/colorScheme 等已知集合；projects 结构校验）；与 BE-14 同 Stage | 07#11 |
| SEC-12 | P2 | `src-tauri/src/hooks/claude/slterm-statusline.js` | 桥接脚本透传执行 `~/.claude/settings.json` 原 statusline 命令，被篡改则命令注入 | 注入/重注入时对原命令做审查：仅记录不阻断（命令来自用户自身配置，审查 = 检测可疑模式如 curl/wget/Invoke-Expression 时 tracing::warn! 告警）；文档登记该信任边界 | 07#12 |
| SEC-13 | P2 | `src-tauri/src/hooks/claude/inject.rs` | 脚本版本检测依赖首行文本，磁盘脚本可被替换为首行匹配的恶意文件 | 状态检测增加内容哈希比对：编译期计算模板 SHA-256，状态检测时对磁盘脚本做哈希比对，不一致 → Outdated | 07#13 |
| SEC-14 | P2 | `src-tauri/src/state.rs:186-197` | `set_project_root` 缺目录存在性/可访问二次校验，失败时旧 root 残留 | 随 BE-04 异步化一并：canonicalize 失败/目录不可读 → 返回 Err 且**清空旧 root**（防沙箱误放行旧路径）；补 L1 测试 | 07#14 |

## 2. BE 后端（21 项）

| ID | 级别 | 位置 | 问题 | 修复要点 | 来源 |
|----|------|------|------|----------|------|
| BE-01 | P0 | `src-tauri/src/pty/spawn.rs:958` | `pty_spawn` 无会话总数上限，可耗尽 ConPTY/进程句柄 | `MAX_PTY_SESSIONS` 硬上限（建议 32），spawn 前检查 `sessions.len()`，超限返回 `AppError::Validation`；补 L1 测试 | 汇总 P0-9 / 09#2、10#1 |
| BE-02 | P0 | `src-tauri/src/notify/mod.rs:103-110` | watcher 递归监听无排除，大仓库事件风暴 | **按 D8**：事件循环中过滤路径分量含 `node_modules`/`target`/`.venv`/`venv`/`dist`/`.git`/`__pycache__` 的事件（watcher 仍注册全树——notify 不支持目录级排除，过滤在事件侧）；fs_read_dir 不动。补 L1 测试 | 汇总 P0-5 / 08 P0-2、09#1、10#2 |
| BE-03 | P0 | `src-tauri/src/fs/mod.rs:64` | `fs_read_file` 一次性 `read_to_string`，大文件内存/IPC 峰值高 | **按 D3**：改 Channel 分块推送（块 256KB；先读元信息校验大小≤10MB 再分块发送）；前端 `ipc/fs.ts` 拼接为完整字符串后交编辑器。契约：`fs_read_file(path, onChunk: Channel<FsReadChunk>)`，`FsReadChunk = { data: string, done: boolean }`。补 L1/L2 测试 | 汇总 P0-6 / 08 P0-3 |
| BE-04 | P0 | `src-tauri/src/state.rs:186`、`notify/mod.rs:287` | `set_project_root`/`notify_watch` 同步命令主线程阻塞 I/O | 两条命令改 `async fn` + `spawn_blocking` 包裹阻塞段（canonicalize / FileWatcher::start）；前端 invoke 签名不变（Promise 语义已具备）。补 L1 测试 | 汇总 P0-7 / 03#3、09#7、10#9 |
| BE-05 | P0 | `src-tauri/src/pty/reader.rs:83-131` | reader 每次 read 成功即 Channel send，IPC 次数与字节数成正比 | reader_loop 引入微批处理：read 后非阻塞 try_read 续读至 64KB 或无可读数据再 send（避免引入固定延迟——用「读到即续读」而非定时器）；ring buffer 同步批量 append（联动 BE-12）。**DOC-01 豁免项 1（reader_loop I/O 编排）变动——同步更新豁免表与注释**。**人工验证点：claude 高输出流畅度实测** | 汇总 P0-8 / 08 P0-5 |
| BE-06 | P1 | `src-tauri/src/pty/spawn.rs:1284-1299` | `pty_kill` 丢弃 kill 结果、`join()` 无超时（已核实 `let _ = child.kill()`） | 检查 kill 返回值，失败 `tracing::warn!` 并继续；`join` 改「带超时的轮询 `is_finished`」（如 3s 超时后放弃 join 记 warn，线程随 Drop 兜底）。补 L1 测试（可测部分抽纯函数） | 汇总 P1-37 / 09#4、10#3 |
| BE-07 | P1 | `notify/mod.rs:49`、`hooks/signal.rs:75` | fs-event/agent-event 无背压/限速 | 后端 emit 侧：fs-event 已有 300ms debounce——补**事件合并上限**（单批 paths 超阈值时合并为 Rescan）；agent-event 为低频控制事件不节流（误伤状态机）——前端 `onAgentEvent` 消费侧不加节流，改为文档登记评估结论。补 L1 测试 | 汇总 P1-38 / 09#5 |
| BE-08 | P1 | `src/App.tsx:110-127`、后端 | 关闭序列仅遍历前端 TerminalRegistry，前后端不一致时后端 session 泄漏 | 新增后端命令 `pty_kill_all`（遍历 sessions 全部 kill+join 超时语义同 BE-06；`lib.rs` 注册 + SEC-07 白名单同步）；关闭序列：先前端 Registry 快速 kill，再 `pty_kill_all` 兜底。补 L1/L2 测试 | 汇总 P1-39 / 09#6、10#5 |
| BE-09 | P1 | `src-tauri/src/state.rs:78`、`git/mod.rs` | `git_repo_cache` 无上限无淘汰（已核实无任何清理点，注释「目录切换时清除」失实） | 改容量上限 LRU（容量 8，手实现简易 LRU 或 lru crate——优先零新依赖手实现）；修正失实注释。补 L1 测试 | 汇总 P1-42 / 10#4 |
| BE-10 | P1 | `src/features/explorer/ExplorerPanel.tsx:183-189` | 只 startWatch 不 stopWatch，旧 watcher 占用至 LRU 淘汰 | 后端新增 `notify_stop_watch` 命令（pool.remove + stop；`lib.rs` 注册 + 白名单同步）；前端 `ipc/notify.ts` 加 wrapper，项目移除/切换时调用。补 L1/L2 测试 | 汇总 P1-43 / 10#8 |
| BE-11 | P1 | `src-tauri/src/notify/pool.rs` | `LruWatcherPool` 容量硬编码 5；暂停仍占 OS 句柄 | 容量改常量 `WATCHER_POOL_CAPACITY = 8` 并注释理由；暂停超时的 watcher 不额外处理（pause/resume 既定机制保留，文档登记）。补 L1 测试 | 汇总 P1-32 / 08 P1-6、10#15 |
| BE-12 | P1 | `src-tauri/src/state.rs:204` | `ring_buffer_append` 每次 append 取 Mutex 写锁（review 误写 RwLock，已核实为 Mutex），高吞吐时竞争 | 随 BE-05 批量 append 自然降频（合并后 append 次数≈send 次数）；不引入无锁结构（复杂度不值）。验收 = BE-05 落地后 append 调用点仅批量一处 | 汇总 P1-33 / 08 P1-7 |
| BE-13 | P1 | `src-tauri/src/error.rs:39-45` | `From<std::io::Error>` 丢失路径上下文 | fs/settings/projects 等涉及文件路径的命令内核改 `map_err` 在调用点注入路径（不改动 From 本身）；错误消息含路径。补 L1 测试 | 汇总 P1-15 / 05#15 |
| BE-14 | P1 | `src-tauri/src/settings.rs:119-148`、`projects.rs:37-54` | 损坏返回 Null/`"{}"`，无法区分无数据/已损坏 | **按 D11**：返回结构改 `{ data: Value, corrupted: bool }`（DTO 双边变更，前端 FE-11 联动）；`.bak` 兜底逻辑保留，bak 命中也算 corrupted=true（数据来自备份）。补 L1 测试 | 汇总 P1-16 / 05#7/#16、09#10 |
| BE-15 | P2 | `src-tauri/src/error.rs:7-37`、多处 | `Notify`/`IoKind` 变体承载异构错误；用户可见消息偏技术 | 新增 `ConfigParse` 变体（配置 JSON 损坏场景）；用户可见消息改业务语义（「保存设置失败」），技术细节进 tracing 日志。前端 parseAppError（FE-02）同步登记新变体。补 L1 测试 | 05#18、05#19 |
| BE-16 | P1 | `src-tauri/src/projects.rs:5` | `projects` 直接导入 `settings::app_data_dir`，违反约束 #2 | `app_data_dir`/`resolve_app_data_dir` 上提到新顶层模块 `src-tauri/src/app_dir.rs`，settings/projects 均从该模块导入；`src-tauri/src/CLAUDE.md` 登记新模块 | 汇总 P1-2 / 03#2 |
| BE-17 | P1 | `lib.rs:137`、`fs/mod.rs:586`、`settings.rs:438`、`agent_history/claude/ops.rs:429,448,477` | 测试代码 `#[cfg(windows)]` 出现在 pty 模块外 | **按 D5**：改运行时 `cfg!(windows)` 分支（无法运行时区分且必须 Windows 才能建的——如 symlink 特权测试——保留 cfg 并在模块 CLAUDE.md 登记豁免，DOC-02 同步） | 汇总 P1-1 / 03#1、02#2 |
| BE-18 | P2 | `src-tauri/src/hooks/claude/config.rs`、`src/types/hooksConfig.ts` | hooks 配置 Rust 端 `serde_json::Value` 无 DTO | 后端补 layer 枚举（`Layer::User/Project/Local`，`parse_layer` 改返回枚举）+ hooks 子树结构体（serde 反序列化校验形态）；与 SEC-05 语义校验共用结构。补 L1 测试 | 04#3 |
| BE-19 | P2 | `src-tauri/src/agent_history/claude/scan.rs` | 历史扫描逐文件读取，无索引/缓存 | 扫描结果按 `(目录 mtime, 文件数)` 做进程内缓存（命中则复用）；`agent_history_scan` 加 `force` 参数供显式刷新。前端 FE-19 联动。补 L1 测试 | 08 P2-6 |
| BE-20 | P2 | `src-tauri/src/hooks/signal.rs:9` | 模块级 `#![allow(dead_code)]` 已过时（API 已被 watcher.rs 消费，已核实） | 移除该属性，clippy 验证零 dead_code 警告 | 01#12 |
| BE-21 | P2 | `src-tauri/src/fs/mod.rs:140` | `fs_read_dir` 返回整个目录列表无分页 | **登记豁免**：懒加载按目录分层 + FileTree 虚拟化（FE-30）覆盖渲染侧，单层万级文件罕见；改分页=IPC 契约破坏性变更，收益不抵成本。写入 fs/CLAUDE.md 决策记录 | 08 P2-3 |

## 3. FE 前端（35 项）

| ID | 级别 | 位置 | 问题 | 修复要点 | 来源 |
|----|------|------|------|----------|------|
| FE-01 | P0 | `src/workspace/Workspace.tsx:250-265`、`stores/projects.ts` | 每个 page 保留 Dockview 实例，内存/DOM 线性增长 | **按 D1**：保持多实例（H6/xterm 限制）；`useProjects` 加页面总数上限 `MAX_PAGES`（建议 20，超限 addPage 拒绝 + toast）；workspace/CLAUDE.md + ADR 登记豁免。补 L2 测试 | 汇总 P0-4 / 08 P0-1 |
| FE-02 | P1 | `src/ipc/`、`src/lib/` | 无统一 AppError 解析器与错误消息提取 | 新建 `src/ipc/appError.ts`：`parseAppError(err): { variant, message } | null`（按 camelCase 变体名解析）+ `getErrorMessage(err): string`；lib re-export。补 L2 测试（全 10+1 变体） | 汇总 P1-14 / 04#5、05#14、05#20 |
| FE-03 | P1 | `src/main.tsx:38`、`src/App.tsx:44-69` | 启动链多处静默吞错（loadSettings/字体/快捷键/侧栏/项目数据） | 各 catch 至少 `console.warn` 带模块名；项目数据损坏经 FE-11 corrupted 通道 toast；其余降级保持（默认值兜底合理）。补 L2 测试 | 汇总 P1-5/P1-6 / 05#1、05#2 |
| FE-04 | P1 | `src/App.tsx:84-87`、`stores/projects.ts:154`、`Workspace.tsx:216` | setProjectRoot 失败仅 console.error | **按 D7**：三处调用点失败时 toast 告警（「项目根路径设置失败，文件操作可能被拒绝」），**仍完成切换**（DBG-9 契约不动）；workspace-switch-order 14 用例补 toast 断言 | 汇总 P0-7 联动 / 05#3、05#6、05#11 |
| FE-05 | P1 | `src/App.tsx:116` | 关闭时 pty.kill 失败仅 console.error | 保持 console.error + 计数失败数，关闭序列结束有失败时 tracing 级日志（前端无后端日志通道——记 console.error 已够；Job Object 兜底已存在）。改为：kill 失败收集 → 全部完成后统一 `console.error` 汇总一条。补 L2 测试 | 汇总 P1-7 / 05#4 |
| FE-06 | P1 | `src/App.tsx:198` | `requestUserAttention(null).catch(() => {})` 静默 | catch 内 `console.warn`（非关键路径，不 toast） | 汇总 P1-8 / 05#5 |
| FE-07 | P1 | `src/features/explorer/useFileTree.ts:46-58` | loadDirectory catch 返回空数组，错误伪装空目录 | store 增加 `error` 状态（按路径记录）；ExplorerPanel 渲染错误占位（错误消息 + 重试按钮）。补 L2 测试 | 汇总 P1-9 / 05#8 |
| FE-08 | P1 | `src/panels/terminal/useXterm.ts:188,272,309-329,374,445,498,547` | PTY write/resize/kill/openUrl 多处 `.catch(() => {})` 或仅 console.error；spawn 失败仅终端内写提示 | 非关键路径（resize/kill/openUrl）保留 console.error；关键路径（spawn 失败、write 连续失败≥3 次）toast。统一经 FE-02 getErrorMessage。补 L2 测试 | 汇总 P1-10/P1-11 / 05#9、05#10 |
| FE-09 | P1 | `stores/fontSize.ts:75`、`keybindings.ts:79`、`sideBar.ts:137` | 设置保存失败仅 console/空 | 三 store 保存失败统一 toast 告警（「设置保存失败，重启后将丢失」）。补 L2 测试 | 汇总 P1-12 / 05#12 |
| FE-10 | P1 | `src/panels/editor/useCodeMirror.ts:197,414,427`、`src/panels/diff/DiffPanel.tsx:470,477` | git diff/外部修改重载失败仅 console.warn，用户可能看到过时内容 | 失败时面板内提示条（diff 面板「内容可能过时」）；编辑器重载失败保留 console.warn + 状态条提示。补 L2 测试 | 汇总 P1-13 / 05#13 |
| FE-11 | P1 | `src/stores/*`、`src/main.tsx` | 前端无法感知持久化损坏 | **按 D11**：`ipc/settings.ts`/`ipc/projects.ts` wrapper 适配 `{ data, corrupted }`；各 store loadFromDisk 消费 corrupted → toast（「配置已损坏，已回退默认值」）。补 L2 测试 | 汇总 P1-16 联动 |
| FE-12 | P1 | `src/types/fs.ts:12-14` | `DirEntry.size/modified` 声明 `?: number`，运行时实际为 `null` | 改 `size: number | null; modified: number | null`；grep 全部消费方（含 explorer 排序/显示、测试工厂）适配。补 L2 测试 | 汇总 P1-4 / 04#1 |
| FE-13 | P2 | `src/types/notify.ts:5` | `FsEventPayload.detail` Rust 必填 TS 可选 | 改 `detail: string`；消费方适配 | 04#2 |
| FE-14 | P2 | `src/types/hooksConfig.ts:9`、`pty.ts:9-16`、`fs.ts:12`、`agentHistory.ts:21`、`agent.ts:21` | HooksLayer 任意 string；cols/rows 无范围；u64→number 精度风险 | HooksLayer 收窄 `"user" \| "project" \| "local"`（当前仅 claude 三层，未来 CLI 加层再泛化——types/CLAUDE.md 登记）；`ipc/pty.ts` spawn wrapper 加 cols/rows 1..32767 前置校验；u64 字段在 types 文件注释标明安全整数范围约定。补 L2 测试 | 04#7、04#6、04#4 |
| FE-15 | P1 | `src/features/explorer/useFileTree.ts` | `file-saved` 无 debounce，refreshExpanded 全量重建 | `file-saved` 事件 300ms debounce；已知路径变更只刷新受影响子树（按变更路径定位最近展开祖先刷新）。补 L2 测试 | 汇总 P1-27 / 08 P1-1 |
| FE-16 | P1 | `src/features/navTree/useNavTree.ts` | 每次状态变化全量重建导航树，O(N×M) 前缀匹配 | 历史归属建索引 `Map<projectId, sessions>`；`tree` 派生用 useMemo 依赖精确化 + 稳定引用缓存。补 L2 测试 | 汇总 P1-28 / 08 P1-2 |
| FE-17 | P1 | `src/panels/terminal/TerminalPanel.tsx:111-129` | 订阅整个 TerminalRegistry 事件，无关会话变化触发重渲染 | 订阅回调内按 `e.panelId === 自身 panelId` 过滤后再 setState。补 L2 测试 | 汇总 P1-29 / 08 P1-3、09#12 |
| FE-18 | P1 | `src/panels/terminal/usePtyOutput.ts:83-84`、`useXterm.ts:485-503` | idle/max 定时器卸载未清理；cleanup 未调 `cancelPendingFlush()`；64B 阈值偏小 | `usePtyOutput` 暴露 `dispose()`（清双定时器 + 清 buffer）；`useXterm` cleanup 调用；阈值随 BE-05 后端批处理上调（64B→256B 直接写、其余 2ms 空闲/16ms 强制不变——后端已合并小写）。补 L2 测试 | 汇总 P1-40/P1-30 / 09#8、10#6/#7、08 P1-4 |
| FE-19 | P1 | `src/features/navTree/useNavTree.ts`、`agentHistory` | 挂载即扫描 + 展开再次扫描，反复读磁盘 | 配合 BE-19 缓存：`agent_history_scan` 默认走缓存，展开历史节点不重复 scan（仅挂载一次 + 显式刷新/恢复后失效）。补 L2 测试 | 汇总 P1-31 / 08 P1-5 |
| FE-20 | P1 | `src/App.tsx:44-69` | 启动串行加载多 store | 字体/快捷键/侧栏三个 `loadFromDisk` 改 `Promise.all` 并行（各自独立 try/catch 保留）；loadAllProjects 保持在其后（markPersistenceReady 时序不动）。补 L2 测试 | 汇总 P1-34 / 08 P1-8 |
| FE-21 | P1 | `src/features/sideViews/SideBarArea.tsx` | 侧栏视图 display:none 保挂载，隐藏视图留 DOM/订阅 | 隐藏视图按需卸载（切换时卸载旧视图组件——状态丢失语义 ADR-0001 已接受）；导航树滚动位置等轻状态不入保活范围。补 L2 测试 | 汇总 P1-35 / 08 P1-9 |
| FE-22 | P1 | `src/workspace/PageDockviewHost.tsx`/`panelRegistry.ts` | 单面板渲染错误扩大为整页崩溃 | 面板组件注册处统一包 inline ErrorBoundary（在 panelRegistry 的 components 映射中 HOC 包裹，单点改动）；补 L2 测试（构造抛错面板验证同页其他面板存活） | 汇总 P1-36 / 05#17、09#3 |
| FE-23 | P1 | `src/features/agentStatus/useAgentStatus.ts:307-344` | 初始扫描无 generation，快速切项目旧 setRows 覆盖新状态 | 引入 `genRef`（照 useFileTree 先例），setRows 前检查 generation。补 L2 测试 | 汇总 P1-41 / 09#9、10#11 |
| FE-24 | P2 | `src/panels/terminal/useXterm.ts:431-448` | `readHistoryTitle` promise 卸载后无取消 | `isDisposedRef` 守卫，卸载后忽略过期结果。补 L2 测试 | 09#11、10#10 |
| FE-25 | P2 | `src/panels/hooksConfig/useHooksConfig.ts:156-173` | `setLayer` async IIFE 异常未捕获；`confirmDiscard` timeout 未清理 | IIFE 加 try/catch + toast；timeout id 存 ref，effect cleanup `clearTimeout`。补 L2 测试 | 09#15、10#12 |
| FE-26 | P2 | `src/workspace/pageApis.ts:88-95` | `switchToPageAndFocus` 100ms×50 轮询不可取消 | 轮询支持 `AbortSignal`，调用处（toast 点击/导航树行点击）传入并在卸载/再次点击时 abort。补 L2 测试 | 10#13 |
| FE-27 | P2 | `src/features/agentHistory/restoreSession.ts:34-43` | `waitFor` 轮询不可取消，恢复流程可能在页面切换后误操作 | `waitFor` 接受 `AbortSignal`，循环前检查 `signal.aborted`；恢复编排四步共享一个 Controller，新恢复发起时 abort 旧的。补 L2 测试 | 10#14 |
| FE-28 | P2 | `src/App.tsx:237-246` | TitleBar/NotificationListener/浮层仅依赖 fullscreen ErrorBoundary | TitleBar、Workspace 容器、NotificationListener、ConfirmDialogHost、ToastHost 分别包 inline ErrorBoundary（降级渲染占位）。补 L2 测试 | 09#13 |
| FE-29 | P2 | `src/panels/terminal/TerminalPanel.tsx:206` | 加载遮罩 `transition: opacity 0.3s` 违反 ADR-0003 无动效 | 移除 transition（保持显隐切换） | 03#4 |
| FE-30 | P2 | `src/features/explorer/FileTree.tsx` | 递归渲染无虚拟化，大目录首屏/重渲染慢 | 引入虚拟化（手实现：扁平化可见节点数组 + 固定行高 +  overscan 滚动窗口——零新依赖）；保持键盘导航/右键菜单/选中模型行为。补 L2 测试。**人工验证点：大目录滚动/展开实测** | 08 P2-1 |
| FE-31 | P2 | `src/panels/editor/useCodeMirror.ts` | 大文件编辑无分页/虚拟化 | **按 D3 关闭**：Channel 分块（BE-03）削峰 + 保持 10MB 上限 + 1MB 警告；CodeMirror 不支持部分文档模型，不虚拟化。editor 模块 CLAUDE.md 登记决策 | 08 P2-2 |
| FE-32 | P2 | `src/panels/terminal/TerminalPanel.tsx` | `useLayout`/`useFontSize` 订阅粒度过粗 | 改 selector 精确订阅（zustand selector 仅取所需字段）。补 L2 测试 | 08 P2-4 |
| FE-33 | P2 | `src/workspace/Workspace.tsx` | `pageCallbacksRef` effect 依赖 allPages 重建回调 map | 回调按 pageId 惰性创建 + 缓存（getOrCreate 模式），effect 依赖收窄。补 L2 测试 | 08 P2-5 |
| FE-34 | P2 | `src/panels/terminal/useXterm.ts` | WebGL 上下文按焦点切换创建/释放 | 评估后改为：WebGL addon 加载失败回退才重建，焦点切换不主动释放（若实测无多上下文压力则登记关闭——人工验证点）。补/调 L3 测试 | 08 P2-7 |
| FE-35 | P2 | `src/features/index.ts`、`src/panels/index.ts`、`agentHistory/index.ts`、`commit/index.ts`、`panelRegistry.ts:20-49`、`workspace/index.ts`、`panels/terminal/index.ts:2`、`ipc/index.ts:19`、`ipc/window.ts:44` | 死代码：无消费 barrel×4、未用常量×4、冗余 re-export、ping() 仅测试用、setFocus() 预留 | 删 4 barrel 文件（grep 零消费确认，含测试）；`PANEL_GIT_SHOW`/`PANEL_DIFF`/`PANEL_HOOKS_CONFIG`/`terminalTabConfig` 生产改常量引用或删除（逐一 grep 后定）；workspace/terminal barrel 清理未消费 re-export；ping() 注释注明「测试专用」保留；setFocus() 删除。**每个删除点先 grep 消费方（含 `src/__tests__/`）** | 01#1~#8、01#10 |

## 4. TE 工具链/依赖（13 项）

| ID | 级别 | 位置 | 问题 | 修复要点 | 来源 |
|----|------|------|------|----------|------|
| TE-01 | P0 | `package.json`/`package-lock.json` | `serialize-javascript@6.0.2` RCE（GHSA-5c6j-r48x-rmvq，已核实 lock 版本）；`@wdio/*` 累计 21 high | `@wdio/*` 全栈升级 9.28.0→9.30.1（含 mocha-framework——serialize-javascript 升至 ≥7.0.5）+ `@wdio/tauri-plugin`/`@wdio/tauri-service` 1.1.0→1.3.0 + `expect-webdriverio` 升级 + Rust 侧 `tauri-plugin-wdio-webdriver` 1.1.0→1.3.0；npm audit 复验 high=0（WDIO 链路）；E2E 全量跑通 | 汇总 P0-1/P1-17 / 06#1、06#2 |
| TE-02 | P1 | `package.json` | `json-schema`（`src/panels/hooksConfig/JsonMode.tsx`）、`@lezer/highlight`（`src/theme/overrides.ts`）未声明（消费点路径已核实） | 显式加入 dependencies（锁定当前传递版本）；knip 复验 | 汇总 P1-18 / 01#14、06#3 |
| TE-03 | P1 | `package.json` xterm 三件套 | 终端核心路径用 beta（6.1.0-beta.288） | **保留 beta**（ADR 已记录特定动机：调查5修复，回退稳定版会回归）；package.json 上方注释或 `.claude/adr.md` 补升级审批约定（xterm 升级须全量 L3+E2E+实测滚轮）。登记关闭 | 汇总 P1-19 / 06#4 |
| TE-04 | P1 | `src-tauri/Cargo.toml` | `notify@9.0.0-rc.4`/`notify-debouncer-full@0.8.0-rc.2` 为 RC | **保持 RC**（rc.4 即最新，无稳定版可升——一手证据：Cargo.toml:36-37 已有跟踪注释）；watcher 回归测试已存在（notify 模块 51 条 L1）。登记关闭 | 汇总 P1-20 / 06#5 |
| TE-05 | P2 | `src-tauri/Cargo.toml` | `git2@0.20.4` 落后 0.21.0 | 升级 0.21 + vendored-libgit2 保持；git 模块 L1 全绿；API 变更适配（status/diff 调用点） | 06#6 |
| TE-06 | P2 | `Cargo.toml`/`package.json` | Tauri 核心/CLI/插件 patch 落后 | `tauri` 2.11.3→2.11.5、`@tauri-apps/cli` 2.11.2→2.11.4、各 plugin 按 wanted 升级；全量测试 | 06#7 |
| TE-07 | P2 | `package.json` | `typescript@6.0.3` → 7.0.2 | **按 D2 纳入**；`npx tsc --noEmit` 全绿 + vitest/eslint 工具链兼容验证；tsconfig 适配 | 06#8 |
| TE-08 | P2 | `package.json` | `dockview-react@6.6.1` → 8.1.0（跨 2 major） | **按 D2 纳入**；breaking changes 逐个适配（布局 serde/组件 API/样式）；layout-serde/workspace 全量测试 + **人工验证点：布局拖拽/分屏/恢复实测 + E2E** | 06#8 |
| TE-09 | P2 | `package.json` | `json-schema-library@9.3.5` → 11.6.2（跨 2 major）；与 `codemirror-json-schema` 职责重叠 | **按 D2 纳入**；升级并评估去重——能统一则统一到单一库（优先保留与 CodeMirror 集成更好的 codemirror-json-schema，保存校验改用它或保留 json-schema-library，以实测 API 为准）；hooksConfig 模块测试全绿 | 06#8、06#9 |
| TE-10 | P2 | `package.json` | `jsdom 29→30`、`jest-dom 6→7`、`@types/node 25→26`、`cross-env 7→10` | **按 D2 纳入**；L2 全绿（jsdom 30 行为变更适配） | 06#8 |
| TE-11 | P2 | `package.json` | 59 依赖版本策略不一致（8 精确 + 51 `^`） | 统一约定：生产运行时依赖精确版本、开发工具 `^`；package.json 逐条调整 + 文档登记约定 | 06#10 |
| TE-12 | P2 | `knip.json`（缺失，已核实不存在） | `@wdio/*` 因 e2e-tests 未入 entry 被 knip 误报 | 新建 `knip.json`：`e2e-tests/**/*.ts` 加入 entry；`npx knip --production` 零误报 | 06#11、01#13 |
| TE-13 | P2 | `.github/workflows/ci.yml` | 无依赖审计/死代码 CI 门禁 | **按 D10**：ci.yml 增加——`npm audit --registry=https://registry.npmjs.org/ --audit-level=high`（high 阻断）、`npx knip --production`、`cargo install cargo-audit && cargo audit`（advisory-db CI 可拉取；本地网络受限跳过） | 06#9 建议、01#8 建议 |

## 5. DOC 文档（10 项）

| ID | 级别 | 位置 | 问题 | 修复要点 | 来源 |
|----|------|------|------|----------|------|
| DOC-01 | P1 | `.claude/CLAUDE.md` 约束 #11 | 全量测试约束与 DOC-01 豁免机制脱节 | 修订约束正文：可自动化部分必须覆盖；不可自动化部分须在 test-inventory 既定豁免清单登记并注明原因与兜底层级 | 02#1 |
| DOC-02 | P1 | `.claude/CLAUDE.md` 约束 #9 | 未区分业务 cfg 与测试 cfg | 按 D5 修订：业务 cfg 仅 pty/conpty_api/shell/win_build；测试 cfg 原则上改 `cfg!()`，例外须模块 CLAUDE.md 登记（BE-17 联动） | 02#2 |
| DOC-03 | P2 | `.claude/CLAUDE.md` | stores/ 全局状态无硬约束 | 新增约束：store 只存状态不存业务逻辑、持久化经指定 IPC、禁止跨 store 隐式依赖 | 02#3 |
| DOC-04 | P2 | `.claude/CLAUDE.md` | 注册表家族无通用契约 | 新增约束：模块级单例、`register/getAll/_reset` 接口形态、side-effect import 触发、`_reset` 测试隔离 | 02#4 |
| DOC-05 | P2 | `.claude/CLAUDE.md` 约束 #5 | 未覆盖 hub 容器 + 注册表分派子编辑器模式 | 补充合法形态条款（hooksConfig 先例） | 02#5 |
| DOC-06 | P2 | `.claude/CLAUDE.md` 约束 #4 | 泛化字段语义值集散落 profile 与 provider 两处 | 补充「字段类型泛化后语义值集须在 profile 与后端 provider 同步登记并配合同步测试」 | 02#6 |
| DOC-07 | P2 | `.claude/CLAUDE.md` 约束 #6 | 配色例外清单未写入约束正文 | 完整例外清单写入约束正文；规定新增例外须同步登记对应模块 CLAUDE.md | 02#7 |
| DOC-08 | P2 | 项目根 | 缺 `README.md` | 新建：项目定位、构建/测试命令、文档链接（CONTEXT.md/adr.md/test-inventory） | 03#5 |
| DOC-09 | P2 | `CONTEXT.md:27` | 面板类型 `html` vs 注册 id `htmlviewer` | 改 `htmlviewer`（与 panelRegistry.ts:18 一致） | 03#6 |
| DOC-10 | P2 | 各登记点 | 豁免/决策登记散落 | 汇总登记：FE-01（Workspace 豁免）、SEC-09（CSP）、SEC-06（剪贴板）、BE-21（read_dir 分页豁免）、FE-31（CM 不虚拟化）、09#14（Mutex 中毒保持现状——parking_lot/catch_unwind 仅作未来引入高风险外部代码时的预案，登记于 src-tauri/CLAUDE.md）、TE-03/TE-04 | 09#14 等 |

## 6. 去重合并留痕（汇总 105 → 本清单 93）

- 汇总已去重 130→105（映射表见 `00-汇总.md` 第 8 节）
- 本清单进一步合并：P1-3 并入 BE-04；P1-10+P1-11 并入 FE-08；P1-30+P1-40 并入 FE-18；01#9+#11+P1-21 并入 SEC-03；05#18+#19 并入 BE-15；04#4+#6+#7 并入 FE-14；06#8 拆为 TE-07~TE-10；06#9 并入 TE-09；08 P2-2 单列 FE-31（决策关闭）；08 P2-3 单列 BE-21（登记豁免）；09#14 并入 DOC-10；05#20 并入 FE-02；01#13 并入 TE-12；01#14 并入 TE-02；10#15 并入 BE-11；05#3/#6/#11 并入 FE-04；07#14 并入 SEC-14（随 BE-04）
