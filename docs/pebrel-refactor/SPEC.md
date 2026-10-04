# slTerminal 参考 pebrel 重构优化 spec（总表）

> 本文件是汇总索引与决策总表；功能点粒度的采纳/不采纳明细见各分片。各分片与总表冲突时，以本表（用户决策）为准。
>
> **pebrel baseline commit**:`e537d528c508e8607d0f5f9fd25e5902f40d661e`(2026-10-04,pebrel main HEAD)。本 spec 全部裁定基于该 commit 的代码与文档。
>
> **增量参考操作法**:baseline 同步写入根 CLAUDE.md;后续参考 pebrel 新改动时，在 pebrel 仓库执行 `git log e537d528..HEAD` 获得增量提交清单，按提交逐个评审是否回采；不引入 git remote / submodule 机制。

## 产品定位约束（不可违背，spec 全程筛选与验收基准）

1. Windows 10/11 原生（不走 WSL)
2. 单窗口单实例
3. 仅暗色模式
4. 渲染 GPU 加速
5. 复制 = Ctrl+Shift+C(Ctrl+C 保留为中断）
6. 默认 shell:pwsh → powershell → cmd 回退
7. 面向所有 AI CLI 调优（27 家 CLI 一等支持；2026-10-04 用户修订：不再仅限 Claude Code)

## 基线决策

| 决策点 | 结论 |
|---|---|
| 迁移方式 | fork 改造：pebrel 代码直接拷贝进本仓库再修改；沿用本仓库 git log，历史不丢失 |
| 架构 | Tauri 2 两进程（Rust+React/xterm.js)→ 单进程 GPUI 模块化单体；渲染 xterm.js → 自绘内核；链路 WebView2→IPC→ConPTY 缩短为 ConPTY 直连 |
| LICENSE | MIT → GPL-3.0(pebrel 上游即 GPL-3.0，派生合规要求）;THIRD-PARTY-NOTICES + licenses/ 合规族照抄 |
| 命名 | 产品名保留 slTerminal;`nebula_*` / `pebrel_*` crate、标识符、环境变量、文件名全量改名 `slterm_*` / `SLTERM_*`;pebrel 的双名兼容层（无历史用户）一律不迁 |
| workspace 目标形态 | `slterm_app`(application)+ `slterm_terminal` / `slterm_split` / `slterm_settings` / `slterm_completions` / `slterm_hook`(core,settings/split/hook 零生产依赖）+ `slterm_config`(±derive，落位时定）;rust 1.97.1 + edition 2024 仓内钉版 |
| GPUI 依赖 | 照抄 pebrel 钉版：Kuddev/zed fork rev `fc05d637`(gpui/gpui_platform)+ Kuddev/gpui-component fork rev `fc5f5cf`；补丁注释治理（原因+范围+上游撤销条件）照抄 |
| 测试体系 | L2(vitest)/L3(xterm-headless)/L4(wdio）随 WebView 全灭；新形态 = Rust 单测主力 + gpui-test-support 虚拟窗口 UI 测试 + 架构门禁三件套（file-budgets 棘轮 + dependencies.toml 依赖方向 + 禁名回流）+ win32 输入基线矩阵；豁免登记制度延续 |
| 文档纪律 | 混合：CLAUDE.md 渐进披露 / adr.md / CONTEXT.md / test-exemptions.md 保留；补 architecture/notes 因果 note(Supersedes 链 + Revisit when 撤销条件）与 docs 防腐条款；adr.md 管当前生效决策、notes 管成因档案，双轨互链 |

## 采纳总表（照抄 / 参考，按领域）

| 领域 | 核心采纳点 | 分片 |
|---|---|---|
| 终端核心 | `nebula_terminal` 整体照抄改名：vte 0.15 不 fork + OSC tee 嗅探（7/9;9/52/133/1337)、TermMode 全集（含 kitty 五标志）、Grid 环形 scrollback + resize/reflow + ConPTY 行锚定、三层渲染合同（viewport 协议 + 单调 revision 快照）、boxdraw 纯几何字形、ConPTY 侧载优先 + DA1 priming + 可中断读写 + drain_detached、event_loop 全链（resize 流边界 + 120ms 光标对账）、选择/搜索/vi 模式、FairMutex、pty_trace | 02 |
| 终端核心（自有资产并入） | slTerminal pty 五件套迁入：DA1/DSR 接管纯函数族、ConPTY flags 能力矩阵（0x1/0x2/0x4/0x8)、Win10 NuGet 捆绑（include_bytes! 嵌入）、Job Object 孤儿防护、SPAWN_LOCK 串行化、shell 白名单深检、pwsh EncodedCommand 集成（B17 守卫）;OSC 52/133 壳侧消费语义上移 | 02 |
| AI CLI 集成 | hook 三层拓扑照抄：`slterm-hook` 纯 std 小进程（exit 0/1MB/2s/stdin 抽干）+ 命名管道服务器（内核进程身份核验）+ 有界仲裁（AiHookEventGate 七判 + AgentActivity 每 pane);`SLTERM_NOTIFY_PIPE` 作用域闸门 + 串台双门；安装权威（codex 版本分流、marker 认领、编辑保留、不修复式覆盖）+ 9 家一等安装器；`AgentKind` 27 家枚举不裁剪；agent_detection TOML 声明式屏幕规则 + screen_context 结构判据 + 用户覆盖；ai_sessions 头部有界扫描 + hook 注册表合并；assistant_answer 有界 transcript 投影；per-pane 环境契约（SLTERM_* 单名） | 03 |
| AI CLI 集成（自有替代） | slTerminal hooks 信号文件通道退役（管道直连替代）;SEC-12 statusline 审查门保留并入新安装器；statusline 用量桥接分通道保留；agent_history 扫描被 ai_sessions 形态吸收；身份事实单源 AgentKind,cliProfiles 退化为 UI 表现层 | 03 |
| Runtime 控制 API | 整体照抄：loopback JSON Lines + runtime.port(port+128bit token)、版本化信封 deny_unknown_fields、RuntimeHub 单一状态权威 + 单调 revision、state_change_seq/after_seq 等待基线、generation 绑定、UI owner 线程执行写操作、pane/agent/tab/git 方法族（参数校验形态）、pane.exec 非 TTY 子进程、orchestrate 单请求编排、资源+动词 CLI、`slterm env` 离线发现、SKILL.md 自发现、SLTERM_RUNTIME_ENDPOINT 注入 + 三级发现优先级 | 04 |
| 单实例 | 移交不驻留：二次启动经 loopback 交「还原窗口 + 新开 tab」给首实例；陈旧端口文件 400ms 超时接管；关窗即退出（驻留保活砍） | 04 |
| 工作区与布局 | `nebula_split` 纯函数分屏树照抄（切割数学、拖拽比例三段曲线、6% 拖关、SplitNav 4 倍漂移惩罚、RemoveOutcome 三态、dock 嫁接）;WorkspaceTab 三件套 + 「pane 集合==树叶」不变式；session v4 schema(permille 比例、1Hz 快照无变化跳过、boot_attempts 断路 + quarantine、clean_exit、原子写、快照即 workspace 导出）——语义重定位为「下次启动恢复布局」;Dockview 体系全弃；面板封闭/布局单点约束以 Rust 封闭枚举 + 单一转换函数对重建 | 05 |
| 设置 | `RuntimeSettings` 权威大结构 + 键域枚举族 + 宽容解析 + 显式值权威三元模式照抄；持久化 JSON 化（砍 txt 与 Lua),slTerminal settings.rs 原子写/.bak/保存锁/损坏三态保留为写通道底座；GUI 设置页照抄（路由式搜索中英关键词索引、localized_select_labels 单点、预览不脏盘/提交可回滚/失败可感知）;settingsCenter 注册表契约保留 | 06 |
| 主题 | TermTheme/ReviewedPalette 语义槽 + custom_theme 值模型 + WCAG 对比度校验（foreground_recommendations)+ theme_library 版本化文档 + 主题包 ZIP 照抄；仅暗色（亮主题全砍）；配色单点纪律跨语言重建（语义槽 + 区域组融合） | 06 |
| i18n | build.rs 静态生成（typed ID + MESSAGES 零分配表）+ en/zh-CN 键集硬合同 + 命名占位符 + 独立合同 workspace 照抄；语言只留 en-US + zh-CN;locale 探测 | 06 |
| 文件与编辑 | file_editor 双模态编辑器整体照抄（live_edit WYSIWYG 投影内核、EditHistory 统一撤销、Outline 四索引同源、虚拟化预览、结构化块事务、input_rules、图片压帧缓存）;code_tab（三栏合并器）+ doc_tabs（图片 tab)+ openable_in_app 路由；file_tree 模型/渲染分层 + path_bar + 回收站删除 + 文件拖放；键位 keybind_pairs 语义 JSON 化 | 07 |
| 文件与编辑（自有归宿） | CM6 编辑器栈与 docViewer iframe 沙箱随 WebView 消亡；能力映射：打开/保存/外部改动→file_editor,git gutter/通用 diff→实现期新建项，markdown→双模态，html→源码只读，gitshow→只读 code tab，大文件→截断 + truncated 提示，explorer 能力→file_tree,fileViewers 注册表→openable_in_app;git 子进程封装保留并补 --no-optional-locks 纪律 | 07 |
| 补全 | pebrel-completions 引擎照抄（三算法匹配、四方言 command_context、git/scripts 语义源纯描述）;fish 式本地历史（WSL/SSH 池砍）、directory_history frecency、git_completion 有界发现、project_scripts 不执行项目代码、ghost + popup 双形态、CompletionStyle 三态 | 08 |
| AI assistant | 照抄（默认关闭）:OSC 133;D 非零退出→便宜门链→LLM 建议条（Ctrl+. 只贴不执行）;should_suggest 防误触规则表、grid_output_tail ≤2000 字符、redact_secrets 打码、is_dangerous 词表；ai_providers 十三家枚举 + 四协议族 + 独立凭据文件 | 08 |
| 数学渲染 | math/ 引擎（parser→validate→IR→layout→compile→rasterizer→cache)+ 终端覆盖层（有界扫描、LineProjection)+ markdown 数学管线 + gpui-component 薄桥照抄；光学常数族注释（决策记录）随迁；scientific_corpus 数学部分保留 | 08 |
| 系统集成 | 托盘（独立 Win32 隐藏窗口 + 消息泵 + attention 橙点 + agent 菜单直达）、AUMID 注册表 toast（免 COM/免安装器）+ 单一漏斗 + 双层节流 + 失败冷却 + 点击回焦、OSC 9;4 任务栏进度（规范外码归清除）、自动更新（GitHub Releases best-effort + 资产四重校验 + .part 流式 + 两阶段 handoff + restore ticket)、开机启动（Startup .lnk + 静默三条件）、窗口特效（GPUI 原生 Mica/透明度只透底色/壁纸有界解码）照抄 | 09 |
| 安全 | 凭据管理器通用封装（platform/credentials 三原语 + Win32 原语）+ provider 凭据独立文件 + 引用字段 + 尾四掩码 + 写即清稿 + Zeroizing 照抄；AES-256-GCM 加密备份（Argon2id + 魔数 AAD + 白名单即边界 + 先验后写 + 恢复点撤回）照抄；远端推送不建 | 10 |
| 测试 | pebrel 分层照抄：三文件模式（tests.rs/native_tests.rs/contract_tests)、gpui-test-support + 虚拟窗口补丁 + ui_tests.rs「真实控件与布局路径」纪律、键绑定真实分发测试、file_line_budget Rust 适配器、windows_console_startup 技巧、门禁自测四件、win32 输入矩阵、conformance(Runtime API 驱动）；架构门禁三件套落地本地（无 CI);L2/L3/L4 资产按类别重生（协议语义→slterm_terminal、载体无关契约→Rust 注册表/serde、行为类→UI 测试 + conformance) | 11 |
| 打包 | Inno Setup 主形态照抄（installer.iss 全参数化、PrivilegesRequired=lowest、双语向导 isl pin)+ 新鲜度核验链（禁 SkipBuild、版本回读）+ staging 清单全等校验 + zip 便携副形态（共享 manifest)+ 体积工程（opt-level="s" + 热路径钉 O3 + 预算钉测试，数字首版实测校准）+ 发布核验纪律 + release notes 双语形态 | 12 |

## 自有功能移植清单（pebrel 无对应物，保留移植）

| 功能 | 移植方向 | 分片 |
|---|---|---|
| plan_balance 套餐余额 | 查询语义原样（双注册表、快照合并、emit 口径、实证红线）;「token 不出后端」升格为「token 不出凭据域」（类型层无 token 字段 + Zeroizing 即用即焚 + 日志不插值）；与 ai_providers 凭据库不共享存储（SEC-18 唯一真源 = user 层 settings.json) | 10 |
| 安全审计 | hooks 域审计三件套（SEC-12 statusline 审查 / SEC-13 Outdated 防篡改 / SEC-17 user 层写入审计）原样存活；SEC 清单按「OS 面保留 / IPC-webview 面消亡」二分归档；SEC-18 红线延续并扩展至新凭据面 | 10 |
| git/fs/notify/projects/preview | git 子进程封装保留；fs 命令层消亡（std::fs 直连，CRLF 检测等语义并入）;notify watcher 保留；IPC/DTO/沙箱随单进程消亡 | 02/07 |
| agent_dirs / agent_history / background_tasks | agent_dirs 目录表扩至 27 家（沙箱放行真值源）;agent_history 被 ai_sessions 吸收；background_tasks 收编为 Rust 任务注册表 + GPUI executor | 03/09 |
| cliProfiles / agentFiles / navTree / sideViews / titleBar / shortcuts / notifications | cliProfiles 退化为 UI 表现层（身份单源 AgentKind);agentFiles/navTree 挂 workspace 侧栏骨架重建；shortcuts 四纪律映射 GPUI 键位模型；notifications 前端消亡、Rust 漏斗重生；titleBar/sideViews 壳重建 | 03/05/07/09 |
| theme/schemes | 多配色方案 → 内置主题目录机制；UiTokens 区域组与 ReviewedPalette 融合；配色单点纪律跨语言重建 | 06 |

## 不采纳总表

| 类别 | 内容 |
|---|---|
| 多平台 | tty/unix、Unix shell 集成（zsh/bash/fish 注入）、Linux/macOS 打包与自更新资产、macOS locale/凭据后端、x11-clipboard patch、WSL 全家（探测/历史池/路径/补全/启动身份） |
| SSH / 远程 | ssh_session / ssh_credentials / sftp / remote_files / ai_hook remote 安装与 OSC 777 通道 / completion connections 源 / Execution::Ssh / mobile 整目录（Android + relay + link)/ conversation.* / mobile_bridge / 远端备份推送（WebDAV/S3/SFTP) |
| Lua 配置 | mlua/vendored Lua、config_builder、nebula.toml 层级、nebula_settings.txt key=value 格式（设置持久化改 JSON) |
| Quick terminal | 违反单窗口定位，全砍（设置模型/键位行/工作区模块） |
| mux 驻留保活 | 关窗即退出；detached tab 恢复、会话驻留语义全砍，仅留单实例移交机制 |
| 科学渲染（部分） | 化学渲染（上游已停用）+ 生物语料 + 语料分子 case 砍；数学渲染保留 |
| i18n（部分） | 9 种额外语言目录砍，只留 en-US + zh-CN |
| 主题（部分） | 亮主题全套（LIGHT_ANSI 替换表、follow_system_theme、is_light 分支） |
| legacy 旧壳 | OpenGL renderer/display 旧渲染路径、legacy-shell feature 通道（winit EventLoopProxy 全族）、third_party/winit patch、window_transition、nebula_gpui 实验场 |
| 兼容层 | 全部 PEBREL_*/NEBULA_* 双名并写、legacy marker/迁移链、旧品牌资产名兼容（全新 fork 无历史用户） |
| CI / 发布编排 | GitHub workflows 本体、CI 专用自测、GitHub Release 编排（stable/preview_release.py 编排面；资产校验纯逻辑保留）、pr-size 阻断、MSIX/Scoop |
| 其他 | pebrel 特有产品面：应用图标 25 色板、TermTheme.powerline 八槽、广播输入、tabs 位置配置化、OS 凭据提示框、安装器附加任务（字体/PATH/右键菜单/WSL 子菜单）、installer-migration.iss |

## 分片索引

| 文件 | 范围 |
|---|---|
| `01-arch-baseline.md` | fork 基线/commit 记录、crate 重组与改名、总删减清单、依赖方向与 ownership map、GPUI 依赖钉版、toolchain 对齐、src-tauri/src 归宿原则 |
| `02-terminal-core.md` | nebula_terminal 照抄要点（VT/OSC/grid/渲染合同/boxdraw/ConPTY/输入/选择）+ slTerminal pty 五件套并入 + xterm.js 消亡清单 |
| `03-ai-cli-integration.md` | hook 三层拓扑、安装策略、屏幕规则、会话恢复、环境契约、27 家清单；slTerminal hooks 通道退役与合流 |
| `04-runtime-api.md` | Runtime 控制 API 照抄要点、SKILL 自发现、generation 绑定；单实例移交（驻留砍） |
| `05-workspace-layout.md` | nebula_split、WorkspaceTab、session v4（语义重定位）;Dockview 弃用与硬约束重建形态 |
| `06-settings-theme-i18n.md` | 设置 JSON 化、GUI 设置页、主题 WCAG、i18n 双语机制；hooks 配置双轨归位 |
| `07-files-editor.md` | file_editor/file_tree/tab 路由照抄；slTerminal 编辑器与预览面板归宿映射；键位 |
| `08-ai-assistants.md` | 补全引擎、AI assistant、数学渲染；runtime skills 投放 |
| `09-system-integration.md` | 托盘/通知/任务栏/自动更新/开机启动/窗口特效/DPI |
| `10-security-plan.md` | 凭据三件套、加密备份；plan_balance 与安全审计移植；凭据存储共享裁定 |
| `11-testing.md` | 测试体系新形态、架构门禁三件套、L2/L3/L4 资产重生类别 |
| `12-docs-packaging.md` | 文档纪律混合终态、Inno 打包、体积工程、LICENSE 切换、release notes |
