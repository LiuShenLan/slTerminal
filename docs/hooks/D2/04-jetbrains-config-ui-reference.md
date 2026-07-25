# JetBrains IDE 配置 UI 设计模式参考

> 调研日期：2026-07-25
> 目的：为 slTerminal 图形化配置功能提供 UI/UX 设计参考

---

## 1. External Tools 配置界面

**路径**：File → Settings → Tools → External Tools (Ctrl+Alt+S)

**来源**：[Settings: External Tools](https://www.jetbrains.com/help/idea/settings-tools-external-tools.html)

### 1.1 主列表页（Master 视图）

列表页展示所有已配置的外部工具，按 Group 分组。核心元素：

| 元素 | 说明 |
|------|------|
| **分组列表** | 每条工具/组前有 **checkbox** 控制启用/禁用（未勾选 = 菜单中隐藏但不删除） |
| **工具栏按钮** | `+`(Alt+Insert) 添加、`-`(Ctrl+Y) 删除、笔图标(Enter) 编辑、上下箭头(Alt+上下) 排序、双页 复制 |
| **排序决定菜单顺序** | 工具在列表中的顺序直接决定菜单中的呈现顺序 |

### 1.2 新建/编辑工具对话框（Detail 视图）

来源：[Create/Edit/Copy Tool Dialog](https://www.jetbrains.com/help/idea/2022.2/settings-tools-create-edit-copy-tool-dialog.html)

对话框使用分组表单布局，分为三个逻辑区域：

#### 基本信息区
| 字段 | 控件 | 说明 |
|------|------|------|
| **Name** | 文本输入框 | 显示在 IDE 菜单和动作列表中的名称 |
| **Group** | 下拉选择 + 可输入新值 | 归类工具到子菜单（如 "Linters"、"Preprocessors"） |
| **Description** | 文本输入框 | 可选描述 |

#### 工具设置区
| 字段 | 控件 | 说明 |
|------|------|------|
| **Program** | 文本输入框 + 浏览按钮(`...`) + 插入宏按钮 | 可执行文件的**绝对路径** |
| **Arguments** | 文本输入框 + 插入宏按钮 | 命令行参数，空格分隔；含空格的参数用双引号包裹，内嵌双引号用反斜杠转义 |
| **Working directory** | 文本输入框 + 浏览按钮 + 插入宏按钮 | 工具执行的**绝对路径**，留空默认项目目录 |

#### 高级选项区
| 字段 | 控件类型 |
|------|----------|
| Synchronize files after execution | Checkbox |
| Open console for tool output | Checkbox |
| Make console active on message in stdout/stderr | Checkbox（依赖 Open console） |
| Output Filters | 多行文本框（每行一个正则），含 `$FILE_PATH$`、`$LINE$`、`$COLUMN$` 宏 |

### 1.3 宏系统

External Tools 的核心设计模式——**参数化占位符**：

- 所有路径/参数字段旁都有 **Insert Macro** 按钮
- 点击弹出宏列表，如 `$FilePath$`、`$ProjectFileDir$`、`$ModuleFileDir$`、`$SelectionStartLine$` 等
- 宏在工具**运行时**被 IDE 实时替换为具体值
- 输出过滤器中宏将工具输出中的路径/行列号片段变为**可点击跳转的超链接**

**设计启示**：宏系统是一个优雅的"参数化配置"方案。相比于让用户手动输入绝对路径，宏提供了上下文感知的变量替换，显著降低配置门槛。slTerminal 的工具配置可借鉴此模式（如 `$FILE_PATH$`、`$PROJECT_ROOT$`、`$SELECTION$` 等变量）。

---

## 2. File Watchers 配置界面

**路径**：Settings → Tools → File Watchers

**来源**：[File Watchers](https://www.jetbrains.com/help/idea/settings-tools-file-watchers.html)、[New Watcher Dialog](https://www.jetbrains.com/help/idea/new-watcher-dialog.html)

### 2.1 主列表页

与 External Tools 相同的 Master-Detail 模式：

| 元素 | 说明 |
|------|------|
| **Level 列** | 每行一个下拉选择：**Project**（仅当前项目）或 **Global**（所有项目） |
| **工具栏** | Add(Alt+Insert)、Edit(Enter)、Remove(Alt+Delete)、Up/Down(排序)、Copy、**Import**、**Export** |
| **排序决定执行顺序** | 多个 Watcher 同时触发时，按列表顺序依次执行 |

**Import/Export**：通过 XML 文件（`watchers.xml`）实现跨项目/跨 IDE 的可移植性。

### 2.2 新建/编辑监视器对话框

采用**渐进式披露**（Progressive Disclosure）设计——将不常用字段折叠隐藏：

#### 模板选择
- 点击 Add 先弹出 **"Choose template" 弹出菜单**：预置模板（LESS、SASS、TypeScript、Stylus、自定义）预填最优默认值

#### 基本信息
| 字段 | 说明 |
|------|------|
| **Name** | 监视器名称（从模板预填） |

#### 监听文件配置
| 字段 | 控件 | 说明 |
|------|------|------|
| **File type** | 下拉选择 | 限制监听的文件类型（如 "Less style sheet"、"TypeScript"） |
| **Scope** | 下拉选择 + 可自定义 | 限制监听目录范围（如 "Project Files"、"Open Files"） |
| **Track only root files** | Checkbox | 仅监听指定目录的顶层文件 |

#### 变更时运行的工具
| 字段 | 控件 | 说明 |
|------|------|------|
| **Program** | 文本输入框 + 浏览 + 插入宏按钮 | 同 External Tools |
| **Arguments** | 文本输入框 + 插入宏按钮 | 同 External Tools |
| **Output paths to refresh** | 文本输入框 + 插入宏按钮 | 工具执行后需要刷新的输出路径 |

#### 工作目录和环境变量（默认折叠）
| 字段 | 说明 |
|------|------|
| **Working directory** | 同 External Tools |
| **Environment variables** | 键值对编辑器，格式 `VAR=value;ANOTHER=val` |

#### 高级选项
| 字段 | 控件 | 说明 |
|------|------|------|
| Auto-save edited files to trigger the watcher | Checkbox | 编辑后自动保存以触发监视器 |
| Trigger the watcher on external changes | Checkbox | IDE 外部的文件变更也触发 |
| Create output file from stdout | Checkbox + 路径字段 | 将工具 stdout 保存为输出文件 |
| Show console | 下拉（Always/Never/On Error） | 控制台可见性策略 |
| Output Filters | 过滤器配置按钮 | 输出中的路径/行列号→可点击跳转 |

### 2.3 File Watchers 的特有设计模式

1. **模板系统**：用预置模板降低新建门槛——常见场景（LESS→CSS、TypeScript→JS）一键创建，无需用户逐字段记忆参数
2. **文件类型/范围过滤器**：File type + Scope 两个下拉实现二维过滤——是"配置目标过滤"的经典组合
3. **Level 列（Project/Global）**：在列表行上内联显示+编辑作用域，是"表格行内编辑"的典型用例
4. **Import/Export**：XML 可移植性解决了"配置跨项目复用"的痛点——一个工具定义写完到处用
5. **渐进式披露**：Working Directory/Environment Variables 默认折叠——减少首次使用时的视觉噪音，高级用户展开即可

---

## 3. IntelliJ Platform 配置表单通用设计模式

**来源**：[Settings Guide](https://plugins.jetbrains.com/docs/intellij/settings-guide.html)、[Layout](https://plugins.jetbrains.com/docs/intellij/layout.html)

### 3.1 MVC 架构（Settings 三层拆分）

JetBrains 平台将每个 Settings 页面拆分为三个类：

```
┌──────────────────────────────────────────────────┐
│  Configurable (Controller)                        │
│  实现 Configurable 接口                            │
│  createComponent() / isModified() /               │
│  apply() / reset() / disposeUIResources()         │
│  与平台交互 + 协调 Model / View                    │
├──────────────────────────────────────────────────┤
│  Settings (Model)                                 │
│  实现 PersistentStateComponent                    │
│  持久化存储，@State 注解声明存储文件               │
│  getState() / loadState()                         │
├──────────────────────────────────────────────────┤
│  SettingsComponent (View)                         │
│  提供 JPanel（Swing form）                        │
│  纯 UI 构造 + getter/setter                      │
│  不触碰持久化/平台 API                             │
└──────────────────────────────────────────────────┘
```

**设计启示**：UI / 数据 / 持久化三层分离是成熟配置系统的核心架构。slTerminal 图形化配置也应保持此分离——表单组件（View）不直接读写磁盘，通过"配置数据模型"（Model）桥接 IPC 持久化。

### 3.2 布局规则

来源：[Layout Guidelines](https://plugins.jetbrains.com/docs/intellij/layout.html)

#### 独立控件排列规则

| 控件类型 | 排列规则 |
|----------|----------|
| **标签 + 输入框** | 标签左对齐，输入框左对齐。标签长度相近时分列排列。**最多两列**，标签过长时标签放输入框上方 |
| **Checkbox / Radio 组** | 默认每行一个。2-3 个短标签（1-3 词）可同行。大量选项：2 列（标签 ≤30 字符）或 3 列（标签 ≤15 字符）。**Radio button 组不可拆分到多列** |
| **按钮 / 链接** | 左对齐。2-3 个短标签可同行。**不使用多列排列** |

#### 关联控件排列规则

| 场景 | 排列规则 |
|------|----------|
| **2-3 个关联控件，每个 ≤30 字符** | 放**同一行** |
| **关联控件较长** | 分多行，下级控件缩进对齐上级控件的输入框左边缘（或标签左边缘 + 水平间距） |
| **全宽主控件 + 小依赖控件（如下拉）** | 依赖控件放主控件**右上角** |
| **不相关控件** | 始终**左对齐**，避免视觉暗示关联 |

#### 间距与分组

- **垂直间距（inset）**：在逻辑组之间添加间距，防止控件"黏在一起"
- **水平间距**：相关控件间距更近，不相关组间距更大
- **分组**：用 `Panel.group()`（有标题的边框面板）或 `Panel.separator()`（水平分隔线+可选标题）明确区分逻辑段
- **真假分组避免**：不必要的间距会造成"伪分组"，误导用户

#### 窗口尺寸规范

| 尺寸 | 像素 | 适用场景 |
|------|------|---------|
| Small | 350 x 250 | 少量纵向堆叠控件 |
| Medium | 500 x 350 | 两列布局，或 2-3 列全宽表格 |
| Large | 750 x 525 | 4+ 列表格、Master-Detail、含代码的双栏 |
| Extra Large | 1000 x 700 | 双栏含代码、3+ 列布局 |

### 3.3 协议接口设计

```java
public interface Configurable {
  JComponent createComponent();  // 懒加载 UI（不在构造函数中构建）
  boolean isModified();          // UI 与持久化状态对比
  void apply();                  // OK 按钮：写回持久化
  void reset();                  // Cancel 按钮：从持久化重载
  void disposeUIResources();     // 关闭时释放引用
}
```

**关键设计决策**：
- `createComponent()` 是懒加载——平台可能在后台线程实例化 Configurable，构造函**不应构建 UI 组件**
- `isModified()` / `apply()` / `reset()` 三个方法形成"脏检查→保存→回滚"闭环
- `disposeUIResources()` 确保关闭对话框后无内存泄漏

### 3.4 持久化声明

Settings 类通过 `@State` 注解声明存储位置：

```java
@State(
    name = "org.example.MySettings",
    storages = @Storage("MyPluginSettings.xml")  // 相对 config 目录
)
```

配置数据在 IDE 重启后自动恢复，存储路径由平台统一管理。

---

## 4. Kotlin UI DSL（声明式表单构造）

**来源**：[Kotlin UI DSL v2](https://plugins.jetbrains.com/docs/intellij/kotlin-ui-dsl-version-2.html)

### 4.1 基础结构

```kotlin
panel {
  row("标签:") {
    textField().bindText(model::property)  // 双向绑定
  }
}
```

- `panel { }` 定义根布局（垂直排列的 row 列表）
- `row { }` 每个 row 一行，label 参数自动对齐
- `.bindText()` / `.bindSelected()` / `.bindIntText()` 等方法自动实现 `apply()`、`reset()`、`isModified()`

### 4.2 分组与结构

| DSL 元素 | 效果 |
|----------|------|
| `group("标题") { }` | 带标题的边框分组，独立网格，有垂直间距 |
| `collapsibleGroup("标题") { }` | **可折叠**的带标题分组面板 |
| `indent { }` | 标准左缩进（用于隶属控件的层级表达） |
| `separator("标题")` | 水平分隔线（可选标题参数），无独立 `titledSeparator` 函数 |
| `comment("<p>html</p>")` | 在分组内显示多行说明文字或提示 |

### 4.3 布局模式

```kotlin
row {
  checkBox("启用高级选项")
  row {  // 嵌套 row = 缩进
    textField("选项值").bindText(model::value)
  }
}
```

- `Row.layout` 三种模式：
  - `LABEL_ALIGNED`（默认）：标签对齐父网格
  - `INDEPENDENT`：独立网格（标签特别长时使用）
  - `PARENT_GRID`：完全融入父网格
- `Cell.resizableColumn()`：标记列可占额外水平空间
- `Cell.gap(RightGap.SMALL)`：同行控件间距
- `Row.topGap` / `Row.bottomGap`：行间垂直间距（`SMALL` = 不相关设置间，`MEDIUM` = 分组间）
- `enabledIf(checkbox.selected)`：条件启用行

### 4.4 控件绑定一览

| 控件 | 绑定方法 |
|------|----------|
| `textField()` | `.bindText(model::prop)` |
| `checkBox()` | `.bindSelected(model::prop)` |
| `intTextField()` | `.bindIntText(model::prop)` |
| `comboBox(items)` | `.bindItem(model::prop)` |
| `slider(min, max, step, default)` | `.bindValue(model::prop)` |
| `spinner(range)` | `.bindIntValue(model::prop)` |
| `buttonsGroup { radioButton() }` | `.bind(model::prop)` |

---

## 5. 设计模式总结与 slTerminal 借鉴

### 5.1 Master-Detail 模式

External Tools / File Watchers 均采用：
- **左侧/上层**：列表/表格展示所有已配置项
- **右侧/弹出**：表单对话框编辑单条配置
- **工具栏**：增删排序复制导入导出

### 5.2 渐进式披露

File Watchers 的 Working Directory + Environment Variables 默认折叠——常见值（项目根目录）不做暴露，高级需求展开即可。此模式适用于 slTerminal 工具配置中的"高级参数"区域。

### 5.3 宏/变量系统

`$FilePath$`、`$ProjectFileDir$` 等宏是 External Tools 和 File Watchers 的**核心亮点**——用户不写死绝对路径，用平台变量实现"上下文感知"配置。slTerminal 的工具执行/命令配置应借鉴此模式。

### 5.4 模板系统

File Watchers 的 Add → Choose template 先选模板再编辑——降低了搜索引擎到实际使用的转化成本。slTerminal 可为常见工具（prettier、eslint、rustfmt、cargo）提供预设模板。

### 5.5 MVC 持久化分离

Configurable / PersistedState / View 三层分离，`isModified()`-`apply()`-`reset()` 协议确保"OK 才保存、Cancel 全回滚"。slTerminal 的配置页应保持此协议。

### 5.6 布局规范

- 独立控件左对齐、最多两列
- 关联控件同行或缩进排列
- 用 `group` / `separator` / `collapsibleGroup` 明确逻辑分组
- 避免伪分组（不必要间距的误导）

### 5.7 Import/Export 可移植性

File Watchers 的 XML 导入导出解决了"配置跨项目/跨安装复用"。slTerminal 可对工具配置提供 JSON/YAML 导入导出。

### 5.8 作用域（Project/Global）

File Watchers 的 Level 列让每条配置可选择作用范围——Project（仅当前项目）或 Global（所有项目）。这是解决"这个配置是项目级还是全局级"的经典 UI 模式。

---

## 6. 来源汇总

| 序号 | 来源 | URL |
|------|------|-----|
| 1 | External Tools Settings | https://www.jetbrains.com/help/idea/settings-tools-external-tools.html |
| 2 | Create/Edit/Copy Tool Dialog | https://www.jetbrains.com/help/idea/2022.2/settings-tools-create-edit-copy-tool-dialog.html |
| 3 | File Watchers Settings | https://www.jetbrains.com/help/idea/settings-tools-file-watchers.html |
| 4 | New Watcher Dialog | https://www.jetbrains.com/help/idea/new-watcher-dialog.html |
| 5 | IntelliJ Platform Settings Guide | https://plugins.jetbrains.com/docs/intellij/settings-guide.html |
| 6 | IntelliJ Platform Layout Guidelines | https://plugins.jetbrains.com/docs/intellij/layout.html |
| 7 | IntelliJ Platform Settings Tutorial | https://plugins.jetbrains.com/docs/intellij/settings-tutorial.html |
| 8 | Kotlin UI DSL v2 | https://plugins.jetbrains.com/docs/intellij/kotlin-ui-dsl-version-2.html |
| 9 | Kotlin UI DSL v1 | https://plugins.jetbrains.com/docs/intellij/kotlin-ui-dsl.html |
| 10 | IntelliJ Platform UI Guidelines | https://plugins.jetbrains.com/docs/intellij/ui-guidelines-welcome.html |
