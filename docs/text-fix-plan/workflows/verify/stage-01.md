# Stage 01 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式，防"改名迎合"；正向意图断言同样写语义式，防"字面通过"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 tsc/eslint 覆盖外文件时补 npx vite build 构建级兜底
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令
-->

## 断言清单

- **PTY-01**：`spawn.rs` 存在 Job Object 纯逻辑（job_name 构造 / limit flags 计算，函数名不限）且有 L1 用例；`JobHandle::drop` 相关路径在 pty 域测试可触及（Read 确认）
- **PTY-02**：`spawn.rs` 存在 `validate_spawn_request` 纯函数（尺寸超限 / shell 白名单 / cwd 沙箱三校验）且被 `pty_spawn` 调用（Read 确认调用点，不限签名细节）；尺寸超限拒绝、shell 白名单拒绝、cwd 沙箱拒绝各有用例（用例经该纯函数或命令层）
- **PTY-03**：`spawn.rs` 存在 `validate_session_ownership` 纯函数（SEC-08 panelId 归属校验）且被 `pty_write`/`pty_resize`/`pty_kill`/`pty_reattach` 调用（Read 确认）；归属放行与归属拒绝各有用例
- **PTY-07**：`build_cmdline` 引号处理用例存在（含空格路径、含空格参数、无空格不加引号三场景，函数名不变）
- **PTY-08**：`spawn_conpty_child` 可纯化部分（命令行/环境块构造）有用例，或标注"由 pty_spawn_custom_conpty 集成测试 + CI 守卫"（二选一，Read 确认）；纯 Win32 调用部分无 L1 用例属预期
- **PTY-09**：`ConPtyMaster::resize` HPCON invalid 分支有用例（drop/关闭后 resize 静默成功且 size 更新）
- **PTY-13①**：spawn.rs 测试区三处重复清理代码已抽 helper（Read 确认测试区无三份以上逐字重复清理块）
- **禁区**：`compute_conpty_flags` 及其 4 条守卫测试零改动（`git diff` 无命中；行数/内容与 checklist 引用一致）
- **门禁**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿；pty 域 `#[test]` 计数 ≥ 基线 105 + 本 Stage 新增数（静态 grep 口径）
- **门禁**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零警告
- **硬约束 #9**：无 `#[cfg(windows)]` 新增到 spawn.rs 以外（grep `#[cfg(windows)]` 于 pty 模块外零新增命中；业务逻辑不撒 cfg）

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
