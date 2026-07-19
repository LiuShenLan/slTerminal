# Stage __STAGE_NUMBER__ 逐项验证断言（唯一真值源）

> stage-__STAGE_NUMBER__ 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式（见下方 FE-00 示例），防"改名迎合"；正向意图断言同样写语义式（见 DBG-00 示例），防"字面通过"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 tsc/eslint 覆盖外文件时补 `npx vite build` 构建级兜底
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态（实证：Stage 03 后四类型，断言误写"五类型"）
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令（实证：要 cargo 统计行但门禁不跑 cargo）
-->

## 断言清单

- **ID-01**：断言内容（含证据要求，如：grep `pattern` 命中 `file:line`；Read 确认某函数无某调用）
- **ID-02**：断言内容
<!-- 语义式断言示例（"禁止存在 X"类，防改名迎合）：
- **FE-00**：`someFile.ts` 中不存在**任何**本地 XX 存储（不限变量名——`xxRef`/`xxId` 变量等均属违规）；所有读取处全部经 `Registry.get(id)` 获取（须 Read 代码确认，非仅 grep）
-->
<!-- 正向意图断言同样写语义式（防字面通过）：
- **DBG-00**：`write()` 的 panelId 必须来自 TerminalRegistry 的 Map 键，不接受字面量/硬编码（须 Read 调用点确认取值来源）
- **DBG-00**：不存在任何未前置 `await setProjectRoot` 的 `setActivePage` 调用点（含 e2e-tests/helpers.ts，须 Grep 全仓逐处确认）
- **DBG-00**：`src/ipc/CLAUDE.md` 的 wrapper 签名描述与 `src/ipc/pty.ts` 当前代码一致（文档类断言须对照真实代码核实，防文档撒谎）
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
