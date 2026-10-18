# 重构遗留问题清单（M3–进行中）

> 重构期间各子步 agent 登记的问题、待裁决项与优化点汇总。重构完成后逐条处理。
> 定位一律用符号名/文件路径（禁行号）；来源标注里程碑与子步。
> 状态图例：**待裁决** = 需要用户拍板；**待修** = 已知缺陷/优化，未修；**计划内** = 归后续子步，届时验证。

## A. 待裁决（需要用户决定）

| # | 问题 | 来源 | 建议 |
|---|------|------|------|
| A1 | **pty_integration 负载瞬态（两类）**：①`conpty_spawn_write_read_resize_close_full_chain` 15s 写读窗口在全量串行首次运行超时（Defender 扫 staged exe 同源），出现 ≥2 次；②`spawn_lock_serializes_and_capacity_32_enforced` 在全量 workspace 跑（slterm_app 千余测试之后）偶发并发抢名额断言失败（曾见「2 成功 vs 期望 1」），单跑/复跑均绿。 | M5.4/M4.5/M9.5 观测 | ①窗口 15s → 30s 注释负载裕度；②需排查是否有跨测试 PTY/线程残留影响容量判定（按 CLAUDE.md 负载裕度口径需批准，且②应先定位再改）。 |
| A2 | **设置类写盘在 UI 线程同步执行**（滑条松手/下拉/开关/主题保存）：与硬约束「阻塞 I/O 用 spawn_blocking」存在张力。上游同形，小文档写、回滚语义依赖同步失败返回。 | M6.2/M6.3 | 接受现状并登记豁免，或改后台任务（相位机随之引入）。 |
| A3 | **代码注释篇号锚点 vs docs/ 禁引纪律**：M3 起落盘注释大量引用设计稿篇号（如「05 篇三件套锚点」），与「仓内（docs/ 外）禁止引用 docs/ 任何文件」有张力。 | M3 收官登记 | 二选一：M11 删稿时统一清理篇号（sed 可机械化），或把纪律收窄为「禁引用文件路径，篇号锚点允许，M11 一并清」。 |
| A4 | **`-e` 第二实例不移交**：带显式命令的第二实例按设计稿不开控制面、开 client-only 窗口，与「单实例」定位有张力。 | M7.5 | 维持现状（-e 视为独立作业）或改移交语义，需产品裁定。 |
| A5 | **git 四方法只对根 checkout 生效**：`verify_repository` 要求 pane cwd == repo root，子目录 pane 被 `git_target_changed` 拒绝。 | M7.3 | 如需子目录支持，重新裁定该条（放宽为「cwd 在 repo 内」并解析相对路径）。 |
| A6 | **`fallback_api_key` 默认不可达**：预设默认全启用且 active 指向首个预设，「无启用 provider 才走 env 兜底」在默认安装不可达。M9.5 按设计稿原语义实现（active 无凭据 → 请求静默放弃，不落 env 兜底），M10.9 前建议条实际不会进 Ready（测试均走注入）。 | M9.2/M9.5 | **仍未裁**：预设默认关，或 active provider 无凭据时继续走 `SLTERM_AI_KEY`/`OPENAI_API_KEY`。 |
| A7 | **PathEditor 无补全**：任务曾写「补全」，设计稿无补全合同（候选来源/弹层/键盘/提交语义未定义），实现按设计签名（输入/校验/提交）。 | M8.4 | 若产品需要，先在设计稿补合同再落 UI。 |
| A8 | **R6-R8 与双槽比例**：旧栈按钮跨区/同区排序（R6-R8）与 zones 持久化未迁；双槽固定 50/50（旧栈持久化过 splitRatio）。 | M5.3b | 若保留旧交互需扩 SideViewDef/schema；比例字段需先裁。 |
| A9 | **分屏不继承锚 pane 的 `-e` 身份**（照上游行为）。 | M5.3a | 若产品要求继承，归启动身份设计（M5.5 已过）另立。 |
| A10 | **recovery 恢复结果无 toast 汇总**（仅 log::info）。 | M5.5 | 通知漏斗归 09 篇 M10.1，届时一并接。 |
| A11 | **hook 失败边默认关的载体**：设计未定义独立键，实现复用 `AssistantConfig.enabled`（与 D08-2「与 AI assistant 默认关闭一致」）。 | M9.3 | 维持复用；若需独立 `hook_failure_edges` 键另行裁决。 |
| A12 | **config_guard 锁竞争不计入失败边**：另一操作进行中不算失败，不投递；`RawSettings::try_load` 失败投递 `slterm-hook settings`。 | M9.3 | 维持边界；若要求锁竞争也计入需裁决。 |
| A13 | **recovery 汇总走直发通道而非通知漏斗**：启动期窗口前台，漏斗门控会丢该信息，故走 `platform::notifications::show`（诊断/更新提示直发通道）。 | M10.1 | 若要求统一走漏斗，需加「启动汇总豁免前台门控」策略位。 |
| A14 | **`silent_start=1/tray=0` 为惰性组合**：实现保持设计稿纯两条件（不隐藏），上游会在开启静默时强制 tray=1。 | M10.6 | 是否在设置页加「开启静默同时补 tray」的 UX 约束。 |
| A15 | **Acrylic 增强路径未迁**：上游 22H2 优先用 DesktopAcrylicController，本仓无该模块/Windows App Runtime 依赖，Acrylic 仅走 WCA state 4 回退。 | M10.7 | 是否补移植（新增依赖面）需裁决；实际观感取决于 WCA。 |
| A16 | **审计写事件噪声**：每次设置写发 warn（含主题 live 预览等高频写路径）。取证价值与日志量需平衡。 | M10.12 | 按操作分级，或预览写单独 source 降为 info（涉及审计可观测面口径）。 |
| A17 | **热路径 O3 与旧体积包络冲突**：启用设计指定 O3 后 setup 17.07 MiB（+12.9%）、zip 22.21 MiB（+15.1%，略超按 M11.1 基线换算的 22.2 线）。当前按 D12-2「全量功能态实测 +15%」取校准态预算 19.63/25.54 MiB。 | M11.2 | 接受校准态预算，或缩减 O3 清单（候选：slterm 16 codegen units、image/resvg）回到旧包络。 |

## B. 待修（已知缺陷/优化）

| # | 问题 | 定位 | 来源 | 建议 |
|---|------|------|------|------|
| B1 | **测试真实创建 `%APPDATA%\slterm`**：`data_dir()` OnceLock + `create_dir_all`，lib_tests 中 detect/identify 与屏幕用例会写真实用户目录。 | `platform/dirs.rs` `data_dir`、`ai_agents.rs` `override_path` | M4.4/M4.5 | 测试专用数据目录覆盖根，或进程启动前注入 tempdir。 |
| B2 | **`PipeServer::shutdown` 等不 EOF 客户端**：若客户端保持连接且不关闭，join 可能等下去（helper 恒关闭，低风险）。 | `ai_hook/windows.rs` `PipeServer::shutdown` | M4.5 | 硬化时加读超时/overlapped I/O。 |
| B3 | **`restore_defaults_at` 绕过 SAVE_LOCK**：重置与保存并发存在丢失更新窗口（单次写仍原子）。 | `slterm_settings/src/reset.rs` | M6.1 | 重置走 app 写通道（导出 `with_save_lock` 或把 reset 写盘入口上移）。 |
| B4 | **`RawSettings::try_load` 根非对象返回 `Ok(空树)`**：与写通道/读三态判损坏不一致；ai_hook config_guard 授权路径使用。 | `slterm_settings/src/lib.rs` `try_load` | M6.1 | M6.5 后评估授权路径是否也应 Err（M1 语义有既有用例锁定）。 |
| B5 | **`settings.corrupted.json` 固定名覆盖**：重复损坏会覆盖上一份隔离件（与 session.crashed.json 同形）。 | `settings_store.rs` quarantine | M6.1 | 若需多代诊断加时间戳。 |
| B6 | **整窗「保存全部」部分失败无聚合提示**：失败横幅只在各自编辑器内，聚焦 tab 未必可见。 | `workspace/documents.rs` `guard_file_window_close` | M8.5 | workspace 根显示聚合 notice。 |
| B7 | **键位仅启动应用，无热重绑**：keybind 段在 `workspace::init` 读一次；设置页无键位编辑 UI。 | `workspace/keyboard_bindings.rs` `install_user_keybinds` | M8.5 | 设置面接入时加变更通知→重建 keymap（处理精确 Unbind）。 |
| B8 | **外部文件修改不刷新 gutter/目录树**：同步触发点仅打开/保存/reload；目录树无 fs-notify 自动重扫（上游裁定不轮询）。 | `file_editor/gutter.rs` `read_for_path`、`side_panel` | M8.4/M8.5 | 定 UI 唤醒策略后接 fs 事件监听。 |
| B9 | **gutter 每编辑器各自 spawn git**：每个打开/保存/重载各跑 `rev-parse`+`status`+`diff`，未复用文件树快照。 | `file_editor/gutter.rs` `read_for_path` | M8.5 | side_panel 快照扇出，或给 git 模块加短 TTL 快照缓存。 |
| B10 | **冲突三栏合并器未交付**：Conflicted 行标点击当前与普通 mark 一样开双栏 diff。 | `code_tab.rs` `GutterClicked.mark` | M8.5 | 单列后续步骤；事件已可分流。 |
| B11 | **FileTreeContextMenu 仅删除项**：explorer CRUD（新建/重命名）未做，菜单骨架（挂 workspace 根/遮罩）已就位。 | `workspace/file_tree.rs` `render_file_tree_context_menu` | M8.4/M8.5 | 按设计改造节 6 补 CRUD。 |
| B12 | **`tab.close` 遇脏文件先回成功**：改为异步弹确认后调用方立即收到成功但 tab 未关。 | `workspace/runtime_control.rs` | M8.2 | runtime 契约增加 force/确认等待语义。 |
| B13 | **atomic_file 替换不保留 ACL/属性**：上游把权限复制到临时文件。 | `atomic_file.rs` `write` | M8.2 | Windows 常规文件无感；有 ACL 诉求时扩展。 |
| B14 | **serve 线程无停止通道**：Drop 只删端口文件；测试进程每次 `spawn_at` 留一个阻塞监听线程。 | `runtime_api/server.rs` | M7.5 | 01 篇 P 表线程生命周期补 stop 句柄。 |
| B15 | **git HEAD 错误统一 `NotInHead`**：`git show` 失败统一归 NotInHead，权限/仓库损坏被误标。 | `git/mod.rs` `GitError` | M8.1 | 预检 `cat-file -e` 或 stderr 分类细分。 |
| B16 | **本地图片 URL 带查询串误判缺失**：`a.png?x=1` 被直接 `base.join(url)` 后当缺失画占位。 | `file_editor/structure_inline_view.rs` `render_inline_image` | M8.3 | 需要时先剥离 query/fragment。 |
| B17 | **chrome 缩减**：工具条 reader-focus/全屏/远程取消未迁（当前为源码↔预览/详情/保存）；详情面板内嵌编辑器、未做 workspace 右栏宿主。 | `file_editor/chrome.rs` | M8.3 | 若工作区右栏宿主需要，回接。 |
| B18 | **编辑器高级字段控件不全**：排版仅字体/字号/行高/不透明度可编辑；字距/字重/layout/blur 保留模板值。 | `settings_pane/theme_editor.rs` | M6.3 | 随 theme_advanced 一起补。 |
| B19 | **主题导入导出 UI 未接**：相位机（Idle/Picking/Inspecting/Committing/Done）未实现；包 API（CheckedPackage/export_package）已就绪。 | `theme_library/package` | M6.3 | 接 UI（M6.5 未做）。 |
| B20 | **侧栏槽标题中文硬编码**：`SideViewId::title`/`content_owner` 未收编 catalog，切语言不改变侧栏标题。 | `workspace/sidebar.rs` | M6.4 | 视图落地时改用 cached `Settings.language` + typed Message。 |
| B21 | **6 家 agent_dirs 目录值待实测复核**：goose/devin/antigravity/pi/maki/kilo 按官方文档取值。 | `agent_dirs.rs` 表 | M4.4 | 07 篇沙箱接线前本机实测复核。 |
| B22 | **`record_hook_session` 签名分歧**：设计稿带 `session_file`，上游实为 cwd；已按设计稿落，project 在 scan 时从文件头补。 | `ai_sessions.rs` | M4.4 | M5.5 恢复注入若只拿 cwd 而需 project 副信息，确认是否补第二入参。 |
| B23 | **`agent.start` kind 语义宽于 Schema**：实现接受更广 `AgentKind::parse`，未核实冷启动返回 `agent_launch_unsupported`（非 `invalid_params`）。 | `runtime_api/agent_api.rs` | M7.3 | 语义合理但 Schema 面更窄，待评审统一。 |
| B24 | **git 内部错误码超出 04 篇错误码表**：`not_a_git_worktree`/`git_query_failed`/`branch_create_failed` 等。 | `git_worktree.rs` | M7.3 | 07/M11 统一收敛。 |
| B25 | **close_pane 无 busy_child 确认框**（上游有 request_close_pane 确认）。 | `workspace/mod.rs` `close_pane` | M5.3a | 确认交互归后续篇（busy_child 判据归 02）。 |
| B26 | **describe.commands 与 CLI 命令清单各自手写**：无交叉一致测试，存在漂移风险。 | `runtime_api/transport.rs` `runtime_description`、`shortcuts.rs` | M7.4 | 补「describe 资源动词 ⊆ catalog 且 catalog 覆盖全部资源动词」用例，或 describe 由 catalog 派生。 |
| B27 | ~~**`SCHEMA_PATH` 与 SKILL `include_str!` 指 `docs/`**~~（M11.1 已迁根级 `assets/`，锚点与 SKILL 指引同步改完；M11.3 复核 `SCHEMA_PATH` 指 `assets/runtime-api-v1.schema.json`）。 | `runtime_api.rs` `SCHEMA_PATH`、`local/runtime_skills.rs` | M7.4 | 已闭环。 |
| B28 | **clap caret 实解析 4.6.7**：声明 `4.2.7` 为 caret。 | `slterm_app/Cargo.toml` | M7.4 | 若 D04-2 要精确基线改 `=4.2.7`。 |
| B29 | **`commands_handle` 探 powershell 5.1**：枚举 cmdlet/函数/别名用 `powershell.exe`，产品默认 shell 是 pwsh 7，可能漏 pwsh 独有命令。 | `suggest.rs` `commands_handle` | M9.1 | M9.5 评估改探 pwsh 或双探。 |
| B30 | **`serde_yaml 0.9.34+deprecated` 依赖**（连带 unsafe-libyaml）：忠实照抄上游。 | `completion/project_scripts/workspace.rs` | M9.1 | 后续以窄解析器替代。 |
| B31 | **`scoped_history` 死分支**：单态下 git/scripts 失败折叠为 `Some(Vec::new())`，兜底路径不可达（上游远程分支残留）。 | `completion.rs` `Request::calculate` | M9.1 | 收敛删除或改语义。 |
| B32 | **provider remove 先删凭据后判存在性**：删除不存在 id 也发一次凭据删除调用。 | `ai_providers.rs` `remove_provider_at` | M9.2 | 影响极小；严格则先查 id 再删。 |
| B33 | **`codex_goals`/`codex_remote_compaction` 未迁**：设计字段清单未列、无消费方。 | `ai_providers.rs` | M9.2 | 若 06 设置页需要 Codex 开关，M9.5/M10 再补。 |
| B34 | **`unborn HEAD` unstage 无 L1 覆盖**：实现走 `git rm --cached`，用例只覆盖已提交 HEAD。 | `git/mod.rs` `unstage` | M8.5 | 补 unborn + staged 新文件用例。 |
| B35 | **gutter 行标像素对齐近似**：用输入实体上一帧 scroll_offset/line_height + 8px 行号留白定位。 | `file_editor/gutter.rs` | M8.5 | 已登记 test-exemptions；后续优先用组件装饰集合/布局快照。 |
| B36 | **`TestAppContext::test_window` 私有**：L4 OS 拖出断言改为解析器注入探针，`external_drag_files` 真断言受限。 | `ui_tests/file_tree_tests` | M8.4 | 上游 API 变化时补真断言。 |
| B37 | **`Execution::prepare()` 恒 true**：若未来恢复远程/嵌套执行环境需重新引入冻结握手（设计已裁，仅记录接缝）。 | `completion/metadata.rs` | M9.1 | 留锚。 |
| B38 | **失败边合成命令偏技术化**：`HookFailure.command` 形如 `slterm-hook install slterm_ai_hooks_claude`，只宜作上下文不宜直接呈现。 | `ai_assistant.rs` `HookFailure` | M9.3 | M9.5 呈现层只作上下文，不直出。 |
| B39 | **`terminal_math/tests.rs` 1970 行贴近 2000 硬限**：后续追加用例前须先拆分。 | `terminal_math/tests.rs` | M9.4 | 拆子模块（按 M8.2 ui_tests 先例）。 |
| B40 | **ghost 仅按「光标列+1」绘制**：未考虑 `suggestion_edit.replace_after_chars` 与自动折行（纯视觉小瑕疵）。 | `element.rs` ghost 绘制 | M9.5 | 按 suggestion_edit 替换区间绘制。 |
| B41 | **公式编译在 paint 的 Term 锁内**：首次遇多条新公式可能拉长一帧持锁时间（上游同构）。 | `math_overlay.rs` `plan_frame` | M9.5 | compile 移出锁外或异步预热。 |
| B42 | **弹窗与 AI 条并存时底部空间受限**（不互相覆盖）。 | `view.rs` 弹窗/AI 条布局 | M9.5 | 低优先级布局打磨。 |
| B43 | **`NotificationChoice` 暂无消费方**：动作按钮回传事件已通车，确认语义归 M10 安全半程，当前仅 debug 日志。 | `notify.rs` | M10.1 | M10 安全半程消费。 |
| B44 | **焦点切换不清任务栏进度**：切换活跃 tab/聚焦 pane 时旧条保留，后台 pane 后续进度被忽略，只能等来源 pane 清除边（TurnDone/133;D/关 pane/OSC 0）。上游同行为。 | `workspace/mod.rs` 焦点路由 | M10.2 | M10.3/M10.4 裁决是否焦点切换补清。 |
| B45 | **taskbar 测试调用记录用进程级 static**：依赖 `--test-threads=1` 既定纪律；若并行需加测试互斥锁。 | `taskbar.rs` | M10.2 | 记录。 |
| B46 | ~~托盘/通知 Focus 未 SW_RESTORE~~（M10.4 facade 已补 `IsIconic→SW_RESTORE→前置`）。 | `windowing/focus.rs` | M10.3 | 已闭环。 |
| B47 | ~~托盘「退出」绕过脏文件守卫~~（M10.4 已过 `guard_file_window_close`，`tray_quit_confirms_dirty_files_before_exiting` 锁定）。 | `workspace/runtime_control.rs` `quit_from_tray` | M10.3 | 已闭环。 |
| B48 | **托盘 CreateWindowExW 失败闸门不复位**：失败后本进程内不再重试（上游同形）。 | `tray.rs` `THREAD_STARTED` | M10.3 | 可重置闸门重试。 |
| B49 | **托盘 shutdown join 等模态菜单收合**（罕见，可接受）。 | `tray.rs` `shutdown` | M10.3 | 记录。 |
| B50 | **标题栏关闭未收 facade**：`apply_titlebar_command` Close 分支（非 Windows/测试路径）仍是 save+remove，绕过脏守卫；Windows 生产由原生控制区走 `on_window_should_close`（风险低）。 | `workspace/title_bar.rs` | M10.4 | 05 标题栏重建时补 `close_window` facade 入口并复用 B47 守卫。 |
| B51 | **`window_hwnd(cx)` 的 `cx` 未参与实现**：单窗用开窗时缓存；未来多窗须改为按 `cx` 活动窗口查询。 | `windowing/focus.rs` | M10.4 | 留锚。 |
| B52 | **更新提示不调 `mark_prompted`**：更新弹窗步落地前，同一版本每次启动都弹 toast。 | `update_check.rs` `deliver` | M10.5 | 弹窗接线时在打开/关闭处调 `mark_prompted`（`remind_later`/`skip_version` 同理）。 |
| B53 | **`restore_ticket()` 未接启动恢复**：`acknowledge_restore` 无生产调用方。 | `update_download/handoff.rs` | M10.5 | 接 `session_recovery::prepare_startup` 前，窗口建好后确认窗口数。 |
| B54 | **托盘 NIM_ADD 失败无重试**：explorer 未就绪/刚崩时 `added=false`，`set_enabled(true)` 状态未变不 post，1Hz 快照不变也不刷新——**静默启动下可能长时间无托盘入口（窗口已隐藏）**。 | `tray.rs` `set_enabled`/`apply_enabled` | M10.6 | `set_enabled(true)` 每次 post WM_APP_SET（或维护 mounted 标志）让 1Hz 自然重试。 |
| B55 | **壁纸设置 UI 缺失**：只有 opacity 滑块/blur 下拉，无 background_image 路径选择与 fit/alignment 控件；目前只能经 settings.json 或主题写入。 | `settings_pane.rs` | M10.7 | 归 06 篇补壁纸选择 + 预览控件（`load_preview` 已备）。 |
| B56 | **终端默认背景仍是壳底色 token**：网格区未绘制 `Settings.palette.background`，默认格透出 Root 壳色。 | `view/element.rs` 容器底色层 | M10.7 | 02/06 篇若引入独立终端卡底色，在卡层之上、单元格之下补单层（直接叠加会二次 alpha）。 |
| B57 | **live opacity 预览与并发 Changed 边界**：拖拽期间恰有其它设置触发 `Changed`，`wallpaper::refresh` 会用已持久化值覆盖预览（概率低）。 | `settings_pane/persistence.rs` `preview_opacity` | M10.7 | live 值并入全局或给 refresh 加预览旁路。 |
| B58 | **禁用态循环语义待裁定**：有订阅但 `enabled=false` 时看门狗循环仍按节拍唤醒跳过执行（自愈形态）；旧后端禁用即退出。当前无任务配置 UI 不可达。 | `workspace/pumps.rs` 看门狗 | M10.8 | 06 接入禁用/启用时裁定是否保留该形态。 |
| B59 | ~~**凭据域「业务只经 secrets 门面」无编译期/扫描门禁**：`platform::credentials` 为 `pub(crate)`，旁路仅靠文档纪律。~~（M10.12 已加 `platform_credential_backend_is_reachable_only_through_the_secrets_facade` 源码扫描守卫）。 | `platform/credentials.rs` | M10.9 | 已闭环。 |
| B60 | **`SecretBytes::as_bytes` 调用点白名单未守卫**：设计测试点要求「域内借出仅限白名单调用点」。 | `secrets.rs` | M10.9 | M10.11 落 as_bytes 调用点扫描断言。 |
| B61 | **真 Credential Manager 用例依赖用户凭据库**：默认随 lib_tests 运行（隔离 target + Drop 清理）；无凭据库的服务账户 CI 会失败。 | `secrets.rs` 测试 | M10.9 | 若 CI 化改环境变量门控并登记豁免。 |
| B62 | **备份 `add_themes` 非法主题静默跳过**：仅 audit 日志，备份会缺该类文件；若要硬失败需改。 | `encrypted_backup.rs` `add_themes` | M10.10 | 安全边界内取舍，登记。 |
| B63 | **`SENSITIVE_KEYS` denylist 未来漂移**：provider/settings 新增字段若取名 `token` 等会被静默剥除；防漂移用例只扫 settings KEYS。 | `encrypted_backup.rs` `SENSITIVE_KEYS` | M10.10 | provider schema 演进时补同类漂移守卫。 |
| B64 | **plan_balance 订阅者漏唤醒窗口**：订阅者在「快照落槽后、通知扇出前」注册会漏一次唤醒；消费方须订阅后先读 `current_snapshot()`（已写 doc）。 | `plan_balance/mod.rs` | M10.11 | 订阅 API 同时返回当前值。 |
| B65 | **plan_balance home 解析仅 USERPROFILE**：旧资产经 home.rs 带 `dirs::home_dir()` 回退；Windows 影响低。 | `plan_balance/source.rs` | M10.11 | 待复核。 |
| B66 | **`SettingsPathGuard` 仅模块内测试可用**：e2e 若需隔离 plan_balance home，需 USERPROFILE 注入或上提共享守卫。 | `plan_balance/source.rs` | M10.11 | 需要时上提。 |
| B67 | **审计双栈重复风险**：`tracing` 用 `log-always`（审计无条件进 log）；若后续装配全局 tracing subscriber，事件双落点。 | `slterm_app/Cargo.toml`/`audit.rs` | M10.12 | 装配 subscriber 时改回 `log` feature 并显式控制桥接方向，或 subscriber 侧过滤 audit。 |
| B68 | **SEC-13 覆盖半面**：claude hooks 的 `HookInspection.needs_repair` 是结构比对、无哈希；磁盘 hook 被改时不产生 SEC-13 哈希事件。 | `ai_hook/local/settings.rs` `current_at`/`inspect_with` | M10.12 | 03 篇补内嵌模板指纹后接 `audit::hook_hash_mismatch`。 |
| B69 | **`UninstallRun` 调 `slterm.exe setup-ai --remove` 但子命令不存在**：卸载清理为 no-op（错误被 runhidden 吞）。 | `scripts/installer.iss` [UninstallRun] | M11.1 | 03 篇接线或首发前删除该行。 |
| B70 | ~~**`license = "GPL-3.0"` 未落**~~（M11.3 已补：`[workspace.package] license = "GPL-3.0"` + 六成员 `license.workspace = true`）。 | 根 `Cargo.toml` | M11.1 | 已闭环。 |
| B71 | ~~**发布脚本无 `#Requires -Version 7` 声明**~~（M11.3 已给 `scripts/` 全部 `.ps1` 首行加声明；5.1 下报错明确指向版本）。 | `scripts/**` | M11.2 | 已闭环。 |
| B72 | **新鲜度判据含根 `Cargo.toml` mtime**：仅改根清单注释（编译输入未变）会让 release 链假红（二进制不重链接）；本次以 `cargo clean` 绕过。 | `scripts/build-windows-product.ps1` `Assert-FreshBinaries` | M11.2 | 改判「参与二进制的输入摘要」或清单变更时强制重链接——M11.4 前裁决。 |
| B73 | **导航树/Commit/Agent 全局文件三个侧栏视图仍是占位**：`render_view_slot` 只对 `Explorer` 渲染真实内容，其余显示 title + 归属文案。README/CHANGELOG 已如实标注「已注册占位」。 | `app_shell/workspace/sidebar.rs` `render_view_slot` | M11.3 复核 | 按 05/07/03 篇补内容实现；或从发布说明与 UI 中移除入口。 |
| B74 | **`slterminal-projects.json` 无任何代码消费**：DECISIONS B.20（保留层级提取、05 消费/07 只读）在 GPUI 单窗口模型中未落地。 | 仓根 `slterminal-projects.json` | M11.3 复核 | 裁定删除该文件，或补迁移消费点并登记归属。 |
| B75 | **源码/测试注释仍大量引用设计稿篇号**（「NN 篇」）：docs 已删，注释锚点悬空。 | 全仓源码注释 | M11.3（承接 A3） | 按 A3 裁决统一清理或收窄纪律为「篇号锚点允许」。 |
| B76 | **`.claude/test-exemptions.md` 仍含旧载体豁免行**（WebView/xterm.js/jsdom/wdio/React/Dockview 等）：设计 11 篇要求 M0 末按载体存亡全表重审，未执行。 | `.claude/test-exemptions.md` | M11.3 复核 | 按「载体消亡销项、PTY/Win32 行保留改写、翻案行保留」重审；销项保留根因一行。 |
| B77 | **禁名门禁无全树模式**：`check_brand_regress.py` 只有 staged/message/push/range（均扫新增行）；设计 M11.3 的「临时稿删除后切全树常挂」未实现。 | `scripts/check_brand_regress.py` | M11.3 复核 | 若要全树扫，需先给历史来源语境（ADR/README/REFACTOR-ISSUES 等）建豁免表；否则维持新增行模式并在文档注明。 |
| B78 | **卸载后安装目录残留 `.slterm-update.slterm-lock`**：GUI 运行在 exe 旁创建更新交接锁（句柄即权威，空文件 5 字节），Inno 卸载清单不含该文件，卸载后安装目录因此保留（与 C29 同源）。 | `platform/update_installation.rs` `guard_base`、`update_download/handoff.rs`；`scripts/installer.iss` `[UninstallRun]` | M11.4 首发核验 | 正常退出删锁（C29 已记同源取舍），或卸载前由应用/安装器清理 exe 目录；裁决前按已知残留登记。 |
| B79 | **开始菜单快捷方式落在 `(Default)` 组**：`installer.iss` 未设 `DefaultGroupName`，安装日志与实测显示 `Programs\(Default)\slTerminal.lnk`；功能可用但用户可见命名异常。 | `scripts/installer.iss` `[Setup]`/`[Icons]` | M11.4 首发核验 | 设 `DefaultGroupName={#AppDisplayName}`（或显式组名）；属发布面变更，需用户裁决。 |

## C. 计划内（归后续子步/阶段，重构完成时验证）

| # | 事项 | 归属 | 备注 |
|---|------|------|------|
| C1 | **OSC 133 `CommandStart`/`CommandDone` 未接**：普通 shell `pane.prompt` 后停 running，`pane.wait --state settled` 超时；`pane.run` 落 `run_start_timeout`/`exit_code_unavailable`；`active_run`/`last_run` 恒空。 | M9.5 / 02 篇 | 两条 OSC 边接 `AgentActivity::submitted/command_finished` 与 active_run/last_run 记账。 |
| C2 | **运行时 skill 投放 UI 化/建议条呈现接线**：`Ctrl+.` paste-only、三态交互、ghost/popup 关键路径。 | M9.5 | 本片 M9.5 出口。 |
| C3 | **markdown 数学渲染管线 + terminal_math overlay**。 | M9.4 | |
| C4 | **`spawn_config_guard` 未挂 app 启动链**。 | 06 篇设置域启动链 | M4.5/M6.5 遗留。 |
| C5 | **SEC-12 Suspended 确认交互**：当前仅以错误文本（含原文）回页面，未做「设置页展示原文 + 确认后 confirm 注入」。 | 03 篇改造节 2 / 06 | |
| C6 | **hooks 编辑面**：多行 JSON 非五型表单控件；仅 claude user 层（project/local 三层归 03）；设计开放问题 4 待产品裁定。 | 03/06 | |
| C7 | **非聚焦 pane veil 固定 `theme().background.opacity(0.35)`**：主题开关归 06（05 开放问题 1）。 | 06 | |
| C8 | **`export_workspace` 无 UI 入口**（设计开放问题 5 默认入口归 04/07）。 | 04/07 | |
| C9 | **注册表后台线程写入无直接自动化断言**（边检测用注入 recorder，写入器 tempdir 单测；spawn 本身未断言）。 | 11 篇测试形态 | |
| C10 | **11 篇 conformance 脚本套件与 golden 未落地**：当前由 Rust 集成测试 `runtime_conformance` 驱动，行为基线可复用。 | 11 篇 | |
| C11 | **i18n 合同独立 workspace CI 登记**：`tools/slterm-i18n-contract` 不在根 workspace，clippy/CI 不覆盖。 | 12 篇打包/CI 清单 | |
| C12 | **旧栈语义资产转测试保留**：git2 五份语义资产（D07-1 裁定转对照用例）。 | 07/M11 | |
| C13 | **docs/ 临时稿删除 + 篇号锚点清理**（与 A3 联动）。 | M11 | docs/ 已删（M11.3）；源码/测试注释中的篇号锚点仍在，待 A3 裁决后清理。 |
| C14 | ~~**设计措辞修正**：05 篇「窗口边缘」应为「内容区外圈」；04 篇 `reorder_batch` 签名稿与上游不符（已照源）；08 篇 `note_bell` 上游不存在（未落）；04 篇 generation 等待错误码口径（实现按设计稿 `agent_identity_mismatch`/`agent_exited`）；08 篇 D08-2 待沉淀仍写「M9.3 落地时拍板」而 DECISIONS 已裁（文档漂移）；08 篇 `compile_formula` 签名草案与上游实源不符（已按源码照抄）；08 篇 #44 `draw_overlays` 未迁，M9.5 自绘 paint pass（设计稿应标注归属）；09 篇 §6 duration 值域（Default\|Short\|Long）与 06 已落六档不一致（实现按六档消费，persistent 无系统常驻档）；09 篇 M10.2 出口只写「agent 退出清进度」，实现按任务书补 TurnDone/pane 关闭清除（doc 待回登）；09 篇 `focus_window(cx)`/`focus_pane(pane,cx)` 为多窗注册表签名，与单窗 GPUI（窗口更新回调内不可再 update）冲突，实现改为显式 `&mut Window` 落点、目标选择/平台调用仍单点；09 篇 `PreparedUpdate::commit(&self)` 实为 `commit(&mut self, windows: &[Session])`，`RELEASE_DOWNLOAD_PREFIX` 落 `update_check.rs` 而非 `update_download`（M11 原位收敛）；09 篇 row 35 写 `.lnk`「静默参数」但 CLI 无静默旗标（实现按设置两条件；doc 待删措辞或另立 `--silent`）；09 篇 TaskDef 无一次性/按需形态，实现新增 `TaskSchedule`（Once/OnDemand，M11 回登）；10 篇 `load_generic_secret` 签名 `Option<Vec<u8>>` 实为 `Option<SecretBytes>`（出域句柄补强）；`store_with_username` 空 secret 拒绝为新增语义（M11 回登或裁除）；10 篇 D10-2 类别清单未回登（实现裁 6 类 + 22 敏感键表 + 导出清洗/导入复验双闸）；`filter_settings` 签名 `Vec<u8>→Vec<u8>` 实为 `Result`；备份依赖 caret 解析 aes-gcm 0.11.1/argon2 0.6.0（上游 0.11.0/0.6.0-rc.8）；**设计输入失实**：上游基线 e537d528 无 plan_balance 模块，行为真值源实为 slTerminal git 历史 `src-tauri/src/plan_balance/`（c59f47f^ 终态），设计稿「上游源」表述待 M11 订正；`AppError::Unknown` 变体未登记 01 篇锚点表；10 篇审计事件字面格式未定，实现取 `operation/path/source/result` 与 `expected/actual`（M11 回登）；上游仓内无 audit/tracing 现成形态（新建）；12 篇构建命令写 `--features slterm/gpui-shell` 但本仓无该 feature（脚本按实际省略；M11.3 订正）；D12-5 默认关时移交链改 `window.focus`+`tab.new`，与 04 篇「ATTACH 先于 tab.new」冲突（跨构建移交会失败，release 同版本无影响；M11.3 登记新形态与撤销条件）；12 篇 manifest 示例写 `docs/`，实际资产在包内 `assets/`。~~（M11.3 收口：设计稿已删，订正对象消亡；实现事实已回登 ADR-0025/0027/0029/0034/0035 与安全成因 note）。 | 设计稿 | 已闭环。 |
| C15 | **`D04-3 windowing_behavior` 未设键**：单窗行为硬编码 `SINGLE_WINDOW_ID`。 | 12 篇随 settings 面统一裁定 | |
| C16 | **`request_once_bounded` 无消费方**（插件宿主）。 | M9.5/M10 | |
| C17 | **`ProviderTestOutcome` 文案本地化**（展示边界）。 | M9.5 | |
| C18 | **`save_api_key` 签名适配**：设计稿无后端参数，实现改为 `&dyn CredentialStore` 显式注入；M10.9 真实实现后调用点直传。 | M10.9 | |
| C19 | **`load/save_final/quarantine/mark_boot_attempt` targeted allow**：设计锚定 API 保留，生产走 `_at` 显式路径。 | 11 篇复核 | |
| C20 | **老会话文件叶/tab 无 launch**：首拍快照规范化为 `Some(Default)`（语义等价，字段形态变化）。 | 记录 | 无迁移义务。 |
| C21 | ~~失败边投影形态~~（M9.5 已裁：失败边**不接** `AnswerInbox` notice——进程级事实无 pane 归属且合成命令偏技术化；维持「进闸门 + log 证据」，若需提示另立进程级 notice 通道）。 | M9.5 | 已闭环。 |
| C22 | ~~`SnapCell` 无 DIM 标志~~（M9.5 已裁：同锁窗口从 Term cell Flags 构造 `OverlayCell`，快照契约不变）。 | M9.5 | 已闭环。 |
| C23 | **真实模型出网无自动化**：`send_model_request` 已登记豁免。 | 后续 | 用本地 mock HTTP server 补四协议族 URL/头/响应解析用例。 |
| C24 | **AUMID 图标未物化**：09 要求 `ensure_icon_file`（appLogoOverride），本仓无 app_icon/PNG 资产（归 12 篇发布面）；横幅无 logo（notify-test 打印 `icon: unavailable`）。 | M12 | 随包落图标并启用（托盘图标同源缺口）。 |
| C25 | **真实 toast 投递面自动化为空**：`cfg(test)` 短路 show/dispatch，已登记豁免（回收站先例）；兜底 = 漏斗纯函数用例 + notify-test 真机 + 人工 smoke。 | 11 篇 | 如需 CI 化可加假 toast sink。 |
| C26 | ~~**`slterm_app/Cargo.toml` 无 `version`**~~（M11.1 已定版 `0.3.0`；`CARGO_PKG_VERSION` 与 `--version` 回读均已生效）。 | D12-4 首发版本线 | 已闭环。 |
| C27 | **更新弹窗/安装授权步未落**：`UpdateAvailable`/`DownloadStatus`/`ready_path`/`prepare/commit`/`schedule`/`mark_prompted` 均已就绪待接。 | 后续 | 消费面清单见 M10.5 报告缝合点。 |
| C28 | **`update-check` CLI 诊断子命令未加**（任务标注可选）。 | 后续 | 排障需要时在 `cli.rs` 挂。 |
| C29 | **`.slterm-update.slterm-lock` 启动残留**：锁权威是内核句柄，空文件无害（上游同构）。 | 产品取舍 | 如需零残留正常退出时删除。 |
| C30 | **`backgroundTasks` 段未进 06 设置键注册表**：当前只落运行时原子量与 `apply_config`（测试覆盖）。 | 06 设置页 / M10.11 | 随 planBalance/任务配置 UI 落地时正式登记该段。 |
| C31 | ~~**日志边界测试用 `log::Log` 而非 tracing**：仓库当前无 tracing 依赖；M10.12 引入 audit target 后需补 tracing 捕获断言。~~（M10.12 已补：`logs_never_contain_credential_plaintext` 同时断言 tracing 捕获与 tracing→log 桥接）。 | M10.12 | 已闭环。 |
| C32 | **`RestorePoint` 无 loader**：names 私有且无 open，仅能撤回本次会话返回的句柄；跨重启恢复点清单/撤回归 06 设置页 UI 决策（上游同限制）。 | 06 UI | 落 `RestorePoint::open(path, passphrase)` 或维持会话内撤回语义。 |
| C33 | **备份 `collect` 整单失败需 UI 展示**：settings/providers 不可解析时整单失败（安全取向），UI 须明示避免误以为成功。 | 06 UI | 随 D10-4 设置页安全分区落地。 |
| C34 | ~~**planBalance 生产启动链未接**：注册表条目与 executor 已就绪，但生产无订阅/poller 循环（当前仅 L1 进程内驱动）。~~（M10.12 已接：`app_shell::run_shell` → `plan_balance::start_background_poll`；虚拟时钟用例锁定节拍/enabled 门控/Drop 退订）。UI 消费 `current_snapshot` + `subscribe` 归 06。 | M10.12 | 启动链已闭环。 |
| C35 | ~~**`assets/runtime-control-api.md` 为新建件**~~（M11.3 已按实现与 `runtime-api-v1.schema.json` 核对：发现优先级、信封、上限、方法族、错误码、CLI 入口与 legacy 关闭均一致）。 | 04 篇 | 已闭环。 |
| C36 | ~~**README/CHANGELOG/根 CLAUDE 待重写**~~（M11.3 已完成：README 双语、CHANGELOG 0.3.0 实写、`.claude/CLAUDE.md` 全量改 GPUI 形态、六个 crate CLAUDE.md 补齐、CONTEXT.md 重写）。 | M11.3 | 已闭环。 |
| C37 | **D11-2 `cargo xtask` 统一入口未落地**：当前定向测试为裸 cargo 命令 + 文档红线；ADR-0028 已按现状沉淀。 | 11 篇 | 若未来封装须保留 `-p` + `--test lib_tests` + `--test-threads=1`。 |
| C38 | **D11-3 内存压测与 `pr_size_hint.ps1` 未落地**：`scripts/windows-memory-stress.*` 与行数提示脚本均不存在；阈值未锁入检查点。 | 11 篇 | 需要长期内存质量指标或大 PR 提示时再建，先登记豁免/延后理由。 |

## D. 已修（存档，防重复登记）

M4.5→M5.4：`split_bounds` 键下标泄漏改 TabId；M5.3b→M6.2：配置钮；M6.2→M6.4：locale 每帧探测；M6.3→M6.2：调色板接主题解析；M7.1→M7.2：schema 方法枚举裁剪；M7.2→M7.3：WorktreeProvenance/git_params/capabilities 路由；M7.3→M7.5：RuntimeControl 泵臂/agent 生命周期；M7.5→M8.5：runtime git.* 接执行体；M8.1→M8.3：图片焦点；M8.2→M8.3/M8.5：撤销双栈/整窗脏守卫/Document 脏守卫/图片压帧；M8.3 feature-clippy 三红（2612d1e，验收补 feature-clippy 覆盖）。

M12/R1（2026-10-18）：B69 `setup-ai --remove` 卸载清理、B72 新鲜度改内容摘要构建戳、B74 删除遗留 `slterminal-projects.json`、B77 禁名门禁 full 全树模式与历史语境豁免、B78 锁文件随释放删除 + `[UninstallDelete]` 兜底、B79 显式 `DefaultGroupName`、C24 应用图标资产（PNG/ICO，托盘与通知同源）。

M12/R2（2026-10-18）：B1 测试构建数据目录默认隔离（临时根）、B3 恢复默认共用保存锁、B26 describe↔CLI 命令清单一致性用例、B60 复核（M10.11 已落借出白名单扫描）、B61 真实凭据管理器用例环境门控、B62 非法主题备份改硬失败、B63 敏感键 denylist 漂移守卫、B67 subscriber 双落点约束注释、B68 claude hook 修复路径补 SEC-13 哈希事件。

M12/R3（2026-10-18）：A2 设置写通道全量后台相位机（Pending/Committed/Failed、队尾合并、失败统一回滚；含 hooks 授权与主题条件写）、A6 active 无凭据回退 env + 显式错误上条、A16 审计成功写降 info、B6 保存全部继续逐件保存并聚合提示、B12 `tab.close` 增 `force` 参数与 `dirty_document` 拒绝、B25 忙碌 pane 关闭确认框、B43 通知动作回执消费（聚焦 + 审计）、B57 拖动预览不被回读覆盖、B64 `subscribe` 返回当前快照、B65 `HOME` 回退、B66 测试守卫宽化到 feature 形态、C5 SEC-12 命中原文展示 + 确认后重试。

## E. 验收纪律补记

- 自 M8.3 起验收同时跑两形态 clippy（`--workspace` 与 `-p slterm --features gpui-test-support`）——此前无 feature 形态遗漏了 M6 起的三处测试端红灯。
- `.claude/test-exemptions.md` 已登记：回收站真桶（M8.4）、gutter 行标像素对齐（M8.5）、`runtime_conformance` 真机（M7.5）。
