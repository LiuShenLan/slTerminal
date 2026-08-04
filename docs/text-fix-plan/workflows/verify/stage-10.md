# Stage 10 逐项验证断言（唯一真值源）

> stage-10 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式，防"改名迎合"；正向意图断言同样写语义式，防"字面通过"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 tsc/eslint 覆盖外文件时补 npx vite build 构建级兜底
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态（commit-view 拆分后计数按新文件推导）
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令
-->

## 断言清单

- **SVC-01**：activityBar 全部 drop 用例含第三参数 index 断言（Read 抽查 ≥3 处 `mock.calls[0][2]` 或 `toHaveBeenCalledWith(..., zone, index)` 形式）
- **SVC-02**：sideBar/fontSize/keybindings 三 store 各有 cancelPendingSave 活跃 timer 取消用例（变更产生 timer → cancelPendingSave → 推进 2s → saveSettings/save_projects 未再调用）
- **SVC-03**：useCommitStatus 去抖与清理有用例（连续 3 次 fs-event 仅 1 次 gitStatus；激活 timer 后 unmount 断言 clearTimeout）
- **SVC-04**：openCommitFile 四守卫路径各有用例（未知 status 不调 addPanel、project 无 rootPath return、addPanel throw 不抛到外层、recomputeTitles 更新标题断言）
- **SVC-05**：resolveTargetZone 含中点恰好值（→bottom）与中点 -1（→top）两边界用例
- **SVC-06**：moveButtonPure R7 目标区非空场景有用例（跨区拖拽未打开视图且目标区已有打开视图 → 仅 zones 归属变化，open 不动）
- **SVC-07**：SideBarArea total<=0 除零守卫有用例（sizes=[0,0] 等输入不 NaN 不崩溃、store 不被写入）
- **SVC-08**：CommitFileList 右键菜单项 hover 断言 + renamed 无 oldPath 时 `params.oldPath` 为 undefined 用例
- **SVC-09**：commitContextMenu 删除 catch 有用例（gitUnstage/deleteEntry reject → console.error 或静默，菜单 action 不抛）
- **SVC-10**：workspace-sideviews props 断言改引用断言（SideBarArea 收到的 switchToPage/onDeletePage 与 Workspace 传入为同一函数，`typeof` 弱断言零残留）
- **SVC-11**：B10 反向用例改经 `openCommitFile` 驱动（同文件不同 suffix 不误聚焦，verify 经 commit 分派路径而非直测 titleManager，Read 确认）
- **SVC-12**：commit 测试中 fake timers 与 waitFor 混用消除（Read 确认 rootPath 切换用例统一计时策略）
- **SVC-13**：sanitizeSideBar/clamp 的 NaN/Infinity/-Infinity 回退 min 有用例
- **SVC-14**：`commit-view.test.tsx` 已拆分（原文件 ≤200 行或不存在，新文件落位：状态机/分派去重/右键菜单；Glob 确认）
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test` 全绿（测试类断言据此判定）

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
