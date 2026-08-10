# 自动化测试用例清单

> **本文档是项目用例数唯一真值源。** 所有 CLAUDE.md、README、CI 配置中引用的用例数均以此文件为准。更新测试后必须同步本文档。

全量 **3088** 用例（Rust 584 + 前端 2329 + L3 138 + E2E 37），2026-08-10 更新。

> **计数口径**：
> - L2 以 `npm test` 实跑（Vitest 报告）为准——`it.each(...)` 参数化与 `describeIpcContract` 工厂（`helpers/ipc-contract.ts`，IHE-06）等按**展开后用例数**计入（138 文件 2333 用例全绿实测）；纯 grep `it(/test(` 块数会少计 it.each 展开（如 colors 85 = 13 块 + 7 组 each 展开 72）。
> - L1 以 `grep -c '#\[test\]'` 统计的 `#[test]` 属性数为准（29 文件 584）。
> - L3 以 `npm run test:l3` 实跑为准（7 文件 138，`test/terminal/**/*.test.ts`）。
> - L4 以 spec 内 `it(`/`it.skip(` 计数为准（8 spec，35 active + 2 skip）。
> - L2 与 L3 **独立运行**：`vitest.config.ts` include 仅 `src/__tests__/**`，L3 走 `vitest.l3.config.ts`（environment: node）。旧注释"L3 同时被 npm test 包含执行"已废弃。

## 既定豁免清单（DOC-01）

> 按 D6 分类处理后残余不可自动化项 + 各 Stage 产出收编（原 review 报告 00-summary 5.3 表与 checklist DOC-01 已随 `docs/` 清理，本表为唯一真值源）。对应模块 CLAUDE.md（pty/e2e-tests）保留逐项明细，三列（项目/豁免原因/当前兜底层级）以本表为唯一真值源。

| 项目 | 豁免原因 | 当前兜底层级 | 来源 |
|------|----------|--------------|------|
| `reader_loop` 残余 I/O 编排分支（channel 锁/send 失败/EOF `child.wait()`/DA1 注入动作/ring buffer 写入/日志告警/读错误常量） | 依赖 Mutex/RwLock/Channel/系统调用无法纯函数化（决策点已抽 `apply_startup_strip`/`should_inject_da1`/`eof_exit_code` 补测；逐分支明细见 `src-tauri/src/pty/CLAUDE.md`「reader_loop I/O 编排残余豁免」） | `pty_integration_tests`（真实 ConPTY 往返 8 条）+ L4 PTY 通信/强杀残留用例 | PTY-12 |
| `spawn_conpty_child` 纯 Win32 调用部分（AttrList set_pty → CreateProcessW） | Win32 API 组合，参数错误无单测定位价值；可纯化部分（命令行/环境块构造）已抽 `build_cmdline`/`build_env_block` 补测 | `pty_spawn_custom_conpty` 集成测试 + Windows CI runner | PTY-08 |
| `lib.rs` `run()` | Tauri 运行时胶水，L1 无法直接启动完整应用 | L4 `terminal.e2e.ts` 启动标题等用例 | SPE-06② |
| ActivityBar 拖拽 mock 理想化（`getBoundingClientRect` mock + 合成 DragEvent） | jsdom 无法模拟真实 DnD hit-test 与布局矩形 | `activityBar.test.tsx` L2 拖拽全链路（含 drop index 断言）+ L4 `sidebar.e2e.ts` 跨区状态机（经 store helper 走真实二进制） | SVC-14 |
| `E2E_ENABLED=false` 生产分支 | L2 恒 true，编译期字面量 DCE 结构性缺口 | `e2e-build-config.test.ts` 字面量表达式断言（IHE-04）+ CI 生产 dist grep 守卫 | IHE-04 |
| L3 生产 WebGL renderer / mouse tracking | headless 不跑 GPU；PASSTHROUGH_MODE 滚轮回归无法自动化（行为级测试假阴性） | L4 全屏 TUI 视觉回归（M2 人工确认）+ `compute_conpty_flags` 4 条守卫锁 0x7 + 文档红线 | 15-#16 |
| L4 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys` 到 WebView2 页面 | 合成事件 + 页面内 dispatch 全链路（监听/匹配/命令 handler/写盘真实执行）；未来 WDIO 支持真实输入时替换 | 13 P-15 |
| HTML postMessage 真实 WebView2 行为（opaque origin 序列化 / CSP 强制） | jsdom 无法模拟 opaque origin 与 WebView2 CSP；`e.origin === "null"` 为 WHATWG 规范推断 | L4 `html.e2e.ts` Ctrl+W postMessage 往返（真实二进制）+ L2 四负面用例（IHE-03） | 13 P-5 |

> 原豁免表中 `FileWatcher::start`/`notify_watch` 与 `claude_history` 命令包装两项已按 D6 **从豁免重分类为补测**（Stage 05：notify 抽 `EventEmitter` trait 注入 mock emitter 驱动事件循环；history 补命令包装层 `block_on` 用例），不再列入豁免表。

## 定位声明（DOC-02）

| # | 层级 | 定位声明 | 来源 |
|---|------|----------|------|
| ① | L3 | **网格状态正确性，非渲染正确性**——headless 不跑 WebGL/GPU/onContextLoss，"渲染正确性"代表性有限；生产 `terminalOptions`/OSC handler 的验证限于 headless 网格可观察语义（`theme-options.test.ts` 文件头已标注） | E2E-04 |
| ② | L4 | **半端到端/部分端到端**——键盘（合成 keydown 非 OS 按键）、拖拽（store helper 非真实 DnD）、恢复编排（断言到 `pty.write` 注入，不含真实进入会话）三类用例的应用内全链路真实执行，唯一"不真实"处是事件来源/前置动作（详见 `e2e-tests/CLAUDE.md`「定位声明」段） | E2E-11 |
| ③ | L2 | **jsdom 模拟**——postMessage origin/source 校验在 jsdom 无法代表真实 WebView2（opaque origin 不可模拟），负面用例守护 JS 侧逻辑，真实行为由 L4 验收（`html-panel.test.tsx` 用例已标注） | IHE-03 |
| ④ | L2 | **term.input 间接验证**——`term.input(...)` → onData 断言是 xterm.js 内部转换的间接验证，非"用户按键 → PTY write"直接链路；L3 keyboard 已降级标注"xterm.js 基础行为回归"，键盘链路由 L2 `attachCustomKeyEventHandler` 委托用例 + L4 半端到端兜底 | 06 #14 |
| ⑤ | L2 | **E2E helper 行为契约**——`app.test.tsx`/`e2e-create-project.test.ts` 验证的是 `__slterm_e2e_createProject` 等 helper 的契约（pending 标记/localStorage 交互），非真实 App 初始化逻辑；真实挂载路径由 `e2e-gating-*` 测试 + L4 使用实证 | 13 P-14 |
| ⑥ | L2 | **浅层组件定位**——`editor.test.tsx` mock `useCodeMirror` 只验证 prop 透传与容器样式，定位为组件集成契约测试（非行为测试），真实编辑器行为由 `use-code-mirror.test.ts` 等覆盖 | 07 G1/G2 |

## L1 — Rust 单元/集成测试（30 文件 / 591 用例）

运行：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src-tauri/tests/git_status_tests.rs` | 41 | git_status 命令层（block_on 真实命令：happy/沙箱拒绝/错误契约）+ 文件状态/绝对路径/ignore 语义/oldPath + 状态着色 status_to_str |
| `src-tauri/tests/git_diff_tests.rs` | 32 | git_diff 命令层 + compute_diff_hunks 边界（修改后多余新增行/prev_was_del flush/非 UnbornBranch 错误）+ hunk 行回调精确验证 + serde camelCase |
| `src-tauri/tests/git_rollback_tests.rs` | 10 | git_rollback 命令层（rollback 后 status 干净/沙箱拒绝/错误契约） |
| `src-tauri/tests/git_file_at_head_tests.rs` | 8 | git_file_at_head 命令层（HEAD 内容读取/UnbornBranch 错误消息"HEAD 中不存在"/已删除文件/沙箱拒绝） |
| `src-tauri/tests/git_unstage_tests.rs` | 6 | git_unstage 命令层（added 文件取消暂存/沙箱拒绝） |
| `src-tauri/tests/ci_config_tests.rs` | 1 | `ci_l1_uses_single_test_thread`（GIT-11 领域污染迁移） |
| `src-tauri/src/pty/reader.rs` | 36 | ConPTY 启动序列剥离（含 OSC 1/3/4/9 保留/CSI 3J/平台守卫）/DA1 查询检测/apply_startup_strip/should_inject_da1/mirror_da1_query/eof_exit_code/16KB 边界 |
| `src-tauri/src/pty/spawn.rs` | 48 | compute_conpty_flags（4 条锁 0x7）/flag 常量/ConPtyMaster MasterPty trait/AttrList 生命周期/create_conpty_pair/build_cmdline 引号/build_env_block/**validate_spawn_request（尺寸/白名单/cwd 三拒绝）**/**validate_session_ownership（SEC-08 放行/拒绝）**/Job Object 纯逻辑（job_name/limit flags）/测试清理 helper |
| `src-tauri/src/pty/shell.rs` | 20 | pwsh 发现 + 三档回退顺序（可控 PATH）/shell-integration.ps1 嵌入/UTF-16LE Base64 往返/which_full_path PATH 顺序/白名单解析后仍非法拒绝 |
| `src-tauri/src/state.rs` | 32 | ring buffer append+eviction+无换行长行淘汰三边界/validate_path_within_root 沙箱（含 `..` 穿越拒绝）/canonicalize_or_ancestor |
| `src-tauri/src/fs/mod.rs` | 31 | read_dir/write_file（真实命令，CRLF 保留）/create_dir/delete/rename + 命令包装单测 + 异常路径（删除不存在/root 外拒绝/TaskJoin panic 映射） |
| `src-tauri/src/notify/mod.rs` | 38 | FileWatcher 生命周期 + classify_by_kind 事件分类（全 7 种 EventKind）+ **EventEmitter trait 注入驱动事件循环（HFN-03/D6）** + Drop 轮询等待（HFN-07） |
| `src-tauri/src/notify/pool.rs` | 13 | LruWatcherPool: 缓存命中/LRU 淘汰/pause_all_except/replace（同 path 二次 insert stop 旧 watcher）/remove/stop_all/Drop |
| `src-tauri/src/hooks/mod.rs` | 19 | AgentInjectionStatus/AgentHookInjectionStatus serde 往返精确断言 + parse_signal_file 快速冒烟 + start_signal_watcher 幂等（#[cfg(test)] 重置钩子） + **命令层泛化（6 命令 cliId 透传 block_on 直测，MC-211）** |
| `src-tauri/src/hooks/signal.rs` | 16 | parse_signal_file 全分支 + camelCase 往返 + **process_signal_file 全流程（emit 注入参数：读→emit→删/emit 失败仍删/非法 JSON 降级，HUK-01）** + **AgentEventPayload 九键 serde（含无 cliId 旧信号反序列化兼容，MC-201）** + agent-event 广播（MC-202） |
| `src-tauri/src/hooks/watcher.rs` | 20 | is_signal_file/collect_signal_files/poll_once（含目录删除重建恢复/幂等）/**run_one_tick 或临时目录真实启动（轮询补漏消费残留，HUK-03）**/生命周期（stop 幂等 + thread.is_finished 断言）——MC-203 核对零改动 |
| `src-tauri/src/hooks/provider.rs` | 3 | CliHooksProvider trait + cliId 键注册表：resolve_provider 命中（身份断言）/未知 cliId Validation（MC-211）/已注册无 hooks 能力 Validation（「不支持 hooks 能力」语义）+ 未注册未知分支（MC-210 新建） |
| `src-tauri/src/hooks/claude/inject.rs` | 35 | 注入幂等（空 settings/已有用户 hooks/已注入升级）/卸载 handler 级剔除（混组保用户 handler/全 slterm 组删除/无 slterm 零写盘）/状态检测三态/非法 JSON 中止/版本比对/**注入/卸载/状态三命令 impl 路径 tempdir 驱动（HUK-02）**/handler_contains_slterm 非字符串分支 + **reporter 模板内嵌校验断言（显式 cliId + SCRIPT_VERSION 递增，MC-215 决策 7）** |
| `src-tauri/src/hooks/claude/usage.rs` | 26 | parse_usage_line 全分支 + scan_transcript_usage 集成（逆行命中/回溯/损坏跳过）+ ContextUsage serde 往返 + TRANSCRIPT_TAIL_BYTES + **命令包装层（HUK-05）+ 端到端五用例（P2-TE-05）**——下沉 claude/ 用例数不变（MC-213） |
| `src-tauri/src/hooks/claude/config.rs` | 27 | parse_layer/resolve_config_path（home 注入 tempdir，HUK-07）/read_hooks_subtree/write_hooks_subtree（原子写/merge 保留/损坏拒绝）+ IO 异常分支（persist 失败，HUK-06）——下沉 claude/ 用例数不变（MC-213） |
| `src-tauri/src/settings.rs` | 25 | 读写往返/文件不存在/JSON 损坏回退 .bak/浅合并/并发写/只读文件 + **block_on 真实 save_settings/load_settings 命令（SPE-01）+ app_data_dir 注入 + persist 失败映射（SPE-05）** |
| `src-tauri/src/projects.rs` | 17 | 序列化往返/ID 生成/路径校验 + **block_on 真实 save_projects/load_projects（SPE-02）+ persist 失败映射** |
| `src-tauri/src/error.rs` | 7 | 序列化/Display/From\<io::Error\>/SessionNotFound + **serde_json/git2/JoinError 三 From 转换（SPE-03）** |
| `src-tauri/src/lib.rs` | 2 | ping 返回 pong/`get_windows_build_number` 返回数字 |
| `src-tauri/src/agent_history/claude/jsonl.rs` | 28 | parse_head（cwd 收集/首条可见 prompt 跳过 4 类/EOF 截断/200 字符截断/标题 last-wins）+ 大文件头尾窗口协同 + parse_tail_title（custom 恒优先/ai 兜底）+ resolve_title 回退链 5 态 + tail 优先（MC-301 下沉 claude/，用例数不变） |
| `src-tauri/src/agent_history/claude/scan.rs` | 16 | resolve_projects_root（env 覆盖/默认）+ 排除 3 类 + 多目录收集 + 扫描根缺失空数组 + 降级条目 + 完整字段回退 + cwdExists + env 端到端 + mtime + ScanRootGuard RAII（HFN-06）（MC-301 下沉 + env 覆盖留 provider 内部，MC-305；命令包装层已迁 mod.rs） |
| `src-tauri/src/agent_history/claude/ops.rs` | 7 | validate_session_id（UUID 双形态/5 类非法拒绝——空串断言错误消息含具体文案）+ delete（jsonl+目录范围/仅 jsonl/不存在 Err）+ 越界防护（MC-301 下沉 + SEC-05 等价保留，MC-304；命令包装层已迁 mod.rs） |
| `src-tauri/src/agent_history/mod.rs` | 13 | AgentHistorySession serde camelCase 八键集合精确匹配（含 cliId 打标）+ 反序列化 + roundtrip + titleSource 开放字符串（claude 值集不变）+ is_uuid_filename（MC-302 更名）+ 聚合 scan 遍历全部 provider（单 provider 失败不阻塞/全部空 → 空数组）+ delete validate_session_id 强制前置 + 未知 cliId Validation（MC-303）+ **命令包装层 4 用例（command_scan_wraps/command_scan_degraded/command_delete_wraps/command_delete_invalid，迁移自 scan.rs/ops.rs——mod.rs:407-464「命令包装层（迁移自 scan.rs/ops.rs）」）** |
| `src-tauri/src/agent_history/claude/mod.rs` | 4 | TitleSource serde camelCase 往返 + title_source as_str 值集映射 + ScanRootGuard env 恢复（MC-302 下沉 claude/，值集不变） |
| `src-tauri/src/agent_history/provider.rs` | 2 | CliHistoryProvider trait 三方法（scan/delete/validate_session_id）+ cliId 键注册表 + resolve_provider 命中（身份断言）/未知 cliId Validation（MC-303/304 新建） |
| `src-tauri/tests/pty_integration_tests.rs` | 8 | PTY 往返/OSC cwd 解析/resize 生效/kill 无孤儿/Custom ConPTY spawn/reattach/env 注入 |

> ① 占位符已实落消除：provider.rs 实落 2 条（resolve_provider 命中/未知 cliId Validation），claude/mod.rs 实落 4 条（TitleSource serde ×2 + as_str 映射 + ScanRootGuard env 恢复）——L1 总数按实落对齐（584 + 净增 7 = 591）。

> `pty/mod.rs`、`pty/win_build.rs`、`main.rs` 不含 `#[test]`，不在此列。git/mod.rs 测试已按 GIT-12 全量拆出至 `tests/`（`#[test]` 零残留）。agent_history 模块 grep 口径：claude/jsonl 28 + claude/scan 16 + claude/ops 7 + mod 13 = 64（命令包装层 4 用例已迁入 mod.rs:407-464，MC-301 下沉时随行）+ claude/mod 4 + provider 2 = 全模块 70；env 测试依赖 L1 `--test-threads=1` 门禁（`std::env::set_var` 全局可变）。

## L2 — 前端单元/集成测试（137 文件 / 2329 用例）

运行：`npm test`（Vitest + jsdom，实跑全绿 2328）

### IPC 层（6 文件 / 116 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/ipc-contract.test.ts` | 65 | pty/fs/settings/projects/notify/git 全模块四维验证（IHE-06 工厂化：8 裸 it + 57 cases）+ DBG-4 PTY payload 契约守卫 + onFsEvent listen 回调解包行为契约（IHE-01②） |
| `src/__tests__/ipc-agent-hooks-contract.test.ts` | 22 | agent hooks 四命令四维验证（命令名 agent_hooks_*/agent_context_usage + 参数含 cliId camelCase + 返回 + 异常）+ ContextUsage 键集合守卫 + AgentEventPayload 8 字段契约 + onAgentEvent 解包（6 裸 it + 16 cases，MC-212 更名同步） |
| `src/__tests__/ipc-agent-history-contract.test.ts` | 8 | agent_history_scan（无参聚合）/agent_history_delete（{cliId, sessionId} 双参 camelCase）两命令四维验证（F7，rename 已移除；MC-306 更名同步） |
| `src/__tests__/ipc-window-contract.test.ts` | 10 | `registerCloseHandler` 关闭生命周期契约（WRK-04 处置：保留 + 契约测试） |
| `src/__tests__/ipc-ping.test.ts` | 2 | `ping()` wrapper 调用（IHE-07① 改调导出函数） |
| `src/__tests__/notification.test.ts` | 9 | `sendToastNotification` catch 静默/`ensureNotificationPermission` 拒绝路径（IHE-02 新建） |

### 终端面板（15 文件 / 214 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-xterm-lifecycle.test.ts` | 71 | PTY spawn/exit/setupRetry/快捷键/rAF 轮询/ResizeObserver/字体/OSC 52/133/8/键盘委托/hook-event 过滤与状态更新（F3 四态，问题 2 同源）/setAgentSession 写入（TRM-01 去重后归位）/OSC 133 C 携 CLI logo（命中/未注册 null——经 matchByCommand 查 profile.iconSrc，OSC133-2b） |
| `src/__tests__/use-xterm-output.test.ts` | 35 | DEC 2026/直写阈值/交替缓冲/Idle+Max 合帧/Uint8Array/非焦点降频/cancelPendingFlush/64KB 淘汰（TRM-04）/退出码透传；TabTitleRegistry/tabRules mock 改指 cliProfiles 注册表（Stage 01 迁移，用例数不变） |
| `src/__tests__/terminal-registry.test.ts` | 24 | register/get/remove/has/幂等/setAgentSession merge 语义（AgentSessionInfo + cliId 字段/null 清空/undefined 不覆盖/缺 lastEventAt 自动填——NAH-02）/sessionChange 事件/`_reset` |
| `src/__tests__/can-fit.test.ts` | 15 | 五条件守卫 + null/undefined 参数防护 |
| `src/__tests__/use-xterm-integration.test.ts` | 12 | 轻 mock（真实 Terminal/FitAddon）；rAF 回退/onData→pty.write/visible 切换 WebGL 释放；tabRules mock 改指 cliProfiles 注册表（Stage 01 迁移，用例数不变） |
| `src/__tests__/keyboard.test.ts` | 12 | `createTerminalShortcuts()` 经 active 指针派发；Ctrl+C 不注册 |
| `src/__tests__/terminal-registry-subscribe.test.ts` | 7 | subscribe register/remove/sessionChange 通知/退订（setAgentSession 触发） |
| `src/__tests__/webgl-setup.test.ts` | 7 | `setupWebglWithRetry` 指数退避/重试耗尽回退 DOM/cancel 清理（TRM-06 新建） |
| `src/__tests__/terminal-instance.test.ts` | 6 | `useTerminalInstance` 四分支：fit 异常/fontSize undefined/prevFontSize 相同跳过/tryLoadWebgl 幂等（TRM-07 新建） |
| `src/__tests__/terminal.test.tsx` | 14 | TerminalPanel：loading 遮罩 1.5s 超时（TRM-05）/Windows build/spawn/active=false 标题恢复/customTitle 挂载恢复 + onDidParametersChange 同步（F8）/tabLogo 状态机（logoRef 命中保持/清空/双清——CliIconRegistry 引用改指 profile 注册表，F9） |
| `src/__tests__/e2e-gating-terminal.test.ts` | 5 | E2E helper 终端门控（`__e2e_sessionReady` 等）；TabTitleRegistry mock 改指 cliProfiles 注册表（Stage 01，D-13 核对） |
| `src/__tests__/terminal-lifecycle.test.ts` | 4 | 挂载→创建→卸载→dispose 完整链路 |
| `src/__tests__/active-terminal.test.ts` | 4 | active 指针 set/get/覆盖、clear 仅匹配时生效 |
| `src/__tests__/detect-webgl.test.ts` | 3 | WebGL2 可用/不可用/抛异常 |
| `src/__tests__/terminal-strictmode.test.ts` | 2 | `smGuardRef` 防双重挂载 |

### CLI profile 注册表（2 文件 / 62 用例，Stage 01）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/cli-profile-registry.test.ts` | 19 | CliProfileRegistry（MC-101/102）：register/get/getAll 注册序/同 id 覆盖（注册序不变）/matchByCommand 首 token 精确匹配（多 commands 非首键/带参变体/前导空白/空命令行/仅空白/未命中/不 toLowerCase/同键冲突先注册者优先）/`_reset`/独立实例/全局单例 + logo 资源守卫泛化（MC-108：遍历注册表全部 profile 断言 iconSrc 磁盘存在 + PNG 魔数，含 mockcli.png 先行资源）——语义并入自 tab-title-registry（13）+ cli-icons（12） |
| `src/__tests__/cli-profile-claude.test.ts` | 43 | claude profile 身份域（MC-104）：side-effect 注册/CLAUDE_CLI_ID 常量与注册一致性/身份域字段完整（含 hooks 五字段）/capabilities.history 未迁入/带参命中/`_reset` 恢复（8）——语义并入自 tab-rules（6）+ hooks 策略（Stage 02，35）：eventToStatus 26 用例语义迁入（原 claude-status，10 事件 × notificationType + STATUS_EMOJI 联合守卫 + AgentStatus 类型兼容）+ classifyNotification 五映射表驱动 9 条（NAH-03 迁入，MC-422） |

### 编辑器面板（8 文件 / 134 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/use-code-mirror.test.ts` | 39 | 字体扩展/Compartment reconfigure/handleSave（有/无 filePath、另存为、失败 alert——EDF-03 大文件拒绝/警告）/slterm:file-saved + file-saved-as |
| `src/__tests__/git-gutter.test.ts` | 32 | StateEffect → RangeSet 映射/GutterMarker DOM/SpacerMarker/HEAD 侧 old 行号映射 + **四 dispatch wrapper 直接调用（EDF-05）** |
| `src/__tests__/language-mapping.test.ts` | 23 | 扩展名→语言映射 + 未知回退 |
| `src/__tests__/editor-confirm.test.ts` | 11 | dirty/clean 外部修改确认/订阅取消/kind 过滤 |
| `src/__tests__/editor.test.tsx` | 9 | **浅层定位（DOC-02⑥）**：mock useCodeMirror，验证 panelId/filePath 透传 + `overflow: clip` 样式——组件集成契约测试 |
| `src/__tests__/editor-font.test.ts` | 8 | 字体 CSS 选择器（`.cm-scroller` vs `.cm-editor`） |
| `src/__tests__/editor-keyboard.test.ts` | 7 | `createEditorShortcuts()` save/toggleWordWrap 经 active 指针派发 |
| `src/__tests__/active-editor.test.ts` | 5 | active 指针 set/get/覆盖、clear 仅匹配时生效 |

### 工作区/布局/页签（15 文件 / 218 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/title-manager.test.ts` | 47 | terminal-N 递增/编辑器 basename/同名冲突相对路径/handleSaveAs/onDeletePage/suffix 标题生成/冲突重算保留后缀/findExistingEditor 匹配隔离（B10） |
| `src/__tests__/layout-serde.test.ts` | 26 | 旧格式修补/白名单过滤（对齐真实 6 种 PANEL_TYPES，WRK-07）/深拷贝/嵌套 branch/activeGroup 保留 + 条目级容错（null 条目跳过） |
| `src/__tests__/panel-registry.test.ts` | 25 | 注册表 6 面板/PANEL_TYPES 长度 6/isValidPanelType/FILE_PANEL_TYPES/isAlwaysRenderPanel |
| `src/__tests__/workspace-defaulttab.test.tsx` | 27 | **生产 DefaultTab 渲染（WRK-05 非手写 Mock）**：tabIcon emoji/img 分支/onDidParametersChange 扁平事件结构回归（event.tabIcon 非 event.params.tabIcon）/tabLogo CLI logo 渲染（仅随 emoji/顺序/动态双向/URL 并存，F9） |
| `src/__tests__/workspace-page-dockview.test.tsx` | 7 | PageDockview 真实组件（WRK-01）：handleReady 空布局不兜底/Watermark 按钮 addPanel/RightHeader「+」/onSaveAs 重算标题 |
| `src/__tests__/pageapis.test.ts` | 11 | pageApis（WRK-02）：switchToPageShared 时序（invocationCallOrder，DBG-5/9）/reject 降级/`__dockviewApi` 重指；switchToPageAndFocus 轮询命中/5s 超时降级 |
| `src/__tests__/workspace-header-actions.test.tsx` | 21 | RightHeader Watermark 按钮/页签操作/右键菜单重命名项（F8：终端 7 项结构/非终端 5 项/action 派发/agentSession 存在禁用——MC-405 更名同步） |
| `src/__tests__/terminal-rename-dialog.test.tsx` | 13 | 重命名弹窗（F8）：预填/受控输入/Enter 提交 trim/空名拒绝行内错误/取消（按钮/Esc/遮罩）/错误清除/initialTitle 跟随 |
| `src/__tests__/terminal-rename-apply.test.ts` | 5 | `applyRename` 纯函数（F8）：updateParameters 展开保留原键 + customTitle/params undefined 分支/setTitle/onLayoutChange 收到 toJSON 值/原对象不被修改 |
| `src/__tests__/workspace-switch-order.test.tsx` | 14 | **真实驱动（WRK-06）**：点击页面行触发 switchToPage 断言 setProjectRoot 先于 setActivePage/reject 降级/SEC-01 effect 兜底 |
| `src/__tests__/workspace-file-panel-types.test.ts` | 14 | FILE_PANEL_TYPES/isAlwaysRenderPanel（5 面板） |
| `src/__tests__/default-layout-format.test.ts` | 10 | makeEmptyLayout 空布局验证 + SidebarTree 实际使用断言（WRK-11②） |
| `src/__tests__/layout-switch.test.ts` | 7 | 页面切换集成/自切换守卫 |
| `src/__tests__/workspace-multi-instance.test.tsx` | 6 | 多 Dockview 实例惰性初始化 + **H6 实例 identity 断言（WRK-09：同一 api 跨切换 + 终端不 dispose）** |
| `src/__tests__/open-hooks-config-panel.test.ts` | 6 | openHooksConfigPanel：addPanel 参数精确/同页单例 focus 不新建/轮询命中/5s 超时降级/getPanel 无 focus 降级（HKC-09） |
| `src/__tests__/workspace-e2e-ready.test.tsx` | 4 | `__slterm_e2e_workspaceReady` 标记同步性 |
| `src/__tests__/workspace.test.tsx` | 4 | Dockview 初始化/项目页面关联 |

### Store 状态管理（4 文件 / 84 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/projects.test.ts` | 46 | Project/Page CRUD/持久化/version 递增/ID 生成/expandedNodes + cancelPendingSave 清理（STS-06） |
| `src/__tests__/font-size.test.ts` | 17 | 默认值/clamp/loadFromDisk/debounce 持久化 + **cancelPendingSave 活跃 timer 取消（SVC-02）** |
| `src/__tests__/keybindings.test.ts` | 17 | setBinding/clearBinding/resetAll/sanitize/loaded 守卫/debounce + **cancelPendingSave（SVC-02）** |
| `src/__tests__/layout.test.ts` | 4 | activePageId 设置/清空/重复 |

### 资源管理器（20 文件 / 272 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/explorer-git-status.test.tsx` | 32 | gitStatusMap 查表着色/配色 token/F5 untracked/slterm:file-saved/mount 单次加载 |
| `src/__tests__/explorer-delete.test.tsx` | 22 | ask 弹窗分支/右键菜单/操作失败 UI 通知/横幅 dismiss+5s 自动消失+卸载清理（EXP-04）/E6 编号统一（EXP-11） |
| `src/__tests__/file-icon.test.tsx` | 36 | 扩展名图标表驱动（含 .pyw/.markdown/.less/.scss/.gitattributes，EXP-05）+ 目录图标 + git 状态着色 |
| `src/__tests__/explorer-file-viewer.test.tsx` | 21 | handleOpenFile 面板分派/FileViewerRegistry/htmlviewer 回退/防御分支（EXP-10） |
| `src/__tests__/explorer-refresh-preserve.test.tsx` | 17 | reloadPreservingExpanded 递归重建/边界容错/三条触发路径/竞态 |
| `src/__tests__/explorer-selection.test.tsx` | 17 | FileTree 选中模型（单击/双击/空白取消/hover 不覆盖）+ 非选中行 hover（EXP-04） |
| `src/__tests__/explorer-keyboard.test.ts` | 15 | `createExplorerShortcuts()` delete/open/rename 经 active 指针派发 + ref 模式闭包不过期 |
| `src/__tests__/use-file-tree.test.ts` | 15 | loadRoot/loadDirectory/toggleExpand/generation 取消 |
| `src/__tests__/explorer-root-contextmenu.test.tsx` | 14 | 根节点右键菜单/新建文件+文件夹 |
| `src/__tests__/explorer-sandbox-race.test.tsx` | 13 | DBG-10：deferred setProjectRoot 竞态回归 |
| `src/__tests__/explorer-notify.test.tsx` | 12 | startWatch 调用时机/loadRoot/toggleExpand |
| `src/__tests__/explorer-input-boundary.test.tsx` | 10 | 内联输入框边界（EXP-06）：Enter 空名/失焦提交/失焦空值/重名/Escape |
| `src/__tests__/explorer-rename-state.test.tsx` | 8 | 重命名状态上提 |
| `src/__tests__/explorer-open-in-terminal.test.tsx` | 7 | 「在终端中打开」（EXP-01）：addPanel 参数（component/cwd/panelId/renderer） |
| `src/__tests__/explorer-race-cleanup.test.tsx` | 6 | useFileTree 竞态清理（EXP-07）：旧请求延迟 resolve 丢弃/卸载清理 |
| `src/__tests__/activeExplorer.test.ts` | 6 | active 指针 set/get/覆盖、clear 仅匹配时生效 |
| `src/__tests__/explorer-rootpath-clear.test.tsx` | 6 | rootPath 变化清空/快速切换 gen 丢弃/同值不清空 |
| `src/__tests__/explorer-focus.test.tsx` | 6 | 焦点管理（tabIndex/usePanelFocus 集成）+ focusin/focusout 上下文栈（EXP-04） |
| `src/__tests__/explorer-rename-keyboard.test.tsx` | 5 | F2 快捷键 → renameSelected 集成 |
| `src/__tests__/explorer-crud-success.test.tsx` | 4 | CRUD 成功路径（EXP-02）：IPC + refresh + 状态重置 |

### 侧栏（1 文件 / 47 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sidebar-actions.test.ts` | 47 | 树结构/右键菜单/内联重命名/项目删除确认/添加项目（dialog 取消降级，EXP-08）/hover 与 stopPropagation（EXP-09）/「打开 Hooks 配置」菜单入口 5 用例 |

### 侧栏视图（6 文件 / 142 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/sideBarState.test.ts` | 53 | toggleViewPure/moveButtonPure（含 R7 目标区非空场景，SVC-06）/deriveLayout/reconcileZones/sanitizeSideBar（含 NaN/Infinity，SVC-13）+ S1-S6 场景序列 |
| `src/__tests__/activityBar.test.tsx` | 33 | 渲染/active/toggle/title/dragStart/**dragOver+drop 全部含第三参数 index 断言（SVC-01）**/hover/**resolveTargetZone 中点 ±1 边界（SVC-05）**/指示线清理——拖拽 mock 理想化见豁免表 |
| `src/__tests__/sideBar.test.ts` | 21 | 默认值/toggle/move/loadFromDisk 5 分支/loaded 守卫/debounce payload 键集合精确匹配 + **cancelPendingSave（SVC-02）** + clamp NaN/Infinity |
| `src/__tests__/sideBarArea.test.tsx` | 15 | 四态布局/preferredSize splitRatio/display 切换保挂载/换区重建/props 透传/onChange→setSplitRatio/**total<=0 除零守卫（SVC-07）**/PANEL_BG token |
| `src/__tests__/workspace-sideviews.test.tsx` | 13 | 活动栏 40px 固定/侧栏区 visible 四态/preferredSize/onChange→setWidth/主区 minSize/props 引用断言（SVC-10） |
| `src/__tests__/sideViewRegistry.test.ts` | 7 | register/getAll/get/重复注册覆盖/未注册 get→undefined/_reset 隔离 |

### Commit 视图（6 文件 / 60 用例，SVC-14 拆分）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/commit-open-file.test.ts` | 17 | 双击分派（SVC-04/11）：四状态 addPanel 参数/去重聚焦（B10 反向经 openCommitFile 驱动）/守卫路径（无 pageApi/未知状态/addPanel 失败降级） |
| `src/__tests__/commit-context-menu.test.ts` | 15 | getContextMenuItems 状态→菜单映射 + action 执行流程 + 删除 catch（SVC-09） |
| `src/__tests__/commit-view-list.test.tsx` | 9 | 列表渲染（token/计数/排序/空态）/折叠交互/fs-event 200ms 去抖（SVC-03）/rootPath 切换（SVC-12 计时统一） |
| `src/__tests__/commit-context-menu-ui.test.tsx` | 8 | 菜单 UI 交互（外点关闭/项点击 → ask → IPC → refresh） |
| `src/__tests__/commit-view-status.test.ts` | 7 | 状态机四态（no-root/loading/error/ready） |
| `src/__tests__/commit-view.test.tsx` | 4 | 残余主干用例（拆分后） |

### hooks 配置面板（10 文件 / 197 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/hooks-config-handlerform.test.tsx` | 40 | 5 种 type 字段矩阵（官方版字段名断言）/switchHandlerType 纯函数/事件支持矩阵过滤/type 切换交互/字段编辑（合法上报/非法保留草稿/清空删键，HKC-04）/注入段禁改 |
| `src/__tests__/hooks-config-gui.test.tsx` | 28 | Master-Detail 渲染/增删事件与 matcher 组/**删除选中项选中态回退空态（HKC-05）**/注入段禁删 |
| `src/__tests__/hooks-config-panel.test.tsx` | 23 | 三态/层级切换器禁用/保存按钮初始禁用/visibilitychange 轻量重读（askGuard 防循环）/JSON 错误单行截断/**注入状态条初始 "--" 帧（HKC-10）**/**非法 JSON 快照不变+保存禁用（HKC-03）**/**uninstall reject 分支（HKC-07）** |
| `src/__tests__/hooks-config-jsonmode.test.tsx` | 18 | CM6 创建 + schema 扩展注册/**linter 顺序身份断言（HKC-01）**/非法 JSON 校验上报/事件导航/MatcherTester |
| `src/__tests__/hooks-config-matcher.test.ts` | 21 | matchHook 全分支（exact-or/regex/all/受限窄字符集/非法正则防御） |
| `src/__tests__/hooks-config-catalog.test.ts` | 19 | eventsCatalog 事件元数据（30 事件 × 10 组/HANDLER_FIELD_MATRIX/纯查询函数） |
| `src/__tests__/hooks-config-model.test.ts` | 17 | jsonToGui/guiToJson 双向转换/round-trip/容错/isSltermManaged |
| `src/__tests__/ipc-hooks-config-contract.test.ts` | 12 | readHooksConfig/writeHooksConfig 四维验证（agent_hooks_config_read/write + cliId 首参，IHE-06 工厂化，MC-212 同步） |
| `src/__tests__/hooks-config-schema.test.ts` | 10 | validateHooksJson 直测边界（HKC-08 新建）：合法/缺 hooks 键/非法 matcher/未知事件告警 |
| `src/__tests__/hooks-config-sync.test.tsx` | 9 | 双模式同步（JSON→GUI/GUI→JSON/非法禁切/脏状态流转） |

### Diff/GitShow 面板（3 文件 / 78 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/diff-panel.test.tsx` | 39 | mock gitFileAtHead+fs+gitDiff+onFsEvent、双栏渲染、加载/错误占位、**保存后刷新链真实断言（EDF-01：writeFile→gitDiff→gutter）**、**五分支补齐（EDF-02：占位刷新/.git 刷新/脏确认/滚动重绑/大文件）**、滚动同步去固定延时（EDF-07） |
| `src/__tests__/gitshow-panel.test.tsx` | 21 | mock gitFileAtHead、三态、readOnly 断言、oldPath 优先、**大文件警告精确断言 + EditorView identity 切换（EDF-04）**、**字号 reconfigure（EDF-09）** |
| `src/__tests__/diff-alignment.test.ts` | 18 | computeAlignment 纯函数全分支 + **key<0 过滤（EDF-06）** |

### 快捷键/命令系统（7 文件 / 127 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/shortcuts.test.ts` | 54 | 注册/注销/引用计数/上下文栈竞态/IME/匹配排序/setOverrides 重绑解绑降级冲突/resolve/forceContext（**含 global 先注册反向分支，STS-03**）/export/list |
| `src/__tests__/keystroke.test.ts` | 26 | formatKeystroke/parseKeystroke/isValidKeystrokeString/format∘parse 恒等 |
| `src/__tests__/global-commands.test.ts` | 13 | createGlobalShortcuts 延迟求值/Ctrl+W 关闭/返回一条命令/无面板透传/（STS-02 名实对齐） |
| `src/__tests__/command-catalog.test.ts` | 17 | 9 命令齐全 + 元数据 + **commandFromMeta 参数化遍历全 9 命令（STS-08）** |
| `src/__tests__/reserved.test.ts` | 9 | isReserved 各 context/保留键命中/global 两集并集 |
| `src/__tests__/use-panel-focus.test.ts` | 5 | focusin→pushContext+onActivate/focusout→popContext+onDeactivate/卸载清理 |
| `src/__tests__/wire-keybindings.test.ts` | 3 | 立即应用/store 变更重应用/unsubscribe |

### 主题/配色/基础（7 文件 / 169 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/colors.test.ts` | 85 | **配色 token 真实导出值断言（STS-01：GIT_FILE/GIT_GUTTER/EXPLORER/SIDEBAR/AGENT_STATUS_USAGE 五组 + uiTokenCases 补 EXPLORER_SELECTION_BG 等，STS-09）**——it.each 表驱动展开；facade 化后 89→85（死配置清理移除 4 条断言） |
| `src/__tests__/scheme-registry.test.ts` | 18 | 方案注册表单测（TST-02）：register/get/getAll/getDefaultId、setActive 已知/未知 id 回退 darcula + console.warn、getActive 默认 darcula、重复注册覆盖、`_reset` 隔离、darcula 四段完整性（ui 6 组键数 7/3/5/8/3/3 + 23 标量、terminal 25 键、editor oneDark 透出非 undefined、libraries dockview 20 条 + allotment 2 键） |
| `src/__tests__/overrides.test.ts` | 7 | 主题 overrides 导出单测（TST-03）：dockviewVarStyle 键集合 20 条且值为 active 方案色、allotmentVarStyle 2 键、editorTheme === active 方案 editor 段、editorColorOverrides 返回 CM6 扩展（lint/searchMatch/background 键生效）、层叠胜出守卫（ACC-05：竞争规则选择器带 .cm-editor 前缀）、setActive 后输出跟随切换 |
| `src/__tests__/agent-status-lib.test.ts` | 6 | lib 层四态常量（MC-401 迁移）：STATUS_EMOJI 恰好 4 键/emoji 值（契约 C7）/null 不在映射表 + getStatusIcon（**null 分支，STS-04**）+ AgentStatus 类型接受 5 值——事件映射用例已随实现迁出至 cli-profile-claude（lib 层不再含 claude 事件名字面量） |
| `src/__tests__/path.test.ts` | 27 | normalizePath/basename/isChildOf/relativePath 边界覆盖 |
| `src/__tests__/inject-script.test.ts` | 21 | HTML 脚本注入/`</script>` 转义/幂等/大小写不敏感/键盘转发+片段链接拦截（STS-11① 性能断言已删） |
| `src/__tests__/theme.test.ts` | 13 | terminalOptions: ANSI 16 色/font/cursor/scrollback/**kittyKeyboard（STS-05）** |
| `src/__tests__/panelId.test.ts` | 5 | parseTerminalPageId 全分支 |

### 通知/Agent 状态（3 文件 / 101 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/agent-status-hook.test.ts` | 39 | useAgentStatus 行建模新语义全分支（Stage 02 更名同步）：纯 shell 无行/双通道建行（hook 通道按 MC-205 三级解析 cliId 写入行）/三通道删行/初始扫描携 transcriptPath 拉 usage/reconcile 对账/now ticker 60s 重算 |
| `src/__tests__/notifications.test.ts` | 33 | **去重缓存 250 事件截断 100 + 最旧再弹（NAH-04）**/hook-event 通知调度（类别判定经 profile.hooks.classifyNotification 委托——MC-420 两段分解，纯函数表驱动 9 条迁入 cli-profile-claude）/窗口失焦门控/任务栏闪烁/积压 flush/并发竞态 |
| `src/__tests__/agent-status-view.test.tsx` | 29 | AgentStatusView 组件（Stage 02 更名 + 空态文案同步）：no-root/empty 占位（**空态「无运行中的编码 CLI 会话」，MC-414**）/**行2 用量条+相对时间断言（NAH-05）**/点击行 switchToPageAndFocus/**双行布局结构**/三级字号/用量口径（contextLimit 来自行 cliId profile.hooks，MC-412）/分段颜色/now prop 驱动重算/三下拉框结构/**标题覆盖真实/受控 history 集成（NAH-06）**/行1 CLI logo（仅随 emoji/图标列 40px 簇/行2 缩进 48px，F9） |

### Claude 历史会话（6 文件 / 117 用例，F7）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/claude-history-model.test.ts` | 41 | 纯函数全分支（Stage 02 更名同步，AgentStatus；Stage 04 类型/工厂同步 AgentHistorySession 八键含 cliId，MC-306）：isCurrentProject（决策 24）/groupByCwd/matchesSearch/formatRelativeTime（决策 26）/deriveActiveSessionStatuses（**sessionId null 回退 basename，NAH-01**） |
| `src/__tests__/claude-history-view.test.tsx` | 33 | 受控 props 注入/三区结构/搜索过滤/组默认收起+计数/**双击运行中 → SessionActionDialog（NAH-09②）**/菜单矩阵 3 项/标题覆盖集成/空态文案/E2E 红线（Stage 02 更名同步，AgentStatus；Stage 04 mock 路径 ../ipc/agentHistory + 删除链传 cliId 断言同步，MC-306） |
| `src/__tests__/claude-history-row.test.tsx` | 19 | 双行渲染/四态标记/✗ 孤儿标记/**图标优先级（status 覆盖 orphan，NAH-10）**/字号断言/单击选中/双击三分派/行1 CLI logo（仅随 status emoji/孤儿行不加图，F9）（Stage 04 类型同步，MC-306） |
| `src/__tests__/claude-history-hook.test.tsx` | 13 | 状态机流转/scan 成功失败/removeLocal 不重扫/**scan generation 竞态（NAH-08）**/subscribe 驱动 activeStatuses/卸载清理（Stage 02 更名同步，AgentStatus；Stage 04 mock 路径 + 类型同步，MC-306） |
| `src/__tests__/claude-history-restore.test.ts` | 7 | 四步编排/pty.write 内容（普通/fork/`\r`）/**防重入（NAH-07①）**/失败 toast/**cwd null 防御性 throw（NAH-07②）**（Stage 04 类型同步，MC-306） |
| `src/__tests__/claude-history-action-dialog.test.tsx` | 7 | SessionActionDialog 渲染/action 回调/取消（按钮/Esc/遮罩）/**空 actions 防御（NAH-11）**（Stage 04 零改动） |

### 启动/关闭（4 文件 / 22 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/close-handler.test.ts` | 11 | flush layout→saveAllProjects→localStorage/超时/异常 + 关窗拦截（WRK-08） |
| `src/__tests__/startup-restore.test.ts` | 7 | localStorage 恢复/空/异常降级/**setProjectRoot 先于 setActivePage 顺序断言（WRK-03）**/requestUserAttention catch |
| `src/__tests__/bootstrap.test.ts` | 3 | `__TAURI_INTERNALS__` 轮询/立即挂载/永不就绪 |
| `src/__tests__/main-bootstrap.test.tsx` | 1 | main.tsx init 失败 catch 分支（WRK-10） |

### 文件查看器/HTML（3 文件 / 80 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/html-panel.test.tsx` | 42 | 三态渲染/竞态取消/sandbox 属性/**postMessage 四负面用例（IHE-03：origin/source/type/fingerprint——jsdom 模拟标注，真实 WebView2 由 L4 验收）**/片段链接拦截/waitForLoaded helper（IHE-08）/注入脚本控制流断言（IHE-07②） |
| `src/__tests__/file-viewer-registry.test.ts` | 31 | 扩展名注册/策略链式调用/隐藏文件排除/大小写/**`_reset` 后预注册恢复 + resolve(".gitignore")/resolve("file.") 边界（EXP-12）** |
| `src/__tests__/csp-config.test.ts` | 7 | tauri.conf.json CSP 不变量：script-src unsafe-inline/dangerousDisableAssetCspModification/default-src 严格/**style-src/connect-src/img-src 关键字段快照（IHE-07④）** |

### E2E 辅助/门控测试（8 文件 / 38 用例）

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `src/__tests__/e2e-build-config.test.ts` | 8 | VITE_E2E build 配置不变量 + **E2E_ENABLED 字面量表达式断言（IHE-04，AST/正则）** |
| `src/__tests__/e2e-enabled.test.ts` | 9 | E2E_ENABLED 真值表（it.each 7 组展开）+ 常量与纯函数一致性 |
| `src/__tests__/app.test.tsx` | 5 | **E2E helper 行为契约（DOC-02⑤）**：`__slterm_e2e_createProject` 行为/pending 标记 |
| `src/__tests__/error-boundary.test.tsx` | 5 | 正常透传/抛错 UI/`__sltermError` 赋值/**variant="inline" 渲染（IHE-05）** |
| `src/__tests__/e2e-clipboard-helper.test.ts` | 3 | writeClipboard/createProject 函数可用性 |
| `src/__tests__/e2e-create-project.test.ts` | 3 | **E2E helper 行为契约（DOC-02⑤）**：pending 标记→localStorage 恢复交互 |
| `src/__tests__/dialog-e2e-hook.test.ts` | 3 | E2E ask 钩子守卫（未设置走真实/true 直接返回/false 返回 false） |
| `src/__tests__/e2e-gating-workspace.test.tsx` | 2 | `__slterm_e2e_workspaceReady` 存在性 |

## L3 — 终端 headless 测试（7 文件 / 138 用例）

运行：`npm run test:l3`（Vitest + `@xterm/headless`，`environment: 'node'`）

> **定位声明（DOC-02①/E2E-04）**：L3 = **网格状态正确性，非渲染正确性**——headless 不跑 WebGL/GPU/onContextLoss；生产 `terminalOptions`/OSC handler 的验证限于 headless 网格可观察语义（`theme-options.test.ts` 文件头已标注），渲染正确性由 L4 视觉回归（M2 人工确认）兜底。`keyboard.test.ts` 已降级标注"xterm.js 基础行为回归（非 slTerminal 键盘链路）"（E2E-01，D4）。

| 文件 | 用例 | 覆盖范围 |
|------|------|---------|
| `test/terminal/terminal-serialize.test.ts` | 41 | 基本文本序列化、多行输出、ANSI 颜色保留、大块数据、**光标定位改 getLine/translateToString 行列精确断言（E2E-07）**、scrollback、resize reflow、SGR 属性叠加、多语言字符、交替屏幕、所有 ED/EL/IL/DL 擦除操作、DECSC/DECRC |
| `test/terminal/keyboard.test.ts` | 36 | Ctrl 组合键（A-Z 全表 + Backslash/Slash）、Alt+字母/Enter、功能键 F1-F12、Home/End/PgUp/PgDn/Insert/Delete、方向键 + Ctrl+方向键、CSI u 协议、退格/回车/制表符——**降级标注：xterm.js 基础行为回归（E2E-01）** |
| `test/terminal/ansi-correctness.test.ts` | 30 | ANSI 颜色正确性——16 色、256 色（**按 SerializeAddon 实际优化行为断言基本 SGR 30-37/90-97，E2E-08**）、TrueColor、SGR 属性与组合、DEC 私有模式、DECSC/DECRC、RIS、DECSTBM |
| `test/terminal/osc.test.ts` | 9 | OSC 序列——标题（OSC 0/2 BEL/ST）、调色板（OSC 4）、嵌入完整性 |
| `test/terminal/theme-options.test.ts` | 5 | **生产 terminalOptions（E2E-02 新增）**：16 色 ANSI 与主题色板一致/CSI>1u 可激活 Kitty/scrollback 容量生效/drawBoldTextInBrightColors 亮色映射 |
| `test/terminal/production-osc.test.ts` | 8 | **生产 OSC 52/133/8 handler（E2E-03 新增）**：OSC 52 → mock writeText 断言（CJK 解码）/OSC 133 复刻段按生产 matchByCommand/profile 取值改写（D-08，8 用例数不变）/OSC 8 → mock openUrl |
| `test/terminal/negative-ansi.test.ts` | 9 | **反向/异常 ANSI（E2E-14 新增）**：非法 ANSI、截断多字节序列、嵌套 OSC、异常 resize（0×0）——headless 不崩溃 + 状态可恢复 |

## L4 — E2E 端到端测试（8 spec / 37 用例，35 active + 2 skip）

运行：`npm run e2e`（= `npm run build:e2e` + `npm run wdio`）  
技术栈：WDIO + `@wdio/tauri-service` 1.1.0 + embedded driver（`webview2-com` COM 直连 `ICoreWebView2`）；specs 通配 `./*.e2e.ts`（E2E-09 拆分，单 worker 顺序执行）

> **定位声明（DOC-02②/E2E-11）**：键盘/拖拽/恢复编排三类用例为**半端到端/部分端到端**——应用内监听/匹配/命令 handler/真实 IPC 全链路在真实二进制执行，唯一"不真实"处是事件来源或前置动作（详见 `e2e-tests/CLAUDE.md`「定位声明」段）。真实 OS 按键豁免见豁免表。

| spec | 用例 | active/skip | 覆盖范围 |
|------|------|-------------|---------|
| `terminal.e2e.ts` | 7 | 7 active | 启动标题/PTY 通信+缓冲断言/helper 写入读取/terminal-N 标题/**H6 跨页面存活**/**全屏 TUI 大负载+切页签往返（E2E-04 视觉回归，M2 人工确认）**/**强杀 slterminal.exe 子进程树无残留（E2E-12）** |
| `editor.e2e.ts` | 5 | 5 active | 编辑器标题 basename/同名冲突相对路径/关闭后重算/Ctrl+S 经 capture 真实写盘（mtime）/外部修改触发 reload 后保存（dirty→clean） |
| `history.e2e.ts` | 8 | 8 active | fixture 6 行展示 + agent-*/非 UUID/subagents 排除/标题回退链/搜索过滤/复制恢复命令（剪贴板断言）/孤儿 ✗ 双击无反应/删除（ask 钩子 + 副本删除）/历史区四态同源/**恢复编排（部分端到端：断言到 pty.write 注入，不断言真实进入会话）**——命令名 agent_history_scan/delete 断言同步（D-14/MC-306，用例数不变） |
| `agent.e2e.ts` | 7 | 6 active + 1 skip | 视图存在性/纯 shell 终端无行（行建模新语义）/动态四态（首个信号即建行→⚡→✅→行消失）/R2 变体（切项目用量保持 + cache 字段）/R3 变体（SessionEnd 删行不复活）/R4 变体（关页签删行）/**toast 触发链路（skip：权限弹窗需用户交互）**——空态文案「无运行中的编码 CLI 会话」断言（MC-414，Stage 02） |
| `hooks.e2e.ts` | 4 | 4 active | 注入/卸载/状态三态/信号文件驱动页签 emoji/**真实 hook reporter 链路（E2E-06：node 执行脚本 + stdin JSON + SLTERM_PANEL_ID → 信号消费 + 非法 JSON exit 0 的 C10 守卫）**/hooksConfig project 层保存写盘 + merge 保留其他字段 |
| `html.e2e.ts` | 2 | 1 active + 1 skip | iframe Ctrl+W postMessage 转发关闭（真实二进制全链路）/**内联脚本/事件 CSP 执行验证（skip：执行断言不稳定）** |
| `sidebar.e2e.ts` | 2 | 2 active | 点击开关（R1/R2）/跨区移动状态机（R6/R7——经 store helper，非真实 DnD） |
| `commit.e2e.ts` | 2 | 2 active | 真实 git 仓库变更列表渲染（Changes/Unversioned）/双击 modified 打开 diff 页签 |

### 用户目录隔离机制（FIX-TE-04 + E2E-05 扩展）

`run-wdio.cjs` 启动时备份 `~/.slterminal/settings.json`、`~/.claude/settings.json`、`~/.slterminal/hooks/`（均复制为 `.e2e-bak`），exit 时同步还原（还原前先删产物再 rename；`~/.slterminal/hooks-events/` 直接清理）。E2E 用例不得触碰真实 `~/.claude/projects/`（`SLTERM_CLAUDE_PROJECTS_DIR` env 指向 `e2e-tests/.tmp-claude-projects/` 副本，fixture 经 run-wdio.cjs 重建）。详见 `e2e-tests/CLAUDE.md`。

### E2E 键盘输入限制（半端到端，TE-17）

embedded WDIO 驱动**无法将 OS 级按键（`browser.keys`）投递进 WebView2 页面**。所有键盘用例均改用合成事件路径（页面内 dispatch 合成 `keydown`/`focusin` 激活 context）→ ShortcutRegistry window capture 真实捕获 → 命令 handler → 真实 IPC。唯一"不真实"处是事件来源为 JS dispatch 而非 OS 键盘；监听/上下文匹配/命令 handler/写盘/转发全链路均在真实二进制中执行。

## 静态检查门禁

| 门 | 命令 | 说明 |
|----|------|------|
| TypeScript | `npx tsc --noEmit` | 全量类型检查 |
| ESLint | `npx eslint src/` | 前端代码规范 |
| Clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Rust 代码规范 |

## 历史变更

- 2026-08-10（multi-cli Stage 04 后端历史泛化 + 前端 ipc/types，MC-301~306 + D-03/D-14 历史段）：L1 claude_history 63 用例迁移 agent_history（claude/jsonl 28 + claude/scan 19 + claude/ops 9 + mod 7 用例数不变，MC-301——下沉 claude/ 行为零改动 + env 覆盖 SLTERM_CLAUDE_PROJECTS_DIR 留 provider 内部 MC-305）；mod.rs 7 覆盖更新（HistorySession→AgentHistorySession serde camelCase 八键含 cliId 打标 + titleSource 开放字符串，MC-302）；新建 provider.rs（①：CliHistoryProvider trait 三方法 + 注册表 + 聚合 scan 遍历/delete 未知 cliId Validation/validate_session_id 强制前置，MC-303/304——计数以 backend 实落为准）。L2：ipc-claude-history-contract → ipc-agent-history-contract 8 用例数不变（命令名 agent_history_scan（无参）/agent_history_delete（{cliId, sessionId} 双参 camelCase）四维同步，MC-306）；claude-history-{model 41/view 33/row 19/hook 13/restore 7} 用例数不变（AgentHistorySession 八键含 cliId + mock 路径 ../ipc/agentHistory + 删除链传 session.cliId 断言同步，MC-306；action-dialog 7 零改动）；调用点中间态：useClaudeHistory/HistorySessionList/historyContextMenu 类型与签名同步（删除链 deleteHistorySession(session.cliId, session.sessionId)，features 目录更名留 Stage 05）。L4 history.e2e 8 用例数不变（命令名 agent_history_scan/delete 断言同步，D-14）。L1 584→584+①，L2 2329 不变，全量 3088→3088+①。
- 2026-08-10（multi-cli Stage 03 后端 hooks 泛化 + 前端 ipc/types，MC-201~215 + D-01/03/09/10/11/14）：L1 hooks 域 133→146（+13）。位置迁移：inject 34→claude/inject.rs 35（+1 reporter 模板内嵌校验断言——显式 cliId + SCRIPT_VERSION 递增，MC-215 决策 7）、usage 26 / config 27 下沉 claude/ 用例数不变（MC-213）、watcher 20 MC-203 核对零改动；mod.rs 12→19（+7 命令层泛化：6 命令 cliId 透传 block_on 直测 + 更名 AgentInjectionStatus/AgentHookInjectionStatus，MC-211）、signal.rs 14→16（+2 AgentEventPayload 九键 serde 含无 cliId 旧信号反序列化兼容 MC-201 + agent-event 广播 MC-202）、新建 provider.rs 3（注册表 resolve_provider 命中/未知 cliId Validation/无 hooks 能力 Validation「不支持 hooks 能力」+ 未注册未知分支，MC-210）。L2：ipc-hooks-contract → ipc-agent-hooks-contract 22 用例数不变（命令名 agent_hooks_*/agent_context_usage + 参数含 cliId camelCase 四维同步，MC-212）；ipc-hooks-config-contract 12 用例数不变（agent_hooks_config_read/write + cliId 首参）；setup.ts 全局 mock 更名 ../ipc/hooks → ../ipc/agentHooks + onHookEvent→onAgentEvent（D-01 红线）；use-xterm-lifecycle 71 / use-xterm-output 35 / use-xterm-integration 12 / agent-status-view 29 / agent-status-hook 39 / notifications 33 / hooks-config-panel 23 / hooks-config-sync 9 用例数不变（事件名 onAgentEvent/"agent-event" + contextUsage 传行 cliId + 中间态 CLAUDE_CLI_ID 断言同步，MC-202/212）；调用点中间态：useHooksConfig/HooksConfigPanel 泛化命令 cliId 暂传 CLAUDE_CLI_ID 常量（Stage 06 回收）、useAgentStatus contextUsage 传行 cliId。L4 hooks.e2e 4 / agent.e2e 7 用例数不变（命令名 agent_hooks_* 与事件名 agent-event 断言同步，D-14）；run-wdio.cjs E2E-05 备份集合注释「随第二 CLI 接入扩展」（D-11，无用例变动）。L2 2329 不变，L1 571→584，全量 3075→3088。
- 2026-08-10（multi-cli Stage 02 前端状态域去 claude 化，MC-401~422）：删 claude-status 32（语义拆分迁出：lib 层 6 用例 → 新建 agent-status-lib；事件映射 26 用例 → cli-profile-claude，落点改此）+ 新建 agent-status-lib 6（STATUS_EMOJI/getStatusIcon/AgentStatus 类型——lib 层不再含 claude 事件名字面量，MC-401）。cli-profile-claude 7→43（+36：eventToStatus 26 用例语义迁入 + classifyNotification 五映射表驱动 9 条迁入（NAH-03，notifications 纯函数层迁出）+ hooks 能力五字段断言 +1 与 capabilities.history 未迁入——contextLimit=200_000/restartHint/hasConfigEditor，MC-214 前端半 + MC-422）。notifications 42→33（classifyEvent 纯函数表驱动 9 条迁出，类别判定改经 profile.hooks.classifyNotification 委托——MC-420）。terminal-registry 24 / terminal-registry-subscribe 7 / use-xterm-lifecycle 71 / use-xterm-output 35 / use-xterm-integration 12 / claude-history-model 41 / claude-history-view 33 / claude-history-hook 13 用例数不变（ClaudeStatus→AgentStatus、claudeSession→agentSession、setClaudeSession→setAgentSession 更名同步 + AgentSessionInfo 新增 cliId——MC-402/403/410）；agent-status-view 29 / agent-status-hook 39 用例数不变（更名 + 空态文案「无运行中的编码 CLI 会话」MC-414 + 用量口径 contextLimit 来自 profile MC-412）；workspace-header-actions 21 用例数不变（F8 禁用判定 agentSession，MC-405）；L4 agent.e2e 7 用例数不变（空态文案断言同步，MC-414）；L3 production-osc 8 用例数不变（更名同步）。L2 2328→2329（137 文件不变），全量 3074→3075。
- 2026-08-10（multi-cli Stage 01 CLI profile 注册表，MC-101~108）：新增「CLI profile 注册表」类目 2 文件 26 用例——cli-profile-registry 18（CliProfileRegistry：register/get/getAll 注册序/同 id 覆盖（注册序不变）/matchByCommand 多 commands·带参·空命令行·仅空白·未命中·不 toLowerCase/`_reset`/独立实例/单例）、cli-profile-claude 8（claude 身份域字段 + CLAUDE_CLI_ID 常量一致性 + side-effect 注册 + 资源守卫泛化遍历——iconSrc 磁盘存在 + PNG 魔数，含 mockcli.png 先行资源）。删 3 文件 31 用例：tab-title-registry 13 / tab-rules 6（语义并入上述两文件）、cli-icons 12（资源守卫语义并入 cli-profile-claude）。use-xterm-lifecycle 71 / use-xterm-output 35 / use-xterm-integration 12 / terminal 14 / e2e-gating-terminal 5 用例数不变（断言与 mock 改指 profile 注册表）；L3 production-osc 8 用例数不变（OSC 133 复刻段改写，D-08）。L2 2333→2328（138→137 文件），全量 3079→3074。
- 2026-08-08（CLI 品牌 logo 显示，F9）：新增 1 文件——cli-icons 12（CliIconRegistry 注册表全分支 + public 文件/PNG 魔数守卫）；use-xterm-lifecycle 70→71（+1 OSC 133 C 未注册 CLI → logo null）、terminal.test.tsx 10→14（+4 tabLogo logoRef 状态机）、workspace-defaulttab 21→27（+6 tabLogo 渲染/顺序/仅随 emoji/动态双向/URL 并存）、agent-status-view 25→29（+4 行1 logo/图标列 40px 簇/行2 缩进 48px）、claude-history-row 16→19（+3 logo 仅随 status emoji/孤儿行不加图）；L3 production-osc 断言同步（OSC 133 C 携 logo）。L2 2303→2333（+30 实跑口径），全量 →3079。
- 2026-08-08（终端页签右键菜单重命名，F8）：新增 2 文件——terminal-rename-dialog 13（弹窗交互全分支）、terminal-rename-apply 5（applyRename 纯函数）；terminal.test.tsx 8→10（+2 customTitle 挂载恢复/参数同步）、workspace-header-actions 16→21（+5 C6-C10 重命名项结构/派发/禁用矩阵，C5 改 7 项结构）。L2 2279→2304（+25 实跑口径），全量 →3050。
- 2026-08-08（color-plan Stage 05 TST-04/05 测试补全与同步）：新增「主题/配色/基础」类目 2 文件——scheme-registry 18（TST-02 方案注册表单测：register/get/getAll/getDefaultId、setActive 已知/未知 id 回退 darcula + console.warn、getActive 默认、重复注册覆盖、`_reset` 隔离、darcula 四段完整性）、overrides 6（TST-03 dockviewVarStyle 20 键 + allotmentVarStyle 2 键 + editorTheme/editorColorOverrides + setActive 跟随）。colors 89→85（facade 化死配置清理移除 4 条 it.each 展开断言——13 块 + 7 组 each 展开 72，实跑口径）。L2 133→135 文件 2258→2278 用例（-4 +18 +6）。全量 3004→3024。TST-04 四文件（theme 13/main-bootstrap 1/gitshow-panel 21+hooks-config-jsonmode oneDark mock/L3 theme-options 5）git diff 零改动 + 逐文件跑绿验证通过。
- 2026-08-05（text-fix-plan Stage 17 DOC-01/02/03 全量校正）：①L1 按 GIT-12 拆分后重列（git/mod.rs `#[test]` 零残留 → tests/ 5 文件 97 + ci_config 1；各模块按 grep `#[test]` 实际计数更新：reader 36/spawn 48（conpty_custom 28 + 顶层 20）/shell 20/state 32/fs 31/notify 38/hooks 各文件/inject 34 等）——L1 449→571（23→28 文件）；②L2 改为 **npm test 实跑口径**（it.each/describeIpcContract 工厂展开计入），133 文件 2258 用例全绿实测（原 grep 块数口径 2060 少计 it.each 展开）；类目文件全面更新（新增 terminal-instance/webgl-setup/notification/ipc-window-contract/hooks-config-schema/commit 拆分 5 文件/main-bootstrap；删除 hooks-config-entry 已随菜单迁移删除）；③L3 116→138（新增 theme-options 5/production-osc 8/negative-ansi 9）；④L4 34→37（E2E-04 全屏 TUI 视觉回归 + E2E-06 真实 reporter 链路 + E2E-12 强杀 Job Object 各 +1 active，35 active + 2 skip）；⑤stale 清理（inject.rs 覆盖范围描述中随功能移除的陈旧条目删除；旧"L3 同时被 npm test 包含执行"口径注释修正为 L2/L3 独立运行）；⑥登记 DOC-01 既定豁免清单（8 项）+ DOC-02 定位声明（6 项）。全量 2658→3004。
- 2026-08-05（text-fix-plan Stage 11 HKC-10 展示分支补断言）：hooks-config-panel 20→21（+1 注入状态条查询完成前初始 "--" 帧断言——getInjectionStatus 挂起 Promise → data-e2e hooks-injection-status 文本「注入状态：--」+ 三态文案不出现）。L2 2059→2060，全量 2658→2659。
- 2026-08-05（text-fix-plan Stage 09 EXP-01~11 explorer 测试补全）：资源管理器 16→20 文件 213→252 用例（+39）。新增 4 文件——explorer-open-in-terminal 7（EXP-01 在终端中打开 addPanel 参数/无去重）、explorer-crud-success 4（EXP-02 删除/重命名/新建文件/新建文件夹成功路径——IPC+refresh+状态重置）、explorer-input-boundary 10（EXP-06 重命名/文件夹级新建输入框 Escape/空名/重名/失焦边界）、explorer-race-cleanup 6（EXP-07 旧请求延迟 resolve 丢弃/rootPath null 回调/fs-event 去抖卸载清理/file-saved 缺 path/卸载清理/gitStatus 过期丢弃）。扩展现有 4 文件——explorer-focus 3→6（EXP-04 focusin/focusout 上下文栈 spy）、explorer-selection 14→17（EXP-04 非选中行 hover enter/leave）、explorer-delete 19→22（EXP-04 横幅 dismiss/5s 自动消失/卸载清理；E6 编号 17-22 统一 + 标题与断言对齐 EXP-11）、explorer-file-viewer 16→19（EXP-10 无 dockviewApi/addPanel 抛错无孤记录/getPanel undefined 回退新建）。EXP-03：删除 useFileTree.fullRefresh 死代码（无调用方），F8 改名「mount 单次加载」+ 断言 readDir/gitStatus 各一次。L2 2020→2059（117→121 文件）。全量 2619→2658。
- 2026-08-02（Claude 历史会话 Stage 07 文档同步补登，F7）：L1 新增 claude_history 模块 4 文件 62 用例（jsonl 28 + scan 14 + ops 13 + mod 7——扫描/降级/env 覆盖/回退链/SEC-01 校验/追加写，计数为 grep `#[test]`）。L2 新增「Claude 历史会话」类目 6 文件 109 用例（model 37 + view 28 + hook 14 + input-dialog 12 + row 11 + restore 7）+ IPC 层 ipc-claude-history-contract 12（88→100，三命令四维验证）+ E2E 辅助 dialog-e2e-hook 3（Stage 06 漏登补登，24→27，ask 钩子守卫）+ agent-status-view 11→15（+4 三下拉框适配，Stage 05 产物补登）。L1 384→446（19→23 文件），L2 1783→1911（109→117 文件）。全量 2317→2507。
- 2026-08-02（Claude 历史会话视图 E2E，TE-01..04）：新增 describe「Claude 历史会话视图」8 用例——fixture 7 形态（custom-title/ai-title/prompt 回退/无 cwd/孤儿/agent-* 平铺/subagents 子目录）+ 搜索过滤 + 复制恢复命令（剪贴板 read_text 断言）+ 孤儿行 ✗ 双击无反应 + 重命名（副本尾部 custom-title 行 Node 断言）+ 删除（ask invoke 拦截降级方案 + 副本文件删除 Node 断言）+ 恢复编排（项目入列/页面切换/终端缓冲含 claude --resume）。新增 fixtures/claude-projects/（9 文件）+ run-wdio.cjs 副本重建与占位符替换 + SLTERM_CLAUDE_PROJECTS_DIR/SLTERM_E2E_PROJECT_DIR env 注入（SEC-02：只动 .tmp-claude-projects 副本）+ .gitignore 条目。L4 26→34（32 active + 2 skip），全量 2309→2317。
- 2026-08-01（Hooks 配置入口迁移）：删 global.openHooksConfig 快捷键命令（Ctrl+Shift+H）——hooks-config-entry.test.ts 整文件删除（7）、command-catalog 14→13（-1 入口契约）、global-commands 13（断言改 1）；新增侧栏右键菜单入口——sidebar-actions 33→38（+5 菜单入口）、新建 open-hooks-config-panel.test.ts（+5 同页单例/轮询/超时）。L2 1781→1783，全量 2307→2309。
- 2026-08-01（外部修改检测改 visibilitychange）：hooks-config-panel 19→20（+1 visibilityState=hidden 不触发；改 3 个 window focus 用例为 visibilitychange 派发——jsdom 需 defineProperty 设 visibilityState 再 dispatch，afterEach Reflect.deleteProperty 还原）。L2 1780→1781，全量 2306→2307。
- 2026-08-01（验收修复 5：卸载改 handler 级剔除）：inject.rs remove_slterm_matchers 组级→handler 级（混组保用户 handler，组空才删组）；20→22 用例（+2：混组保用户 handler/全 slterm 组删除）。L1 382→384，全量 2304→2306。
- 2026-08-01（验收修复 4：JSON 模式三修）：删自动补全（jsonCompletion + jsonLanguage.data.of + @codemirror/autocomplete 直接依赖移除，传递依赖仍在）；加 EditorView.theme height:100% + overflow:clip（竖向滚动条，验收 1.3）；hooks-config-panel 18→19（+1 错误提示单行截断断言，验收 1.2）；jsonmode 17（断言改 height theme，用例数不变）。L2 1779→1780，全量 2303→2304。
- 2026-08-01（验收修复 3：GUI 接入 HandlerForm）：hooks-config-gui 21→25（+4：选中渲染表单/字段编辑上抛新模型/切换选中表单跟随/托管 handler 表单只读）。L2 1775→1779，全量 2299→2303。
- 2026-08-01（验收修复 2：外部修改检测改 window focus）：hooks-config-panel 17→18（删 3 个 focusin relatedTarget 用例——机制移除；改/增 4 个 window focus 用例——可见触发/不可见跳过/面板内点击不触发/ask 弹窗期间回归不二次弹窗）。L2 1774→1775，全量 2298→2299。
- 2026-08-01（验收修复 1：删除单条启停）：删除 hooks-config-store.test.ts（21 用例）+ hooks-config-disable.test.tsx（10 用例）；model 22→17（删 filterDisabled describe 5 条）；sync 9→8（删 filterDisabled 保存链路用例）；panel 17（去失效记录条描述，用例数不变）。L2 1811→1774，全量 2335→2298。
- 2026-08-01（验收 #1 修复）：hooks-config-panel.test.tsx 15→17（+2 focusin relatedTarget 判定——面板内焦点转移不重读/面板外进入重读）。L2 1809→1811。全量 2333→2335。
- 2026-08-02（agent 侧栏 7 项人工审查修复）：L1：claude_history/ops.rs 13→7（删 rename 命令 6 条测试——问题 7 全链路移除），L1 446→440。L2：claude 历史会话 6 文件 109→115（model 37→40 +3 deriveActiveSessionStatuses——问题 2 四态同源；view 28→33 +5 受控 props/组收起计数/双击动作弹窗/标题覆盖/字号断言——问题 3/4/5/6；hook 14→13 -1 删 updateLocalTitle——问题 7；row 11→15 +4 四态标记/字号——问题 2/4；删 input-dialog 12 + 新建 action-dialog 7——问题 5/7）；agent-status-hook 35→36（+1 sessionId 字段——问题 6）；agent-status-view 15→19（+4 双行布局/相对时间——问题 1/4）；use-xterm-lifecycle 79→80（+1 HUK9 Notification status undefined——问题 2）；terminal-registry 15→18（+3 sessionId/status 存储与 merge——问题 2）；ipc-claude-history-contract 12→8（-4 rename 四维——问题 7）；colors 13 不变（断言 7→8 token 计数）。L2 1911→2013（117 文件）。L4：E2E 删重命名用例 +1 历史区四态用例（34 不变）；历史区行操作用例前置展开全部组（组默认收起——问题 3）。全量 2333→2603。
- 2026-08-01（Phase 3 Stage 10 全量重算）：按计数口径实跑重写。L1：新增 hooks/config.rs（18 用例，P3-BE 读写命令纯逻辑），usage.rs 23→28（+5 cache 字段用例），L1 359→382（18→19 文件）。L2：新增 hooks-config-entry（7）+ hooks-config-gui（21）+ hooks-config-sync（9），hooks-config-panel 9→15（+6 注入状态条/失效记录条）；panel-registry 29→32（+3 hooksConfig 六面板注册）、command-catalog 13→14（+1 openHooksConfig 入口命令契约）；修正「主题/配色/基础」类目标头 113→108（原与文件实际和 108 不符）。L2 1717→1809（106→111 文件）。L4：新增 hooks 配置面板保存链路（P3-TE-18），23→24 active（25→26 总，含 2 skip）。全量 2227→2333。
- 2026-08-01（Phase 3 Stage 07）：新增 hooks-config-disable.test.tsx（10 用例，P3-TE-15/16——禁用状态往返/失效禁用记录/事件树启停 checkbox）；hooks-config-handlerform.test.tsx 托管断言适配（lockRow 移除禁用 checkbox——C13-8 禁禁用=不渲染，P3-FE-19）。hooks 配置面板 8→9 文件 159→169。L2 1717→1727。全量 2217→2227。
- 2026-08-01（Phase 3 Stage 04）：新增 hooks-config-jsonmode.test.tsx（17 用例，P3-TE-09/10——CM6 渲染/schema 扩展注册/非法 JSON 校验上报/事件导航跳转/MatcherTester 试测）；hooks-config-panel.test.tsx 9 用例适配 JsonMode 接入（占位文案断言 → JsonMode value 传递断言）。补登 Stage 01-03 遗留 6 文件：ipc-hooks-config-contract（12）+ hooks-config-catalog（19）+ hooks-config-matcher（21）+ hooks-config-model（22）+ hooks-config-store（21）+ hooks-config-panel（9）。vitest.config.ts 新增 `server.deps.inline: ["codemirror-json-schema"]`（0.8.1 ESM dist 无扩展名相对导入，Node ESM 无法解析）。L2 1593→1717（99→106 文件）。全量 2093→2217。
- 2026-07-29（Phase 2 FIX-DOC-03）：Stage 01-04 完成后按 `npm test` 实跑重写。L2：ipc-hooks-contract 21→22（+1 ContextUsage 四字段键集合守卫）、notifications 32→25（-7 删 toast 路由化 6 用例+首 token/sendToastNotification 适配）、agent-status-hook 31→35（+4 行建模新语义——纯 shell 无行/双通道建行/三通道删行/初始扫描携 transcriptPath 拉 usage/reconcile 对账/cache 字段/contextUsage 静默 catch 可观测）、agent-status-view 11 不变（用量新口径重算）、tab-title-registry 8→13（+5 首 token 匹配——带参命中/空命令行/仅空白/首 token 无规则）、terminal-registry 7→15（+8 setClaudeSession 全分支+sessionChange 事件+幂等保留旧 session）、terminal-registry-subscribe 3→7（+4 sessionChange 通知/setClaudeSession 触发 sessionChange）。L2 1578→1593。L4：Agent Status 静态行反转（纯 shell 无行）+ 动态四态首个信号即建行 + 新增 R2/R3/R4 变体 3 条防复发；22→23（21 active + 2 skip）。全量 2075→2091。
- 2026-07-28（Phase 2 FIX-DOC-01）：按实跑全量重写。L1：mod.rs 剔除 hooks_context_usage（不存在）、usage.rs 改正函数名为 parse_usage_line/scan_transcript_usage。L2：ipc-hooks-contract 16→21（+5 条 contextUsage 合约 + HookEventPayload 字段约束）、notifications 33→32（-1，剔 4 项不存在描述并补 sendClickableNotification 回调绑定/onclick 聚焦路由）、agent-status-hook 21→31（+10，剔"轮询"改事件驱动 + FE-03 TerminalRegistry.subscribe 增删 + FE-04 dockviewApi 标题查找与回退 + FE-05 null 不覆盖 + FE-06 无额外订阅）、agent-status-view 8→11（+3，剔 tooltip/加载态/错误态 + 补用量条分段颜色断言 + 点击路由 switchToPageAndFocus）、colors 12→13（+1 AGENT_STATUS_USAGE_COLORS 三 token）。补登 2 新文件：panelId.test.ts（5）+ terminal-registry-subscribe.test.ts（3）。L2 1552→1578，97→99 文件。L4：Agent Status 原 skip 拆为 3 条 active（视图存在性 + 静态行渲染 + 动态四态信号文件），toast 保持 it.skip，CSP 脚本用例保持 skip；18→22（20 active + 2 skip）。全量 2045→2075。
- 2026-07-27：Stage 2 通知/Agent 状态——L1 hooks 模块拆分为 5 文件（mod.rs 10→8、inject.rs 12→20、新增 signal.rs 9 + watcher.rs 6 + usage.rs 23），L1 274→318。L2 IPC 层 ipc-contract 53→65（+12：notification + hooks_context_usage 合约）、ipc-hooks-contract 8→16（+8：hooks_context_usage）；新增「通知/Agent 状态」类目 3 文件 62 用例（notifications 33 + agent-status-hook 21 + agent-status-view 8），L2 1415→1497。L4 新增 Agent Status 视图 1 用例，17→18。全量 1822→1949。
- 2026-07-26：hooks 宿主侧增强（P1-DOC）——L1 新增「hooks」模块 2 文件 22 用例（mod.rs 10 + inject.rs 12）+ pty/spawn.rs 28→29（+1 env 注入）。L2 新增「hooks」IPC 合约 1 文件 8 用例（ipc-hooks-contract）+「claude-status」纯函数 1 文件 14 用例；终端面板 use-xterm-lifecycle 71→77（+6 hook-event 过滤/F3 四态）。L4 新增 hooks 注入/信号 2 用例，15→17。L1 251→274，L2 1387→1415，L4 15→17，全量 1769→1822。
- 2026-07-19：commit 视图（CV-DOC）——L1 git/mod.rs 62→70（+8：git_file_at_head/recurse_untracked_dirs/oldPath/rename_detection），L1 243→251。L2 新增「Commit 视图」类目 1 文件 28 用例 +「Diff/GitShow 面板」3 文件 40 用例（diff-alignment 16 + diff-panel 11 + gitshow-panel 13），既有文件增量：ipc-contract 50→53 + git-gutter 20→28 + panel-registry 23→29 + title-manager 36→44 + workspace-file-panel-types 11→13。L2 1207→1387。L4 新增「commit 视图」describe 2 用例，14→15。全量 1580→1769。
- 2026-07-19（fix）：跨区拖拽修复 + 中线 zone 判定 + splitRatio 重置——activityBar 16→29（+13 跨区/边界/清理），sideBarArea 13→14（+1 splitRatio 重置）。L2 1193→1207，全量 1566→1580。
- 2026-07-19：侧栏视图系统（SB-26）——新增 L2「侧栏视图」类目 6 文件 118 用例（sideBarState 50 + sideViewRegistry 7 + sideBar 19 + activityBar 16 + sideBarArea 13 + workspace-sideviews 13）；L4 新增「侧栏视图」describe 2 用例（12→14）。L2 1075→1193，全量 1446→1566。
- 2026-07-18：DBG-11 同步——纳入 Stage 1/2 新增用例（DBG-4 契约守卫 3 条、DBG-9 switchToPage 时序 14 条、DBG-10 explorer-sandbox-race 13 条），L2 1045→1075，全量 1416→1446。
- 2026-07-17：重写——实测全量用例数（L1=243, L2=1045, L3=116, L4=12），统一计数口径，标注 E2E 键盘局限，声明唯一真值源。纳入 Stage 9/10 新增用例。
- 2026-07-13（旧版）：全量 ~1234 用例（Rust 193 + 前端 1020 + L3 9 + E2E 12），计数失实且 L3 少报 107 用例。
