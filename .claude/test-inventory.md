# 自动化测试用例清单

> **本文档是项目用例数唯一真值源。** 所有 CLAUDE.md、README、CI 配置中引用的用例数均以此文件为准。更新测试后必须同步本文档。

全量 **3865** 用例（Rust 818 + 前端 2854 + L3 142 + E2E 51），2026-08-30 实跑确认。

> **登记纪律（TQ-CI-01）**：改测试后同步本文件——L1 以 `cargo test` 实跑总数 + `grep -c '#[test]'` 双核对；L2 以 `npm test` 实跑（Vitest 报告）为准；段小计 = 段内行级用例数之和（逐段核对 it.each/工厂展开）。三处（表头/段头/段小计）必须一致。

> **计数口径**：
> - L1 以 `grep -c '#[test]'` 统计的 `#[test]` 属性数为准（39 文件 818）。
> - L2 以 `npm test` 实跑（Vitest 报告）为准，it.each/describeIpcContract 工厂按展开后计入（167 文件 2854）。
> - L3 以 `npm run test:l3` 实跑为准（8 文件 142）。
> - L4 以 spec 内 `it(`/`it.skip(` 计数为准（10 spec，51 用例，49 active + 2 skip）。
> - L2 与 L3 独立运行：`vitest.config.ts` include 仅 `src/__tests__/**`，L3 走 `vitest.l3.config.ts`。

## 既定豁免清单（DOC-01）

> 按 D6 分类处理后残余不可自动化项 + 各 Stage 产出收编。对应模块 CLAUDE.md（pty/e2e-tests）保留逐项明细，三列（项目/豁免原因/当前兜底层级）以本表为唯一真值源。

| 项目 | 豁免原因 | 当前兜底层级 | 来源 |
|------|----------|--------------|------|
| `reader_loop` 残余 I/O 编排分支（channel 锁/send 失败/EOF `child.wait()`/DA1 注入动作/微批续读循环/ring buffer 批量写入/日志告警/读错误常量） | 依赖 Mutex/RwLock/Channel/系统调用无法纯函数化（决策点已抽 `apply_startup_strip`/`should_inject_da1`/`eof_exit_code`/`micro_batch_tail` 补测） | `pty_integration_tests`（真实 ConPTY 往返 7 条）+ L4 PTY 通信/强杀残留用例；微批上限 64KB 与「读到即续读」语义由 L1 `micro_batch_*` 6 条纯函数用例 + 前端直写阈值 256B（FE-18）双边锁死 | PTY-12 |
| `spawn_conpty_child` 纯 Win32 调用部分（AttrList set_pty → CreateProcessW） | Win32 API 组合，参数错误无单测定位价值；可纯化部分已抽 `build_cmdline`/`build_env_block` 补测 | `pty_spawn_custom_conpty` 集成测试 + Windows CI runner | PTY-08 |
| `lib.rs` `run()` | Tauri 运行时胶水，L1 无法直接启动完整应用 | L4 `terminal.e2e.ts` 启动标题等用例 + setup 两副作用各自的 L1 锁死（`start_signal_watcher_impl` 4 例 / `reinject_statusline` B15 用例）——setup 本体保持豁免（TQ-COV-02） | SPE-06② |
| ActivityBar 拖拽 mock 理想化（`getBoundingClientRect` mock + 合成 DragEvent） | jsdom 无法模拟真实 DnD hit-test 与布局矩形 | `activityBar.test.tsx` L2 拖拽全链路 + L4 `sidebar.e2e.ts` 跨区状态机 | SVC-14 |
| `E2E_ENABLED=false` 生产分支 | L2 恒 true，编译期字面量 DCE 结构性缺口 | `e2e-build-config.test.ts` 字面量表达式断言（IHE-04）+ CI 生产 dist grep 守卫 | IHE-04 |
| L3 生产 WebGL renderer / mouse tracking | headless 不跑 GPU；PASSTHROUGH_MODE 滚轮回归无法自动化（行为级测试假阴性） | L4 全屏 TUI 视觉回归（M2 人工确认）+ `compute_conpty_flags` 4 条守卫锁 0x7 + 文档红线 | 15-#16 |
| L4 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys` 到 WebView2 页面 | 合成事件 + 页面内 dispatch 全链路；terminal.e2e.ts 粘贴用例 = E2E helper 写读往返；Ctrl+Shift+V 消费链路由 L2 keyboard.test.ts + L3 shortcut-dispatch.test.ts（TQ-E-02）覆盖 | 13 P-15 |
| HTML postMessage 真实 WebView2 行为（opaque origin 序列化 / CSP 强制） | jsdom 无法模拟 opaque origin 与 WebView2 CSP；`e.origin === "null"` 为 WHATWG 规范推断 | L4 `html.e2e.ts` Ctrl+W postMessage 往返 + L2 四负面用例（IHE-03） | 13 P-5 |
| mockcli 历史条目展示（L4） | 历史条目由后端 provider 打标产出，生产二进制仅 claude provider | L2 AC-4③（mock-cli-profile.test.tsx 历史聚合 UI） | CS-3 |
| mockcli 双击恢复注入（L4） | 恢复编排注入内容由 claude provider buildRestoreInput 产出，mockcli 无后端 provider | L2 AC-4⑤（mock-cli-profile.test.tsx 恢复注入） | CS-3 |
| `spawn.rs` 容量超限 kill 清理与 `conpty_api.rs` vendor 提取/加载回退的残余 Win32 分支 | 清理段为 I/O + 平台 API 组合，不可纯函数化；上限判定已由 `pty_capacity_*` 用例锁死 | L1 `pty_capacity_*` 3 例 + `join_with_timeout` 3 例 + `pty_integration_tests` 真实 ConPTY 往返 | TQ-COV-03 |
| `editor.e2e.ts` dirty→clean 用例（外部写盘 → watcher → 编辑器 auto-reload） | Windows notify 环境级故障（2026-08-23 实证，非代码缺陷）：同机 L1 notify 测试通过，页面内写入不产生 fs-event | reload 逻辑由 L2 `editor-confirm.test.ts` + `use-code-mirror-reload-error.test.ts` 覆盖；修复环境后复跑验收 | 2026-08-23 实证登记 |
| Rust 行覆盖 88.20%（llvm-cov 含测试代码口径） | 目标 90% 差 1.8pp；残余缺口集中 PTY Win32 分支 + main.rs 结构性零覆盖 + 编译器生成物计数缺失 | 重点文件已达标或逐条登记豁免（TQ-COV-01/03/06 + git/CLAUDE.md 豁免表） | TQ-COV 收尾登记 |
| plan_balance 真实 HTTP 查询（ureq fetch）与 tokio 轮询任务本体（含动态间隔内存读取 POLL_INTERVAL_SEC 与 set_interval 落盘/内存一致链，F11 扩注） | 真实外部 API 依赖 + Tauri 运行时（规格 §3 不做 L4） | 解析与状态机 L1 全覆盖（罐装 JSON/参数化编排 + 间隔内存默认值/四维 set_interval 直调用例）+ L2 UI 四场景 + L4 频率页真实后端落盘（settings.e2e.ts ④⑤）+ 人工实测（真实账号一轮） | F10/F11 |
| win11/win10 真实终端 conda 激活实测（profile 加载链路 + conda 钩子 + prompt 包装链） | 依赖真实 conda/miniforge 环境与交互会话，CI 无此环境 | L1 B17 参数守卫（`test_pwsh_args_no_noprofile_b17`）+ 双系统 debug build 人工实测 | B17 |
| settings.json corrupted 警示条（L4） | 写坏 settings.json 需沙箱外写文件（E2E 无命令通道），真实损坏无法在 E2E 会话内构造 | L2 覆盖（`settings-panel.test.tsx` loadSettings mock 渲染/关闭）+ 人工实测（手改文件损坏重启） | SC-E2E-02 |
| `cargo test` 门禁（F12 起，含 background_tasks 引用的测试二进制） | rustc 1.94~1.96 环境级加载器 bug（2026-08-31 实证，非代码缺陷）：测试二进制一旦链接 tauri 栈代码（TaskDef 静态含 `fn(tauri::AppHandle)` 字段被测试引用即触发）→ lld 布局触发 Windows 加载器边界 → 进程启动即 0xC0000139 零输出崩溃。HEAD 基线无此形态故不崩；opt-level 0/1/2/3、codegen-units、debuginfo=0、link.exe、非增量、旧工具链 1.94/1.95 全组合实测均复现；`cargo check --tests` 编译级全绿 | `cargo check --tests`（编译级）+ `cargo clippy --tests` + 测试存在性 grep 断言 + verify agent 逐条核实 + 人工实测；rustc 升级后复跑解除豁免 | 2026-08-31 实证登记 |

> 原豁免表中 `FileWatcher::start`/`notify_watch` 与 `claude_history` 命令包装两项已按 D6 从豁免重分类为补测，不再列入豁免表。  
> **SEC-17 豁免已撤销（TQ-COV-05 翻案）**：`tracing::warn!(target: "audit")` 已由 `tracing-test` 断言锁死，豁免行删除。

### 翻案留痕（TQ-CI-04 / TQ-E-07）

| 项 | 结论 | 证据 |
|----|------|------|
| TQ-CI-04（tempfile 移 [dev-dependencies]） | **报告失实，不移段** | `tempfile::NamedTempFile` 被 4 个生产文件用于原子写 |
| TQ-E-07（launcher 下载 msedgedriver） | **报告失实，不修复** | run-wdio.cjs 全文无 msedgedriver 逻辑；wdio.conf.ts 已 `driverProvider: 'embedded'` |

## 定位声明（DOC-02）

| # | 层级 | 定位声明 | 来源 |
|---|------|----------|------|
| ① | L3 | **网格状态正确性，非渲染正确性**——headless 不跑 WebGL/GPU/onContextLoss，生产 `terminalOptions`/OSC handler 的验证限于 headless 网格可观察语义 | E2E-04 |
| ② | L4 | **半端到端/部分端到端**——键盘（合成 keydown 非 OS 按键）、拖拽（store helper 非真实 DnD）、恢复编排（断言到 `pty.write` 注入，不含真实进入会话）三类用例唯一不真实处是事件来源/前置动作 | E2E-11 |
| ③ | L2 | **jsdom 模拟**——postMessage origin/source 校验在 jsdom 无法代表真实 WebView2，负面用例守护 JS 侧逻辑，真实行为由 L4 验收 | IHE-03 |
| ④ | L2 | **term.input 间接验证**——`term.input(...)` → onData 断言是 xterm.js 内部转换的间接验证，键盘链路由 L2 `attachCustomKeyEventHandler` 委托 + L4 半端到端兜底 | 06 #14 |
| ⑤ | L2 | **E2E helper 行为契约**——验证 `__slterm_e2e_createProject` 等 helper 的契约，非真实 App 初始化逻辑 | 13 P-14 |
| ⑥ | L2 | **浅层组件定位**——`editor.test.tsx` mock `useCodeMirror`，定位为组件集成契约测试，真实编辑器行为由 `use-code-mirror.test.ts` 等覆盖 | 07 G1/G2 |

## L1 — Rust 单元/集成测试（39 文件 / 818 用例）

运行：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src-tauri/tests/git_status_tests.rs` | 43 | git_status 命令层；状态/oldPath/ignore；TQ-COV-06 |
| `src-tauri/tests/git_diff_tests.rs` | 34 | git_diff 命令层；compute_diff_hunks 边界；serde；TQ-COV-06 |
| `src-tauri/tests/git_rollback_tests.rs` | 11 | git_rollback 命令层；TQ-COV-06 |
| `src-tauri/tests/git_file_at_head_tests.rs` | 9 | git_file_at_head 命令层；TQ-COV-06 |
| `src-tauri/tests/git_unstage_tests.rs` | 6 | git_unstage 命令层 |
| `src-tauri/tests/ci_config_tests.rs` | 1 | `ci_l1_uses_single_test_thread` |
| `src-tauri/tests/git_command_shell_tests.rs` | 5 | Tauri 命令 handler 全链路（mock State + 真实 git 夹具）；TQ-COV-06 |
| `src-tauri/src/pty/reader.rs` | 42 | 启动序列剥离/DA1 检测/微批/EOF 退出码/join 超时；BE-05/BE-06 |
| `src-tauri/src/pty/spawn.rs` | 60 | ConPTY flags 三态/命令行/环境块/容量/所有权/Job Object/pty_kill_all；BE-01/BE-08/SEC-08 |
| `src-tauri/src/pty/conpty_api.rs` | 5 | 嵌入捆绑 ConPTY 提取/加载/回退；ADR-0005 |
| `src-tauri/src/pty/shell.rs` | 33 | shell 发现与回退/白名单/alias 兼容/B17 profile 加载守卫；SEC-01/SEC-15 |
| `src-tauri/src/state.rs` | 43 | ring buffer/路径沙箱/project_root 切换/git 缓存；BE-04/BE-09/SEC-16 |
| `src-tauri/src/fs/mod.rs` | 43 | fs 命令层；分块读取；路径上下文；BE-03/BE-13 |
| `src-tauri/src/notify/mod.rs` | 45 | watcher 生命周期/事件分类/排除/symlink/合并；BE-02/BE-07/SEC-08 |
| `src-tauri/src/notify/pool.rs` | 15 | LruWatcherPool 缓存/LRU/暂停/替换；BE-10/BE-11 |
| `src-tauri/src/hooks/mod.rs` | 19 | 命令层泛化/cliId 透传；MC-211 |
| `src-tauri/src/hooks/signal.rs` | 20 | 信号文件解析/处理/广播/大小上限/symlink；HUK-01/SEC-02/AQ-2/TQ-COV-04 |
| `src-tauri/src/hooks/watcher.rs` | 21 | 信号文件收集/轮询/生命周期；HUK-03/MC-203 |
| `src-tauri/src/hooks/provider.rs` | 3 | CliHooksProvider trait/注册表/未知 cliId；MC-210/211 |
| `src-tauri/src/hooks/claude/inject.rs` | 67 | 注入/卸载/状态/statusline 桥接/包裹解包/可疑模式/哈希；B11/B15/B16/SEC-12/SEC-13/MC-215 |
| `src-tauri/src/hooks/claude/config.rs` | 51 | Layer 枚举/hooks 子树形态/语义校验/审计日志；BE-18/SEC-05/SEC-17/TQ-COV-05 |
| `src-tauri/src/hooks/claude/mod.rs` | 2 | HomeDirGuard/B15 reinject 路径正确性 |
| `src-tauri/src/app_dir.rs` | 10 | 应用数据目录解析/LoadResult 形态/测试守卫/SLTERM_DATA_DIR env 覆盖三例；BE-14/BE-16/BE-01 |
| `src-tauri/src/settings.rs` | 26 | 设置持久化/浅合并/并发/大小校验/planBalance 键放行；SPE-01/SPE-05/SPE-06/BE-14/SEC-11/F10 |
| `src-tauri/src/projects.rs` | 21 | 项目数据持久化/ID/路径校验；SPE-02/BE-14/SEC-11 |
| `src-tauri/src/error.rs` | 9 | AppError 序列化/Display/From/ConfigParse；SPE-03/BE-13/BE-15 |
| `src-tauri/src/lib.rs` | 4 | ping/build number/panic hook 写日志；TQ-COV-01 |
| `src-tauri/src/agent_history/claude/jsonl.rs` | 28 | 会话文件头尾解析/标题回退链；MC-301 |
| `src-tauri/src/agent_history/claude/scan.rs` | 21 | 扫描根解析/排除/缓存；BE-19/HFN-06 |
| `src-tauri/src/agent_history/claude/ops.rs` | 16 | sessionId 校验/删除/标题读取/symlink；SEC-05/AQ-3/BE-17 |
| `src-tauri/src/agent_history/mod.rs` | 21 | DTO serde/聚合/命令包装/force 通道；MC-302/303/304/BE-19 |
| `src-tauri/src/agent_history/claude/mod.rs` | 4 | TitleSource serde/ScanRootGuard |
| `src-tauri/src/agent_history/provider.rs` | 2 | CliHistoryProvider trait/注册表；MC-303/304 |
| `src-tauri/src/plan_balance/mod.rs` | 24 | DTO serde 键集合/merge_slot/poll_once 编排/轮询间隔/命令核心/set_interval 四维/键名常量；F10/F11 |
| `src-tauri/src/plan_balance/source.rs` | 8 | resolve_env 纯函数/命令层 home 注入；F10 |
| `src-tauri/src/plan_balance/query.rs` | 9 | URL 归一化/匹配查找/注册表序；F10 |
| `src-tauri/src/plan_balance/deepseek.rs` | 6 | 响应解析纯函数（罐装 JSON）；F10 |
| `src-tauri/src/plan_balance/kimi.rs` | 24 | 双窗解析（remaining 优先/detail 内层/真实响应快照锚点）/配额耗尽冻结/全有或全无；F10 |
| `src-tauri/tests/pty_integration_tests.rs` | 7 | 真实 ConPTY 往返/OSC cwd/resize/kill/自定义 spawn/隔离/env |

> `pty/mod.rs`、`pty/win_build.rs`、`main.rs` 不含 `#[test]`。git/mod.rs 测试已按 GIT-12 全量拆出至 `tests/`。env 测试依赖 `--test-threads=1`。

### 条件跳过用例（有效覆盖依赖 runner 环境）

以下用例依赖 Windows 应用执行别名或 symlink 创建权限，环境不满足时跳过但仍计「通过」：
- `pty/shell.rs::test_allowlist_accepts_real_alias_when_present`
- `hooks/signal.rs::process_symlink_signal_deletes_without_read`、`hooks/watcher.rs::collect_excludes_symlink_files`、`notify/mod.rs` symlink 两用例、`agent_history/claude/ops.rs` symlink 三用例（BE-17/D5 豁免先例）

本地开发机（已开开发者模式）为真实覆盖来源；CI runner 未开权限时上述分支覆盖记为「不确定」。

## L2 — 前端单元/集成测试（167 文件 / 2854 用例）

运行：`npm test`

### IPC 层（9 文件 / 187 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/ipc-contract.test.ts` | 80 | pty/fs/settings/projects/notify/git 全模块四维契约；IHE-06/DBG-4 |
| `src/__tests__/ipc-agent-hooks-contract.test.ts` | 21 | agent hooks 四命令/ContextUsage/AgentEventPayload/onAgentEvent；MC-212 |
| `src/__tests__/ipc-agent-history-contract.test.ts` | 18 | agent_history scan/delete/read_title 四维契约；F7/MC-306/BE-19/TQ-C-02 |
| `src/__tests__/ipc-window-contract.test.ts` | 9 | `registerCloseHandler` 生命周期；WRK-04/FE-26 |
| `src/__tests__/ipc-window.test.ts` | 6 | window 三 wrapper；TQ-COV-10 |
| `src/__tests__/ipc-ping.test.ts` | 2 | `ping()` wrapper |
| `src/__tests__/notification.test.ts` | 9 | toast 通知静默/权限；IHE-02 |
| `src/__tests__/app-error.test.ts` | 26 | `parseAppError`/`getErrorMessage` 全变体；FE-02/BE-15 |
| `src/__tests__/ipc-plan-balance-contract.test.ts` | 16 | planBalance 三命令四维契约（含 setPlanBalanceInterval payload 键集合）/onPlanBalanceUpdated 解包/DTO 键集合；F10/F11 |

### 终端面板（17 文件 / 282 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/use-xterm-lifecycle.test.ts` | 97 | xterm 生命周期/PTY/OSC/键盘/代理事件/标题/错误分支；TRM-01/F3/F9/HUK/ZQ/TQ-COV-07 |
| `src/__tests__/use-xterm-output.test.ts` | 38 | 输出合帧/直写阈值/交替缓冲/64KB 淘汰/dispose；BE-05/FE-18/TRM-04 |
| `src/__tests__/terminal-registry.test.ts` | 28 | TerminalRegistry CRUD/sessionChange/merge；NAH-02 |
| `src/__tests__/can-fit.test.ts` | 15 | `canFit` 纯函数边界 |
| `src/__tests__/use-xterm-integration.test.ts` | 12 | 真实 Terminal/FitAddon 轻集成 |
| `src/__tests__/keyboard.test.ts` | 12 | 终端快捷键派发；FE-08 |
| `src/__tests__/terminal-registry-subscribe.test.ts` | 7 | subscribe 通知/退订 |
| `src/__tests__/webgl-setup.test.ts` | 7 | WebGL 重试/回退；TRM-06 |
| `src/__tests__/terminal-instance.test.ts` | 12 | `useTerminalInstance` 分支/dispose；TQ-COV-07 |
| `src/__tests__/terminal.test.tsx` | 27 | TerminalPanel 渲染/标题/logo/customTitle/状态/Build 钳制；F8/F9/B12/B13/B14/IC-03/FE-17 |
| `src/__tests__/win-build-clamp.test.ts` | 4 | `clampWindowsBuildForXterm` 边界；ADR-0004 |
| `src/__tests__/e2e-gating-terminal.test.ts` | 5 | E2E helper 终端门控 |
| `src/__tests__/terminal-lifecycle.test.ts` | 4 | 挂载-创建-卸载-dispose 链路 |
| `src/__tests__/active-terminal.test.ts` | 4 | active 指针 |
| `src/__tests__/detect-webgl.test.ts` | 4 | WebGL2 检测；FE-26 |
| `src/__tests__/terminal-strictmode.test.ts` | 2 | `smGuardRef` 防双重挂载 |
| `src/__tests__/use-xterm-error-toast.test.ts` | 4 | 终端错误 toast；FE-08 |

### CLI profile 注册表（5 文件 / 95 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/cli-profile-registry.test.ts` | 19 | 注册表行为/首 token 匹配/logo 资源守卫；MC-101/102/108 |
| `src/__tests__/cli-profile-claude.test.ts` | 54 | claude 身份域/hooks 策略/history 策略；MC-104/214/315/316/422/KZ-1/KZ-4/AQ-1 |
| `src/__tests__/mock-cli-profile.test.tsx` | 13 | mock profile 全链路；AC-4/KZ-7/CS-3 |
| `src/__tests__/no-claude-literals.test.ts` | 6 | 通用层无 claude 字面量；AC-5 |
| `src/__tests__/emoji-scan.test.ts` | 3 | 装饰 emoji 字面量守卫；IC-09 |

### 编辑器面板（9 文件 / 137 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/use-code-mirror.test.ts` | 39 | CM6 扩展/handleSave/大文件；EDF-03/FE-44 |
| `src/__tests__/git-gutter.test.ts` | 32 | gutter 标记/dispatch wrapper；EDF-05 |
| `src/__tests__/language-mapping.test.ts` | 23 | 扩展名→语言映射 |
| `src/__tests__/editor-confirm.test.ts` | 11 | dirty/clean 外部修改确认 |
| `src/__tests__/editor.test.tsx` | 9 | 组件集成契约（DOC-02⑥） |
| `src/__tests__/editor-font.test.ts` | 8 | 字体 CSS 选择器 |
| `src/__tests__/editor-keyboard.test.ts` | 7 | 编辑器快捷键 |
| `src/__tests__/active-editor.test.ts` | 5 | active 指针 |
| `src/__tests__/use-code-mirror-reload-error.test.ts` | 3 | 外部重载失败提示；FE-10 |

### 工作区/布局/页签（17 文件 / 300 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/title-manager.test.ts` | 47 | terminal-N/标题冲突/suffix；B10 |
| `src/__tests__/layout-serde.test.ts` | 27 | 布局序列化/旧格式修补/白名单（含旧 hooksConfig 面板被过滤）；WRK-07/SC-FE-06 |
| `src/__tests__/panel-registry.test.ts` | 26 | 面板注册表/错误边界；FE-22/FE-35 |
| `src/__tests__/workspace-defaulttab.test.tsx` | 38 | 页签形态/状态圆点/logo/关闭 hover/× 关闭守卫；IC-03/TAB-01/TAB-02/F9/SC-FE-07 |
| `src/__tests__/workspace-page-dockview.test.tsx` | 33 | PageDockview/右键菜单/重命名/关闭/visible；B12/TAB-03/TAB-04/TE-06/TQ-COV-08 |
| `src/__tests__/pageapis.test.ts` | 20 | pageApis/会话反查/AbortSignal；FE-09/FE-26/BE-23 |
| `src/__tests__/workspace-header-actions.test.tsx` | 23 | 页签操作/重命名菜单；F8/MC-405/TAB-04/FE-04 |
| `src/__tests__/terminal-rename-dialog.test.tsx` | 14 | 重命名弹窗；F8/FE-13 |
| `src/__tests__/terminal-rename-apply.test.ts` | 5 | `applyRename` 纯函数；F8 |
| `src/__tests__/workspace-switch-order.test.tsx` | 19 | 项目切换/setProjectRoot 时序；DBG-5/BE-10/FE-38 |
| `src/__tests__/workspace-callback-cache.test.tsx` | 3 | pageCallbacksRef 惰性缓存；FE-33 |
| `src/__tests__/workspace-file-panel-types.test.ts` | 14 | FILE_PANEL_TYPES/isAlwaysRenderPanel |
| `src/__tests__/default-layout-format.test.ts` | 10 | makeEmptyLayout/NavTree 使用；WRK-11/NAV-06 |
| `src/__tests__/layout-switch.test.ts` | 7 | 页面切换集成 |
| `src/__tests__/workspace-multi-instance.test.tsx` | 6 | 多 Dockview 实例/H6；WRK-09 |
| `src/__tests__/workspace-e2e-ready.test.tsx` | 4 | E2E workspace ready 标记 |
| `src/__tests__/workspace.test.tsx` | 4 | Dockview 初始化/项目页面关联 |

### Store 状态管理（4 文件 / 99 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/projects.test.ts` | 55 | Project/Page CRUD/全局页数上限/corrupted/loadSucceeded 空写守卫；FE-01/FE-36/FE-11 |
| `src/__tests__/font-size.test.ts` | 21 | fontSize 加载/持久化/段形态；FE-09/FE-11/SVC-02 |
| `src/__tests__/keybindings.test.ts` | 19 | 快捷键绑定/持久化；FE-09/FE-11/SVC-02 |
| `src/__tests__/layout.test.ts` | 4 | activePageId 状态 |

### 资源管理器（24 文件 / 309 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/explorer-git-status.test.tsx` | 32 | git 状态着色；TH-10 |
| `src/__tests__/explorer-delete.test.tsx` | 26 | 删除确认/菜单/横幅；EXP-04/EXP-11/UI-802/TQ-B-08 |
| `src/__tests__/file-icon.test.tsx` | 44 | 文件图标六色盘/目录 git 着色；IC-04/FE-20 |
| `src/__tests__/explorer-file-viewer.test.tsx` | 21 | 打开文件面板分派；EXP-10 |
| `src/__tests__/explorer-refresh-preserve.test.tsx` | 23 | 增量刷新/去抖/子树范围；FE-15 |
| `src/__tests__/explorer-selection.test.tsx` | 18 | 选中模型/hover；EXP-04/FE-15 |
| `src/__tests__/explorer-keyboard.test.ts` | 15 | Explorer 快捷键 |
| `src/__tests__/use-file-tree.test.ts` | 16 | 文件树加载/展开/generation；FE-41 |
| `src/__tests__/explorer-root-contextmenu.test.tsx` | 14 | 根节点右键菜单 |
| `src/__tests__/explorer-sandbox-race.test.tsx` | 14 | setProjectRoot 竞态；DBG-10/TQ-B-03 |
| `src/__tests__/explorer-notify.test.tsx` | 6 | useFileTree 加载交互残余 |
| `src/__tests__/explorer-input-boundary.test.tsx` | 10 | 内联输入框边界；EXP-06 |
| `src/__tests__/explorer-rename-state.test.tsx` | 8 | 重命名状态上提 |
| `src/__tests__/explorer-open-in-terminal.test.tsx` | 7 | 在终端中打开；EXP-01 |
| `src/__tests__/explorer-race-cleanup.test.tsx` | 6 | useFileTree 竞态清理；EXP-07 |
| `src/__tests__/activeExplorer.test.ts` | 6 | active 指针 |
| `src/__tests__/explorer-rootpath-clear.test.tsx` | 6 | rootPath 变化清空 |
| `src/__tests__/explorer-focus.test.tsx` | 6 | 焦点管理；EXP-04 |
| `src/__tests__/explorer-rename-keyboard.test.tsx` | 5 | F2 重命名 |
| `src/__tests__/explorer-crud-success.test.tsx` | 4 | CRUD 成功路径；EXP-02 |
| `src/__tests__/explorer-error-placeholder.test.tsx` | 5 | 加载错误占位；FE-07 |
| `src/__tests__/dir-entry-null.test.tsx` | 3 | DirEntry null 适配；FE-12 |
| `src/__tests__/explorer-virtualization.test.tsx` | 8 | FileTree 虚拟化/滚动跟随；FE-30/FE-40/TQ-B-01 |
| `src/__tests__/explorer-keyboard-panel.test.tsx` | 6 | ExplorerPanel 键盘动作链路；TQ-COV-09 |

### 导航树（5 文件 / 105 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/nav-tree.test.tsx` | 45 | 统一导航树层级/选中/搜索/右键菜单/hover；NAV-01~04/09/FE-03/TQ-COV-09/10 |
| `src/__tests__/nav-history-row.test.tsx` | 16 | 历史行渲染/状态/logo/hover；FE-25/MC-311/UI-501 |
| `src/__tests__/nav-tree-history.test.tsx` | 11 | 历史折叠节点/归属/恢复；NAV-03/08/FE-16/FE-19 |
| `src/__tests__/plan-balance-model.test.ts` | 22 | 货币符号/logo 路径/重置时间格式化/行文案四场景/tooltip；F10 |
| `src/__tests__/plan-balance-footer.test.tsx` | 11 | footer 四场景渲染/隐藏态/初始拉取/事件订阅/点击节流/logo onError/tooltip；F10 |

### 侧栏视图（6 文件 / 156 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/sideBarState.test.ts` | 54 | 状态机纯函数/场景序列；SVC-06/SVC-13/NAV-05/07/FE-22 |
| `src/__tests__/activityBar.test.tsx` | 40 | 活动栏渲染/开关/拖拽/配置钮；SVC-01/SVC-05/SVC-14/NAV-05/IC-06/TE-05 |
| `src/__tests__/sideBar.test.ts` | 24 | store 持久化/迁移；SVC-02/NAV-07/FE-09/11 |
| `src/__tests__/sideBarArea.test.tsx` | 17 | 侧栏区布局/卸载/比例；SVC-07/FE-19/FE-21/TH-10 |
| `src/__tests__/workspace-sideviews.test.tsx` | 13 | Workspace 三栏集成；SVC-10/NAV-05 |
| `src/__tests__/sideViewRegistry.test.ts` | 8 | 侧栏视图注册表；NAV-05/TQ-COV-10 |

### Commit 视图（6 文件 / 64 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/commit-open-file.test.ts` | 17 | 双击分派/去重；SVC-04/11/B10 |
| `src/__tests__/commit-context-menu.test.ts` | 17 | 右键菜单/action；SVC-09/UI-802 |
| `src/__tests__/commit-view-list.test.tsx` | 9 | 列表渲染/去抖/空态；SVC-03/SVC-12 |
| `src/__tests__/commit-context-menu-ui.test.tsx` | 10 | 菜单 UI/危险项；UI-802 |
| `src/__tests__/commit-view-status.test.ts` | 7 | 状态机四态 |
| `src/__tests__/commit-view.test.tsx` | 4 | 主干渲染/提示色 |

### hooks 配置编辑器（11 文件 / 228 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/hooks-config-handlerform.test.tsx` | 40 | HandlerForm 字段矩阵/切换/编辑；HKC-04 |
| `src/__tests__/hooks-config-gui.test.tsx` | 28 | Master-Detail 渲染/增删；HKC-05 |
| `src/__tests__/settings-hooks-page.test.tsx` | 38 | 设置中心内 Hooks 页三态/CLI 选择行/分派/持久化/点击已选中 CLI 短路（SettingsPageProps 形态）；MC-502~507/KZ-1/KZ-4/MC-503/HKC-03/07/09/10/OV-02/SC-FE-05 |
| `src/__tests__/hooks-config-jsonmode.test.tsx` | 18 | JSON 编辑器/schema/校验；HKC-01 |
| `src/__tests__/hooks-config-matcher.test.ts` | 21 | `matchHook` 全分支 |
| `src/__tests__/hooks-config-catalog.test.ts` | 19 | eventsCatalog 元数据 |
| `src/__tests__/hooks-config-model.test.ts` | 17 | jsonToGui/guiToJson 双向转换 |
| `src/__tests__/ipc-hooks-config-contract.test.ts` | 12 | read/writeHooksConfig 四维契约；IHE-06/MC-212 |
| `src/__tests__/hooks-config-schema.test.ts` | 10 | `validateHooksJson` 边界；HKC-08 |
| `src/__tests__/hooks-config-sync.test.tsx` | 15 | 双模式同步/user 层二次确认；MC-220/KZ-4/FE-25/SEC-05 |
| `src/__tests__/statusline-bridge-behavior.test.ts` | 10 | statusline 桥接脚本行为；B11/B16 |

### 设置中心（10 文件 / 91 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/settings-page-registry.test.ts` | 7 | 注册表注册/getAll 分组过滤/order 排序/重复 id 覆盖/_reset；SC-FE-01 |
| `src/__tests__/settings-pages-registration.test.ts` | 1 | 真实 pages 注册/getAll 精确三条断言（不 mock pages）；SC-FE-01/TE-04 |
| `src/__tests__/open-settings.test.ts` | 6 | 无项目 toast 且不切页/活跃项目优先/兜底第一个项目/切页先于开面板；SC-FE-02 |
| `src/__tests__/open-settings-panel.test.ts` | 7 | addPanel 参数精确/单例 focus 不新建/深链 selectedPage/5s 超时降级；SC-FE-02 |
| `src/__tests__/settings-panel.test.tsx` | 14 | 导航组序/选中渲染/切换 persist/corrupted 警示条/pageParams 透传/saveLayout 落盘/不可变合并/空态；SC-FE-03/TE-06 |
| `src/__tests__/settings-plan-balance.test.tsx` | 16 | 频率页显示回退/合法提交调命令+refresh/非法行内红字/Err toast+保留输入；SC-FE-04 |
| `src/__tests__/settings-dirty-registry.test.ts` | 5 | set/is/clear 真值源；SC-FE-07 |
| `src/__tests__/settings-panel-dirty.test.tsx` | 6 | 切页 confirm 确认/取消/非 dirty 直切/圆点显隐；SC-FE-07 |
| `src/__tests__/settings-keybindings.test.tsx` | 20 | 分组渲染/override 高亮/未绑定占位/录制 Esc/Backspace 解绑/保留键拒绝/冲突放行/卸载清 suspended；SC-FE-09 |
| `src/__tests__/settings-panel-autoclose.test.tsx` | 9 | 切项目自动关闭/同项目不关/未水合首轮不消费 firstRun/初始不一致静默关/activePageId null 不关/dirty confirm 取消不关；SC-FE-08 |

### Diff/GitShow 面板（4 文件 / 87 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/diff-panel.test.tsx` | 40 | diff 双栏/保存刷新/脏确认；EDF-01/02/07/FE-02/43 |
| `src/__tests__/gitshow-panel.test.tsx` | 25 | gitshow 三态/大文件/只读；EDF-04/09/FE-18/TQ-COV-10 |
| `src/__tests__/diff-alignment.test.ts` | 18 | `computeAlignment` 纯函数；EDF-06 |
| `src/__tests__/diff-panel-stale-banner.test.tsx` | 4 | 内容过时提示条；FE-10 |

### 快捷键/命令系统（7 文件 / 131 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/shortcuts.test.ts` | 56 | 注册/注销/上下文/重绑/IME/录制态屏蔽；STS-03/SC-FE-09 |
| `src/__tests__/keystroke.test.ts` | 26 | keystroke 格式化与解析 |
| `src/__tests__/global-commands.test.ts` | 13 | 全局快捷键命令；STS-02 |
| `src/__tests__/command-catalog.test.ts` | 18 | 命令目录/参数化遍历；STS-08/SEC-04 |
| `src/__tests__/reserved.test.ts` | 9 | 保留键判定 |
| `src/__tests__/use-panel-focus.test.ts` | 5 | focus 上下文栈 |
| `src/__tests__/wire-keybindings.test.ts` | 4 | store 与 keybindings 集成；TQ-B-16 |

### 主题/配色/基础（8 文件 / 202 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/colors.test.ts` | 96 | token 真实值/linear 化/ROOT_CSS_VARS；STS-01/09/TH-01/10/FE-08 |
| `src/__tests__/scheme-registry.test.ts` | 18 | 方案注册表/linear 四段完整性；TST-02/TH-03/04 |
| `src/__tests__/overrides.test.ts` | 11 | overrides/CM6 扩展/层叠；TST-03/TH-07/TQ-C-01/ACC-05 |
| `src/__tests__/agent-status-lib.test.ts` | 3 | 四态常量/emoji 退役；IC-03/MC-401 |
| `src/__tests__/path.test.ts` | 27 | path 工具函数边界 |
| `src/__tests__/inject-script.test.ts` | 21 | HTML 脚本注入/拦截；STS-11 |
| `src/__tests__/theme.test.ts` | 13 | terminalOptions/ANSI/kitty；STS-05/TH-11 |
| `src/__tests__/panelId.test.ts` | 13 | terminal panelId 生成/解析；B14 |

### 通知/Agent 状态（2 文件 / 98 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/agent-status-hook.test.ts` | 49 | useAgentStatus 行建模/ContextUsage/标题订阅；F5/MC-205/313/NAH-01/08/ZQ-2/3 |
| `src/__tests__/notifications.test.ts` | 49 | 去重缓存/通知调度/窗口失焦；NAH-04/MC-420/ZQ-2 |

### Agent 历史会话（4 文件 / 83 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/agent-history-model.test.ts` | 47 | 纯函数/复合键/keyOf；MC-313/ZQ-1/7/NAH-01 |
| `src/__tests__/agent-history-hook.test.tsx` | 15 | useAgentHistory 状态机/scan generation；MC-313/NAH-08 |
| `src/__tests__/agent-history-restore.test.ts` | 13 | 四步恢复/防重入/abort；F7/NAH-07/ZQ-4/MC-315/FE-27/48 |
| `src/__tests__/agent-history-action-dialog.test.tsx` | 8 | SessionActionDialog；NAH-11/FE-12 |

### 启动/关闭（5 文件 / 40 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/close-handler.test.ts` | 19 | 关闭序列/ptyKillAll 兜底；WRK-08/BE-08/FE-47 |
| `src/__tests__/startup-restore.test.ts` | 13 | 启动恢复/加载失败错误页/重试/空状态继续/错误页标题栏（复用 TitleBar，三钮渲染+关闭调 closeWindow）/setProjectRoot 时序；WRK-03/DBG-6/FE-02/FE-06/10 |
| `src/__tests__/bootstrap.test.ts` | 5 | `__TAURI_INTERNALS__` 轮询；FE-03 |
| `src/__tests__/main-bootstrap.test.tsx` | 1 | main.tsx init 失败；WRK-10 |
| `src/__tests__/startup-store-fail-warn.test.tsx` | 2 | store 加载失败可感知；FE-03/20 |

### 文件查看器/HTML（4 文件 / 91 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/html-panel.test.tsx` | 49 | sandbox/postMessage/注入脚本/nonce；IHE-03/07/08/SEC-04 |
| `src/__tests__/clipboard-guard.test.ts` | 4 | 剪贴板读权限路径守卫；SEC-06 |
| `src/__tests__/file-viewer-registry.test.ts` | 31 | 扩展名策略链；EXP-12 |
| `src/__tests__/csp-config.test.ts` | 7 | tauri.conf.json CSP 不变量；IHE-07④ |

### E2E 辅助/门控测试（7 文件 / 38 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/e2e-build-config.test.ts` | 8 | VITE_E2E 配置/E2E_ENABLED 字面量；IHE-04 |
| `src/__tests__/e2e-enabled.test.ts` | 9 | E2E_ENABLED 真值表 |
| `src/__tests__/app.test.tsx` | 7 | E2E helper 契约（含设置中心 helper 存在性）；DOC-02⑤/SC-E2E-01 |
| `src/__tests__/error-boundary.test.tsx` | 6 | ErrorBoundary inline 重试；IHE-05/FE-46 |
| `src/__tests__/e2e-clipboard-helper.test.ts` | 3 | helper 函数可用性 |
| `src/__tests__/e2e-create-project.test.ts` | 3 | E2E helper 契约；DOC-02⑤ |
| `src/__tests__/e2e-gating-workspace.test.tsx` | 2 | workspaceReady 标记 |

### 标题栏/统一浮层（3 文件 / 30 用例）

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `src/__tests__/title-bar.test.tsx` | 11 | 自绘标题栏/拖拽区/窄订阅；TB-06/FE-21 |
| `src/__tests__/confirm-dialog.test.tsx` | 14 | 统一确认弹窗/焦点陷阱/视觉规格；OV-01/UI-801/FE-13/14 |
| `src/__tests__/toast.test.tsx` | 5 | toast 通知/自动消失；OV-01/UI-804 |

## L3 — 终端 headless 测试（8 文件 / 142 用例）

运行：`npm run test:l3`（Vitest + `@xterm/headless`，`environment: 'node'`）

> **定位声明（DOC-02①）**：L3 = **网格状态正确性，非渲染正确性**；渲染正确性由 L4 视觉回归（M2 人工确认）兜底。  
> **职责边界（TQ-E-10）**：L3 全部用例运行于 node + @xterm/headless，无后端 PTY/ConPTY/shell 集成。生产关键路径（reader 微批、ConPTY flags、OSC 注入、SPAWN_LOCK、路径沙箱）归属 L1/L4。

| 文件 | 用例 | 覆盖要点 |
|------|------|---------|
| `test/terminal/terminal-serialize.test.ts` | 41 | 文本序列化/颜色/光标/scrollback/resize/擦除 |
| `test/terminal/keyboard.test.ts` | 36 | xterm.js 基础按键行为回归（E2E-01） |
| `test/terminal/ansi-correctness.test.ts` | 30 | ANSI 16/256/TrueColor/SGR/DEC 模式 |
| `test/terminal/osc.test.ts` | 9 | OSC 标题/调色板 |
| `test/terminal/theme-options.test.ts` | 5 | 生产 terminalOptions 与主题一致（E2E-02） |
| `test/terminal/production-osc.test.ts` | 8 | 生产 OSC 52/133/8 handler（E2E-03/TQ-E-01） |
| `test/terminal/shortcut-dispatch.test.ts` | 4 | 生产按键分发链路（TQ-E-02） |
| `test/terminal/negative-ansi.test.ts` | 9 | 反向/异常 ANSI（E2E-14） |

## L4 — E2E 端到端测试（10 spec / 51 用例，49 active + 2 skip）

运行：`npm run e2e`（= `npm run build:e2e` + `npm run wdio`）  
技术栈：WDIO + `@wdio/tauri-service` 1.1.0 + embedded driver；specs 通配 `./*.e2e.ts`，单 worker 顺序执行。

> **定位声明（DOC-02②）**：键盘/拖拽/恢复编排三类用例为**半端到端**——应用内监听/匹配/命令 handler/真实 IPC 全链路在真实二进制执行，唯一不真实处是事件来源/前置动作。

| spec | 用例 | active/skip | 覆盖要点 |
|------|------|-------------|---------|
| `terminal.e2e.ts` | 7 | 7 active | 启动标题/PTY 通信/H6 跨页面存活/全屏 TUI 视觉回归/强杀无残留 |
| `editor.e2e.ts` | 5 | 5 active | 编辑器标题/同名冲突/Ctrl+S 真实写盘/外部修改 reload |
| `history.e2e.ts` | 7 | 7 active | 导航树历史/标题回退/搜索/复制恢复/恢复编排/删除 |
| `agent.e2e.ts` | 7 | 6 active + 1 skip | NAV-10 导航树视图/动态四态/R2-R4 变体/toast（skip） |
| `hooks.e2e.ts` | 5 | 5 active | 注入/卸载/状态/真实 reporter 链路/hub 选择行 |
| `html.e2e.ts` | 2 | 1 active + 1 skip | postMessage 转发/CSP（skip） |
| `sidebar.e2e.ts` | 2 | 2 active | 侧栏视图开关/跨区状态机 |
| `commit.e2e.ts` | 2 | 2 active | 变更列表/双击 modified 打开 diff |
| `mockcli.e2e.ts` | 3 | 3 active | mock profile 冒烟/CS-3 agent-event/hub 分派 |
| `settings.e2e.ts` | 11 | 11 active | 配置钮打开/单例/切页持久化/频率页真实后端落盘/非法不落盘/录制落盘/切项目自动关闭/同项目保留/hooks 页冒烟/dirty 切页守卫/× 关闭 dirty 守卫 |

### 用户目录隔离机制（FIX-TE-04 + E2E-05 扩展）

`run-wdio.cjs` 启动时备份 `~/.slterminal/settings.json`、`~/.claude/settings.json`、`~/.slterminal/hooks/`（`.e2e-bak`），exit 时还原；`~/.slterminal/hooks-events/` 直接清理。E2E 不触碰真实 `~/.claude/projects/`（`SLTERM_CLAUDE_PROJECTS_DIR` 指向临时副本）。详见 `e2e-tests/CLAUDE.md`。

### E2E 键盘输入限制（半端到端，TE-17）

embedded WDIO 无法投递 OS 级按键；所有键盘用例改用页面内 dispatch 合成事件 → ShortcutRegistry → 命令 handler → 真实 IPC。唯一不真实处是事件来源。

## 静态检查门禁

| 门 | 命令 |
|----|------|
| TypeScript | `npx tsc --noEmit` |
| ESLint | `npx eslint src/` |
| Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` |
| rustfmt | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` |

## 版本校准

- 2026-08-23：全量 3634（Rust 742 + 前端 2710 + L3 142 + E2E 40）。历史变更日志已移除，旧版本变更详见 git log。
- 2026-08-28：全量 3731（Rust 794 + 前端 2755 + L3 142 + E2E 40）。F10 新增 plan_balance 五文件 51 例 + settings.rs +1、前端三文件 45 例。
- 2026-08-29：全量 **3746**（Rust 809 + 前端 2755 + L3 142 + E2E 40）。B17 新增 `test_pwsh_args_no_noprofile_b17` 1 例（shell.rs 32→33），kimi.rs 解析实测修正（+14）已于同日校准。
- 2026-08-30：全量 **3846**（Rust 815 + 前端 2839 + L3 142 + E2E 50）。F11 设置中心：plan_balance/mod.rs +6（set_interval 四维/内存默认/键名常量）；L2 +9 文件 -2 文件净 +7 文件 +84（设置中心 9 文件 88 例，settings-hooks-page 替代 hooks-config-panel 37，open-settings 两文件取代 open-hooks-config 两文件，ipc-plan-balance-contract +4、workspace-defaulttab +4、shortcuts +2、layout-serde +1、app +2）；L4 +settings.e2e.ts 10 例。
- 2026-08-30（实跑确认）：全量 **3865**（Rust 818 + 前端 2854 + L3 142 + E2E 51）。settings-center-fixes 修复链实跑增量：L1 +3（BE-01 app_dir 三例）；L2 +15（FE-01 projects +5、FE-02 startup-restore 净 +3、FE-03 autoclose +1、TE-04 settings-pages-registration 新文件 +1、TE-05 hooks-page +1、TE-06 settings-panel +1、FE-04 适配 0、错误页标题栏 startup-restore +2）；L4 +1（TE-03 settings.e2e.ts 用例⑪ × 关闭 dirty 守卫）。
