# F11 设置中心测试覆盖质量 Review

> 范围：`git diff 2154493..825ed56` 测试变更 + 波及面核对。
> 结论：L1/L2/L3 计数已实跑核对（815 / 2839 / 142），与 `.claude/test-inventory.md` 一致；L4 未独立执行，仅静态读 spec。

---

## 问题清单

### 1. 设置中心注册页 `pages.ts` 无 L2 直接守卫
- **严重级别**：低
- **文件**：`src/features/settingsCenter/pages.ts:1` / `src/__tests__/settings-panel.test.tsx:19`
- **问题描述**：`SettingsPanel` 测试把 `../features/settingsCenter/pages` mock 成 `{}` 后手动注册 stub 页，导致真实的 side-effect 注册（`planBalance`/`keybindings`/`hooks` 的 id/group/order）在 L2 从未被验证。L4 只做 hooks 页冒烟，无法守卫注册表漂移。
- **证据**：
  - `settings-panel.test.tsx:19`：`vi.mock("../features/settingsCenter/pages", () => ({}));`
  - `settings-page-registry.test.ts` 只测 Registry 行为，不测 `pages.ts` 实际注册内容。
- **修复建议**：新增 `settings-pages-registration.test.ts`，取消 mock 真实 `pages.ts`，断言 `getSettingsPageRegistry().getAll()` 包含 `{id:"keybindings",group:"global",order:10}`、`{id:"planBalance",group:"global",order:20}`、`{id:"hooks",group:"project",order:100}`。

### 2. `persistSelectedCli` 迁移后纯函数语义覆盖缺失
- **严重级别**：低
- **文件**：`src/__tests__/settings-hooks-page.test.tsx:782-796` / `src/__tests__/settings-panel.test.tsx:235-284`
- **问题描述**：原 `hooks-config-panel.test.tsx` 有 4 条 `persistSelectedCli` 纯函数用例（`updateParameters` 合并原键、`onLayoutChange` 显式保存、`params` 对象不可变、selectedCli 与当前一致时短路）。迁移为 `SettingsPageProps.onPageParamsChange` 后，新测试只断言“点击切换会回调 `selectedCli`”，未覆盖合并、不可变、短路三条语义。
- **证据**：
  - diff 显示原 43 例 → 现 37 例，删除的 4 例为 `updateParameters 展开保留原键`、`onLayoutChange 收到 saveLayout(containerApi) 结果`、`原 params 对象不被修改`、`selectedCli 与当前一致时组件侧已短路`。
  - `settings-hooks-page.test.tsx:782-796` 仅断言点击 testcli 后 `onPageParamsChange({ selectedCli: "testcli" })` 被调用一次。
- **修复建议**：
  - 在 `settings-hooks-page.test.tsx` 增加“点击当前已选中 CLI 不触发 `onPageParamsChange`”用例。
  - 在 `settings-panel.test.tsx` 增加 selectedCli 持久化时合并既有 params 且不修改原对象的用例。

### 3. `settings-panel` 的 `onPageParamsChange` 未断言 `saveLayout`
- **严重级别**：低
- **文件**：`src/__tests__/settings-panel.test.tsx:248-270`
- **问题描述**：`SettingsPanel.persistParams` 的设计契约是 `updateParameters + 显式 onLayoutChange(saveLayout(containerApi))`（`SettingsPanel.tsx:238-243`）。pageParams patch 用例只断言了 `updateParameters` 的合并参数，未断言 `saveLayout` 被触发，存在“只改 params 不落盘布局”的假绿空间。
- **证据**：
  - `settings-panel.test.tsx:248-270`：断言 `api.updateParameters` 被调用两次，未断言 `containerApi.toJSON` 或 `saveLayout`。
  - 同文件 `选中切换持久化` 用例（:159）断言了 `containerApi.toJSON`，但 pageParams 路径未覆盖。
- **修复建议**：在 pageParams patch 用例中增加 `expect(containerApi.toJSON).toHaveBeenCalled()`（或 mock `saveLayout` 并断言其被调用）。

### 4. L4 未覆盖 × 关闭 dirty 守卫
- **严重级别**：低
- **文件**：`e2e-tests/settings.e2e.ts:497-552`
- **问题描述**：L4 spec 用例 ⑩ 覆盖了“dirty 切配置页 confirm 取消”，但未覆盖 Dockview 页签 × 按钮在 dirty 设置面板上的关闭守卫。该守卫在 `PageDockviewHost` 中实现，仅由 L2 `workspace-defaulttab.test.tsx` 覆盖，缺少真实二进制上的完整链路验证。
- **证据**：
  - `settings.e2e.ts` 10 个用例清单无 × 关闭相关 case。
  - `workspace-defaulttab.test.tsx:497-552` 用 mock PanelApi 测试了该守卫。
- **修复建议**：在 `settings.e2e.ts` 增加用例：hooks 页置 dirty → 点击设置面板页签 × → confirmDialog 出现 → 取消后面板不关闭；或明确在 `test-inventory.md` 登记“× 关闭 dirty 守卫由 L2 覆盖，L4 豁免”。

### 5. L4 同项目切页“面板保留”断言过弱
- **严重级别**：低
- **文件**：`e2e-tests/settings.e2e.ts:575-606`
- **问题描述**：用例 ⑧ 仅断言 `document.querySelectorAll('[data-e2e="settings-panel"]').length === 1` 且 `selectedPage` 不变。若面板被 Dockview 移动到新的活跃页面，上述断言仍可能通过，无法真正证明面板仍属于原页面。
- **证据**：
  - :592-594 只检查 DOM 面板总数。
  - :598-601 只检查 `state?.selectedPage === "planBalance"`。
- **修复建议**：在切页前后分别断言 helper 读到的面板 `panelId` 仍为 `settings-<originalPageId>`，或断言原页面 active api 中仍存在该面板。

### 6. 迁移测试残留未使用的 Dockview props mock
- **严重级别**：低
- **文件**：`src/__tests__/settings-hooks-page.test.tsx:40-52` / `src/__tests__/hooks-config-sync.test.tsx:60-67`
- **问题描述**：`HooksSettingsPage` 已改为 `SettingsPageProps` 形态（不再消费 `api`/`containerApi`），但上述测试仍保留 `mockApi`/`mockContainerApi` 并在 `beforeEach` 中 `mockReset`，属于迁移遗留的死代码，会增加后续维护者误解。
- **证据**：
  - `settings-hooks-page.test.tsx:40-52` 定义 `mockApi.updateParameters`/`mockContainerApi.toJSON`，组件未使用。
  - `hooks-config-sync.test.tsx:60-67` 同样保留 `mockApi` 定义。
  - 两个文件的 `beforeEach` 仍在 reset 这些 mock（如 `settings-hooks-page.test.tsx:300-303`）。
- **修复建议**：删除 `mockApi`/`mockContainerApi` 定义及对应 reset 代码，仅保留 `mockOnPageParamsChange`。

### 7. L4 后端 settings.json 路径硬编码
- **严重级别**：低
- **文件**：`e2e-tests/settings.e2e.ts:64`
- **问题描述**：`settingsJsonPath` 直接写死 `join(process.cwd(), "src-tauri", "target", "debug", "settings.json")`。若 CI/本地运行时的 cwd 与预期不一致，或构建产物目录变更，用例 ④/⑤/⑥ 会直接失败，且该路径与 `run-wdio.cjs` 备份集合不一致（未备份 exe 同级 settings.json）。
- **证据**：`e2e-tests/settings.e2e.ts:64` 硬编码路径；`e2e-tests/CLAUDE.md` 明确说明 `run-wdio.cjs` 不覆盖 exe 同级 settings.json。
- **修复建议**：从 `tauri.conf.json` 或 WDIO `tauri-service` 的 application path 推导 exe 目录，再拼接 `settings.json`；或在 `run-wdio.cjs` 中把 exe 同级 settings.json 纳入快照备份。
