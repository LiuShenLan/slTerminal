# 03-vscode-config-ui-reference.md 审查报告

## 错误 1: Custom Editor API 遗漏第三种 Provider 类型

- **文件+行号**: 第 4.1 节（约第 234 行起）
- **原声称**: "两种 Editor Provider：CustomTextEditorProvider / CustomEditorProvider"
- **错误类型**: 事实错误
- **正确信息**: VS Code `registerCustomEditorProvider` API 实际接受**三种** provider 类型：`CustomTextEditorProvider | CustomReadonlyEditorProvider<CustomDocument> | CustomEditorProvider<CustomDocument>`。文档遗漏了 `CustomReadonlyEditorProvider`——用于只读模式的二进制文件编辑（如 PDF 预览），扩展只需实现 `openCustomDocument` 和 `resolveCustomEditor`。
- **反证来源**: VS Code API Reference — `window.registerCustomEditorProvider` 签名含三种类型；Custom Editor API Guide 明确三个场景：text/readonly/custom

## 错误 2: RunOptions 字段列表不完整

- **文件+行号**: 第 2.1 节 RunOptions 表（约第 106 行）
- **原声称**: RunOptions 仅有 `reevaluateOnRerun` 和 `runOn` 两个字段
- **错误类型**: 过时信息
- **正确信息**: RunOptions 还有 `instanceLimit`（number，限制同时运行的任务实例数）和 `instancePolicy` 字段。文档声称仅两个字段不完整。
- **反证来源**: VS Code tasks appendix — `RunOptions` 定义含 `instanceLimit?: number` 和 `instancePolicy?: string`

## 错误 3: SchemaStore schema 数量不准确（500+ 偏低）

- **文件+行号**: 第 6.4 节（约第 369 行）
- **原声称**: "覆盖 500+ 常见配置文件"
- **错误类型**: 过时信息
- **正确信息**: schemastore.org 当前实际覆盖 **1,384** 个 schema 文件（网站声明 "JSON Schemas are available for the following 1384 files"）。"500+" 严重低估。
- **反证来源**: https://www.schemastore.org/ 首页明确显示 1,384

## 错误 4: SchemaStore catalog API 路径错误

- **文件+行号**: 第 6.4 节（约第 369 行）
- **原声称**: "完整目录：https://www.schemastore.org/api/json/catalog.json"
- **错误类型**: 事实错误
- **正确信息**: SchemaStore 的 catalog API 正确路径为 `https://www.schemastore.org/api/v1/catalog.json`（含 `/v1/` 段），而非 `/api/json/catalog.json`。后者返回 404。
- **反证来源**: 实测 `https://www.schemastore.org/api/json/catalog.json` 返回 404

## 错误 5: compounds 声称所在页面不包含相关内容

- **文件+行号**: 第 3.3 节末尾（约第 213 行）
- **原声称**: "来源：https://code.visualstudio.com/docs/editor/debugging#_launch-configurations"
- **错误类型**: 来源不支撑
- **正确信息**: 该页面（实际 URL `https://code.visualstudio.com/docs/debugtest/debugging`）当前版本中不包含 "compounds"、"composite" 或 "multi-target" 相关内容。实测页面文本搜索 `compound` 返回 "not found"。compounds 功能确实存在于 VS Code，但官方文档位置可能已变更（可能在独立页面或不同锚点）。
- **反证来源**: evaluate_script 搜索页面文本，`compound`/`compounds`/`composite`/`multi-target` 均未命中

## 错误 6: yyc/command-variable 来源无法验证

- **文件+行号**: 第 5.3 节（约第 316 行）
- **原声称**: "来源：https://github.com/yyc/command-variable"
- **错误类型**: 来源不支撑
- **正确信息**: GitHub 仓库 `yyc/command-variable` 无法通过网络验证存在性（连接超时）。对应 VS Code Marketplace 扩展 `yyc.command-variable` 返回 404。文档将其作为社区扩展方案引用，但无法确认该资源当前状态。
- **反证来源**: GitHub 持续超时；marketplace.visualstudio.com/items?itemName=yyc.command-variable 返回 404

## 错误 7: Input Variables 节归属误导

- **文件+行号**: 第 2.2 节（约第 113 行起）
- **原声称**: "2.2 Input Variables（用户交互输入）" 位于 "2. tasks.json Schema 参考" 大节下
- **错误类型**: 内部矛盾
- **正确信息**: Input Variables 并非 tasks.json schema 的正式组成部分，而是 VS Code 变量系统的独立功能。官方文档将其放在 Variables Reference 页面（https://code.visualstudio.com/docs/reference/variables-reference#_input-variables），同时涵盖 launch.json 和 tasks.json。文档自身引用了正确的 URL，但将其放在 "tasks.json Schema 参考" 标题下会造成误导——读者可能以为这是 tasks.json schema 的正式组成部分。
- **反证来源**: VS Code Variables Reference 页面实测证实 Input Variables 为独立文档章节；tasks 附录页不包含 input variables 内容

---

未发现错误（已验证 8 项声称）:
- tasks.json version "2.0.0" — 正确（Variables Reference 页面示例 + tasks appendix 确认）
- task types "shell"/"process" — 正确（tasks appendix 确认）
- PresentationOptions 字段列表（reveal/echo/focus/panel/clear/group）— 正确（tasks appendix 确认，另有一个 showReuseMessage 文档未列但非错）
- 三种 input 类型（promptString/pickString/command）— 正确（Variables Reference 页面确认）
- pickString 支持 string[] 和 label/value 两种格式 — 正确（页面原文："An option can be a string value or an object with both a label and value"）
- pickString 不支持变量替换 — 正确
- jsonc-parser 的 modify() 函数 — 正确（npm 页面确认："the modify API computes edits to insert, remove or replace a property or value"）
- Tingly Debug Configurations / Tasks Shell Input / Depot Data Editor 三个扩展 — 均存在于 Marketplace（子代理验证确认）
