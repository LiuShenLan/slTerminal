# REVIEW-c 可行性与风险（角度 C)

> 对抗性 review 第三棒（A=跨篇一致性、B=spec 一致性、C=可行性与风险）。核查基准:design = `docs/pebrel-design/`(00+01–12+DECISIONS);pebrel 源码 baseline `e537d528`;GPUI 源码 = 本地 cargo 缓存 Kuddev/zed rev `fc05d63`（与设计钉版同 rev)。与 A/B 不重复:D04-1/D05-x/D06-1/D10-1 等裁决-正文冲突已由 A 覆盖，本报告只收其实现落点与行为后果面。

## 🔴 阻塞

### C-1 | M3（首个可运行态）无归属设计篇；GPUI 终端视图移植无签名级设计承载

- 定位:00-roadmap「阶段序列」M3 行;05 篇「与 M3 的接口契约」节;02 篇缝合点 1/4、xterm.js 消亡清单表;04 篇改造节 1/3。
- 问题:13 篇中每个 M 都有归属篇（M0/M1→01、M2→02、M4→03、M5→05、M6→06、M7→04、M8→07、M9→08、M10→09+10、M11→12),**唯 M3 无任何篇承接细分步与签名级设计**。M3 的主体 = pebrel `nebula_app/src/gpui_shell/terminal/`(view/element/keymap/session_pump/event_mailbox/mouse_protocol/copy_feedback/IME/inline_image 等，实测约 2.1 万行）的移植——02 篇把它推给「壳归 M3」且 core 纪律禁 GPUI 入 core;05 篇明说「终端视图归 02/M3；本体本片不展开」;04 篇只给启动骨架。M3 出口（虚拟窗口 UI 测试终端渲染关键路径过）的**被测对象本身**无设计承载；渲染（GPUI element 自绘 RenderSnapshot/字形 atlas/boxdraw 几何）、输入（keymap 三路径编码器）、IME 三件 M3 必需件均无归属；亦无 M3.x 拆分子步缓冲（app 是逐 crate 迁入链中耦合面最大的阶段）。
- 影响:M3 是全路线最高风险阶段（首窗口点亮 + 最大单件移植 + 壳事件队列/RuntimeDispatch 通道的 M7 预埋缝都在此定型）,无签名级设计 = 实现期现拍架构，返工面最大且直接卡在 M4/M5/M7 全部下游的入口。
- 建议：补 M3 设计篇（或明令 05 篇扩编承接）,至少给出 `gpui_shell/terminal/` 移植裁剪清单、TerminalView/TerminalElement 与 02 篇 RenderSnapshot/ViewportTracker 的缝合签名、壳事件队列形态（M7 RuntimeCallback 的预埋缝）与 M3.x 细分步。

## 🟡 应修

### C-2 | 00-roadmap M3 声称「消费 RuntimeHub 状态权威」,RuntimeHub 本体归 04 篇 M7.2

- 定位:00-roadmap 阶段序列 M3 行（「壳即前端，消费 RuntimeHub 状态权威」);04 篇阶段归属 M7.2(RuntimeHub + 投影入账）。
- 问题:M3–M6 壳的状态权威形态无设计——是 M3 先落最小 hub、还是壳直读 Workspace 待 M7 再接，无任何篇裁定；05 篇 M3 交付清单与 11 篇 M3 伴生行均无 RuntimeHub。
- 影响：若 M3 直读 Workspace,M7.2 要把投影/publish 链重新插进已运行的壳（返工面随 M4–M6 叠加）;若 M3 要预埋 hub，则 04 篇欠一个 M3 最小件契约。
- 建议：一句话裁定并回写 roadmap M3 措辞——推荐「M3 直读 Workspace,M7.2 投影函数以 facade 缝接入」或「M3 携最小 RuntimeHub（仅 publish/current)」二选一。

### C-3 | 03 篇 M4.5 工作项系统性前倾消费后阶段件

- 定位:03 篇阶段归属「M4.5 壳侧点亮」;对照 05 篇 M5.3/M5.5、09 篇 M10.1、05 篇「与 M3 的接口契约」M3 交付清单。
- 问题:M4.5 列的三件工作均消费未落地件——「05 篇 `restore_agent` 缝合」（恢复注入执行器归 M5.5)、「09 篇通知漏斗接通」（漏斗归 M10.1)、「`TerminalPane` 挂 `AgentActivity`/`AnswerInbox`」(`TerminalPane` 三件套锚定 05 篇、M5.3 才落；M3 交付清单只有 `TerminalView` 实体，M4 时 pane 宿主结构无定义）。M4.5 出口本身不含这些件，绿灯不受阻塞，但工作项清单与阶段错位。
- 影响：落地时三件必然搁置（形成隐性欠账）或倒挂实施（抢跑未设计件）;05/09 各自阶段开工时还要回认 M4 的临时形态。
- 建议:M4.5 工作项改写为「AgentActivity 挂 M3 单 pane 宿主（形态归 M3 篇）」;`restore_agent` 缝合与通知漏斗分别回登 05 篇 M5.5、09 篇 M10.1 工作项。

### C-4 | M2 出口含 win32 输入矩阵基线，但 M2 无可驱动 exe（首个 exe M3 才存在）

- 定位:00-roadmap M2 出口（「`cargo test` 全绿（含 ConPTY 集成用例、win32 输入矩阵基线）」);02 篇 M2.4;11 篇伴生表 M2 行（「win32 矩阵脚本 + 基线 -Record 建基」）与关键类型节（矩阵脚本 `$Exe` 默认 `target\debug\slterm.exe`,PostMessage 驱动最小化实例）。
- 问题：矩阵是「驱动真实 exe 窗口 → 探针收字节 → 逐字节比对」的端到端基线;M2 是纯库态（00-roadmap 自陈 M0–M2 无可运行产品）,没有可 PostMessage 的 slterm.exe,`-Record` 无对象可录，「常挂比对过」无机可跑。
- 影响:M2/M3 两个出口对同一基线的归属错位——按现文 M2 出口不可达成，实际只能在 M3 补录；绿灯语义（出口只认可机验项）在此处给出不可机验项。
- 建议：win32 基线建基与常挂比对移至 M3 出口（M3 已有 exe + 键位语义）,M2 出口收窄为「矩阵脚本 GPUI 化改造完成 + 基线语义登记」。

### C-5 | D04-1 console 子系统：技术可行已证实，但 shell 阻塞后果与启动链改造落点无归属

- 定位:DECISIONS A 表 D04-1;04 篇待沉淀 D04-1;11 篇关键类型节 windows_console_startup 段；pebrel `nebula_app/src/platform/startup/console.rs`(`prepare_console_for_gui`);GPUI 核查 = Kuddev/zed `fc05d63` `crates/gpui_windows/`(平台层零 console/AttachConsole/AllocConsole 依赖，纯 Win32 窗口 + DirectX——console 子系统不阻塞 GPUI 启动链，裁决技术可行）。
- 问题:(a) console 子系统下从 pwsh/cmd 启动 `slterm`(GUI 人格）,父 shell 同步等待子进程退出——即「从终端里开终端会卡住原终端」,比 DECISIONS 已接受的「闪控制台窗」更影响自家用态，无任何篇登记该后果与缓解（快捷方式/Start-Process/文档明示）;(b) pebrel 的 `prepare_console_for_gui` 启动链（GUI 子系统前提）在 console 本体下语义反转（继承父控制台，FreeConsole 时机与后果全变）,09 篇（console 启动准备归其所有，据 11 篇）无对应改造子步;(c) 11 篇 `windows_console_startup` 对照目标在 console 本体下理据失效（A-30 已记文档冲突）,其存废/重设的**实现子步**无归属。
- 影响:(a) 属发布级行为意外，首发即被自家目标用户（AI CLI 重度 shell 用户）撞见;(b)(c) 属 M3 启动链工作，无子步承接 = 实现期裸改。
- 建议：一句补登——DECISIONS D04-1 备注追加「shell 同步等待后果 + 缓解口径」;09 篇（或 M3 归属篇）增列 console 启动链改造与对照测试重设子步。

### C-6 | `conpty_sideload_enabled` 改读 settings JSON,与「slterm_terminal 无本地边/路径权威在 settings crate」双源冲突

- 定位:02 篇关键类型 tty 面(`pub fn conpty_sideload_enabled() -> bool; // settings JSON 键承载（归 06）`)与 M2.3;01 篇 workspace 终态表（slterm_terminal 无本地边）与缝合点 1(「路径优先级唯一权威在 settings crate」);pebrel 现状 = `nebula_terminal/src/tty/windows/mod.rs` 内嵌自有目录推导 + txt 解析（`nebula_settings_value`)。
- 问题:slterm_terminal(core，无本地边）要读 settings.json 只能照 pebrel 旧病再内嵌一份「数据目录推导 + JSON 解析」——与 01 篇「路径权威唯一在 settings crate」直接双源（SLTERM_CONFIG_DIR/空串过滤/默认 %APPDATA% 逻辑将存在两份，演化必漂移）。
- 影响：设置路径优先级任何调整都要双侧同步，属已登记原则（01 缝合点 1）的结构性破例；且 06 篇键域未见该键登记（归属缝隙）。
- 建议：改由壳（app）读键后经 `Options`/Config 注入 core——pebrel `nebula_app/src/config/ui_config.rs` 计算 `suppress_bringup_da1` 已是同形态先例，core 侧零路径逻辑。

### C-7 | M10 体量 2.4 倍于其它阶段，且自动更新链与凭据域两面同压一段

- 定位:09 篇阶段归属 M10.1–M10.8;10 篇阶段归属 M10.9–M10.12；对照 02/03/04/05/06 篇各 4–5 子步。
- 问题:M10 合计 12 子步（通知/任务栏/托盘/窗口管理/自动更新/开机启动/特效/后台任务 + 凭据/加密备份/plan_balance/审计）,为任一相邻阶段（4–5 步）的 2.4 倍；其中 M10.5 自动更新链（检查→下载→handoff 两阶段自覆盖）单独即一子系统，凭据域（类型层边界）与系统集成（大量 Win32 面）风险性质互异，共享一段出口。
- 影响：末段体量集中 = 点亮节奏尾部淤积；12 子步共用一个 M 出口，任一子步红灯即整段不过，进度颗粒度失真。
- 建议：拆为「M10 系统集成 1(M10.1–M10.4)」「M10′ 系统集成 2 + 安全（M10.5–M10.12)」两段，或在 00-roadmap 显式登记该段体量及其理由。

## 🟢 建议

### C-8 | 段嵌套（D06-1）的键→段归属表与三清单重构量未挂任何子步

- 定位:06 篇关键类型/实例（全按平铺，A-13 已记文档面）、D06-1 待沉淀（自陈「RESET_KEYS/白名单/键域枚举三清单都是平铺键集合」「keybind/conpty_input_modes 两域外段引入跨篇段归属协议」)、M6.1 子步。
- 问题：段嵌套裁定后，哪些键进哪段的映射表不存在；白名单（拒未知顶层键→段嵌套下顶层只剩段名，校验粒度变化）、RESET_KEYS、键域枚举三清单须按段重构；跨篇段归属协议（07 keybind、02 conpty_input_modes）无承接点。M6.1 写通道落地前这批设计须先产出，未挂子步。
- 影响:M6.1 开工即撞设计真空，写通道与三清单返工一次。
- 建议:M6.1 子步补前置项「键→段映射表 + 三清单段形态先行（含 07/02 段归属回登）」。

### C-9 |（待核）Kuddev fork 虚拟窗口补丁点未逐行核实

- 定位:11 篇关键类型 feature 开关段（「TestWindow 对不存在的原生窗口/显示句柄返回 Unavailable 而非 panic」);01 篇钉版注释（组件分支 `nebula-v1.16.1-test-support`)。
- 问题：本地缓存（rev fc05d63，与钉版一致）确认 fork 含 `gpui_windows` test-support feature 与 `VisualTestContext` 基链，但「句柄缺失返回 Unavailable」的具体补丁点未逐行核到（缓存中未见直接对应代码段，可能在 window.rs 条件编译分支）。缺证据：逐行 diff 补丁分支。
- 影响：若补丁形态与设计假设不符，M3 出口（虚拟窗口测试）直接卡死——但该风险由 M3 首个 ui_tests.rs 冒烟天然兜住，代价低。
- 建议:M3 开工第一步跑首个 ui_tests.rs 冒烟用例即闭环，无需事前补查。

## 核查通过项（抽查结论，防重复排查）

- **签名级抽查 8/8 与 pebrel 一致**（无结构性误判）:02 篇 RenderSnapshot/TerminalViewport/ViewportTracker(= `render.rs`)、Event 枚举(= `event.rs`)、StreamProcessor/Msg/READ_BUFFER 1MiB/MAX_LOCKED_READ 64KiB/ALIGN_DELAY 120ms(= `event_loop.rs`)、tty Options/Pty::new(= `tty/mod.rs`、`tty/windows/mod.rs`)、DA1 priming `\x1b[?61c`(= `conpty.rs`);03 篇 `spawn_gpui_server`/`spawn_pipe_server`/`serve` 闭包形态与环境闸门(= `ai_hook/windows.rs`);04 篇 RuntimeHub(Arc<Mutex<HubState>>/publish/subscribe cap 16/移交 400+700ms 常量，= `runtime_api.rs`、`server.rs`、`mux.rs`);05 篇 `layout_from_tree`/`tree_from_layout` 转换对(= `gpui_shell/session_restore.rs`)。
- **提取清单无空洞**:01 篇 M0 提取源（pty 六件/settings.rs/plan_balance/hooks/git/notify/preview/fs/agent_dirs/agent_history/background_tasks/projects.rs）逐一在 slTerminal git HEAD 核实存在；M3–M5 关窗丢布局的过渡态 05 篇已显式登记为预期态，无设计真空。
- **测试伴生序**:11 篇伴生表 M0–M11 行覆盖各 M 出口所需测试基础设施（M3 虚拟窗口、M7 conformance、M10 内存压测等）,除 C-4 外无「出口先于基础设施」倒挂。
