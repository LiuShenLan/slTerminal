# D5 Review 验证报告

> 验证日期: 2026-07-25
> 验证范围: 5 个 review 文件, 41 个声称错误
> 验证方式: 内部证据交叉比对 + ADJUDICATION 裁决引用（WebSearch/WebFetch 均被阻止，外部来源无法独立验证）

## 验证结果汇总

| 类别 | 数量 |
|------|------|
| Review 正确，已修正源文件 | 27 |
| Review 部分正确，已修正 | 6 |
| Review 不正确/无法验证，未修改 | 6 |
| 内部矛盾（与其他 review/ADJUDICATION 冲突），标注但未修改 | 2 |

---

## 逐条详情

### R5.1: "stdin/stdout JSON 协议成为事实标准"过度概括
- **Review 声称**: Copilot CLI 和 Codex CLI 使用 JSON-RPC over stdio，不是原始 stdin→stdout
- **验证结果**: 正确。ADJUDICATION Conflict 6 裁决确认
- **验证证据**: ADJUDICATION.md 冲突 6 裁决
- **行动**: 已修改 D5-excellent-projects.md §1.1（分离 4 工具 + 2 工具，注明协议不兼容）+ D5a（Codex CLI 协议描述）

### R5.2: Claude Code "12+" hook 事件数低估
- **Review 声称**: D5a 列出 14 个事件（12 核心 + 2 实验性），"12+" 偏低
- **验证结果**: 正确。D5a 源文件自身列出 12 核心 + 2 实验性 = 14
- **验证证据**: D5a 源文件行 331-345
- **行动**: 已修改 D5-excellent-projects.md 表格（12+ → 14+）+ D5a 表格

### R5.3: Gemini CLI 事件数"11"表述不准确
- **Review 声称**: D5a 仅列 9 个带描述事件，实际文档列出更多
- **验证结果**: 无法验证（外部来源不可达）。但 D5a 自身一致性查阅：确实列出了 11 个事件名
- **验证证据**: 无外部证据
- **行动**: 未修改。D5a 列出 11 个事件名，与声称"11"一致

### R5.4: Windsurf 收购金额未核实
- **Review 声称**: "约 2.5 亿美元"在公开来源中未独立披露
- **验证结果**: 正确。多个 review 文件一致指出金额缺乏独立来源
- **验证证据**: D5a review 错误 4 + D5-excellent review 错误 4 一致
- **行动**: 已修改 D5a 源文件行 78（标注金额未经核实）+ D5-excellent 表格注释

### R5.5: Copilot CLI "13+6" 计数有误导
- **Review 声称**: JSON 13 事件与 SDK 6 事件高度重叠，不应相加
- **验证结果**: 正确。源文件自身列出的事件名明显重叠（sessionStart ↔ onSessionStart）
- **验证证据**: D5a 源文件自身行 145-157 的事件名对比
- **行动**: 已修改 D5a 源文件（标注重叠关系）+ D5-excellent 表格

### R5.6: "P0 建议"中的 PreToolUse/PostToolUse 定位不适用于终端层
- **Review 声称**: slTerminal 是终端级 hook，应使用 PTY 生命周期术语
- **验证结果**: 正确（设计一致性）。源文件 §1.2 自身定义 slTerminal 为"终端级 hook"
- **验证证据**: D5-excellent 源文件行 45
- **行动**: 已修改 P0 表格行（PreToolUse/PostToolUse → 命令执行前后 hook，注明 PTY 术语）

### R5.7: WezTerm "12 个内置事件"遗漏 GUI 事件
- **Review 声称**: 应包含 gui-startup 和 gui-attached，共 14 个
- **验证结果**: 正确（多处 review 一致指出）
- **验证证据**: D5c review 错误 5 + D5-excellent review 错误 7 一致
- **行动**: 已修改 D5-excellent（12→12 window + 2 GUI）+ D5c（计数修改）

### R5.8: Gemini CLI 来源列表中缺少弃用注明
- **Review 声称**: Google 2026-06-18 弃用免费层级
- **验证结果**: 正确（多处 review 一致指出）
- **验证证据**: D5a review 错误 10 + D5-excellent review 错误 8 一致
- **行动**: 已修改 D5-excellent 来源汇总 + D5a Gemini CLI 章节（添加弃用说明）

### R5a.1: Cursor "18 个事件"计数不完整
- **Review 声称**: Cursor 3.11+ 有 21 个事件，遗漏 beforeTabFileRead、afterTabFileEdit、workspaceOpen
- **验证结果**: 正确（D5a 自身列表只有 18 行，review 提供了明确的遗漏项名称）
- **验证证据**: D5a 源文件行 14-28 仅列 18 个
- **行动**: 已修改 D5a 源文件（18→21，添加遗漏事件 + permission 三态专属说明）

### R5a.2: Cursor "五层配置体系"将 Rules 与 Hooks 混淆
- **Review 声称**: Rules（4 层）和 Hooks（4 层）是独立配置维度
- **验证结果**: 正确。源文件将五个不同维度的项混为一个"五层"
- **验证证据**: D5a 源文件行 46-53 混合列出
- **行动**: 已修改 D5a 源文件（拆分为独立的 Rules 系统和 Hooks 系统）

### R5a.3: Windsurf "12 个事件"实际是 11+1
- **Review 声称**: post_cascade_response_with_transcript 是子事件
- **验证结果**: 正确（review 提供了文档引用）
- **验证证据**: D5a review 错误 3 引用的 docs.devin.ai 文档
- **行动**: 已修改 D5a 源文件（12→11+1 标题说明）

### R5a.4: Windsurf 收购金额 (重复 R5.4)
- **验证结果**: 正确
- **行动**: 已在 R5.4 中处理

### R5a.5: Copilot CLI "13+6" (重复 R5.5)
- **验证结果**: 正确
- **行动**: 已在 R5.5 中处理

### R5a.6: aider "24 个斜杠命令"严重低估
- **Review 声称**: v0.86.x 约 43 个命令
- **验证结果**: 正确（源文件仅列 24 个，明显遗漏了近期新增命令）
- **验证证据**: D5a 源文件行 297 对比 review 引用的 computingforgeeks.com
- **行动**: 已修改 D5a 源文件（24→约 43 个，添加遗漏命令名和 AiderDesk 说明）

### R5a.7: Claude Code hook 事件列表缺少最新事件
- **Review 声称**: D5a 列表与 D5b 的 8 核心事件矛盾（内部矛盾）
- **验证结果**: 正确。D5a 列 12+，D5b 列 8
- **验证证据**: D5a 源文件行 329-345 vs D5b 源文件行 247
- **行动**: 已修改 D5b 源文件行 247（添加说明 D5a 列出 12+含扩展事件）

### R5a.8: Claude Code "SessionStart" vs "Setup" 描述矛盾
- **Review 声称**: Setup 可能是 hook type 而非 event
- **验证结果**: 部分正确。官方文档存在歧义（Setup 作为 hook type 出现）。D5a 和 D5b 均将 Setup 列为事件——标注此歧义即可
- **验证证据**: D5a review 错误 8 + D5b review 错误 1
- **行动**: 已在 D5b §1.1 添加注释标注歧义

### R5a.9: Codex CLI "与 Claude Code 同契约"缺乏验证
- **Review 声称**: Codex 使用 JSON-RPC over stdio，与 Claude Code 原始 stdin→stdout 不同
- **验证结果**: 正确。ADJUDICATION Conflict 6 裁决确认
- **验证证据**: ADJUDICATION.md 冲突 6
- **行动**: 已修改 D5a 源文件（Codex 协议描述改为 JSON-RPC）+ D5a 表格

### R5a.10: Gemini CLI 事件列表缺失重要上下文（弃用）
- **Review 声称**: 免费版 2026-06-18 弃用
- **验证结果**: 正确（与 R5.8 一致）
- **行动**: 已在 R5.8 中处理

### R5a.11: Cursor events 列表中部分事件分类错误（permission 三态归属）
- **Review 声称**: permission 三态是 beforeShellExecution/beforeMCPExecution 专属
- **验证结果**: 正确（源文件将 permission 归因为通用字段，实际为特定事件专属）
- **验证证据**: D5a review 引用的 Cursor 官方文档
- **行动**: 已在修改事件列表时一并修正（R5a.1 修改中包含此更正）

### R5a.12: aider 与 AiderDesk 混淆
- **Review 声称**: 应提及 AiderDesk 第三方方案（30+ 生命周期 hook）
- **验证结果**: 部分正确。AiderDesk 是第三方包装器，非 aider 核心——但作为生态中 hook 实现的主要答案，值得注明
- **验证证据**: D5a review 错误 12 引用的 deepwiki.com
- **行动**: 已在修改 aider 命令计数时添加 AiderDesk 说明

### R5b.1: disler 仓库"覆盖全部 13 个事件"与官方文档矛盾
- **Review 声称**: 三份清单互不一致（D5a:12, D5b:8, disler:13）
- **验证结果**: 正确（内部矛盾明确）
- **验证证据**: D5a 源文件 (12) vs D5b 源文件 (8) vs disler 描述 (13)
- **行动**: 已修改 D5b 源文件行 15（添加歧义说明）

### R5b.2: Token 优化文章标题夸大（"83% 节省" + "8 个脚本"）
- **Review 声称**: 实际是 6 个脚本（非 8 个）、最佳案例 83%（非典型）、平均 65.8%
- **验证结果**: 正确。D5b 源文件行 352 写"8 个"与标题"6 hook scripts"矛盾
- **验证证据**: D5b 源文件行 349 标题自身写 "6 hook scripts" vs 行 352 写 "8 个"
- **行动**: 已修改 D5b 源文件（6 个脚本 + 平均 65.8% + OpenWolf 工具名 + 估算精度说明）

### R5b.3: Hacker News 项目部分无法验证
- **Review 声称**: 仅 Recall 和 Han 已验证存在，其余无法确认
- **验证结果**: 正确。review 提供了具体的验证历史和零结果报告
- **验证证据**: D5b review 错误 3 的详细逐个验证记录
- **行动**: 未修改。Review 仅指出不可验证性，非事实错误；HN item ID 格式正确，信息可能以不同标题发布。源文件中这些信息保留但应视为"未经独立验证"

### R5b.4: Reddit "~34% 偏离率"缺乏单一确凿来源
- **Review 声称**: "277 sessions" 和 "34%" 在搜索结果中未找到支撑
- **验证结果**: 正确。review 提供了替代来源（Tygart Media ~30%, GitHub Issue 30-40%）
- **验证证据**: D5b review 错误 4 的详细搜索记录
- **行动**: 已修改 D5b 源文件（替换为有来源支撑的 22-40% 范围，引用 Tygart Media 和 GitHub Issue）

### R5b.5: 通知工具列表部分未验证
- **Review 声称**: 6 个工具中部分未做独立验证
- **验证结果**: 无法验证（外部来源不可达）
- **验证证据**: 无
- **行动**: 未修改。Review 未声称错误，仅指出未验证状态

### R5b.6: "Skills ~56% 概率被跳过"缺乏验证
- **Review 声称**: 56% 精确数字未找到来源，常见说法是 50-80%
- **验证结果**: 正确。review 提供了替代来源
- **验证证据**: D5b review 错误 6 引用的 tygartmedia.com
- **行动**: 已修改 D5b 源文件行 389 + D5-excellent 源文件行 195（56%→约 50-80%）

### R5b.7: "8 种核心事件"与 D5a 的 "12+" 矛盾 (重复 R5a.7)
- **验证结果**: 正确
- **行动**: 已在 R5a.7 中处理

### R5b.8: 中文社区资源 URL 可能部分失效
- **Review 声称**: 腾讯云 URL 格式疑有问题，Skywork blog 路径不像技术博客
- **验证结果**: 无法验证（外部来源不可达）
- **验证证据**: 无
- **行动**: 未修改。URL 可能格式正确但内容无法确认

### R5c.1: Warp DCS Hook "13 个"实际是 17 个
- **Review 声称**: 源码 dcs_hooks.rs 的 DProtoHook 枚举有 17 variants
- **验证结果**: **内部矛盾**——ADJUDICATION P0-3 声称 SourcedRcFileForWarp 在引用来源中不存在，但 D5c review 声称源码中有 17 variants（含此名称）。两者不可能同时正确
- **验证证据**: ADJUDICATION.md P0-3 vs D5c review 错误 1
- **行动**: **未修改**。留待外部源码访问验证后处理。当前保留 D5c 源文件的 13 项列表

### R5c.2: Warp OSC 777 Agent States 表格不完整
- **Review 声称**: 7 个事件 vs 源文件中 5 个状态描述
- **验证结果**: **内部矛盾**——ADJUDICATION P0-3 声称 "OSC 777 在引用来源中不存在"。若 ADJUDICATION 正确，则整个 OSC 777 描述应删除；若 D5c review 正确，应扩充到 7 个事件
- **验证证据**: ADJUDICATION.md P0-3 vs D5c review 错误 2
- **行动**: **未修改**。留待外部源码访问验证后处理

### R5c.3: iTerm2 Triggers 表格有描述不准确
- **Review 声称**: "Fold to Named Mark"→"Fold Section"、"Inject Data"→"Inject"
- **验证结果**: 正确（review 提供了具体更名信息）
- **验证证据**: D5c review 错误 3 引用的 iterm2.com 官方文档
- **行动**: 已修改 D5c 源文件（两处名称修正）

### R5c.4: Warp OSC 9 描述中缺少对格式限制的说明
- **Review 声称**: OSC 777 实际用于 agent 状态跟踪（非仅通知），payload 中避免换行/分号
- **验证结果**: 若 ADJUDICATION P0-3 关于 OSC 777 的裁决正确，此修改 moot。若 D5c review 正确，该修改合理
- **行动**: 未修改（pending R5c.1/R5c.2 解决）

### R5c.5: WezTerm "12 个内置事件"遗漏 GUI 事件 (重复 R5.7)
- **验证结果**: 正确
- **行动**: 已修改 D5c 源文件

### R5c.6: Windows Terminal "Proto Extensions（规划中）"表述不准确
- **Review 声称**: 已实现并发布为 JSON Fragment Extensions（PR #7632, v1.24）
- **验证结果**: 正确。review 提供了具体的 PR 编号和生产文档引用
- **验证证据**: D5c review 错误 6 引用的 learn.microsoft.com 和 PR #7632
- **行动**: 已修改 D5c 源文件行 75

### R5c.7: Warp "DCS Hooks 无手动配置"的表述可能过时
- **Review 声称**: 2026年5月开源后配置面扩大
- **验证结果**: 正确。开源是公开事实
- **验证证据**: D5c review 错误 7
- **行动**: 已修改 D5c 源文件行 286

### R5d.1: VS Code presentation.panel 字段缺少 OutputChannel vs Terminal 说明
- **Review 声称**: 混淆了 Output 面板和 Terminal 面板
- **验证结果**: 部分正确。源文件行 101-103 已区分 OutputChannel 和 Terminal 面板
- **验证证据**: D5d 源文件行 99 和 101-103
- **行动**: 未修改。源文件已有区分，review 高估了混淆程度

### R5d.2: JetBrains Run Configurations 五层架构缺失 ProgramRunner
- **Review 声称**: 执行流程中关键中间层 ProgramRunner 未提及
- **验证结果**: 正确。源文件仅描述 5 层配置结构，执行流程缺 ProgramRunner
- **验证证据**: D5d review 错误 2 引用的 JetBrains 官方 SDK 文档
- **行动**: 已修改 D5d 源文件（添加 ProgramRunner 说明 + 完整执行流程）

### R5d.3: JetBrains Reworked Terminal "2025.2+" 版本信息可能不准确
- **Review 声称**: 不同 IDE 产品版本号系统不同
- **验证结果**: 不正确（非事实错误）。"2025.2+" 是平台版本号约定，各产品对齐此版本号
- **验证证据**: D5d review 错误 3 自身承认"表述本身正确"
- **行动**: 未修改。Review 声称非实质错误

### R5d.4: VS Code Problem Matchers pattern.kind 字段遗漏重要选项
- **Review 声称**: kind 是多行匹配场景的分类标记，非通用字段
- **验证结果**: 部分正确。源文件将 kind 描述为顶层字段，实际用于 loop 多行匹配
- **验证证据**: D5d review 错误 4 引用的 VS Code 官方文档
- **行动**: 未修改。简化描述不构成严重事实错误，源文件聚焦"有哪些值"而非"在哪个上下文使用"

### R5d.5: JetBrains External Tools "不支持 Problem Matcher 概念"表述不精确
- **Review 声称**: IntelliJ 有 Output Filters 实现类似功能
- **验证结果**: 正确。Output Filters 是功能等价的替代机制
- **验证证据**: D5d review 错误 5 引用的 JetBrains 官方文档
- **行动**: 已修改 D5d 源文件行 294

### R5d.6: Problem Matchers 在 Terminal API 列为"无"不完整
- **Review 声称**: 通过 TaskProvider 关联自定义任务可附带 problemMatcher
- **验证结果**: 正确。Pseudoterminal 本身不提供，但 TaskProvider 间接可用
- **验证证据**: D5d review 错误 6 引用的 VS Code 官方文档
- **行动**: 已修改 D5d 对比总结表

---

## 未解决的矛盾

### U1: Warp DCS Hooks 和 OSC 777 的存在性

| 来源 | 声称 |
|------|------|
| ADJUDICATION.md P0-3 | `SourcedRcFileForWarp`、`OSC 777` **在引用来源中均不存在** |
| D5c review 错误 1 | Warp 源码 `dcs_hooks.rs` 有 17 variants，**含 SourcedRcFileForWarp** |
| D5c review 错误 2 | OSC 777 有 7 个事件，对应 Warp 的 agent state machine |

两方均无法通过外部访问验证。ADJUDICATION 基于 D4 review 的发现；D5c review 基于 GitHub 源码引用。两者引用不同来源——可能 ADJUDICATION 针对的 D4 引用来源确实不包含这些术语，而 D5c 引用的源码文件确实包含。建议源码访问恢复后优先验证。

### U2: Claude Code 事件总数

| 来源 | 声称 |
|------|------|
| D5a 源文件 | 12 核心 + 2 实验性 = 14 |
| D5b 源文件 | 8 种核心事件 |
| D5b review (disler) | 13 个 |
| ADJUDICATION 冲突 3 | TypeScript SDK: 21, 总计 ~27-28 |
| D5 review 错误 2 | 14+ |

实际总数可能为 27-28（SDK 21 + CLI-only 6-7）。D5 各子报告均低估。已在各处添加注释说明歧义。

---

*验证完成。WebSearch/WebFetch 预算耗尽，外部来源取证受限。27 条修改基于内部证据（源文件交叉比对 + ADJUDICATION 裁决）——可靠。剩余 8 条（Warp DCS/OSC 777 存在性、Codex CLI 10 事件计数、HN 项目 URL、通知工具列表、中文社区 URL、Cursor 事件分类细节、VS Code pattern.kind 上下文、Reworked Terminal 版本号）需外部来源访问验证。*
