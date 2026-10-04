# 分片 05:工作区与布局

## 优化面

slTerminal 工作区 = WebView 侧 Dockview 共享宿主 + 页组派生归属模型:`src/workspace/layoutSerde.ts` 单点序列化宿主全量 `toJSON` 再按页切片入 store,恢复经 `composeHostLayout` 汇编 + `loadPageGroup` 运行期并入(白名单过滤、旧格式迁移、深拷贝防污染、restoreGuard 守卫、`syncHostLayoutToStore` 写回链全在这一层);面板封闭于 `src/panelRegistry.ts` 注册的固定组件面(terminal/editor/htmlviewer/markdownviewer/gitshow/diff/settings,docViewer 为预览类面板共享底座),页组可见性 = dockview 叶级 `setVisible`,xterm 不可二次 `open()` 逼出 `renderer="always"` 恒挂载白名单;跨页守卫、聚焦意图令牌、页签右键菜单自研、标题集中管理(`titleManager`)等全是 Dockview 行为缺口的手工补偿。**布局语义本身全在 Dockview 网格模型里**——分屏朝向交替、拖拽/停靠/最大化全属库行为,仓内只有归属协议与守卫,没有自己的布局树。

pebrel 侧 = 左侧垂直 tab 侧边栏 + 主内容区:`nebula_app/src/gpui_shell/workspace.rs` 的 `WorkspaceTab` 持有 panes+tree+focused 三件套,不变式「panes 的 id 集合==树的叶集合」;布局语义全部下沉 `nebula_split` 纯函数 crate(零 UI 类型),壳只做渲染递归、canvas 矩形回写、拖拽 overlay 与松手提交;持久化 = session v4 schema(每 tab 完整分屏树 + 逐 pane 启动身份 + 聚焦叶 + 窗口态),1Hz 快照无变化跳过、`boot_attempts` 崩溃断路、`session.json` 即 workspace 导出格式。

裁定背景:Dockview 布局体系整体弃用;`nebula_split` 纯函数分屏树与 session v4 schema 照抄(全局已定);mux 驻留保活砍掉后 session 持久化定位重界定——回答「下次启动恢复成什么样」,不回答「关窗后谁还活着」;`nebula_*` → `slterm_*` 改名。本节列功能点粒度裁定。

## 采纳点

### 分屏树纯函数内核(照抄,全局已定)

1. **SplitTree<T> 二分树 + Rect 像素几何,照抄。** 叶子=pane id(泛型),内部节点=方向+提交比例+预览比例+dragging 标记;零 UI 类型、坐标系屏幕像素左上原点;树操作全是纯函数,天然独立成 crate。pebrel 侧:`nebula_split/src/lib.rs` 的 `SplitTree`、`Rect`、`SplitLayout`、`SplitDirection`。
2. **layout 切割数学,照抄。** 可用长度=总长-分隔条,第一段先 floor 再双向钳制到至少一个单元格,第二段吃余数;`use_preview` 开关让分隔条跟手而 pane 内容稳定;每 Split 产出一条 Divider(矩形+方向+树路径+节点视口)。pebrel 侧:`nebula_split/src/lib.rs` 的 `SplitTree::layout`、`collect_rects`。
3. **Divider 视觉 2px + 命中扩边 8px,照抄。** 视觉线细、抓取目标刻意加宽,两常量成文,命中测试按 scale 换算扩边。pebrel 侧:`nebula_split/src/lib.rs` 的 `DIVIDER_GAP`、`HIT_SLOP`、`hit_divider`。
4. **拖拽比例曲线三段式,照抄。** `drag_ratio` 指针→原始比例**不钳制**(预览逐像素跟手,越界值=进入拖拽关闭领域);`preview_ratio` 把 6% 边距关闭区钉死在 0.02/0.98 让 pane 可见地塌下去、常规带钳 10%-90%;`drag_close_target` 裁定被挤侧;`commit_ratio` 松手时吸附整数单元格再硬钳——PTY 尺寸只跟随提交值,预览不动网格。pebrel 侧:`nebula_split/src/lib.rs` 的 `CLOSE_MARGIN`、`RATIO_CLAMP`、`drag_ratio`、`preview_ratio`、`drag_close_target`、`commit_ratio`。
5. **SplitNav 最近邻导航(垂直漂移 4 倍惩罚),照抄。** 方向过滤 + 非对齐轴 4 倍计距,尽量停留在同一行/列。pebrel 侧:`nebula_split/src/lib.rs` 的 `SplitNav`、`nav_target`。
6. **RemoveOutcome 三态塌缩,照抄。** `NotFound`/`WasRoot`(关整个 tab)/`Collapsed`(兄弟收编空间、焦点交幸存子树首叶)——关 pane 的全部结局由树函数裁定,壳不自行散判。pebrel 侧:`nebula_split/src/lib.rs` 的 `RemoveOutcome`、`SplitTree::remove_leaf`。
7. **树编辑原子操作集,照抄。** `split_leaf`/`replace_leaf`(保留树形方向比例)/`set_leaf_parent_ratio`(只改最近父节点,target 在第二侧写 1-ratio,防嵌套 resize 错改祖先)/`node_mut`(路径寻址)。pebrel 侧:`nebula_split/src/lib.rs` 的 `SplitTree::split_leaf`、`replace_leaf`、`set_leaf_parent_ratio`、`node_mut`、`contains`、`first_leaf`。
8. **dock 整树嫁接,照抄(纯函数部分)。** `joined`/`dock_at_leaf` 把一棵现成子树 graft 到目标叶指定侧、目标在手势途中消失则原样退回——「整组分屏拖进另一个分屏」的几何基础。pebrel 侧:`nebula_split/src/dock.rs` 的 `SplitTree::joined`、`dock_at_leaf`。

### GPUI 壳工作区形态(参考为主,壳层代码不逐字搬)

9. **WorkspaceTab 三件套 + 核心不变式,照抄语义。** tab 持有 panes(实体属主,无序按 id 查)+tree(叶=pane id)+focused,不变式「panes 的 id 集合==树的叶集合」贯穿 split/close/restore 全部路径;pane id 全 workspace 唯一、终生不复用。pebrel 侧:`nebula_app/src/gpui_shell/workspace.rs` 的 `WorkspaceTab::Terminal`、`TerminalPane`、模块头「分屏 pane 生命周期合同」。
10. **pane 矩形由布局树裁定、prepaint 回写,照抄语义。** 渲染递归按提交比例铺 GPUI flex 树;canvas 探针在 paint 阶段把 Split 视口/pane 矩形回写 `Rc<RefCell>` store,方向导航与拖拽换算读上一帧记录;终端实际网格尺寸由终端 element 的 prepaint 观测矩形→网格→PTY 下发。pebrel 侧:workspace.rs 的 `SplitBoundsStore`、`PaneBoundsStore`、`render_split_node`、`to_split_rect`。
11. **拖拽 overlay 不动 pane 树,照抄语义。** 交互拖拽只更新轻量 overlay(分隔线 + 关闭区高亮),pane flex 树保持提交比例——高频 pointer 事件不重建不重排终端网格;刷新按 8ms 节流、关闭目标变化即时刷新;松手才 commit_ratio/关被挤 pane。pebrel 侧:workspace.rs 的 `SplitDrag`、`split_drag_visual_geometry`、`update_split_drag`、`split_drag_raw_ratio`、`finish_split_drag`。
12. **resize 两段式(burst+settle 与结构性立即下发),照抄语义。** 连续几何变化(窗口拖边)尾沿去抖一次提交,去抖窗内网格先 reflow、PTY 等 settle;一次性结构变化(分屏创建/关闭、zoom 切换)走 `mark_structural_resize` 立即同步 grid+PTY——消除「子进程仍按旧宽度输出、提交时本地 reflow 与 conhost rewrap 各折一套行数」的窗口期;启动稳定闸(grid_synced+宽限期)防开窗 resize 异步落地前首帧来回重排。pebrel 侧:workspace.rs 的 `mark_structural_resize`;`nebula_app/src/gpui_shell/terminal/view.rs` 的 `RESIZE_SETTLE_DELAY`、`structural_resize`、`grid_synced` 及模块头合同注释(PTY 尺寸链细节归分片 02,本片登记交界)。
13. **非聚焦 pane 压暗 veil,参考。** 不用焦点描边,非活动 pane 终端区覆半透明黑纱(标题条不压,防四个标题一起糊灰),主题配置可关。pebrel 侧:workspace.rs `render_split_node` 的 veil 分支、`NEBULA_UNFOCUSED_SPLIT_DIM`。
14. **pane 满卡 zoom,参考。** 聚焦 pane 临时满卡,任何结构性操作(split/close/导航/点击别的 pane)先解除;几何从半卡跳整卡按结构性 resize 立即下发。pebrel 侧:workspace.rs 的 `toggle_zoom`。
15. **tab 类型封闭枚举形态,参考。** `WorkspaceTab` enum = 唯一合法 tab 类型集(Terminal/Settings/Image/Document/Code),新增类型=加变体+渲染分支+关闭语义。slTerminal 旧面板映射方向:terminal → Terminal tab 的 pane;editor/htmlviewer/markdownviewer/gitshow/diff → Document/Code 类只读 tab 形态(细节归分片 07);settings → 单例 Settings tab(归分片 06)。pebrel 侧:workspace.rs 的 `WorkspaceTab`、`render_terminal_tab`。
16. **tab 拖拽停靠交互,参考。** tab 级拖拽(DockTarget 四分位)挂 `dock_at_leaf` 实现整树/单叶停靠,落点消失优雅退回;slTerminal Dockview 原生拖拽行为的替代形态。pebrel 侧:workspace.rs 的 `dock_tree`、`tab_drag` 模块;`nebula_split/src/dock.rs`。

### 布局持久化(照抄 schema,重定位语义)

17. **session v4 schema 整体结构,照抄。** `version` 字段 + 老版本内存就地升级(首存回写当前格式);`TabSession`(cwd/custom_name/color/launch/layout/active_pane);`Session`(active_tab/tabs/window/boot_attempts/clean_exit)。pebrel 侧:`nebula_app/src/session.rs` 的 `Session`、`TabSession`、`VERSION`、`parse`。
18. **LayoutSession 持久化树(permille 整数比例),照抄。** `Split{axis, ratio_permille}` + `Pane{cwd, custom_name, ...}`;比例落整数千分比,自动保存变化检测与文件 diff 不被 f32 序列化噪声绊倒;追加字段走 serde default 免升版。pebrel 侧:session.rs 的 `LayoutSession`、`SplitAxis`。
19. **运行时树↔持久化树纯转换单点,照抄形态。** `layout_from_tree`/`tree_from_layout` 双向往返:比例 f32↔permille、叶数据经闭包注入/DFS 序配对——「布局单点」硬约束在 GPUI 世界的落点:全仓只有这一对函数摸持久化树形状。pebrel 侧:`nebula_app/src/gpui_shell/session_restore.rs` 的 `layout_from_tree`、`tree_from_layout`。
20. **1Hz 连续快照无变化跳过,照抄。** persistence 持 latest/saved 两份快照,相等不写盘;tick 内联逐窗 snapshot→落盘,失败只 warn 不拖垮终端;设置/文档 tab 不进会话。pebrel 侧:`nebula_app/src/gpui_shell/workspace/session_persistence.rs` 的 `SessionPersistence::save_with`;`nebula_app/src/gpui_shell/workspace/windowing.rs` 的 `autosave_tick`;`nebula_app/src/gpui_shell/workspace/session_recovery.rs` 的 `snapshot_session`。
21. **boot_attempts 崩溃断路 + quarantine,照抄。** 恢复尝试前先计数落盘,活到第一次自动保存即归零(首次成功快照带 boot_attempts=0);连续 3 次未活过恢复 → 现场**挪走**到 crashed 副本再干净启动——挪而非删:启动成功一秒后自动保存会盖掉「一恢复就崩」的唯一诊断材料。pebrel 侧:session.rs 的 `MAX_BOOT_ATTEMPTS`、`should_restore`、`mark_boot_attempt`、`quarantine`;session_recovery.rs 的 `try_restore_session`。
22. **clean_exit 收尾标记与 was_crash 判定,照抄。** 1Hz 快照恒写 false,只有正常收尾路径写 true,读到 false=上次崩溃/强杀/断电;空会话(一路关标签关干净)不算崩溃;老文件缺字段按 false 解析,恢复行为不变、只多一条提示。pebrel 侧:session.rs 的 `Session::clean_exit`、`save_final`、`was_crash`。
23. **原子写落盘,照抄。** 每秒一写的文件崩在半写不能赔掉它存在的意义;快照紧凑写,workspace 导出 pretty 打印(用户可读可 diff 的版本化产物)。pebrel 侧:`nebula_app/src/atomic_file.rs`;session.rs 的 `try_save`、`save_to`。
24. **快照文件即 workspace 导出格式,参考。** session.json=未命名的 workspace,导出/导入同一 schema;定位重界定后它回答「下次启动恢复布局」,不承诺关窗后会话存活。pebrel 侧:session.rs 模块头合同;workspace.rs 的 `export_workspace`。
25. **恢复注入顺序(先逐 pane spawn、后建树),照抄语义。** `LayoutSession::leaves` 的 DFS 序与运行时 pane 创建序一一配对,`tree_from_layout` 分配 id 与 leaves 索引对齐注入 cwd/启动身份;`active_pane` 越界回退首叶;恢复期保持文件次序、不套新 tab 插入策略。pebrel 侧:session_recovery.rs 的 `restore_tab`。
26. **AgentSession 随叶持久化,参考(归分片 03)。** 快照记录 pane 前台 AI CLI 对话身份,冷恢复据此续接;只存安全启动描述(来源名+id,不存正文/启动参数原文),resume 命令构造对异形 id/注入全拒。本片只登记持久化挂载点与「叶子=恢复注入单位」契约。pebrel 侧:session.rs 的 `AgentSession`、`valid_native_session_file`、`AgentSession::resume_command`;session_recovery.rs 的 `restore_agent` 调用点。

### 硬约束的 GPUI 重建形态

27. **「面板封闭」(#5)→ Rust 侧封闭类型集,参考形态。** 封闭集从「panelRegistry 白名单 + 组件映射 + PANEL_TYPES + FILE_PANEL_TYPES」迁移为壳层枚举/注册表单一形态;新增类型 = 变体 + 渲染 + 关闭/持久化语义同步;fromJSON 白名单校验、withPanelBoundary、isAlwaysRenderPanel、panelIdInPage 页前缀协议等 Dockview 伴生机制随之消亡。pebrel 侧:workspace.rs 的 `WorkspaceTab`。
28. **「布局单点」(#7)→ 单一 schema + 单一转换对,照抄形态。** 持久化布局只经 session schema 存取,运行时分屏树与持久化树之间只有 session_restore 一对纯函数;页切片(slicePageLayout)/汇编(composeHostLayout)/运行期并入(loadPageGroup)/迁移补丁等宿主汇编机制无存在前提。pebrel 侧:session_restore.rs 全文。

## 不采纳点

1. **Dockview 体系整体,不采纳(已定)。** 共享宿主(WorkspaceDockHost)、页组派生归属协议(pageGroups/panelIdInPage/auditGroupMembership)、页切片与汇编(slicePageLayout/composeHostLayout/loadPageGroup/syncHostLayoutToStore)、旧格式迁移与修补(patchLegacyShape/normalizePageLayout)、叶级 setVisible 可见性、renderer="always" 恒挂载白名单、`__dockviewApi` 宿主单例、restoreGuard——全部随 WebView 前端消亡;xterm 不可二次 `open()` 的约束在 Rust 原生渲染下不复存在。slTerminal 侧:`src/workspace/layoutSerde.ts`、`pageGroups.ts`、`WorkspaceDockHost.tsx`、`persistPanelParams.ts`、`src/panelRegistry.ts`。
2. **多窗口会话段,不采纳。** `WindowLayout`/`combine_sessions`/`save_combined_session` 的多窗合并与 per-window snapshot 组合——单窗口单实例下 Session 扁平化为唯一窗口,window 字段只保留单窗尺寸/最大化态。pebrel 侧:session.rs 的 `WindowLayout`、`combine_sessions`;windowing.rs 的 `combined_session`。
3. **LaunchSession::Ssh,不采纳(已定)。** SSH 目的地保存/恢复自动重随 SSH 整支砍掉;启动身份枚举只留本地形态。pebrel 侧:session.rs 的 `LaunchSession::Ssh`。
4. **LaunchSession::Shell(WSL 发行版),不采纳。** pebrel 的 Shell variant 承载 WSL distro 检测启动;slTerminal 默认 pwsh→powershell→cmd 回退链,跨发行版概念不进启动身份。Profile variant 的「完整命令内嵌保可移植」形态保留(本地 profile)。pebrel 侧:session.rs 的 `LaunchSession::Shell`;`nebula_app/src/gpui_shell/workspace/shell_launch.rs`。
5. **mux 驻留配合的恢复语义,不采纳(已定)。** detached tab 恢复、关窗会话存活、按窗口角色过滤快照(WindowRole::Regular)等随 mux 砍掉;快照断路器/quarantine 语义保留(采纳点 21)。pebrel 侧:windowing.rs 的 WindowRole 过滤;`nebula_app/src/gpui_shell/workspace/residency.rs`。
6. **broadcast 广播输入,不采纳。** 聚焦 pane 击键同步到本 tab 其余 pane 的内存态功能,产品面无对应需求;其「绝不写进 session、重启不带回隐形模式」的决策原则可借鉴。pebrel 侧:workspace.rs `WorkspaceTab::Terminal` 的 broadcast 字段。
7. **tabs 位置多形态(顶栏/侧栏可配 + 折叠),不采纳配置化。** pebrel 的 TabsPositionName/settings_should_fold_sidebar 双形态是历史包袱;slTerminal 取左侧垂直侧边栏单一形态。pebrel 侧:workspace.rs 的 `settings_should_fold_sidebar`、`top_tabs` 模块。
8. **侧栏拖宽的 per-theme 分界补偿细节,不采纳数值。** 主题画线/不画线两种卡缝下热区偏移的补偿逻辑(曾因默认主题切换整体偏移)不照抄——壳层卡缝模型重建后重定,交互形态(热区+拖拽+overlay)参考。pebrel 侧:workspace.rs 的 `sidebar_resize_offset_for`、`sidebar_resize_visual_offset`、`sidebar_resize` 模块。
9. **Quick terminal,不采纳(已定)。** `nebula_app/src/gpui_shell/workspace/quick_terminal.rs` 整支。
10. **侧栏内容面板整组,不采纳(归对应分片)。** file_tree/vcs_panel/details_panel/ssh_hosts/remote_files/agents/recipes/palette 等 pebrel 侧栏面板不属于本片;本片只定「侧栏 = tab 列表 + 主内容区」骨架与 tab 类型封闭形态。pebrel 侧:gpui_shell/workspace/ 下对应模块。
11. **文档/图片 tab 的 pebrel 具体实现,不采纳实现(归分片 07)。** ImageTabView/DocTabView/CodeTabView 的具体渲染不进本片裁定;只采纳「tab 类型封闭枚举 + 只读文档 tab 形态」的方向。pebrel 侧:`nebula_app/src/gpui_shell/doc_tabs.rs`、`code_tab.rs`、`file_preview.rs`。
12. **WindowState 回放出形态,不采纳。** v4 起窗口尺寸/最大化态只写不回放(启动按配置列行数定形),字段作诊断与前向兼容数据保留进 schema;「记住上次窗口大小」是否恢复另行裁决。pebrel 侧:session.rs 的 `WindowState` 字段注释。
13. **tab 展示态元数据整面,不采纳。** 快照的 custom_name/color 照抄;壳侧 TabMeta 的运行时展示态(shell_tag 徽标、bell 标记等)不整体搬迁,按 slTerminal 状态徽章既有链重接。pebrel 侧:workspace/tab_presentation.rs 的 `TabMeta`。
14. **nebula_* 命名与 nebula 数据目录,不采纳(已定)。** 全部 slterm_* / slterm 数据目录;session.json 文件名形态保留。

## 优化方向

工作区在 GPUI 单进程壳内重建为单一 Rust 实体:持有 tab 列表 + 活跃 tab + 侧栏状态,左侧垂直 tab 侧边栏 + 主内容区的骨架形态照 pebrel;分屏语义全部由 slterm_split 纯函数 crate 裁定(照抄 nebula_split 改名),壳层只做四件事——按提交比例渲染递归、canvas 回写节点/pane 矩形、拖拽 overlay、松手提交。

「pane 属主集合 == 树叶集合」作为壳层核心不变式:split/close/restore 全路径由树函数返回值驱动(RemoveOutcome 三态决定关 tab 还是收编移焦),失败路径(建树挂不上立即回收 pane 不留给孤儿 PTY)随合同照抄;pane id 全 workspace 唯一、终生不复用。

布局持久化单点重建:session schema(v4 形态,slterm 命名)为唯一布局存储面,运行时树↔持久化树转换全仓唯一函数对;1Hz 快照 + 无变化跳过 + 原子写 + boot_attempts 断路 + quarantine 整体保留。语义重界定——它回答「下次启动长什么样」,不回答「关窗后谁还活着」;恢复粒度 = tab 列表 + 每 tab 分屏树 + 逐 pane 启动身份 + 聚焦叶,崩溃/强杀/断电与正常退出统一回到一秒内的现场,无「恢复?」对话框。

面板封闭重建:tab/pane 合法类型收敛为 Rust 侧单一封闭集,新增类型走变体 + 渲染 + 关闭/持久化语义同步;slTerminal 旧面板映射——终端 → 分屏树 pane 叶,编辑器/预览/diff/git 类 → 只读文档 tab(分片 07),设置 → 单例 tab(分片 06);docViewer 的恒挂载保活、跨页守卫、页前缀协议、标题重算链等 Dockview 伴生问题全部无存在前提。stores 布局相关状态(useLayout 的 activePageId、projects store 的页布局字段)随前端消亡,「页面=宿主内页组」的派生归属模型不迁移。

TTY 尺寸链改为「布局树矩形 → prepaint 观测 → 网格 → ConPTY」,保留 burst+settle 尾沿去抖与结构性立即下发的两段式合同;分屏拖拽期间 PTY 只跟随提交比例,预览纯 overlay,松手一次落定。

测试方向:slterm_split 纯函数 crate 全部 Rust 单测随照抄迁移(切割数学/比例曲线/塌缩/dock 嫁接/导航);壳层不变式、恢复注入顺序、断路器/quarantine 走集成测试;布局持久化 round-trip 按 schema 单点收敛。测试体系总体归分片 11。
