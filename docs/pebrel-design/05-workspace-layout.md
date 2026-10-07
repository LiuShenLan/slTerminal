# 05 工作区与布局详细设计

> pebrel-design 分片 05/12。上游 spec:`docs/pebrel-refactor/SPEC.md`(总表)+ `docs/pebrel-refactor/05-workspace-layout.md`(spec 分片,含采纳点编号);骨架:`docs/pebrel-design/00-roadmap.md`;改名映射单点表与类型锚点规则:`docs/pebrel-design/01-arch-baseline.md`。
>
> 本篇是 **pane/tab 标识、分屏树类型、session v4 schema** 的锚点归属篇(01 篇类型锚点表已登记):`PaneId`/`TabId`/`SplitTree`/`WorkspaceTab`/`Session` schema 在此签名级定义,他篇(02/03/04/06/07 等)只许 `use` 或经 facade 传参,禁重定义。引 pebrel 一律符号名 + 文件路径(baseline `e537d528c508e8607d0f5f9fd25e5902f40d661e` 钉死),禁行号。

## 目标形态

工作区在 GPUI 单进程壳内重建为单一 Rust 实体:`slterm_app::gpui_shell::workspace::Workspace`(GPUI `Entity`)持有 tab 列表 + 活跃 tab + 侧栏骨架态;左侧垂直 tab 侧边栏 + 主内容区骨架照 pebrel `nebula_app/src/gpui_shell/workspace.rs` 形态。分屏语义全部由 `slterm_split` 纯函数 crate 裁定(照抄 `nebula_split` 改名,零生产依赖契约归 01 篇);**壳层只做四件事**——按提交比例渲染递归、canvas 回写节点/pane 矩形、拖拽 overlay、松手提交。TTY 尺寸链(burst+settle 两段式、prepaint 观测)与终端面板本体归 02 篇,本片登记交界契约。

### 模块终态

| 模块 | 来源 | 职责 |
| --- | --- | --- |
| `slterm_split/src/lib.rs` | pebrel 照抄改名 | `SplitTree<T>`/`Rect`/`Divider`/`SplitLayout`/`RemoveOutcome`/`SplitDirection`/`SplitNav` + 切割/拖拽曲线/导航纯函数(本篇锚点) |
| `slterm_split/src/dock.rs` | pebrel 照抄改名 | `joined`/`dock_at_leaf` 整树嫁接 |
| `slterm_split/src/ids.rs` | 新建 | `PaneId`/`TabId` 纯标识 newtype(锚点,见下) |
| `slterm_app/src/session.rs` | pebrel 照抄裁剪 | session schema(本篇锚点;语义重定位见改造节) |
| `slterm_app/src/atomic_file.rs` | pebrel 照抄 | 临时文件 + rename 原子写 + try_lock |
| `slterm_app/src/gpui_shell/session_restore.rs` | pebrel 照抄改名 | `layout_from_tree`/`tree_from_layout` 转换对——「布局单点」唯一落点 |
| `slterm_app/src/gpui_shell/workspace.rs` | pebrel 参考重建 | `Workspace`/`WorkspaceTab`/`TerminalPane` 三件套 + 生命周期合同 + 渲染递归 + zoom + SplitDrag overlay |
| `slterm_app/src/gpui_shell/workspace/split_drag.rs` | pebrel 参考重建 | 分隔条拖拽 overlay(`split_drag_visual_geometry`/`update_split_drag`/`finish_split_drag`) |
| `slterm_app/src/gpui_shell/workspace/tab_drag.rs` | pebrel 参考重建 | tab 级拖拽 + `DockTarget` 四分位 + 落点提交 |
| `slterm_app/src/gpui_shell/workspace/session_persistence.rs` | pebrel 照抄裁剪 | latest/saved 双快照、无变化跳过、SaveReason 状态机(单窗化) |
| `slterm_app/src/gpui_shell/workspace/session_recovery.rs` | pebrel 照抄裁剪 | `snapshot_session`/`try_restore_session`/`restore_tab`(单窗化,去 guest/SSH 臂) |
| `slterm_app/src/gpui_shell/workspace/sidebar.rs` | 参考 pebrel sidebar + slTerminal 既有侧栏形态重建 | tab 列表渲染 + sideViews 槽位骨架(归下) |
| `slterm_app/src/terminal_registry.rs` | 新建(硬约束 #8 对应物) | 模块级单例:PaneId → 会话元数据索引(签名见下) |

### 核心不变式(壳层合同,照抄 pebrel workspace.rs 模块头)

1. Terminal tab 持有 `panes`(实体属主,无序按 id 查)+ `tree`(`slterm_split` 布局树,叶 = pane id)+ `focused`;**panes 的 id 集合 == 树的叶集合**,贯穿 split/close/restore 全路径。
2. pane id 全 workspace 唯一、终生不复用;`PaneId` 由 `Workspace` 单调计数器分配(newtype 不自带全局态,测试可注入)。
3. 关 pane 的结局由树函数裁定(`RemoveOutcome` 三态):`WasRoot` 关整个 tab、`Collapsed(id)` 兄弟收编、焦点交幸存子树首叶;壳不自行散判。被摘 pane 立即显式 `shutdown`,失败路径(建树挂不上)立即回收 pane,不留孤儿 PTY。
4. 布局持久化单点:持久化布局只经 session schema 存取,运行时树与持久化树之间只有 `session_restore` 一对纯函数;「面板封闭」= `WorkspaceTab` 封闭枚举,新增 tab 类型 = 变体 + 渲染分支 + 关闭/持久化语义同步归对应分片(06/07)。

### session 语义重定位(一句话)

spec 采纳点 17-26 照抄 schema、不采纳点 5 砍掉驻留语义后,session.json 只回答**「下次启动恢复成什么样」**,不回答「关窗后谁还活着」;`WindowLayout`/`combine_sessions`/detached 恢复/按窗口角色过滤等只出现在消亡语境。

## 边界与不变更项

1. **产品定位七条**与 00-roadmap 跨领域不变量全适用;本篇涉及 WebView / IPC / Tauri / Dockview / xterm.js / vitest 的词一律只在「消亡 / 映射 / 来源」语境。slTerminal 旧 Dockview 体系(`src/workspace/layoutSerde.ts` 的宿主汇编/页切片/迁移修补、`pageGroups.ts` 页前缀协议、`panelRegistry.ts` 白名单、`TerminalRegistry.ts` 跨页存活、页组 `setVisible` 显隐、`renderer="always"` 恒挂载)整体消亡,本片只重建其硬约束(#5 面板封闭 / #7 布局单点 / #8 会话元数据单点)的 Rust 对应物。
2. **类型锚点纪律**:本篇唯一定义 `PaneId`/`TabId`/`SplitTree` 族(`Rect`/`Divider`/`SplitLayout`/`RemoveOutcome`/`SplitDirection`/`SplitNav`)/`WorkspaceTab` 族(`TerminalPane`/`TabEntry`)/session schema 族(`Session`/`TabSession`/`LayoutSession`/`SplitAxis`/`LaunchSession`/`AgentSession`/`WindowState`)。他篇引用纪律:只许 `use` 或经 facade 传参,禁重定义、禁别名漂移、禁字段语义改写;新增跨领域类型先回 01 篇锚点表登记归属。`Rgb` 唯一定义归 06 篇(主题),本篇 schema 引用之。
3. **启动身份只留本地形态**:`LaunchSession` 砍 `Ssh`(spec 不采纳点 3)与 `Shell`(WSL 发行版,spec 不采纳点 4),只留 `Default` 与本地 `Profile`(「完整命令内嵌保可移植」形态保留);默认 shell 回退链 pwsh→powershell→cmd 归 02 篇,不进 schema。
4. **多窗/驻留语义不迁**:`Session.window_layout` 段、`WindowLayout`/`combine_sessions`/`save_combined_session` 的多窗合并、per-window snapshot 组合、`WindowRole` 过滤、residency 模块,全部不迁(spec 不采纳点 2/5);单窗口下 `Session` 扁平化为唯一窗口,`window` 字段只保留单窗尺寸/最大化态。
5. **WindowState 只写不回放**(spec 不采纳点 12):窗口尺寸/最大化态是诊断与前向兼容数据,启动按配置列行数定形;「记住上次窗口大小」是否恢复归待沉淀 D05-1。
6. **运行时态不进会话**:zoom(`WorkspaceTab::Terminal.zoomed`)、broadcast(spec 不采纳点 6,字段一并砍)、侧栏开合/sideViews 选中、reader focus、重命名编辑中状态,全部内存态;快照只收 Terminal tab(设置/文档/图片 tab 不进会话,pebrel 同合同)。
7. **快照纪律照抄**:1Hz 快照恒写 `clean_exit=false`、`boot_attempts=0`;只有正常收尾(关窗)最后一笔写 `clean_exit=true`;`MAX_BOOT_ATTEMPTS` 断路值照抄;空会话(一路关标签关干净)不算崩溃。
8. **schema 版本号重启为 1**(本篇决策,见改造节):pebrel 的 v1–v3 内存就地升级链不迁(slTerminal 无历史会话文件),但「追加字段走 `serde(default)` 免升版」的演进纪律保留。
9. **核心不变式四条件**(见目标形态)是测试与 review 的硬判据;shell 侧 `SessionPersistence` 的 `isolated`(提权隔离不写共享存储)语义保留——单实例下管理员/普通进程仍须互不覆盖会话文件。

## 关键类型与签名

> 均为草稿级签名。照抄部分的签名与 pebrel 一致;`pebrel` 侧为 `u64` 裸 pane id 处,本片以 `PaneId` newtype 承接(改动理由见各条)。`slterm_split` 保持零生产依赖(dependencies.toml 契约,归 01 篇)。

### 标识(`slterm_split/src/ids.rs`,本篇锚点,新建)

```rust
/// 终端 pane 标识:全 workspace 唯一、终生不复用。
/// 分配权威在 Workspace 的单调计数器(newtype 不持全局态,测试可注入序列)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneId(pub u64);

/// tab 稳定标识:跨排序/活跃切换不变;不进 session schema(schema 按位置存取)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(pub u64);
```

归属说明:标识放进 `slterm_split` 而非 `slterm_app`,因为树 crate 的叶类型即 pane id,且 03(hook 环境)/04(runtime API)等 app 内模块需要引用标识而不应互相穿透;newtype 零成本、零依赖。pebrel 用裸 `u64`(session_restore.rs 的 `SplitTree<u64>`),newtype 化是本篇唯一类型面偏差,防「比例/id 混算」类笔误,不改任何语义。

### 分屏树(`slterm_split`,照抄锚点)

```rust
// 常量契约(照抄):DIVIDER_GAP=2.0 / HIT_SLOP=8.0 / CLOSE_MARGIN=0.06 / RATIO_CLAMP=(0.10,0.90)
pub enum SplitDirection { LeftRight, TopBottom }
pub enum SplitNav { Left, Right, Up, Down }
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }   // 屏幕像素,左上原点
pub struct Divider { pub rect: Rect, pub direction: SplitDirection, pub path: Vec<bool>, pub viewport: Rect }
pub enum RemoveOutcome<T> { NotFound, WasRoot, Collapsed(T) }

pub enum SplitTree<T> {
    Leaf(T),
    Split {
        direction: SplitDirection,
        ratio: f32,                    // 提交值:第一个孩子占比
        preview_ratio: Option<f32>,    // 拖拽预览值;PTY 尺寸只跟随提交值
        dragging: bool,
        first: Box<SplitTree<T>>,
        second: Box<SplitTree<T>>,
    },
}
pub struct SplitLayout<T> { pub panes: Vec<(T, Rect)>, pub dividers: Vec<Divider> }

impl<T: Copy + Eq> SplitTree<T> {
    pub fn leaf(id: T) -> Self;
    pub fn is_leaf(&self) -> bool;
    pub fn leaves(&self) -> Vec<T>;            // DFS 序——恢复注入配对的权威序
    pub fn first_leaf(&self) -> T;
    pub fn contains(&self, id: T) -> bool;
    pub fn replace_leaf(&mut self, target: T, replacement: T) -> bool;
    pub fn split_leaf(&mut self, target: T, new: T, direction: SplitDirection, ratio: f32) -> bool;
    pub fn remove_leaf(&mut self, target: T) -> RemoveOutcome<T>;
    pub fn node_mut(&mut self, path: &[bool]) -> Option<&mut SplitTree<T>>;
    pub fn set_leaf_parent_ratio(&mut self, target: T, target_ratio: f32) -> bool;
    pub fn layout(&self, viewport: Rect, cell_w: f32, cell_h: f32, divider: f32, use_preview: bool) -> SplitLayout<T>;
    // dock.rs:
    pub fn joined(self, source: Self, side: SplitNav) -> Self;        // 根级 50/50
    pub fn dock_at_leaf(&mut self, target: T, source: Self, side: SplitNav) -> Result<(), Self>;
}
// 自由纯函数(照抄):
pub fn hit_divider<'a>(dividers: &'a [Divider], x: f32, y: f32, slop: f32) -> Option<&'a Divider>;
pub fn drag_ratio(direction: SplitDirection, viewport: Rect, divider: f32, cell: f32, x: f32, y: f32) -> f32; // 不钳制
pub fn preview_ratio(raw: f32) -> f32;          // 关闭区钉死 0.02/0.98,常规带钳 0.10-0.90
pub fn drag_close_target(raw: f32) -> Option<bool>;
pub fn commit_ratio(preview: f32, extent: f32, divider: f32, cell: f32) -> f32; // 吸附整格再硬钳
pub fn nav_target<T: Copy + Eq>(panes: &[(T, Rect)], focused: T, nav: SplitNav) -> Option<T>; // 非对齐轴 4 倍计距
```

运行时实例化形态:`SplitTree<PaneId>`。树内 `ratio` 永远描述第一个孩子;`set_leaf_parent_ratio` 目标在第二侧写 `1-ratio`、递归到最近父节点(防嵌套 resize 错改祖先)——语义照抄,测试随迁。

### WorkspaceTab 三件套(`slterm_app::gpui_shell::workspace`,锚点)

```rust
/// 一个终端 pane:视图实体 + 宿主订阅。id 全 workspace 唯一、终生不复用
/// (AI hook 的 SLTERM_PANE_ID 环境值与此同源,eg 03 篇)。
pub struct TerminalPane {
    pub id: PaneId,
    pub custom_name: Option<String>,     // 用户重命名(持久化归 session 叶)
    pub view: Entity<TerminalView>,      // 终端视图归 02/M3;本体本片不展开
    _subscription: Subscription,
}

/// 唯一合法 tab 类型集(面板封闭的 Rust 落点,spec 采纳点 27/15)。
/// 新增类型 = 变体 + 渲染分支 + 关闭/持久化语义归对应分片(06 设置 / 07 文件)。
pub enum WorkspaceTab {
    Terminal {
        panes: Vec<TerminalPane>,        // 实体属主,无序按 id 查
        tree: SplitTree<PaneId>,         // 叶 = pane id
        focused: PaneId,                 // 键盘焦点与 split/close 作用对象
        zoomed: bool,                    // 聚焦 pane 满卡(内存态,快照不含)
    },
    Settings { view: Entity<SettingsPane> },          // 单例,归 06
    Image    { view: Entity<ImageTabView> },          // 只读图片归 07
    Document { view: Entity<DocTabView> },            // markdown/文本归 07
    Code     { view: Entity<CodeTabView> },           // 源码只读归 07
}

/// tab + 展示态元数据同槽存放(pebrel 为 tabs/tab_meta 平行 Vec 下同标合同,
/// 本篇收拢为单 Vec<TabEntry>——消下同标耦合;TabMeta 展示字段归 07 篇演化)。
pub struct TabEntry { pub id: TabId, pub tab: WorkspaceTab, pub meta: TabMeta }
pub struct TabMeta {
    pub custom_name: Option<String>,     // tab 级用户命名(快照照抄,spec 采纳点 13 的 color 同)
    pub color: Option<Rgb>,              // 侧栏色标归 06 调色板
    pub launch: Option<LaunchSession>,   // 快照时 tab 启动身份
    // shell_tag/bell 等运行时展示态归 07/09,不整体搬迁(spec 不采纳点 13)
}

pub struct Workspace {
    tabs: Vec<TabEntry>,
    active: usize,                       // 位置索引(活跃定位);TabId 为稳定身份
    next_pane: u64, next_tab: u64,       // 单调分配器(PaneId/TabId 唯一性权威)
    sidebar: SidebarState,               // 侧栏骨架态:开合/选中归下
    // SplitBoundsStore/PaneBoundsStore(canvas 回写归 M3,本篇登记契约见数据流节)
}
```

与 pebrel 的形态偏差共三处,均不改变行为语义:`u64`→`PaneId`/`TabId` newtype;tab 身份由位置索引升级为稳定 `TabId`(runtime API归 04 需要跨重排的稳定寻址,pebrel 的位置索引在 drag 重排后会漂移);`broadcast` 字段随 spec 不采纳点 6 删除。

### session schema(`slterm_app::session`,本篇锚点)

```rust
/// 最高格式版本。pebrel 为 4(v1–v3 升级链服务其历史用户);slTerminal 无历史
/// 会话文件,重启为 1——升级链代码不迁,「追加字段走 serde(default) 免升版」保留。
pub const VERSION: u32 = 1;
pub const MAX_BOOT_ATTEMPTS: u32 = 3;

/// tab 首 pane 的启动身份(可持久化的 LaunchSession 子集;仅本地形态)。
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchSession {
    Default,
    Profile { name: String, command: String, args: Vec<String>,
              cwd: Option<String>, #[serde(default)] shell_id: Option<String> },
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis { LeftRight, TopBottom }

/// 快照瞬间 pane 前台 AI CLI 对话身份归 03 篇消费;本篇只锚持久化挂载点与
/// 「叶子 = 恢复注入单位」契约(spec 采纳点 26)。只存安全启动描述。
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSession {
    pub source: String,                                   // hook 直报的 CLI 名(eg 03)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
}

/// 持久化布局树:叶带逐 pane 启动环境,Split 带轴 + 第一个孩子占比(千分比
/// 整数——变化检测与文件 diff 不被 f32 噪声绊倒,pebrel 同因)。
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutSession {
    Pane {
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")] custom_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")] agent: Option<AgentSession>,
        #[serde(default, skip_serializing_if = "Option::is_none")] launch: Option<LaunchSession>,
    },
    Split { axis: SplitAxis, ratio_permille: u16,
            first: Box<LayoutSession>, second: Box<LayoutSession> },
}
impl LayoutSession {
    pub fn pane_count(&self) -> usize;
    pub fn first_cwd(&self) -> &str;      // 恢复时首 pane 的种子目录
    pub fn leaves(&self) -> Vec<&LayoutSession>;   // DFS 序 = 运行时 pane 创建序(索引配对权威)
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct TabSession {
    pub cwd: String,                     // 焦点 pane 目录;boot 路径在树重建前种子首 pane
    #[serde(default)] pub custom_name: Option<String>,
    #[serde(default)] pub color: Option<Rgb>,
    #[serde(default)] pub launch: Option<LaunchSession>,
    #[serde(default)] pub layout: Option<LayoutSession>,   // 缺省 = 单 pane at cwd
    #[serde(default)] pub active_pane: usize,              // DFS 下标,越界回退首叶
}

/// 只写不回放(spec 不采纳点 12):诊断 + 前向兼容,启动按配置列行数定形。
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct WindowState { pub width: u32, pub height: u32, #[serde(default)] pub maximized: bool }

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub version: u32,
    #[serde(default)] pub boot_attempts: u32,   // 断路器:活到首次自动保存即归零
    pub active_tab: usize,
    pub tabs: Vec<TabSession>,
    #[serde(default)] pub window: Option<WindowState>,
    // clean_exit:1Hz 恒 false,正常收尾最后一笔 true;老文件缺省 false(多一条提示而已)
    #[serde(default)] pub clean_exit: bool,
    // window_layout 段不迁(多窗砍,边界条 4)
}

// 自由函数(照抄 pebrel session.rs 语义):
pub fn load() -> Option<Session>;            // 数据目录 session.json(路径归 01 篇 dirs)
pub fn load_from(path: &Path) -> Option<Session>;        // workspace 导入同一 schema
pub fn try_save(session: &Session) -> io::Result<()>;    // 紧凑写,原子写归 atomic_file
pub fn save_to(path: &Path, session: &Session) -> io::Result<()>;  // 导出 pretty 打印
pub fn should_restore(s: &Session) -> bool;  // boot_attempts < MAX && !tabs.is_empty()
pub fn was_crash(s: &Session) -> bool;       // !clean_exit && !tabs.is_empty()
pub fn save_final(s: &mut Session);          // 收尾:打 clean_exit=true 再落盘
pub fn quarantine() -> Option<PathBuf>;      // 挪(非删)到 session.crashed.json
pub fn mark_boot_attempt(s: &mut Session);   // 恢复尝试前先计数落盘
pub fn valid_dir(cwd: &str) -> Option<PathBuf>;  // 目录已消失则回退默认 cwd
```

JSON 实例(快照形态,即 session.json 也是 `slterm-workspace.json` 导出格式):

```json
{
  "version": 1,
  "boot_attempts": 0,
  "active_tab": 1,
  "tabs": [
    {
      "cwd": "D:/work/api",
      "custom_name": "后端",
      "color": [97, 175, 239],
      "launch": { "kind": "profile", "name": "pwsh-elevated",
                  "command": "pwsh.exe", "args": ["-NoLogo"], "cwd": "D:/work/api" },
      "layout": {
        "kind": "split",
        "axis": "left_right",
        "ratio_permille": 618,
        "first": {
          "kind": "pane", "cwd": "D:/work/api",
          "agent": { "source": "claude", "session_id": "0199a213-c2a4-7cf5-8f6b-d746fbb6e86c" }
        },
        "second": {
          "kind": "split",
          "axis": "top_bottom",
          "ratio_permille": 500,
          "first": { "kind": "pane", "cwd": "D:/work/api/logs" },
          "second": { "kind": "pane", "cwd": "" }
        }
      },
      "active_pane": 2
    },
    { "cwd": "D:/work/web", "custom_name": null, "active_pane": 0,
      "layout": { "kind": "pane", "cwd": "D:/work/web" } }
  ],
  "window": { "width": 1280, "height": 720, "maximized": false },
  "clean_exit": false
}
```

### 布局转换对(`slterm_app::gpui_shell::session_restore`,照抄形态)

```rust
/// 运行时树 → 持久化树。比例 f32→permille(clamp 0.05..0.95 后 round);
/// 叶数据经闭包按 PaneId 注入(cwd/agent/launch/custom_name)。
pub fn layout_from_tree(
    tree: &SplitTree<PaneId>,
    leaf_data: &impl Fn(PaneId) -> (String, Option<AgentSession>, Option<LaunchSession>, Option<String>),
) -> LayoutSession;

/// 持久化树 → 运行时树。叶 id 由 alloc 逐叶分配(DFS 序),返回 id 列表与
/// LayoutSession::leaves 的 DFS 序一一对应——恢复注入(逐 pane cwd/启动身份)
/// 靠该配对;比例 permille→f32(同 clamp)。全仓唯一摸持久化树形状的函数对。
pub fn tree_from_layout(
    layout: &LayoutSession,
    alloc: &mut impl FnMut() -> PaneId,
) -> (SplitTree<PaneId>, Vec<PaneId>);
```

「布局单点」硬约束(#7)的本世界落点:持久化布局只经 session schema 存取,运行时分屏树与持久化树之间只有这一对纯函数;Dockview 时代的页切片/宿主汇编/迁移修补无存在前提。机械保障 = 模块私有化(`LayoutSession` 构造只在此对函数内)+ 架构门禁 grep 断言(eg 11 篇登记)。

### 会话元数据单点对应物(硬约束 #8,`slterm_app::terminal_registry`,新建)

双层权威:**Workspace 是 pane 实体的唯一属主**(panes Vec,生命周期与回收全走它,pebrel 不变式照抄);`terminal_registry` 是**模块级单例索引**,供 workspace 之外的消费者(eg 03 hook 域 / 04 runtime)按 `PaneId` 查询会话元数据——反向不持有实体、不裁决生命周期。

```rust
// slterm_app/src/terminal_registry.rs —— 模块级单例
// (契约形态对齐 slTerminal 旧 TS 注册表:register/get/remove/subscribe;
//  _reset 仅测试归 11 篇门禁)
pub struct PaneSessionMeta {
    // AI 会话身份归 03 篇字段级定义(spec 采纳点 26);此处只锚索引形状
    pub agent: Option<AgentSessionRef>,   // eg 03
    pub shell_kind: ShellKind,            // eg 02 篇锚点
    pub prompt_ready: bool,               // 语义 prompt 首标记归 02 事件
}
pub fn register(id: PaneId, meta: PaneSessionMeta);
pub fn get(id: PaneId) -> Option<PaneSessionMeta>;
pub fn remove(id: PaneId);
pub fn subscribe(f: impl Fn(&RegistryEvent) + 'static) -> Subscription;
```

纪律:索引写入点唯一(spawn/关 pane 两处);面板只订阅、不自存会话元数据(旧约束 #8 的 Rust 重建)。注册表本体与 PaneId/TabId 的消费关系在 M3 集成时校准(02 篇开放问题 2 的承接,本片不 preempt 壳实现)。

### 侧栏骨架 + sideViews 槽位(`workspace::sidebar`,重建)

slTerminal 旧侧栏三件套(sideViews/navTree/titleBar)经 workspace 侧栏骨架重建;**骨架与槽位模型归本片,视图内容归对应分片**(explorer eg 07、agentFiles eg 03、commit eg 07、navTree 数据源归 D05-2)。

```rust
/// 侧栏骨架态:单窗口下的单槽位模型(形态决策见改造节)。
pub struct SidebarState {
    pub collapsed: bool,                  // 开合(内存态;设置键归 06)
    pub active_view: Option<SideViewId>,  // 当前可见 side view(单槽)
    pub width: f32,                       // 拖宽(持久化归 06 设置键)
}
/// side view 注册项;SideViewId 封闭集归 07 篇,icon归主题归 06。
pub struct SideViewDef { pub id: SideViewId, pub title: String }
```

titleBar(自绘窗口标题栏 + 最小化/最大化/关闭三钮 + 拖拽区)是窗口 chrome 而非布局,壳重建归 09 篇;本片只登记它与 workspace 骨架的交界:标题数据源 = 活跃 tab 的展示名归 07 篇 TabPresentation、窗口三钮消息归 09 篇窗口域。

## 数据流与状态机

### 分屏渲染与矩形回写(M3 建立、M5 扩展,交界归本片)

```
Workspace::render
  └─ render_terminal_tab(active tab)
       ├─ zoomed 或单叶 → 聚焦/首 pane 满卡直渲(canvas 探针回写 pane 矩形)
       └─ 否则 render_split_node 递归:
            SplitTree::layout(提交比例, use_preview=false) → 每叶 Rect
            → GPUI flex 树铺陈;每个 Split/pane 挂 canvas 探针
            → prepaint 阶段回写 SplitBoundsStore / PaneBoundsStore
              (Rc<RefCell>,键 = (tab 下标, 节点路径) / pane id)
拖拽换算与 nav_target 读上一帧记录;终端网格尺寸由 TerminalElement
prepaint 观测矩形 → 网格 → PTY 下发(eg 02 篇 viewport 合同)
```

不变式:渲染只读**提交比例**;`preview_ratio`/`dragging` 只喂 overlay 与分隔条跟随,PTY 尺寸直到松手 `commit_ratio` 落定才变(spec 采纳点 11)。

### 分隔条拖拽状态机(spec 采纳点 11 照抄语义)

```
pointer-down 命中 hit_divider(HIT_SLOP 按 scale 换算)
  → SplitDrag { tab, path, direction, preview_ratio, close_target, last_notified }
pointer-move(高频)
  → drag_ratio(不钳制) → preview_ratio(三段曲线) + drag_close_target
  → 仅更新 overlay(split_drag_visual_geometry:分隔线 + 关闭区高亮)
     pane flex 树保持提交比例,终端网格零重排
  → 刷新 8ms 节流;close_target 变化即时刷新
pointer-up
  ├─ Some(关闭侧) → 同 close_pane 路径(RemoveOutcome 三态)
  └─ 否则 commit_ratio(吸附整格 + 硬钳) 写回树节点 → mark_structural_resize
```

### zoom 状态机(spec 采纳点 14)

聚焦 pane 满卡开关;任何结构性操作(split/close/导航/点击别的 pane)先解除;几何从半卡跳整卡按结构性 resize 立即下发(eg 02 篇 mark_structural_resize 交界)。`zoomed` 内存态,快照不含(边界条 6)。

### 关 pane / 关 tab 状态机(不变式驱动)

```
close_pane(tab, pane_id)
  → tree.remove_leaf → RemoveOutcome:
      NotFound → 空操作
      WasRoot  → close_tab(tab)
      Collapsed(next_focus) → 摘 pane + shutdown + 焦点交幸存子树首叶
close_tab(ix) → 逐 pane shutdown → 实体引用清零(TerminalView::drop 兜底)
```

被摘 pane 立即显式 shutdown,失败路径(建树挂不上)立即回收 pane,不留孤儿 PTY;`pane_bounds` 索引同步摘除(不变式,测试硬判据)。

### tab 拖拽停靠状态机(spec 采纳点 16)

```
tab 级拖拽(threshold 4px)→ DockTarget { pane: Option<PaneId>, nav: SplitNav }
  pane=None → 窗口边缘根级停靠(outer rim)
  pane=Some → pane 矩形四分位最近边(nearest_edge)
overlay 预览 dock_preview_area;pointer-up 提交:
  dock_tab_into_active = SplitTree::joined / dock_at_leaf 纯函数
  目标在手势途中消失 → 原样退回(Err(source) 语义)
```

### 会话快照与写盘状态机(单窗化改造,照抄机制)

```
1Hz autosave_tick → snapshot_session → SessionPersistence::save(Checkpoint):
  latest vs saved 相等 → 跳过(无变化不写盘)
  失败只 warn,不拖垮终端;成功后 saved=latest,首次成功快照带 boot_attempts=0
tabs 全关(TabsClosed)→ 空快照落盘(clean_exit=true;空会话不恢复)
关窗(WindowClose)→ 最后一笔 clean_exit=true
启动:load → should_restore?
  否(boot_attempts≥3 且非空)→ quarantine 挪走 session.crashed.json,干净启动 + 提示
  是 → mark_boot_attempt(计数落盘)→ 逐 tab restore_tab → toast 汇总恢复结果
```

### 恢复注入顺序(spec 采纳点 25 照抄)

```
restore_tab(tab):
  layout = tab.layout ?? 单 pane at tab.cwd
  逐 leaf(DFS 序)spawn pane:cwd 经 valid_dir 校验,launch 取 leaf.launch
    ?? (index==0 ? tab.launch : 默认本地启动归 02);agent 归 03 restore_agent
  tree_from_layout(alloc=pane id 序列)→ 建树挂不上立即回收 pane
  focused = panes[active_pane] ?? 首叶;插入位置=尾部(恢复期不套新 tab 策略)
```

## 照抄拷贝清单 + 改名映射引用 + 缝合点

### 照抄拷贝清单(薄写;每项一行:pebrel 源 · 符号 + 缝合点 + 因果链一句)

| pebrel 源(路径 · 符号) | 缝合点 | 因果链一句 |
| --- | --- | --- |
| `nebula_split/src/lib.rs` `SplitTree`/`Rect`/`Divider`/`SplitLayout`/`SplitDirection`/`SplitNav` | `slterm_split/src/lib.rs` 全量改名 | 纯函数分屏规则 = 布局语义唯一权威,零 UI 类型是独立成 crate 的前提 |
| `nebula_split/src/lib.rs` `layout`/`collect_rects` 切割数学(floor→双向钳制→余数) | 同上全量 | 与旧壳逐字一致的分屏几何,任何偏差即用户可感布局漂移 |
| `nebula_split/src/lib.rs` `DIVIDER_GAP`/`HIT_SLOP`/`hit_divider` | 同上全量 | 视觉 2px 与命中 8px 的刻意分离,成文常量防"修偏移"式回归 |
| `nebula_split/src/lib.rs` `CLOSE_MARGIN`/`RATIO_CLAMP`/`drag_ratio`/`preview_ratio`/`drag_close_target`/`commit_ratio` | 同上全量 | 三段拖拽曲线(跟手/钉边塌缩/提交吸附)是拖关交互的完整语义 |
| `nebula_split/src/lib.rs` `SplitNav`/`nav_target` 4 倍漂移惩罚 | 同上全量 | 方向导航尽量停留同行/列,惩罚系数是行为契约 |
| `nebula_split/src/lib.rs` `RemoveOutcome`/`remove_leaf` 三态塌缩 | 同上全量 | 关 pane 全部结局由树裁定,壳不散判——不变式的执行器 |
| `nebula_split/src/lib.rs` `split_leaf`/`replace_leaf`/`set_leaf_parent_ratio`/`node_mut`/`contains`/`first_leaf` | 同上全量 | 树编辑原子操作集;1-ratio 与最近父语义防嵌套 resize 错改祖先 |
| `nebula_split/src/dock.rs` `joined`/`dock_at_leaf` | `slterm_split/src/dock.rs` 全量 | 整组分屏拖进另一分屏的几何基础;目标消失原样退回 |
| `nebula_app/src/gpui_shell/workspace.rs` 模块头「分屏 pane 生命周期合同」 | 改写为 `slterm_app` workspace 模块头 | panes==树叶集合等四不变式是全篇测试硬判据,合同先于实现 |
| `nebula_app/src/gpui_shell/workspace.rs` `WorkspaceTab` 三件套 + `TerminalPane` | 参考重建(PaneId/TabId 化归本篇) | 封闭枚举 = 面板封闭的 Rust 落点;三件套结构照抄 |
| `nebula_app/src/gpui_shell/workspace.rs` `SplitBoundsStore`/`PaneBoundsStore`/`render_split_node`/`to_split_rect` | M3 建立 canvas 回写,M5 扩展 | 矩形由树裁定、prepaint 回写,导航/拖拽读上一帧 |
| `nebula_app/src/gpui_shell/workspace.rs` `SplitDrag`/`split_drag_visual_geometry`/`update/finish_split_drag` | `workspace/split_drag.rs` 重建 | 拖拽只动 overlay,PTY 跟随提交值——高频事件不重排网格 |
| `nebula_app/src/gpui_shell/workspace.rs` `toggle_zoom` | 同上重建 | 结构性操作先解除 zoom;满卡跳变走立即下发 |
| `nebula_app/src/gpui_shell/workspace/tab_drag.rs` `DockTarget`/`TabDrag`/`dock_nav_at`/`dock_preview_area` | `workspace/tab_drag.rs` 重建 | Dockview 原生拖拽的替代形态;四分位 + 落点消失退回 |
| `nebula_app/src/gpui_shell/workspace.rs` `dock_tree` | 同上重建 | 壳侧薄封装:`joined`/`dock_at_leaf` 的唯一生产调用点 |
| `nebula_app/src/session.rs` `Session`/`TabSession`/`LayoutSession`/`SplitAxis`/`LaunchSession`/`AgentSession`/`WindowState` | `slterm_app/src/session.rs` 照抄裁剪(Ssh/Shell 变体砍) | schema 整体照抄是「快照即导出格式」的根基;语义重定位见改造节 |
| `nebula_app/src/session.rs` `LayoutSession` permille(`ratio_permille`) | 同上全量 | f32 噪声不进变化检测与文件 diff;整数千分比是演进纪律的载体 |
| `nebula_app/src/session.rs` `VERSION`/`parse` 就地升级链 | 只抄机制不抄链:`VERSION=1`,`parse` 裁为「版本不符即 None」 | 无历史用户,升级链无存在前提;serde(default) 免升版纪律保留 |
| `nebula_app/src/gpui_shell/session_restore.rs` `layout_from_tree`/`tree_from_layout` | `gpui_shell/session_restore.rs` 照抄(u64→PaneId) | 「布局单点」唯一落点:全仓唯一摸持久化树形状的函数对 |
| `nebula_app/src/gpui_shell/workspace/session_persistence.rs` `SessionPersistence`/`SaveReason`/`save_with` | `workspace/session_persistence.rs` 照抄裁剪(单窗化,去逐窗 Vec) | 1Hz 快照 + latest/saved 无变化跳过 + 失败只 warn;`isolated` 语义保留(提权隔离) |
| `nebula_app/src/gpui_shell/workspace/windowing.rs` `autosave_tick` | 单窗化:1Hz tick 直调 workspace 快照 | 多窗逐窗快照组合随 windowing 多窗臂消亡;tick 节奏本身照抄 |
| `nebula_app/src/gpui_shell/workspace/session_recovery.rs` `snapshot_session`/`try_restore_session`/`restore_tab` | `workspace/session_recovery.rs` 照抄裁剪(去 guest/SSH/detached 臂) | 先逐 pane spawn 后建树的注入顺序 + toast 汇总;恢复粒度的执行器 |
| `nebula_app/src/session.rs` `MAX_BOOT_ATTEMPTS`/`should_restore`/`mark_boot_attempt`/`quarantine` | 同上全量 | 断路 + 挪不删:启动成功一秒后的自动保存会盖掉「一恢复就崩」的唯一诊断材料 |
| `nebula_app/src/session.rs` `clean_exit`/`save_final`/`was_crash` | 同上全量 | 1Hz 恒 false、收尾唯一笔 true;空会话不算崩溃 |
| `nebula_app/src/atomic_file.rs` `write`/`try_lock`/`try_lifetime_lock`/`replace` | `slterm_app/src/atomic_file.rs` 照抄 | 每秒一写的文件崩在半写不能赔掉它存在的意义;跨进程锁防双实例互踩归 04 交接 |
| `nebula_app/src/session.rs` `try_save`/`save_to`(快照紧凑写 / 导出 pretty) | 同上全量 | 同一 schema 双写形态:机读紧凑 + 人读可 diff 的版本化产物 |
| `nebula_app/src/gpui_shell/workspace.rs` `export_workspace` | 重建(整窗导出;单 tab 导出臂归 07 裁定) | session.json 即 workspace 导出格式的 UI 入口 |
| `nebula_app/src/gpui_shell/workspace/sidebar.rs` tab 列表渲染(徽章/新建钮/折叠槽) | `workspace/sidebar.rs` 参考重建 | 左侧垂直 tab 侧边栏骨架;徽章事件语义归 07/09 |

### 改名映射引用

本篇一切改名以 01 篇「改造 / 移植 / 新建设计」节的**改名映射单点表**为唯一权威,本篇不另立映射;直接消费点:`nebula-split` → `slterm-split`(A 节)、`pebrel-workspace.json` → `slterm-workspace.json`(D 节,本片 export_workspace 落点)、`session.json`/`session.crashed.json` 文件名 = 不变(D 节)、`NEBULA_PANE_ID`/`PEBREL_PANE_ID` → `SLTERM_PANE_ID`(C 节,per-pane 环境归 03 篇,本片只锚 PaneId 类型)。

### 缝合点

1. **TTY 尺寸链 × 分屏树**(归 02 篇):`SplitTree::layout` 产叶 Rect → `TerminalElement` prepaint 观测 → 网格 → PTY;burst+settle 与 `mark_structural_resize` 交界合同照 spec 采纳点 12,本片只登记「渲染只读提交比例」一侧。
2. **注册表本体 M3 校准**:`terminal_registry` 单例与 PaneId/TabId 的消费关系在 M3 集成时校准(02 篇开放问题 2 的承接),本篇只锚索引形状与写入点纪律。
3. **AgentSession 字段级定义归 03**:本片锚持久化挂载点(`LayoutSession::Pane.agent`)与「叶子 = 恢复注入单位」契约;`AgentSessionRef`/`restore_agent` 的形状归 03 篇。
4. **Settings 单例 tab 归 06**:`WorkspaceTab::Settings` 变体的渲染分支与关闭语义(单例不持久化)归 06 篇;本片只锚变体存在。
5. **Document/Image/Code tab 归 07**:三变体的渲染、tab 路由、单 tab 导出臂归 07 篇;`TabMeta` 展示字段(图标/标题派生)随 07 演化。
6. **titleBar 归 09**:窗口三钮与拖拽区是窗口 chrome;本片只登记「标题数据源 = 活跃 tab 展示名」交界。
7. **session 数据目录归 01**:`session.json` 落点(`platform::dirs::data_dir`)归 01 篇锚定,本片只消费。

## 改造 / 移植 / 新建设计

### session v4 语义重定位:schema 不变、写盘时机收窄

spec 采纳点 17-26 照抄 schema、不采纳点 5 砍掉驻留语义后,session.json 只回答**「下次启动恢复成什么样」**。对 schema 与写盘时机的影响:

- **schema 面零改动**:`Session`/`TabSession`/`LayoutSession`/`LaunchSession`/`AgentSession`/`WindowState` 字段级照抄(pebrel 为 v4,本片 `VERSION=1` 重启,升级链不迁);砍的只有 `LaunchSession::Ssh`/`Shell` 两变体与 `window_layout` 段——这是裁剪,不是语义重定位。
- **写盘时机收窄为四条路径**:(a) 1Hz `autosave_tick` 快照;(b) 关窗收尾 `save_final`;(c) 启动恢复前 `mark_boot_attempt` 计数一笔;(d) workspace 导出(`save_to` pretty)。pebrel 侧随 mux/多窗存在的「关窗后会话继续存活、逐窗 per-window snapshot 组合、detached tab 恢复、按 WindowRole 过滤」全部无存在前提——**关窗即退出**(跨领域不变量 5),最后一笔 clean_exit=true 后进程消亡,不再有「窗关了但 session 还活着」的中间态。
- **断路语义保留且更紧要**:无 mux 驻留后,「恢复即崩」的循环直接暴露为启动失败,`boot_attempts`/`quarantine` 是唯一防线;`quarantine` 挪不删的因果链(诊断材料会被一秒后的自动保存盖掉)原样照抄。
- **单窗化对 `SessionPersistence` 的收敛**:pebrel `update_windows` 返回逐窗 Vec 再组合的多窗臂删;`latest`/`saved` 双快照、`SaveReason` 状态机、`isolated`(提权隔离不写共享存储)语义保留——单实例下管理员/普通进程互不从对方窗口写 session 的问题,在单窗世界退化为「同机双身份进程互不覆盖会话文件」,`try_lock` 跨进程锁仍是裁决者(与 04 篇单实例移交的锁关系在 04 篇登记)。
- **空会话语义照抄**:tabs 全关 → 空快照落盘 `clean_exit=true`;`should_restore` 的 `!tabs.is_empty()` 守卫即「空会话不恢复」的机械表达,无对话框、无提示。

### Dockview 面板族映射进封闭枚举

旧 `PANEL_TYPES`(terminal/editor/htmlviewer/markdownviewer/gitshow/diff/settings)+ `panelRegistry.ts` 白名单 + `FILE_PANEL_TYPES` + `withPanelBoundary` + `isAlwaysRenderPanel` 恒挂载白名单整面消亡,映射为 `WorkspaceTab` 封闭枚举(锚点见上):

| 旧面板(Dockview) | 新落点 | 归篇 |
| --- | --- | --- |
| `terminal` | `WorkspaceTab::Terminal` 的 pane 叶(分屏树承载) | 本片 + 02 |
| `settings` | `WorkspaceTab::Settings` 单例 tab | 06 |
| `editor` | `WorkspaceTab::Code`(双模态编辑 eg M8 演化,初态只读) | 07 |
| `markdownviewer`/`htmlviewer` | `WorkspaceTab::Document`(归 07 裁定细分) | 07 |
| `gitshow`/`diff` | `WorkspaceTab::Document`/`Code` 只读臂(归 07) | 07 |
| docViewer 共享底座/恒挂载保活 | 无对应物:xterm 不可二次 `open()` 约束在 Rust 原生渲染下不复存在(不采纳点 1) | — |
| 页前缀协议/跨页守卫/页组 setVisible | 无对应物:单窗口单 workspace,无「页」概念 | — |

「面板封闭」硬约束(#5)的 Rust 落点 = 封闭枚举 + 新增类型三件套(变体 + 渲染分支 + 关闭/持久化语义);fromJSON 白名单校验消亡,合法性由类型系统机械保证。

### navTree / sideViews / titleBar 侧栏骨架重建

- **形态取 pebrel 骨架**:左侧垂直 tab 侧边栏 + 主内容区,tab 列表渲染照 pebrel `workspace/sidebar.rs`(新建钮/折叠槽/徽章位)。**不采纳** pebrel 的 `tabs` 位置配置化与折叠(不采纳点 7)。
- **sideViews 双槽模型重建**:slTerminal 旧 `sideViews`(ActivityBar + 上下双槽 SideBarArea,`toggleViewPure`/`deriveLayout`/`reconcileZones` R1-R9 纯函数族)是既有产品面,骨架照旧重建为 Rust 侧栏骨架的一部分:活动栏按钮区 + 双槽(上/下)内容区,`SideViewRegistry` 形态收敛为 `SideViewDef` 封闭集(id + title,icon 归 06,内容归 07/03);R1-R9 状态函数以纯函数形态随迁(`deriveLayout` 的双槽布局推导可机测)。**本片「关键类型与签名」节的 `SidebarState` 单槽草稿即双槽/单槽裁定的挂账处**,按 D05-2 收敛(单槽草稿不 preempt 决策)。
- **navTree 数据源重接**:旧 navTree(项目 → 页面 → 活跃会话 → 历史会话)的「页面」层级随页组模型消亡;「项目」层归 01 篇开放问题 4 提取的 `slterminal-projects.json` 数据裁决(默认提取),重建后的层级与数据源归 D05-2 所在篇(本篇只登记骨架槽位)。
- **titleBar**:窗口 chrome 归 09;本片只消费「活跃 tab 展示名 → 窗口标题」的只读交界(eg 07 篇 TabPresentation)。

### 与 02 终端面板、07 编辑器 tab 的缝合契约

- **02 篇契约**:`TerminalPane.view: Entity<TerminalView>` 为本片持有的唯一实体引用;spawn/shutdown 语义归 02(默认 shell 回退链、PTY 生命周期);本片保证「被摘 pane 立即 shutdown、失败路径立即回收」的调用纪律,PTY 网格尺寸链只在提交比例变化时经 02 的 `mark_structural_resize` 落定。`ShellKind`/`prompt_ready` 元数据经 `terminal_registry` 单点写入,本片不二次缓存。
- **07 篇契约**:Document/Image/Code 三变体的关闭语义 = 无进程可回收(纯视图 drop),持久化语义 = 不进 session(边界条 6);单 tab 导出臂(eg `export_workspace(Some(ix))`)归 07 与整窗导出共用 schema;`TabMeta` 展示字段的演化归 07,本篇锚的 `TabMeta` 只含持久化四字段(custom_name/color/launch + id)。
- **04 篇契约**:TabId 稳定身份供 runtime API 跨重排寻址归 04;本片只保证 TabId 单调分配、终生不复用。

## 测试点清单

> 引测试一律例名;测试基础设施形态(虚拟窗口、夹具、门禁)归 11 篇,本篇只列领域用例。slterm_split 纯函数用例随照抄整体迁移(pebrel 对应例名照抄不改,下表「迁移」列);壳层与 session 用例按本篇不变式新建。

| 测试 | 层级 | 机制 |
| --- | --- | --- |
| `split_and_remove_roundtrip` | L1 slterm_split | 迁移(pebrel `nebula_split/src/lib.rs` 同例) |
| `replace_leaf_preserves_depth_first_position` | L1 | 迁移;DFS 序 = 恢复注入配对权威的守卫 |
| `layout_splits_left_right_with_floor_and_divider` | L1 | 迁移;floor→双向钳制→余数切割数学 |
| `layout_clamps_each_side_to_a_cell` | L1 | 迁移;每侧至少一格 |
| `layout_nested_paths_address_dividers` | L1 | 迁移;嵌套路径寻址 |
| `layout_preview_only_when_asked` | L1 | 迁移;`use_preview` 开关 = 「预览不动网格」的纯函数证据 |
| `divider_hit_uses_slop` | L1 | 迁移;视觉 2px 与命中 8px 分离 |
| `drag_ratio_tracks_pointer_unclamped` | L1 | 迁移;越界值 = 进入拖关领域 |
| `preview_curve_pins_close_zone` | L1 | 迁移;6% 关闭区钉 0.02/0.98 |
| `drag_close_targets_squeezed_side` | L1 | 迁移;被挤侧裁定 |
| `commit_snaps_to_whole_cells` | L1 | 迁移;提交吸附整格再硬钳 |
| `nav_prefers_aligned_panes` | L1 | 迁移;非对齐轴 4 倍漂移惩罚 |
| `node_mut_follows_paths` | L1 | 迁移;路径寻址 |
| `leaf_parent_ratio_is_local_and_target_relative` | L1 | 迁移;1-ratio + 最近父,防嵌套 resize 错改祖先 |
| `fifth_pane_only_splits_the_hovered_quadrant` | L1 slterm_split::dock | 迁移(pebrel `nebula_split/src/dock.rs` 同例) |
| `docking_preserves_a_multi_pane_source` | L1 | 迁移;整组子树嫁接保持内部结构 |
| 树编辑后「panes 的 id 集合 == 树的叶集合」不变式(split/close/restore 三路径) | L1 slterm_app workspace | 新建;不变式 1 的机械守卫 |
| `close_pane` 三态结局:NotFound 空操作 / WasRoot 关 tab / Collapsed 焦点交幸存子树首叶 | L1 | 新建;RemoveOutcome 消费的壳侧证据 |
| 被摘 pane 显式 shutdown + 建树失败回收无孤儿(注入失败路径) | L1 | 新建;失败路径用例,照「bugfix 附防复发」形态 |
| zoom 状态机:结构性操作先解除、半卡跳整卡走立即下发 | L1 | 新建 |
| session 往返:`layout_from_tree` ∘ `tree_from_layout` = 恒等(permille 精度内)+ 叶数据闭包注入按 DFS 序配对 | L1 session_restore | 新建;「布局单点」round-trip |
| schema 往返:`Session` serde JSON → parse → 字段级相等(version/clean_exit 缺省老文件形态) | L1 session | 新建;JSON 实例(本篇)即夹具 |
| `VERSION=1` 且版本不符文件 parse 拒收;追加字段 serde(default) 免升版 | L1 session | 新建;演进纪律 |
| `should_restore`:`boot_attempts>=MAX` 或 tabs 空 → false;空会话不算崩溃 | L1 | 迁移语义新建 |
| `mark_boot_attempt` 先计数落盘、首次成功快照归零(`boot_attempts=0`) | L1 | 迁移语义新建 |
| `quarantine` 挪不删:断路后 session.crashed.json 存在、原路径清空 | L1(临时目录隔离) | 迁移语义新建 |
| `save_final` 唯一笔 `clean_exit=true`;1Hz 快照恒 false;`was_crash` 判定 | L1 | 迁移语义新建 |
| `SessionPersistence` 无变化跳过(latest==saved 不写盘)+ 失败只 warn 不拖垮 + `isolated` 不写共享存储 | L1(临时目录 + 注入写函数) | 迁移 pebrel `unchanged_checkpoints_do_not_rewrite_storage`/`isolated_windows_never_write_shared_session_storage` 单窗化 |
| 原子写:崩在半写的 session.json 落盘不可见(临时文件 + rename)+ try_lock 双进程互斥 | L1(临时目录) | 迁移语义新建 |
| 恢复注入顺序:逐叶 spawn(DFS 序)→ 建树 → `active_pane` 越界回退首叶;`valid_dir` 目录消失回退默认 cwd | L1 | 迁移语义新建 |
| 恢复失败路径:建树挂不上立即回收已 spawn pane,不留孤儿 | L1 | 新建 |
| UI 关键路径:分屏渲染 + 分隔条拖拽 overlay(preview 不动网格、松手 commit)+ tab 拖拽四分位停靠 + 关 pane 收编 | L3/L4 虚拟窗口 | 新建;归 11 篇基础设施 |
| 快照=导出同 schema:导出 pretty 打印 → `load_from` 回读字段级相等 | L1 | 新建 |

## 阶段归属与出口标准

本片主体归 **M5 工作区与布局**(00-roadmap),细分步与可机验出口:

**M5.1 `slterm_split` 纯函数树落地**(依赖序中紧随 `slterm_settings`,实际与 M1 末段重叠——00-roadmap 依赖序已含 split,M1 内完成照抄改名即点亮)。
出口 = 上表 slterm_split 全部用例绿;`dependencies.toml` 零生产依赖契约校验过;禁名门禁过。

**M5.2 session schema + atomic_file + 转换对**:schema 族/原子写/`layout_from_tree`/`tree_from_layout` 落 `slterm_app`(此时 app 骨架已在 M3 点亮)。
出口 = session 往返/断路/quarantine/原子写用例全绿;「全仓唯一摸持久化树形状」grep 门禁断言过(eg 11 篇登记形态)。

**M5.3 Workspace 三件套 + 分屏接入**:`Workspace`/`WorkspaceTab`/`TabEntry` + 渲染递归 + canvas 回写 + split_drag overlay + zoom。
出口 = 不变式(panes==树叶)三路径用例绿;L3/L4 分屏渲染与拖拽关键路径过;`cargo test` 全绿。

**M5.4 tab 拖拽停靠 + dock 嫁接**:`tab_drag` 四分位 + `joined`/`dock_at_leaf` 生产接线。
出口 = dock 纯函数用例(迁移)绿;落点消失退回路径用例绿。

**M5.5 快照/恢复闭环**:persistence/recovery 单窗化 + 启动恢复 + export_workspace。
出口 = persistence/quarantine/注入顺序用例绿;「崩溃恢复 → 一秒内现场」E2E 路径过(eg L4)。

**与 M3 的接口契约**(M3 先点亮、本片扩不改):

- M3 交付:GPUI 窗口 + 单终端 pane + ConPTY 直连 + `TerminalView` 实体 + `SplitBoundsStore`/`PaneBoundsStore` 回写机制 + `terminal_registry` 单例本体。
- 本片对 M3 的唯一改动面 = 单 tab 单 pane 的 `Workspace` 包装层(panes Vec + tree 单叶 + focused),M3 的「壳即前端」消费链不动;渲染递归从「满卡直渲」扩展为递归,满卡路径保留为 zoom 特例。
- M3 期 session 不持久化;本片 M5.5 才接 persistence——M3-M5 之间关窗丢布局是过渡期预期态,不入不可用清单额外登记。

## 待沉淀决策

> [待沉淀] **D05-1 · WindowState 回放(「记住上次窗口大小」)**。真实权衡:pebrel v4 起只写不回放,启动按配置列行数定形(可预期、跨机一致);slTerminal 旧产品面无此功能,但单窗口终端的「记住大小」是普遍用户期望,字段已在 schema 里写着,回放只差消费代码。意外因素:与 09 篇窗口域(启动定形、特效、DPI)交叉,改动面不在本片。难逆点:一旦消费即为行为承诺,再退回只写不回放 = 砍功能;schema 字段语义(诊断 vs 行为输入)随之定死。截止:M5.5 快照闭环落地前定;边界条 5 已预告本篇。

> [待沉淀] **D05-2 · 侧栏骨架形态:双槽(sideViews R1-R9)vs 单槽(pebrel 骨架)**。真实权衡:双槽 = slTerminal 既有产品面(ActivityBar + 上下槽,纯函数族 R1-R9 可机测随迁),代价是骨架复杂度与 SideViewId 封闭集同步扩;单槽 = pebrel 骨架照抄最省,代价是砍掉既有产品能力。意外因素:navTree 数据源重接后的视图集规模未知(项目层级去留见开放问题 3),槽位需求随视图集变化。难逆点:`SidebarState` 形态与侧栏渲染在 M5.3 落地后改槽位 = 布局重写 + 持久化键重域。截止:M5.3 侧栏骨架编码前定;本篇「关键类型与签名」的 `SidebarState` 单槽草稿按本决策收敛。

> [待沉淀] **D05-3 · 侧栏状态(collapsed/width/active_view)归 session 还是设置域**。真实权衡:归 session = 跟布局恢复走,一秒内回到现场,代价是 1Hz 快照面掺入偏好态、设置页看不到;归设置 = 全局偏好、写通道统一(eg 06),代价是「布局恢复」不再完整(侧栏开合不随现场)。意外因素:双槽形态(D05-2)直接改变本决策的字段面。难逆点:两域写入时机不同(1Hz 快照 vs 设置写通道),落地后迁移 = 数据搬迁义务。截止:M5.5 与 M6 交界定(依赖 D05-2 先定)。

## 开放问题

1. **非聚焦 pane 压暗 veil**(spec 采纳点 13 参考):pebrel `render_split_node` 的 veil 分支 + `NEBULA_UNFOCUSED_SPLIT_DIM` 是产品面偏好而非布局机制;M5 照抄几何时带不带 veil 主题开关归 06 联动。默认:M5 带上 veil 渲染但主题开关归 06 键域。
2. **tab 右键菜单 / tab 复制**(pebrel `tab_menu.rs`/`tab_duplication.rs`):pebrel 有现成实现,但菜单内容(重命名/复制/关闭/导出)跨 07/04 领域;M5 范围只含拖拽/停靠,菜单归不归 M5 未裁定。默认:菜单归 M8 与编辑器 tab 面一起定。
3. **`slterminal-projects.json` 提取后的「项目」层级去留**(01 篇开放问题 4 的承接):提取默认已定,但 navTree 重建后「项目 → 会话」层级是否保留、projects 数据归本篇还是 07 篇数据源未定;影响 D05-2 的视图集规模。默认:保留层级、数据归本篇消费,eg 07 只读引用。
4. **pane 标题条**(pebrel `pane_header.rs` + 因果 note `2026-09-28-pane-titles`):分屏后每 pane 的标题展示(目录/徽章)是 07 TabPresentation 的 pane 级演化还是本篇 TerminalPane 内建字段未定。默认:字段归 `TabMeta`/`PaneSessionMeta` 演化,eg 07 裁定展示。
5. **`slterm-workspace.json` 导出入口的 UI 归属**:pebrel 走 palette(`ExportWorkspace` 动作);slTerminal palette 归篇未定(04 CLI 也能导出)。默认:导出函数归本片,入口归 04/07 各自接线,不双份实现。
