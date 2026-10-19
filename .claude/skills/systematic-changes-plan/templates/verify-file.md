# Stage __STAGE_NUMBER__ 逐项验证断言（唯一真值源）

> stage-__STAGE_NUMBER__ 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件 + 符号）；全量测试任一命令失败则相关项判 not_fixed。

<!--
生成纪律：
1. 逐条目对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写「检查是否合理」
3. 「禁止存在 X」类断言写语义式，防「改名迎合」；正向意图断言同样写语义式，防「字面通过」
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 core 生产代码时补架构门禁与 clippy，触及文档时补文档策略门禁，触及 UI feature 时补虚拟窗口 target，触及 PTY 时补集成 target
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令
-->

## 断言清单

- **<条目 slug>**：断言内容（含证据要求，如：grep `pattern` 命中 `<文件>`；Read 确认某函数无某调用）
- **<条目 slug>**：断言内容
<!-- 语义式断言示例（「禁止存在 X」类，防改名迎合）：
- **settings-write-channel**：`settings_store` 之外不存在任何直写 settings.json 的调用点（不限函数名——`fs::write`/`OpenOptions` 等均属违规）；所有写入全部经 settings_store/audit 包装（须 Read 代码确认，非仅 grep）
-->
<!-- 正向意图断言同样写语义式（防字面通过）：
- **runtime-api-contract**：新增方法必须同时出现在 schema、`runtime.describe` 与随包文档中，参数/错误码键集合精确一致（须 Read 三处确认）
- **ui-thread-discipline**：不存在任何在 GPUI 前台线程执行阻塞 I/O、网络或子进程等待的调用点（含 UI 测试辅助代码，须 Grep 全仓逐处确认）
- **doc-accuracy**：相关 `CLAUDE.md` 的描述与当前代码一致（文档类断言须对照真实代码核实，防文档撒谎）
-->

## 全量测试（全部通过为门禁）

__TEST_COMMANDS__
<!-- 示例：
1. `cargo test --workspace -- --test-threads=1`
2. `cargo test -p slterm --test lib_tests --features gpui-test-support -- --test-threads=1`
3. `cargo clippy --workspace --all-targets`
4. `cargo fmt --all -- --check`
5. `python scripts/check_architecture.py`
6. `python scripts/check_doc_policy.py full`
（有 Stage 特殊要求时补充，如触及 PTY 时加 `cargo test -p slterm_terminal --test pty_integration_tests -- --test-threads=1`）
-->
