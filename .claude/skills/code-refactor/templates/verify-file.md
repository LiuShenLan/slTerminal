# Stage __STAGE_NUMBER__ 逐项验证断言（唯一真值源）

> stage-__STAGE_NUMBER__ 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式（见下方 FE-00 示例），防"改名迎合"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
-->

## 断言清单

- **ID-01**：断言内容（含证据要求，如：grep `pattern` 命中 `file:line`；Read 确认某函数无某调用）
- **ID-02**：断言内容
<!-- 语义式断言示例（"禁止存在 X"类，防改名迎合）：
- **FE-00**：`someFile.ts` 中不存在**任何**本地 XX 存储（不限变量名——`xxRef`/`xxId` 变量等均属违规）；所有读取处全部经 `Registry.get(id)` 获取（须 Read 代码确认，非仅 grep）
-->

## 全量测试（全部通过为门禁）

__TEST_COMMANDS__
<!-- 示例：
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
（有 Stage 特殊要求时补充，如 Stage 含 L3 变更时加 `npm run test:l3`）
-->
