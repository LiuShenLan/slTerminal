# 命令、焦点与覆盖层

> 目标态实现级设计；需求真值源见 docs/featureSpec，裁决见 ../spec-issues.md。

> 拥有命令与动作注册表、快捷键上下文与重绑、命令派发优先级、焦点协调器、覆盖层单一栈、IME 门控与键盘/焦点原语；面板、侧栏与设置中心只声明命令、可用性谓词与焦点目标，不自行解析按键或维护浮层。

### 决策与红线

- **命令是唯一动作入口**：工具栏按钮、菜单项、快捷键、Runtime 命令与页签菜单最终都产生 `CommandId` 并进入同一派发器。原因：多入口各自调用业务函数会产生可用性、审计与错误反馈分叉。边界：纯展示交互（悬停、滚动、选区拖拽）不走命令通道。
- **上下文决定候选，层序决定胜负**：快捷键只在所属上下文成立时成为候选；同一按键有多层候选时按固定层序裁决，不做“最近按过谁”的启发式。原因：启发式会让同一按键在相同焦点下行为漂移。边界：设置页的冲突警告只提示同上下文重复绑定，不改变运行期层序。
- **命令层序唯一**：面板固定键/保留键 > 面板命令 > 侧栏视图快捷键 > 全局命令 > 终端输入。终端输入对派发器已绑定的和弦让位，未绑定按键透传 PTY。原因：终端与编辑器的核心输入语义不能被应用命令截获，同时侧栏入口必须随时可达。边界：全局关闭面板 `Ctrl+W` 高于终端输入，但低于终端保留键集合；侧栏视图快捷键位于面板命令之后、终端输入之前。
- **焦点意图一次性**：显式打开/新建面板产生带令牌的焦点意图，面板就绪后消费一次；面板被切走、面板组失焦、项目切换或意图被更新时作废。原因：异步创建的面板可能在就绪前已被替换，延迟聚焦会抢错面板。边界：布局恢复不产生焦点意图；命中已有面板的打开请求直接同步聚焦，不新建意图。
- **覆盖层单一栈**：对话框、菜单、弹出层与通知（含可操作撤销）都进入同一 `OverlayStack`，由栈决定输入命中、`Esc` 关闭顺序、焦点陷阱与锚点失效。原因：多个平行浮层会竞争 `Esc`、焦点与 z 序。边界：通知为非模态，不参与焦点陷阱；对话框为模态，同一时刻最多一个。
- **IME 组合期间零命令匹配**：组合态按键只交给输入法或文本输入，不匹配单键与组合键命令。原因：CJK 输入法用字母键选择候选，任何命令匹配都会打断出字。边界：组合结束后的提交文本不触发命令；`Esc` 取消组合由输入法处理，应用不消费。
- **键盘与焦点是结构而非补丁**：焦点环、名称、选中态与减少动效在组件原语中提供，业务组件只填语义。原因：事后逐控件补标签必然遗漏。边界：真实输入法候选窗与视觉观感登记人工验收。

### 命令与动作注册表

```rust
pub struct CommandId(&'static str);          // 稳定字符串，跨设置、绑定与 Runtime 调用持久化

pub enum CommandLayer {
    Input,          // 面板输入保留键，不进入命令表
    Panel,          // 面板命令
    SidebarView,    // 侧栏视图显隐快捷键
    Global,         // 全局命令
    Overlay,        // 浮层自身命令
}
// 枚举顺序不是派发顺序；派发顺序见“命令派发层序”。

pub enum CommandVisibility { UserFacing, Internal }

pub enum CommandGroup { Global, Terminal, Editor, FileExplorer }

pub struct CommandDef {
    pub id: CommandId,
    pub group: CommandGroup,      // 设置中心快捷键页的固定分组
    pub layer: CommandLayer,
    pub context: ShortcutContext,
    pub default_chord: Option<KeyChord>,
    pub rebindable: bool,
    pub visibility: CommandVisibility,
    pub title: Message,
    pub order: u32,               // 注册顺序；同层同键并列时的确定性次序
}

pub enum ShortcutContext {
    Global,
    Terminal,
    Editor,        // 编辑器、Markdown 源码态、Diff 工作区侧
    FileExplorer,  // 文件树与 Agent 文件树
    Navigation,    // 导航树项目/会话行
    Diff,
    GitShow,
    Markdown,
    Settings,
}

pub enum CommandGate {
    Enabled,
    Disabled(Message),   // 候选成立但当前不可执行；消费按键，防止误输入
    Inactive,            // 上下文不成立；不是候选，按键继续下传
}

pub enum CommandOutcome {
    Handled,
    Unhandled,
    Failed(ErrorReport),
}

pub enum DispatchResult {
    Handled(CommandId),
    PassThrough,
    Composition,
    Recording,
    ReservedRejected(KeyChord),
}

pub trait Dispatcher {
    fn dispatch(&mut self, event: &KeyEvent, context: &InputContext) -> DispatchResult;
}
```

- 注册表只保存描述，不持有 UI 实体。启动时由壳组装 `CommandRegistry`；重复 `CommandId` 是启动期硬错误（调试构建 panic，发布构建记录并保留首个注册）。原因：重复命令会让设置页与绑定表出现两个“同名不同物”。边界：同一命令可因平台能力缺失而不可用，但不可重复注册。
- 命令处理函数由应用层的 `CommandHost` 以封闭分派实现，签名形态为 `fn execute(&mut self, id: CommandId, context: &mut CommandContext<'_>) -> CommandOutcome`。领域 crate 不引用 GPUI，也不引用 `CommandHost`。
- `order` 只用于稳定排序与同层并列裁决，不暴露给用户，也不进入设置键。
- `CommandVisibility::Internal` 的命令不进入设置中心快捷键页，但仍可被菜单、测试与 Runtime 内部调用。Diff 的 `F6`/`F7` 与面板内部固定键不进入可重绑目录；终端搜索不在目标范围，不注册命令。
- 命令可用性在派发时求值，不在注册时缓存。数据依赖（有活跃项目、有激活面板、面板可编辑）通过 `InputContext` 判定，I/O 依赖（Git 可用、文件存在）由命令处理函数在执行时判定并返回 `Failed`。

### 快捷键模型与重绑

```rust
pub struct Modifiers { pub ctrl: bool, pub shift: bool, pub alt: bool, pub meta: bool }

pub enum PhysicalKey {
    KeyA, KeyZ, Digit0, F1, F12,
    Comma, Period, Slash, Backslash, BracketLeft, BracketRight,
    Enter, Escape, Tab, Space, Backspace, Delete, Insert,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Home, End, PageUp, PageDown,
}

pub struct KeyChord { pub modifiers: Modifiers, pub key: PhysicalKey }

pub struct BindingOverrides {               // 设置域 shortcuts.overrides
    pub entries: Vec<BindingOverride>,      // 按 CommandId 排序，稳定写盘
}

pub struct BindingOverride {
    pub command: CommandId,
    pub chord: Option<KeyChord>,            // None = 用户解绑
}

pub struct BindingBuild {
    pub table: BindingTable,
    pub diagnostics: Vec<BindingDiagnostic>,
}

pub enum BindingDiagnostic {
    UnknownCommand(CommandId),
    InvalidChord(String),                   // 原始值保留供修复横幅展示
    ReservedChord { command: CommandId, chord: KeyChord, context: ShortcutContext },
    DuplicateInContext { chord: KeyChord, commands: Vec<CommandId> },
}
```

- 键位存储采用物理键名：修饰键固定顺序 `Ctrl`、`Shift`、`Alt`、`Meta`，末段为 `KeyW`/`KeyC`/`F2` 形态。解析大小写不敏感，写盘统一规范化；修饰键顺序不同视为同一和弦。
- 保留键集合按上下文定义，`Global` 上下文取并集：
  - 终端：`Ctrl+KeyC`、`Ctrl+KeyV`、`Ctrl+KeyX`、`Ctrl+KeyZ`、`Ctrl+KeyA`。
  - 编辑器：`Ctrl+KeyF`、`Ctrl+KeyZ`、`Ctrl+KeyY`、`Ctrl+Shift+KeyZ`、`Tab`、`Shift+Tab`。
  - 全局：上述并集。
- `Ctrl+KeyC` 在终端上下文始终透传为中断信号，不进入命令派发；用户可以把“终端中断”命令改绑到其他非保留键。
- 启动时与设置热更新时都重建绑定表。`BindingDiagnostic` 不静默：未知命令、非法格式、保留键覆盖使用默认键并在快捷键页顶部显示修复横幅；同上下文重复绑定允许生效，但同时显示冲突警告。
- 重绑录制状态机：

```text
Idle
  └─ 点击/Enter/Space → Recording { command }
       ├─ 合法和弦 → Committed（写覆盖，退出录制）
       ├─ Backspace/Delete → Unbound（写 None，退出录制）
       ├─ Esc → Cancelled（不写）
       ├─ 仅修饰键 → Recording（等待下一键）
       ├─ IME 组合态按键 → 忽略（保持录制）
       └─ 保留键 → RejectedReserved（行内错误，保持录制）

Recording 期间全局命令派发整体暂停；配置页卸载、面板失活、项目切换、面板关闭 → Cancelled。
```

- 非法覆盖不改变运行期默认键，但原始坏值保留在设置中直到用户修复或恢复默认；不得静默删除。
- 设置页只展示可重绑命令；面板内部固定键与保留键只读展示。面板固定键的上下文冲突由对应面板文档定义，不进入快捷键页。

### 命令派发层序

`InputContext` 由焦点协调器、覆盖层栈与 IME 状态共同派生：

```rust
pub struct InputContext {
    pub focus: FocusSnapshot,
    pub panel: Option<PanelKind>,
    pub sidebar_view: Option<SideViewId>,
    pub overlay: Option<OverlayKind>,
    pub text_input: bool,
    pub ime_composing: bool,
    pub recording_shortcut: bool,
}
```

| 顺序 | 输入层 | 候选来源 | 消费规则 |
|---|---|---|---|
| 1 | 快捷键录制 | 无 | 所有命令暂停；录制器独占按键，组合态按键忽略 |
| 2 | IME 组合 | 无 | 对所有层生效，不匹配任何命令；文本走输入法 |
| 3 | 栈顶模态/菜单/弹出层 | `Overlay` 命令与菜单键位 | 输入被限制在栈顶面内；`Esc` 关闭栈顶可关闭层 |
| 4 | 面板固定键与保留键 | 面板输入上下文 | 终端保留键透传 PTY；编辑器保留键执行固定语义 |
| 5 | 面板命令 | 当前焦点面板的 `Panel` 层 | 上下文成立且启用才消费 |
| 6 | 侧栏视图快捷键 | `SidebarView` 层 | 全局生效，位于面板命令之后、其他全局命令之前，并优先于终端输入 |
| 7 | 全局命令 | `Global` 层 | 低于面板命令；无激活面板时 `Ctrl+W` 不消费 |
| 8 | 焦点输入控件 | 终端输入、文本编辑器、列表 | 接收未被上层消费的按键；终端输入对派发器已绑定的和弦让位，未绑定按键透传 PTY |

- 上下文匹配规则：终端命令只在终端面板焦点时成立；编辑器命令在编辑器、Markdown 源码态与 Diff 工作区侧成立；文件浏览器命令在文件树或 Agent 文件树获得焦点时成立；侧栏视图命令在所有上下文中成立。
- 固定键先于同键全局命令的例子：编辑器搜索的 `Ctrl+Shift+G` 在编辑器上下文先于“打开 Commit”；焦点不在编辑器时，同一和弦进入侧栏视图层并打开 Commit。该冲突不进入可重绑目录。
- 终端页签的 `F2` 重命名只在页签聚焦上下文成立；终端输入区聚焦时该命令不是候选，`F2` 透传 PTY。
- 同一层同一和弦出现多个命令时，按 `CommandDef.order` 升序取第一个 `Enabled` 候选。若候选为 `Disabled`，消费按键并显示其 `Disabled` 原因，不再下传到更低层；若所有候选均为 `Inactive`，按键继续下传。
- 命令处理函数返回 `Failed` 时，由命令自身决定反馈层级；派发器只保证错误进入统一通道，不吞掉。
- `Ctrl+Tab`/`Ctrl+Shift+Tab` 是焦点逃逸键：终端与编辑器不得拦截，派发器在面板命令层之前显式保留该和弦给焦点协调器；焦点在活动栏、侧栏或主区时都可进入面板循环。
- 没有激活面板时 `Ctrl+W` 返回 `Unhandled`，不关闭窗口，按键不被标记为已消费。
- 菜单打开时，方向键、`Enter`、`Space`、`Esc` 由菜单消费；菜单外按键先关闭菜单还是透传由 `OverlaySpec.dismiss` 决定，默认先关闭菜单并消费该键。

### 焦点协调器

```rust
pub struct FocusToken(u64);

pub enum FocusOwner {
    Workspace,
    ActivityBar,
    Sidebar(SideViewId),
    TabStrip(PanelGroupId),
    Panel { group: PanelGroupId, panel: PanelId, target: FocusTarget },
    Overlay(OverlayId),
}

// FocusTarget 定义于 domain-model.md（slterm_workspace::focus）；本节只引用，不重复定义。

pub enum FocusOrigin {
    ExplicitOpen,     // 打开文件、新建面板、命令激活
    ProjectSwitch,
    Restore,          // 布局恢复/启动恢复：不产生意图
    Command,          // 焦点循环等
}

pub struct FocusIntent {
    pub token: FocusToken,
    pub project: ProjectId,
    pub panel: PanelId,
    pub group: PanelGroupId,
    pub target: FocusTarget,
    pub origin: FocusOrigin,
}

// 归 ProjectView 持有；Workspace 不持第二份
pub struct FocusMemory {
    pub group: PanelGroupId,
    pub panel: PanelId,
    pub target: FocusTarget,
}

pub enum CycleDirection { Next, Previous }

impl FocusCoordinator {
    pub fn request(&mut self, intent: FocusIntent);
    pub fn consume_ready(&mut self, token: FocusToken, panel: PanelId) -> bool;
    pub fn focus_now(&mut self, panel: PanelId, target: FocusTarget);
    pub fn switch_project(&mut self, project: ProjectId);
    pub fn close_focused(&mut self, closed: PanelId);
    pub fn cycle(&mut self, direction: CycleDirection);
}
```

状态与规则：

- `FocusCoordinator` 持有唯一当前 `FocusOwner` 与至多一个待消费 `FocusIntent`。运行期 `FocusMemory` 由各 `ProjectView` 持有，协调器只按项目读取/写回；`Workspace` 不持第二份。新意图替换旧意图并使旧令牌失效。
- `request(intent)`：`Restore` 直接忽略；其他来源写入待消费意图。`consume_ready(token, panel)` 只有令牌仍是最新、项目仍活跃、面板仍是所在面板组激活面板且没有更新的焦点意图时才执行聚焦，否则丢弃。
- 布局恢复只重建结构、可见性与激活态，不触发打开/激活事件，不产生焦点意图；启动完成后没有面板自动获得键盘焦点。
- 命中已有面板的打开请求调用 `focus_now(panel, target)`，不创建意图；因此不会出现“延迟一帧才聚焦”的二次跳转。
- 点击面板页签或面板内容：激活该面板及其面板组，更新 owner 并聚焦该面板的焦点落点；点击非活跃项目的可见内容前先经导航/会话入口切换活跃项目。点击已激活面板内容只更新焦点，不重建意图。
- 项目切换：先作废所有待消费意图，显示目标项目布局，再读取该项目焦点记忆并请求 `ProjectSwitch` 意图；无记忆时焦点停在 `Workspace`。
- 关闭当前聚焦面板：同面板组按页签顺序取下一個，没有下一个取上一个；同组没有其他面板时取相邻面板组的激活面板；没有其他面板组时停在 `Workspace`。
- `Ctrl+Tab`/`Ctrl+Shift+Tab` 遍历活跃项目的面板组深度优先顺序与组内页签顺序，循环并聚焦目标面板的焦点落点。焦点在活动栏、侧栏或主区时都可进入循环；活跃项目没有面板时返回 `Unhandled`，不消费。
- 面板组获得焦点时，其激活面板成为焦点落点；非聚焦面板组保留激活面板但不接受键盘焦点。
- 焦点记忆只在运行期有效，不写盘；重启不恢复键盘焦点。
- 焦点变化写回该项目的 `ProjectView.focus_memory` 仅当 owner 是 `Panel` 且项目活跃；打开覆盖层不改变面板记忆。

### 覆盖层单一栈

通知的类型、时长、覆盖、去重与缓存归 `cross-cutting.md`；本节拥有覆盖层栈的入栈/出栈、z 序、输入命中、`Esc` 关闭顺序、焦点陷阱与锚点失效。

```rust
pub struct OverlayId(u64);

pub enum OverlayKind { Dialog, Menu, Popover, Notification }

pub enum ZBand { Notification = 10, Popover = 30, Menu = 40, Dialog = 50 }

pub enum OverlayDismiss { Escape, OutsideClick, Explicit, Timer, Never }
pub enum DismissReason { Escape, OutsideClick, Explicit, Timer, AnchorLost }

pub struct OverlaySpec {
    pub kind: OverlayKind,
    pub anchor: Option<OverlayAnchor>,
    pub modal: bool,
    pub focus_trap: bool,
    pub dismiss: Vec<OverlayDismiss>,
    pub restore_focus: Option<FocusToken>,
    pub z: ZBand,
}

impl OverlayStack {
    pub fn push(&mut self, spec: OverlaySpec) -> OverlayId;
    pub fn pop(&mut self, id: OverlayId);
    pub fn pop_top_dismissible(&mut self, reason: DismissReason) -> Option<OverlayId>;
    pub fn top_input_scope(&self) -> Option<OverlayId>;
}
```

- `OverlayStack::push(spec) -> OverlayId` 按 `z` 插入；同 `z` 按入栈顺序。`pop(id)` 只移除指定层；`pop_top_dismissible()` 供 `Esc` 使用。
- 模态层最多一个。已有对话框时发起新对话框请求：旧请求按取消处理，新请求入栈；栈内绝不并列两个模态。
- 键盘输入只进入栈顶可接收键盘的层。模态层启用焦点陷阱：`Tab`/`Shift+Tab` 在层内循环，初始焦点由调用方指定，危险操作默认落在安全按钮。关闭后焦点回到 `restore_focus` 指向的控件。
- 通知层非模态、不抢焦点、不设陷阱；通知关闭按钮在焦点顺序中排在主区之后。通知在项目切换、面板切换与布局恢复期间保持可见；时长、覆盖、去重与缓存按 `cross-cutting.md`。
- 菜单与弹出层在项目切换、布局结构变化、锚点控件销毁或窗口失焦时关闭；对话框在项目切换时保持挂起，目标对象由发起时确定。对话框的触发层级与错误反馈归 `cross-cutting.md`。
- 撤销为可操作通知（`NotificationSpec.action`），同一时刻只有一个未决撤销；不设独立 `UndoBar` 覆盖层。
- 通知合并、去重、缓存与错误通知保留规则归 `cross-cutting.md`；覆盖层栈只保证通知层不抢焦点与稳定 z 序。
- 启动期间产生的通知进入队列；界面挂载后，未超过自动消失时长的通知照常显示。窗口失焦时应用内通知仍显示；系统通知另按平台门控。
- 覆盖层不参与窗口横向分区布局，统一挂在应用根部；z 序只由 `ZBand` 与入栈顺序决定，业务组件不得自行设置更高层。

### IME 门控与 CJK

```rust
pub struct ImeSnapshot {
    pub composing: bool,
    pub cursor_range: Option<Range<usize>>,
    pub committed_text: Option<String>,
}

pub enum KeyAdmission {
    Composition,     // 交给输入法，应用零命令匹配
    Shortcut(u32),   // 录制器独占
    Command(CommandId),
    TextInput,
    Ignored,
}
```

- 组合态判定以输入法快照为准，不以按键字符是否可打印为准。`composing == true` 时，任何单键、组合键、功能键都不进入命令匹配；方向键与 `Esc` 交给输入法取消/移动候选。
- 终端面板的 IME 预览由 GPUI 输入处理器绘制，提交文本才编码为 UTF-8 写入 PTY；组合中间态不得写入 PTY，避免半个码点或候选串进入 shell。
- 编辑器的组合态由文本输入控件持有；组合期间不触发自动补全提交、不触发单键快捷键、不触发保存。
- 快捷键录制期间组合态按键忽略，组合结束后再按正常规则处理。
- CJK 文本处理以字素簇为单位：退格、删除、选区扩展、标题省略、搜索高亮不得拆开代理对或组合字符。标题省略使用字素安全截断并保留尾部省略号。
- 中文输入法出字、候选窗定位、焦点切换不得因覆盖层或面板切换被中断；真实候选窗行为登记人工验收。

### 键盘与焦点原语

- 组件库提供 `focusable`、`roving_tab_index`、`tab_list`、`dialog_scope` 原语；业务组件只声明焦点目标与名称，不直接操作窗口焦点。
- 图标按钮的名称与悬停提示同源；纯图标窗口控制钮的提示即名称。页签关闭按钮名称为 `关闭 <面板标题>`，脏状态标记名称为“未保存”；面板横幅与通知的关闭按钮均可键盘到达并有明确名称。
- 焦点环使用主题 token，始终在聚焦时可见且不被面板内容裁切；焦点顺序为活动栏 → 侧栏 → 主区页签栏 → 激活面板 → 覆盖层。
- 动态状态（保存失败、任务完成、注入状态、通知出现）以文本或结构呈现，不只依赖颜色；错误通知不得被静默丢弃。
- 尊重系统减少动效：`MotionPolicy::{Full, Reduced}`；减少动效下面板切换、拆分、通知与菜单动画取消或缩短到不承载信息。加载与进度必须有文本或结构替代。
- 最小指针命中区为 24×24 逻辑像素，视觉尺寸可以更小。
- 状态不得只靠颜色：终端状态圆点同时提供文本与名称；脏状态有形状/标记；错误提示有文字；焦点有焦点环。

### 验收与测试

- 纯函数层：和弦解析/规范化往返、保留键拒绝、同上下文冲突诊断、层序裁决表、焦点意图令牌失效、覆盖层入栈/出栈与 `Esc` 顺序。
- 虚拟窗口层：新建面板后可直接输入；打开已有面板不新建意图；关闭焦点面板的三种后继路径；项目切换恢复焦点；模态焦点陷阱与关闭后回焦；菜单在项目切换时关闭；通知在项目切换时保留；IME 组合态不触发命令。
- 键盘层：仅用键盘完成面板切换、侧栏显隐、活动栏菜单、文件树操作、Diff 导航、设置页导航与对话框确认/取消；焦点环可见且无陷阱。
- 自动化不可覆盖：真实中文输入法候选窗与视觉动效观感；登记人工验收与兜底。
- 门禁：`cargo test -p <crate> --test lib_tests <filter> -- --test-threads=1`；虚拟窗口用例走 `--features gpui-test-support`。
