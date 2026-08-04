# Stage 14 逐项验证断言（唯一真值源）

> stage-14 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **NAH-01**：claude-history-model 含 `sessionId: null` 回退用例（注册表条目 `{sessionId:null, transcriptPath:"C:/x/abc.jsonl", status:"working"}` → `deriveActiveSessionStatuses().get("abc")==="working"`）
- **NAH-03**：classifyEvent 导出（D2 最小可测性重构，Read 确认导出）且表驱动覆盖全分支：PermissionRequest→permission、Notification 两型（permission_prompt→permission / 其他→null 或对应类别）、Stop→done、StopFailure/PostToolUseFailure→error、未识别→null；断言返回类别 + toast 触发与否 + 标题/正文
- **NAH-04**：通知去重缓存截断用例存在（构造 >200 事件 → 截断为 100 → 最旧事件重新触发再弹 toast）
- **NAH-05**：AgentStatusRow 行 2 断言存在（完整 usage 行渲染：outputTokens 文本 + formatRelativeTime 相对时间出现）
- **NAH-06**：AgentStatusView 标题覆盖集成用例（真实 useClaudeHistory 或受控数据含 rename 后 title → 活跃区行标题被覆盖；无匹配 sessionId 回退原标题，Read 确认非纯 mock useClaudeHistory）
- **NAH-07**：restoreSession 防重入用例（同步连调两次仅执行一次四步编排）+ cwd null 抛 "cwd required" 用例
- **NAH-08**：useClaudeHistory.scan generation 竞态用例（首次 scan 延迟 resolve + 二次立即 resolve → sessions 来自第二次）
- **NAH-09**：HistorySessionList ①expandedGroups 初始为空、点击组标题后含该组 key 断言；②右键触发断言 onCopy/onFork/onDelete 回调参数（fork 标志 true）
- **NAH-10**：HistorySessionRow `status="working" && orphan=true` 渲染 ⚡ 而非 ✗ 用例
- **NAH-11**：SessionActionDialog `actions={[]}` 不渲染弹窗用例
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
