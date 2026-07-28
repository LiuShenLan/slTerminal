# Stage 03 逐项验证断言（唯一真值源）

> stage-03 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 行号引用为修复前快照（checklist 实证 2026-07-28），修复后可能漂移——以符号名定位为准。

## 断言清单

- **V1（PF2-BE-01）**：`src-tauri/src/hooks/usage.rs` `ContextUsage` 含四字段且 serde camelCase——grep `cache_read_input_tokens` 与 `cacheReadInputTokens` 断言串均存在于文件；`#[serde(default)]` 作用于 cache 两字段（语义式：transcript 缺 cache 字段反序列化不报错且为 0，不限属性写法）。
- **V2（PF2-BE-01）**：`parse_usage_line` 提取 cache 两字段缺失 `unwrap_or(0)`；`input_tokens` 缺失仍整行 None（沿用现状）——Read 源码确认两分支。
- **V3（PF2-FE-11）**：`src/types/hooks.ts` `ContextUsage` 四字段必填；TS 字段名 `cacheReadInputTokens`/`cacheCreationInputTokens` 与 Rust serde 名**逐字符一致**（Read 双边对照）。
- **V4（PF2-FE-11）**：`src/features/agentStatus/AgentStatusRow.tsx` total 口径 = `inputTokens + cacheReadInputTokens + cacheCreationInputTokens`（Read 源码确认 `outputTokens` 不在总占用求和内，仅作信息展示）。
- **V5（PF2-DOC-01）**：`docs/hooks-dev/contract.md` C12 段含四字段定义 + 用量口径（`(input + cacheRead + cacheCreation) / 200_000`，output 不计占用保留为信息字段）+ 缺 cache 默认 0 兼容约定（Read 该段确认三点齐全）。
- **V6（PF2-TE-07）**：`src/__tests__/ipc-hooks-contract.test.ts` 含 ContextUsage 键集合精确匹配断言——`Object.keys(usage).sort()` 精确等于四字段（照 :240-268 HookEventPayload 先例，Read 断言确认）；原 :310 mockUsage 字面量含 cache 两字段。
- **V7（PF2-TE-02 cache 部分）**：`src/__tests__/agent-status-view.test.tsx` 用量断言按新口径——75% 用例 total = input + cacheRead + cacheCreation 推导值与断言一致（Read 该用例数值人工核算）；:266,341,351,362 区域字面量含 cache 字段。
- **V8（波及面）**：grep `inputTokens:` 全仓测试文件无 2 字段字面量残留——每个命中点 Read 确认均含 `cacheReadInputTokens`（含 `src/__tests__/agent-status-hook.test.ts` T7，Stage 01 产物接力补齐）。
- **V9（门禁）**：全量测试五命令全绿（见下）；`usage.rs` 新增 L1 用例存在——cache 提取/缺省 0/旧格式兼容/显式 0/serde 四字段 camelCase（Read `#[cfg(test)]` 逐条点名）；`parse_extra_fields_ignored`（原 :143-151）未被改动语义（仍用 `cache_read`/`cache_write` 字段名）。

## 全量测试（全部通过为门禁）

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
3. `npx tsc --noEmit`
4. `npx eslint src/`
5. `npm test`
