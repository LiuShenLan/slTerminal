# 11 测试体系详细设计

> pebrel-design 分片 11/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/11-testing.md`(spec 分片,含与 12 篇的协调改判);骨架:`docs/pebrel-design/00-roadmap.md`。
>
> 本篇为「测试基础设施类型」锚点归属篇(01 篇类型锚点表末行):测试支持库(虚拟窗口构建、输入注入、快照断言、矩阵夹具)的签名级定义归本篇,02-10 篇测试点清单只消费形态、禁重复设计。架构门禁三件套脚本本体归 01 篇,本篇只写其自测四件与测试消费形态。

## 目标形态

旧四级金字塔(L1 cargo / L2 vitest / L3 xterm-headless / L4 wdio)随 WebView 全灭。新形态四层,自下而上:

| # | 形态 | 载体 | 运行命令 | 覆盖门禁职责 |
|---|---|---|---|---|
| 1 | Rust 单测(绝对主力) | 各 crate 内嵌 `mod <领域>_tests`;超大测试面按三文件模式拆分;`tests/` 目录承载跨模块契约 | 全量:`cargo test --workspace -- --test-threads=1`(BE-06 延续);定向:`cargo test -p <crate> <filter> -- --test-threads=1` | 协议语义、纯函数契约、serde round-trip、注册表契约、SPAWN_LOCK 类平台串行化 |
| 2 | GPUI 虚拟窗口 UI 测试 | `#[gpui::test]` + `TestAppContext` / `VisualTestContext`,feature `gpui-test-support` 统一门控,模块级 `ui_tests.rs` | 与单测同二进制(feature on 形态);feature off 形态整体不编译 | 真实控件 / 布局 / 键盘 / hover 路径(L2 组件测试与 L4 窗口内交互的覆盖空缺承接) |
| 3 | 脚本层本地门禁与工具箱 | python 3.11+ unittest / PS1 / cargo | `python3 scripts/check_architecture.py`、`python3 scripts/check_brand_regress.py staged\|message\|push`、`powershell -File scripts/win32_input_matrix.ps1`、`python scripts/conformance/run.py --app <exe> --platform windows-x86_64` | 行数棘轮、依赖方向、禁名回流(三件套归 01)、Win32 输入字节基线、Runtime API 行为级端到端、内存压测 |
| 4 | 豁免登记制度 | `.claude/test-exemptions.md` 单表真值源 + 各 crate/模块 CLAUDE.md 明细 | 提交前人工核对;「豁免未登记禁合入」由硬约束 11 承接 | 一切残余不可自动化面(实机验收点、条件跳过用例) |

体积预算钉测试与安装器自测(`package-release.tests.ps1` / `installer.tests.ps1` 同族)为脚本层第 3 层的打包对位件——打包链归 12 篇,测试钉归本篇(见改造节 6)。GitHub Release 编排测试不随迁(spec 分片 11 不采纳点 2,协调改判已定)。

### 命名约定新形态

- 内嵌测试模块:`mod <领域>_tests` 领域具名,函数「对象_行为_场景」snake_case 裸名——slTerminal 现状约定原样延续,不换成 pebrel 风格。
- 文件级拆分:单文件测试面过大时按 pebrel 三文件模式拆出(`tests.rs` / `native_tests.rs` / `*_contract_tests.rs`),拆分是内嵌模块的自然延伸,非新层级。
- 集成测试:`<域>_tests.rs`(snake_case)落各 crate `tests/` 目录。
- 脚本层:`test_*.py`(python unittest)、`*.tests.ps1` 自断言 PS1,照抄 pebrel 命名。
- L2/L3 时代的 kebab-case 前端测试文件名随载体消亡。

## 边界与不变更项

1. 产品定位七条不可违背(00-roadmap 跨领域不变量):仅 Win10/11 原生、单窗口单实例、仅暗色、GPU 加速、复制 = Ctrl+Shift+C、默认 shell pwsh→powershell→cmd、27 家 AI CLI 一等。复制语义与 kitty 协议兼容是本测试体系必须锚定的两条产品级契约。
2. 旧载体词仅准出现在「消亡 / 映射 / 来源」语境:WebView、IPC、Tauri 命令、Dockview、xterm.js、vitest、wdio、四级金字塔(L1-L4 编号旧义)。
3. 本篇只设计测试**体系与基础设施**;各领域的用例点归 02-10 篇测试点清单,本篇禁重复列举领域用例(仅重生映射表登记类别方向)。
4. 延续纪律清单(具体新表述归改造节 2):BE-06 全量回归串行、负载裕度登记口径、bugfix 防复发、SPAWN_LOCK 串行化、命名约定、豁免登记制度、硬约束 11 覆盖门禁。
5. 绿灯语义:本篇各阶段出口只认可机验项;「测试基建可用」以对应命令实跑通过为准,不设模糊出口。
6. 引用纪律:引 pebrel 用符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e`),禁行号;引本仓源码用符号名;引测试用例名。

## 关键类型与签名

> 本节是 01 篇类型锚点表中「测试基础设施类型」的归属落地。跨篇纪律:他篇引用下列形态只许 `use` / 按签名仿制,禁重定义、禁字段语义改写;新增测试基础设施类型先回本节登记。

### feature 开关(slterm_app/Cargo.toml)

```toml
gpui-test-support = ["gpui-shell", "gpui/test-support", "gpui-component/test-support"]
```

一条 feature 同时打开 gpui 与 gpui-component 的测试支持(照抄 pebrel `nebula_app/Cargo.toml` 同名条目);`#[gpui::test]` 用例与 `#[test]` 用例同 crate 混编,feature 未开时 UI 用例整体不编译。Kuddev fork 的虚拟窗口补丁(TestWindow 对不存在的原生窗口/显示句柄返回 `Unavailable` 而非 panic、组件 `test-support` feature 不安装需原生句柄的转发器)与「原因 + 范围 + 上游撤销条件」注释块归 01 篇钉版,本篇消费其测试形态。

### 虚拟窗口 UI 测试的模块级 builder 形态

slterm 侧不另封装共享 builder——照 pebrel 形态,每个被测模块自建私有 builder,模板签名(以 pebrel `nebula_app/src/gpui_shell/workspace/settings_navigation/ui_tests.rs` 的 `open_workspace` / `click_tab` / `tab_bounds` 为范):

```rust
#[gpui::test]
fn <对象>_<行为>_<场景>(cx: &mut gpui::TestAppContext) { ... }

fn open_workspace(count: usize, cx: &mut TestAppContext)
    -> (tempfile::TempDir, Entity<SltermWorkspace>, VisualTestContext);
fn click_tab(selector: &'static str, cx: &mut VisualTestContext);
fn tab_bounds(selector: &'static str, cx: &mut VisualTestContext) -> gpui::Bounds<gpui::Pixels>;
// 要点:window.refresh() + draw 后 debug_bounds 取真实布局坐标 → simulate_click / simulate_keystrokes;
// 配置一律隔离(SLTERM_CONFIG_DIR 或 tempfile),不许触开发者真实目录。
```

「真实控件与布局路径」纪律(照抄 pebrel `docs/project-constraints.md` §5):点击真实布局与控件、覆盖点击留白、hover 显示、键盘路径、重复触发;布局核对真实命中范围。slTerminal 定位的直接咬合点:复制 = Ctrl+Shift+C 的可感知反馈、Ctrl+C 中断不被劫持——键绑定用例必须走真实分发(下条)。

### 键绑定真实分发测试锚(slterm_app 终端 keymap)

照 pebrel `nebula_app/src/gpui_shell/terminal/keymap.rs` 的 `mod tests` 双形态:

- 纯解析矩阵(无窗口):`encode(&Keystroke, &TermMode) -> Option<Vec<u8>>` 的 TermMode × 键组合矩阵(含 `native_window_shortcuts_follow_the_host_window_policy` 的 Windows 宿主快捷键吞键族)。
- 真实分发(#[gpui::test] + `#[cfg(feature = "gpui-test-support")]` 双门控):构造 `Keystroke` 注入 Root + 终端 key_context 的 on_key_down 路径,断言动作命中(`native_window_shortcuts_propagate_through_root_and_terminal` 同族)。**产品键盘语义(复制/中断/CLI 保留键)一律以此端到端形态锚定,只测解析函数不算覆盖。**

### kitty 键盘契约夹具(slterm_terminal)

照 pebrel `nebula_terminal/src/term/keyboard_contract_tests.rs`:

```rust
struct Replies(std::rc::Rc<std::cell::RefCell<Vec<String>>>);
impl EventListener for Replies { /* 收集 Event::PtyWrite */ }

fn assert_steps(steps: &[(&[u8], u8)]);
// 字节进(parser.advance)/ 字节出(PtyWrite 应答)+ TermMode kitty 位断言,纯编译期无窗口。
// 矩阵族:push/pop 嵌套、alt screen 双屏各存其帧、并差集、溢出保 title 栈、RIS 双清。
```

此为 L3 键盘编码/ANSI 正确性用例在 Rust 世界的直接重生承载(重生映射表行 1)。

### file_line_budget Rust 适配器(slterm_app/tests/file_line_budget.rs)

照抄 pebrel `nebula_app/tests/file_line_budget.rs` 两函数两用例,消费 01 篇 `architecture/file-budgets.txt` 同一数据文件(单一真值源双侧消费):

```rust
fn line_count(content: &[u8]) -> usize;                       // 物理行:数 \n + 末行无换行补一
fn collect_sources(root: &Path, files: &mut Vec<PathBuf>);    // 递归收集,禁符号链接,Cargo 包根跳 target/

#[test] fn every_source_file_respects_its_line_budget();       // 超限/额度失效/缺失源三报错族
#[test] fn physical_lines_ignore_encoding_and_newline_convention();
```

### windows_console_startup console 子系统对照目标

照抄 pebrel `nebula_app/tests/windows_console_startup.rs` 整文件技巧,落 `slterm_app/tests/windows_console_startup.rs`:

```rust
#![cfg(windows)]
#![windows_subsystem = "console"]                       // 测试目标显式 console 子系统
#[path = "../src/platform/startup/console.rs"]           // 挂载被测源码
mod console;
#[path = "../src/platform/startup/console_tests.rs"]     // 挂载其测试模块
mod console_tests;
```

因果链:console 子系统目标自动继承父进程 Ctrl+C 状态,GUI 目标 `AttachConsole` 会重置并掩盖该 bug;GPUI 应用本体是 GUI 子系统,此对照目标是唯一可观测继承语义的载体。被测对象(console 启动准备)归 09 篇,本篇锚定测试技巧。

### i18n 零分配契约(slterm_app/tests/i18n_contract.rs)

照抄 pebrel `nebula_app/tests/i18n_contract.rs`(`tools/i18n-contract` 全量 catalog 合同规模不迁,spec 不采纳点 5;零分配形态保留):

```rust
struct CountingAllocator;            // #[global_allocator],alloc/realloc 计数,thread_local 门控
#[test] fn first_and_repeated_translation_lookups_allocate_nothing();  // 全语言 × 千次查找 = 0 分配
#[test] fn translation_lookup_fits_a_small_stack();                    // 64 KiB 子进程隔离跑,防栈溢出拖垮全套件
#[test] fn embedded_translations_stay_within_the_initial_payload_budget();
```

### conformance 驱动接口(scripts/conformance)

照抄 pebrel `scripts/conformance/harness.py` + `run.py`,Windows 单平台裁减(posix/macos 驱动不迁,spec 不采纳点 3):

```python
# harness.py
class ConformanceError(RuntimeError): ...        # 前置条件/断言失败
class ApiFailure(ConformanceError): ...          # Runtime API 错误响应(code/message)
class SkipCase(RuntimeError): ...                # 可选用例环境不可 exercising(显式跳过入报告)
def require(condition: bool, message: str) -> None: ...
PROTOCOL_NAME = "nebula.runtime"  # → slterm.runtime eg 04 篇信封,本篇只按 04 单点表消费
DEFAULT_STARTUP_TIMEOUT = 20.0    # 契约数值,登记于此为锚

# run.py CLI
python scripts/conformance/run.py --app <exe> --platform windows-x86_64 \
    --output dist/conformance/windows-report.json [--update-golden] [--no-platform-golden]
python scripts/conformance/run.py --compare <report...>
```

用例序照抄 README 步骤语义:boot → echo → resize → split → scrollback → session → paste → cjk_roundtrip → close(close 末位,故意终止进程)。`ssh_loop` 用例不迁(SSH 砍)。golden 治理:每次运行校验 `golden/common.json`;平台 golden(`golden/windows-x86_64.json`)缺席 = 报告明示不算失败;`--update-golden` 在用例失败时拒写。

### win32 输入基线矩阵(scripts/win32_input_matrix.ps1)

照抄 pebrel 同名脚本,参数面改造(其默认值硬编码 `D:\temp_build\nebula` 属本机残留):

```powershell
param([switch]$Record, [string]$Exe = '<repo>\target\debug\slterm.exe')
# 断言模式:PostMessage 驱动最小化实例(不抢焦点)→ 探针进程(node reader)收字节
#   → 与 scripts\win32_input_matrix.baseline.txt 逐字节比对
# -Record:重登记基线;矩阵 = 普通键 + 扩展键 + 裸修饰键(必须零字节)+ 哨兵键
# LIMITS 注释照抄:chord 需前台 SendInput + 值守空闲机,不无人值守 → 实机验收点归豁免表
```

契约数值锚:Send-Key 内 Sleep(60ms/240ms)与 probe ready 轮询(30×1s)属负载裕度预算,注释须标「负载裕度,非断言放宽」(口径见改造节 2)。

### native_tests.rs 真桌面 QA 门禁形态

照 pebrel `nebula_app/src/gpui_shell/settings_pane/cursor_motion/native_tests.rs` 同族(spec 采纳点 3,参考级):

```rust
#[test]
#[ignore = "requires a native desktop with a real display; run explicitly for QA"]
fn native_<对象>_<行为>(...) {
    let qa_dir = std::env::var_os("SLTERM_CURSOR_QA_DIR").expect("set SLTERM_CURSOR_QA_DIR ...");
    // 绝对输出目录 + 隔离 SLTERM_CONFIG_DIR;不设置环境变量时 #[ignore] 兜住
}
```

GPUI 虚拟窗口覆盖绝大部分 UI 回归;原生外观、IME 合成等少数路径仍需真桌面——统一「ignore + 环境门禁 + 隔离配置目录」后,进 test-exemptions.md 实机验收点清单而非散落。环境变量名归 01 篇 C 节 QA 族新立 `SLTERM_*` 口径。

### 门禁自测四件(scripts/tests/)

门禁脚本自身的正确性由测试锁死——「门禁不许静默放宽」的机械保障,归本篇测试组织形态(脚本本体归 01 篇):

| 文件(pebrel 源 → slterm 落位) | 锁什么 |
|---|---|
| `test_architecture_budgets.py` → 同名 | `Budget.parse` 非法输入族、棘轮语义(豁免只减不增/缩容销豁免/删文件销额度) |
| `test_architecture_dependencies.py` → 同名 | 层判定、生产边无环、白名单边、workspace 成员全等 |
| `test_architecture_governance.py` → 同名 | 因果 note 九段结构(`NOTE_REQUIRED_SECTIONS`)、文档链接不悬空——与文档规范天然互补 |
| `test_prohibited_names.py` → `test_brand_regress.py` | 四模式正/反例 + `fixtures/prohibited_names/` 假阳边界夹具(词表改 nebula/pebrel) |

运行形态:`python3 -m unittest`(scripts/tests 为 discover 根);ps1/mjs 族自断言,退出码即判定。

## 数据流与状态机

测试体系无持久状态机;五股数据流,全部为「启动 → 观测 → 比对 → 退出码」一次性形态:

1. **cargo 流**:根 workspace 单一入口;feature off 形态 UI 用例整体不编译,feature on 形态全量编跑。两形态都过 = 双形态编译门禁(见改造节 7)。SPAWN_LOCK 静态锁在 PTY 集成用例间串行(BE-06 全局串行已蕴含)。
2. **win32 矩阵流**:`Start-Process -WindowStyle Minimized`(不抢焦点)→ PostMessage 逐键 → 探针(node reader)累积字节 → hex 拼接 → 与 baseline diff → `-Record` 时重写 baseline。基线文件被 git 追踪,修改必须随代码变更同提交(单独提交基线 = 禁,防静默放宽)。
3. **conformance 流**:临时 `SLTERM_CONFIG_DIR` 起孤立应用进程 → 读 `runtime.port`(loopback 端口 + token,eg 04 篇)→ JSONL 逐用例驱动 → 产 report.json → 对 `golden/common.json` + 平台 golden 比对 → 只杀自己拉起的进程;失败/超时同。golden 缺席 ≠ 失败(新基线治理经验照抄)。
4. **门禁流**:归 01 篇(读声明文件 + git/文件系统 → 纯函数判定 → exit code,无状态)。
5. **豁免登记流**:发现不可自动化面 → 人工裁决 → 登记 test-exemptions.md(单表)→ 对应模块 CLAUDE.md 补明细 → 提交前核对。登记即承诺,无登记 = 硬约束 11 违规。

**基线治理状态机**(win32 baseline / conformance golden / 内存压测检查点三族同律):`初始缺失(明示不算失败)→ -Record/--update-golden 建立(须全用例绿)→ 常挂比对 → 语义变更时「代码 + 基线」同提交 → 单提基线被禁名/评审拦`。任何基线文件的数值变更必须在提交信息说明触发原因(协议变更 / 环境漂移 / 阈值治理)。

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(pebrel 源 → slterm 落位,对应 spec 分片 11 采纳点 1-33)

| # | 来源(pebrel 路径 / 符号) | 落位 | 因果链一句 |
|---|---|---|---|
| 1 | `nebula_app/src/gpui_shell/file_editor/tests.rs`、`nebula_app/src/ai_hook/native_tests.rs`、`nebula_terminal/src/term/keyboard_contract_tests.rs` 三文件模式 | 同形态散落各 crate | 超大测试面从源文件拆出;`native_tests.rs` 专放真桌面用例,`*_contract_tests.rs` 专放协议矩阵——与 `mod <领域>_tests` 同源,文件级拆分是自然延伸 |
| 2 | `keyboard_contract_tests.rs` 的 `assert_steps` + `Replies` | `slterm_terminal` 同签名(见关键类型节) | 字节进/字节出矩阵 + kitty 应答断言,L3 键盘协议用例的 Rust 直接重生;27 家 CLI 的 kitty 兼容语义以此锚定 |
| 3 | `settings_pane/cursor_motion/native_tests.rs` 的 ignore + 环境门禁 + 隔离配置形态 | 同形态,环境变量改 `SLTERM_*_QA_DIR` | 原生外观/IME 少数路径仍需真桌面;统一形态后进豁免表实机验收点清单 |
| 4 | `tools/i18n-contract` 零分配契约 + `nebula_app/tests/i18n_contract.rs` 的 `CountingAllocator` | `slterm_app/tests/i18n_contract.rs`(全量 catalog 合同不迁) | 首次与重复翻译查找零分配的回归锚;单进程 UI 翻译查找频率不低于 pebrel |
| 5 | `nebula_app/tests/file_line_budget.rs` | `slterm_app/tests/file_line_budget.rs` | Python 门禁与 Rust 测试消费同一 budget 文件,单一真值源双侧消费(01 篇缝合点 4 回指) |
| 6 | `nebula_app/tests/windows_console_startup.rs` | 同名 | console 子系统目标才能观测 Ctrl+C 继承语义;GPUI 应用是 GUI 子系统,对照必要 |
| 7 | `nebula_app/Cargo.toml` `gpui-test-support` feature | 同名同义 | 一条 feature 同时打开 gpui + gpui-component 测试支持,UI 与单测同二进制共存 |
| 8 | 根 `Cargo.toml` 虚拟窗口补丁注释块(「下游虚拟窗口测试支持」「窗口生命周期修补」) | 归 01 篇钉版,本篇消费形态 | 无原生句柄跑完整控件回归是本地无 CI 可重复 UI 测试的根基;注释治理结构随补丁照抄 |
| 9 | `settings_navigation/ui_tests.rs` 的 `open_workspace`/`click_tab`/`tab_bounds` 及 `switching_top_tabs_keeps_settings_reachable_until_explicit_close`;同族 `code_actions_tests.rs`、`inline_selection_tests.rs`、`cursor_motion/tests.rs`、`segmented/tests.rs`、`platform/acrylic/gpui_tests.rs` | 各模块 `ui_tests.rs` 同模板 | 「真实控件与布局路径」执行模板:refresh + draw + debug_bounds 取真实坐标 + simulate_click/keystrokes |
| 10 | `docs/project-constraints.md` §5(§5.2 复制合同 + 交互测试条款) | 机制落 UI 测试纪律 + 约定入 CLAUDE.md 族 | L2 毁灭后 UI 行为覆盖的唯一权威纪律;与复制 = Ctrl+Shift+C、仅暗色定位直接咬合 |
| 11 | `gpui_shell/terminal/keymap.rs` `mod tests`(解析矩阵 + `native_window_shortcuts_propagate_through_root_and_terminal` 真实分发) | `slterm_app` 终端 keymap 同双形态 | 产品核心键盘语义必须以「注入按键 → 观测动作」端到端锚定 |
| 12 | `#[gpui::test]` 与 `#[test]` 混编、feature 统一门控纪律 | 同形态 | 门禁须一条命令覆盖 feature on/off 两形态(改造节 7) |
| 13 | `scripts/tests/` 门禁自测四件 + 其余 python unittest/PS1/mjs 形态 | 四件随迁(词表/根目录改造);功能向产品测试(test_shell_integration 远端族等)不迁 | 无 CI 下门禁自身必须有测试,否则静默失效无人知——体系命门 |
| 14 | `scripts/tests/run_completion_e2e.ps1` 隔离设计(空输出目录 + QA 环境变量隔离) | 同形态 | 脚本与隔离设计照抄;「永不切换桌面的值守机」运行模式不迁,压缩为人工验收点 |
| 15 | `scripts/tests/test_windows_memory_stress.py` 的 sample/complete_samples/checkpoints 相位采样 + `scripts/windows-memory-stress.md` | 同名同形态 | 终端多开关闭后的私有提交内存回落是长期质量指标;阈值首版实测校准归待沉淀 D3 |
| 16 | `scripts/win32_input_matrix.ps1` + `win32_input_matrix.baseline.txt` | 同名;exe 默认路径参数化(去 `D:\temp_build\nebula` 硬编码) | PostMessage 不抢焦点、可边打字边跑;字节级回归是终端命脉;Windows-only 与定位重合 |
| 17 | `scripts/conformance/`(README + harness.py + run.py + golden 治理) | Windows 单平台裁减版 | Runtime API 驱动的行为级端到端;golden「缺席不算失败」基线治理经验照抄 |
| 18 | `.github/workflows/architecture.yml` 步骤序(自测 → 禁名 → budgets/dependencies → 契约测试) | 本地单一入口归 01 篇;本篇定测试消费序 | 步骤序表达的逻辑依赖关系与载体无关 |
| 19 | `scripts/check_architecture.py` 纯本地化 + `check_prohibited_names.py` 四模式 | 归 01 篇 | 门禁脱离 CI 本地可跑是采纳前提 |
| 20 | `.github/workflows/pr-size.yml` 行数统计口径 | 本地提示脚本(超阈警告不阻断) | 无 PR 概念;阻断与绕行不采纳;是否纳入日常入口归开放问题 2 |
| 21 | `scripts/check_architecture.py` 双检查合一入口 + `--base` 棘轮 | 归 01 篇;本篇自测四件消费 | 行数棘轮 = 「未来最优」的机械执行器:存量登记、只降不增、禁通配豁免 |
| 22 | `architecture/dependencies.toml` 声明式依赖方向 + `scripts/architecture/dependencies.py` `check` | 归 01 篇;本篇自测消费 | 硬约束 2「后端按功能分模块」从约定升级为机械检查;分层内容归 01 目标 workspace |
| 23 | `scripts/check_prohibited_names.py` 禁名回流(词表/豁免模式/逐提交扫描/range fail-closed) | eg 01 篇改 `check_brand_regress.py`,词表 = nebula/pebrel | 旧名回流是真实腐化通道(pebrel 自身即被改名产品);增量防回流机制与词表无关可复用骨架 |
| 24 | 门禁自测四件(`test_architecture_budgets` / `test_architecture_dependencies` / `test_architecture_governance` / `test_prohibited_names` + fixtures) | 归本篇关键类型节,四件随迁 | 门禁自身正确性由测试锁死,是门禁进本地日常入口的前提;governance 件的 note 九段检查与文档规范互补 |
| 25 | slTerminal 命名约定(`mod <领域>_tests` + 对象_行为_场景) | 原样延续 | pebrel 文件级拆分采纳为拆分手段,函数命名不换风格 |
| 26 | BE-06 全量回归串行 + 负载裕度口径 | 原样延续,扩面到 UI 轮询预算与矩阵 Sleep 预算 | 根因(scan_bench 对 CPU 负载敏感)与框架无关;pebrel 无对应物 |
| 27 | TQ-COV-06 / lib_tests 红线 | 改写为预防性知识 + 防线检查项(改造节 1) | tauri 栈消失后根因不复活,但 manifest 类零输出崩溃的排查经验保留 |
| 28 | `src-tauri/src/pty/spawn.rs` `SPAWN_LOCK` + `pty_integration_tests` | eg 02 篇并入 slterm_terminal,组织形态归本篇 | ConPTY 并发缺陷与框架无关,原样保留 |
| 29 | `.claude/test-exemptions.md` 豁免登记制度 | 制度延续,全表 fork 时按载体存亡重审(改造节 3) | 硬约束 11 在新形态继续生效,登记处形态不变 |
| 30 | bugfix 防复发纪律 + pebrel 防复发锚点注释文化 + `user-feedback-regressions.md` 登记思想 | 升级为测试内三要素锚点注释(现象/根因/防什么回归),文档登记表退役 | 锚点随用例走,不腐化 |
| 31 | L3 `test/terminal/` ANSI/OSC/键盘编码/序列化矩阵 → `slterm_terminal` Rust 网格测试 | 归 02 篇用例点,形态归本篇(键盘契约夹具等) | xterm.js 语义消失后,协议真值由 Rust 终端核心自持——是重建同等覆盖,不是搬运 |
| 32 | L2 载体无关契约(layout-serde / 注册表家族 / 键绑定 / 主题 / stores / fs 语义) | Rust serde round-trip / 注册表同构契约(硬约束 13 直接形态)/ keymap 双形态 / settings 测试 | 见重生映射表;IPC 契约测试随 IPC 消亡不重生,DTO 校验语义由 serde 测试承接 |
| 33 | L4 行为类(终端生命周期/工作区/设置落盘/后台任务/agent 链) | UI 测试 + conformance + Rust 集成测试三层分摊;真桌面值守类压为实机验收点 | L4 的「跨进程编排真实性」在单进程世界由三层分摊,等价性逐类登记豁免表 |

### 改名映射引用

全文改名以 01 篇「改造 / 移植 / 新建设计」节**改名映射单点表**为唯一权威,本篇不另立。本篇涉及的改名消费点:crate `nebula_*` → `slterm_*`;`PEBREL_CONFIG_DIR` → `SLTERM_CONFIG_DIR`;QA 环境变量族 `PEBREL_*_QA_*` → 按需新立 `SLTERM_*_QA_DIR` / `SLTERM_TEST_*`(01 篇 C 节);`win32_input_matrix.baseline.txt` 产物名不变(无品牌);conformance 协议名 `nebula.runtime` → `slterm.runtime`(eg 04 篇信封,本篇消费);门禁自测四件之一改名 `test_brand_regress.py`(归 01 篇脚本,本篇自测用例同步改)。

### 缝合点

1. **01 篇门禁 × 本篇自测**:三件套脚本 eg 01,本篇只定自测四件与「门禁先自测自身」的测试组织;file-budgets 的 Rust 适配器归本篇(01 缝合点 4 回指,双写字段禁漂移)。
2. **04 篇 Runtime API × conformance**:信封/方法族归 04;本篇裁定驱动形态(桩 vs 真 CLI,改造节 5,承接 04 开放问题 2)。
3. **12 篇打包链 × 本篇测试钉**:Inno 链/staging/体积工程归 12;`package-release.tests.ps1`、`installer.tests.ps1` 随迁的测试钉设计归本篇(改造节 6)。
4. **02 篇 PTY 领域 × 本篇组织形态**:SPAWN_LOCK、ConPTY flags 矩阵、pty 五件套的领域用例归 02;本篇只定集成测试目录组织与串行纪律。
5. **06 篇 i18n × 零分配契约**:build.rs 静态生成归 06;`i18n_contract.rs` 测试归本篇基础设施。
6. **各篇测试点清单**:02-10 篇只列领域用例点,形态一律引用本篇;本篇禁重复列举领域用例(重生映射表只到类别级)。

## 改造 / 移植 / 新建设计

### 1. TQ-COV-06 在多 crate workspace 的等价风险与防线

旧红线:`src-tauri/Cargo.toml` `[lib] test = false` + `[[test]] lib_tests`(path = src/lib.rs);根因 = tauri 栈静态导入 `TaskDialogIndirect` 需 comctl32 v6 manifest,而 `build.rs` 的 `rustc-link-arg-tests` 只作用于显式 test target;`cargo test` 带 filter 或 `--lib` 绕过显式 target → 0xc0000139 零输出崩溃。

fork 后判定(spec 采纳点 27):

- **根因不复活**:GPUI 应用无 tauri 依赖,Windows 平台 manifest(comctl32 v6)由 gpui 自带激活,`nebula_app` 无此结构,预期成立。
- **等价风险面转移**:多 crate workspace 下,定向测试入口从「单 crate 的 filter/--lib 陷阱」变为「`-p` 选择子 + 目标类型组合」。防线两条:
  1. **预防性知识保留**(写入根 CLAUDE.md 测试节):任何测试目标再遇 0xc0000139 零输出崩溃,第一检查项 = 该测试目标是否嵌入了 v6 manifest(`rustc-link-arg-tests` 是否覆盖该 target);症状与根因的映射不变,排查顺序不变。
  2. **显式 test target 形态归留归待沉淀 D1**:是否每 crate 复刻 `lib_tests` 显式 target,按 M1 落位时实测(`cargo test -p slterm_settings --lib` 是否正常)定,不照搬。
- **文档红线**:旧豁免表中 TQ-COV-06 翻案行随 tauri 载体销项,但销项记录保留一行「已销项(根因载体消亡)」形态——与既有豁免表翻案先例同格式,防后人再排查同类症状时无据可查。

### 2. 旧纪律保留清单与新表述

| 纪律 | 旧表述(Tauri 时代) | 新表述(单进程 GPUI) |
|---|---|---|
| BE-06 全量回归串行 | L1/L2 禁并行,基准对 CPU 负载敏感 | `cargo test --workspace -- --test-threads=1` 为唯一全量入口;python 门禁与 cargo 全量禁并行;根因与框架无关 |
| 负载裕度登记口径 | 超时/轮询预算提额须注释标明「负载裕度,非断言放宽」 | 口径扩面:GPUI UI 测试的 `run_until_parked`/轮询预算、win32 矩阵 Send-Key 内 Sleep、conformance `DEFAULT_STARTUP_TIMEOUT` 三类同样适用;判别标准不变(断言语义不变、真 bug 仍红) |
| bugfix 防复发 | 修复须附防复发测试 + 对照老代码补回归用例 | 追加:每个防复发用例带测试内三要素锚点注释(现象 / 根因 / 防什么回归),替代文档登记表——锚点随用例,不腐化 |
| SPAWN_LOCK | 串行化 ConPTY spawn 防输出管道卡死 | 原样并入 `slterm_terminal`,静态锁 + PTY 集成用例串行依赖不变;改名后符号随迁归 02 篇 |
| 命名约定 | `mod <领域>_tests` + 对象_行为_场景 | 原样延续;pebrel 三文件模式仅作拆分手段(目标形态节) |
| 覆盖门禁(硬约束 11) | 可自动化必测,不可自动化登记豁免 | 「可自动化」判定标准改为「Rust 可测或 GPUI 虚拟窗口可测」;豁免登记形态归改造节 3 |
| 契约数值锚定 | 超时/预算/矩阵字节以登记文件为锚 | 扩面:golden/baseline 文件 + 本节签名 + architecture 声明文件四类为锚;测试代码内禁散落魔法数 |
| 全量回归与门禁顺序 | npm test + cargo test 串行 | 本地入口测试消费序归改造节 7 |

### 3. 豁免登记制度延续形态

制度骨架不动(单表真值源 + 模块明细两级),三处新形态:

1. **fork 时全表重审**(M0 末一次性动作,与门禁三件套建文件同批):既有豁免表逐条按「载体是否消亡」二分——WebView/IPC/jsdom/wdio 相关行销项(销项保留一行根因记录,同改造节 1);PTY/Win32/ConPTY 行保留改写(slterm_terminal 路径与新例名);翻案行(已修复)按先例格式保留。
2. **实机验收点清单集中登记**:native QA 用例(改造节上文的 ignore + 环境门禁形态)、win32 chord 矩阵(前台 SendInput 值守机,LIMITS 注释明示)、IME 合成、剪贴板确认对话框、completion e2e 值守机验收——统一进 test-exemptions.md 实机验收点段,每项注明环境前置与执行方式;不散落各 crate。
3. **条件跳过用例段延续**:依赖开发者模式 symlink 权限等环境条件用例,沿用「环境不满足跳过但仍计通过」登记形态;新形态下同类项 = 需真显示器的 native QA(由 `#[ignore]` 双保险)。
4. **重生映射的落盘**:旧 L2/L3/L4 高价值用例的「类别级清单登记」归 `.claude/test-exemptions.md` 新增「资产重生映射」附表(永久归属,禁放临时稿——本设计目录 M11 删除);删除前端测试文件前按该表逐类核对去向。

### 4. 旧 L1-L4 资产按类别重生映射表

删除时机:M0 一次性删 `test/`、`e2e-tests/` 时执行;登记先于删除,覆盖意图只记类别级,不逐文件搬运。

| 旧资产类别 | 代表文件(test/ 与 e2e-tests/) | 重生去向 | 承载形态 |
|---|---|---|---|
| 终端协议语义(ANSI/OSC 矩阵、键盘编码、序列化、主题选项) | `terminal/ansi-correctness`、`keyboard-encoding`、`terminal-serialize` 等 | `slterm_terminal` Rust 单测 | `keyboard_contract_tests.rs` 族 + grid/OSC 测试(02 篇用例点) |
| 快捷分发与键绑定语义 | `terminal/shortcut-dispatch`、`keybindings`、`keystroke-format` | keymap 双形态测试 | 解析矩阵 + `#[gpui::test]` 真实分发(关键类型节) |
| 载体无关契约(layout serde / 注册表家族 / 主题 stores 转换) | `layout-serde*`、`panel-registry`、`file-viewer-registry`、`theme-*`、`workspace-*` | 各对应 crate 的 serde round-trip 与注册表同构契约 | 硬约束 13 在 Rust 世界的直接形态:模块级单例 + register/getAll + `_reset()` 测试隔离 |
| explorer/fs 语义 | `fs_*_tests` 已大半 L1 | `slterm_app` fs 域 + 07 篇 | 残余补齐归 07 篇测试点 |
| IPC 契约测试 | `ipc-*.test.ts` | 不重生(IPC 消亡) | DTO 校验语义由 Rust serde/校验函数测试承接归各篇 |
| WebView 载体专属(postMessage origin/CSP/WebGL/CM 行为/像素判读) | `html-*`、`markdown-*`、`glyph-*` | 不重生(载体消亡) | 豁免表销项;相关语义归新载体 UI 测试与实机验收点 |
| L4 行为类(终端生命周期、workspace 分屏/tab、设置落盘、后台任务、agent/history/hooks 链) | `terminal.e2e.ts`、`workspace-split.e2e.ts`、`settings.e2e.ts` 等 | GPUI UI 测试 + conformance + Rust 集成测试三层分摊 | 等价性逐类在豁免表「资产重生映射」附表登记 |
| 载体机制(wdio 启动器、E2E helper、execCommand 输入链) | `run-wdio.cjs`、E2E helper 族 | 不重生 | conformance harness 与 win32 矩阵是行为级对位,机制级无对应物 |

### 5. conformance 驱动桩 vs 真 CLI(裁定建议,承接 04 篇开放问题 2)

裁定建议:**协议桩替身为主,真 CLI 全链路不自动化**。

- 理由一(可重复性,裁定标准):真 AI CLI 需要凭据、网络、配额、账号态——27 家矩阵不可能全自动化;即便单家(CLI 如 claude)也非确定性外部依赖, golden 基线会被外部漂移持续打断,conformance 失去「机械回归锚」的意义。
- 理由二(覆盖等价):conformance 的价值在「Runtime API → 编排层 → pane 子进程 → 网格」链路的真实驱动;该链路真实性由**应用进程与被驱动协议**保证,pane 内子进程是 echo 桩还是真 CLI 不影响链路覆盖。echo 类用例(boot/echo/resize/split/scrollback/paste/cjk_roundtrip)用确定性桩进程承载;pane.exec 非 TTY 子进程归 04 篇,正好承载桩。
- 理由三(边界清晰):真 CLI 端到端行为属「产品验收」而非「回归门禁」,登记为豁免表实机验收点(有界清单:默认 shell 启动、一家 CLI 冷启动握手目测)——与 pebrel 明示「不声称覆盖 IME/剪贴板确认」的边界纪律同构。
- 落地形态:conformance harness 增加桩模式参数(`--stub echo` 形态,桩 = 仓内小脚本或编译期 test binary,确定性回显 + bracketed paste 宣告);默认入口跑桩;真 CLI 通道不建。session 用例的「强制进程终止/冷恢复」对桩与真 shell 同义,照跑。

### 6. package-release 体积预算钉测试与安装器自测随迁(与 12 篇边界)

边界:12 篇写打包链本体(Inno installer.iss、staging、zip、体积工程 profile);本篇只写**测试钉**——钉的对象是 12 篇的契约数值,契约数值本身归 12 篇首版实测校准。

随迁两件(形态照抄 pebrel `scripts/tests/` 同族,自断言 PS1,退出码即判定):

1. **`installer.tests.ps1`**(对位 pebrel 同名族):钉安装器构建产物的结构契约——installer.iss 参数化值回读(AppName/版本/AppId GUID 归 12 篇)、输出文件存在性与命名、双语 isl pin 完整性。隔离设计照抄 pebrel:强制空输出目录 + 输出路径拒绝逃逸出 `target/`(pebrel `package-release.tests.ps1` 的 `resolvedOutput.StartsWith($expectedRoot)` 守卫同形态)。
2. **`package-release.tests.ps1`**(对位 pebrel 同名):钉 staging 清单全等校验逻辑、新鲜度核验链(禁 SkipBuild 语义、版本回读与 Cargo.toml 一致)、体积预算钉(exe 体积上限断言,数字归 12 篇钉)。体积预算钉数字首版实测校准前,测试以「登记值 + 注释标明首版校准」形态入仓,校准后同提交锁死。

不随迁(协调改判已定):GitHub Release 编排测试(preview/stable release 族)、CI 专用自测。运行入口归 12 篇打包链;本篇登记进测试体系总图表(目标形态节第 3 层)。

### 7. 门禁本地化入口的测试消费序与双形态编译门禁

本地化总入口归 01 篇(git hooks + CLAUDE.md 登记);本篇定测试消费序——入口内测试阶段的固定顺序(照 architecture.yml 步骤序的依赖关系):

1. 门禁自测四件先跑(门禁不可信则后续全不可信);
2. 禁名回流 staged + push 模式;
3. file-budgets / dependencies(`--base` 棘轮归 01);
4. `cargo test --workspace -- --test-threads=1`(feature on 形态,BE-06);
5. **双形态编译门禁**:feature off 形态 `cargo check -p slterm_app` 必须过(UI 用例整体不编译的形态同样是受支持形态;照 spec 采纳点 12「一条命令同时覆盖两种形态」);
6. clippy + rustfmt;
7. 可选项(不在每次提交跑,归验收点/定期):win32 输入矩阵、内存压测、conformance、体积钉。

GPUI UI 测试与单测混编的开关纪律照抄(spec 采纳点 12):feature 未开时 UI 用例整体不编译——这保证无 GPU/无显示环境也能跑纯单测形态;两形态都绿才算全绿。

### 8. 脚本层新增本地件清单

除照抄件外,本篇新建的本地脚本(均入 file-budgets 的 `scripts` root):

- `scripts/check_all.ps1`:上述消费序的单一入口(归 01 篇 hooks 互备,用户手动全量跑);
- `scripts/pr_size_hint.ps1`:行数统计提示(超 1500 警告不阻断,生成物排除口径照抄 pr-size.yml;是否纳入 check_all归开放问题 2);
- `scripts/tests/test_test_infra.py`:本篇测试点清单中脚本层断言的承载(unittest 形态,归门禁自测族)。

## 测试点清单

本篇只列**测试体系自身**的自测用例;各领域用例归 02-10 篇。

| 测试 | 层级/形态 | 机制 |
|---|---|---|
| 门禁自测四件全绿(budgets / dependencies / governance / brand_regress) | python unittest | 照抄 pebrel 对应用例,词表/根目录改造(01 篇出口) |
| 禁名门禁假阳边界夹具(ordinary-comparison 等) | 同上 fixtures | 照抄,词表替换 |
| `file_line_budget` 适配器两用例 | Rust(tests/) | 见关键类型节;与 python 门禁同数据文件双侧消费一致性 |
| `windows_console_startup` console 子系统目标编译 + Ctrl+C 继承用例 | Rust(tests/) | 见关键类型节 |
| i18n 零分配/小栈/载荷预算三用例 | Rust(tests/) | 若 06 篇 i18n 落地即建(已照抄裁定,非条件项) |
| `gpui-test-support` 双形态编译门禁(feature on 全量绿 / feature off check 过) | cargo | check_all 步骤 4-5;rust-analyzer 默认 feature 形态也要绿 |
| 虚拟窗口冒烟:首个 `ui_tests.rs` 模板用例(debug_bounds 取真实坐标 + simulate_click 命中) | #[gpui::test] | M3 终端渲染关键路径落位时建(模板归关键类型节) |
| 键绑定真实分发双形态(解析矩阵 + Root 分发传播) | 单测 + #[gpui::test] | 复制 = Ctrl+Shift+C 语义的首个端到端锚 |
| SPAWN_LOCK 串行化用例(并发 spawn 不卡死管道) | Rust 集成 | 归 02 篇领域,组织形态本篇定型(M2) |
| win32 矩阵基线自身健康:probe ready 轮询预算、`-Record` 重登记路径 | PS1 自断言 + 注释 | 预算值标「负载裕度」 |
| conformance harness 自测:golden 缺席 ≠ 失败、失败拒写 golden、只杀自有进程 | python unittest(test_test_infra.py) | 桩模式参数解析归改造节 5 |
| 内存压测相位模型自检(sample/complete_samples/checkpoints 空转通过) | python unittest | 阈值归待沉淀 D3 |
| 豁免表结构断言:销项行保留根因、实机验收点段存在、重生映射附表存在 | python(unittest discover 或 governance 件扩) | 防豁免表静默退化 |
| 命名约定抽查:新增测试文件符合三模式(mod 具名/三文件拆分/tests 目录) | 脚本 lint(check_all 提示级) | 非阻断 |
| 体积钉与安装器自测两件(installer.tests.ps1 / package-release.tests.ps1) | PS1 自断言 | 归改造节 6,M11 随迁 |
| 防复发三要素锚点注释抽查(bugfix 提交) | 评审清单(非机械) | 口径归改造节 2 |

## 阶段归属与出口标准

测试基础设施**随各 M 阶段伴生,不设独立测试阶段**。每个 M 阶段开工前必须就绪的测试件与机验出口:

| 阶段 | 开工前必须就绪的测试件 | 本篇范围的机验出口 |
|---|---|---|
| M0 fork 基线 | 门禁三件套 + 自测四件 + hooks(归 01);`.claude/test-exemptions.md` 重审初稿(改造节 3) | 自测四件全绿;豁免表重审完成(销项/保留/改写三分完毕) |
| M1 底库迁入 | 各 crate `mod <领域>_tests` 组织形态;三文件拆分模式可用;file_line_budget 适配器就位 | `cargo test --workspace` 全绿;file_line_budget 两用例过;双形态编译门禁建立 |
| M2 终端核心 | `keyboard_contract_tests.rs` 族夹具;pty 集成测试目录组织;win32 矩阵脚本 + 基线 `-Record` 建基 | 键盘契约矩阵绿;SPAWN_LOCK 集成用例绿;win32 基线常挂比对过 |
| M3 app 骨架 | `gpui-test-support` feature + 虚拟窗口补丁消费;首个 `ui_tests.rs`(终端渲染关键路径);keymap 双形态 | 虚拟窗口 UI 测试过(00-roadmap M3 出口);feature on/off 两形态绿 |
| M4 AI CLI 集成 | hook 链路契约测试组织(归 03 用例点) | hook 契约测试过(归 03 出口,本篇只认组织形态就绪) |
| M5 工作区与布局 | session round-trip serde 测试组织;分屏树 UI 测试 builder | session 往返测试过(归 05) |
| M6 设置/主题/i18n | i18n 零分配契约三用例;i18n 键集硬合同(eg 06) | 零分配/小栈/预算三用例绿 |
| M7 Runtime API + 单实例 | conformance 套件(Windows 裁减 + 桩模式) + golden 建基;单实例移交测试归 04 | conformance 全用例绿 + golden 建基;「golden 缺席 ≠ 失败」自测过 |
| M8 文件与编辑 | 编辑器内核测试组织归 07;UI builder 复用 | 归 08 出口 |
| M9 AI 辅助 | 补全引擎/should_suggest 测试归 08 | 归 09 出口 |
| M10 系统集成 + 安全 | 内存压测脚本 + 检查点;凭据域边界测试归 10 | 内存压测相位自检过(阈值归 D3) |
| M11 打包 + 收尾 | installer.tests.ps1 / package-release.tests.ps1 随迁;文档防腐条款归 12 | 体积钉/安装器自测绿(eg 12 出口);豁免表终版(实机验收点全量登记) |

伴生纪律:任何 M 阶段新建测试基础设施类型(新 builder、新夹具、新基线文件),先回本篇「关键类型与签名」节登记;各篇测试点清单引用本篇形态时只写例名,不写机制。

## 待沉淀决策

> [待沉淀] **D1 · lib_tests 显式 test target 形态归留**。真实权衡:多 crate workspace 下每 crate 复刻 `[lib] test = false` + `[[test]] name = "<crate>_tests"` 显式 target = manifest 控制可及(未来再遇 comctl32 v6 类静态导入时防线直接生效),代价 = 每 crate 两处样板 + 定向测试入口文档全部按此写;用默认 lib test target = 零样板,代价 = 防线价值仅存于预防性知识。难逆点:CLAUDE.md/豁免表/本篇一旦按某形态写死,翻转 = 全仓入口改法。意外因素:GPUI 依赖树未来若引入静态导入 comctl32 v6 的 crate(安装器/对话框类),默认 target 形态可能重新踩坑——届时无显式 target 即无 `rustc-link-arg-tests` 落点。截止:M1 首 crate(slterm_settings)落位时实测 `cargo test -p slterm_settings --lib` 与 doc test 行为后定,决策记录入 adr.md。

> [待沉淀] **D2 · 统一测试入口是否封装(xtask / PS1 vs 裸 cargo 命令)**。真实权衡:封装入口(`cargo xtest <filter>` 或 `scripts/test.ps1`)= 定向测试防错(强制 `-p` + 串行 + feature 形态切换一条命令),代价 = 抽象层与习惯迁移成本,且 Rust 生态习惯是裸 cargo;裸命令 = 零抽象,代价 = TQ-COV-06 类陷阱靠人记忆防。难逆点:入口被 CLAUDE.md/豁免表/本篇引用后改名成本高。意外因素:若 D1 定「每 crate 显式 target」,定向入口复杂度上升,封装价值同步上升——D1/D2 联动。截止:M1 门禁入口首写时定(默认裸命令 + 文档写明红线,封装作为 D1 选中显式 target 时的伴随项重估)。

> [待沉淀] **D3 · 内存压测阈值首版校准**。真实权衡:私有提交内存回落阈值与检查点数值定严 = 驱动/合成器环境差异致长期误红(GPU 驱动版本、DPI、壁纸解码路径都影响私有提交),定松 = 泄漏回归漏报;pebrel 只给相位模型不给 slterm 数值。难逆点:数值入仓即契约,日后收紧 = 与历史报告不可比。意外因素:GPUI 的 allocator 行为与 winit/glutin 驱动相关,M10 前无真实长稳数据。截止:M10 系统集成点亮后,实测一周稳定基线再以 `-Record` 同形态锁入检查点(与 win32 基线同治理律)。

## 开放问题

1. **conformance 驱动桩 vs 真 CLI**(承接 04 篇开放问题 2):本篇改造节 5 已给裁定建议(桩为主,真 CLI 登记实机验收点),最终裁决权归用户——04 篇不预设本篇也不预设;若用户选真 CLI 通道,豁免表实机验收点段同步增列有界清单(默认 shell + 一家 CLI 冷启动)。
2. **pr-size 本地提示脚本是否纳入日常入口**:spec 采纳点 20 遗留裁决(「是否纳入日常入口由用户裁决」)。默认:`pr_size_hint.ps1` 建但不挂 check_all,作为按月/按 release 提示;挂入 = check_all 加一步警告级。
3. **实机验收点的执行节奏**:豁免表实机验收点清单(native QA、chord 矩阵、IME、剪贴板确认、真 CLI 验收)登记后,触发点与频率归用户习惯——候选:每次 M 阶段出口前过一遍 / 仅 M11 前全量过;默认后者 + 前者可选,写进豁免表执行方式列。







