# D5d：IDE Task/Extension 系统 UI 参考

> 研究日期：2026-07-25

## 1. VS Code

### 1.1 Tasks 系统

#### tasks.json 设计

VS Code 的 Task 系统以 `.vscode/tasks.json` 为核心配置文件，格式版本 `"2.0.0"`。每个 task 是一个 JSON 对象，核心字段如下：

| 字段 | 类型 | 说明 |
|------|------|------|
| `label` | string | 任务唯一标识/显示名 |
| `type` | string | 任务类型：`"shell"`（默认）、`"process"`、或自定义 type |
| `command` | string | 要执行的命令/程序 |
| `args` | string[] | 命令行参数 |
| `options` | object | 运行时选项：`cwd`（工作目录）、`env`（环境变量） |
| `presentation` | object | 输出面板行为：`reveal`（always/silent/never）、`echo`、`focus`、`panel`（shared/dedicated/new）、`showReuseMessage`、`clear`、`group` |
| `group` | object | 任务分组：`kind`（build/test/none）、`isDefault` |
| `dependsOn` | string[] | 前置依赖任务（并行执行） |
| `dependsOrder` | string | 依赖顺序：`"parallel"` / `"sequence"` |
| `isBackground` | boolean | 是否为持续运行的后台任务 |
| `problemMatcher` | string\|string[] | 关联的 problem matcher |
| `runOptions` | object | 自动运行选项：`runOn: "folderOpen"` |

**三种执行类型：**
- **ShellExecution** -- 通过 shell 执行命令字符串，支持变量替换
- **ProcessExecution** -- 直接执行可执行文件（不经过 shell）
- **CustomExecution** -- 回调返回 `Pseudoterminal`，完全控制输入输出

**task 的 group 分类：**
- `"build"` -- 构建任务（`Ctrl+Shift+B` 触发默认构建）
- `"test"` -- 测试任务
- 无 group -- 普通任务，仅通过命令面板 `Run Task` 触发

来源：VS Code 官方文档 `code.visualstudio.com/docs/editor/tasks`，通过 Context7 `/prudhvi-dev9/vscode-docs-api`，2026-07-25

#### Problem Matchers

Problem Matcher 将任务输出中的编译/检查错误解析为 VS Code **Problems 面板**中的结构化条目（文件、行、列、严重级别、消息）。

**核心结构（JSON 定义）：**
```json
{
  "name": "gcc",
  "owner": "cpp",
  "fileLocation": ["relative", "${workspaceFolder}"],
  "pattern": {
    "regexp": "^(.*):(\\d+):(\\d+):\\s+(warning|error):\\s+(.*)$",
    "file": 1,     // 正则捕获组 1 → 文件路径
    "line": 2,     // 捕获组 2 → 行号
    "column": 3,   // 捕获组 3 → 列号
    "severity": 4, // 捕获组 4 → 严重级别 (warning/error)
    "message": 5   // 捕获组 5 → 消息文本
  }
}
```

**pattern 的字段说明：**
- `regexp` -- 正则表达式，匹配输出行
- `file` / `line` / `column` / `severity` / `message` / `code` -- 对应捕获组编号（1-based）
- `loop` -- 多行匹配时作为循环起始标记
- `kind` -- `"file"` 或 `"location"`

**注册方式：**
- **扩展贡献**：`package.json` 的 `contributes.problemMatchers`，定义命名 matcher（如 `$gcc`）
- **tasks.json 内联**：直接在 task 内定义匿名 pattern
- **预置 matcher**：VS Code 内置 `$tsc`、`$tsc-watch`、`$esbuild`、`$esbuild-watch`、`$msCompile` 等

**后台任务 watching matcher：**
对于 `isBackground: true` 的任务，需要后台 problem matcher：
- 用 `background.activeOnStart: true` 标记任务启动完成
- 用 `beginsPattern` / `endsPattern` 匹配启动/结束信号
- 预置：`$tsc-watch`、`$esbuild-watch`

来源：VS Code 官方文档 <https://code.visualstudio.com/docs/editor/tasks>，Context7，2026-07-25

#### Output Panel

任务的输出可以通过 `presentation` 配置控制显示行为：

| 选项 | 值 | 效果 |
|------|----|------|
| `reveal` | `"always"` | 任务开始时总是显示输出面板 |
| | `"silent"` | 仅在无 problem matcher 时显示 |
| | `"never"` | 从不自动显示（用户手动打开） |
| `focus` | boolean | 是否聚焦到终端/输出面板 |
| `panel` | `"shared"` | 所有任务共享同一面板 |
| | `"dedicated"` | 每种任务类型独享面板 |
| | `"new"` | 每次执行新建面板 |
| `clear` | boolean | 运行前清空输出 |
| `echo` | boolean | 是否回显命令本身 |
| `group` | string | 按 group 名称分组显示 |
| `showReuseMessage` | boolean | 面板复用时显示提示 |

**Channel 机制：**
VS Code 不是为每个 task 创建独立输出面板，而是将输出定向到关联的 **Terminal** 实例。`panel: "dedicated"` 时，不同 task type 使用不同 terminal tab。这与 IDE 风格的 "Output" 工具窗口有本质区别 -- VS Code 的任务输出就是终端输出。

**扩展输出通道（OutputChannel）：**
扩展可通过 `vscode.window.createOutputChannel("name")` 创建纯文本输出通道，支持 `appendLine`、`show`、`hide`、`clear`、`dispose`。此通道显示在 Output 面板下拉菜单中，与 Terminal 面板不同。

来源：<https://code.visualstudio.com/docs/editor/tasks>，<https://code.visualstudio.com/api/extension-capabilities/common-capabilities>，Context7，2026-07-25

#### 终端集成

任务执行直接集成 VS Code 终端系统：
- `ShellExecution` / `ProcessExecution` 的任务输出直接渲染到终端
- 终端支持 ANSI 颜色、光标控制、scrollback
- 任务可设置 `presentation.panel` 决定使用哪个终端实例
- 任务终止：终端内 `Ctrl+C` 或命令面板 `Terminate Task`

**Task Provider API（扩展自定义任务类型）：**
```typescript
// 1. package.json 声明 task type
"contributes": {
  "taskDefinitions": [{ "type": "myTaskType", "command": "...", "default": true }]
}

// 2. 实现 TaskProvider
class MyTaskProvider implements vscode.TaskProvider {
  provideTasks(token): Thenable<Task[]> { ... }
  resolveTask(task, token): Thenable<Task> { ... }
}

// 3. 注册
vscode.tasks.registerTaskProvider('myTaskType', new MyTaskProvider());
```

**CustomExecution + Pseudoterminal（最灵活）：**
扩展可为 task 提供自定义 `Pseudoterminal`，完全接管输入/输出：
- `Pseudoterminal` 接口：`onDidWrite`（事件发射输出）、`handleInput(data)`（用户键盘输入）、`setDimensions(dim)`（终端尺寸变化）、`open(initialDimensions)` / `close()`
- 自定义 task 可跨 run 保持状态（如增量构建缓存）
- 输出直接渲染到 VS Code 终端 UI，表现和原生 task 一致

来源：<https://code.visualstudio.com/api/extension-guides/task-provider>，Context7，2026-07-25

### 1.2 Extension UI

#### Extension 面板

VS Code 的扩展管理通过 **Extensions 视图**（侧栏）呈现：
- 搜索、安装、卸载、启用/禁用扩展
- 每个扩展显示：名称、描述、版本、发布者、下载数、评分
- 详情页包含：README、功能贡献列表、设置项、快捷键、许可证
- 扩展通过 `package.json` 的 `contributes` 字段声明功能贡献

#### 设置编辑器

VS Code 提供两种设置界面：
- **Settings UI**（图形编辑器）：分组的表单编辑器，搜索过滤，自动补全
- **settings.json**（JSON 编辑器）：直接编辑 JSON，带 Schema 校验和自动补全

扩展通过 `package.json` 的 `contributes.configuration` 定义设置项：
```json
"contributes": {
  "configuration": {
    "title": "My Extension",
    "properties": {
      "myExtension.enableFeature": {
        "type": "boolean",
        "default": true,
        "description": "Enable feature X"
      }
    }
  }
}
```

#### 通知/状态栏

**通知系统：**
- `vscode.window.showInformationMessage(msg, ...items)` -- 信息
- `vscode.window.showWarningMessage(msg, ...items)` -- 警告
- `vscode.window.showErrorMessage(msg, ...items)` -- 错误
- `vscode.window.withProgress(options, callback)` -- 进度通知
- 通知位置：`ProgressLocation.Notification` / `Window` / `SourceControl`

**状态栏（StatusBarItem）：**
```typescript
const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
item.text = "$(zap) My Extension";    // $(icon-id) 引用产品图标
item.tooltip = "Click to run";
item.command = "extension.command";
item.color = new vscode.ThemeColor('errorForeground');
item.show();  // item.hide()
```
- 状态栏项可显示图标（Codicon）、文本、背景色
- 对齐：`StatusBarAlignment.Left` / `Right`
- 优先级：数字越大越靠左
- 在 `activate()` 中创建，push 到 `context.subscriptions`

来源：<https://code.visualstudio.com/api/extension-capabilities/extending-workbench>，<https://code.visualstudio.com/api/ux-guidelines/status-bar>，Context7，2026-07-25

### 1.3 Terminal Profiles & API

**终端 Profiles 系统：**
VS Code 允许定义多种终端配置（profiles），在终端下拉菜单中切换：
```json
// settings.json 中定义
"terminal.integrated.profiles.windows": {
  "PowerShell": { "source": "PowerShell", "icon": "terminal-powershell" },
  "Git Bash": { "path": "C:\\Git\\bin\\bash.exe", "args": ["-l"] },
  "Command Prompt": { "path": "${env:windir}\\System32\\cmd.exe" }
}
```

**扩展贡献终端 Profile：**
```json
// package.json
"contributes": {
  "terminal": {
    "profiles": [
      { "title": "Profile from extension", "id": "my-ext.terminal-profile" }
    ]
  }
}
```

```typescript
// 注册 provider 以返回 TerminalOptions 或 ExtensionTerminalOptions
vscode.window.registerTerminalProfileProvider('my-ext.terminal-profile', {
  provideTerminalProfile(token) {
    return { name: 'Profile from extension', shellPath: 'bash' };
    // 或 { name: 'Custom PTY', pty: new MyPseudoterminal() }
  }
});
```

**TerminalOptions vs ExtensionTerminalOptions：**
- `TerminalOptions`：使用 shell（`shellPath`, `args`, `cwd`, `env`, `name`, `color`, `icon`, `message`, `hideFromUser`, `isTransient`）
- `ExtensionTerminalOptions`：使用自定义 `Pseudoterminal`（`pty`, `name`, `icon`, `isTransient`）

**终端 API：**
```typescript
// 创建终端
const term = vscode.window.createTerminal('My Terminal');
term.show();
term.sendText('echo hello');

// 创建带 options 的终端
const term2 = vscode.window.createTerminal({
  name: 'Build Output',
  shellPath: 'pwsh.exe',
  cwd: workspace.workspaceFolders[0].uri.fsPath,
  env: { NODE_ENV: 'production' }
});

// 创建 Pseudoterminal 终端
const term3 = vscode.window.createTerminal({
  name: 'My Custom Terminal',
  pty: {
    onDidWrite: writeEmitter.event,
    open: () => writeEmitter.fire('Terminal opened\r\n'),
    close: () => { /* cleanup */ },
    handleInput: (data) => { /* process keystrokes */ }
  }
});

// 事件监听
vscode.window.onDidOpenTerminal(t => {});
vscode.window.onDidCloseTerminal(t => {});
vscode.window.onDidChangeActiveTerminal(t => {});
vscode.window.terminals; // 所有终端实例
```

来源：<https://code.visualstudio.com/api/references/vscode-api#Terminal>，<https://code.visualstudio.com/api/references/contribution-points#contributes.terminal>，Context7，2026-07-25

---

## 2. JetBrains IDEs

### 2.1 External Tools

JetBrains IDE 的 External Tools 系统位于 **Settings | Tools | External Tools**，是经过 UI 配置的"外部程序启动器"：

**配置项：**
- **Program** -- 可执行文件路径（支持宏/macro 变量如 `$FilePath$`, `$ProjectFileDir$`）
- **Arguments** -- 命令行参数（支持宏）
- **Working directory** -- 工作目录（支持宏）
- **Advanced Options** -- 环境变量、输出过滤等

**与 IDE 集成方式：**
- External Tools 可通过 **Tools 菜单**、**右键菜单**、**快捷键**触发
- 输出显示在 **Run 工具窗口**（Run tool window）的 Console 中
- Keymap 中可为每个 External Tool 绑定快捷键
- 配置使用 IDE 内置的宏变量系统传递上下文（文件路径、项目路径、选择文本等）

**限制：**
- 静态 JSON 式/表单式配置，非编程接口
- 仅支持"启动→等待完成"模式（无后台/持续运行语义）
- 输出是普通控制台文本，不支持结构化问题解析（需自行实现）
- 无 VS Code 风格的 Problem Matchers（regex→结构化条目映射到 Problems 面板），但可通过 **Output Filters**（Run Configuration 的 Logs 标签页）实现类似功能：regex 匹配输出行并转为可点击导航链接。External Tools 的 Output Filters 能力有限（不如完整的 Run Configurations Logs 系统）

来源：JetBrains 官方文档 `jetbrains.com/help/idea/settings-tools-external-tools.html`，2026-07-25（基于个人知识补充，因 WebFetch 受阻）

### 2.2 File Watchers

File Watchers 是 JetBrains 的一项内置功能（非 External Tools），用于在文件保存时自动执行外部工具：

**设计要点：**
- 监听文件系统变更（保存事件）→ 自动执行配置的程序
- 典型用途：编译 SCSS/LESS、压缩 JS、代码格式化、lint
- 配置界面在 **Settings | Tools | File Watchers**
- 每个 File Watcher 指定：
  - **File type** -- 触发文件类型
  - **Scope** -- 监听范围（项目文件/指定 scope）
  - **Program** -- 执行程序
  - **Arguments** -- 参数（宏变量）
  - **Working directory**
  - **Output paths to refresh** -- 输出后自动刷新的路径
  - **Immediate file synchronization** -- 立即同步
  - **Show console** -- 控制台行为（always/never/on error）

**与 External Tools 的区别：**
- File Watchers 是**自动触发**（保存时），External Tools 是**手动触发**
- File Watchers 有明确的"文件类型→工具"映射
- 输出同样到 Run tool window

来源：JetBrains 官方文档 `jetbrains.com/help/idea/settings-tools-file-watchers.html`，2026-07-25（基于个人知识补充）

### 2.3 Run Configurations

Run Configurations 是 IntelliJ Platform 最核心的"任务执行"系统，比 VS Code tasks.json 更结构化且类型化。

**五层架构（自下而上，执行流程中还有关键的 ProgramRunner 层）：**

| 层级 | 类/接口 | 职责 |
|------|---------|------|
| 1. ConfigurationType | `ConfigurationTypeBase` | 声明运行配置类型（ID、名称、图标） |
| 2. ConfigurationFactory | `ConfigurationFactory` | 创建模板配置 + 定义持久化 options class |
| 3. RunConfiguration | `RunConfigurationBase<Options>` | 具体配置实例，持持久化参数，提供 SettingsEditor |
| 4. SettingsEditor | `SettingsEditor<T>` | UI 表单编辑界面（JPanel 构建） |
| 5. RunProfileState | `CommandLineState` | 执行状态：构建命令行 → 启动进程 → 返回 ProcessHandler |

完整执行流程：Executor → **ProgramRunner**（负责选择哪个 runner 处理该配置类型） → ExecutionEnvironment → RunProfile.getState() → RunProfileState.execute()。`CommandLineState.startProcess()` 通过 `RunProfileState.execute(executor, runner)` 间接调用。

**注册（plugin.xml）：**
```xml
<extensions defaultExtensionNs="com.intellij">
  <configurationType implementation="com.example.DemoRunConfigurationType"/>
</extensions>
```

**执行流程：**
```
用户点击 Run → Executor(Run/Debug/Coverage) → ExecutionEnvironment
  → RunProfile.getState(executor, env) → RunProfileState
  → CommandLineState.startProcess()
  → GeneralCommandLine(command).withEnvironment(env).withWorkDirectory(cwd)
  → OSProcessHandler(commandLine)  // 管理进程 I/O + 生命周期
  → ExecutionConsole  // 在 Run tool window 显示输出
```

**关键类型：**
- `GeneralCommandLine` -- 构造命令行（exe、args、env、workDir、charset、redirectErrorStream）
- `OSProcessHandler` -- 进程句柄：start/kill/destroy/waitFor、notifyTextAvailable（输出回调）、addProcessListener
- `ProcessHandlerFactory.createColoredProcessHandler` -- 带 ANSI 颜色支持的进程处理器
- `ProcessTerminatedListener.attach(handler)` -- 进程退出后显示退出码
- `RunContentBuilder` -- 构造 Run tool window 内容（console + toolbar）

**持久化：**
Run Configurations 以 XML 存储（`.idea/runConfigurations/` 目录），支持 share 到 VCS。Options class（`BaseState` 子类）自动序列化/反序列化字段。

**三种 Executor：**
- `Executor.EXECUTOR_RUN_ID` -- 运行
- `Executor.EXECUTOR_DEBUG_ID` -- 调试
- `Executor.EXECUTOR_COVERAGE_ID` -- 代码覆盖率运行

来源：<https://plugins.jetbrains.com/docs/intellij/run-configurations.html>，<https://plugins.jetbrains.com/docs/intellij/execution.html>，Context7，2026-07-25

### 2.4 终端集成

IntelliJ Platform 2025.2+ 以 **Reworked Terminal** 为默认终端实现（替代 Classic Terminal）。

**三种终端实现：**
| 实现 | 状态 | 说明 |
|------|------|------|
| Classic Terminal | 保留 | 旧默认，功能完整 |
| Reworked Terminal | 默认 (2025.2+) | 新架构，API 推荐 |
| Experimental Terminal | 废弃 | 已被 Reworked 取代 |

**Reworked Terminal API 核心接口：**

**获取终端实例：**
```java
// 从 DataContext 获取当前终端视图
TerminalView view = TerminalView.DATA_KEY.getData(dataContext);

// 获取所有终端 tabs
TerminalToolWindowTabsManager.getTabs();

// 创建新 tab
TerminalToolWindowTabsManager.createTabBuilder()
    .withTitle("My Tab")
    .withWorkingDirectory("/path/to/project")
    .show();
```

**向终端发送命令：**
```java
// 发送文本到终端（推荐使用 Builder）
terminalView.createSendTextBuilder("ls -l")
    .shouldExecute()  // 自动追加换行
    .send();
```

**读取终端输出：**
```java
// 获取输出模型（只读）
TerminalOutputModelsSet outputModels = terminalView.outputModels;
TerminalOutputModel regularBuffer = outputModels.getRegular();       // 常规缓冲
TerminalOutputModel alternativeBuffer = outputModels.getAlternative(); // 交替缓冲
// TerminalOutputModel 提供绝对偏移导航（因历史裁剪）
```

**终端 Shell 命令处理扩展：**
实现 `TerminalShellCommandHandler` 并在 `plugin.xml` 注册，可拦截终端中执行的命令：
```xml
<terminal.shellCommandHandler implementation="com.example.MyHandler"/>
```

**Run Anything 集成：**
`Run Anything` 弹出窗（双 Ctrl）支持直接输入并执行命令，输出到 Run tool window。

来源：<https://plugins.jetbrains.com/docs/intellij/embedded-terminal.html>，Context7，2026-07-25

### PS：External System Integration（外部构建系统）

IntelliJ Platform 的 External System 子系统专门支持 Maven/Gradle/sbt 等：
- 提供 API 包装外部系统元素
- 暴露任务列表（Tasks），支持执行
- `ExternalSystemTaskManager` / `ExternalSystemTaskExecutionManager`
- 独立于 Run Configurations 的任务执行体系

来源：<https://plugins.jetbrains.com/docs/intellij/external-system-integration.html>，Context7，2026-07-25

---

## 对比总结

| 维度 | VS Code Tasks | VS Code Terminal API | JetBrains Run Configurations | JetBrains Embedded Terminal |
|------|--------------|---------------------|------------------------------|------------------------------|
| **配置方式** | JSON（tasks.json） | JSON（profiles）+ 编程 API | Java UI 表单 + XML 持久化 | 编程 API + UI |
| **任务类型** | shell / process / custom（Pseudoterminal） | shell / extension pty | 强类型 RunConfiguration 子类 | shell 命令 |
| **输出面板** | 终端面板（terminal tab） | 终端面板 | Run tool window（ConsoleView） | 终端 tool window |
| **问题解析** | Problem Matchers（正则 → Problems 面板） | Pseudoterminal 本身不提供，但通过 TaskProvider 关联自定义 task 可附带 problemMatcher | 无内置机制（可通过 Output Filters regex 实现类似导航功能） | 无 |
| **后台执行** | `isBackground: true` + watching matcher | Pseudoterminal | ProcessHandler（事件驱动） | 终端进程管理 |
| **依赖管理** | `dependsOn`（parallel/sequence） | 无声明式依赖 | 无声明式依赖 | 无 |
| **触发器** | 手动 / `runOn: folderOpen` / `Ctrl+Shift+B` | 手动 / profile 选择 | 手动 / 快捷键 / 右键菜单 | 手动 / 启动脚本 |
| **可扩展性** | TaskProvider API + CustomExecution + Pseudoterminal | TerminalProfileProvider + Pseudoterminal | ConfigurationType + RunConfiguration + ProgramRunner | TerminalShellCommandHandler |
| **进程管理** | vs.task.onDidEndTask 事件 | Pseudoterminal.close() | OSProcessHandler（start/kill/destroy/waitFor） | TerminalWidget 管理 |
| **UI 粒度** | 任务 → 终端 tab | 终端 profile → 终端 tab | 配置 → Run tool window tab | 终端 tab → shell |
| **持久化** | JSON 文件（可 VCS 共享） | settings.json | XML（.idea/runConfigurations/） | Tab 会话恢复 |

**核心设计哲学差异：**
- **VS Code**：以 JSON 声明式配置为核心，扩展性通过"TaskProvider + Pseudoterminal"编程注入，输出即终端，Problem Matcher 连接 Tasks 和 Problems 面板
- **JetBrains**：以类型化 Java API 为核心，RunConfiguration 是强类型配置类+UI 编辑器+执行器三位一体，输出通过 ConsoleView/ProcessHandler 管理，终端是独立子系统

来源：综合 Context7 `/prudhvi-dev9/vscode-docs-api` + `/websites/plugins_jetbrains_intellij`，2026-07-25
