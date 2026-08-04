# Stage 09 逐项验证断言（唯一真值源）

> stage-09 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **EXP-01**：OpenInTerminal 用例存在且断言 addPanel 参数：`component:"terminal"`、`params.cwd` 正确（文件取父目录/目录取自身）、panelId 前缀 `terminal-` 含 pageId、`renderer:"always"`
- **EXP-02**：删除/重命名/新建（文件+文件夹）CRUD 各有成功路径断言（IPC 调用 + `refresh()` 触发（readDir 二次调用）+ `setRenamingPath(null)`/`setNewFileName(null)` 状态重置；Read 确认成功分支被断言）
- **EXP-03**：`fullRefresh` 处置落实（死代码删除或接线+用例，二选一）；F8 用例名实一致（改名"初始加载调用 gitStatus"或重写为真实 fullRefresh 驱动）
- **EXP-04**：focusin→pushContext+setActiveExplorer、focusout→popContext+clearActiveExplorer 链路有用例；hover 高亮（非选中态）断言；错误横幅 dismiss（3s 自动消失 fake timers + 卸载清理）断言
- **EXP-05**：FileIcon 表驱动覆盖 `.pyw`/`.markdown`/`.less`/`.scss`/`.gitattributes`（各断言返回对应图标或文本非空）
- **EXP-06**：FileTree 三种内联输入（rename/newFile/newFolder）的 Escape 取消、blur 空值、onChange 边界各有用例
- **EXP-07**：useFileTree 竞态清理分支有用例（rootPath=null 时旧请求延迟 resolve 被丢弃不抛错、卸载后 fs-event 回调去抖 timer 被清理、file-saved 事件缺 path 仍刷新）
- **EXP-08**：SidebarTree 错误降级分支有用例（dialog.open reject/返回数组/返回 null → store 不变不抛错；proj 不存在点击项目行菜单不抛错）
- **EXP-09**：SidebarTree hover 样式与重命名中点击行 `stopPropagation`（不触发 switchToPage）有用例
- **EXP-10**：handleOpenFile 防御分支有用例（无 activePageId 不调 addPanel、无 `__dockviewApi` 不抛错、重复打开去重聚焦）
- **EXP-11**：E6 标题与断言一致（标题改"deleteSelected 被调用但内部不执行删除"或断言 handler 返回 false，二选一）；explorer 测试用例编号无重复
- **EXP-12**：FileViewerRegistry `_reset` 用例存在 + `_reset` 后预注册恢复（或 per-test 新实例）；`resolve(".gitignore")`/`resolve("file.")` 边界有用例
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
