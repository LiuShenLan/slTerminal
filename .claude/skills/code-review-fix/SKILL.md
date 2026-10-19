---
name: code-review-fix
description: 对代码库执行多维度 exhaustive 审查，生成报告，并按指定级别修复发现的问题。
disable-model-invocation: true
---

# 代码审查 + 修复

对代码库执行全量多维度审查，生成优先级报告，并自动修复发现的问题。

## 参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--fix-level` | `all` | 修复级别：`P0` / `P1` / `P2` / `all` |
| `--dimensions` | 全部维度 | 审查维度，逗号分隔：`perf,security,dead-code` |
| `--output` | `temp/reviews/` | 报告输出目录（不入库） |

## 审查维度

| 维度 | 参数名 | 内容 |
|------|--------|------|
| 性能 | `perf` | 渲染管线、锁竞争、文件 I/O、启动耗时、内存使用 |
| 稳定性 | `stability` | panic 面、竞态条件、异常路径、错误恢复、资源耗尽 |
| 死代码 | `dead-code` | 未使用导出/依赖/模块、空文件、调试残留 |
| 测试覆盖 | `test-coverage` | 单元/集成/虚拟窗口/脚本层测试盲区、Runtime API 与 hook 契约覆盖、mock 质量 |
| 安全性 | `security` | 凭据域、沙箱路径、命令注入面、输入校验、依赖漏洞 |
| 架构合规 | `architecture` | 项目 CLAUDE.md 中声明的架构约束逐条验证 |
| 错误处理 | `error-handling` | 错误变体覆盖、unwrap/expect 审计、错误传播与静默 catch、用户可见消息 |
| 依赖审计 | `dependencies` | 安全漏洞、过期依赖、未使用依赖、许可证兼容 |
| 契约一致性 | `contract` | Runtime API 方法/字段、hook 协议、设置键、i18n 键集、DTO 双边对应 |
| 资源管理 | `resource` | 生命周期追踪、句柄泄漏、事件监听解绑、环形缓冲正确性 |

## 执行流程

### 阶段 0：准备

1. 读取项目根 `.claude/CLAUDE.md` 与目标路径最近的 `CLAUDE.md`，提取：
   - 架构约束（作为 `architecture` 维度的审查依据）
   - 模块划分（作为文件分组依据）
   - 测试命令（用于验证）
2. 根据项目规模动态决定 Explore agent 数量，探索项目结构：
   - 目录树、文件分布、语言组成
   - 测试框架和覆盖率工具
   - 依赖清单（`Cargo.toml`、`scripts/` 依赖）

### 阶段 1：审查 — 无依赖维度（并行执行）

将无依赖维度用 **一个 Workflow** 编排，每个维度内部按文件/模块拆分子 agent。

| 维度 | 拆分策略 |
|------|----------|
| 死代码 | 按 crate 拆（`slterm_app` / core crates / `slterm_hook` / scripts） |
| 测试覆盖 | 按测试层拆（单元/集成 + 虚拟窗口 + 脚本门禁 + 豁免登记） |
| 架构合规 | 按约束条目逐一验证 |
| 契约一致性 | Runtime API schema/describe、hook 协议、设置键、i18n 键集逐对校验 |
| 错误处理 | 按模块拆（终端 core + 应用壳 + 平台封装 + 用户可见消息） |
| 依赖审计 | `cargo audit` + 依赖清单核对 |

每个维度 agent 直接写入 `<output>/<序号>-<维度>.md`。

### 阶段 2：审查 — 依赖前批结果的维度（并行执行）

依赖前批结果的维度 agent prompt 中需注入前批关键发现作为上下文。

| 维度 | 拆分策略 | 依赖 |
|------|----------|------|
| 安全性 | 凭据域 + 沙箱路径 + 命令注入面 + 输入校验 | 死代码、架构、契约 |
| 性能 | 渲染管线 + 锁竞争 + 文件 I/O + 内存 | 死代码、错误处理 |
| 稳定性 | panic 面 + 竞态条件 + 异常路径 | 错误处理、架构 |
| 资源管理 | 生命周期追踪 + 句柄泄漏 + 事件解绑 | 架构、死代码 |

同样直接写入 `<output>/<序号>-<维度>.md`。

### 阶段 3：汇总

一个 agent 读取全部维度报告，去重合并，生成 `<output>/README.md`：

1. **审查概览**：日期、范围、方法、总体评分
2. **发现统计**：维度 × 级别 矩阵表
3. **P0 必须修复清单**（详细说明 + 修复方案 + 预计工作量）
4. **P1 高优先级清单**
5. **P2 改进建议清单**（按维度分组）
6. **修复路线图**：建议的批次和顺序
7. **去重映射表**：跨维度重复问题的合并记录

### 阶段 4：修复

按 `--fix-level` 参数执行。使用一个 Workflow 编排，按优先级分组避免文件冲突：

| 优先级 | 拆分策略 | 约束 |
|--------|----------|------|
| P0 | 每个问题独立 agent | 同文件问题合并为一个 agent，避免编辑冲突 |
| P1 | 同模块问题合并 agent | 与 P0 无文件冲突即可并行 |
| P2 | 同类型问题合并 agent | 与 P0/P1 无文件冲突即可并行 |

每个修复 agent 必须：
1. 读取相关文件 → 做出修改 → 为新代码编写测试 → 运行测试 → 确认全绿
2. 输出修复记录到 `<output>/fixes/<issue-slug>.md`

**硬约束：**
- 不破功能：修改前后业务逻辑不变
- 测试全绿：改动前跑测试确认基线，改动后跑全量确认全绿
- 增量覆盖：每个改动添加对应用例
- 隔离执行：同文件修复必须串行，无关文件修复可并行
- 未登记豁免：不可自动化部分必须登记 `.claude/docs/testing/exemptions.md`

### 阶段 5：验证

修复完成后执行全量回归：

| 步骤 | 命令 |
|------|------|
| 构建 | `cargo build -p slterm --bin slterm -p slterm_hook --bin slterm-hook` |
| 全量测试 | `cargo test --workspace -- --test-threads=1` |
| 虚拟窗口测试 | `cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1` |
| PTY 集成测试 | `cargo test -p slterm_terminal --test pty_integration_tests -- --test-threads=1` |
| 静态检查 | `cargo clippy --workspace --all-targets`；`cargo fmt --all -- --check` |
| 架构门禁 | `python scripts/check_architecture.py` |
| 文档策略门禁 | `python scripts/check_doc_policy.py full` |
| 脚本门禁自测 | `python -m unittest discover -s scripts/tests -p "test_*.py"` |
| 打包钉测试 | `pwsh -File scripts/tests/package-release.tests.ps1` |

输出 `<output>/fixes/README.md`：汇总所有修复记录 + 验证结果。

## 判定标准

| 级别 | 定义 | 示例 |
|------|------|------|
| **P0** | 崩溃/panic、数据丢失、安全漏洞、不可恢复错误 | 未处理的 unwrap、路径穿越、凭据泄露 |
| **P1** | 功能异常、架构违规、测试盲区、竞态条件、用户体验退化 | 违反架构约束、关键路径无测试、静默吞错 |
| **P2** | 未使用代码、优化建议、代码风格、文档缺失、技术债 | 过期注解、重复代码、内存微优化 |

## 审查方法

- **Rust**：`cargo clippy --workspace --all-targets` + codegraph 调用链分析 + 逐文件阅读
- **脚本（Python/PS1）**：`python -m unittest` + 逐文件阅读
- **依赖**：`cargo audit` + 许可证检查
- **测试**：运行现有测试套件，识别盲区与豁免登记漂移

## 报告格式

每份维度报告统一结构：

```markdown
# <维度> 审查报告

## 1. 发现列表
| # | 位置(file:line) | 级别(P0/P1/P2) | 描述 | 建议 |
|---|-----------------|-----------------|------|------|

## 2. 扫描范围
列出所有审查过的文件路径和审查方式

## 3. 未覆盖区域
哪些文件/模块因何种原因未审查

## 4. 建议修复
优先级排序的具体修复方案
```

## 注意事项

1. **Workflow 优于独立 Agent**：维度审查使用 Workflow 编排子 agent，而非单个 agent 串行扫描。大型文件应再拆分，小型文件合并。
2. **依赖注入**：后批 agent prompt 必须包含前批关键发现，避免重复扫描和矛盾结论。
3. **文件冲突处理**：修复阶段必须识别共享文件的 agent 并串行执行。编辑前先 Read 确认文件状态。
4. **增量测试**：修复 agent 先跑现有测试确认基线，改后跑全量确认无回归。
5. **去重优先**：汇总阶段跨维度去重，同一问题保留最详细描述，标注合并来源。
6. **CLAUDE.md 自适应**：审查依据从项目 CLAUDE.md 动态提取，不硬编码特定项目的约束。
7. **Explore 数量动态**：小型项目 1 个，中型 2-3 个，大型/多语言 3-5 个。
