# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **PTY-04**：`strip_conpty_startup` 的 OSC 1/3/4/9 保留、CSI 3J 各有用例；非 Windows 原样返回分支有标注或 `cfg!(windows)` 常量断言（Read 确认，不限实现方式）
- **PTY-05**：`ring_buffer_append` 无换行淘汰三边界有用例（恰好 1024 / 超 1024 / 含换行）
- **PTY-06**：`resolve_shell_info` 回退顺序经可控 PATH 验证（pwsh→powershell→cmd 三场景，tempdir 放假 exe；函数名不变）
- **PTY-10**：`resolve_shell` 回退顺序用例存在；白名单"PATH 解析后仍非法 shell"拒绝用例存在
- **PTY-11**：`validate_path_within_root` 相对路径 `..` 穿越拒绝用例存在且调该函数（D7 防复发）
- **PTY-12**：reader_loop 可抽决策点（channel 断开→ring buffer 分流、EOF 处理）有用例，**或** `src-tauri/src/pty/CLAUDE.md` 出现豁免标注草稿（二选一必有其一，Read 确认）
- **PTY-13②③**：`canonicalize_or_ancestor` relative path 分支、`which_full_path` PATH 顺序各有用例（state.rs / shell.rs 测试区）
- **禁区**：`compute_conpty_flags` 及其 4 条守卫测试零改动（git diff 无命中）
- **门禁**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿
- **门禁**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零警告

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
