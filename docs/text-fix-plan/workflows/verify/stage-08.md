# Stage 08 逐项验证断言（唯一真值源）

> stage-08 脚本与 fix-loop 的 verify agent 均以本文件为准。
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

- **WRK-01**：PageDockviewHost 真实组件用例存在——DefaultTab 渲染生产组件（tabIcon emoji/img 两分支 + `event.tabIcon` 事件结构，非手写 Mock）、Watermark 按钮 addPanel、RightHeader、handleReady 空布局不兜底创建终端、onSaveAs 重算标题（Read 确认各行为有测试触及）
- **WRK-02**：`switchToPageShared` 用例含 await 顺序断言（spy invocationCallOrder：setProjectRoot 完成先于 setActivePage）+ `__dockviewApi` 重指断言；`switchToPageAndFocus` 轮询命中与超时降级各有用例
- **WRK-03**：启动恢复用例含 `setProjectRoot` 先于 `setActivePage` 顺序断言（spy invocationCallOrder 或等价）；requestUserAttention reject 静默 catch 有用例
- **WRK-04**：`src/ipc/window.ts` 的 `onFocusChanged`/`setFocus` 处置三选一已落实：删除 / 标注预留（注释）/ 补最小契约测试（四维：命令名/参数/返回/异常）——ipc/window.ts 与测试一致（Read 确认三选一落地）
- **WRK-05**：workspace-defaulttab 测试改渲染生产 DefaultTab（Read 确认无手写 MockDefaultTab 替代生产组件）；断言 params 变化 → 图标切换
- **WRK-06**：workspace-switch-order 真实驱动 `Workspace.switchToPage`/`switchToPageShared` 断言顺序（Read 确认非手动模拟 mock 调用序）；3000ms 超时已收敛
- **WRK-07**：layout-serde mock 的合法面板类型与真实 `PANEL_TYPES` 一致（Read 对照 panelRegistry.ts，6 种全量或断言 mock 与真实一致）；gitshow/diff/hooksConfig 白名单过滤有用例
- **WRK-08**：close-handler 测试含阻止默认关闭断言（preventDefault 或等价；Read 确认）
- **WRK-09**：multi-instance 用例含实例 identity 断言（同一 api 对象跨切换存活，非仅 CSS display；Read 确认）
- **WRK-10**：main.tsx bootstrap catch 分支有用例（init 失败不白屏/错误展示）
- **WRK-11**：①titleManager/layoutSerde 残留容错行已补测或标注；②default-layout-format 补"SidebarTree 实际使用 makeEmptyLayout"断言；③FILE_PANEL_TYPES 重复断言合并为单点（panel-registry 与 workspace-file-panel-types 不重复断言同一集合，Read 确认）
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
