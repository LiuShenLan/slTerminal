# Stage 05 逐项验证断言（唯一真值源）

> stage-05 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **HFN-01**：fs 写文件用例改调真实 `fs_write_file` 命令（grep `fs_write_file` 命中测试区）；测试区无重写 `use_crlf` 检测逻辑残留（Read 确认，测试只做固定输入/输出断言）
- **HFN-02**：pool 替换测试无手动 `remove`（同 path 直接两次 insert，断言旧 watcher 被 stop——grep/Read 确认测试不再先 `pool.remove`）
- **HFN-03**：`EventEmitter` trait（或等价 emit 抽象）存在且 L1 用 mock emitter 驱动事件循环（Read 确认）；notify_watch 沙箱校验/pool 交互分支有用例
- **HFN-04**：fs 异常路径用例存在（删除不存在路径、create_dir/delete root 外拒绝、spawn_blocking panic → AppError 映射；方式不限）
- **HFN-05**：claude_history 命令包装层最小用例存在（spawn_blocking/参数透传）+ IO 降级用例（不可读文件 → 降级条目/mtimeMs=0）
- **HFN-06**：`ScanRootGuard`（或等价 RAII）存在，scan.rs env 测试经 guard 设置/恢复（Read 确认无裸 `set_var`/`remove_var` 手动成对残留）
- **HFN-07**：notify Drop 测试改轮询等待 `thread.is_finished()`（2s 超时）——测试区固定 `sleep(100ms)` 后断言已消除（grep 确认）
- **HFN-08**：fs 测试区无 `transmute` 构造 `State<AppState>`（grep `transmute` 于 fs 测试区零命中；命令内核已抽纯函数或改安全构造）
- **HFN-09**：①pool p9 drop 测试含线程退出/stop 断言；②`scan_multiple_sessions_sorted_input_order` 已改名（不暗示顺序，grep 旧名零命中）；③ops 空串 UUID 用例改断言错误消息含具体校验文案（`msg.contains("")` 恒真断言零残留）
- **SPE-01**：settings 用例经 `block_on` 调真实 `save_settings`/`load_settings`（grep `block_on` 命中 settings.rs 测试区）；`app_data_dir()` 已可注入（测试注 tempdir）；备份恢复/浅合并不擦他段/原子写有用例
- **SPE-02**：projects 用例经 `block_on` 调 `save_projects`/`load_projects`（grep 命中）
- **SPE-03**：error.rs 三 From（serde_json/git2/tokio JoinError）各有用例（变体 + 消息契约断言）
- **SPE-04**：`app_data_dir()` 错误分支已可注入测试（current_exe 失败、exe 无父目录两分支，Read 确认存在可注入失败点）
- **SPE-05**：`NamedTempFile::persist` 失败 → AppError 映射用例存在（目标路径只读/冲突构造，方式不限）
- **SPE-06**：settings 边界用例（可行范围内：并发写/只读文件/超大 JSON 至少一例）；`app_data_dir` 依赖真实 current_exe 的测试有注释说明；lib.rs `run()` 维持豁免（→ DOC-01，本 Stage 无需动作）
- **禁区**：SLTERM_CLAUDE_PROJECTS_DIR env 测试恒在 `--test-threads=1` 下（门禁已保证）
- **门禁**：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿
- **门禁**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零警告

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
