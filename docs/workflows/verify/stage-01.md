# Stage 1 逐项验证断言（唯一真值源）

> stage-01 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 覆盖清单 ID：SB-1~SB-13（docs/sidebar-checklist.md「Stage 1 — 状态层」）。

## 断言清单

- **SB-1**：`src/features/sideViews/sideBarState.ts` 存在，导出类型与常量与 `docs/sidebar-execution-plan.md` §2 契约逐字一致（Read 比对）：类型 `Zone` / `Zones` / `OpenState` / `LayoutKind` / `SideBarSlice`；grep 命中常量 `ACTIVITY_BAR_SIZE = 40`、`WIDTH_DEFAULT = 250`、`WIDTH_MIN = 160`、`WIDTH_MAX = 500`、`SPLIT_DEFAULT = 0.5`、`SPLIT_MIN = 0.1`、`SPLIT_MAX = 0.9`；Read 确认 `DEFAULT_ZONES = { top: ["projects", "explorer"], bottom: [] }`、`DEFAULT_OPEN = { top: "projects", bottom: null }`。
- **SB-2**：Read `toggleViewPure` 确认语义：先查 id 归属 zone；`open[zone] === id` → 该 zone 置 null；否则 `open[zone] = id`（覆盖即隐式关闭）；id 不在 zones 任一区 → 原样返回。**语义式（防字面通过）**：函数无任何"历史/前一个"态——返回值仅由入参推导，被替换视图不出现在返回的 OpenState 中。
- **SB-3**：Read `moveButtonPure` 确认语义：源区移除 id、index clamp 到 [0, len] 后插入目标区；`open[源区] === id` → `open[源区] = null` 且 `open[目标区] = id`；未打开则 open 不变；同区移动只调顺序不动 open；函数内不校验 id 是否已注册（注册表职责分离）。
- **SB-4**：Read 确认三个纯函数语义：`deriveLayout`——双空→`"hidden"`、仅上→`"single-top"`、仅下→`"single-bottom"`、双开→`"split"`；`reconcileZones`——过滤未注册 id、保留 saved 顺序、缺失的注册 id 追加上区末尾、open 指向 reconcile 后 zones 中不存在的 id → null；`sanitizeSideBar`——raw 非对象→全默认；`zones.top/bottom` 非 string[] → 默认；`open.top/bottom` 非 string|null → null；width/splitRatio 非 number → 默认否则 clamp；返回完整 SideBarSlice（无 partial）。
- **SB-5**：`src/features/sideViews/dropTarget.ts` 存在；Read `computeDropTarget` 确认语义：clientY 在按钮上半 → 该按钮 index，下半 → index+1；空白区 → 数组末尾。**纯函数**：grep `document|window|getBoundingClientRect` 在 `dropTarget.ts` 为零。
- **SB-6/SB-7**：`src/features/sideViews/sideViewRegistry.ts` 存在，Read 比对契约：`SideViewComponentProps = { switchToPage: (projectId: string, pageId: string) => void; onDeletePage: (projectId: string, pageId: string) => void }`；`SideViewDef = { id; title; icon; component: React.ComponentType<SideViewComponentProps> }`；模块级单例 `sideViewRegistry` 含 `register`（同 id 覆盖）/ `getAll`（注册序）/ `get` / `_reset`。
- **SB-8**：`src/stores/sideBar.ts` 存在，Read 确认 `SideBarState` 形状（`zones; open; width; splitRatio; loaded` + `toggleView; moveButton; setWidth; setSplitRatio; loadFromDisk`）；`toggleView`/`moveButton` 委托 `toggleViewPure`/`moveButtonPure`（import 自 `../features/sideViews/sideBarState`——Read import 语句确认）；`setWidth`/`setSplitRatio` 内部 clamp；默认值取自 SB-1 常量。
- **SB-9**：Read 确认持久化链：`loadFromDisk` = `loadSettings()` → `saved?.sideBar` → `sanitizeSideBar` → `reconcileZones(slice.zones, slice.open, sideViewRegistry.getAll().map(d => d.id))` → set + `loaded: true`，try/catch 保默认；`subscribe` + `loaded` 守卫 + `PERSIST_DEBOUNCE_MS`（import 自 `./projects`）2s debounce → `saveSettings(...)`；导出 `cancelPendingSave()`。**键集合精确匹配**：debounce 保存 payload 的 top-level 键集合恰为 `{ "sideBar" }` 一键，且 `sideBar` 子键恰为 `{ zones, open, width, splitRatio }`（Read 确认，不接受多写/少写键）。
- **SB-10**：grep `sideBar` 命中 `src/stores/index.ts`（re-export `useSideBar` + 类型）。
- **SB-11**：`src/__tests__/sideBarState.test.ts` 存在且全绿；覆盖 toggleViewPure 4 分支 / moveButtonPure 8 分支（跨区跟随替换、跟随空区、未打开仅归属、区内前移、后移、同位、index clamp）/ deriveLayout 4 态 / reconcileZones 5 分支 / sanitizeSideBar 6 分支 / computeDropTarget 用例；**S1–S6 场景序列 6 条**（语义式：连续调用纯函数断言终态，直译需求 §5 验收场景——grep `S1`…`S6` 标记或等价 describe/it 文案命中 6 处）。
- **SB-12**：`src/__tests__/sideViewRegistry.test.ts` 存在且全绿；覆盖 register/getAll/get、重复注册覆盖、未注册 get→undefined、`_reset` 隔离。
- **SB-13**：`src/__tests__/sideBar.test.ts` 存在且全绿；覆盖默认值、toggle/move 经 store、loadFromDisk 合法/脏数据 sanitize/缺失/异常降级、loaded 守卫防启动空写、fake timers 验证 debounce → `saveSettings({ sideBar })` payload 键集合精确匹配（mock `../ipc/settings`）。
- **ARCH-1（硬约束 #1）**：grep `invoke|@tauri-apps/api` 在 `src/stores/sideBar.ts` 与 `src/features/sideViews/` 全部文件为零（前端不碰 OS，仅经 `../ipc/settings`）。
- **REG-1（存量零回归）**：`npm test` 全量绿，含全部存量套件。

## 全量测试（全部通过为门禁）

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
