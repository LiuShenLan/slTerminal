# VS Code 配置编辑 UI 参考

VS Code 对 tasks.json / launch.json 等 JSON 配置文件的编辑采用了"原始 JSON 为主 + Schema 驱动辅助 + 有限图形化"的混合模式。本文档梳理其设计模式，作为 slTerminal 图形化配置的参考案例。

---

## 1. 总体设计理念

VS Code **没有为 tasks.json 和 launch.json 提供完整的图形化编辑器**。用户直接编辑原始 JSON 文件，通过以下机制降低使用门槛：

| 机制 | 说明 |
|------|------|
| **JSON Schema 驱动 IntelliSense** | `$schema` 声明后自动获得自动补全、悬停文档、字段校验（红色波浪线）|
| **模板生成** | "创建 launch.json 文件"时根据语言自动生成预填模板（Node.js/Python/Chrome 等）|
| **下拉选择器** | 调试面板中 `configurations` 以 dropdown 形式切换 |
| **Command Palette 引导** | `Tasks: Run Task` / `Tasks: Configure Task` 通过命令面板交互式选择 |
| **自动检测** | 扫描项目中的 npm scripts / gulpfile / tsconfig 等，自动生成 task 条目 |

**核心权衡**：JSON 文本编辑在灵活性上远胜图形化表单（支持数十种语言和调试类型），Schema 驱动的 IntelliSense 填补了发现性缺口。

来源：https://code.visualstudio.com/docs/editor/tasks
来源：https://code.visualstudio.com/docs/editor/debugging

---

## 2. tasks.json Schema 参考

### 2.1 完整 Schema 结构

官方 Schema 附录位于：https://code.visualstudio.com/docs/reference/tasks-appendix

#### 顶层 `TaskConfiguration`

```
TaskConfiguration {
  version: "2.0.0"          // 必填，固定值
  windows?: BaseTaskConfiguration   // Windows 平台覆盖
  osx?: BaseTaskConfiguration       // macOS 平台覆盖
  linux?: BaseTaskConfiguration     // Linux 平台覆盖
}

BaseTaskConfiguration {
  type?: "shell" | "process"       // 任务执行类型
  command?: string                   // 执行的命令
  isBackground?: boolean             // 后台任务
  options?: CommandOptions           // 执行选项 (cwd, env, shell)
  args?: string[]                    // 命令参数
  presentation?: PresentationOptions // 输出面板行为
  problemMatcher?: ...               // 问题匹配器
  tasks?: TaskDescription[]          // 任务数组
}
```

#### `TaskDescription`（核心任务定义）

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `label` | string | 是 | 任务名称/标识符 |
| `type` | string | 否 | 任务类型："shell"/"process" 或扩展注册的自定义类型（如 "npm"、"typescript"）|
| `command` | string | 否 | 执行命令 |
| `args` | string[] | 否 | 命令参数（process 类型用）|
| `group` | "build"/"test"/{kind,isDefault} | 否 | 任务分组，isDefault 设为 true 后对应"运行生成任务"快捷键 |
| `dependsOn` | string/string[]/{...} | 否 | 任务依赖，支持串行/并行 |
| `presentation` | PresentationOptions | 否 | 输出展示行为 |
| `problemMatcher` | string/object/array | 否 | 问题匹配器 |
| `runOptions` | RunOptions | 否 | 运行时机控制 |
| `isBackground` | boolean | 否 | 是否后台持续运行 |

#### `PresentationOptions`（输出面板行为）

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `reveal` | "never"/"silent"/"always" | "always" | 何时显示终端输出 |
| `echo` | boolean | true | 是否回显命令 |
| `focus` | boolean | false | 是否聚焦到终端 |
| `panel` | "shared"/"dedicated"/"new" | "shared" | 面板共享策略 |
| `clear` | boolean | false | 运行前清屏 |
| `group` | string | - | 分屏终端组名 |

#### `RunOptions`（自动运行时机）

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `reevaluateOnRerun` | boolean | true | 重新运行时重算变量 |
| `runOn` | "default"/"folderOpen" | - | 文件夹打开时自动运行 |

#### `CommandOptions`（执行环境）

| 字段 | 类型 | 说明 |
|------|------|------|
| `cwd` | string | 工作目录 |
| `env` | Record<string,string> | 环境变量 |
| `shell` | {executable,args} | 指定 shell |

### 2.2 Input Variables（用户交互输入，VS Code 变量系统的独立功能，非 tasks.json schema 的正式组成部分）

来源：https://code.visualstudio.com/docs/editor/variables-reference#_input-variables

tasks.json 支持三种 input 类型，在运行时弹出 UI 收集用户输入，引用语法为 `${input:variableId}`：

#### `promptString` -- 文本框输入

```json
{
  "id": "buildComment",
  "type": "promptString",
  "description": "Enter a build comment",
  "default": "regular build"
}
```

#### `pickString` -- 下拉选择

支持两种 options 格式：

```json
// 简单字符串数组
{
  "id": "pickProgram",
  "type": "pickString",
  "description": "Select client or server",
  "options": ["client.js", "server.js"],
  "default": "client.js"
}

// Label/Value 分离
{
  "id": "selectEnvironment",
  "type": "pickString",
  "description": "Select deployment environment",
  "options": [
    { "label": "Development", "value": "dev" },
    { "label": "Staging", "value": "staging" },
    { "label": "Production", "value": "prod" }
  ],
  "default": "dev"
}
```

#### `command` -- 扩展驱动输入（动态数据源）

```json
{
  "id": "pickTestDemo",
  "type": "command",
  "command": "shellCommand.execute",
  "args": {
    "command": "ls -1 *.txt",
    "cwd": "${workspaceFolder}"
  }
}
```

**关键限制**：
- `pickString` 的 `options` 数组**不支持变量替换**（`${fileDirname}` 等被视为字面量）
- 原生不支持层级/条件选择（选 A 后动态决定 B 的选项）
- 动态数据源必须走 `type: "command"` + 第三方扩展（如 `augustocdias.tasks-shell-input`）

来源：https://stackoverflow.com/questions/57977832/is-there-a-way-to-get-a-pickstring-dynamically-populated-in-a-vs-code-task

### 2.3 任务自动检测（Auto-Detection）

VS Code 自动扫描以下工具配置，生成 task 条目：

| 工具 | 配置文件 | 自动检测的任务 |
|------|---------|-------------|
| npm | package.json (scripts) | npm: install, npm: test ... |
| TypeScript | tsconfig.json | tsc: build, tsc: watch |
| Gulp | gulpfile.js | gulp: build ... |
| Grunt | gruntfile.js | grunt: build ... |
| Jake | Jakefile | jake: build ... |

可通过设置关闭自动检测：
```json
{
  "typescript.tsc.autoDetect": "off",
  "npm.autoDetect": "off"
}
```

来源：https://code.visualstudio.com/docs/editor/tasks#_task-autodetection

### 2.4 `$schema` 引用

tasks.json 使用 VS Code 内置 schema URI（非 SchemaStore 外链）：

```json
{
  "$schema": "vscode://schemas/tasks",
  "version": "2.0.0",
  "tasks": [...]
}
```

该 schema 是**运行时动态生成**的——任务类型（如 "npm"、"typescript"）由已安装扩展注册，不同用户/工作区的 schema 可能不同。因此不在 schemastore.org 上托管。

来源：https://stackoverflow.com/questions/73333047/where-can-i-find-schema-files-for-vs-codes-json-files-like-launch-json-and-task

---

## 3. launch.json Schema 参考

### 3.1 顶层结构

```json
{
  "version": "0.2.0",
  "configurations": [...],
  "compounds": [...]       // 可选：多配置组合启动
}
```

### 3.2 `configurations` 数组项

每条 configuration 老三样必填：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 显示在调试面板下拉菜单中的名称 |
| `type` | string | 是 | 调试器类型：node/python/go/java/chrome/cppdbg/coreclr 等 |
| `request` | string | 是 | "launch"（启动）/ "attach"（附加到已运行进程）|

常用可选字段：

| 字段 | 说明 |
|------|------|
| `program` | 入口文件路径 |
| `args` | 命令行参数数组 |
| `cwd` | 工作目录 |
| `env` | 环境变量对象 |
| `envFile` | .env 文件路径 |
| `console` | "internalConsole"/"integratedTerminal"/"externalTerminal" |
| `preLaunchTask` | 调试前运行的任务名（关联 tasks.json）|
| `postDebugTask` | 调试结束后运行的任务 |
| `stopAtEntry` | 入口处断点 |
| `sourceMaps` | 启用 source map |
| `skipFiles` | 调试跳过的文件 glob |
| `runtimeExecutable` | 运行时可执行文件绝对路径 |
| `runtimeArgs` | 运行时参数 |

attach 模式专有字段：
`processId`、`port`、`address`、`restart`、`localRoot`、`remoteRoot`

### 3.3 `compounds`（复合配置）

同时启动多个调试目标：

```json
"compounds": [
  {
    "name": "Full Stack",
    "configurations": ["Launch Server", "Launch Client"]
  }
]
```

来源：https://code.visualstudio.com/docs/debugtest/debugging（compounds 功能的页面位置可能已变更）

---

## 4. VS Code Custom Editor API

### 4.1 架构概览

VS Code 提供 Custom Editor API，允许扩展**完全替代默认文本编辑器**来编辑特定文件类型。

**三种 Editor Provider**（`registerCustomEditorProvider` 签名接受三种类型：`CustomTextEditorProvider | CustomReadonlyEditorProvider<CustomDocument> | CustomEditorProvider<CustomDocument>`）：

| Provider | 数据模型 | 适用场景 | VS Code 管理的职责 |
|----------|---------|---------|-------------------|
| `CustomTextEditorProvider` | VS Code 的 `TextDocument` | 文本文件（JSON、XML、CSV 等）| 保存、备份、热退出 |
| `CustomReadonlyEditorProvider` | 扩展自有模型 | 只读二进制文件预览（如 PDF、图片）| 扩展仅需实现 `openCustomDocument` + `resolveCustomEditor` |
| `CustomEditorProvider` | 扩展自有模型 | 可编辑二进制文件（图片、3D 模型等）| 扩展自行处理保存/备份 |

来源：https://code.visualstudio.com/api/extension-guides/custom-editors

### 4.2 package.json 声明

```json
"contributes": {
  "customEditors": [
    {
      "viewType": "myExt.visualConfig",
      "displayName": "Visual Config Editor",
      "selector": [
        { "filenamePattern": "*.myconfig.json" }
      ],
      "priority": "default"
    }
  ]
}
```

关键字段：
- `viewType`：唯一标识符，代码中 `registerCustomEditorProvider` 绑定
- `displayName`：在 "View: Reopen with" 菜单中显示的名称
- `selector`：文件匹配 glob 数组
- `priority`："default"=默认使用自定义编辑器；"option"=需用户手动切换

### 4.3 WebView 通信模式

Custom Editor 的视图层使用 **WebView**（HTML/CSS/JS），通过 message passing 与扩展后端通信：

```
WebView (UI层)  ←→  postMessage/onDidReceiveMessage  ←→  Extension (后端)
```

### 4.4 `jsonc-parser` -- 最小化编辑

**核心问题**：CustomTextEditorProvider 中如果每次更改都替换整个文档，会丢失光标位置和 Undo 历史。

**解决方案**：Microsoft 的 `jsonc-parser` npm 包提供 `modify()` API，计算精确的文本偏移量编辑：

```typescript
import { modify, applyEdits } from 'jsonc-parser';

// 仅修改 path 指向的字段，不改动文档其余部分
const edits = modify(
  document.getText(),           // 当前文档文本
  ['configurations', 0, 'name'], // JSON path（数组索引或属性名）
  'New Name',                    // 新值
  { formattingOptions: { insertSpaces: true, tabSize: 2 } }
);
const updatedText = applyEdits(document.getText(), edits);
```

**优势对比**：

| 无 jsonc-parser | 使用 jsonc-parser |
|----------------|------------------|
| 整个文档替换 | 仅替换变化的文本区间 |
| 光标和 Undo 历史丢失 | 光标和 Undo 历史保留 |
| 大文件低效 | 大文件也高效 |

来源：https://stackoverflow.com/questions/77591085/how-to-compute-minimal-edits-to-a-json-document-for-a-vscode-workspaceedit

### 4.5 Depot 扩展案例（电子表格编辑 JSON）

**Depot Data Editor** 是利用 Custom Editor API 的典型案例——将 JSON 数据以电子表格形式编辑：

- `.dpo` 文件内部存储为 JSON（Git 友好）
- UI 为行列网格，支持 Text/Int/Float/Bool/Image/Select 等多种列类型
- 数据模型自描述（schema 与数据同存于一个文件）
- 支持嵌套 Sheet 编辑
- 通过 `workbench.editorAssociations` 设置可将自定义扩展名路由到 Depot 编辑器

来源：https://marketplace.visualstudio.com/items?itemName=afterschool.depot

---

## 5. 第三方图形化编辑工具

### 5.1 Tingly Debug Configurations

提供 **JetBrains 风格的 launch.json 可视化编辑器**：

- **可视化表单编辑**：字段输入替换原始 JSON 编辑
- **实时 JSON 预览**：编辑表单时同步显示 JSON 预览
- **树视图管理**：以 TreeView 方式管理多个 configuration
- **一键操作**：Debug、运行、复制、删除配置
- 支持 Node.js / Python / Chrome / Edge / Firefox / .NET CoreCLR

来源：https://marketplace.visualstudio.com/items?itemName=Tingly-Dev.tingly-debug

### 5.2 Tasks Shell Input

用于解决 tasks.json 的 `pickString` 不支持动态选项的限制：

- 允许 tasks.json 的 `type: "command"` 输入通过 shell 命令动态获取下拉选项
- 支持 `${workspaceFolder}` / `${file}` 等预定义变量在 shell 命令中展开

```json
{
  "id": "dynamicPick",
  "type": "command",
  "command": "shellCommand.execute",
  "args": {
    "command": "ls -1 *.txt",
    "cwd": "${workspaceFolder}"
  }
}
```

来源：https://marketplace.visualstudio.com/items?itemName=augustocdias.tasks-shell-input

### 5.3 Command Variable 扩展

扩展变量替换能力，支持 `pickStringRemember`（记忆上次选择）、多值关联返回等高级功能。

来源：https://github.com/yyc/command-variable（暂时无法验证可用性，该仓库/Marketplace 扩展当前可能不可访问）

---

## 6. JSON Schema 集成机制

VS Code 对 JSON 文件的内置支持（自动补全、校验、悬停文档）通过 JSON Schema 驱动。

### 6.1 `$schema` 关键字（文件内声明）

```json
{
  "$schema": "https://example.com/myschema.json",
  "...": "..."
}
```

支持三种 URL 形式：远程 HTTP URL、相对路径、SchemaStore URL（如 `http://json.schemastore.org/coffeelint`）。

### 6.2 `json.schemas` 设置（全局/工作区映射）

```json
{
  "json.schemas": [
    {
      "fileMatch": ["src/surveys/*.json"],
      "url": "./schemas/survey.schema.json"
    }
  ]
}
```

支持内联 schema 定义（无需单独文件）：
```json
{
  "json.schemas": [
    {
      "fileMatch": ["/.myconfig"],
      "schema": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "description": "The name" }
        }
      }
    }
  ]
}
```

### 6.3 `jsonValidation` 扩展贡献点

扩展在 `package.json` 中声明 Schema → 文件匹配，无需编写运行时代码：

```json
"contributes": {
  "jsonValidation": [
    {
      "fileMatch": "*.myschema.json",
      "url": "./schemas/my.schema.json"
    }
  ]
}
```

已有特性请求支持运行时动态 schema 关联（VS Code Issue #230136），目前尚未落地。

来源：https://code.visualstudio.com/api/references/contribution-points#contributes.jsonValidation

### 6.4 SchemaStore

schemastore.org 是社区维护的 JSON Schema 目录，覆盖 1,384 个常见配置文件（ESLint、Prettier、tsconfig、GitHub Actions 等）。

- 完整目录：https://www.schemastore.org/api/v1/catalog.json
- tasks.json 和 launch.json **不在此目录**（因为 VS Code 的 schema 是运行时动态生成的）

来源：https://www.schemastore.org

---

## 7. 对 slTerminal 的设计启示

### 7.1 VS Code 模式的优缺点

**优点：**
- JSON 文本编辑 + Schema IntelliSense 是最省实现成本的方案
- 灵活性极高——任何新配置字段只需更新 schema，无需修改 UI
- 自定义 task type 由扩展注册，架构解耦

**缺点：**
- 发现性差——用户必须知道字段名才能得到自动补全
- 对非开发者用户不友好
- launch.json 的 `type` 决定后续可用字段，JSON 编辑无法做条件显隐

### 7.2 推荐策略

| 策略 | 说明 | 实现难度 |
|------|------|---------|
| **JSON Schema 驱动表单** | 将 JSON Schema 渲染为动态表单（字段名、类型、下拉选项、校验规则从 schema 自动生成）| 中 |
| **$schema 声明方式** | 配置文件首行声明 `$schema`，编辑器据此渲染对应 UI（同 VS Code 模式）| 低 |
| **pickString 等效机制** | 对枚举型字段，提供下拉选择 UI；支持 label/value 分离（显示友好标签，存储内部值）| 低 |
| **jsonc-parser 最小编辑** | 表单修改时只编辑变化的 JSON 字段，保留 Undo 历史和光标位置 | 中 |
| **条件显隐** | 根据 type 字段的值动态显示/隐藏后续字段（如 type=shell 显示 command，type=npm 显示 script）| 中 |
| **自动检测** | 扫描项目中的 package.json / Cargo.toml 等，自动生成推荐配置条目 | 中 |
| **模板生成** | 新建设置文件时提供预设模板（根据项目语言/工具链选择）| 低 |

### 7.3 关键参考指标

- **tasks.json Input Variables**：三种交互模式（文本输入/下拉选择/命令驱动）覆盖了配置输入的绝大多数场景
- **Custom Editor API + jsonc-parser**：为 JSON 文件提供 Visual Editor 的标准路径，`modify()` 解决"精确编辑"问题
- **Schema 驱动 IntelliSense**：仅靠 JSON Schema 即可获得 80% 的编辑体验提升（自动补全、校验、文档），这是最低成本的起点
- **Tingly 第三方扩展**：证明了"调试配置可视化"的市场需求，其"表单 + JSON 预览双栏"模式是常见 UI 模式
