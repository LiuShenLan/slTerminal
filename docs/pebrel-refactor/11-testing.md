# pebrel 重构优化 spec · 分片 11：测试体系

baseline commit：`e537d528c508e8607d0f5f9fd25e5902f40d661e`（pebrel，Rust + GPUI 模块化单体）

## 优化面

本分片覆盖六件事：

1. **测试总形态重建**——既定全局决策：现四级金字塔中 L2（vitest+jsdom）/ L3（xterm-headless）/ L4（wdio+embedded driver）随 WebView 前端全灭；L1（cargo test）部分存活并扩展为绝对主力。本分片给出 pebrel 测试分层逐类裁定，以及填补 UI 覆盖空缺的 GPUI 虚拟窗口测试层。
2. **pebrel 测试分层逐一裁定**——crate 内单测（tests.rs / native_tests.rs / *_contract_tests.rs 模式）、集成测试（nebula_app/tests/ 四件）、GPUI UI 测试（gpui-test-support feature + 各模块 ui_tests.rs）、脚本层（scripts/tests/ Python unittest + PS1 + mjs）、平台一致性（win32_input_matrix.ps1 + baseline、scripts/conformance）。
3. **CI 门禁的本地化落地**——slTerminal 无 CI（本地开发），裁定 pebrel .github/workflows/ 中哪些步骤内容落地为本地脚本、哪些随 CI 本体不采纳。
4. **架构门禁三件套**——check_architecture.py（file-budgets.txt 行数棘轮 + dependencies.toml 依赖方向合一入口）、check_prohibited_names.py（禁名回流）、门禁自测（scripts/tests/test_architecture_*）的逐条裁定；slTerminal 版禁名清单 = 禁 nebula / pebrel 回流。
5. **slTerminal 侧现状对照**——L1 定向红线 TQ-COV-06（comctl32 v6 manifest / 0xc0000139）fork 后存续性判定、SPAWN_LOCK 串行化、BE-06 全量回归串行纪律与负载裕度登记口径、测试命名约定、test-exemptions.md 豁免登记制度的延续。
6. **L2/L3/L4 测试资产的重生类别**——只列类别方向（终端协议语义、布局 serde、注册表契约、键绑定、终端生命周期、设置落盘等），不逐文件搬。

## 采纳点

### crate 内单测组织

1. **照抄：领域内嵌测试模块三文件模式**。pebrel 侧位置：`nebula_app/src/gpui_shell/file_editor/tests.rs`、`nebula_app/src/ai_hook/native_tests.rs`、`nebula_terminal/src/term/keyboard_contract_tests.rs`。形态：超大测试面从源文件拆出独立测试文件挂载；`native_tests.rs` 专放需真桌面/真窗口的用例（统一 `#[ignore]` + 环境变量门禁，如 `PEBREL_CURSOR_QA_DIR`）；`*_contract_tests.rs` 专放协议/契约矩阵。理由：与 slTerminal 现有「一文件多测试模块用功能具名」规则（`mod <领域>_tests`）同源，文件级拆分是自然延伸，解决 Rust 单体世界单文件测试膨胀问题。
2. **照抄：键盘协议契约测试形态**。pebrel 侧位置：`nebula_terminal/src/term/keyboard_contract_tests.rs` 的 `assert_steps` + `Replies`（`EventListener` 实现收集 `Event::PtyWrite`）——字节进 / 字节出矩阵 + kitty 协议应答断言，纯编译期无窗口。理由：这是 slTerminal L3 `test/terminal/keyboard-encoding.test.ts`、`ansi-correctness.test.ts` 等用例在 Rust 世界的直接重生形态，「面向所有 AI CLI 调优」的 kitty 协议兼容语义必须以此锚定。
3. **参考：native_tests.rs 真桌面 QA 用例的登记形态**。pebrel 侧位置：`nebula_app/src/gpui_shell/settings_pane/cursor_motion/native_tests.rs` 的 `native_cursor_motion_menu_keeps_all_choices_visible`（`#[ignore = "requires a native desktop ..."]` + 绝对输出目录 + 隔离 `PEBREL_CONFIG_DIR`）。理由：GPUI 虚拟窗口覆盖绝大部分 UI 回归，但原生外观、IME 合成等少数路径仍需真桌面；统一「ignore + 环境门禁 + 隔离配置目录」形态后，这些用例进 test-exemptions.md 的实机验收点清单而非散落。
4. **照抄：i18n 热路径零分配契约**。pebrel 侧位置：`tools/i18n-contract`（独立小 crate）+ `nebula_app/tests/i18n_contract.rs` 的 `CountingAllocator`（`#[global_allocator]` 计数 + `first_and_repeated_translation_lookups_allocate_nothing`）。理由：首次与重复翻译查找零分配的回归锚，GPUI 单进程 UI 的翻译查找频率不低于 pebrel；若 slTerminal 落地 i18n 则整体照抄（见「不采纳点」第 5 条的对偶裁定）。

### 集成测试（nebula_app/tests/）

5. **照抄：file_line_budget.rs——同一 budget 文件的 Rust 适配器**。pebrel 侧位置：`nebula_app/tests/file_line_budget.rs` 的 `every_source_file_respects_its_line_budget`（`include_str!` 读 `architecture/file-budgets.txt` → 按 root 递归统计 → 超限报错 / 额度失效报错）+ `physical_lines_ignore_encoding_and_newline_convention`（行数口径用例）。理由：Python 门禁与 Rust 测试消费同一数据文件（单一真值源双侧消费），slTerminal 以 Rust 为主体，此形态比纯 Python 更贴；改名后 `nebula_app` → `slterm_app`、路径同步。
6. **照抄：windows_console_startup.rs 的 console 子系统测试目标技巧**。pebrel 侧位置：`nebula_app/tests/windows_console_startup.rs`——`#![cfg(windows)]` + `#![windows_subsystem = "console"]` + `#[path = "../src/platform/startup/console.rs"]` 挂载被测源码与测试模块。理由：「console 子系统目标自动继承父进程 Ctrl+C 状态、GUI 目标 AttachConsole 会重置并掩盖 bug」这类控制台继承语义，只能在 console 子系统的测试目标里观测；GPUI 应用恰是 GUI 子系统，此对照测试在 slTerminal 同样必要。

### GPUI UI 测试层（填补 L2/L4 毁灭后的 UI 覆盖）

7. **照抄：gpui-test-support feature 三合一开关**。pebrel 侧位置：`nebula_app/Cargo.toml` 的 `gpui-test-support = ["gpui-shell", "gpui/test-support", "gpui-component/test-support"]`。理由：一条 feature 同时打开 gpui 与 gpui-component 的测试支持，虚拟窗口 UI 测试与普通单测在同一 crate 内共存，CI 与本地共用同一测试二进制形态。
8. **照抄：Kuddev fork 虚拟窗口补丁及其治理记录**。pebrel 侧位置：根 `Cargo.toml` 的「下游虚拟窗口测试支持」补丁条目（`crates/ui` 的 test-support feature；`src/lib.rs` 与 `src/root.rs` 在该模式下不安装需要原生窗口句柄的 macOS 无障碍转发器）与「窗口生命周期修补」注释块（`TestWindow` 对不存在的原生窗口/显示句柄返回 `Unavailable` 而非 panic；X11 队列 drain 问题）。理由：无原生窗口句柄跑完整对话框/控件回归，是 slTerminal 本地开发（无 CI、无 Xvfb 类设施）可重复 UI 测试的根基；补丁注释块「原因 + 范围 + 上游撤销条件」的治理结构随补丁一并照抄。
9. **照抄：#[gpui::test] + TestAppContext/VisualTestContext 的模块级 ui_tests.rs 组织**。pebrel 侧位置：`nebula_app/src/gpui_shell/workspace/settings_navigation/ui_tests.rs` 的 `open_workspace` / `click_tab` / `tab_bounds`（`window.refresh` + `draw` + `debug_bounds` 取真实布局坐标 + `simulate_click`）与 `switching_top_tabs_keeps_settings_reachable_until_explicit_close`；同族分布于 `nebula_app/src/gpui_shell/file_editor/code_actions_tests.rs`、`inline_selection_tests.rs`、`settings_pane/cursor_motion/tests.rs`、`settings_pane/segmented/tests.rs`、`platform/acrylic/gpui_tests.rs` 等。理由：这就是「测试真实控件和布局路径」的执行模板——编译/状态单测不能代替 hover / 命中 / 键盘路径，GPUI 虚拟窗口让这套测试在无头本地环境可跑。
10. **照抄：「真实控件与布局路径」交互测试纪律**。pebrel 侧位置：`docs/project-constraints.md` §5（§5.2 复制操作合同：复制必须有可感知反馈、成功失败不能只靠颜色区分；交互测试条款：点击真实布局与控件、覆盖点击留白、hover 显示、键盘路径、重复触发；布局测试核对真实命中范围、工具栏不占位、选中线可见、长行/窄窗口）。理由：L2 组件测试毁灭后，这是 UI 行为覆盖的唯一权威纪律来源，且与 slTerminal「复制 = Ctrl+Shift+C」「仅暗色」定位直接咬合。
11. **照抄：键绑定走真实分发的 UI 测试**。pebrel 侧位置：`nebula_app/src/gpui_shell/terminal/keymap.rs` 的 `mod tests`（`Keystroke` 构造后注入真实分发路径断言动作命中）。理由：slTerminal 产品定位的核心键盘语义（复制 = Ctrl+Shift+C、Ctrl+C 保留为中断、各 AI CLI 保留键）必须以「注入按键 → 观测动作」的端到端形态锚定，而非仅测解析函数。
12. **参考：gpui::test 用例与单测同仓混编的开关纪律**。pebrel 侧位置：全仓 `#[gpui::test]` 用例（数百量级）与 `#[test]` 混编于各模块测试文件，经 feature 统一门控。理由：slTerminal 沿用同一混编形态；feature 未开时 UI 用例整体不编译，门禁需一条命令同时覆盖两种形态（见「优化方向」）。

### 脚本层测试（scripts/tests/）

13. **照抄：脚本层测试目录形态与门禁自测**。pebrel 侧位置：`scripts/tests/`——`test_architecture_budgets.py`、`test_architecture_dependencies.py`、`test_architecture_governance.py`、`test_prohibited_names.py`（门禁自测四件，含 `fixtures/prohibited_names/ordinary-comparison.md` 假阳边界夹具）、`test_completion_editor.py`、`test_shell_integration.py`、`test_platform_cfg.py`、`test_windows_memory_stress.py` 等 Python unittest 为主，PS1（`run_completion_e2e.ps1`、`*.tests.ps1`）与 mjs（`opencode-notification-bridge.test.mjs`）按需补充。理由：无 CI 环境下，门禁脚本自身必须有测试，否则门禁静默失效无人知；「门禁先自测自身」是这套体系的命门。
14. **参考：completion e2e 验收脚本的隔离设计**。pebrel 侧位置：`scripts/tests/run_completion_e2e.ps1`（强制空输出目录 + `PEBREL_COMPLETION_QA_DIR` / `PEBREL_CONFIG_DIR` 隔离 + 可选 Prediction 通道）。理由：脚本形态与隔离设计照抄；其「必须在永不切换桌面的值守机上跑」的运行模式不采纳（见「不采纳点」）。
15. **参考：Windows 内存压测的规格化形态**。pebrel 侧位置：`scripts/tests/test_windows_memory_stress.py` 的 `sample` / `complete_samples` / `checkpoints` 相位采样模型 + `scripts/windows-memory-stress.md` 规格文档。理由：「终端多开/关闭后的私有提交内存回落」是终端产品长期质量指标，slTerminal 作为常驻单窗终端同样需要；落地为本地脚本 + 登记进验收清单。

### 平台一致性与输入基线

16. **照抄：win32_input_matrix.ps1 + 登记基线**。pebrel 侧位置：`scripts/win32_input_matrix.ps1`——PostMessage 驱动最小化实例（不抢焦点，用户同时在打字也可跑）→ 探针进程经 node reader 收集收到的字节 → 与 `scripts/win32_input_matrix.baseline.txt` 比对；`-Record` 重登记；矩阵含普通键、扩展键、裸修饰键（必须不产生字节）+ 哨兵键；LIMITS 注释明确 chord 需前台 SendInput 值守机、不无人值守。理由：Win32 输入管线（含 ConPTY 键盘输入模式）的字节级回归是终端产品的命脉，恰好 Windows-only 本地脚本与 slTerminal 定位重合；照抄时基线路径与产物名 nebula → slterm 化、exe 默认路径参数化（pebrel 默认值硬编码到 `D:\temp_build\nebula`，属本机残留）。
17. **参考：scripts/conformance 运行时一致性套件**。pebrel 侧位置：`scripts/conformance/README.md` + `harness.py` + `run.py`——私有 `NEBULA_CONFIG_DIR` 启动孤立应用进程，经 loopback TCP JSONL Runtime API 驱动 boot / echo / resize / split / scrollback / session / paste / cjk_roundtrip / close 用例，产出 report.json 与 golden 基线比对；明示不声称覆盖 IME 与剪贴板确认对话框（需 GUI 自动化，登记为不可达）。理由：这是「Runtime API 驱动的行为级端到端」形态，与分片 04 的 Runtime API 决策同源；slTerminal 版若保留 Runtime API 则照此建套件，若裁剪 API 面则相应裁剪用例；golden 报告的「新平台缺席不算失败」基线治理经验照抄。仅保留 windows 用例（见「不采纳点」）。

### CI 门禁内容本地化

18. **照抄：architecture.yml 的步骤序而非其载体**。pebrel 侧位置：`.github/workflows/architecture.yml`——「Test the guardrails themselves」（先跑四个门禁自测 unittest）→ 禁名回流 range 检查 → `check_architecture.py`（带 base 则 `--base`）→ 无 UI 依赖图的设置契约直编测试（`rustc --test` 单文件编译 `nebula_settings/src/lib.rs`）→ `file_line_budget.rs` 直编测试 → i18n 契约 → 品牌/资源测试。理由：slTerminal 无 CI，但步骤序所表达的「门禁先自测、再扫名、再查体积与方向、再跑契约测试」依赖关系是逻辑要求，与载体无关；落地为本地单一入口脚本（见「优化方向」）。
19. **照抄：门禁脚本纯本地化设计**。pebrel 侧位置：`scripts/check_architecture.py` 的 `Revision`（纯 `git rev-parse` / `ls-tree` / `show` 读基线，无 GitHub API 依赖）与 `check_prohibited_names.py` 的 `staged` / `message` / `push` / `range` 四模式（staged 查暂存、message 查提交信息文件、push 查待推送提交、range 查 CI 区间——前三模式纯本地）。理由：门禁脱离 CI 也能在本地对每个 commit 全量跑，这是「无 CI 仓库」采用 pebrel 门禁的前提条件。
20. **参考：pr-size.yml 的行数统计机制**。pebrel 侧位置：`.github/workflows/pr-size.yml`（1500 行硬限、lock / docs / 图片等生成物不计、size-exempt 绕行）。理由：无 PR 概念，阻断与标签机制不采纳；「commit range 变更行数统计 + 生成物排除」的统计口径落地为本地提示脚本（超阈警告不阻断），是否纳入日常入口由用户裁决。

### 架构门禁三件套（slTerminal 版）

21. **照抄：check_architecture.py 双检查合一入口**。pebrel 侧位置：`scripts/check_architecture.py` 的 `run`（`budgets.check` + `dependencies.check` 同跑，errors 合并）+ `main` 的 `--base` / `--report` 参数面；`scripts/architecture/budgets.py` 的 `Budget.parse` / `source_files` / `check`（`limit` / `root` / 例外三记录形态、例外必须超 limit 且属源后缀、禁绝对路径与 `..` 与符号链接、Cargo 目录自动排 `target`）、`--base` 相对基线语义（limit 不可升、root 不可减、例外额度只降不升、低于默认限即报错要求销额度、缩了提示 tighten、基线无此文件则打印初始债清单）。理由：行数棘轮是防模块退化的机械闸门，「存量按实测登记、只降不增、禁通配豁免」正是「未来最优」取向的机械执行器；slTerminal 版 crate 名换 `slterm_*`。
22. **照抄：architecture/dependencies.toml 声明式依赖方向**。pebrel 侧位置：`architecture/dependencies.toml`——每 crate 声明 `layer`（core / application / hook / lab）+ 三类依赖（`dependencies` / `build-dependencies` / `dev-dependencies`）白名单 + `zero_production_dependencies` 契约 + 顶层 `renderer_packages` 清单（gpui / gpui_platform / gpui-component 等渲染包禁入 core 生产依赖）；`scripts/architecture/dependencies.py` 的 `check`：workspace 成员必须显式枚举（禁通配）、成员与分类必须一一对应、path 依赖必须解析到成员、core/hook 层禁渲染包、core 只能依赖 core / application 禁依赖 lab、生产依赖图无环（config ↔ config_derive 的 dev 环为天然豁免因只查生产边）、自定义 target path 必须落在登记的 source root 内。理由：与硬约束 2「后端按功能分模块」同源，从「约定」升级为「机械检查」；slTerminal 版分层按分片 01 的目标 workspace 重写内容、照抄机制。
23. **照抄：check_prohibited_names.py 禁名回流门禁（slTerminal 版 = 禁 nebula / pebrel 回流）**。pebrel 侧位置：`scripts/check_prohibited_names.py` 的 `PROHIBITED_PATTERNS`（词边界正则表，大小写不敏感）/ `EXEMPT_PATHS`（仅门禁自身与其自测夹具可豁免——「门禁必须能维护自己的关键词表」）/ `LEGAL_ATTRIBUTION_PATTERNS` / `PROTOCOL_COMPATIBILITY_PATTERNS` / `DEPENDENCY_REFERENCE_PATTERNS`（协议兼容名、版权归属行、依赖坐标等精确豁免模式，剔除后同行其余文案仍检查，禁按目录或整文件豁免）/ `scan_pending_commits`（逐提交扫新增行，防「先加入后删除」在 range diff 中被抵消；merge commit 走 combined diff）/ `checked_range`（merge-base 校验，无法解析 fail-closed）。理由：产品改名（nebula → pebrel；slTerminal fork 后 nebula/pebrel 均属旧名）后旧名回流是真实腐化通道（pebrel 自身就是被改名产品）；slTerminal 版关键词表 = nebula / pebrel 及其组合形态，豁免模式同构（协议兼容引用、上游归属、依赖坐标）。
24. **照抄：门禁自测四件**。pebrel 侧位置：`scripts/tests/test_architecture_budgets.py`（Budget 解析与棘轮语义）、`test_architecture_dependencies.py`（layer / 环 / 白名单判定）、`test_architecture_governance.py`（文档链接完整性、notes 结构段要求——`NOTE_REQUIRED_SECTIONS` 的 Status / Context / Evidence / Decision 等九段）、`test_prohibited_names.py` + `fixtures/prohibited_names/`（假阳边界夹具）。理由：门禁自身的正确性由测试锁死，这是门禁能进本地日常入口的前提；governance 自测的「文档链接不悬空」检查与 slTerminal 文档规范天然互补。

### slTerminal 侧现状延续

25. **照抄延续：测试命名约定**。slTerminal 现状：`mod <领域>_tests` 领域具名 + 函数「对象_行为_场景」snake_case 裸名、文件级 kebab-case（L2/L3 将灭，此条主要指 L1 内嵌约定）。pebrel 同族实践位置：`nebula_app/src/gpui_shell/file_editor/code_actions_tests.rs` 等文件级拆分、各 `mod tests` / 领域具名测试模块。裁定：命名约定在 Rust 单体世界原样延续；pebrel 文件级测试文件模式作为「单文件测试面过大」的拆分手段采纳（与采纳点 1 同条落地），函数命名保持 slTerminal 约定不换成 pebrel 风格。
26. **照抄延续：BE-06 全量回归串行纪律 + 负载裕度登记口径**。pebrel 侧无对应物（其 CI 并行跑，stress 类用例靠 `#[ignore]` 排除）。理由：该纪律源于 slTerminal 本机实测（scan_bench 对 CPU 负载敏感致间歇红），fork 后测试主体仍是本机 cargo test，纪律原样保留；「提额须注释标明负载裕度、非断言放宽」的口径延续到 GPUI UI 测试的轮询预算与 win32 矩阵的 Sleep 预算上。
27. **参考：TQ-COV-06 / lib_tests 显式 test target 红线的改写**。slTerminal 现状：`src-tauri/Cargo.toml` 的 `[lib] test = false` + `[[test]] name = "lib_tests"`（path = src/lib.rs），根因 = tauri 栈静态导入 `TaskDialogIndirect` 需 comctl32 v6 manifest，而 `build.rs` 的 `rustc-link-arg-tests` 只作用于显式 test target。pebrel 侧位置：`nebula_app/Cargo.toml` 无此结构（GPUI 应用无 tauri 依赖，windows 平台 manifest 由 gpui 自带激活 v6）。理由：fork 后该根因随 tauri 一起消失，0xc0000139 不复现的预期成立，但「定向测试必须走显式 test target」的经验改写为预防性知识保留——若未来任何测试目标再遇 0xc0000139 零输出崩溃，第一检查项 = manifest 是否嵌入该测试目标；新 workspace 是否保留 lib_tests 形态按实际 target 布局在 crate 落位时定，不照搬。
28. **照抄延续：SPAWN_LOCK 串行化与 PTY 集成测试**。slTerminal 现状：`src-tauri/src/pty/spawn.rs` 的 SPAWN_LOCK（并发 spawn 卡死 ConPTY 输出管道）+ `src-tauri/tests/pty_integration_tests.rs`。理由：ConPTY 并发缺陷与框架无关，fork 后原样保留；改名 slterm 后路径随迁。
29. **照抄延续：test-exemptions.md 豁免登记制度 + 条件跳过声明**。slTerminal 现状：`.claude/test-exemptions.md` 的既定豁免清单（单表真值源 + 模块 CLAUDE.md 明细）+ 定位声明 + 条件跳过用例段（依赖开发者模式 symlink 权限的用例）。理由：硬约束 11 在新形态下继续生效，登记处形态不变；fork 时全表按「载体是否消亡」逐条重审（WebView 相关豁免行随载体销项，PTY / Win32 豁免行保留改写）。
30. **照抄延续：bugfix 防复发测试纪律**。pebrel 同族实践位置：`scripts/tests/user-feedback-regressions.md`（用户反馈回归登记）与全仓防复发锚点注释文化。理由：纪律本身不动；落地约定升级为——每个防复发用例在测试内标注「现象 / 根因 / 防什么回归」三要素锚点注释，替代原依赖文档登记的方式。

### L2/L3/L4 资产的重生类别（方向级）

31. **照抄：L3 终端协议语义整层重生为 Rust 网格测试**。类别方向：`test/terminal/` 的 ANSI/OSC 语义矩阵（`ansi-correctness`、`negative-ansi`、`osc-sequences`、`osc-production-handlers`）、键盘编码（`keyboard-encoding`）、快捷分发（`shortcut-dispatch`）、序列化（`terminal-serialize`）、主题选项（`theme-options`）→ 全部落 `slterm_terminal` 的 Rust 单测（对应 pebrel `nebula_terminal` 的 `keyboard_contract_tests.rs` 同族与 grid 测试）。理由：xterm.js 的 JS 语义消失后，终端协议真值由 Rust 终端核心自持，这类用例不是「搬」而是「在 Rust 语义下重建同等覆盖」。
32. **照抄：L2 中载体无关契约的 Rust 重生**。类别方向：布局 serde（`layout-serde*.test.ts`）→ `slterm_app` 布局模块的 serde round-trip 测试；注册表家族契约（`panel-registry`、`file-viewer-registry`、`sideViewRegistry`、`settings-page-registry` 等）→ Rust 注册表「模块级单例 + register/getAll + `_reset` 测试隔离」同构契约（硬约束 13 在 Rust 世界的直接形态）；键绑定（`keybindings`、`wire-keybindings`、`shortcuts`、`keystroke-format`）→ keymap 解析单测 + 采纳点 11 的 UI 分发测试；主题（`theme-colors`、`theme-overrides`、`theme-scheme-registry`）→ `slterm_settings` 测试；stores 状态转换（`terminal-registry*`、`sideBarState`、`workspace-*`）→ 状态单测；explorer/fs 语义 → 已大半在 L1（`fs_*_tests`），残余补齐；IPC 契约测试（`ipc-*.test.ts`）随 IPC 消亡不重生，其中 DTO 校验语义由 Rust serde/校验函数测试承接。
33. **照抄：L4 中行为类别的对位重生**。类别方向：终端生命周期与恢复编排、工作区分屏/页签（`workspace-split`、`tab-menu`）、设置端到端落盘（`settings.e2e.ts`）、后台任务启停（`background-tasks.e2e.ts`）、agent/history/hooks 行为链 → 分别对位：GPUI UI 测试（窗口内真实交互）/ Runtime API conformance 套件（若保留，见采纳点 17）/ win32 输入矩阵 / Rust 集成测试；真桌面值守类压缩为 test-exemptions.md 实机验收点。理由：L4 的价值是「跨进程编排的真实性」，在单进程世界由「UI 测试 + Runtime API + 集成测试」三层分摊承接，覆盖等价性逐类在豁免表登记。

## 不采纳点

1. **CI workflow 本体**（`.github/workflows/` 全部：architecture / windows-conformance / pr-size / release 等的事件驱动、标签写入、artifact 上传、merge_group 语义）。理由：slTerminal 无 CI（本地开发是定位约束）；只采纳其步骤内容（采纳点 18-20），载体整体不落地。
2. **GitHub Actions 专用自测**（`scripts/tests/test_ci_plan.py`、`test_ci_native_tests.py`、`test_ci_cache.py`、`test_copilot_review_workflow.py`、`test_pr_size_workflow.py`、`test_preview_release.py`、`test_stable_release.py`、`test_android_release.py`、`test_macos_dmg.py` 等）。理由：被测对象（workflow / CI 计划 / 发布管线）不存在。**协调改判（分片 12 定稿）**：打包主形态已定 Inno 安装器（Q17），`package-release.tests.ps1` 体积预算钉测试与安装器自测（`installer.tests.ps1` 同族）随迁；GitHub Release 编排测试（preview/stable release 族）仍不随 fork 走。
3. **跨平台一致性矩阵的非 Windows 部分**（`scripts/conformance` 的 `posix_process.py`、`macos_launch.py`、`macos_text.swift`、`cases` 中的 macOS / Linux 用例；`scripts/tests/test_xplat.py` 的 POSIX 隔离用例；`scripts/24-bit-color.sh`、`fg-bg.sh`、`colors.sh` 等 POSIX 探针）。理由：产品定位约束 Windows 10/11 原生，平台矩阵收敛为 Windows 单平台；conformance 套件仅保留 windows 驱动与 golden（采纳点 17 的对偶边界）。
4. **pr-size 的阻断与绕行机制**（1500 行 XL 强制失败 + `size-exempt` 标签人工绕行）。理由：本地无 PR 概念；仅保留行数统计口径作本地提示（采纳点 20），不建阻断。
5. **i18n 全量 catalog 合同规模**（`tools/i18n-contract` 全量语言对合同 + 每键分配断言；`docs/internationalization.md` 体系）。理由：slTerminal 无已登记的多语言需求；采纳点 4 的零分配契约形态保留为「若做 i18n 则照此」的参考，不在 fork 初建合同。
6. **check_platform_cfg.py + platform_cfg_budget.txt（跨平台 cfg 预算门禁）**。理由：硬约束 9 的平台分支收敛（业务 cfg 仅 pty 模块）已由代码评审 + clippy 承载，且单 Windows 目标下 cfg 面积极小，不另建预算门禁；其「预算棘轮」思想已由 file-budgets 覆盖。
7. **completion e2e 的值守桌面验收链**（`run_completion_e2e.ps1` 要求「desktop that is never switched to」的 Prediction 验收、PS1 内嵌的真机参数矩阵）。理由：值守机依赖与本地开发习惯冲突；脚本形态与隔离设计采纳（采纳点 14），运行模式压缩为人工验收点登记。
8. **GPUI 组件 lab 验收测试**（`nebula_gpui` crate 的组件验收场，随该 crate 整体不采纳，见分片 01）。理由：已定不采纳实验场，其测试不随迁。
9. **pebrel 品牌/竞品名关键词表本体**（`PROHIBITED_PATTERNS` 的 30 余个竞品名）。理由：那是 pebrel 自己的禁名清单；slTerminal 版关键词表重建为 nebula / pebrel 及组合形态（采纳点 23），竞品名清单不继承。
10. **`scripts/tests/` 中产品功能向 Python 测试**（`test_shell_integration.py` 的远端 hook 族、`test_remote_hooks.py`、`opencode-bridge` / `pi-notification-bridge` 的 mjs 测试等）。理由：这些是 pebrel 特有功能（远端会话桥、通知桥）的测试，功能本体不随迁（分片 03/09 裁定），测试随迁无对象。
11. **L2/L3/L4 的 WebView 载体专属用例**（postMessage origin / CSP / WebGL 初始化 / iframe 注入接管 / CodeMirror 行为 / glyph 像素判读 / wdio 启动器分支等）。理由：载体整体消亡，语义在新载体不存在对应物；相关豁免行在 test-exemptions.md 销项，不做类别重生（与采纳点 31-33 的边界：重生的是「行为类别」，不是「载体机制」）。
12. **pebrel 提交信息里的「门禁自维护关键词表」以外的文档豁免惯例**。理由：`EXEMPT_PATHS` 机制照抄（采纳点 23），但 slTerminal 版除门禁自身与夹具外不放行任何路径，pebrel 因 mobile 构建遗留的宽豁免（`mobile/android/...` 整目录前缀剔除）不继承——slTerminal 无此历史债。

## 优化方向

**目标测试金字塔（单进程 Rust 世界）**。自下而上：L1 Rust 单测为绝对主力——`slterm_*` workspace 各 crate 内嵌 `mod <领域>_tests` 模块（函数命名「对象_行为_场景」延续），超大测试面按 pebrel 三文件模式（tests.rs / native_tests.rs / *_contract_tests.rs）拆分；`tests/` 集成测试目录承载跨模块契约（file_line_budget 适配器、windows console 子系统对照、pty 集成、git 集成）。其上新增 GPUI 虚拟窗口 UI 测试层——`gpui-test-support` feature 统一门控，`#[gpui::test]` 驱动真实控件 / 布局 / 键盘 / hover 路径，承接原 L2 组件测试与 L4 窗口内交互的覆盖空缺。再上是脚本层本地门禁与工具箱——架构三件套 + 禁名回流 + 门禁自测 + win32 输入矩阵 + 内存压测 + conformance（若 Runtime API 保留）。豁免登记制度承载一切残余不可自动化面。

**门禁本地化形态**。所有门禁是纯本地脚本（Python 3.11+ / PS1 / cargo），无 GitHub 状态依赖；单一入口（本地脚本，如 `scripts/check_all.ps1` 或既有约定等价物）按 architecture.yml 的步骤序串行执行：门禁自测 → 禁名回流（staged + push 模式）→ file-budgets / dependencies 检查（默认 `--base HEAD~1` 或入口参数）→ `cargo test` 全量串行（BE-06 延续，SPAWN_LOCK 原样）→ gpui UI 测试 → 静态检查四件套（tsc 退出历史舞台后由 clippy / rustfmt 接替）→ 可选项（win32 输入矩阵、内存压测、体积提示）。全量串行与负载裕度注释口径延续到新层。

**覆盖门禁（硬约束 11）新形态落地**。「可自动化」判定标准改为「Rust 可测或 GPUI 虚拟窗口可测」；豁免登记沿用 test-exemptions.md 单表真值源 + 模块 CLAUDE.md 明细的两级形态，fork 时全表逐条按载体存亡重审销项/改写；真桌面验收点（native QA、chord 矩阵、IME、剪贴板确认）集中登记为条件跳过用例 + 实机验收清单。

**防复发与契约锚点文化延续**。bugfix 防复发纪律不变，升级为测试内三要素锚点注释（现象 / 根因 / 防什么回归）；file-budgets / dependencies.toml / 禁名清单三文件为架构契约单一真值源，Python 门禁与 Rust 适配器双侧消费；测试中的契约数值（超时 / 预算 / 矩阵字节序列）以登记文件与基线文件为锚，禁散落魔法数。

**资产处置顺序**。fork 启动时 L2/L3/L4 测试文件随前端目录整体删除；重生类别（采纳点 31-33）在 `slterm_terminal` / `slterm_settings` / `slterm_app` 落位过程中逐类重建，删除前对高价值用例做类别级清单登记（只记类别与覆盖意图，不逐文件搬运）；win32 输入矩阵基线在首个可跑构建产出后以 `-Record` 建立 slterm 基线；test-exemptions.md 重审与门禁三件套建文件同批完成，保证「豁免未登记禁合入」自始至终有机械与制度双兜底。
