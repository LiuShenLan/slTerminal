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
