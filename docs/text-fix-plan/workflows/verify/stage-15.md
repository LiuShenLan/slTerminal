# Stage 15 逐项验证断言（唯一真值源）

> stage-15 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **E2E-01**：`keyboard.test.ts` 含降级标注（文件头或 describe："xterm.js 基础行为回归（非 slTerminal 键盘链路）"字样，Read 确认）；用例保留不删
- **E2E-02**：存在用生产 `terminalOptions`（import `src/panels/terminal/theme.ts`）创建 headless Terminal 的用例（grep `theme` import 命中测试文件）；断言含：16 色 ANSI 与主题色板一致、`CSI>1u` 可激活 Kitty、scrollback 容量生效、drawBoldTextInBrightColors 亮色映射
- **E2E-03**：OSC 52/133/8 三 handler 各有触发+断言用例——①`\x1b]52;c;<base64>\x07` → mock `src/ipc/clipboard` writeText 被调 + CJK 解码正确；②`\x1b]133;C;<cmd>\x1b\\` → onTabStateChange 参数（icon/title）；③`\x1b]8;;<url>\x1b\\` → mock `src/ipc/shell` openUrl
- **E2E-07**：CUP/reflow/SGR 用例含行列精确断言（`getLine(y).translateToString()` 或 `getCell(x,y).getFgColorMode()`；Read 抽查 ≥2 处非 `toContain` 存在性断言）
- **E2E-08**：256 色用例按 SerializeAddon.ts:259-262 实际优化行为断言（palette 0-15 → 基本 SGR `\x1b[30m`/`\x1b[97m` 等优化后序列；grep `\x1b[3` / `\x1b[9` 命中）；误导注释已修正（Read 确认注释与断言一致，无自矛盾）
- **E2E-14**：L3 负面用例存在（非法 ANSI、截断多字节序列、嵌套 OSC、异常 resize 0×0；headless 不崩溃 + 状态可恢复，Read 确认）
- **门禁**：`npx tsc --noEmit` + `npm run test:l3` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npm run test:l3`
