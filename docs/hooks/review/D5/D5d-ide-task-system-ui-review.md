# D5d IDE Task/Extension 系统 UI -- 事实核查

## 错误 1: VS Code `presentation.panel` 字段缺少 `"dedicated"` 与终端实例的关系说明

- **文件+行号**: `D5d-ide-task-system-ui.md` (行 84-97)
- **原声称**: "panel: 'shared' | 'dedicated' | 'new'"
- **错误类型**: 事实错误（不完整）
- **正确信息**: 
  - `panel` 字段的值描述正确（shared/dedicated/new），但 D5d 后续解释 (行 99) 说 "不是为每个 task 创建独立输出面板，而是将输出定向到关联的 Terminal 实例"——这混淆了 **Output 面板**（OutputChannel）和 **Terminal 面板**的区别
  - VS Code tasks 通过 ShellExecution/ProcessExecution 执行时，输出确实进入 **Terminal** 实例（不是 Output 面板）。Task 输出 = 终端输出，这是正确的
  - 但 `OutputChannel`（扩展通过 `vscode.window.createOutputChannel` 创建）显示在独立的 Output 面板中，与 Terminal 面板是**不同的 UI 区域**。D5d 含混处理了这两者
- **反证来源**: 
  - `code.visualstudio.com/docs/editor/tasks` — tasks 使用 Terminal 面板
  - `code.visualstudio.com/api/extension-capabilities/common-capabilities` — OutputChannel 是独立面板

## 错误 2: JetBrains "Run Configurations 五层架构"描述准确但过度简化

- **文件+行号**: `D5d-ide-task-system-ui.md` (行 329-335)
- **原声称**: 五层架构：ConfigurationType → ConfigurationFactory → RunConfiguration → SettingsEditor → RunProfileState
- **错误类型**: 事实错误（轻微不准确）
- **正确信息**: 
  - 五层结构总体正确，但执行流程中有一个关键中间层未提及：**ProgramRunner**
  - 完整流程是：Executor → ProgramRunner（负责选择哪个 runner 处理该配置类型） → ExecutionEnvironment → RunProfile.getState() → RunProfileState.execute()
  - `RunContentBuilder`（行 359）被列为关键类型，但实际上它是 UI 构造器，非执行管线的核心环节——更准确的核心类型应包含 `ExecutionResult` 和 `ConsoleView`
  - `CommandLineState.startProcess()` 描述为直接调用——实际上是通过 `RunProfileState.execute(executor, runner)` 间接调用，中间经过 `ExecutionEnvironment` 的 runner 分派
- **反证来源**: 
  - `plugins.jetbrains.com/docs/intellij/run-configurations.html` — 官方 SDK 五层 + ProgramRunner
  - `plugins.jetbrains.com/docs/intellij/execution.html` — 执行流程含 ProgramRunner

## 错误 3: JetBrains Reworked Terminal "2025.2+" 版本信息可能不准确

- **文件+行号**: `D5d-ide-task-system-ui.md` (行 373)
- **原声称**: "Reworked Terminal 为默认终端实现（替代 Classic Terminal）——2025.2+"
- **错误类型**: 来源不支撑
- **正确信息**: IntelliJ Platform 2025.2 中 Reworked Terminal 确实成为默认。但具体版本号需验证——不同 JetBrains IDE 产品（IntelliJ IDEA、WebStorm 等）的版本号系统不同（IntelliJ IDEA 2025.2、WebStorm 2025.2 等）。表述本身正确，但引用来源 2026-07-25 若未能直接访问文档则属 "基于个人知识补充"（D5d 行 296 已注明 WebFetch 受阻）
- **反证来源**: 
  - `plugins.jetbrains.com/docs/intellij/embedded-terminal.html` — 官方 Reworked Terminal API 文档

## 错误 4: VS Code Problem Matchers 描述中 `pattern.kind` 字段遗漏重要选项

- **文件+行号**: `D5d-ide-task-system-ui.md` (行 64-65)
- **原声称**: "kind: 'file' 或 'location'"
- **错误类型**: 事实错误（不完整）
- **正确信息**: `pattern.kind` 字段实际支持的值包括 `"file"`（文件级匹配）和 `"location"`（位置级匹配），但这不是 `pattern` 对象的直接字段——它是 `pattern.regexp` 在**多行匹配**场景中，通过 `loop` 字段标记循环起始行后，各行的分类标记。标准单行匹配 pattern 不需要 `kind` 字段。D5d 的描述过于简化
- **反证来源**: 
  - `code.visualstudio.com/docs/editor/tasks` — Problem Matcher 完整 schema

## 错误 5: JetBrains External Tools "不支持 Problem Matcher 概念"表述不精确

- **文件+行号**: `D5d-ide-task-system-ui.md` (行 294)
- **原声称**: "不支持 Problem Matcher 概念"
- **错误类型**: 事实错误（略有误导）
- **正确信息**: JetBrains 没有 VS Code 风格的 Problem Matchers（regex→结构化条目映射到 Problems 面板），但 IntelliJ 通过 **Output Filters**（在 Run Configuration 的 Logs 标签页中配置）实现了类似功能——regex 匹配输出行，将其转为可点击的导航链接。功能等价但 UI 路径不同。此外，External Tools 的输出过滤器确实能力有限（不如 Run Configurations 的完整 Logs 系统）
- **反证来源**: JetBrains `jetbrains.com/help/idea/settings-tools-external-tools.html` — Output Filters 配置

## 错误 6: 对比总结表中 VS Code 的"问题解析"列为"Problem Matchers"，但 Terminal API 列为"无"

- **文件+行号**: `D5d-ide-task-system-ui.md` (行 446)
- **原声称**: VS Code Tasks 列 "Problem Matchers（正则 → Problems 面板）"，Terminal API 列 "无"
- **错误类型**: 事实错误（不完整）
- **正确信息**: VS Code 的 Pseudoterminal API 本身不提供 Problem Matchers（正确）。但通过 Pseudoterminal + TaskProvider API 创建的 custom task 可以**关联** Problem Matchers——task definition 中的 `problemMatcher` 字段同样适用于 custom execution 任务。所以严格说 Pseudoterminal 不提供，但通过 TaskProvider 间接可用
- **反证来源**: 
  - `code.visualstudio.com/api/extension-guides/task-provider` — CustomExecution + TaskProvider 支持 problemMatcher
