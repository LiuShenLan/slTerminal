# Stage 11 逐项验证断言（唯一真值源）

> stage-11 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **HKC-01**：JsonMode 用例含 linter 顺序身份断言（`linterCalls[0][0]` 为 jsonParseLinter、`linterCalls[1][0]` 为 jsonSchemaLinter，非仅 options）
- **HKC-02**：load() 竞态用例存在（先挂起旧层 read → 切层 → 新层 resolve → 旧层延迟 resolve 被丢弃，最终 configJson 为目标层数据）
- **HKC-03**：handleJsonChange 非法 JSON 用例存在（onChange 传非法文本 → configJson 保持原快照、保存按钮禁用、不崩溃）
- **HKC-04**：HandlerForm record/stringArray 清空删键用例存在（清空 args/headers 等 textarea → onChange 新对象无该键）
- **HKC-05**：GuiMode 删除选中项选中态重置用例存在（先选中 matcher 组/handler 再删除 → 详情区回退空态/HandlerForm 消失）
- **HKC-06**：EventTree 未知事件分组用例存在（未知 group 事件 → 渲染"未知事件"分组标题且含该行）
- **HKC-07**：handleUninstall 失败分支用例存在（uninstall reject → 错误提示出现、状态条不变）
- **HKC-08**：`hooks-config-schema.test.ts` 存在（Glob 命中）且直测 validateHooksJson 边界（合法/非法 handler type/缺必填 command/顶层数组/空对象合法）
- **HKC-09**：open-hooks-config-panel 无 focus 降级用例存在（getPanel 返回无 focus 对象 → 不抛错、addPanel 不再调用）
- **HKC-10**：三处展示断言存在（JsonMode schema hover 触发、注入状态条初始 "--"、MatcherTester placeholder 随事件变化）
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
