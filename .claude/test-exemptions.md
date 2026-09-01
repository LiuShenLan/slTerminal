# 自动化测试豁免与定位

> **本文档职责**：豁免登记处 + 测试定位声明 + 条件跳过说明。用例计数与测试文件导航不属本文档——凡能通过阅读代码直接理解的信息（文件清单、用例数、入口路径）不在此重复（代码自证原则，ADR-0011）。

> **覆盖纪律（硬约束 #11）**：改动的代码可自动化部分必须添加全量自动化测试用例覆盖；不可自动化部分须在本文档既定豁免清单登记（登记处即本文档），注明豁免原因与当前兜底层级，禁止未登记豁免。

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
| win11/win10 真实终端 conda 激活实测（profile 加载链路 + conda 钩子 + prompt 包装链） | 依赖真实 conda/miniforge 环境与交互会话，CI 无此环境 | L1 B17 参数守卫（`pwsh_args_no_noprofile_b17`）+ 双系统 debug build 人工实测 | B17 |
| settings.json corrupted 警示条（L4） | 写坏 settings.json 需沙箱外写文件（E2E 无命令通道），真实损坏无法在 E2E 会话内构造 | L2 覆盖（`settings-panel.test.tsx` loadSettings mock 渲染/关闭）+ 人工实测（手改文件损坏重启） | SC-E2E-02 |
| ~~`cargo test` 门禁（F12 起）~~ **已修复（2026-08-31 当日翻案）** | 初判环境 bug；当日定位真根因：测试二进制无 manifest → SxS 未激活 comctl32 v6 → 系统解析到 v5 → tauri 栈静态导入的 `TaskDialogIndirect`（v6 导出）缺入口 → 0xC0000139 零输出崩溃（与 TQ-COV-06 8-23 预防性登记同因，彼时 `rustc-link-arg-tests` 对默认 lib test 目标不生效故从未真正激活）。修复：Cargo.toml `[lib] test = false` + 显式 `[[test]] lib_tests`（path = src/lib.rs）——使 `cargo:rustc-link-arg-tests`（存量 build.rs）真正作用于 lib 单测目标，manifest 嵌入生效 | 全量 `cargo test -- --test-threads=1` 实测 827 例全绿（711 lib + 116 集成，2026-08-31） | 2026-08-31 实证登记 → 当日翻案 |
| background_tasks spawn/emit 包装层（`spawn_poller` 循环本体与 `background_tasks_set_config` 命令包装层的 emit/重 spawn 分支） | 需 `AppHandle` 与 tauri runtime（async_runtime spawn/事件发射），L1 无法直测；可测部分（`set_config_core` 校验→落盘→内存链、registry 解析钳制）已 L1 全覆盖 | L4 勾选启停端到端（`background-tasks.e2e.ts` C）+ 人工实测（运行中改配置观察 poller 生效） | BE-02 |
| tick 失败静默 E2E 豁免（E2E-03 用例 G） | tick 失败需后端扫描故障注入通道，E2E 沙箱内无可控注入手段 | 调度器 L2 用例（`background-tasks-scheduler.test.ts` 失败处理：tick 失败快照不变/manual 失败置 error）+ 人工观察 | E2E-03 |
| `ExplorerPanel.handleRename` 同名兜底短路分支（oldPath === newPath） | UI 不可达——同名已在 `FileTree.confirmRename` 拦截（比较 basename），测试无法直传同名进入 onRename；属防御层死代码 | confirmRename 同名短路 L2 用例（explorer-rename-state 3 例）+ 后端 src==dst 幂等 L1 用例（`fs_rename_src_equals_dst_*` 2 例）双边锁死同层语义 | 修复「重命名取消误删文件」登记 |

> 原豁免表中 `FileWatcher::start`/`notify_watch` 与 `claude_history` 命令包装两项已按 D6 从豁免重分类为补测，不再列入豁免表。  
> **SEC-17 豁免已撤销（TQ-COV-05 翻案）**：`tracing::warn!(target: "audit")` 已由 `tracing-test` 断言锁死，豁免行删除。

## 定位声明（DOC-02）

| # | 层级 | 定位声明 | 来源 |
|---|------|----------|------|
| ① | L3 | **网格状态正确性，非渲染正确性**——headless 不跑 WebGL/GPU/onContextLoss，生产 `terminalOptions`/OSC handler 的验证限于 headless 网格可观察语义 | E2E-04 |
| ② | L4 | **半端到端/部分端到端**——键盘（合成 keydown 非 OS 按键）、拖拽（store helper 非真实 DnD）、恢复编排（断言到 `pty.write` 注入，不含真实进入会话）三类用例唯一不真实处是事件来源/前置动作 | E2E-11 |
| ③ | L2 | **jsdom 模拟**——postMessage origin/source 校验在 jsdom 无法代表真实 WebView2，负面用例守护 JS 侧逻辑，真实行为由 L4 验收 | IHE-03 |
| ④ | L2 | **term.input 间接验证**——`term.input(...)` → onData 断言是 xterm.js 内部转换的间接验证，键盘链路由 L2 `attachCustomKeyEventHandler` 委托 + L4 半端到端兜底 | 06 #14 |
| ⑤ | L2 | **E2E helper 行为契约**——验证 `__slterm_e2e_createProject` 等 helper 的契约，非真实 App 初始化逻辑 | 13 P-14 |
| ⑥ | L2 | **浅层组件定位**——`editor.test.tsx` mock `useCodeMirror`，定位为组件集成契约测试，真实编辑器行为由 `use-code-mirror.test.ts` 等覆盖 | 07 G1/G2 |

## 条件跳过用例（有效覆盖依赖 runner 环境）

以下用例依赖 Windows 应用执行别名或 symlink 创建权限，环境不满足时跳过但仍计「通过」：
- `pty/shell.rs::allowlist_accepts_real_alias_when_present`
- `hooks/signal.rs::process_symlink_signal_deletes_without_read`、`hooks/watcher.rs::collect_excludes_symlink_files`、`notify/mod.rs` symlink 两用例、`agent_history/claude/ops.rs` symlink 三用例（BE-17/D5 豁免先例）

本地开发机（已开开发者模式）为真实覆盖来源；CI runner 未开权限时上述分支覆盖记为「不确定」。

## L4 运行机制

### 用户目录隔离机制（FIX-TE-04 + E2E-05 扩展）

`run-wdio.cjs` 启动时备份 `~/.slterminal/settings.json`、`~/.claude/settings.json`、`~/.slterminal/hooks/`（`.e2e-bak`），exit 时还原；`~/.slterminal/hooks-events/` 直接清理。E2E 不触碰真实 `~/.claude/projects/`（`SLTERM_CLAUDE_PROJECTS_DIR` 指向临时副本）。详见 `e2e-tests/CLAUDE.md`。

### E2E 键盘输入限制（半端到端，TE-17）

embedded WDIO 无法投递 OS 级按键；所有键盘用例改用页面内 dispatch 合成事件 → ShortcutRegistry → 命令 handler → 真实 IPC。唯一不真实处是事件来源。
