# Stage 17 逐项验证断言（唯一真值源）

> stage-17 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **SEC-05**：`src-tauri/src/hooks/claude/config.rs` 的 `agent_hooks_config_write` 存在三规则语义校验（语义式，须 Read 确认：① 事件名白名单——HOOK_EVENTS 10 事件；② handler type == "command"；③ command 非空字符串）；校验失败返回 `AppError::Validation`（grep 命中）
- **SEC-05**：新增 L1 用例存在（非法事件名/非法 type/空 command 拒绝、合法写入放行——grep 测试函数逐类确认）
- **SEC-05**：前端 `src/panels/hooksConfig/` 写入链路 user 层（layer==="user"）调 confirmDialog（grep 命中）；project/local 层不弹确认直接写（语义式，须 Read 确认分支条件）；新增 L2 用例存在（确认/取消/不弹三分支）
- **SEC-12**：`src-tauri/src/hooks/claude/inject.rs` 注入/重注入路径存在原 statusline 命令可疑模式审查（语义式：curl/wget/Invoke-Expression 等模式命中时 tracing::warn!——Read 确认**仅记录不阻断**，发现阻断写入判 not_fixed）
- **SEC-13**：inject.rs 状态检测存在 SHA-256 哈希比对（语义式：include_str! 内嵌模板 + 磁盘脚本哈希比对，不一致 → Outdated——Read 确认）；新增 L1 用例存在（首行保留的篡改脚本被检出 Outdated）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
