# Review2 · Stage 04 前端清理（FE-01/02/07/09/10/11）

> commit: 2ecd7cb；核验维度：保真度/实现质量/新引入问题；范围：Stage diff 面 + 触碰文件全量复查。
> 仅登记问题，不分严重度；已核验通过项不列出。

实跑证据：L2 定向 10 文件 272 例全绿（`npx vitest run use-file-tree panel-registry workspace-host-pages terminal-rename-apply workspace-defaulttab workspace-header-actions shortcuts html-panel markdown-panel`）；`npx knip --production` exit 0；`rg "PageDockviewHost" src/ knip.json` 零命中；`rg "exportContextBindings|ExportedBinding" src/` 零命中；`restoringRef.current = true` 2 命中（:474/:498）；`disposablesRef` 5 命中；`"settings"` 于 src/panelRegistry.ts 仅 :25 常量定义值 1 命中（映射表键 :75 为裸键不被该 pattern 命中，语义符合收窄预期）；`git log --follow` 证实 git mv 保历史（rename 检测 `.../{PageDockviewHost.tsx => tabChrome.tsx}`）。

## 问题清单

### R2-FE-01 · FE-01-2 测试断言弱于 checklist 写死规格——「恰好一次」未被锁住
- **关联修复项**: FE-01
- **位置**: src/__tests__/use-file-tree.test.ts:1037-1040
- **问题**: checklist FE-01 步骤 4 写死「onViewStateChange **恰好一次**且 expandedPaths 反映首帧」；实现仅 `toHaveBeenCalled()`（≥1）+ lastPayload 空集断言。恰好一次语义无任何断言承载，上呼两次的实现也能绿（见 R2-FE-02）。测试未锁住规格，属弱断言。
- **证据**: checklist.md:519「onViewStateChange 恰好一次」；测试文件 :1037-1040 仅 `expect(onViewStateChange).toHaveBeenCalled()`。
- **建议**: 补 `toHaveBeenCalledTimes(1)`（先修 R2-FE-02 的双提交再补，否则必红）。

### R2-FE-02 · 生产环境无快照路径 onViewStateChange 双提交（微任务先于心智渲染，test env 不可暴露）
- **关联修复项**: FE-01
- **位置**: src/features/explorer/useFileTree.ts:499-502（`.then(restoreExpanded)`）+ :525-527（rootNodes 渲染提交 effect）+ :466-468（无快照分支显式 commitViewState）
- **问题**: 生产（WebView2/Chromium，React 18 Scheduler 走 MessageChannel 宏任务）下时序为：loadRoot body 内 `setRootNodes(首帧)` → 函数返回 → promise resolve → `.then(restoreExpanded)` 入微任务队列**先于** React 渲染宏任务执行。restoreExpanded 无快照分支此时 `commitViewState()` 读到的是过期空树（rootNodesRef 未更新），上呼空集（第 1 次）；随后首帧渲染落定，commit effect 因 rootNodes 引用变化再触发 `commitViewState()` 上呼空集（第 2 次）。同 payload 双上呼——对注册表槽幂等无害，但与 checklist「恰好一次」契约相悖，且 L2（act 同步 flush 环境）结构性地无法暴露该时序。test env 与生产行为分叉。
- **证据**: useFileTree.ts:499-502（loadRoot(gen).then → restoreExpanded）；:160（首帧 setRootNodes 与函数返回间无 await，nextCursor null 时 while 跳过）；React 18 调度语义（微任务先于 MessageChannel 宏任务）。FE-01-2 测试（:1007-1041）在 act 环境绿且无法计数。
- **建议**: 无快照分支去掉显式 `commitViewState()`（首帧渲染落定的 commit effect 自然会补一次且读到真实树），或显式提交与渲染提交二选一收口。

### R2-FE-03 · restoringRef 抑制态无超时兜底——loadRoot 永不 settle 则提交永久抑制
- **关联修复项**: FE-01
- **位置**: src/features/explorer/useFileTree.ts:498（置位）/ :499-502（解除依赖 loadRoot settle）
- **问题**: FE-01 把抑制置位提前到 effect 同步阶段，全部解除点（restoreExpanded 无快照分支/队列耗尽/首帧失败 catch/本 effect 下次运行）都依赖 loadRoot 的 promise settle。readDirPage 无超时——Tauri 命令挂起（后端死锁/通道丢失）场景下加载窗口永不闭合，此后用户一切展开/折叠操作的上呼被永久短路，展开态槽位停摆。FE-01 前无此窗口故无该缺口；属新状态机的有界未覆盖边。
- **证据**: useFileTree.ts:498 置位；:499-502 解除仅在 `.then`；ipc/fs.ts readDirPage 为裸 invoke 无超时；catch 分支（:173-192）同样依赖 reject 到达。
- **建议**: 加载窗口加超时/代际兜底解除（如 loadRoot 挂起 N 秒后按 gen 校验解除抑制），或登记为已知有界风险。

### R2-FE-04 · rootPath effect 新注释解除点列举遗漏第 4 个解除点
- **关联修复项**: FE-01
- **位置**: src/features/explorer/useFileTree.ts:496-497
- **问题**: 新注释写「解除点：restoreExpanded 无快照分支 / 恢复队列耗尽 effect / 本 effect 下次运行」共 3 个，遗漏 FE-07 联动的第 4 个解除点——loadRoot catch 首帧失败分支（:182-184 `restoringRef.current = false`）。注释与真实状态机不完整对齐，后续维护按注释枚举解除点会漏路径。
- **证据**: useFileTree.ts:496-497 注释 3 解除点；:182-184 第 4 个解除点存在于 catch 首帧失败分支。
- **建议**: 注释补「loadRoot 首帧失败 catch」一项。

### R2-FE-05 · 计划文档 FE-02 路径错误：`src/panels/panelRegistry.ts` 不存在（存量）
- **关联修复项**: FE-02（存量 plan doc 问题，非本 commit 引入）
- **位置**: docs/compromises-fix-review-fix/workflows/verify/stage-04.md:11；docs/compromises-fix-review-fix/stages.md:169
- **问题**: 两处写 FE-02 目标文件为 `src/panels/panelRegistry.ts`，实际文件在 `src/panelRegistry.ts`（checklist.md:532 路径正确）。按字面执行 `rg "PANEL_SETTINGS" src/panels/panelRegistry.ts` 因路径不存在直接报错，FE-02 门禁断言不可原样执行（本 review 实跑时在正确路径上 3 命中通过）。verify/stage-04.md 为本 Stage 验证唯一真值源，路径失实削弱门禁可执行性。
- **证据**: 实跑 `rg -n "panelRegistry" docs/compromises-fix-review-fix/workflows/verify/stage-04.md` 命中 :11 错路径；`rg -n '"settings"' src/panelRegistry.ts` 实命中 :25；仓内不存在 src/panels/panelRegistry.ts（stat 与 fs 实证）。
- **建议**: 两处改回 `src/panelRegistry.ts`。

### R2-FE-06 · knip.json 追加 `docs/compromises-fix-review-fix/**` 整目录 ignore 宽于必要
- **关联修复项**: FE-10（连带处置）
- **位置**: knip.json:14-15
- **问题**: 为压 8 项 `docs/compromises-fix-review-fix/workflows/*.js` unused 红，ignore 从目录粒度整包排除 `docs/compromises-fix-review-fix/**`——该目录此后任何新增脚本均不再产生 knip 信号。最小化口径（对齐硬约束 #10 精神）应为仅列 workflows 子目录或 8 个具体文件。
- **证据**: 2ecd7cb knip.json diff（`"docs/compromises-fix-review-fix/**"` 入 ignore 数组）；commit body 自述红因 = 8 项 workflows/*.js。
- **建议**: 收窄为 `docs/compromises-fix-review-fix/workflows/*.js` 或逐文件登记。
