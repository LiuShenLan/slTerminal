# 重构遗留问题修复计划（FIX-PLAN）

> 依据 `docs/REFACTOR-ISSUES.md` 全量澄清结果制定（2026-10-18）。
> A 段 17 条已全部裁决；B 段 79 条全量分批；C 段 38 条逐条处置。
> 编号沿用原清单，本文件不重复登记新问题；行号引用禁用，定位一律用符号名/文件路径。

## 1. 批次总览

修复按风险优先级分为 8 批，批内按模块聚合；每批独立提交、独立验收，按 R1→R8 顺序推进（后者依赖前者的写通道/门禁改动）。

| 批次 | 优先级 | 主题 | 成员 |
|---|---|---|---|
| R1 | P0 | 发布链与卸载面 | B69、B72、B74、B77、B78、B79、C24 |
| R2 | P0 | 数据与安全 | B1、B3、B26、B60、B61、B62、B63、B67、B68 |
| R3 | P1 | 设置写入与安全确认 | A2、A6、A16、B57、B6、B12+B25、B43+C5、B64、B65、B66 |
| R4 | P1 | 运行时/工作区/恢复 | A1、A5、A10+A13、C1、C4、B48、B50、B53、B54、B56 |
| R5 | P1 | git/编辑器正确性 | B8、B9、B15、B16、B21、B29、B34 |
| R6 | P2 | 功能补全 | B7、B10、B11、B17、B18、B19、B20、B55、B73、C6、C8、B38 |
| R7 | P2 | 体验与呈现 | A8、A14、A15、C7、C27+B52、C33、C17、B40、B41、B42、C23 |
| R8 | P3 | 工程卫生 | A3+B75、B2、B5、B13、B14、B23、B24、B28、B30、B31、B32、B39、B76、C9、C13 |

**每批出口标准（可机验）**：

- 定向测试：`cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`；涉及跨 crate 时 `cargo test --workspace -- --test-threads=1`（串行红线）。
- 双形态 clippy：`cargo clippy --workspace --all-targets` 与 `cargo clippy -p slterm --all-targets --features gpui-test-support` 零警告。
- `cargo fmt --all -- --check`。
- `python scripts/check_architecture.py`。
- `python scripts/check_brand_regress.py staged`。
- 涉及发布链：`pwsh -File scripts/tests/package-release.tests.ps1` 与 `pwsh -File scripts/verify-release.ps1 -Version <当前版本>`。
- 每条修复附防复发测试（对照老代码补回归用例）；不可自动化部分登记 `.claude/test-exemptions.md`。
- 负载裕度类改动（仅放大时间预算、断言语义不变）须在改动点注释标明「负载裕度，非断言/语义放宽」。

## 2. A 段裁决（17 条）

| # | 裁决 | 落地动作 | 归属 |
|---|---|---|---|
| A1 | 改测试结构 + 提额 | 容量用例：工作线程返回 Pty 不立即 drop，主线程 join 后断言恰 1 成功再统一释放（根因是测试内调度竞态，非残留/锁失效）；全链 15s→30s 负载裕度 | R4 |
| A2 | 全量相位机 | 设置写通道后台化：Pending→Committed/Failed，UI 乐观更新，失败回滚持久化快照并提示；live 预览合并节流同通道 | R3 |
| A3 | 清理篇号锚点 | 机械清理全仓「NN 篇」注释锚点；纪律维持「仓内禁引 docs 任何内容」 | R8 |
| A4 | 维持 | `-e` 视为显式独立作业，不并入单实例控制面；登记模块文档 | 关闭 |
| A5 | 放宽 | `verify_repository` 放宽为「cwd 在 repo 内」，按 repo root 解析相对路径 | R4 |
| A6 | env 兜底 | active provider 无凭据时回退 `SLTERM_AI_KEY`→`OPENAI_API_KEY`，仍无则显式错误（取代静默放弃） | R3 |
| A7 | 不做 | 设计稿无补全合同，不自造 | 关闭 |
| A8 | 比例持久化 | 不迁 R6-R8；session schema 增双槽比例字段 + 拖拽分隔条 | R7 |
| A9 | 维持 | 分屏不继承锚 pane 的 `-e` 身份；登记 | 关闭 |
| A10/A13 | 统一漏斗 | recovery 汇总走通知漏斗，新增「启动汇总豁免前台门控」策略位 | R4 |
| A11 | 复用 | 维持复用 `AssistantConfig.enabled`；登记 | 关闭 |
| A12 | 维持 | config_guard 锁竞争不计失败边；登记 | 关闭 |
| A14 | UX 约束 | 开启静默启动时自动置 `tray=1` 并提示；底层保持纯两条件 | R7 |
| A15 | 补移植 | DesktopAcrylicController 动态探测 Windows App Runtime，存在则用、否则回退 WCA state 4，不硬依赖 | R7 |
| A16 | 审计分级 | 成功写降 info、失败保持 warn；高频提交由 A2 相位机合并；不引入 debug 分流 | R3 |
| A17 | 接受预算 | 校准态预算保持 19.63/25.54 MiB；O3 清单缩减留作后续独立评估 | 关闭 |

## 3. B 段批次明细

### R1 发布链与卸载面（P0）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| B69 | `scripts/installer.iss` `[UninstallRun]`；`slterm_app/src/cli.rs` `Subcommands` | 新增 `setup-ai` 子命令及 `--remove`：清理各家 AI CLI 配置中的 marker 组注入；保留 `[UninstallRun]` | CLI 解析用例；临时 home 下安装→`--remove`→inspect 零残留 |
| B72 | `scripts/build-windows-product.ps1` `Assert-FreshBinaries`/`Get-NewestSourceTime` | 构建后写源码树+清单内容摘要 stamp；断言时重算比对，stamp 缺失回退 mtime | 脚本自测：仅改注释不红、改源码红、stamp 缺失回退 |
| B74 | 仓根 `slterminal-projects.json` | 删除文件与 `.gitignore` 对应行（e2e 遗留、未跟踪、零消费） | 无（无代码消费） |
| B77 | `scripts/check_brand_regress.py` | 新增 `full` 模式（全树 tracked 文件）+ 历史语境豁免表；先全树扫描分类命中（历史语境入表、真违规修） | python 单测：full 模式、豁免表、真违规命中 |
| B78 | `slterm_app/src/atomic_file.rs` `LifetimeFileLock`；`scripts/installer.iss` | Drop 先释放句柄再尝试删锁（失败忽略）；installer 新增 `[UninstallDelete]` 清 `.slterm-update*` | 锁生命周期用例（Drop 后文件不存在）；安装/卸载实测 |
| B79 | `scripts/installer.iss` `[Setup]` | 设 `DefaultGroupName={#AppDisplayName}` | 安装实测开始菜单组路径 |
| C24 | `assets/`、`notify.rs`、`tray.rs` | 引入应用图标资产：用户提供原图；未提供则生成占位 ICO 并标注待替换；启用 AUMID icon 与托盘同源 | `notify-test` 真机人工 + 打包清单全等 |

### R2 数据与安全（P0）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| B1 | `platform/dirs.rs` `data_dir`；`ai_agents.rs` `override_path` | 测试专用数据目录覆盖根（环境变量或注入），相关用例全程 tempdir | 断言测试不写真实 `%APPDATA%` |
| B3 | `slterm_settings/src/reset.rs` `restore_defaults_at` | 重置写盘入口上移至 `settings_store`（走 SAVE_LOCK），保留备份语义 | 并发 reset+save 用例 |
| B26 | `runtime_api/transport.rs` `runtime_description`、`shortcuts.rs` | 补 describe↔catalog 一致性测试（资源动词双向覆盖） | 新增一致性用例 |
| B60 | `secrets.rs` `SecretBytes::as_bytes` | 源码扫描守卫：白名单调用点之外禁止 `as_bytes` | 扫描守卫用例（违规夹具红） |
| B61 | `secrets.rs` 真 Credential Manager 用例 | 环境变量门控默认跳过；登记豁免 | 未设变量时跳过用例 |
| B62 | `encrypted_backup.rs` `add_themes` | 非法主题改硬失败（整单失败，安全取向） | 非法主题备份返回错误用例 |
| B63 | `encrypted_backup.rs` `SENSITIVE_KEYS` | provider schema 字段漂移守卫（敏感词命中即红） | 漂移守卫用例 |
| B67 | `slterm_app/Cargo.toml` tracing feature、`audit.rs` | 维持单向桥接；装配 subscriber 时禁 log-always 双落点（代码注释 + ADR 记录） | 日志边界用例扩展断言 |
| B68 | `ai_hook/local/settings.rs` `current_at`/`inspect_with` | 内嵌模板指纹 + 磁盘 hash 比对；命中发 `audit::hook_hash_mismatch` | 篡改 hook 触发哈希事件用例 |

### R3 设置写入与安全确认（P1）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| A2 | `settings_store.rs` `save_settings_blocking`/`save_settings_checked`；`settings_pane/persistence.rs` | 新异步写通道（相位机 + 合并 + 回滚）；全部调用点迁移；复位默认同通道 | 状态转换/失败回滚/合并节流/串行化用例 |
| A6 | `ai_assistant.rs` `request_target_from` | active 无凭据回退 env key，仍无则显式错误；重写原「静默」用例 | env 命中/无 env 报错/有凭据不走 env |
| A16 | `audit.rs` `write_audited` | 成功 info、失败 warn | tracing 捕获断言 level |
| B57 | `settings_pane/persistence.rs` `preview_opacity` | 预览值并入相位机全局预览态，`wallpaper::refresh` 不覆盖 | 预览中并发 Changed 不丢预览用例 |
| B6 | `workspace/documents.rs` `guard_file_window_close` | 整窗聚合失败 notice | 部分失败聚合提示用例 |
| B12+B25 | `workspace/runtime_control.rs` `tab.close`；`workspace/mod.rs` `close_pane` | `tab.close` 增 `force`，脏文件返回 `dirty_document`；`close_pane` 补 busy_child 确认框，复用同一确认通道 | runtime 契约用例 + 确认框 UI 用例 |
| B43+C5 | `notify.rs` `NotificationChoice`；SEC-12 Suspended | 消费动作回调；设置页展示原文 + 确认后注入 | 通知动作→确认→注入用例 |
| B64 | `plan_balance/mod.rs` 订阅 API | `subscribe` 同时返回当前快照 | 订阅后先读当前值用例 |
| B65 | `plan_balance/source.rs` home 解析 | 补 `dirs::home_dir()` 回退 | USERPROFILE 缺失回退用例 |
| B66 | `plan_balance/source.rs` `SettingsPathGuard` | 上提共享守卫供 e2e 隔离 | e2e 隔离用例 |

### R4 运行时/工作区/恢复（P1）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| A1 | `slterm_terminal/tests/pty_integration_tests.rs` | 容量用例改测试结构（见 A 段裁决）；全链 15s→30s | 全量复跑稳定；断言不再依赖调度 |
| A5 | `git/mod.rs` `verify_repository`/`git_target_changed` | 放宽 cwd 在 repo 内 + 相对路径解析 | 子目录 pane 四方法用例 |
| A10+A13 | `session_recovery`、`notify.rs` | 汇总走漏斗 + 启动豁免前台门控策略位 | 启动期汇总投递、后台不打扰用例 |
| C1 | `slterm_terminal` OSC 133 处理、`gpui_terminal` 记账 | `CommandStart`/`CommandDone` 接 `AgentActivity::submitted/command_finished` 与 active_run/last_run | `pane.wait --state settled`、`pane.run` 退出码用例 |
| C4 | `ai_hook` `config_guard` | `spawn_config_guard` 挂 app 启动链 | 启动后 guard 探针用例 |
| B48 | `tray.rs` `THREAD_STARTED` | 闸门复位重试 | 失败后重试用例 |
| B50 | `workspace/title_bar.rs` `apply_titlebar_command` | Close 分支走 `close_window` facade + 脏守卫 | 关闭确认用例 |
| B53 | `update_download/handoff.rs` `restore_ticket` | 接 `session_recovery::prepare_startup` | 更新后恢复流程用例 |
| B54 | `tray.rs` `set_enabled`/`apply_enabled` | 每次 `set_enabled(true)` post WM_APP_SET（或 mounted 标志），让 1Hz 快照自然重试 | NIM_ADD 失败后重试用例 |
| B56 | `view/element.rs` 容器底色层 | 网格区绘制 `Settings.palette.background` 单层 | 背景 token 快照用例 |

### R5 git/编辑器正确性（P1）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| B8 | `file_editor/gutter.rs`、`side_panel` | fs-notify watcher + 有界通道 + 前台合并刷新 | 外部修改触发 gutter/树刷新用例 |
| B9 | `file_editor/gutter.rs` `read_for_path` | 复用 side_panel 快照或短 TTL 缓存 | git 调用次数断言 |
| B15 | `git/mod.rs` `GitError` | 预检 `cat-file -e` 或 stderr 分类，区分权限/损坏/不在 HEAD | 错误分类用例 |
| B16 | `file_editor/structure_inline_view.rs` `render_inline_image` | 剥离 query/fragment 后再 join | 带查询串图片用例 |
| B21 | `agent_dirs.rs` 表 | 六家（goose/devin/antigravity/pi/maki/kilo）实测复核；不可自动化部分登记豁免 | 实测记录 + 表值更新 |
| B29 | `suggest.rs` `commands_handle` | pwsh 优先 + 5.1 回退双探 | 双探分支用例 |
| B34 | `git/mod.rs` `unstage` | 补 unborn HEAD + staged 新文件用例 | 新增用例 |

### R6 功能补全（P2）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| B7 | `workspace/keyboard_bindings.rs` `install_user_keybinds`；设置页 | 变更通知→重建 keymap（处理精确 Unbind）；设置页键位编辑 UI | 重绑生效用例 |
| B10 | `code_tab.rs` `GutterClicked` | 冲突三栏合并器（事件已可分流） | 合并器 UI 用例 |
| B11 | `workspace/file_tree.rs` 上下文菜单 | 新建/重命名/删除 CRUD | CRUD 用例 |
| B17 | `file_editor/chrome.rs` | 补全屏 + reader-focus（远程取消不做） | 控件行为用例 |
| B18 | `settings_pane/theme_editor.rs` | 字距/字重/layout/blur 控件 | 高级字段编辑用例 |
| B19 | `theme_library/package` | 导入导出 UI 相位机 | 导入导出往返用例 |
| B20 | `workspace/sidebar.rs` `SideViewId::title`/`content_owner` | 收编 catalog + typed Message，随语言切换 | 中英切换用例 |
| B55 | `settings_pane.rs` | 壁纸路径选择 + fit/alignment + 预览 | 壁纸设置用例 |
| B73 | `workspace/sidebar.rs` `render_view_slot` | Nav/Commit/AgentFiles 三视图实现 | 三视图渲染用例 |
| C6 | hooks 编辑面 | 五型表单控件（user 层）；project/local 三层另行裁定 | 表单编辑往返用例 |
| C8 | `export_workspace` | UI 入口 | 导出入口用例 |
| B38 | `ai_assistant.rs` `HookFailure` | 呈现层只作上下文，不直出合成命令 | 呈现文案用例 |

### R7 体验与呈现（P2）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| A8 | `workspace/sidebar.rs`、`session.rs` | schema 增双槽比例字段 + 拖拽分隔条 | 比例持久化往返用例 |
| A14 | `settings_pane/pages.rs` | `silent_start` 开启时联动置 `tray=1` 并提示 | 联动写用例 |
| A15 | `platform/` 新模块 | DesktopAcrylicController 动态探测 + WCA state 4 回退 | 探测/回退分支用例（真机部分登记豁免） |
| C7 | 非聚焦 pane veil | 接主题开关，替换固定 `theme().background.opacity(0.35)` | 主题切换用例 |
| C27+B52 | `update_check.rs`、更新弹窗 | 弹窗接线 + `mark_prompted`/`remind_later`/`skip_version` | 弹窗状态机用例 |
| C33 | 备份 UI | 整单失败明示，避免误以为成功 | 失败 UI 用例 |
| C17 | `ProviderTestOutcome` 文案 | 本地化 | 中英文案用例 |
| B40 | `element.rs` ghost 绘制 | 按 `suggestion_edit` 替换区间绘制 | ghost 区间用例 |
| B41 | `math_overlay.rs` `plan_frame` | 编译移出 Term 锁或异步预热 | 持锁时间断言/预热用例 |
| B42 | `view.rs` 弹窗/AI 条布局 | 底部空间协调 | 布局用例 |
| C23 | AI 请求测试 | 本地 mock HTTP server 补四协议族 URL/头/响应解析用例 | 四协议族用例 |

### R8 工程卫生（P3）

| # | 定位 | 方案 | 测试点 |
|---|---|---|---|
| A3+B75 | 全仓源码/测试注释 | 机械清理「NN 篇」锚点（改为符号名或直接描述），完成 C13 | 全仓 grep 零命中 |
| B2 | `ai_hook/windows.rs` `PipeServer::shutdown` | 读超时/overlapped 硬化 | shutdown 超时用例 |
| B5 | `settings_store.rs` quarantine | 时间戳多代隔离件 | 多次损坏不覆盖用例 |
| B13 | `atomic_file.rs` `write` | 权限复制到临时文件 | ACL 保留用例（不可自动化部分门控） |
| B14 | `runtime_api/server.rs` | serve 线程 stop 句柄 | serve 线程退出用例 |
| B23 | `runtime_api/agent_api.rs` | `agent.start` kind 语义与 Schema 对齐评审 | 契约用例 |
| B24 | `git_worktree.rs` | git 内部错误码收敛 | 错误码映射用例 |
| B28 | `slterm_app/Cargo.toml` clap | 评估精确 pin（`=4.2.7` 或维持 caret） | 构建通过 |
| B30 | `completion/project_scripts/workspace.rs` | 窄解析器替代 `serde_yaml` | 解析往返用例 |
| B31 | `completion.rs` `Request::calculate` | 删除死分支或改语义 | 分支覆盖用例 |
| B32 | `ai_providers.rs` `remove_provider_at` | 先查 id 再删凭据 | 不存在 id 零调用用例 |
| B39 | `terminal_math/tests.rs` | 拆子模块（按 ui_tests 先例） | 拆分后测试全绿 |
| B76 | `.claude/test-exemptions.md` | 按载体存亡重审：销项保留根因一行，PTY/Win32/翻案行保留改写 | 文档审查 |
| C9 | 注册表后台线程 | 补 spawn 断言 | 断言用例 |

## 4. C 段处置（38 条）

| # | 处置 | 依据/前置 |
|---|---|---|
| C1 | 纳入 R4 | OSC 133 两边界未接，真实功能缺口 |
| C2 | 核实后关闭 | 核实 M9.5 交付（`Ctrl+.` paste-only/三态/ghost-popup）后关闭；未交付转 R6 |
| C3 | 核实后关闭 | 核实 M9.4 交付（数学渲染管线 + overlay）后关闭；未交付转 R6 |
| C4 | 纳入 R4 | `spawn_config_guard` 未挂启动链 |
| C5 | 纳入 R3（与 B43 合并） | SEC-12 确认交互 |
| C6 | 纳入 R6 | hooks 编辑面五型表单 |
| C7 | 纳入 R7 | veil 主题开关 |
| C8 | 纳入 R6 | `export_workspace` UI 入口 |
| C9 | 纳入 R8 | 注册表后台线程断言 |
| C10 | 延后 | 前置：conformance 脚本化需求成立；Rust 集成测试已覆盖行为基线 |
| C11 | 延后 | 前置：CI 接入；当前无 CI 配置 |
| C12 | 延后 | 前置：对照用例设计；旧栈已删，语义资产需从 git 历史提取 |
| C13 | 纳入 R8（随 A3） | 篇号锚点清理完成后关闭 |
| C14 | 关闭，移 D 存档 | 设计稿已删，实现事实已回登 ADR |
| C15 | 延后 | 前置：多窗口/设置面统一裁定 |
| C16 | 延后 | 前置：插件宿主落地 |
| C17 | 纳入 R7 | 文案本地化 |
| C18 | 关闭 | M10.9 已按显式注入实现 |
| C19 | 关闭 | targeted allow 已复核 |
| C20 | 关闭 | 无迁移义务（首拍快照已规范化） |
| C21 | 关闭，移 D 存档 | M9.5 已裁（失败边不接 AnswerInbox） |
| C22 | 关闭，移 D 存档 | M9.5 已裁（同锁窗口构造 OverlayCell） |
| C23 | 纳入 R7 | mock HTTP server 四协议族用例 |
| C24 | 纳入 R1 | 图标资产与 AUMID |
| C25 | 关闭 | 已登记豁免（回收站先例 + 纯函数用例兜底） |
| C26 | 关闭，移 D 存档 | 版本线已定 0.3.0 |
| C27 | 纳入 R7（与 B52 合并） | 更新弹窗与 prompt 状态 |
| C28 | 延后 | 前置：排障需要时再挂 CLI 诊断子命令 |
| C29 | 关闭（随 B78） | 锁文件删除策略落地后同源关闭 |
| C30 | 延后 | 前置：任务配置 UI 落地 |
| C31 | 关闭，移 D 存档 | 日志边界已覆盖 tracing 捕获 |
| C32 | 延后 | 前置：设置页安全分区对恢复点语义的裁定 |
| C33 | 纳入 R7 | 备份整单失败 UI 明示 |
| C34 | 关闭，移 D 存档 | 生产启动链已接 |
| C35 | 关闭，移 D 存档 | 运行时 API 文档已核对一致 |
| C36 | 关闭，移 D 存档 | README/CHANGELOG/CLAUDE/CONTEXT 已重写 |
| C37 | 延后 | 前置：如未来封装统一入口，须保留定向测试红线 |
| C38 | 延后 | 前置：内存质量指标或大 PR 提示需求成立 |

## 5. 关闭与登记清单（不进批次）

| # | 处置 | 理由 |
|---|---|---|
| A4 | 维持 | `-e` 为显式独立作业，不属单实例控制面；登记模块文档 |
| A7 | 不做 | 设计稿无补全合同，不自造 |
| A9 | 维持 | 分屏不继承锚 pane 的 `-e` 身份（上游同行为） |
| A11 | 维持 | hook 失败边复用 `AssistantConfig.enabled`，无独立键需求证据 |
| A12 | 维持 | 锁竞争不构成失败事实 |
| A17 | 接受 | 校准态预算 19.63/25.54 MiB 已钉；O3 缩减另评估 |
| B4 | 维持 | `try_load` 根非对象返回空树有既有用例锁定；授权路径语义不变 |
| B22 | 维持 | 已按设计稿落 `session_file` 签名 |
| B33 | 延后 | `codex_goals`/`codex_remote_compaction` 无消费方；设置页需要时再补 |
| B35 | 登记 | gutter 像素近似已登记豁免；优先组件装饰集合 |
| B36 | 登记 | `TestAppContext::test_window` 私有，上游 API 变化时补真断言 |
| B37 | 留锚 | `Execution::prepare()` 接缝保留（设计已裁不恢复远程/嵌套） |
| B44 | 维持 | 焦点切换不清任务栏进度（上游同行为）；登记 |
| B45 | 登记 | taskbar 测试记录用进程级 static，依赖串行纪律 |
| B49 | 登记 | 托盘 shutdown join 等模态菜单收合，罕见可接受 |
| B51 | 留锚 | `window_hwnd(cx)` 单窗缓存；多窗时按 cx 查询 |
| B58 | 维持 | 禁用态自愈循环保留；当前无任务配置 UI 不可达 |
| C2/C3 | 核实后关闭 | 见 C 段处置表 |
| B27/B46/B47/B59/B70/B71 | 已闭环 | 见 `REFACTOR-ISSUES.md` 对应行的闭环证据，不重复登记 |

## 6. ADR 沉淀清单

按 ADR 三条件（难逆转、未来读者会问、真实取舍）复核后沉淀，不满足者写入模块 CLAUDE.md：

| 主题 | 载体 |
|---|---|
| 设置写通道异步相位机与失败回滚语义（A2/A16/B57） | ADR |
| AI 请求凭据解析：active 优先、env 兜底、显式错误（A6） | ADR |
| 启动期 recovery 汇总的漏斗豁免策略位（A10/A13） | ADR 或模块文档 |
| Acrylic 动态探测与 WCA 回退（A15） | ADR |
| `tab.close` `force` 契约与关闭确认语义（B12/B25） | ADR |
| `setup-ai` 子命令与卸载清理契约（B69） | ADR 或 CLI 模块文档 |
| 发布新鲜度判据：内容摘要 stamp（B72） | packaging note |
| 禁名全树模式与历史语境豁免表（B77） | 模块文档 |
| 锁文件删除策略（B78） | 模块文档 |
| 双槽比例持久化 schema（A8） | session 模块文档 |

## 7. 执行纪律

- 每批开始前从 `REFACTOR-ISSUES.md` 取该批成员原文核对定位；完成后回写状态（修毕标 D 段，关闭项注明理由）。
- 每批一个提交（或按模块拆分提交），提交信息注明批次号与成员编号。
- bugfix 须附防复发测试（对照改动前老代码补回归用例）。
- 涉及用户可见行为/契约的改动（A2/A6/A13/A15/B12/B25/B69/B72）先沉淀 ADR 再落代码。
- 不可自动化面（真实 toast/托盘/凭据库/真机视觉）在 `.claude/test-exemptions.md` 登记兜底层级后才可结项。
