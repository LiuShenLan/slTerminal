# Stage 13 逐项验证断言（唯一真值源）

> stage-13 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **IHE-01**：`src/ipc/CLAUDE.md` 含 mockIPC 盲区说明段（Read 确认"契约测试只防 wrapper 写错命令名/参数结构，真实序列化由 L4 守卫"字样）；契约测试含 listen 回调解包行为契约用例（模拟驱动断言 event.payload 解包，非仅 mockIPC 拦截）
- **IHE-02**：`notification.test.ts` 存在（Glob 命中）且含拒绝/异常分支用例（sendToastNotification catch 静默、ensureNotificationPermission 拒绝路径）
- **IHE-03**：html-panel 测试含 origin/source/type/fingerprint 四负面用例（origin≠"null" 不 dispatch、source≠contentWindow 不 dispatch、type≠"slterm_key" 不 dispatch、未知 fingerprint 不 dispatch）+ jsdom 局限标注（Read 确认）
- **IHE-04**：e2e-enabled 或 e2e-build-config 测试含 `E2E_ENABLED` 字面量表达式断言（AST/正则：定义为内联 `import.meta.env` 表达式，不调用 computeE2eEnabled）
- **IHE-05**：error-boundary 测试含 `variant="inline"` 渲染用例
- **IHE-06**：四 IPC 契约文件经共享工厂（`src/__tests__/helpers/ipc-contract.ts` 存在且被 contract/hooks-contract/hooks-config-contract/claude-history-contract 四文件 import，grep 确认）；四维断言（命令名/参数/返回/异常）不丢
- **IHE-07**：①ipc-ping 改调 `src/ipc/index.ts` 导出的 `ping()`（grep 确认非裸 invoke）；②html-panel 注入脚本断言关键控制流（postMessage 字段构造/preventDefault/监听绑定，非仅字符串包含）；③HtmlPanel `err instanceof Error` false 分支复跑确认命中（未中则修用例）；④csp-config 扩展 style-src/connect-src/img-src 关键字段快照
- **IHE-08**：html-panel 测试提取 `waitForLoaded`/`waitForError` 局部 helper（重复 waitFor 模式消除，Read 抽查）
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
