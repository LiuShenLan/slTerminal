# D2 Review 复验报告 (Round 2)

> 验证日期: 2026-07-25
> 方法: chrome-devtools MCP + curl 直接抓取页面

## 概要

对 Round 1 中标记为"无法独立验证"的 5 条 review 声称 + 1 项 D2 内部矛盾，共 6 项进行复验。

| 结果 | 计数 |
|------|------|
| Review 正确，已修正源文件 | 2 |
| Review 不正确，未修改 | 3 |
| 已在前轮解决，无需操作 | 1 |

---

## 逐条详情

### R4.1: CustomReadonlyEditorProvider（03-vscode-config-ui-reference.md §4.1）

- **Review 声称**: `registerCustomEditorProvider` 接受三种 provider 类型，源文件仅列两种，遗漏了 `CustomReadonlyEditorProvider`
- **验证 URL**: `https://code.visualstudio.com/api/references/vscode-api`
- **页面实际内容**: evaluate_script 搜索 `CustomReadonlyEditorProvider` 命中，上下文为：
  ```
  registerCustomEditorProvider(viewType: string, provider: 
  CustomTextEditorProvider | CustomReadonlyEditorProvider<CustomDocument> | 
  CustomEditorProvider<CustomDocument>, options?: ...)
  ```
- **验证结果**: Review **正确**。官方 API 签名明确列出三种 provider 类型。
- **行动**: 已修正 `03-vscode-config-ui-reference.md` §4.1，从"两种 Editor Provider"改为"三种"，新增 `CustomReadonlyEditorProvider` 行（只读二进制文件预览）。

---

### R4.2: RunOptions 字段（03-vscode-config-ui-reference.md §2.1）

- **Review 声称**: RunOptions 还有 `instanceLimit` 和 `instancePolicy` 字段，源文件仅列出两个字段不完整
- **验证 URL**: `https://code.visualstudio.com/docs/reference/tasks-appendix`
- **页面实际内容** (页面更新日期 7/15/2026):
  ```typescript
  interface RunOptions {
    reevaluateOnRerun?: boolean;
    runOn?: string;
  }
  ```
  全文搜索 `instanceLimit`、`instancePolicy`、`instance` —— 均**未命中**。
- **验证结果**: Review **不正确**。官方 tasks appendix 最新版本中 `RunOptions` 仅定义 `reevaluateOnRerun` 和 `runOn` 两个字段，与源文件完全一致。不存在 `instanceLimit`/`instancePolicy` 字段。
- **行动**: 未修改。源文件已正确。

---

### R5.1: `titledSeparator` 函数名（04-jetbrains-config-ui-reference.md §4.2）

- **Review 声称**: Kotlin UI DSL v2 中带标题分隔线通过 `separator("标题")` 实现（`separator()` 支持可选标题参数），而非独立的 `titledSeparator()` 函数
- **验证 URL**: `https://plugins.jetbrains.com/docs/intellij/kotlin-ui-dsl-version-2.html`
- **页面实际内容**: `Panel.separator` 章节描述为 "Adds horizontal line separator with an optional title." 整个页面无 `titledSeparator` 提及。
- **验证结果**: Review **正确**。`separator()` 接受可选 title 参数，无独立 `titledSeparator()` 函数。
- **行动**: 已修正 `04-jetbrains-config-ui-reference.md` §4.2，`separator()` / `titledSeparator("标题")` → `separator("标题")`（无独立 `titledSeparator` 函数）。

---

### R5.2: 布局规则"最多两列"表述（04-jetbrains-config-ui-reference.md §3.2）

- **Review 声称**: "最多两列"缺少上下文限定，是针对"标签+输入框"的推荐而非所有控件的硬性上限
- **验证 URL**: `https://plugins.jetbrains.com/docs/intellij/layout.html`
- **页面实际内容**:
  - Labeled input controls 节: "organize them in two columns. Do **not** use more than two columns."
  - Checkboxes 节: "4 and more checkboxes can be arranged in columns: labels up to 30 chars in 2 columns; labels up to 15 chars in 3 columns."
- **验证结果**: Review **不正确**。源文件已在"标签 + 输入框"行标注"最多两列"，在"Checkbox / Radio 组"行标注"2 列（标签 ≤30 字符）或 3 列（标签 ≤15 字符）"——上下文限定和复选框多列例外均已正确呈现，与 JetBrains 官方指南完全一致。
- **行动**: 未修改。源文件已正确。

---

### R5.3: 复选框多列例外（04-jetbrains-config-ui-reference.md §3.2）

- **Review 声称**: 遗漏"4 个及以上复选框可排列为多列"的例外
- **验证 URL**: `https://plugins.jetbrains.com/docs/intellij/layout.html`
- **页面实际内容**: Checkboxes section 明确列出 "4 and more checkboxes can be arranged in columns: 2 cols (labels <= 30 chars), 3 cols (labels <= 15 chars)." Radio buttons: "Do **not** arrange radio buttons from one group..."
- **验证结果**: Review **不正确**。源文件已包含"大量选项：2 列（标签 ≤30 字符）或 3 列（标签 ≤15 字符）"以及"Radio button 组不可拆分到多列"——与官方指南完全一致。Review 声称"遗漏"的内容实际已存在于源文件中。
- **行动**: 未修改。源文件已正确。

---

### D2 内部矛盾: PreCompact 可阻止性

- **状态**: 已在 Round 1 的 D2-verify.md 中确认并修正。PreCompact 可阻止（v2.1.105+）。D2-config-management.md §2.6 和 02-settings-json-schema.md §4.5 均已标注"是 (v2.1.105+)"。
- **行动**: 无需操作（已在前轮解决）。

---

## 修改汇总

| 文件 | 修改内容 |
|------|---------|
| `docs/hooks/D2/03-vscode-config-ui-reference.md` §4.1 | 新增 `CustomReadonlyEditorProvider` 类型 + 表格从两行改为三行 |
| `docs/hooks/D2/04-jetbrains-config-ui-reference.md` §4.2 | `separator()` / `titledSeparator("标题")` → `separator("标题")` |
