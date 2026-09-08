# Review 02 · 后端架构与平台（CP-004/005/006/007/008/009/010/011/034）

## 问题清单

### 1. S11 页前缀协议与 S10 预览 label 字符集冲突——生产预览链路整体被 validate_label 拒绝
- **CP**: CP-004
- **位置**: `src-tauri/src/preview.rs:52-68`（validate_label）；`src/ipc/preview.ts:46-48`（makePreviewLabel 原样拼接）；`src/workspace/openFile.ts:99-112`（生产面板 id 带 ":"）；`src/panels/html/HtmlPanel.tsx:257`、`src/panels/markdown/MarkdownPanel.tsx`（`panelId={params.panelId}`）
- **问题**: S11 把全部面板 id 改为 `{pageId}:{localId}` 形态，而 S10 落地的 preview 窗口 label = `preview-<panelId>` 且 `validate_label` 字符集仅允许 ASCII 字母数字/`_`/`-`——":" 被硬拒。preview_sync / preview_close / preview_render（store_render_content）/ preview_pull（pull_stored_content）四个命令入口全部先过 validate_label（preview.rs:290、345、386、417），生产路径上 docViewer 预览的建窗、内容存储、拉取全部 `Err(Validation)`——S10 交付的独立 webview 预览在生产中整体不可用。
- **证据**: 链路逐跳核实——openFile.ts:100 `panelIdInPage(activePageId, localId)` 产出含 ":" 的 id 并经 :112 `params: { panelId }` 入面板；HtmlPanel.tsx:257 把该 params.panelId 原样传 PreviewFrame；preview.ts:46 `makePreviewLabel` 无安全化直接拼 label；preview.rs:61-64 `rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')` 对 ":" 返回 false。e2e 未暴露该缺陷：html.e2e.ts:329-338 用裸 id（`e2e-html-close-${Date.now()}`，无页前缀）经 `__dockviewApi.addPanel` 直注面板，label 天然无 ":"（2bde7b8 commit body 亦自认「无页前缀面板（e2e 裸 id/防御形态）」），L4 绿掩盖生产形态全灭。
- **建议**: label 构造单点加安全化（":" → "_" 或同款 `[^a-zA-Z0-9_-]` 替换，与 writeSignalFile/slterm-hook-reporter.js 同形态），或放宽 validate_label 收 ":"；补一条生产形态（页前缀 id）的 L1/L2 契约用例防再回归。

### 2. 页内分屏能力随修复消亡，未在销项注记登记为新妥协
- **CP**: CP-004
- **位置**: `src/workspace/CLAUDE.md`「共享宿主 + 页组模型」节（「页内分屏（单页多组）不可用——页 = 单组多页签，拖拽拆分产物被回迁守卫清理」）；`src/workspace/WorkspaceDockHost.tsx:91-116`（enforcePanelGroupMembership）
- **问题**: 旧多实例架构下每页持有完整 Dockview 实例，页内分屏（同页多组分屏）是原生能力；共享宿主改造后页 = 单一顶级组，任何拖拽拆分产物（dockview 自生非 page- 组）都被回迁守卫逐回，页内分屏在用户侧彻底不可用。该能力损失仅在 workspace/CLAUDE.md 作为设计约束记录，compromises.md CP-004 销项注记（「多实例架构取代、MAX_PAGES 消亡」）未披露此项功能性回退，不构成登记在册的新妥协。
- **证据**: WorkspaceDockHost.tsx:96 `panelBelongsToGroup` 对非 page- 组恒 false → :109 `moveTo` 回迁；workspace/CLAUDE.md 明文「页内分屏（单页多组）不可用」。compromises.md:34 CP-004 销项注记无对应字样。
- **建议**: 在 compromises.md CP-004 销项注记补登「页内分屏能力消亡」为修复引入的新形态（或产品决策确认后登记豁免）。

### 3. CP-011 销项注记「无裸 join 无界阻塞路径」与 checklist 验证命令均与实仓不符
- **CP**: CP-011
- **位置**: `src-tauri/src/notify/mod.rs:172`、`src-tauri/src/notify/mod.rs:305`、`src-tauri/src/hooks/watcher.rs:137`；checklist.md CP-011 验证节（:851）
- **问题**: compromises.md CP-011 销项称「无裸 join 无界阻塞路径」，checklist 验证节写明 `rg "handle\.join\(\)|\.join\(\)\s*;" src-tauri/src` 零命中（仅余 join_with_timeout 内部）。实跑该命令命中 6 处，其中 3 处为生产代码裸 join（notify stop、notify Drop、hooks watcher stop——先发停止信号后 `handle.join()`，结构上仍无超时保护）。PTY kill 链本身确已全链带超时（state.rs Drop、spawn.rs pty_kill/pty_kill_all 均走 join_with_timeout，实测代码属实），但登记措辞未限定范围、验证命令按字面执行恒非零命中。
- **证据**: `rg "handle\.join\(\)" src-tauri/src` 输出：notify/mod.rs:172（`pub fn stop` 生产）、notify/mod.rs:305（`impl Drop` 生产）、hooks/watcher.rs:137（`pub fn stop` 生产）、pty/reader.rs:179（join_with_timeout 内部，允许）、notify/mod.rs:968 与 hooks/watcher.rs:448（测试代码）。
- **建议**: 销项注记与 checklist 验证节措辞限定为「PTY 销毁链」范围；notify/hooks 三处裸 join 若要全仓一致纪律，另立小项换装 join_with_timeout（或登记豁免）。

### 4. fs_read_dir 续页失败时 catch 清空已渲染首帧
- **CP**: CP-006
- **位置**: `src/features/explorer/useFileTree.ts:148-180`
- **问题**: `loadRoot` 用单一 try 包裹首帧拉取与 while 续页循环；任一续页 reject 落入同一 catch，除记录 dirErrors 外无条件 `setRootNodes([])`（:179）——已渲染并可能被用户交互的首帧 500 条被清空。注释（:171）自称「首帧失败 → 错误占位；续页失败 → 可重试」，但续页失败实际与首帧失败同路径清空，「可重试」语义不成立（无保留首帧的降级）。
- **证据**: useFileTree.ts:149 首帧 `readDirPage(rp)`、:158 `setRootNodes(toTreeNodes(first.entries))` 已落地首帧；:161-169 while 续页同处 try 内；:170-180 catch 无区分分支统一 `setRootNodes([])`。`src/__tests__/use-file-tree.test.ts` 仅覆盖 P1/P2/F3-8（拼接、gen 丢弃），无续页失败用例钉住该行为。
- **建议**: catch 前区分首帧/续页（续页失败保留已渲染首帧 + 记录错误），补续页失败用例。

### 5. 偏移游标遇目录中途增删可跨页重复/遗漏，契约文档未登记该边界
- **CP**: CP-006
- **位置**: `src-tauri/src/fs/mod.rs:523-580`；`src/features/explorer/useFileTree.ts:95-108`（collectDirEntries 聚合）
- **问题**: 游标 = 排序后 offset（base64），每次分页调用重新 read_dir 全量+排序+切片。目录在分页间隙新增条目时 offset 移位 → 续页拼接出现重复或遗漏；目录缩水时已有「游标越界 → 空页 + null」登记，但「目录变长移位」面未登记。属偏移分页固有特性，但销项注记声称「排序过滤后切片跨页稳定」无此条件限定，前端聚合层也无去重/存在性守卫。
- **证据**: fs/mod.rs:566-580 排序后 `entries[start..end]` 切片，每调用独立快照；useFileTree.ts:104 `all.push(...page.entries)` 裸拼接；fs/CLAUDE.md 仅登记「游标越界（目录已缩水）→ 空页 + null」。
- **建议**: fs/CLAUDE.md 契约节补登「分页间隙目录变长 → 续页可能重复/遗漏」边界（或游标改为按条目名 keyset 以根治），前端聚合按需去重。

### 6. CP-005 grep 守卫自匹配——按字面执行的守卫命令恒非零命中
- **CP**: CP-005
- **位置**: `src-tauri/src/CLAUDE.md:53`；checklist.md CP-005 验证节（:674）
- **问题**: 换装后全仓代码 `std::sync::(Mutex|RwLock)` 确零命中，但守卫描述行自身含该字面量（「禁止再引入 `std::sync::Mutex/RwLock`（grep 守卫）」），`rg "std::sync::(Mutex|RwLock)" src-tauri/` 唯一命中即该文档行——checklist 验证节声称的「零命中（退出码 1）」按字面永不成立，CLAUDE.md 自述的「grep 守卫」无法作为可机检门禁直接执行。
- **证据**: 实跑 `rg "std::sync::(Mutex|RwLock)" src-tauri/` → 唯一命中 src-tauri/src/CLAUDE.md:53（exit=0）；代码面（*.rs）零命中。
- **建议**: 守卫命令收敛为 `rg "std::sync::(Mutex|RwLock)" src-tauri/src -g '*.rs'`，或 CLAUDE.md 描述行改写避开字面模式（如 `std::sync::Mutex` 拆写）。

### 7. 宿主 onReady 的 disposables 清理闭包永不执行
- **CP**: CP-004
- **位置**: `src/workspace/WorkspaceDockHost.tsx:207-265`
- **问题**: handleReady 内组装的 `disposables` 数组（含 :259-265 自定义清理：apiRef 置空、`window.__dockviewApi` 置 undefined、unregisterHostApi）无任何调用 dispose 的路径——DockviewReact `onReady` 是一次性事件回调，返回值不作 cleanup 消费。api 事件订阅随 api dispose 自动释放，实际影响仅限自定义清理为死路径：宿主 React 卸载（测试重挂载场景）后 hostApi 注册表与 `__dockviewApi` 残留旧引用。生产宿主终身挂载不受影响。
- **证据**: WorkspaceDockHost.tsx:259-265 push 自定义 disposable；全文件无 `disposables.forEach(d => d.dispose())` 或等价调用（rg 核实仅 6 处 push 无一处消费）。
- **建议**: 自定义清理移入组件 unmount useEffect（或显式注释登记「仅 api dispose 联动，自定义项不消费」）。

### 8. git_status 命令层残留 ignored 字样的陈旧注释
- **CP**: CP-008
- **位置**: `src-tauri/src/git/mod.rs:185`
- **问题**: CP-008 删除 is_ignored 死分支并同步了 GitStatusEntry.status 值集注释（:33 已无 ignored），但 :185 路径语义注释仍列「非 renamed（modified/added/untracked/conflict/ignored）」——ignored 已永不进入 git_status 结果（同文件 :55 注释自证），该列举失实，属死分支清理的注释残留。
- **证据**: git/mod.rs:185「// - 非 renamed（modified/added/untracked/conflict/ignored）：」对照 :53-55「`IGNORED` 落入 None：include_ignored 恒关，永不置位（CP-008 删死分支）」。
- **建议**: :185 注释删 `ignored`。

### 9. PageDockviewHost.tsx 名实不符
- **CP**: CP-004
- **位置**: `src/workspace/PageDockviewHost.tsx:1-11`（574 行）
- **问题**: 原「每页一 Dockview 实例宿主组件」已消亡，文件头注释自述本文件只保留 DefaultTab/Watermark/RightHeader/页签菜单等共享件——文件名仍叫 PageDockviewHost（"Host"），名实不符；checklist CP-004 步骤 1 给的方向是「改造为页组渲染组件或删除」，实际两未取，旧名承载无关职责，后续检索/渐进披露易误导。
- **建议**: 改名（如 `tabChrome.ts`/`sharedTabComponents.tsx`）或并入 WorkspaceDockHost。

## 界外观察

- `src-tauri/src/notify/mod.rs:968` `self.handle.join().unwrap()` 与 `src-tauri/src/hooks/watcher.rs:448` 裸 join 位于 `#[cfg(test)]` 模块内，生产零影响（finding 3 已按范围区分，此处仅备查）。
- e2e-tests/CLAUDE.md:102 已如实改写 reset 语义（CP-004 口径），docs/stores 两侧 MAX_PAGES 消亡登记一致，无漂移。
