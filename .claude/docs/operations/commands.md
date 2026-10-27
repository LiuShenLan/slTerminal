# 命令矩阵

> 构建、测试、静态检查、门禁与发布命令；工作区根即仓根，无 npm/Node。

## 决策与红线

- **构建**：`cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook`；发布构建走 `scripts/release.ps1` 的同一 feature 图。
- **验收**：用 debug 构建产物 `target/debug/slterm.exe` 实测，不走交互式 dev 模式。原因：验收对象必须是实际交付形态。
- **全量测试**：`cargo test --workspace -- --test-threads=1`；串行执行是硬红线。
- **core 发布模式测试**：`cargo test --release --workspace --exclude slterm --exclude slterm_hook -- --test-threads=1`；覆盖调试断言关闭后的语义，排除集合与架构层一致。
- **定向测试**：`cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`；禁止 `--lib` 或裸 filter 直打默认 lib target。
- **虚拟窗口测试**：`cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`。
- **平台/真机测试**：`cargo test -p slterm_terminal --test pty_integration_tests -- --test-threads=1`；`cargo test -p slterm --test windows_console_startup -- --test-threads=1`。
- **静态检查**：`cargo clippy --workspace --all-targets`；`cargo clippy -p slterm --all-targets --features gpui-test-support`；`cargo fmt --all -- --check`。
- **架构门禁**：`python scripts/check_architecture.py`。
- **文档策略门禁**：`python scripts/check_doc_policy.py full`；提交前可用 `staged`。
- **禁名门禁**：`python scripts/check_brand_regress.py staged`；提交信息与推送由 `.githooks/` 三钩扫描。
- **git hooks 启用**：`git config core.hooksPath .githooks`（一次性）。
- **脚本门禁自测**：`python -m unittest discover -s scripts/tests -p "test_*.py"`。
- **打包钉测试**：`pwsh -File scripts/tests/package-release.tests.ps1`；`pwsh -File scripts/tests/windows-runtime.tests.ps1`。
- **发布**：`pwsh -File scripts/release.ps1 -Version <X.Y.Z>`；要求 PowerShell 7。
- **发布核验**：`scripts/release-checklist.md` 人工全勾；「命令成功 ≠ 发布核验完成」。

## 验收与测试

- 提交前全部门禁全绿：架构/依赖、文档策略、禁名、clippy、fmt、测试。
- 全量测试与脚本钉测试不得并行。
- 测试策略与豁免规则见 `.claude/docs/testing/strategy.md` 与 `.claude/docs/testing/exemptions.md`。

## 操作

- 本地开发循环：`cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook` → 运行 `target/debug/slterm.exe` → 定向测试。
- 发布循环：改版本与 CHANGELOG → `pwsh -File scripts/release.ps1 -Version <X.Y.Z>` → 人工核验清单。
- 脚本与环境细节见 `.claude/docs/release/packaging.md` 与 `scripts/CLAUDE.md`。
