# Stage 07 逐项验证断言（唯一真值源）

> stage-07 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **EDF-01**：diff-panel 保存链用例真实触发保存（调 handler 或 dispatch），断言 `fs.writeFile` 被调 → `gitDiff` 重调 → 双侧 gutter/占位刷新（非 toBeDefined；Read 确认断言链完整）
- **EDF-02**：DiffPanel 五分支各有用例——refreshPlaceholders 占位刷新同步（含空/有值两路）、左侧 `.git` 变更重取 HEAD、外部修改脏确认弹窗（dirty=true 分支）、滚动同步重绑定（state.kind/headContent 变化后 effect 重绑）、大文件阈值（Read 确认各分支被测试触及）
- **EDF-03**：useCodeMirror 大文件三分支有用例（>10MB 拒绝且 filePathRef 清空、>1MB confirm 取消、`fs.writeFile` reject → alert 且不派发保存事件；mock fs + dialog）
- **EDF-04**：gitshow 大文件警告 header 精确断言（文案出现）；params.filePath 切换用例断言 EditorView 实例 identity 变化或销毁/创建计数（可区分新旧 view）
- **EDF-05**：gitGutter 四 wrapper（updateDiffGutter/clearDiffGutter/updateHeadDiffGutter/clearHeadDiffGutter）各有直接调用用例（真实 EditorView + StateEffect dispatch，Read 确认非仅 buildRangeSet 层）
- **EDF-06**：alignment `key < 0` 过滤分支有用例（newStart=0 输入）
- **EDF-07**：diff 测试中固定 `200ms` 延时零残留（grep `setTimeout(r, 200)` 或等价 `200)` 于 diff-panel.test.tsx 零命中；改 fake timers 或轮询断言）
- **EDF-08**：`justSavedRef` Set 多实例语义有用例（双实例并存：A 保存 a.ts 不影响 B 对 a.ts 的自动重载，或等价隔离断言）
- **EDF-09**：gitshow 字号变化用例断言 `fontCompartment` reconfigure 触发（mock reconfigure 或捕获 dispatch，非仅 createEditorFontExtension 被调）
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
