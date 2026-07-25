# 04-jetbrains-config-ui-reference.md 审查报告

## 错误 1: `titledSeparator` 函数名未经证实

- **文件+行号**: 第 4.2 节 DSL 元素表（约第 340 行）
- **原声称**: `titledSeparator("标题")` 是 Kotlin UI DSL v2 的独立函数
- **错误类型**: 事实错误
- **正确信息**: Kotlin UI DSL v2 中带标题的分隔线通过 `separator("标题")` 实现（`separator()` 函数支持可选标题参数），而非独立的 `titledSeparator()` 函数。Context7 检索 IntelliJ Platform SDK 文档未找到名为 `titledSeparator` 的独立函数。v1 DSL 中使用 `titledRow`。
- **反证来源**: IntelliJ Platform SDK Kotlin UI DSL v2 文档（Context7 检索），`separator()` 函数签名支持 `title: String?` 参数

## 错误 2: 布局规则 "最多两列" 表述过于绝对

- **文件+行号**: 第 3.2 节独立控件排列规则表（约第 269 行）
- **原声称**: "最多两列"（针对标签+输入框排列）
- **错误类型**: 过时信息/表述不准确
- **正确信息**: IntelliJ Platform Layout Guidelines 原文为 "you may organize them in up to two columns"——这是针对标签+输入框控件的推荐做法，而非对所有控件的硬性上限。复选框在 4 个及以上时可排列为多列（短标签最多 3 列）。文档中的 "最多两列" 缺少上下文限定。
- **反证来源**: IntelliJ Platform SDK Layout Guidelines（Context7 检索），明确区分了标签+输入框（最多两列）与复选框（2-3 列）的不同排列规则

## 错误 3: "复选框每行一个" 遗漏例外场景

- **文件+行号**: 第 3.2 节独立控件排列规则表（约第 270 行）
- **原声称**: "Checkbox / Radio 组：默认每行一个。2-3 个短标签（1-3 词）可同行"
- **错误类型**: 过时信息
- **正确信息**: 后半句 "2-3 个短标签可同行" 正确描述了例外。但遗漏了另一重要例外：4 个及以上复选框可排列为 2-3 列（取决于标签长度）。原始声明未提及多列排列选项。
- **反证来源**: IntelliJ Platform SDK Layout Guidelines 原文规定：大量选项时，2 列（标签 <= 30 字符）或 3 列（标签 <= 15 字符）

## 错误 4: External Tools / File Watchers / Create Tool Dialog 来源 URL 未验证

- **文件+行号**: 第 6 节来源汇总（约第 460 行起），引用 jetbrains.com/help/idea/ 产品文档
- **原声称**: 引用以下 URL 作为来源：
  - `https://www.jetbrains.com/help/idea/settings-tools-external-tools.html`
  - `https://www.jetbrains.com/help/idea/2022.2/settings-tools-create-edit-copy-tool-dialog.html`
  - `https://www.jetbrains.com/help/idea/settings-tools-file-watchers.html`
- **错误类型**: 来源不支撑
- **正确信息**: 这些是面向用户的 IntelliJ IDEA 产品文档（非 IntelliJ Platform SDK），网络限制导致无法独立验证其当前内容。特别是 `2022.2` 版本号指向两年前的 IDE 版本，当前 UI 可能已有变更。文档应标注 "2022.2 版本，当前 UI 可能不同"。
- **反证来源**: 无法访问 jetbrains.com 域名进行内容核实

---

未发现错误（已验证 9 项声称）:
- Configurable 接口方法（createComponent/isModified/apply/reset/disposeUIResources）— 正确（Context7 SDK 文档确认）
- PersistentStateComponent（@State 注解/getState/loadState）— 正确
- MVC 三层分离（Configurable/Settings/SettingsComponent）— 正确（SDK 明确记录此模式）
- Kotlin UI DSL 元素（panel/row/group/collapsibleGroup/indent/separator/comment）— 正确（Context7 确认）
- Row.layout 三种模式（LABEL_ALIGNED/INDEPENDENT/PARENT_GRID）— 正确
- 绑定方法（bindText/bindSelected/bindIntText/bindItem/bindValue/bindIntValue/bind）— 正确
- 窗口尺寸规范（Small 350x250 / Medium 500x350 / Large 750x525 / Extra Large 1000x700）— 100% 正确
- "标签左对齐" 规则 — 正确
- 渐进式披露（Progressive Disclosure）设计模式描述 — 正确
