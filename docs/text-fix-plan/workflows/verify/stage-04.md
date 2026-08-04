# Stage 04 逐项验证断言（唯一真值源）

> stage-04 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **HUK-01**：`process_signal_file` 全流程用例存在（tempdir 构造信号文件，断言 读→emit→删；emit 可注入——闭包/trait 形式不限）；emit 失败仍删除文件、非法 JSON 降级各有用例
- **HUK-02**：注入三命令（inject/uninstall/injection_status）经路径可注入 impl 的 L1 场景用例存在（tempdir 驱动：注入/幂等/非法 JSON 中止/卸载混组保用户 handler/状态三态，Read 确认 impl 存在且被命令调用）
- **HUK-03**：watcher 双通道用例存在（`run_one_tick` 纯函数或临时目录真实启动集成测试：轮询补漏消费残留文件、目录删除重建后恢复；二选一，Read 确认）
- **HUK-04**：`start_signal_watcher` 有 `#[cfg(test)]` 重置钩子且幂等启动用例存在（首次启动/重复启动仅一个 watcher）
- **HUK-05**：`hooks_context_usage` 命令包装层用例存在（参数透传 transcriptPath、None/Some 返回映射）
- **HUK-06**：config 读写 IO 异常分支有用例（home_dir 失败或注入失败点、persist 失败或不可写路径；方式不限）+ 命令包装透传用例
- **HUK-07**：user 层 home 解析已可注入（测试注入 tempdir，不依赖真实 `dirs::home_dir()`——Read 确认测试无真实 home 依赖）
- **HUK-08**：`inject_adds_10_events` 改结构断言（每事件 handler 数组含 `{type:"command", timeout:5, command 含 slterm-hook-reporter}`，键集合精确匹配）
- **HUK-09**：hooks/mod.rs、signal.rs serde 测试改序列化→反序列化往返精确断言（`contains` 弱断言零残留，grep 确认）
- **HUK-10**：P2-TE-05 与 scan_transcript_usage 重复用例已合并去重（同一纯函数路径只存一组；保留组经命令包装层调用或改名，Read 确认无逐字重复）
- **HUK-11**：watcher stop 测试含 `thread.is_finished()` 断言；`handler_contains_slterm` 非字符串 command（number/null/缺失）返回 false 用例存在
- **禁区**：`assets/slterm-hook-reporter.js` 零改动（git diff 无命中）——C10 契约不可改
- **门禁**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿
- **门禁**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零警告

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
