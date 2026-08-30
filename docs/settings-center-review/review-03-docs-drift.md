# F11 设置中心文档一致性 + 计划偏离审计报告

> 审查范围：`2154493..825ed56`（9 个 commit）。
> 只写问题；无问题的文档不列出。

---

## 一、文档撒谎检测（文档描述与当前代码不符）

### 1. 需求规格仍把 schema 单点标在已删除的目录

- **严重级别**：高
- **位置**：`docs/settings-center-requirements.md:189`
- **问题描述**：文档称 hooks 配置 schema 单点仍保留在 `src/features/hooksConfig/`，并仅删除 `openHooksConfig.ts`；但当前代码中 `src/features/hooksConfig/` 目录已整体删除，schema 已随编辑器组件迁移到 `src/features/cliProfiles/profiles/claude/configEditor/schema/`。
- **证据**：
  - 文档原文："`- src/features/hooksConfig/`：schema 单点保留（claude 专属资产，MC-223 语义不变），`openHooksConfig.ts` 删除。"
  - 代码现状：`src/features/cliProfiles/profiles/claude/configEditor/schema/index.ts:27` import 本地 `./claude-code-settings.json`；`Glob` 确认 `src/features/hooksConfig` 不存在。
- **修复建议**：将 §5.3 中 `src/features/hooksConfig/` 一行的描述改为 "已整体删除；schema 单点迁入 `src/features/cliProfiles/profiles/claude/configEditor/schema/`"。

### 2. 需求规格把面板注册流程写成含不存在的 `panels/index` barrel

- **严重级别**：中
- **位置**：`docs/settings-center-requirements.md:170`
- **问题描述**：文档称新增面板类型走 "目录 → `panels/index` → `panelRegistry` → `PANEL_TYPES` → capabilities" 五步，但仓库中不存在 `src/panels/index.ts` barrel。根文件硬约束 #5 与 `src/panels/CLAUDE.md` 均明确新增面板类型只需 "加目录 + 在 `panelRegistry.ts` 注册"。
- **证据**：
  - 文档原文："面板封闭（硬约束 #5）：新增面板类型 `settings` 走既定五步（目录 → panels/index → panelRegistry → PANEL_TYPES → capabilities）"
  - 代码现状：`src/panels/CLAUDE.md:200-204` 步骤 2 明确写 "在 `src/panelRegistry.ts` 注册组件映射（**无 `src/panels/index.ts` barrel**）"。
- **修复建议**：删除 `panels/index` 步骤，改为 "目录 → `panelRegistry.ts` 注册 → `PANEL_TYPES` 追加；如涉及新 IPC 命令再追加 capabilities"。

### 3. 测试迁移文档新增文件清单遗漏 3 个关键文件

- **严重级别**：低
- **位置**：`src/__tests__/CLAUDE.md:69`
- **问题描述**：F11 新增测试文件清单只列 7 个文件，遗漏了同样新增的 `open-settings.test.ts`、`open-settings-panel.test.ts`、`settings-hooks-page.test.tsx`；虽然上方映射表已体现更名关系，但 "新增" 小节本身不完整，容易让人误以为只有 7 个新增文件。
- **证据**：
  - 文档原文："新增：`settings-page-registry.test.ts` / `settings-panel.test.tsx` / ... / `settings-panel-autoclose.test.tsx`"
  - 代码现状：`Glob` 与 `test-inventory.md` 均显示存在 `open-settings.test.ts`、`open-settings-panel.test.ts`、`settings-hooks-page.test.tsx`。
- **修复建议**：在新增清单中补上 `open-settings.test.ts`、`open-settings-panel.test.ts`、`settings-hooks-page.test.tsx`。

### 4. 文件型页签判据注释仍引用已退役的 `hooksConfig` 类型

- **严重级别**：低
- **位置**：`src/workspace/PageDockviewHost.tsx:423`
- **问题描述**：`DefaultTab` 的 `filePath` 判据注释把 `terminal/hooksConfig` 并列作为不携带 `filePath` 的类型，但 `hooksConfig` 面板类型已退役，当前只剩 `terminal`/`settings` 不携带 `filePath`。
- **证据**：
  - 注释原文："（只有 FILE_PANEL_TYPES ——editor/htmlviewer/gitshow/diff——的面板携带 filePath；terminal/hooksConfig 恒不设置，见 panelRegistry.ts）"
  - 代码现状：`src/panelRegistry.ts:73-80` 的 `PANEL_TYPES` 已不含 `hooksConfig`。
- **修复建议**：把 `terminal/hooksConfig` 改为 `terminal/settings`。

---

## 二、计划偏离审计（ checklist 与实际 diff 对比）

`report.md` 已登记 3 项偏离（关闭守卫判据改为 `params.panelId` 前缀、e2e 与 vite build 串行、keystroke 格式含 `Key` 前缀），复核均属实。

### 5. E2E helper `__slterm_e2e_getSettingsPanelCount` 计数范围与 checklist 约定不符（未登记偏离）

- **严重级别**：中
- **位置**：`e2e-tests/helpers.ts:325-334`
- **问题描述**：`SC-E2E-01` 明确约定该 helper 应 "全部页面 api 计数"，但当前实现只遍历 `window.__dockviewApi`（当前活跃页面的 DockviewApi）。虽然设置中心全局单例让两者在功能上等价，但 helper 契约与计划产物不一致，且未在 `report.md` 登记。
- **证据**：
  - checklist 原文（`docs/settings-center/checklist.md:326`）："`__slterm_e2e_getSettingsPanelCount(): number`（全部页面 api 计数）"
  - 代码现状：
    ```ts
    window.__slterm_e2e_getSettingsPanelCount = () => {
      const api = window.__dockviewApi;
      if (!api) return 0;
      let count = 0;
      for (const panel of api.panels) {
        if (panel.id.startsWith("settings-")) count += 1;
      }
      return count;
    };
    ```
- **修复建议**：二选一：a) 改为遍历所有已注册页面 api（`pageApiMap` 或暴露全局页面 api 列表）并计数全部 settings 面板，使实现与 checklist 一致；b) 若确认活跃页面计数足够，在 `report.md` 第四节补登记为第 4 项执行期偏离，并同步 `e2e-tests/CLAUDE.md` helper 说明。

---

## 汇总

| 项 | 高 | 中 | 低 | 合计 |
|---|---|---|---|---|
| 问题数 | 1 | 2 | 2 | 5 |

> 注：按正文条目计数（文档撒谎 4 条 + 偏离审计 1 条）。

**产出文件**：`docs/settings-center-review/review-03-docs-drift.md`
