# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

设置中心（F11）把「配置钮 → 单一配置面板」从 hooks 专属升级为统一配置入口：`SettingsPageRegistry` 注册表承载配置页集合，`pages.ts` 集中注册全部配置页，`openSettings.ts` 是活动栏配置钮的公共编排，`dirtyRegistry.ts` 是页 dirty 真值源。不这样设计则每类配置都要开新面板类型、各自维护打开/持久化/dirty 链路——注册表形态把「新增配置页」收敛为注册一条（硬约束 #13 家族契约）。

## 关键约束与决策

### 注册表家族契约（硬约束 #13）

`SettingsPageRegistry` 是模块级单例：`register(page)` 同 id 幂等覆盖 / `getAll(group?)` 按 `order ?? 注册序` / `get(id)` / `_reset()`（仅测试，清空全部条目）。`getSettingsPageRegistry()` 惰性导出。配置页类型 `SettingsPage`（id/title/group/component/order），`SettingsPageProps` 是壳透传给页组件的通道（onDirtyChange 上报 + pageParams 槽 + onPageParamsChange patch）。

### 注册触发点（side-effect import）

`pages.ts` import 即注册全部配置页——生产注册触发点为 `SettingsPanel.tsx` 顶部 `import "../../features/settingsCenter/pages"`（import 链保持引用，禁止隐式初始化）。新增配置页 = pages.ts 追加 register 调用即可，壳零改动。测试在 beforeEach/afterEach 调 `_reset()` 保证用例隔离。

### openSettings 编排（活动栏配置钮入口）

`openSettings(settingsPageId?)` 照旧 openHooksConfigFromActivityBar 编排：目标项目 = 活跃页面所属项目优先，兜底第一个项目；目标页面 = 已有操作页面取 pages[0]，无 → 新建空布局页；`await switchToPageShared(pageId)`（内部完成 setProjectRoot 前置，DBG-5）→ `openSettingsPanel(pageId, settingsPageId)`。

**无项目 → `toast.show("warning", "请先创建项目")` + return（R1 修订）**：原编排静默 return 不可感知，但设置面板无 Dockview 宿主可挂（无项目 = 无页面），必须 toast 显式提示。面板打开失败（页面 DockviewApi 5s 未就绪）由 openSettingsPanel 内部 console.warn 降级，本函数 fire-and-forget 不抛异常。

### openSettingsPanel 同页单例（workspace/pageApis.ts）

面板 id = `settings-{pageId}`；getPanel 命中 → focus 返回 true，未命中 → addPanel（component "settings"，settingsPageId 深链注入 params.selectedPage）；100ms×50 轮询 getPageApi 就绪，超时 console.warn 降级返回 false。调用方须先切到目标页（本函数不切页）。

### dirtyRegistry 真值源（SC-FE-07）

`Map<panelId, boolean>`：`setSettingsDirty(panelId, dirty)` / `isSettingsDirty(panelId)` / `clearSettingsDirty(panelId)`。壳挂载注册 false、卸载 clear（面板关闭后不存在「未保存修改」——新挂载不可能 dirty）；DefaultTab × 关闭拦截与壳共享同一真值源，防两处状态漂移。判据为 `settings-` 前缀（DefaultTab 拿不到 panel，`panel.view.contentComponent` 红线不适用该场景），与壳以同一 params.panelId 注册，无漂移。

## 外部坑/红线

- **壳是 params 持久化单点**：页组件不得自行 updateParameters/onLayoutChange，页内参数一律经 `onPageParamsChange` patch → 壳 persistParams（settings 页随布局 JSON 持久化）。
- **组序 global→project 固定**：规格 §4.3 组序「全局」在上、「项目」在下；注册时 group 归错会破坏导航组序。
- **corrupted 警示条 L4 豁免**：写坏 settings.json 需沙箱外写文件，无命令通道——L2 覆盖（loadSettings mock），L4 豁免登记于 test-inventory。

## 测试模式

L2 测试位于 `src/__tests__/`：`settings-page-registry.test.ts`（注册/getAll 分组过滤/order 排序/重复 id 覆盖/_reset 隔离）；`open-settings.test.ts`（无项目 toast 且不切页/活跃项目优先/兜底第一个项目/切页先于开面板）；`open-settings-panel.test.ts`（addPanel 参数精确/单例 focus 不新建/深链 selectedPage/5s 超时降级）。壳与页组件测试见 `src/panels/CLAUDE.md`。
