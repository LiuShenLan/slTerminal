# 测试策略与门禁

> 测试分层、隔离纪律、命名约定与不可自动化面的登记规则；具体用例以代码为准。

## 决策与红线

- **分层职责**：

  | 层 | 形态 | 载体 | 职责 |
  |---|---|---|---|
  | 单元与集成层 | Rust 单测与集成 target | 各 crate 内嵌 `mod <领域>_tests` + `tests/` 集成 target | 协议语义、纯函数契约、serde round-trip、注册表契约、平台串行化 |
  | 虚拟窗口 UI 层 | `#[gpui::test]` + `TestAppContext`/`VisualTestContext` | feature `gpui-test-support` 统一门控 | 真实控件、布局、键盘、hover 路径 |
  | 脚本门禁层 | python unittest、PS1、cargo | `scripts/tests/` 与仓库脚本 | 行数棘轮、依赖方向、禁名回流、打包钉测试、输入矩阵 |
  | 豁免登记层 | 单表真值源 + 模块文档明细 | `.claude/docs/testing/exemptions.md` | 一切残余不可自动化面（实机验收点、条件跳过） |

- **显式测试 target**：每个 crate 使用 `[lib] test = false` + 显式 `[[test]] name = "lib_tests" path = "src/lib.rs"`。原因：默认 lib test target 可能收不到测试专用链接参数与 manifest 控制，历史上表现为零输出崩溃。边界：定向测试一律 `cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。遇零输出崩溃先查测试 target 形态与链接参数，不要当业务代码 bug 排查。
- **全量回归串行**：cargo 全量与脚本钉测试禁止并行。原因：性能基准与 PTY 容量断言对 CPU 负载敏感，并行会间歇误红并掩盖真实回归。边界：全量命令固定带 `--test-threads=1`。
- **发布模式语义回归**：debug 全量之外，core 必须执行 release 串行测试。原因：调试断言关闭与优化差异可能暴露 debug 测试无法发现的语义偏差。边界：release core 不替代 application、hook 或虚拟窗口测试；排除集合由架构层治理测试钉住。
- **隔离优先**：Rust 测试用 `tempfile::tempdir()` 构造独立 `AppPaths` 双根；共享测试缓存只指向进程专属临时双根，不读真实数据根覆盖变量。进程级资源用 `SPAWN_LOCK`/`ENV_LOCK` 串行化；UI 测试用虚拟窗口与隔离配置目录。原因：测试不得读写真实用户目录或互相污染。边界：涉及真实桌面会话、凭据库、安装器的用例必须走条件跳过或豁免登记。
- **负载裕度登记**：因 CPU 负载导致间歇红而提额超时/轮询预算时，必须在改动点注释标明「负载裕度，非断言/语义放宽」。原因：区分时间预算调整与断言语义放宽。边界：断言语义不变、只放大时间预算，且真 bug 条件下该红仍红；凡须同步放宽断言值/判据的，按语义变更走完整评审。
- **缺陷修复附防复发测试**：修复缺陷或优化历史功能时，除对改动代码添加全量测试外，还须对照改动前老代码补回归用例。原因：防止同类缺陷以另一种形态复发。边界：无法自动化的部分按豁免规则登记。
- **恢复超时分类回归**：Shell 就绪等待的测试必须覆盖真实程序名分类（cmd、PowerShell、未知）与各自上限；测试夹具的伪 Shell 名不属于真实程序，按未知分支的上限处理。原因：终端视图曾自带一份「非 cmd 一律长等待」的分类，统一到导航恢复后未知分支一度被设为短延迟，导致夹具在 Shell 未就绪时注入；伪 Shell 名不参与真实分类，改上限或新增类别必须同时重跑 `terminal_restore_timeout_is_bounded_and_single_shot` 与 `resume_delay_is_shell_specific_and_bounded`。边界：真实 PTY 的提示符到达时序仍由实机人工验收。
- **测试输出不截断**：全量、发布模式与定向 cargo 测试禁止用 `grep`／`tail` 等管道过滤或截断输出；后台执行必须把完整 stdout／stderr 直接重定向到日志文件。原因：管道会丢弃逐测试进度，`tail` 还只在输入 EOF 时才输出，测试挂起时日志长期为空，无法判断是编译慢、执行慢还是死锁；曾因此把一次测试挂起误判为执行缓慢。边界：只允许在成功结束后对日志做检索；执行中的观察读日志文件本身。
- **挂起先判性质再定位**：长时无结果的测试命令先核对进程 CPU 时间与线程状态，再决定是否等待。原因：编译与正常执行会持续消耗 CPU，挂起进程的 CPU 时间近似零且线程全部处于 Wait，两者处置完全不同。边界：`cargo test` 自身 CPU 近零且测试 exe 已存在但 CPU 停滞即按挂起处理；测试 exe 归属可用 `target/debug/deps/lib_tests-<hash>.d` 首行源文件列表确认。
- **命名约定**：内嵌测试模块 = `mod <领域>_tests`（一文件多测试模块用功能具名）；测试函数 = 「对象_行为_场景」snake_case 裸名，禁 `test_` 前缀与规格编号前缀；集成测试 = `<域>_tests.rs`；脚本 = `test_*.py` / `*.tests.ps1`。
- **覆盖面判定**：「可自动化」= Rust 可测或 GPUI 虚拟窗口可测；其余登记豁免并注明根因与兜底层级，禁止未登记豁免。

## 验收与测试

- 全量：`cargo test --workspace -- --test-threads=1`。
- core 发布模式：`cargo test --release --workspace --exclude slterm --exclude slterm_hook -- --test-threads=1`。
- 定向：`cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`。
- 虚拟窗口：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- 平台/真机：`cargo test -p slterm_terminal --test pty_integration_tests -- --test-threads=1`；`cargo test -p slterm --test windows_console_startup -- --test-threads=1`。
- 脚本门禁自测：`python -m unittest discover -s scripts/tests -p "test_*.py"`。
- 豁免明细见 `.claude/docs/testing/exemptions.md`。

## 操作

- 新增测试前先判断层级：纯函数与协议进单元层；控件/布局/键盘进虚拟窗口层；门禁与工具箱进脚本层。
- 新增 crate 必须带显式 test target 才能进 workspace 测试门禁。
- UI 测试只在 feature 形态编译，生产构建不带测试夹具。
- 长测试后台执行模板：`conda run --no-capture-output -n claude cargo test ... > <日志文件> 2>&1`，完成后再检索日志；执行中只看日志文件，不另起过滤管道。`conda run` 默认捕获并缓冲子进程输出，缺 `--no-capture-output` 时日志在进程结束前可能一直为空。
- 挂起处置：先记录进程名、PID、CPU 时间与线程等待状态；`taskkill /PID <cargo PID> /T /F` 终止整棵进程树；串行 libtest 日志最后一行未带 `ok` 的测试名即挂起点，用 `--exact` 或模块过滤器单独复现；修复根因前不重跑全量。
