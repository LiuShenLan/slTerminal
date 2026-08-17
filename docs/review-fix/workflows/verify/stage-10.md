# Stage 10 逐项验证断言（唯一真值源）

> stage-10 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

## 断言清单

- **FE-12**：`src/types/fs.ts` `DirEntry` 的 `size` 与 `modified` 均为 `number | null`（Read 确认，无 `?:` 可选形态）
- **FE-12**：DirEntry 消费方完成 null 适配（`npx tsc --noEmit` 全绿为兜底证据；另 grep `\.size\b|\.modified\b` 于 explorer 排序/显示、测试工厂抽查无 `?? 0` 之外的裸用判 partial）
- **FE-13**：`src/types/notify.ts` `FsEventPayload.detail` 无 `?`（Read 确认必填）
- **FE-14**：`src/types/hooksConfig.ts` `HooksLayer` 为 `"user" | "project" | "local"` 字面量联合（Read 确认）
- **FE-14**：`src/ipc/pty.ts` spawn wrapper 存在 cols/rows 范围校验（语义式：1..=32767，越界抛错不 invoke——Read 确认校验在 invoke 之前）
- **FE-14**：`src/types/pty.ts`、`agentHistory.ts`、`agent.ts` 中 u64 对应字段含安全整数范围注释（grep 「2^53」或「安全整数」命中）
- **BE-18**：`src-tauri/src/hooks/claude/config.rs` 存在 `Layer` 枚举（User/Project/Local，serde snake_case）且 `parse_layer` 返回该枚举（Read 确认）；hooks 子树结构体存在（serde 反序列化形态校验，不含 SEC-05 校验规则——S17 才加）
- **BE-18**：新增 L1 用例存在（Layer 序列化/反序列化、非法 layer 拒绝）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
