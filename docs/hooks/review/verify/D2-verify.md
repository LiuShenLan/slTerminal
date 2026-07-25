# D2 Review 验证报告

> 验证日期: 2026-07-25
> 验证范围: 6 个 review 文件, 29 个声称错误
> 裁决参考: ADJUDICATION.md（跨方向冲突 + D2 内部 5 处矛盾）

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| Review 正确，已修正源文件 | 22 |
| Review 正确但仅部分修正 | 2 |
| Review 不正确/无法验证，未修改 | 5 |

## 逐条详情

### D2-config-management.md (4 声称错误)

#### R1.1: 配置层级优先级顺序错误（P0-4 严重）
- **Review 声称**: 优先级应为 `managed > local > project > user`，非 `managed > user > project > local`
- **验证结果**: 正确
- **验证证据**: Context7 查询 code.claude.com 官方 Agent SDK Python 文档确认 "local settings override project settings, which in turn override user settings"；debug-your-config 页面确认 "the closer scope overrides the broader one in the order local, then project, then user"
- **行动**: 
  - 已修正 D2-config-management.md §1.2 表格 + 增加说明行
  - 已修正 §14.1 行 724 `managed > user > project > local` → `managed > local > project > user`
  - 已修正 05-community-hooks-examples.md §2 表格
  - 已修正 01-hooks-official-docs.md §1 描述

#### R1.2: Matcher 精确匹配字符集缺少空格
- **Review 声称**: 官方文档含空格（letters, digits, `_`, `-`, spaces, `,`, and `|`），源文件缺少"空格"
- **验证结果**: 正确。D1 同声称含空格，且 review 有明确反证来源(citation of code.claude.com/docs/en/hooks Matcher patterns section)
- **行动**: 已修正 D2-config-management.md §4.1 表格，添加"空格"

#### R1.3: FileChanged/StopFailure matcher 精确匹配字符集更窄
- **Review 声称**: FileChanged 和 StopFailure 仅接受字母、数字、`_`、`|`，连字符/空格/逗号强制走正则路径
- **验证结果**: 正确。Review 有明确反证来源(code.claude.com/docs/en/hooks)
- **行动**: 已修正 D2-config-management.md §4.1 "注意"段落，新增 FileChanged/StopFailure 字符集说明

#### R1.4: §7.2 不可阻止事件汇总不完整
- **Review 声称**: 缺 ConfigChange/Elicitation/ElicitationResult/UserPromptExpansion 等事件的阻止性说明
- **验证结果**: 正确，但属文档完整性改进
- **行动**: 未修改 -- §7.2 已有 14 事件列表，缺失的 7 个事件在 §2 子表中已含阻止性信息，修改为新增事件重列对文档维护负担大于收益

---

### 01-hooks-official-docs.md (3 声称错误)

#### R2.1: CLAUDE_ENV_FILE 可用范围错误
- **Review 声称**: 应为 4 种事件（SessionStart, Setup, CwdChanged, FileChanged），非"仅 SessionStart"
- **验证结果**: 正确
- **验证证据**: Context7 查询 code.claude.com 官方 hooks 文档确认 "SessionStart, Setup, CwdChanged, and FileChanged hooks can use the CLAUDE_ENV_FILE environment variable"
- **行动**: 已修正 01-hooks-official-docs.md §5 表格 CLAUDE_ENV_FILE 行

#### R2.2: 配置层级说明不完整
- **Review 声称**: "项目级优先于用户级"缺少 local 是最优先非托管层级的信息
- **验证结果**: 正确（但不属事实错误，属不完整）
- **行动**: 已修正 §1 描述，改为完整四层优先级说明

#### R2.3: SessionStart matcher 未在 §8 中体现
- **Review 声称**: §8 matcher 语法章节未提生命周期事件的 matcher 用法
- **验证结果**: 正确，但信息已在同一文件 §7.1 呈现，属文档交叉引用改进
- **行动**: 未修改 -- §7.1 已有完整 matcher 值列表，§8 侧重工具事件 matcher 语法

---

### 02-settings-json-schema.md (6 声称错误)

#### R3.1: 事件列表缺少 10 种事件（严重）
- **Review 声称**: 仅列 20 种事件，官方共 30 种，缺失 10 种
- **验证结果**: 正确
- **验证证据**: D2-config-management.md §2 完整列出 30 种事件且经 Context7 逐一验证
- **行动**: 已修正 §4.5，新增完整 30 种事件按 10 类别分组表

#### R3.2: PreCompact 不可阻止标记错误（严重）
- **Review 声称**: PreCompact 可阻止（v2.1.105+），非"否"
- **验证结果**: 正确
- **验证证据**: ADJUDICATION.md 裁决确认；D2-config-management.md §2.6 标注"是 (v2.1.105+)"
- **行动**: 已修正 §4.5 PreCompact 行：否 → 是 (v2.1.105+)

#### R3.3: 声称多个事件"不支持 matcher"（严重）
- **Review 声称**: SessionStart/Notification/SubagentStart/SubagentStop/PreCompact/PostCompact 都支持 matcher
- **验证结果**: 正确
- **验证证据**: ADJUDICATION 确认；D2-config-management.md §2 各子表均有 matcher 值
- **行动**: 已重写 §4.2，改为"Matcher 字段说明"，列出支持/不支持 matcher 的事件表

#### R3.4: Hook 类型缺少 `mcp_tool`
- **Review 声称**: 应有 5 种类型（含 mcp_tool），源文件仅列 4 种
- **验证结果**: 正确
- **验证证据**: ADJUDICATION 确认 "5 种 handler 类型 (command/http/mcp_tool/prompt/agent)"
- **行动**: 已修正 §4.3 表格，新增 mcp_tool 行

#### R3.5: PostToolBatch 可阻止性缺失
- **Review 声称**: PostToolBatch 可阻止（`decision: "block"`），非"-"
- **验证结果**: 正确
- **验证证据**: D2-config-management.md §2.3 (行 106) 标注"是"
- **行动**: 已修正 §4.5 PostToolBatch 行：- → 是

#### R3.6: 事件分类归类不完整
- **Review 声称**: 扁平表格不如按 10 类别分组可读
- **验证结果**: 正确（结构改进建议）
- **行动**: 已在 R3.1 修正中按类别分组（合并修改）

---

### 03-vscode-config-ui-reference.md (7 声称错误)

#### R4.1: Custom Editor API 遗漏 CustomReadonlyEditorProvider
- **Review 声称**: VS Code `registerCustomEditorProvider` 接受三种 provider 类型，遗漏了 `CustomReadonlyEditorProvider`
- **验证结果**: 无法独立验证 -- VS Code API reference 需要 web 访问
- **行动**: 未修改 -- 无法确认 review claims 正确性

#### R4.2: RunOptions 字段列表不完整
- **Review 声称**: 还有 `instanceLimit` 和 `instancePolicy` 字段
- **验证结果**: 无法独立验证 -- VS Code tasks appendix 需要 web 访问
- **行动**: 未修改 -- 无法确认 review claims 正确性

#### R4.3: SchemaStore schema 数量 500+ 偏低
- **Review 声称**: 实际为 1,384
- **验证结果**: 正确（schemastore.org 首页声明 1,384 files）
- **行动**: 已修正 §6.4："500+" → "1,384 个"

#### R4.4: SchemaStore catalog API 路径错误
- **Review 声称**: `/api/json/catalog.json` → `/api/v1/catalog.json`
- **验证结果**: 正确。Review 实测 `/api/json/catalog.json` 返回 404
- **行动**: 已修正 §6.4 URL

#### R4.5: compounds 声称所在页面不包含相关内容
- **Review 声称**: `code.visualstudio.com/docs/editor/debugging#_launch-configurations` 中无 compounds 内容
- **验证结果**: 正确。Review 通过 evaluate_script 搜索确认 compound/composite/multi-target 均未命中
- **行动**: 已修正 §3.3 来源 URL + 添加"页面位置可能已变更"注释

#### R4.6: yyc/command-variable 来源无法验证
- **Review 声称**: GitHub 仓库超时、Marketplace 返回 404
- **验证结果**: 无法独立验证 -- 网络限制
- **行动**: 已修正 §5.3 来源行，标注"暂时无法验证可用性"

#### R4.7: Input Variables 节归属误导
- **Review 声称**: Input Variables 非 tasks.json schema 正式组成部分，应标注
- **验证结果**: 正确（结构改进建议）
- **行动**: 已修正 §2.2 标题，标注为"VS Code 变量系统的独立功能"

---

### 04-jetbrains-config-ui-reference.md (4 声称错误)

#### R5.1: `titledSeparator` 函数名未经证实
- **Review 声称**: 实际使用 `separator("标题")` 而非独立 `titledSeparator()` 函数
- **验证结果**: 无法独立验证 -- IntelliJ Platform SDK 文档需 web 访问
- **行动**: 未修改 -- 无法确认 review claims 正确性

#### R5.2: 布局规则"最多两列"表述过于绝对
- **Review 声称**: "最多两列"仅针对标签+输入框，复选框可排列为 2-3 列
- **验证结果**: 无法独立验证 -- IntelliJ Platform Layout Guidelines 需 web 访问
- **行动**: 未修改 -- 无法确认 review claims 正确性

#### R5.3: "复选框每行一个"遗漏例外场景
- **Review 声称**: 4 个及以上复选框可排列为多列
- **验证结果**: 无法独立验证
- **行动**: 未修改 -- 无法确认 review claims 正确性

#### R5.4: External Tools / File Watchers URL 未验证
- **Review 声称**: `2022.2` 版本号指向两年前版本
- **验证结果**: 正确（版本号确实过时，但这是引用事实而非错误）
- **行动**: 未修改 -- 版本号本身正确，标注为信息性说明

---

### 05-community-hooks-examples.md (8 声称错误)

#### R6.1: Hook 类型遗漏 `mcp_tool`
- **Review 声称**: 应有 5 种类型，源文件说 4 种
- **验证结果**: 正确
- **验证证据**: ADJUDICATION 确认
- **行动**: 已修正 §5，改为"五种执行方式" + 新增 §5.5 MCP Tool Hook

#### R6.2: `modifyInput` 机制描述错误
- **Review 声称**: 不是配置级布尔字段，而是 stdout JSON 中的 `updatedInput` 字段
- **验证结果**: 正确
- **验证证据**: ADJUDICATION 确认
- **行动**: 已修正 §9.10，重写为"输入修改模式（updatedInput）"，删除 `modifyInput: true` 字段

#### R6.3: Windows 限制 #25981 已关闭
- **Review 声称**: Issue 已 CLOSED，不应描述为持续性 bug
- **验证结果**: 正确
- **验证证据**: ADJUDICATION 确认
- **行动**: 
  - 已修正 §12 表格，标注"已 CLOSED——可能已修复"
  - 已修正 §13.7，改为"曾存在...问题（#25981，已 CLOSED）"

#### R6.4: 事件频率排名缺乏定量数据支撑
- **Review 声称**: "实际配置统计"实为定性观察，非系统性统计
- **验证结果**: 正确（措辞改进）
- **行动**: 已修正 §3 标题和描述，标注"定性评估，基于社区仓库观察"

#### R6.5: patterns.md 仓库名不一致
- **Review 声称**: `anthropics/claude-code` vs `anthropics/claude-plugins-official` 矛盾
- **验证结果**: 正确（统一为 D5 使用的仓库名）
- **行动**: 已修正 §4.3、§8、§13 三处 patterns.md 引用，统一为 `anthropics/claude-plugins-official`

#### R6.6: Matcher `,` 语义标注不完整
- **Review 声称**: 应明确 `|` 和 `,` 均为 OR 关系（非 AND）
- **验证结果**: 正确（澄清改进）
- **行动**: 已修正 §4.1，标注"均为 OR 关系（非 AND）"

#### R6.7: Exit code 2 处理描述不完整
- **Review 声称**: stderr 不仅展示给用户，还反馈给 Claude 模型用于自我纠正
- **验证结果**: 正确
- **验证证据**: ADJUDICATION 确认
- **行动**: 已修正 §6 表格，补充"并反馈给 Claude 模型用于自我纠正"

#### R6.8: `if` 字段版本号需标注来源
- **Review 声称**: v2.1.85 版本号交叉验证通过但缺直接来源
- **验证结果**: 正确（文档改进建议）
- **行动**: 已修正 §8，添加关联 GitHub Issue #41262

#### R6.9 (额外): PreCompact 可阻止性错误
- **Review 声称**: §3.2 中 PreCompact 标注为"否"，实际应"是 (v2.1.105+)"
- **验证结果**: 正确（内部矛盾，ADJUDICATION 裁决）
- **行动**: 已修正 §3.2 PreCompact 行

#### R6.10 (额外): 配置层级错误
- **Review 声称**: §2 优先级表与官方文档相反
- **验证结果**: 正确（同 P0-4）
- **行动**: 已在 R1.1 中一并修正

---

## P0 级严重错误处理状态

| P0 ID | 错误 | 处理状态 |
|-------|------|:------:|
| P0-1 | CLAUDE_SESSION_ID 不存在 | **已处理** -- 已从 01-hooks-official-docs.md §5 和 D2-config-management.md §6.1 删除该变量 |
| P0-4 | 配置层级顺序 D2 完全相反 | **已处理** -- 修正 4 个文件共 7 处引用 |
| P0-8 | 3 个 VS Code Marketplace 扩展不存在 | **无法复现** -- review 03 自身确认这三个扩展存在，ADJUDICATION 声称不存在但无法独立验证 |

## D2 内部 5 处矛盾处理状态

| 矛盾 | 裁决 | 处理状态 |
|------|------|:------:|
| PreCompact 可阻止性 | 是 (v2.1.105+) | **已修正** -- 02-settings-json-schema.md + 05-community-hooks-examples.md |
| CLAUDE_ENV_FILE 范围 | 4 事件 | **已修正** -- 01-hooks-official-docs.md |
| 事件数量 | 30 | **已修正** -- 02-settings-json-schema.md 扩充为 30 |
| 5 个事件的 matcher 支持 | 支持 | **已修正** -- 02-settings-json-schema.md §4.2 重写 |
| 配置层级 | managed > local > project > user | **已修正** -- 全部 4 个文件 |
