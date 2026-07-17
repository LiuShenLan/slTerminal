# Review 汇总报告

> 6 个维度 subagent 全量 review 的汇总。只列存在的问题，详细论证见各分报告。
> 分报告：`review-01-rust-backend.md` / `review-02-frontend-react.md` / `review-03-tests.md` / `review-04-docs-consistency.md` / `review-05-arch-constraints.md` / `review-06-security.md`

## 总览

| 维度 | 高 | 中 | 低 | 合计 |
|------|----|----|----|------|
| 01 Rust 后端质量 | 3 | 5 | 10 | 18 |
| 02 前端 TS/React 质量 | 2 | 8 | 11 | 21 |
| 03 测试用例质量 | 4 | 10 | 4 | 18 |
| 04 文档与代码一致性 | 4 | 8 | 2 | 14 |
| 05 架构硬约束合规 | 1 | 3 | 1 | 5 |
| 06 安全审查 | 3 | 5 | 2 | 10 |
| **合计（含跨维度重复）** | **17** | **39** | **30** | **86** |

## 跨维度共性问题（多份报告独立发现，可信度高）

| 问题 | 出现维度 |
|------|---------|
| PTY 命令（spawn/write/resize/reattach）同步执行阻塞 I/O，未用 `spawn_blocking` | 01（高）+ 05（高，违反约束 #3） |
| 路径沙箱失效：`project_root` 从未被设置 + `validate_path_within_root` 相对路径绕过 | 01（高）+ 06（高） |
| capabilities 权限模型与硬约束 #10 矛盾（自定义命令默认放行，无逐条白名单） | 01（中）+ 04（高）+ 06（中） |
| test-inventory.md L3 用例数严重失实（实际 116，文档写 9） | 03（高）+ 04（高） |
| `forwardGlobalShortcuts.ts` 死代码（生产已走 postMessage 路径，文档仍描述旧路径） | 02（低）+ 04（高） |
| `useFontSizeBridge.ts` 死代码（实际用的是 `lib/useFontSizeWheel.ts`） | 02（低）+ 04（中） |
| `fs_rename` 行为缺陷（Windows 不覆盖已存在文件；目标为目录时无条件递归删除） | 01（中）+ 06（中） |

## 高危问题清单（去重后 13 项）

### 1. 路径沙箱全面失效（01+06）

- **位置**：`src-tauri/src/state.rs:73/89/99-135`、`src-tauri/src/fs/mod.rs:113-166`
- **问题**：三重叠加——① `AppState.project_root` 全代码库无写入点，永远为 `None`，`validate_path_within_root` 直接返回 `Ok(())`，所有 `fs_*`/`notify_watch` 沙箱校验形同虚设；② `fs_read_dir` 根本未调用校验；③ 校验函数本身对不存在的相对路径（`..\secret.txt`）用 `join` + `starts_with` 词法比较，可穿越到项目根外。
- **建议**：新增 `set_project_root` 命令在前端切换项目时写入；`fs_read_dir` 补校验；相对路径先解析为绝对再 `dunce::canonicalize`，目标不存在时拒绝而非词法拼接。

### 2. `pty_spawn` 允许前端指定任意 shell 路径与 cwd（06）

- **位置**：`src-tauri/src/pty/spawn.rs:599-604`、`src-tauri/src/pty/shell.rs:50-65`
- **问题**：`SpawnRequest.shell`/`cwd` 无白名单、无沙箱校验，前端可让后端 `CreateProcessW` 启动任意可执行文件。结合 Tauri 2 默认放行自定义命令，等于本地任意代码执行入口。
- **建议**：shell 限制为允许列表（pwsh/powershell/cmd）；cwd 过 `validate_path_within_root`。

### 3. PTY 四个命令同步执行阻塞 I/O（01+05，违反约束 #3）

- **位置**：`src-tauri/src/pty/spawn.rs:600/764/786/847`（`pty_spawn`/`pty_write`/`pty_resize`/`pty_reattach`）
- **问题**：同步 `#[tauri::command]` 直接执行 ConPTY 创建、管道写、`ResizePseudoConsole`，占用 webview 调度线程致 UI 冻结；`pty_kill` 已改 `async + spawn_blocking`，其余未改。`notify_watch`（`notify/mod.rs:213`）同样问题。
- **建议**：统一改 `async fn` + `spawn_blocking` 包裹阻塞段。

### 4. Job Object 分配失败时子进程成孤儿（01）

- **位置**：`src-tauri/src/pty/spawn.rs:686-705`
- **问题**：`add_to_job_object` 失败时 `?` 提前返回，`RawChild` drop 只关句柄不杀进程，子进程脱离 `KILL_ON_JOB_CLOSE` 保护。
- **建议**：失败路径显式 `child.kill()`，或将 Job 分配并入 `spawn_conpty_child` 使其原子化。

### 5. `useCodeMirror` 文件加载过期写入竞态（02）

- **位置**：`src/panels/editor/useCodeMirror.ts:232-289`
- **问题**：`filePath` 快速切换时，旧 `initEditor` 的 `await fs.readFile` 完成后看到 `mountedRef.current === true`（已被新 effect 重置），旧 `EditorView` 覆盖新实例——用户看到旧文件内容，且旧 view 永不 cleanup（DOM + 监听器泄漏）。
- **建议**：引入 generation 计数或 AbortController，异步回调先比对再写入。

### 6. HtmlPanel 注入脚本未处理宿主 HTML 的 `</script>`（02）

- **位置**：`src/panels/html/HtmlPanel.tsx:45-61`
- **问题**：用户 HTML 含 `</script>` 时提前闭合注入脚本，后续代码被当 HTML 解析，键盘转发/片段拦截失效，甚至执行非预期脚本。
- **建议**：注入前将宿主 HTML 的 `</script>` 转义为 `<\/script>`。

### 7. 核心 PTY 命令与生命周期零测试覆盖（03）

- **位置**：`src-tauri/src/pty/spawn.rs:599-884`、`src-tauri/src/pty/reader.rs:31-153`
- **问题**：5 个 Tauri 命令（env 注入、CPR 注入、ring buffer 回放、reattach、Job Object）无任何单元/集成测试；spawn.rs 的 13 条测试只覆盖 `conpty_custom` 子模块。
- **建议**：mock AppState + 假 Channel 补单元测试；集成测试补 reattach 回放用例。

### 8. `useXterm` 编排层测的是 mock 而非实现（03）

- **位置**：`src/panels/terminal/useXterm.ts:304-407`
- **问题**：7 个子 hook 全 mock，真实编排分支（rAF 轮询失败回退 80×24、`term.onData`→`pty.write` 链路、visible 切换 WebGL 释放/重建、64KB 上限丢弃）均无覆盖。
- **建议**：增加"轻 mock"集成测试（真实 Terminal/FitAddon，仅 mock IPC）。

### 9. `titleManager.onDeletePage` 完全未测（03）

- **位置**：`src/workspace/titleManager.ts:173-176`
- **问题**：页面删除时 registry/counters 清理逻辑零用例，内存泄漏与标题错乱风险无守卫。
- **建议**：补删除后终端编号重置、SaveAs 同名冲突用例。

### 10. 硬约束 #10 与实际权限模型矛盾（01+04+06）

- **位置**：根 `.claude/CLAUDE.md:46` vs `src-tauri/capabilities/default.json:7-13`、`src-tauri/src/lib.rs:23-29`
- **问题**：文档要求"新增命令必须在 capabilities 显式放行"，实际 Tauri 2 自定义命令默认全放行，PTY 5 条命令均未声明；生产二进制还携带 `wdio-webdriver:default` 权限。
- **建议**：二选一——收紧 capabilities 逐命令白名单 + 按 profile 分离 wdio 权限；或更新文档承认默认放行模型。

### 11. test-inventory.md 与实际测试规模严重脱节（03+04）

- **位置**：`.claude/test-inventory.md:3-5/165-167`
- **问题**：L3 实际 4 文件 116 用例，文档写"2 文件 9 用例"；总数、L1/L2 数字、`npm test` 范围描述均失实；计数口径不统一（`it.each` 展开 vs 定义数）。
- **建议**：按 grep 实测数字重写清单，统一口径，作为用例数单一真值源。

### 12. 根 CLAUDE.md 规划目录含不存在的 `claude/` 模块（04）

- **位置**：根 `.claude/CLAUDE.md:33`
- **问题**：`src-tauri/src/` 下无 `claude/` 目录，误导后续开发。
- **建议**：移除或标注未实现。

### 13. HTML 面板键盘转发机制文档多处过时（02+04）

- **位置**：`src/features/shortcuts/CLAUDE.md:179`、`e2e-tests/CLAUDE.md:80` vs `src/panels/html/HtmlPanel.tsx:32-34/45-61/88-112`
- **问题**：文档声称 iframe 用 `sandbox="allow-scripts allow-same-origin"` + `attachGlobalShortcutForwarder(contentDocument)` 转发按键；实际生产为 `sandbox="allow-scripts"`（无 allow-same-origin）+ 注入脚本 postMessage 桥。`forwardGlobalShortcuts.ts` 已成死代码仅测试引用。文档描述的机制与真实攻击面（postMessage 未校验 origin，见中危-安全）不符，误导安全判断。
- **建议**：更新两份 CLAUDE.md 与 `forwardGlobalShortcuts.ts` 文件头注释，明确 postMessage 为现行路径、旧函数为测试保留（或删除）。

## 中危问题索引（39 条，摘要）

### Rust 后端（5）

- `fs_write_file` 为检测行尾读整个文件入内存 → 只读前 64KB 样本（`fs/mod.rs:79-87`）
- `fs_rename` Windows 上不覆盖已存在文件，与注释不符 → rename 前统一删目标（`fs/mod.rs:244-250`）
- `save_settings` 的 `create_dir_all` 在 `spawn_blocking` 外执行 → 移入闭包（`settings.rs:34-37`）
- git 仓库缓存 `workdir.starts_with(&search)` 会误命中子仓库 → 删除该分支（`git/mod.rs:78-90`）
- `notify_watch` 持池锁期间创建 watcher（大目录注册 ~2s 阻塞全池）→ 锁外 start（`notify/mod.rs:233-250`）

### 前端（8）

- `Workspace.tsx:168-182` render 阶段直接增删 ref（反模式，StrictMode 下半一致状态）→ 移入 useEffect
- `PageDockviewHost.tsx:262-304` 三个 Dockview 监听器 disposable 未保存/清理 → ref 收集统一 dispose
- `main.tsx:8-19` bootstrap `setInterval` 无上限无清理 → 加最大轮次 + 超时报错
- `useFileTree.ts:166-184` `refreshExpanded` 的 gitStatus 无 generation 防护 → 快照比对
- `ExplorerPanel.tsx:100-110` `addPanel` 未 try-catch，失败后 titleManager 仍注册 → 成功后方可注册
- `usePtyOutput.ts:156-178` retry 的 `term.onData` disposable 卸载路径未 dispose → cleanup 补 dispose
- `TerminalPanel.tsx:72` `state.icon!` 非空断言 → 改 `?? null`
- `layoutSerde.ts:97-98` `fromJSON(layout as any)` → 用 `Parameters<DockviewApi["fromJSON"]>[0]`

### 测试（10）

- Rust 集成测试固定 sleep（800/1000/500ms）依赖真实 cmd.exe，flaky 风险 → 改输出 marker 轮询
- `layout-serde.test.ts:3` 注释声称"覆盖率 0%"与实际 16 用例矛盾 → 删误导注释
- `layoutSerde` 嵌套 branch 修补、`activeGroup` 保留未覆盖
- `ShortcutRegistry` 监听器安装/移除无 spy 断言；`setOverrides(undefined)`、`exportContextBindings` 覆盖影响未测
- `notify/pool.rs` 测试 stop/Drop 断言不足；`make_test_watcher` 的 stop 通道与线程不联通
- 两处 CPF 测试 `if (writeCalls.length > 0)` 条件断言（未写入即自动通过）→ 先断言长度
- NF3 测试标题与断言矛盾（标题说抑制直写，断言直写不被抑制）
- 大量 `waitFor` 未设超时（默认 1s，CI 易 flaky）→ 显式 2000-5000ms
- 多文件 debounce 测试用真实 `setTimeout` → 统一 `vi.useFakeTimers()`
- `setup.ts` 全局 mock（notify、canvas getContext）跨测试泄漏风险 → 注释警告 + 恢复机制

### 文档（8）

- `pty/CLAUDE.md` JobHandle"非 Windows 零占位"与实际 `#[cfg(windows)]` 初始化矛盾（非 Windows 编译会 E0063）
- `workspace/CLAUDE.md` 称 `Workspace.tsx`"已废弃"，实际仍是主组件
- `stores/CLAUDE.md` 规则要求持久化走 `ipc/fs`，实际 fontSize/keybindings 走 `ipc/settings`
- `ipc/CLAUDE.md` 未记录 `index.ts` 的 `ping()` 命令
- `panels/CLAUDE.md` 称 useXterm 组合 `useFontSizeBridge`，实际为 `useFontSizeWheel`
- `panels/CLAUDE.md` 注入脚本描述（两部分 + location.hash）与实际（三部分 + classList）不符
- `layoutSerde.ts:70-74` 注释仍写"失败回退创建默认终端"，与现行 Watermark 策略矛盾
- 根 CLAUDE.md L1/L2 用例数（~196/~1095）与实际（190/1020）不符

### 架构约束（3）

- 违反 #6：`App.css:5-8`、`FileTree.tsx:71`、`SidebarTree.tsx:82` 硬编码颜色 → 收编 `theme/colors.ts` token
- 违反 #8：`useXterm` 在 TerminalRegistry 外自建 `sessionIdRef` 副本并透传子 hook → 统一从 Registry 读取
- 违反 #3（notify_watch 部分见"Rust 后端"末条）

### 安全（5）

- HTML postMessage 键盘桥未校验 `e.origin`（`"*"` 发送 + 不验证接收），不可信 HTML 可伪造全局快捷键 → 校验 origin、加信任标记
- 全局 CSP `script-src 'unsafe-inline'` 为预览放宽，主应用失去内联脚本防护 → 维护放宽清单，新渲染面逐案评估
- `fs_rename` 目标为已存在目录时无条件 `remove_dir_all` → 返回错误，覆盖需显式确认
- `git_status`/`git_diff` 未过项目根校验，`Repository::discover` 可上溯到父仓库泄露信息 → 入口补校验
- 生产 capabilities 含 `wdio-webdriver:default`（见高危 #10）

## 低危问题索引（30 条，一句话）

- **Rust（10）**：`RawChild::try_wait/wait` 未查 `WaitForSingleObject` 错误值；`clone_killer` 句柄失败 panic；多处锁中毒 `.expect` panic；`build_env_block` HashMap 顺序不定；`which_full_path` 固定 `;` 分割 PATH；SPAWN_LOCK 粒度超注释范围；启动序列剥离跨 16KB 边界失效；`COORD` 尺寸 `as i16` 溢出回绕；notify_watch 持锁创建 watcher（同中危末条，维度定级不同）；unsafe 块缺 SAFETY 注释
- **前端（11）**：`panelRegistry.isAlwaysRenderPanel` 冗余分支；`useFontSizeBridge.ts` 死代码；`forwardGlobalShortcuts.ts` 死代码；`document.fonts.ready` Promise 无取消；useTerminalInstance dispose/cleanup 逻辑重复；`ipc/notify.ts` 与 `types/notify.ts` 的 `detail` 可选性不一致；`TerminalRegistry.getAll` 返回可写 Map；`main.tsx`/`ErrorBoundary` 用 `as any` 访问 window；`handleSave` 根路径 git repoDir 计算有误；SidebarTree 内联子组件每次渲染重建；FileTree paddingLeft 魔法数字；PageDockviewHost 访问 `panel.view` 非公共 API
- **测试（4）**：`src/__tests__/CLAUDE.md` 用例数不一致；git 测试用 `compute_diff_hunks` 副本验证（副本与生产易脱节）；fs/settings 命令包装层未直接测试；E2E 键盘用例无法覆盖真实 OS 按键路径（已知局限）
- **文档（2）**：10+ 处测试用例数与文档不符（panels/shortcuts/__tests__ 各 CLAUDE.md）；useXterm.ts 行数描述偏差（~410 vs 实际 422）
- **架构（1）**：约束 #9 `#[cfg(windows)]` 扩散到 state.rs/fs/lib.rs 的测试代码（6 处）
- **安全（2）**：`resolve_shell` 旧路径仍接受任意 shell（死代码隐患）；`pty_write` 不校验 session 归属（UUID 难猜，风险低）

## 修复优先级建议

1. **P0（安全 + 数据正确性）**：高危 #1 路径沙箱、#2 shell 白名单、#5 useCodeMirror 竞态、#6 HtmlPanel `</script>`、#4 Job Object 孤儿
2. **P1（性能 + 架构合规）**：高危 #3 spawn_blocking 改造、约束 #8 sessionId 单点化、约束 #6 颜色 token 化
3. **P2（测试可信度）**：高危 #7/#8/#9 覆盖盲区补测、条件断言/真实定时器/waitFor 超时整改
4. **P3（文档同步）**：高危 #10/#11/#12 及全部文档失实项——以 test-inventory.md 为用例数单一真值源重写
