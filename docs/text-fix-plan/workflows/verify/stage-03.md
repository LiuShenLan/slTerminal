# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式，防"改名迎合"；正向意图断言同样写语义式，防"字面通过"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 tsc/eslint 覆盖外文件时补 npx vite build 构建级兜底
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态（本 Stage 拆分先行，git 测试分散至新文件，计数按新布局推导）
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令
-->

## 断言清单

- **GIT-01**：五命令（git_status/git_diff/git_file_at_head/git_rollback/git_unstage）各有经 `block_on` 调真实命令的用例（grep `block_on` 命中各命令测试文件，每命令 ≥3 条：happy/沙箱拒绝/错误契约）；inline 重写 git2 调用序列的测试已改为调真实命令或标注"底层原语"（Read 抽查，无复制命令内部逻辑的整段测试）
- **GIT-02**：`git_rollback_two_step_` 前缀用例不复存在（grep 零命中）或显式标注已废弃（二选一）
- **GIT-03**：`non_renamed_old_path` 用例重写为构造非 renamed/renamed 两路断言（Read 确认，不接受循环内 continue 后恒真条件）
- **GIT-04**：`status_to_str` 表驱动含 `git2::Status::CONFLICTED` → `"conflict"` 用例
- **GIT-05**：`compute_diff_hunks` 三处边界各有用例（修改后多余新增行精确 hunk、prev_was_del flush、非 UnbornBranch HEAD 错误）
- **GIT-06**：`init_temp_repo` 内设置仓库局部 `core.autocrlf=false`、`core.safecrlf=false`、`init.defaultBranch=main`（Read 确认）
- **GIT-07**：git_status 五条弱断言用例改精确断言（路径集合 + 状态串 + 条目数，Read 抽查 ≥2 处）
- **GIT-08**：四条 diff 测试改名或补精确断言；.gitignore 用例改用 `add_ignore_rule` 内存规则或消除磁盘时序（Read 确认）；模块 CLAUDE.md 声明 git CLI 最低版本（→ DOC-04，本 Stage 可只留注释待 Stage 17 收编）
- **GIT-09**：`git_file_at_head_unborn_branch_err` 改调真实命令，断言 AppError 消息含"HEAD 中不存在"
- **GIT-10**：五命令 `validate_path_within_root` 拒绝用例各 ≥1（随 GIT-01 命令层测试落位，grep 拒绝用例存在于各命令测试文件）
- **GIT-11**：`ci_l1_uses_single_test_thread` 位于 `src-tauri/tests/ci_config_tests.rs`（grep 命中），`git/mod.rs` 零命中
- **GIT-12**：git 测试按命令拆分完成——`tests/` 下存在 status/diff/at_head/rollback/unstage 测试文件（Glob 命中）；`init_temp_repo`/`commit_file` 位于共享 test_utils（Read 确认）；`git/mod.rs` 测试区 `#[test]` 计数下降、拆分新文件计数上升，git 域总用例数 ≥ 基线 88 − 删除数 + 新增数（静态 grep 口径，inventory 记录口径）
- **门禁**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿
- **门禁**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零警告

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
