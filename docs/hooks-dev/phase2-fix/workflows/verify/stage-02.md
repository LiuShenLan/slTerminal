# Stage 02 逐项验证断言（唯一真值源）

> stage-02 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 行号引用为修复前快照（checklist 实证 2026-07-28），修复后可能漂移——以符号名定位为准。

## 断言清单

- **V1（PF2-FE-08）**：`src/ipc/notification.ts` 导出 `sendToastNotification(title, options)` 且签名无第三参数（语义式：无任何形式的 onClick 参数，须 Read 签名确认）；实现内无 `new Notification(`（grep 零命中）；主路径为 Tauri 原生 `sendNotification`；失败 catch 含 `console.error`；`ensureNotificationPermission` 保留；文件内无「委托 OS 原生通知中心」旧注释（grep 零命中），存在 AUMID 平台限制结论注释（未打包 Win32 WebView2：banner 抑制/onclick 不路由/shim 无 close/构造不抛）。
- **V2（PF2-FE-09）**：`src/features/notifications/useClaudeNotifications.ts` 无 `routeToPanel`/`findPanelTitle` 定义与调用（grep 零命中；语义式：不存在"toast 点击后路由到面板"的代码路径，不限函数名——无 toast onClick → 面板聚焦/setFocus/switchToPageAndFocus 链路）；无 `sendClickableNotification` 引用（grep 零命中）；相关 import（setFocus/switchToPageAndFocus/getPageApi）如无其他使用已清理（Read import 段确认无未使用引用——eslint 全绿即证无 unused import）。
- **V3（PF2-FE-09）**：三类事件（permission/done/error，对应 classifyEvent 全分类）路径均调用 `requestUserAttention`（Read 源码确认闪烁调用不被事件类别条件排除——不存在"仅 permission 闪烁"的类别分支）。
- **V4（PF2-FE-09）**：失焦门控（`__slterm_windowFocused !== false` 才发送）与 60s 去重逻辑（seenRef）保留（Read 源码确认两处仍在）；toast body 不含面板标题查找结果（语义式：body 由项目名 + 事件类文案组成）。
- **V5（PF2-FE-10）**：flashTaskbar（原 :119-121 `requestUserAttention` 的 `.catch`）内已补 `console.error`（Read 该 catch 块确认）。
- **V6（PF2-TE-04 + 门禁）**：`src/__tests__/notifications.test.ts` 无 onClick 路由 describe（grep `onClick` 路由相关 describe/it 零命中——语义式：不存在"点击 toast 路由到面板"用例）；「任务栏闪烁细分」三类（Stop/StopFailure/PostToolUseFailure）均断言**触发** `requestUserAttention`（Read 断言确认，无「不触发」旧断言残留）；mock 为 `sendToastNotification`（两参数无 onClick）；失焦门控/60s 去重/classifyEvent 过滤/正文文案断言保留。全量测试三命令全绿（见下）。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`

## 人工验证点（不在本文件判定范围）

Win11 真实环境 Tauri 原生 `sendNotification` banner 可见性——收尾人工实测，不弹则接受退化（决策基线 1），结果写入 Stage 05 文档。
