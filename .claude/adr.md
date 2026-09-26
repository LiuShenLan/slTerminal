# ADR

架构决策记录。各条正文一律为**当前有效形态**——被后续决策推翻的段落已原位收敛，修订要点见各条末尾「沿革」行；完整编年史从 git log 查。

## 索引

| ADR | 标题 | 状态 |
|-----|------|------|
| 0001 | 活动栏 + 共享侧栏区（侧栏视图单槽位状态机） | accepted |
| 0002 | 配色方案单点（schemes/ + 注册表 + facade + 重载切换） | accepted |
| 0003 | UI 全面重设计（Linear 极黑克制） | accepted |
| 0004 | Windows build 号钳制至 xterm「新 ConPTY」分支下界 | accepted |
| 0005 | Win10 嵌入捆绑新版 ConPTY 宿主 | accepted |
| 0006 | 依赖版本策略（生产精确 / 开发 ^） | accepted |
| 0007 | xterm 三件套 beta 保留 + 升级审批约定 | accepted |
| 0008 | notify RC 保持 | accepted |
| 0009 | review-fix 豁免与决策汇总登记 | accepted |
| 0010 | review-phase2-fix 决策与债务登记 | accepted |
| 0011 | 文档代码自证原则 | accepted |
| 0012 | 设置中心（统一配置入口 + 配置页注册表 + 后端轻量收口） | accepted |
| 0013 | 后台定时任务双端抽象 | accepted |
| 0014 | CLI 别名持久化走通用 save_settings 段透传 | accepted |
| 0015 | CLI 别名不进 profile.commands，注册表持独立别名快照 | accepted |
| 0016 | E2E 假 home 隔离 | accepted |
| 0017 | 预览容器信任模型：md/html 同态渲染 | accepted |
| 0018 | 本地资源通道：项目根只读 + data: 内联 | partially superseded by 0019（主窗口 CSP data: 放行回收） |
| 0019 | 预览安全域模型（自定义协议宿主页 + sandbox iframe） | partially superseded by 0021（预览载体） |
| 0020 | 页内分屏 | accepted |
| 0021 | 预览载体回迁主窗内跨源沙箱 iframe | accepted |
| 0022 | DA1 后端全量接管 + 恢复注入就绪闸门 | accepted |
| 0023 | 设置中心组模型改全局/Agent 二分 | accepted |
| 0024 | agent 目录沙箱白名单 + pinned watcher | accepted |

## 0001 活动栏 + 共享侧栏区（侧栏视图单槽位状态机）

**Status**: accepted（2026-07-18）

**上下文**：原布局为 Allotment 常驻三栏（项目列表 250px + 文件浏览器 250px + 主区），两栏均不可关闭，挤压主区宽度。需求是两者可关闭、可分区展示，且未来会新增更多同类视图。

**决策**：重构为「活动栏 + 共享侧栏区」——最左新增常驻窄条**活动栏**（不可关闭），各视图变为**侧栏视图**，经活动栏按钮开关；视图共享同一**侧栏区**（Allotment 拖宽 160–500 保留），侧栏区垂直分**上区/下区**（Allotment 垂直嵌套，分隔条比例可调），按钮经拖拽归属半区。核心状态机：**每半区最多一个打开的视图（单槽位），无"打开未展示"中间态，无历史记忆**——展示是 `f(按钮归属, 各区打开的视图)` 的纯推导。

**关键规则**：

| 操作 | 行为 |
|------|------|
| 点击未打开按钮 | 所在半区打开该视图；同区已有打开的 → 隐式关闭它 |
| 点击已打开按钮 | 关闭该视图；两半区皆空 → 侧栏区整体隐藏，主区占满 |
| 拖拽按钮换区（视图打开中） | 视图立即跟随到新区展示；目标区已有打开的 → 关闭那个 |
| 拖拽按钮换区（视图未打开） | 仅改归属，展示不变 |
| 单视图打开 | 全高展示（不分半区） |
| 双视图分属两区 | 上下分开展示 |

- 拖拽支持半区内排序 + 跨区按落点插入；新注册视图默认追加上区末尾。
- 关闭 = `display:none` 隐藏不卸载（视图内部状态如展开目录保留，与操作页面多实例显隐同模式）。
- 无快捷键（用户明确不要）。
- 按钮样式：VS Code 风格 active 指示（高亮 + 左侧指示条），配色走 `theme/colors.ts` token（硬约束 #6）。
- 持久化（`~/.slterminal/settings.json` `sideBar` 段，复用 settings 浅合并，零后端改动）：按钮归属+区内顺序、侧栏区宽度、上下分割比例、各区打开的视图；重启恢复现场。

**被否决的备选**：

- **"打开未展示"态 + 历史记忆**（关闭当前视图后自动回到前一个打开的视图）：用户明确要求"仅考虑当前状态，不要历史记忆"；且多一层状态，与全部给定示例不符（规则 3：同区两点后关闭 → 整体隐藏而非回到前者）。
- **拖拽不跟随**（视图展示位置可与按钮归属临时分离，下次点击才生效）：两个事实脱节，需额外记录"视图实际所在半区"，实现与心智成本都高，易出 bug。
- **关闭即卸载组件**：文件浏览器展开目录、滚动位置等状态丢失；隐藏不卸载与 H6 多实例显隐模式一致，代价仅少量内存。

**后果**：

- 新增 `SideViewRegistry` 模块级单例（同项目现有 panelRegistry / TabTitleRegistry / FileViewerRegistry / ShortcutRegistry 模式）：新增侧栏视图 = 实现组件 + `register({ id, title, icon, component })`，活动栏按钮、侧栏区展示、持久化全部自动生效，无需改动框架代码。
- 状态最少（按钮归属 + 每区打开的视图 id），所有展示由状态纯推导，无 UI 临时态。
- 原 `SidebarTree`、`ExplorerPanel` 组件本体不变，仅宿主从 Allotment 常驻栏变为侧栏区视图槽。
- **换区重建（已确认接受）**：拖拽按钮跨区（上↔下）时，视图组件从上区 pane 移入下区 pane——React 视为不同父节点，触发卸载+重建，组件内部状态（如 explorer 展开状态、rootNodes）丢失。权衡：换区为低频操作（用户通常设定一次后不改），重建成本低于跨父节点保持实例的架构复杂度。

**沿革**：初始视图清单（项目列表/文件浏览器两槽）与 Emoji 图标样式已随注册表扩展与 ADR-0003 重设计演化（现为导航树/文件/Commit/Agent 全局文件四槽 + 线性 SVG 图标），以代码现状为准。

## 0002 配色方案单点（schemes/ + 注册表 + facade + 重载切换）

**Status**: accepted（2026-08-07）

**上下文**：颜色散落在 6+ 条独立色源（colors.ts 32 token、xterm 22 色、oneDark、dockview --dv-*、Allotment、App.css --sl-fg-*、库默认色），改色多点人工同步且有漏网点（数值重复不联动、死 token、1 处真违规硬编码）。需求：全部应用可控色源收拢单点 + 每色注释消费位置 + 为新增方案与一键切换留扩展接口。

**决策**：
- src/theme/ 下设 schemes/（ColorScheme 四段：ui/terminal/editor/libraries + darcula 内置方案）+ SchemeRegistry 模块级单例（项目注册表惯例第 6 例）。
- colors.ts 改 facade：31 个同名导出值代理 active 方案，369 处消费点零改动。
- 方案切换 = settings.json `colorScheme` 段手编 + 重载窗口生效；main.tsx 启动时 React 挂载前解析（App 改动态 import 保证 facade 求值晚于 setActive）。
- 三方库色经 overrides 通道：dockview --dv-* 与 Allotment 变量内联注入挂载点；oneDark 作 editor.theme 引用 + lint/search/背景 token 化覆盖。
- 零视觉变化：所有覆盖值取现行有效值；死配置全清（3 死 token + 2 零消费 CSS 变量 + 1 违规收敛）。
- 启动链 fail-safe 色（index.html/tauri.conf.json/main.tsx 三处静态层）经**构建期通道**同步：`scripts/sync-startup-colors.mjs` 从方案文件提取改写三处消费点，运行期仍不经 facade（CP-027）。

**被否决的备选**：
- 运行期即时切换（壳层 token 全面响应式）：369 处常量消费 + xterm 创建期消费需全量改造，代价 vs 暗色系低频切换收益不成比例。编辑器侧障碍后经 CP-039 消除（见后果），壳层全面响应式仍否决。
- oneDark 完全 token 化自绘语法色板：每方案需 10+ 语法色定义，工作量与审美风险大；editor 段引用已预留未来自定义。
- 启动链 fail-safe 运行期通道收编：index.html/tauri.conf.json 为静态层无法用 TS token（构建期通道成立后本条仅约束运行期）。

**后果**：
- 新增方案 = schemes/ 新文件 + register 一行，消费方/测试守卫零改动。
- 注释单点在 types.ts 接口槽位（消费位置与方案无关），新方案零注释负担。
- main.tsx 静态 import 图收敛为 react/react-dom/lib/e2eEnabled/theme/startupColors（零依赖常量模块，不触发 facade 求值）；E2E helpers 与 ROOT_CSS_VARS 注入保持原相对顺序。
- **编辑器主题热重配置（CP-039）**：`editorTheme` 模块级常量改 `getEditorTheme()` 函数形，消费点改 `editorThemeSlot`（Compartment + `schemeRegistry.onDidChange` 订阅热重配置）——CM 主题随方案切换即时生效、编辑器不重建（文档/光标/undo 保留）。

**沿革**：2026-09 CP-027 启动链 fail-safe 构建期同步通道成立；CP-039 editorTheme 常量化消除。

## 0003 UI 全面重设计（Linear 极黑克制）

**Status**: accepted（2026-08-16）

**上下文**：现状 UI（darcula 时代）被评价为「古板呆滞」——中灰蓝底平铺、实色粗边框、盒式页签、装饰 emoji、侧栏多区块堆叠。用户调研 11 款软件暗黑 UI 后要求整套重设计：仅暗黑、JetBrains Mono 全局、无动效、不做多主题切换。只交付设计产物（设计方案 + 需求规格 + 视觉稿），不含实现。

**决策**：经 13 题澄清（风格锚点/底色温度/强调色/标题栏/图标/页签/密度/IA/内容色/交付流程/规格粒度）+ 6 个候选视觉稿（3 骨架内 + 3 突破型）浏览器实测对比，选定**候选 A「Linear 极黑克制」**，无微调：

- 风格锚点 Linear/Zed 近纯黑现代风；暖黑**明度阶梯** 6 档（`#0a0a0b`→`#2b2b31`），内容区最暗；分隔一律**发丝线**（半透明白 0.055/0.09）。
- 单强调色现代蓝 `#6e9ff2`（<5% 像素占比）；语义色低饱和暖协调；终端 ANSI 16 色与编辑器语法色全量重调（**双轨配色**：壳层 token 与内容色板互不混用）。
- 自绘 34px 一体化标题栏（原生标题栏退役）；扁平页签 + 底部 2px 指示条 + hover 才显关闭钮。
- 侧栏 IA 重构为**统一导航树**（项目 → 页面 → 会话，历史会话折叠计数）；文件浏览器独立为活动栏视图；活动栏固定槽位（现为四槽：导航树/文件/Commit/Agent 全局文件，第四槽随 ADR-0024 追加）。
- 装饰图标全部单色线性 SVG（15px/1.5px 描边/currentColor）；状态 emoji → **状态圆点**（绿/黄/灰，语义来源 F3 不变）；CLI 品牌 logo 保留彩色。
- 交付物：`design.md`（设计方案）、`requirements.md`（UI-xxx 编号需求 + 可测验收 + P0/P1 + 对比度自检附录）、`final-mockup.html`（主界面 + 组件集双页静态稿）。

**与 theme 系统对接**（后续实现期）：全部色值经 ADR-0002 配色方案单点落位——新增 scheme 文件替换 darcula 内置方案，需求规格每条色值标注 types.ts 槽位；启动链 fail-safe 静态色经 sync 脚本从 linear.ts 构建期注入（CP-027）；多主题切换机制不建（硬约束）。

**被否决的备选**：
- 候选 B（Zed 实体边框感）/ C（最暖+大圆角）：骨架内变体，层级靠边框/温度而非纯明度差，不如 A 的「界面消失」感。
- 候选 D（紫罗兰强调色+冷黑底）/ E（One Dark 盒式页签）/ F（teal+胶囊页签）：突破型对照组，验证已定决策（现代蓝/暖黑/扁平页签）成立，无需突破。
- 多 UI 模式/亮色系：用户硬约束，明确排除。

**后果**：
- 实现期已按 requirements.md 逐条验收（P0）完成；交付物四件（design.md/requirements.md/final-mockup.html/checklist.md）已随 docs/ 清理退役，原文从 git log 查。
- F3 四态 emoji 的**视觉呈现**被状态圆点取代（事件→状态映射逻辑不变）；F9 品牌 logo 保留。
- 现状侧栏（项目列表 + Agent Status 等区块）IA 将随实现重构为统一导航树；行为逻辑（快捷键/右键菜单/会话恢复）不变。

**实现期追加决策（2026-08）**：Stage 01-08 实施（checklist.md 46 项）期间确认，逐条落地于代码：

- **UI-405/406/407 三条剔除**（Agent 面板/composer/状态行）：为远期新功能而非视觉重设计，不属本期范围；2026-08-16 定调——聊天式 Agent 面板为独立产品方向，未来单独立项。requirements.md 三条已补「远期愿景，本期不实施」注记（DOC-04），编号保留。
- **darcula 删除、linear 替换**：新方案 `linear` 替换并删除 darcula（schemes/ 只余 linear.ts）；启动链默认 id 改 `linear`，未知 id 回退机制内建（schemeRegistry.setActive）。
- **配置钮入口唯一化**：活动栏底部「配置」钮 = 设置中心唯一入口（后由 ADR-0012 承载）；SidebarTree 右键菜单「打开 Hooks 配置」项随 SidebarTree 退役删除。
- **导航树挂法**：活跃会话挂页面下（panelId→pageId 归属）；历史会话折叠节点挂项目下（cwd 归属——规范化前缀匹配，孤儿目录不展示）。
- **两新依赖**：`lucide-react`（装饰图标全部线性 SVG 单点封装 src/lib/icons.tsx）、`@fontsource/jetbrains-mono`（400/500 woff2 随产物打包，断网可用）。
- **自绘标题栏取舍**：`decorations:false` + 自绘 34px 一体化标题栏，**接受失去原生标题栏/阴影与拖拽悬停时的 Snap Layouts 预览**（Win+方向键 Aero Snap 是 OS 窗口管理功能，与 decorations:false 无关，仍可用——2026-08 实机验证修订）；关闭钮经 `getCurrentWindow().close()` 复用 P1-19 关窗杀 PTY 链路。

## 0004 Windows build 号钳制至 xterm「新 ConPTY」分支下界（21376）

**Status**: accepted（2026-08-17）

**上下文**：Win11 正常、Win10 上 claude code 全屏模式出现四症状（字符错位/输入字符不显示/滚轮全屏滚动+重复表头/流式重复行缺行）。根因：xterm.js（6.1.0-beta.288）以 `windowsPty.buildNumber < 21376` 为阈值分叉 ConPTY 兼容行为（`@xterm/xterm/src/common/CoreTerminal.ts:283`）——旧分支启用 wrapping 启发式（每次 LF + CSI H 强制重算 `isWrapped`，`src/common/WindowsMode.ts`），claude 全屏 TUI 高频重绘时行末常为非空格字符（边框/进度条）→ 误标 wrapped → buffer 行结构错乱。Win10 19045 落旧分支，Win11 26100 落新分支，与症状分布吻合。同阈值还控制 resize reflow 开关（`Buffer.ts:318`，<21376 关闭）。

**决策**：前端 `useXterm` 写入 `term.options.windowsPty` 时把真实 build 号钳制至下界（`clampWindowsBuildForXterm = Math.max(build, 21376)`）——Win10 走「新 ConPTY」分支，行为与 Win11 对齐。真实 build 号获取链（`TerminalPanel` → `pty.getWindowsBuildNumber()`）与后端均不动；spawn 请求本就不带 buildNumber。

**被否决的备选**：

- **不设 windowsPty**：`Buffer.ts:199` resize 行补充策略落入非 Windows 分支（ybase 滚动），与 ConPTY「自己重印屏幕」语义打架，且 Win11 现有正常路径同样被改——回归风险最高。
- **升级 xterm.js**：未证实上游已移除该启发式（网络受限无法核对），且项目升到 6.1.0-beta.288 有特定动机（调查5修复），大版本跳变回归面最大。
- **pnpm patch 改阈值**：可只关启发式、保持 Win10 不 reflow（最保守），但引入项目首个 patch 维护负担（升级时失效风险）。

**后果**：

- Win10 上启发式关闭（目标达成）+ resize reflow 连带启用——唯一新增风险：Win10 19045 conhost 若未 backport wrap 标记修复，resize 时 reflow 可能短暂错乱（claude 全屏重绘自愈）。已列 Win10 实机验证专项；异常则降级 pnpm patch 方案。
- **xterm.js 升级时须重评估**：钳制语义依赖上游阈值 21376 与启发式实现，升级后核对 `CoreTerminal.ts` 分叉逻辑是否仍成立。
- 行为固化点：`src/panels/terminal/useXterm.ts` 的 `XTERM_CONPTY_MIN_BUILD` / `clampWindowsBuildForXterm`（L2 `win-build-clamp.test.ts` 守卫）。

## 0005 Win10 嵌入捆绑新版 ConPTY 宿主（conpty.dll + OpenConsole.exe）

**Status**: accepted（2026-08-17）

**上下文**：ADR-0004 修复四症状后，Win10 实测暴露滚轮问题：claude code 全屏模式鼠标滚轮无法滚动（键盘 PageUp/Down 正常）。排查链：① 0x7（初始）与 0x3 分叉（去 WIN32_INPUT_MODE）两条输入路径均实测失败——推翻 flags 假设；② 键盘正常说明通路 OK；③ WT 在 Win10 滚轮正常因自带新版 conhost 代码、不经系统 conhost。根因坐实：老 Win10 in-box conhost 的 ConPTY **不转发鼠标 VT 序列**（microsoft/terminal#376「吞掉 mouse reporting escape sequences」，修复 PR #4856 只在新版 conhost）。外部先例：WispTerm、WezTerm 均以捆绑新版 conpty.dll + OpenConsole.exe 修复。此前旁置 vendor 文件的捆绑尝试因部署冲突失败（用户只部署 exe + slterminal_lib.dll，未拷贝 vendor 文件 → 静默回退系统 conhost，实验无效）。

**决策**：vendor 两文件（官方 NuGet `Microsoft.Windows.Console.ConPTY` 1.24.260710001，MIT）经 `include_bytes!` **嵌入 slterminal_lib.dll 资源**，仅 Win10（build < 21376）在首次 pty_spawn 前提取到 `%LOCALAPPDATA%\slterminal\conpty\` 并 `LoadLibraryW` 动态加载（`OnceLock` 进程级单次解析）。部署形态零变化（仍 exe + dll 两文件）；提取幂等（大小一致复用，vendor 升级自愈）；提取/加载任一失败 → `tracing::warn!` + 静默回退系统 ConPTY（行为 = 现状）；Win11 恒系统路径零变化。flags 改三态：捆绑/系统 Win11 → 0x7，系统 Win10 回退 → 0x3（键盘已实测正常，保留）。

**被否决的备选**：

- **0x3 分叉**（去 WIN32_INPUT_MODE，回归传统 VT 输入路径）：2026-08-17 实机验证失败——滚轮仍不可用，证实老 conhost 两条输入路径均不转发鼠标，与 flags 无关。
- **旁置 vendor 文件 + package.ps1 打包**：与用户「只部署 exe + slterminal_lib.dll」工作流冲突，验收必漏文件（已实测踩坑）；仅嵌入方案能保住部署形态。
- **更新系统 conhost**：不可要求用户改 OS。
- **前端模拟 PageUp/Down**：全屏 TUI 滚动语义复杂且不可靠，绕过根因。

**后果**：

- vendor 二进制入库（`src-tauri/vendor/conpty/`，含 LICENSE/README，更新流程见 README）；slterminal_lib.dll 体积 +~1.1MB。
- 杀软风险：OpenConsole.exe 为微软签名二进制，提取到用户目录风险低（先例：.NET 官方同法分发）。
- **自动化无法守卫真实鼠标转发**（先例同 0x8/0x3）——Win10 实机验证红线：真实 claude 滚轮 + 键盘/IME/kitty + resize + 删除提取目录回退验证。
- 行为固化点：`src-tauri/src/pty/conpty_api.rs`（L1 5 条测试守卫决策/路径/幂等）+ `compute_conpty_flags` 三态（L1 7 条）。

## 0006 依赖版本策略（生产精确 / 开发 ^）

**Status**: accepted（2026-08-18）

**上下文**：59 个 npm 依赖版本策略不一致（8 精确 + 51 `^`），生产运行时依赖浮动升级不可控——终端核心路径（xterm 等）的静默 minor/beta 浮动曾引入回归（调查5）。需求：统一版本声明策略，生产与开发工具分流。

**决策**：`package.json` 双区版本策略——

- **dependencies（生产运行时）全精确版本**（无 `^`，锁死当前解析版本）：浮动的任何升级都必须显式改 package.json，进入评审流程。
- **devDependencies（开发工具）全 `^`**：开发工具升级风险低、频次高，允许 minor 浮动。
- **overrides 段保持现状**（`^`），不随本策略调整。成因与『谁钉谁』对齐契约登记于 `e2e-tests/CLAUDE.md`（CP-032）；上游放开硬钉后逐条去 overrides 化。
- 精确版本一律以 package-lock.json 当前解析版本为准（pin 不改解析版本本身，`npm install` 刷新 lock）。

**后果**：

- 升级依赖 = 显式改 package.json 版本号 + 跑对应测试门禁，杜绝 `npm install` 静默升级。
- 例外登记：xterm 三件套 beta 保留（ADR-0007）与 notify RC 保持（ADR-0008）为 pre-release 场景既定例外，升级审批见各自条目；Cargo.toml Rust 侧沿用语义化版本区间（不属本策略范围）。

## 0007 xterm 三件套 beta 保留 + 升级审批约定

**Status**: accepted（2026-08-18）

**上下文**：终端核心路径依赖 xterm beta（`@xterm/xterm` 6.1.0-beta.288 + addon-webgl/fit）。升级到该版本有特定动机（调查5修复，ADR-0004 被否决备选记载）——回退稳定版会回归已修复问题；且 ADR-0004 后果明示「xterm.js 升级时须重评估」：`windowsPty` build 钳制语义依赖上游阈值 21376 与 wrapping 启发式实现（`CoreTerminal.ts:283`），升级后须核对分叉逻辑是否仍成立。

**决策**：**保留 beta，不降稳定版**；三件套在 package.json 中精确锁定（ADR-0006 策略）。任何 xterm 版本变更（升/降）须经审批，审批门禁：

1. 全量 L3 headless 渲染测试（`npm run test:l3`）通过；
2. 全量 E2E（`npm run e2e`）通过；
3. **真实 claude 实机滚轮测试**（全屏 TUI + 滚轮滚动；滚轮行为自动化无法守卫——ADR-0005 先例）；
4. Win10 上核对 `CoreTerminal.ts` 的 21376 分叉阈值与 wrapping 启发式现状，评估 ADR-0004 钳制是否仍成立。

**后果**：

- 版本变更必须显式改 package.json 精确版本号，变更本身即触发审批流程。
- 上游发布稳定版时按上述门禁评估升级；发布「新 beta」同样须走审批（不得经 `^` 浮动自动引入）。
- CP-009（2026-09-08）追加口径：PASSTHROUGH_MODE（0x8）由「永久禁用」改为「默认禁用 + 能力矩阵可配置化」（ConptyInputModes 矩阵经 `conptyInputModes` 设置段暴露，设置页开关可启用）——任何 0x8 启用或默认矩阵位翻转同样须过本 ADR 门禁第 3 条（真实 claude 实机滚轮：全屏 TUI + 滚轮滚动）+ 第 4 条（Win10 21376 阈值核对），且 `conpty_flags_default_matrix_matches_legacy_tristate` 守卫用例须绿；无实测记录禁合入。

## 0008 notify RC 保持（9.0.0-rc.4 / notify-debouncer-full 0.8.0-rc.2）

**Status**: accepted（2026-08-18）

**上下文**：Rust 侧 `notify`（文件系统监听核心）与 `notify-debouncer-full` 为 RC 版本。一手证据（`src-tauri/Cargo.toml:36-37、48-49` 跟踪注释）：`notify` 9.0.0 仍为 RC 阶段——最新稳定版 8.2.0（2025-08-03），而 rc.4（2026-05-02）为当前**最新**版本，无更高稳定版可升；降 8.x 属功能回退。watcher 行为有 notify 模块 51 条 L1 回归测试守护。

**决策**：**保持 RC，不降 8.x**；沿用 Cargo.toml 跟踪注释关注 `https://crates.io/crates/notify` 正式发布。上游发布稳定版后升级，升级时跑 notify 模块全量 L1（51 条 watcher 回归）+ 大目录监听实测。

**后果**：

- `notify` 正式版发布即升级触发点（Cargo.toml 注释已登记），升级走常规依赖变更流程。
- RC 风险（API 变动/缺陷）由 51 条 L1 回归守护兜底，与 9.x 前瞻收益（后续版本能力）权衡后接受。

## 0009 review-fix 豁免与决策汇总登记

**Status**: accepted（2026-08-18）

**上下文**：review 修复（93 项）多处以「登记豁免/保留现状 + 文档登记」关闭，登记点散落各模块文档。此处汇总 root 侧豁免决策，避免各登记点失散；模块内明细以对应模块 CLAUDE.md 为准。

**决策**：

| 标识 | 决策 | 登记点 |
|------|------|--------|
| FE-01 | **已作废**：Workspace 多 Dockview 实例保持 + `MAX_PAGES = 20` 上限——多实例架构被共享宿主 + 页组模型取代（CP-004/S11），「页 = 单组」子约束再被 ADR-0020 推翻 | src/workspace/CLAUDE.md |
| SEC-09 | **已被 ADR-0019 取代**：主窗口 CSP 回收 `script-src 'unsafe-inline'` 与 `dangerousDisableAssetCspModification`（CP-012）；预览沙箱经自定义协议域承载 | src-tauri/tauri.conf.json 注释 |
| SEC-06 | 剪贴板读权限 `clipboard-manager:allow-read-text` **保留**（D6）：唯一消费点为 keyboard.ts 的 Ctrl+Shift+V 显式手势，改后端命令不缩小攻击面（前端上下文被注入时同样能 invoke）；grep 级守卫测试锁消费点集合 | src/ipc/CLAUDE.md |
| BE-21 | **已作废**：`fs_read_dir` 不分页登记——CP-006 已改游标分页（默认 500/上限 1000，过滤排序后切片、游标 opaque，前端续页拼接）；FileTree 虚拟化（FE-30）渲染侧保留 | src-tauri/src/fs/CLAUDE.md |
| FE-31 | CodeMirror 大文件**不虚拟化**（按 D3 关闭）：fs_read_file Channel 分块（BE-03）削峰 + 10MB 上限 + 1MB 警告已覆盖峰值；CM6 文档模型不支持部分加载 | src/panels/editor/CLAUDE.md |
| 09#14 | **已作废**：后端 Mutex 已换装 parking_lot（CP-005），中毒攻击面结构性消除 | src-tauri/src/CLAUDE.md |
| TE-03 | xterm 三件套 beta 保留 + 升级审批门禁（L3 + E2E + 真实 claude 实机滚轮 + Win10 21376 阈值核对） | ADR-0007（本文件） |
| TE-04 | notify 9.0.0-rc.4 / notify-debouncer-full 0.8.0-rc.2 保持（rc.4 即最新，无稳定版可升；51 条 L1 watcher 回归守护） | ADR-0008（本文件） |

**后果**：

- 新增豁免须先在本表或对应模块 CLAUDE.md 登记再关闭，禁止只改代码不留档。
- 行为固化点：CSP（tauri.conf.json）、剪贴板守卫测试、`GIT_REPO_CACHE_CAPACITY`/`WATCHER_POOL_CAPACITY`/`MAX_PTY_SESSIONS`（L1 契约测试）。

**沿革**：2026-09-26 文档收敛——死行（FE-01/SEC-09/BE-21/09#14）压缩为一行指针，翻转链全文从 git log 查；`MAX_PAGES` 固化点随 FE-01 作废移除。

## 0010 review-phase2-fix 决策与债务登记

**Status**: accepted（2026-08-22）

**上下文**：review-phase2-fix 清单第 0 节决策表 D12~D20（续 review-fix D1~D11，编号规则：未闭环/partial 沿用原 ID，新发现续编 SEC-15~17/BE-22~25/FE-36~48/TE-14~16/DOC-11~14）。此处汇总 root 侧决策、TE-07 妥协结论与 TE-15 工程债务，模块内明细以对应模块 CLAUDE.md 为准。

**决策（D12~D20）**：

| 编号 | 决策点 | 结论 |
|------|--------|------|
| D12 | 修复范围 | 全量修复：P0+P1+P2+未闭环 10 项+fmt 基线，去重合并后 **37 项**（含 FE-39 验证项） |
| D13 | TE-12 knip 门禁 | 方案 A：补 `entry`/`ignoreExports`/`ignoreFiles` 至 `npx knip --production` 退出码 0；不窄化 CI 口径 |
| D14 | TE-07 TS7 声明失真 | **双 TS 并存（side-by-side）**——见下「TE-07 终态」节 |
| D15 | SEC-15 shell fallback | 收窄为「两侧 canonicalize 均失败且归一化字符串完全相同」才放行，单侧失败即拒绝；`pty/CLAUDE.md` 登记残余风险；补 L1 拒绝用例。不引入 Win32 文件身份比对。alias 兼容保持（Store 版 pwsh 场景两侧指向同一路径、双侧均失败，仍走 fallback 放行）。**2026-09 残余风险已销**：字符串回退改 Win32 句柄级文件身份比对，SEC-15 单侧拒绝保留为纵深 |
| D16 | SEC-04 nonce | **威胁模型已消除**：键转发/命令重放通道随 ADR-0019 预览迁移退役；nonce 保留为纵深。0021 以收窄 keyfwd 重建通道并登记新威胁面（global 命令集锁死，command-catalog.test.ts 守卫） | src/panels/CLAUDE.md |
| D17 | SEC-16 root 竞态 | 后端 `tokio::sync::Mutex` 串行化整个 `set_project_root_impl`（Cargo.toml tokio 补 `"sync"` feature）；前端零改动 |
| D18 | FE-37 store IPC | `setProjectRoot` 调用上提调用方（store 纯状态化）；toast 由 `switchToPageShared` 承担（BE-23 同链修）；不登记豁免 |
| D19 | FE-39 嵌套项目 | 接受「最深前缀」语义；实查测试已固化（`nav-tree-history.test.tsx:302-336`），零代码改动，仅 verify 断言确认存在 |
| D20 | FE-40/FE-41/FE-46 | 三项 P2 均实修（滚动跟随 / 空目录行移除 / ErrorBoundary 重试） |

**核验留痕**：计划期已实读全部修复点代码原文（FE-39 已固化降为验证项；FE-45 实查为 5 处 catch{}，05 报告列 3 处失实）。

**TE-07 终态（双 TS 并存）**：主 typescript 直改 ^7.0.2 **不可行**，D14 三支 fallback 实测走尽（typescript-eslint 8.67.0 peerDependencies `typescript: '>=4.8.4 <6.1.0'` 全系拒绝 TS7、且模块加载期硬校验 `ts.versionMajorMinor >= 7` 崩在加载期，与 type-aware 规则开关无关；overrides 钉兼容组合与根依赖 `^7.0.2` 冲突不可行）。正式化妥协：`"typescript": "npm:@typescript/typescript6@^6.0.2"`（TS6 包装器，供 typescript-eslint 消费）+ `"@typescript/native": "npm:typescript@^7.0.2"`（tsc bin = TS7）。**升级触发条件（机检，`scripts/check-ts7-trigger.mjs`，CP-001 登记硬化）**：退出码 0 = 双条件达成（① issue #10940 `state=closed` ② `typescript` dist-tags.latest = 7.1.x 稳定版）；退出码 1 = 未达成；退出码 2 = 查询失败。触发后删 TS6 包装器与 `@typescript/native` 别名，`"typescript"` 直改 `^7.1.0`。

**TE-15 工程债务**：**已消解（CP-002，2026-09-08）**——json-schema-library 9.x/11.x 双 major 并存随 codemirror-json-schema 摘除消亡（自绘 lint/hover 层 jsonSchemaCm.ts 直消费 11.x 编译单例）。

**沿革**：2026-09-26 文档收敛——D14 执行结果折叠为「TE-07 终态」、D15 残余风险已销并入、D16 威胁模型消除并入、TE-15 消解并入、FE-31 登记点确认等簿记注记删除。

## 0011 文档代码自证原则（CLAUDE.md 只记代码无法自证的信息）

**Status**: accepted（2026-08-23）

**上下文**：项目 30+ 份 CLAUDE.md 长期积累大量「现状描述」——模块职责、文件清单、入口路径、接口形态。这类内容读代码即可得，且代码演化后文档不跟随，必然腐化为失真文档（失真比缺失更有害：误导决策）。

**决策**：全部文档执行**代码自证原则**——凡能通过阅读代码直接理解的信息（职责、文件表、入口、接口签名、数据流、技术栈、现状）一律不写入 CLAUDE.md/rules；文档只记录代码无法自证的信息：① 设计决策与原因（why，含被否决备选）② 外部依赖/OS/三方库的坑与红线 ③ 操作指令（命令、测试门禁）④ 约定与豁免登记（编号索引、配色例外清单等）。模块子文件模板同步改为「存在理由 → 关键约束与决策 → 外部坑/红线 → 测试模式（仅非显而易见部分）」，删「职责」「文件表」两节。根文件模块索引精简为「模块 → CLAUDE.md 链接」两列（仅保留导航职能）；需求编号索引每行压缩为一句话定义。

**被否决的备选**：

- **保留一句话职责/文件表辅助导航**：导航职能由模块索引链接承担；职责描述正是腐化重灾区，开例外即回潮。
- **只去腐不瘦身**：不解决根因——只要原则允许记录现状，腐化会持续累积。

**后果**：

- 改写类任务判定「可读出」必须先实读对应模块代码，禁止凭文档推断。
- 历史修复细节（B10–B16 根因全文等）从根文件压缩为一句话核心约束；细节从 git log 获得。
- 新原则随本次全量优化落地（根文件「文档规范」首条），后续新建/修改文档均受此约束。
- 2026-08-31 演进：根文件「模块索引」与「需求编号索引」随本原则进一步整体删除——子 CLAUDE.md 由 Claude Code 自动加载承接导航（读取子路径文件时自动加载该路径 CLAUDE.md），编号就近在所属模块文档定义。

## 0012 设置中心（统一配置入口 + 配置页注册表 + 后端轻量收口）

**Status**: accepted（2026-08-30）

**上下文**：活动栏底部「配置」钮直达 hooks 配置面板（单一 hub），而配置项将增长为两类——全局级（套餐余量查询频率、快捷键等）与项目级（Hooks 配置等），「配置钮直达单一面板」的形态无法承载。前端需要统一的设置中心交互形态且新增配置页零框架改动；后端需要配置代码高内聚、低耦合、易扩展。

**决策**：
- **载体 = Dockview 面板**（面板类型 `settings`）：左导航固定 180px + 右侧配置页槽位，经 **SettingsPageRegistry**（硬约束 #13 注册表家族新成员，side-effect import 注册）分派渲染；页签标题固定「设置」。左导航组模型后经 ADR-0023 修订为全局/Agent 二分。
- **保存模型**：各配置页自治（即时保存页离散提交不上报 dirty）+ 壳层 dirty 汇聚守卫（`onDirtyChange` 上报、导航圆点、切换页/关闭/自动关闭三处守卫走 confirmDialog）。
- **后端轻量收口三段式**：域键名常量归域模块（settings.rs 只聚合白名单数组）；settings.rs 保持哑存储（白名单/浅合并/原子写/.bak/锁，不引入域语义）；后端消费型域自备专用命令（校验 + 内存态 + 落盘一体收在域模块内，`plan_balance_set_interval` 先例）。
- **写入通道二分**：前端消费型配置（keybindings/fontSize 等）走通用 `save_settings` 段写；后端消费型配置（planBalance.intervalSec）走域专用命令，禁止自建第二写通道（防 SPE-06 并发写竞态回归）。
- **切项目自动关闭 + 全局单例**：面板自订阅自关闭（所在页面不属于活跃项目 → 经 dirty 守卫后自行关闭，dirty 时 confirm 确认丢弃后才关）；与「入口恒解析到项目 pages[0] + 同页单例」叠加 ⇒ 全应用任意时刻至多一个设置中心面板。
- **无项目 toast（R1 计划期实证回弹）**：无项目点击配置钮 → toast「请先创建项目」，面板不打开——无项目=无页面=无 Dockview 宿主（Workspace.tsx 主区 allPages.map），面板无处承载；「项目组禁用 + 空态提示」语义随删除（R3 不可达）。
- **内存间隔值落点（R2 计划期修订）**：套餐余量轮询间隔内存值落点经 R2 计划期修订为 plan_balance 模块级 static 原子量（读写双方同在 plan_balance 模块，无跨模块共享；`SNAPSHOT` 模块级 static 先例；`State<AppState>` 注入使 L1 无法直调命令 fn，现有命令测试全为直调）。

**被否决的备选**：
- **模态框**：遮罩终端、违背面板封闭心智；**独立窗口**：违背单窗口定位约束；**侧栏视图**：宽度放不下双模式（GUI + JSON）编辑器。
- **完整后端注册表**（自注册 crate 依赖）：后端实际消费域仅 1 个（planBalance），机制空转。
- **inventory 自注册**（清单/扫描驱动自动生成注册条目）：引入生成步骤，违背注册表家族 side-effect import 显式注册契约，难以调试。
- **`Ctrl+,` 键盘入口**：推翻「配置钮唯一入口」历史收口决策，第一期维持鼠标唯一入口。

**后果**：
- 新增配置页 = 前端注册一条（组件 + `register`）+ 后端消费型另加专用命令，框架零改动。
- **`hooksConfig` 面板类型退役**：注册表与 PANEL_TYPES 移除，老布局由恢复白名单过滤静默丢弃（无迁移映射）；hub 改造为 HooksSettingsPage 迁入设置中心（组模型后随 ADR-0023 再调整），编辑器归域 `cliProfiles/profiles/claude/configEditor/`。
- **F10 豁免口径更新**：套餐余量轮询间隔从「启动时读一次」改为运行期可改（plan_balance 模块级 static 原子量 + 专用命令写入，每轮末按内存值 sleep），F10 相关豁免登记随之修订。

## 0013 后台定时任务双端抽象（任务元数据单点在后端 + 配置单写通道）

**Status**: accepted（2026-08-30）

**上下文**：套餐余量查询（F10 后端 tokio poller）之后新增第二个后台定时任务「session 历史刷新」。两任务执行位置本质不同——套餐余量需后端 OS/网络能力且快照预热语义在后端；session 刷新必须与手动刷新钮严格同一代码路径（前端 `scan(true)`），而两任务配置又需统一管理与展示。浅合并写通道（settings.rs 顶层键粒度）下两任务共用 `backgroundTasks` 一段会产生写覆盖冲突（前端写整体替换顶层键，丢对方子键）。

**决策**：

- **双端各自抽象，不设跨端统一调度器**：后端泛化 plan_balance 通用件（间隔内存原子量/每轮末 sleep/读盘初始化/set 命令）为任务骨架（静态切片注册表，照 `SOURCES`/`QUERIES` 先例）；前端新建 backgroundTasks 调度器（#13 注册表家族：全局单例、订阅者计数启停、首轮立即执行、tick 防重入、triggerNow）。
- **任务元数据单点 = 后端注册表**（含前端任务的代管：执行体字段 None 即前端任务，后端只管 id/标题/边界/默认值与配置读写）；设置页与前端调度器统一经 `background_tasks_list()` 读通道取数，前端不复制边界/默认值——DTO `BackgroundTaskInfo` 六键**无 default 字段**（FR-2 写死）的直接后果：行内提示只写范围不写默认值，默认值变更只动后端注册表。taskId 合法值集前后端同步测试锁死（HooksLayer ↔ `Layer` 枚举先例，硬约束 #4）。
- **配置单写通道 = 后端 `background_tasks_set_config` 命令**（taskId 子键读-改-写合并 → 复用 settings.rs 写通道），前端任务的配置也经此命令代管落盘——杜绝浅合并顶层键互覆；前端消费型 `save_settings` 段写不适用于本段。
- **配置结构**：统一顶层段 `backgroundTasks.{taskId}.{enabled,intervalSec}`；白名单 `planBalance` 键替换为 `backgroundTasks`；单用户不做旧键迁移。
- **配置变更前端感知 = 后端 emit 事件，不建前端总线**：`background_tasks_set_config` 成功后 emit `background-tasks-updated`（payload = 完整 `BackgroundTaskInfo[]`），footer/设置页订阅即知；`background_tasks_list` 只作读通道不 emit。后端单写通道是配置真值源，前端自建总线会造成双真值源脱节。
- **session 刷新 = 扫描执行体单一化**：手动刷新（刷新钮/triggerNow）与定时 tick 同一执行体（遍历全部已注册 history provider 逐个 `scan(true)` 聚合），仅失败处理按触发来源分化（tick 静默 / manual 置 error）；`useAgentHistory` 的 sessions/state 真值源上移调度器快照，hook 退为订阅方。

**被否决的备选**：

- **全收敛前端统一调度器**（套餐余量改前端定时调 `refresh_plan_balance`，删后端 poller）：丢启动预热语义、动 F10 已稳定的轮询编排与测试基座，违反「已有功能不受影响」红线。
- **拆两顶层键各走各写通道**（planBalance 专用命令 + sessionRefresh 通用段写）：无冲突但白名单随任务数膨胀、两任务配置模型不统一，与「统一管理」目标相悖。
- **通用 save_settings 段写 + 后端每轮读盘**：丢运行期立改语义且每轮读盘，劣于内存原子量方案。
- **session 刷新放后端 poller + emit 推送**：与刷新钮成两套通道（invoke vs 事件），违背「同一套逻辑」红线，且 useAgentHistory 订阅模型需重写。

**后果**：

- 新增后端任务 = 注册表一条元数据 + 执行体闭包；新增前端任务 = 后端一条元数据（执行体 None）+ 前端调度器 register 一条；设置页/写通道/调度框架零改动。
- `plan_balance_set_interval` 命令退役；套餐余量新增 enabled 语义（停轮询 + footer 隐藏 + 快照保留），默认间隔 60 → 10s。
- 前端调度器与 UI 解耦：NavTree 换区重建（ADR-0001）不影响定时刷新；无订阅者不空转扫盘。
- settings.rs 浅合并语义不变，跨任务写冲突由「单写通道 + 子键合并」在命令层消化。

## 0014 CLI 别名持久化走通用 save_settings 段透传（校验全前端，Rust 白名单加键）

**Status**: accepted（2026-09-05）

**上下文**：CLI 别名（claude 等编码 CLI 的用户自定义启动命令名）需持久化到 settings.json。settings 模块既有两条路径：① 通用 `save_settings` 浅合并顶层段透传（fontSize/keybindings/sideBar 先例，Rust 仅白名单校验）；② 域模块专用命令（backgroundTasks 先例，Rust 端持有任务注册表故校验/合并/emit 收后端）。别名域的语法与 D3「全命名空间唯一」（不得撞任何 profile 内置命令或其它别名）判定需要「全部 CLI 内置命令名集合」——该知识只存在于前端 CliProfileRegistry（注册表 + profiles/* 静态声明），Rust 侧仅有 hooks/history 两个 cliId 键 provider 注册表（无 commands 概念）。若仿 backgroundTasks 走专用命令，Rust 必须复刻一份 CLI 内置命令名单——双源漂移：新增 CLI 需三处同步（前端 profile、hooks provider、history provider）变四处，违背高内聚要求。

**决策**：

- 别名配置存 settings.json `cliAliases` 段（cliId → 别名数组），Rust `SETTINGS_ALLOWED_KEYS` 白名单加键，内容**纯透传**：不设专用命令/DTO/emit，语法与唯一性校验全前端（cliProfiles 域纯函数 `aliasValidation.ts`），存储走通用 `save_settings` 段写 + 2s debounce（stores/cliAliases.ts，keybindings 模式同构）。
- 手改文件/版本残留产生违例数据 → 前端 loadFromDisk sanitize 兜底（孤儿 cliId 键丢弃、语法不过丢弃、撞内置/重复先到先占）；运行期磁盘改不改内存快照（与 keybindings 行为一致，接受）。
- 未消费方模型：别名运行时经 App 组合层注入注册表别名快照（ADR-0015），后端零感知。

**被否决的备选**：

- **Rust 专用命令校验**（backgroundTasks 形态）：Rust 无 CLI commands 知识源，需复刻注册表或至少内置命令名单 → 知识双源、新 CLI 四处注册，且唯一性跨 cli 判定的真值源（前端注册表）与校验执行地（Rust）分离会随时间漂移。
- **别名并入 profile.commands 静态字段**：见 ADR-0015（独立成案）。
- **后端校验 + 前端注册表推送名单**：为低价值校验域引入跨端同步协议，复杂化无收益。

**后果**：

- 新增 CLI 的步骤不变（前端 profile + 三处后端 provider），别名能力对新 CLI 自动适用（UI 注册表驱动分区）。
- 白名单键集需前端 store 与后端双侧测试锁死（save_accepts_cli_aliases_key + 段形态契约）。
- 逆转触发点：未来别名需要后端参与（如 shell 层展开/注入）或出现第二个前端外知识源时，重估专用命令方案。

**沿革**：白名单键数随后续加键演化（ADR-0024 追加 `agentGlobalFiles`），以代码现状为准。

## 0015 CLI 别名不进 profile.commands，注册表持独立别名快照（matchByCommand 内置 → 别名回退）

**Status**: accepted（2026-09-05）

**上下文**：CodingCliProfile.commands 是「首 token 精确匹配键集」静态声明（types.ts 注释支持 ["claude","cc"] 多首词形态）。用户别名需求出现后，最直觉的落点是往 commands 数组追加——但 commands 承担三个不允许被污染的职责：① D3 命名空间计算的真值源（内置命令名集合 = `getAll().flatMap(commands)`，用户别名混入后无法区分来源）；② `register` 同 id 覆盖语义（静态覆盖 vs 用户配置生命周期不同）；③ 遍历 profile 的消费方（logo 资源守卫等）对 commands 的静态假设。且别名须运行时增删即时生效（D4），改静态字段需重注册/重遍历。

**决策**：

- `CliProfileRegistry` 增加旁路别名快照：`aliasByToken`（别名 token → cliId 逆映射平铺）+ `setAliases(byCliId)`（全量替换，单一写点，低频）。`register/get/getAll` 契约零改动。
- `matchByCommand` 保持全仓唯一首 token 解析点（MC-102）原样，内置 `profile.commands` 查表未中后回退 `aliasByToken` 查表 → 返回映射 profile 或 null。匹配顺序「内置 → 别名」：D3 保证两空间无交；手改文件绕过 sanitize 产生违例时内置优先为保守方向（内置命令语义不可被别名遮蔽）。
- 消费方零改动：别名命中返回与内置完全相同的 profile 对象，oscHandlers → TerminalRegistry.setAgentSession(cliId) → 页签标题/logo/侧栏 session 全链路自动一致（matchedCommand 记 cliId 而非原始 token，与内置命中同语义）。
- `_reset()` 契约扩为清全部注册态（profile + 别名快照）。
- 快照写入编排在 App.tsx 组合层（loaded 守卫 + store subscribe → setAliases，仿 wireKeybindings），别名数据状态在 stores/cliAliases.ts。

**被否决的备选**：

- **别名直接并入 `profile.commands`（注册时拼接/动态改数组）**：破坏内置命令名计算（D3 判定、跨 cli 冲突识别全失真）；`register` 覆盖语义与 profile 类型契约（types.ts，跨边界 spec 00 §3.1）被破坏；logo 守卫等遍历消费方污染；别名「当前 CLI 配置」与「CLI 自身静态身份」生命周期耦合。
- **matchByCommand 参数注入别名集 / 检测层组合查询**：检测链（oscHandlers → useCommandDetection）持注册表方法引用惰性注入，改组合层需自解析首 token 或外迁 MC-102 解析点，违背单点化契约；mock 调用面全量波及。
- **注册表外建第二解析点（useCommandDetection 先查别名再调注册表）**：两处首 token 解析 = MC-102 回归。

**后果**：

- commands 字段语义收窄为「内置命令静态声明」，注释无需修改（其多首词描述针对未来 CLI 自身多命令名场景，仍正确）。
- D3 唯一性查询无需注册表新 API（内置集合就地 flatMap，别名集合读 store），API 不膨胀。
- 新 CLI profile 注册流程零变化，别名能力自动覆盖；别名与内置的命名空间冲突在 UI 校验层消化（aliasValidation）。

## 0016 E2E 假 home 隔离（USERPROFILE 指向临时假屋，替代备份/还原）

**Status**: accepted（2026-09-05）

**上下文**：E2E 会真实写盘用户 home 配置——`settings.e2e.ts`/`background-tasks.e2e.ts` 的 `writeFakePlanEnv` 把 `~/.claude/settings.json` 的 env 键覆写为 `sk-test-e2e` 假值（suite 末才还原，窗口分钟级）；agent.e2e 起 `ensureHooksInjected` 注入 hooks matcher + statusLine 桥接，跨整个 run（10+ 分钟）。2026-09-05 事故实证：假值窗口内真实 claude 会话启动读到假 token → API 401「no token found」。旧备份/还原机制（run-wdio `.e2e-bak`）只能保证 run 后恢复，**窗口期污染与 kill 残留固化均无法消除**——且 e2e app 的 plan_balance poller 全程用真实 home env 打真实 API（真实 token 副作用）。

**决策**：

- **假 home 隔离**：run-wdio.cjs 建临时假屋（`<tmp>/slterm-e2e-home-<pid>`）并把 `USERPROFILE` 指向它——Node `os.homedir()`（libuv uv_os_homedir，每调用重读 USERPROFILE）全链跟随（spec 9+ 引用、注入 JS 脚本、tauri driver、PTY 子进程），e2e 全部用户目录写入落假屋，**真实用户目录零接触**。假屋目录每次运行唯一（pid 后缀）：IME/遥测组件（搜狗输入法等经 WebView2 激活拉起、继承假 env）会把自身 AppData 数据写进假屋并长驻句柄——固定名假屋启动清空必撞 EPERM（实证），唯一名免清空，exit 清理 best-effort。
- **Rust 侧代码收敛（硬前提）**：dirs 6.0.0 / dirs-sys 0.5.0 的 `home_dir()` 走 `SHGetKnownFolderPath(known_folder_profile)`，**完全不读 USERPROFILE/HOME env**（dirs-sys 0.5.0 源码实证）——纯 env 注入只能隔离 Node/子进程，Rust 4 个消费点（hooks/claude、hooks/watcher 信号目录、plan_balance、agent_history fallback）必须统一到新建顶层共享件 `crate::home::home_dir()`：cfg(test) `HomeDirGuard` > env `USERPROFILE`（非空）> `dirs::home_dir()`。两处照抄守卫（hooks/claude、plan_balance）收编删除；生产代码禁裸 `dirs::home_dir()`（grep 收敛纪律）。`SLTERM_CLAUDE_PROJECTS_DIR`（history fixture 副本）保持独立且优先级高于 home。
- **备份/还原机制整体退役**：run-wdio 约 145 行备份/还原死代码删除；`SLTERM_DATA_DIR` 应用数据隔离保留。
- **防复发校验**：覆盖 USERPROFILE 前对真实屋（`~/.claude/settings.json`、`~/.slterminal/statusline-backup.json`、`~/.slterminal/hooks/`）做存在性 + sha256 快照，exit 逐项比对——任何泄漏（未来新裸 dirs 消费点/Rust 收敛遗漏）独立报红 `exitCode=1`。`hooks-events` 仅当启动时不存在才校验（存在 = 用户会话在用，防误报）。校验失败同时可能是外部进程并发写真实屋（如用户同时开 claude）——错误消息列排查方向。

**被否决的备选**：

- **保留备份/还原 + 缩短 spec 污染窗口**（writeFakePlanEnv 收敛到用例级 try/finally + 残留自检）：hooks 注入宽窗口（跨整 run 改 statusLine）仍在；kill 残留固化的根因未除（下次 run 备份坏值再还原坏值）；「e2e 必须写真实用户配置」的错误前提未推翻。
- **仅 Node/子进程层 USERPROFILE 注入、Rust 不动**：dirs 6.0 Windows 不读 env → Rust 侧仍读写真实屋，隔离名存实亡。
- **假屋固定名 + 启动强删**：第三方组件（IME 等）长驻句柄 → EPERM fail（实证），除非杀用户进程（不可接受）。

**后果**：

- e2e 期间用户可安全使用真实 claude/slterminal（真实屋零接触）；planBalance 不再用真实 token 打真实 API。
- 真实屋零接触承诺范围 = `~/.claude` + `~/.slterminal`；`%LOCALAPPDATA%` WebView2 数据不在承诺内（环境变量未动，与手动运行一致）。
- 残留假屋目录（IME 句柄删不净）留在 tmp 无害，OS 可回收；`.e2e-bak` 旧残留由一次性人工清理。
- 逆转触发点：未来出现不经 `crate::home` 的 home 消费点时由 grep 纪律 + exit 校验双兜底报红；恢复备份/还原机制需重估隔离键选择。

## 0017 预览容器信任模型：md/html 同态渲染（sandbox 无 allow-same-origin + global 命令集不扩）

**Status**: accepted（2026-09-06）

**上下文**：为 .md 新增预览能力时，渲染容器选型存在两条路：① 与 htmlviewer 同款的 sandbox iframe（opaque origin，注入桥 + postMessage 总线——SEC-03/04 校验体系、zoomRuntime/键转发/HUD 全现成）；② 宿主 DOM 直接注入（dangerouslySetInnerHTML，样式隔离自理）。容器选择连带决定信任模型：md 渲染产物含 raw HTML（用户 md 内嵌 <details>/<table>/<script> 等），htmlviewer 的既有语义是「本地文件全信任」（CSP 'unsafe-inline' + 无消毒，Tauri CVE-2024-35222 红线排除了 allow-same-origin）。跨 html/md/workspace/shortcuts 四模块、与 SEC-03/04 及既有 CVE 决策直接勾连，需要决策记录锚点。

**决策**：

- **md 预览与 html 渲染同态**：iframe sandbox="allow-scripts"（无 allow-same-origin，Tauri CVE-2024-35222）、注入桥（键转发/缩放/滚动/链接路由）与 postMessage 校验（origin="null" + source + nonce + type）收 docViewer/PreviewFrame 单点（复用不复制）；raw HTML 透传（markdown-it html:true）。原「宿主 `<script>` 因 escapeScriptClose 转义静态化」为登记存量缺陷，已随预览迁移消除（CP-031：转义函数删除，宿主脚本于预览域真实执行）。
- **global 命令集不扩充**：预览键转发只触及 global context 命令（当前仅 global.closeTab，command-catalog.test.ts 锁死）；扩充须先重评 SEC-04 威胁模型（nonce 明文内联于注入产物，防外部伪造不防预览内容自身）。通道形态历经「主窗 iframe 键转发（本条）→ ADR-0019 独立窗口退役 → ADR-0021 收窄 keyfwd 重建（表单字段不转发、载荷不含 key 值）」，命令集最小化原则不变。
- **链接分派收父侧**：linkRouter 段仅上行 href（slterm_nav），分类（external → 系统浏览器 opener / local → 应用内打开链路）在面板侧 linkPolicy 纯函数做——iframe 内不做任何打开决策。
- **缩放/滚动恢复语义**：keepZoom/keepScrollRatio 重建下行恢复（钳制/比例近似，登记已知行为），缩放状态不跨会话持久化（html 现状语义继承）。

**被否决的备选**：

- **宿主 DOM 直接注入 md 渲染产物**：与 htmlviewer 形成两套渲染面（沙箱/信任/键桥/缩放全复制或抽象重建）；宿主权限执行预览内容扩大信任面；相对资源/CSS 隔离/闪白需自研全套——安全红线登记（SEC-03/04）与威胁模型将分叉。
- **md 渲染做消毒管线（DOMPurify）**：用户需求确认信任边界 = 与 html 渲染同等（md 是用户本地文件，非第三方内容）；消毒引入「html 可显示 md 不可显示」的双轨行为与额外依赖面。

**后果**：

- 新预览型文件（未来 pdf/png 等）扩展路径 = 面板目录 + docViewer 注入段/回调组合，安全面不新开。
- 扩充 global 命令集前必读本 ADR（威胁面现经 ADR-0021 决策 3 登记：keyfwd 载荷伪造危害边界 = global 命令集）。
- 逆转触发点：出现预览不可信第三方内容需求（届时重新评估消毒/隔离）。

**沿革**：宿主 script 静态化缺陷经 CP-031 消除（ADR-0019）；键转发通道经 ADR-0019 退役、ADR-0021 收窄重建。

## 0018 本地资源通道：项目根只读 + data: 内联（沙箱内二进制入渲染面首例）

**Status**: partially superseded by 0019（主窗口 CSP 的 img-src/font-src data: 放行已回收，CP-035）（2026-09-06）

**上下文**：md/html 预览的本地相对资源（图片等）首次需要「按路径读任意二进制进渲染面」。既有 fs_read_file 只读 UTF-8 文本（编码校验）；iframe 为 opaque origin（无相对路径/asset 协议可达性，CSP font-src 无 data: 放行）。候选通道：① Tauri asset protocol（convertFileSrc）——静态 scope 无法跟随动态项目根，违背「仅项目根沙箱内」用户决策；② blob: URL——无生命周期管理点（iframe 重建即失效需 revoke，CSP 需放行 blob:）；③ 新后端命令 + data: URL 内联。全局 CSP 变更（首个 data: 放行）与「沙箱内二进制入渲染面」通道形态影响所有未来预览型面板，需决策记录锚点。

**决策**：

- **新命令 fs_read_resource（项目根只读通道）**：复用 extract_root 沙箱（root=None 拒绝）+ validate_path_within_root + 10MB 上限（复用 fs_read_file 常量）+ spawn_blocking；256KB 原字节分块 base64 Channel 推送（UTF-8 安全、削峰同 BE-03）；前端 readResourceBase64 聚合，MIME 推断在前端扩展名白名单（png/jpg/jpeg/gif/webp/avif/bmp/ico——svg 后于 CP-035 剔除，见下）——后端保持「读字节」单一职责。三处注册（lib.rs/build.rs/capabilities）。
- **data: URL 内联（非 blob:）**：渲染产物替换为 data: URL——字符串可比（重建去重）、无 revoke 生命周期；资源读取失败回退原 src（缺口不阻塞整篇）；LRU 缓存（50 项）防逐字重渲染反复读盘。
- **KaTeX 字体构建期内联**：katex.min.css woff2 url → data:font/woff2;base64（scripts/gen-katex-inline.mjs 产物提交入库，woff/ttf 回退剔除）；katex 升级重跑脚本，CI diff 守卫（漏跑即红，CP-033）为长期形态。
- **渲染执行分层**：mermaid 宿主侧渲染（dynamic import + 按 code Promise 缓存）成 SVG 字符串注入——iframe 内零重排版、CSP 零新增、2MB 库不进主包；暗色主题变量映射 linear 内容色。
- **CSP 终态（CP-035 回收后）**：主窗口 img-src `'self' asset: https://asset.localhost`（回收 data:）、font-src `'self'`（回收 data:），csp-config.test.ts 三守卫锁死；预览域 CSP = 宿主页 meta 域级（`img-src data:; font-src data:` 已放行，SEC-02 落地，见 ADR-0019 决策二 9）。回收期唯一主窗口 data: 消费点 = CM6 lint 诊断波浪线（上游 baseTheme 以 `background-image: url(data:image/svg+xml,…)` 渲染）——改 text-decoration wavy 技法消除（零 CSP 指令依赖），theme-overrides.test.ts 加「规则文本零 data: url」防回潮断言。
- **svg data: 显式禁用（CP-035）**：markdown assets.ts MIME 白名单剔除 image/svg+xml（本地 .svg 引用不再内联——缺口语义）——svg 载体可嵌脚本，`<img>` 惰性上下文仅为 W3C 行为单点不作安全边界；markdown-assets.test.ts 锁「svg MIME 不在白名单」。

**被否决的备选**：

- **Tauri asset protocol + convertFileSrc**：assetProtocol scope 静态配置无法跟随 set_project_root 动态根（静态大 scope 违背最小沙箱）；跨源 CORS 在内容域不可过（CP-033 双证据：① tauri 2.11.5 源码实证 asset 协议响应恒带 `Access-Control-Allow-Origin: <webview 自身 origin>`，与 opaque origin 内容 iframe 的 null Origin 不匹配；② 真实 WebView2 实测 cors fetch asset 域拒绝）。
- **blob: URL + 逐轮 revoke**：生命周期管理点缺失（iframe 重建竞态）、CSP 需放行 blob:、字符串不可比（无法跳过等值重建）。
- **mermaid 运行时注入 iframe**：2MB+ 源码进注入串违反无 `</script>` 字面量纪律（库内必然出现，需逃逸变换）；300ms 防抖每轮重建全量初始化，成本为宿主渲染数量级倍数。

**后果**：

- 「沙箱内二进制入渲染面」有了统一通道（html/md 预览共用；html 相对图片顺带可用）；未来新资源类型走 MIME 白名单扩展。
- KaTeX 构建期内联经真实 WebView2 实证在预览域可用（字体族命中 + `document.fonts.check` 为真，CP-033）——原「opaque origin 内行为未实证」缺口关闭。
- 逆转触发点：动态 asset scope 出现（Tauri 支持跟随项目根时重估协议通道）；或需读 >10MB 资源/任意沙箱外路径（重估上限与边界）。

**沿革**：CP-033（2026-09-08）KaTeX 预览域实证 + asset 通道维持否决双证据 + CI diff 守卫；CP-035（2026-09-08）主窗口 data: 回收 + svg 禁用 + lint 波浪线技法替换；SEC-02（2026-09-09）预览域 CSP 经宿主页 meta 落地。2026-09-26 文档收敛——「维持记录/回收记录/落地复核注记」三个追加块折叠入正文。

## 0019 预览安全域模型：自定义协议宿主页 + sandbox iframe（S10-① spike + S10-② 迁移落地）

**Status**: partially superseded by 0021（预览载体：独立 WebviewWindow → 主窗内跨源沙箱 iframe；安全域决策全部保留）（2026-09-08）

**上下文**：CP-012/013/035/044 同根（预览通道 iframe srcDoc 与主窗口共享 CSP/上下文，SEC-09 结构性问题）。修复方向 = 预览渲染迁出主窗口 CSP 域。整个 S10 的 go/no-go 依赖「WDIO（embedded driver）能否枚举/驱动独立预览 webview」——此前零实证。

**决策一（S10-① spike 实证，2026-09-08，spike 代码不入库）**：

1. **go——四问全 yes**：可枚举（`getWindowHandles()` = webview_windows label 全集）、execute 可达（switchToWindow 后作用于预览页上下文）、焦点语义解耦（显式 switch 后无 +5s 惩罚）、销毁语义（closeWindow 后句柄收缩、driver 存活）。
2. **CSP 无 per-webview 覆盖**（tauri 2.11.5 builder API 无 csp 属性；CSP 为 app 级配置，资产页恒被注入全局 CSP 头 + 静态内联脚本哈希化——tauri 源码实证）——**该结论仍成立**，是预览域 CSP 只能由宿主页 meta 承载的根因。
3. 载体相关结论（跨独立窗口无 postMessage → 消息桥走 Tauri event/IPC；WDIO 按 label 驱动窗口）已随 ADR-0021 载体回迁失效——0021 决策 2（同窗口树 postMessage 三层通道）与决策 5（E2E 探针模式）取代。

**决策二（S10-② 迁移落地，2026-09-08）**——安全域决策全部延续至今：

1. **预览承载域 = 自定义协议宿主页**：预览加载 Rust 注册的自定义协议 `slterm-preview`（Windows 映射 `http://slterm-preview.localhost/preview-host.html`，register_uri_scheme_protocol 文档实证；响应不带全局 CSP）。asset 协议页恒被注入全局 CSP 头——主窗口收紧 script-src 后，资产域内运行时内联注入与宿主自带脚本全灭 → asset 候选否决。
2. **宿主页 = 固定桥接页**（preview.rs 内嵌 const HOST_PAGE）：建 sandbox iframe（allow-scripts、无 allow-same-origin——CVE-2024-35222 红线延续，iframe 无 Tauri IPC 注入——main_frame_only 实证）。独立窗口时代的内容通道（preview_render/preview_pull/preview_sync/preview_close 四命令 + 内容存储 + 会话守卫）已经 ADR-0021 整体删除，改 postMessage 三层通道；事件名单点登记 previewMessages.ts（TS / 宿主桥 / 守卫测试三处同步）。
3. **注入机制原样迁入**：injectScript + buildInjectedScript + nonce 装配于主窗 PreviewFrame，产物推送预览域执行；**字符串级转义消亡**（escapeScriptClose 删除——宿主 `<script>` 不经转义进入渲染文档且真实执行，CP-031，html.e2e 宿主 script 用例真实 WebView2 断言）。
4. **上行命令面（CP-013 → ADR-0021 重建）**：独立窗口形态下键转发整体退役（预览窗口 focusable=false，键盘不跨窗口），上行集合收窄为 {slterm_zoom, slterm_scroll, slterm_nav}；ADR-0021 决策 3 以收窄 keyfwd 重建键盘通道（表单字段不转发、载荷不含 key 值），global 命令集保持最小（command-catalog.test.ts 锁死）。
5. **主窗口 CSP 终态（CP-012）**：`script-src 'self'` + 删除 dangerousDisableAssetCspModification 整键（img-src/font-src data: 回收见 ADR-0018；`frame-src http://slterm-preview.localhost` 后经 ADR-0021 决策 4 收窄式新增）。
6. **CM 保活维持（CP-037 复核）**：预览迁出后 workspace 层 CSS 显隐保活对预览不适用——CM 保活（面板内 allotment visible=false 恒挂载）**维持**（edit/split/preview 形态往返仍须保留 undo/光标）。独立窗口时代的「预览窗口隐藏保活（hide 不销毁）」机制随 ADR-0021 回迁消亡（显隐随主窗 DOM 天然正确）。
7. **面板形态（延续至今）**：面板根 = 工具条带（切换条/HUD 悬浮带，FloatingArea direction="row"）+ 内容区（PreviewFrame 锚点）列排。独立窗口专属部分（窗口 owned 无边框/focusable(false)/skip_taskbar、锚点矩形几何换算 + 200ms 轮询同步）随 ADR-0021 消亡。
8. **键盘边界已知行为——已消除**：独立窗口形态的「预览文档内表单键入/系统复制不可达」经 ADR-0021 决策 3 收窄 keyfwd 解禁。
9. **域级 CSP 落地（SEC-02，2026-09-09）**：宿主页 HOST_PAGE 加 `<meta http-equiv="Content-Security-Policy">`——指令表 `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:`（`default-src 'none'` 断全部出网；内联 script/style 为宿主桥 + 注入产物必需；img/font `data:` 为 markdown 本地图内联与 KaTeX 内联字体通道，ADR-0018）；srcdoc iframe 继承宿主 CSP（W3C 行为）→ 预览文档同受此约束。tauri 2.11 无 per-webview CSP 配置面 → 域级 CSP 只能由宿主页 meta 承载，加宽指令须复核 SEC 面。

**被否决的备选**：

- **同窗口 add_child 子 webview（多 webview-in-window）**：driver 不可枚举且宿主窗口整体消失（实测）。
- **data: URL 注入承载预览**：内容被全局 CSP meta 包裹改写 + opaque origin + 需 webview-data-url feature（实测）。
- **asset 协议宿主页承载预览（spike 期首选）**：CSP 无 per-webview 覆盖下资产域恒带全局 CSP 头 + 静态哈希——收紧后运行时注入/宿主脚本不可行。
- **主窗侧保留 iframe + 运行时经 'self' 外部 js 注入（资产域候选路线）**：运行时代码须运行时生成（nonce/段型/内容变化），无法静态化；且宿主文档 `<script>` 永不可执行（CP-031 判定三不可达）、预览域 data: 放行无落点。（注：本备选特指资产域 `'self'` 注入路线；ADR-0021 路线 A 的「主窗内 iframe」= 自定义协议宿主页承载，安全域不变，不属本条。）

**后果**：

- 安全域资产全部延续至今：自定义协议宿主页域、sandbox 无 allow-same-origin、main_frame_only 无 IPC 注入、域级 CSP meta（决策二 9）、nonce + 类型白名单校验链。
- 独立窗口载体族已整体消亡（ADR-0021）：Rust preview 模块四命令/内容存储/会话守卫、前端几何编排/轮询/force-sync、capabilities preview.json、E2E label 驱动窗口族。
- **逆转触发点**：Tauri 提供 per-webview CSP 时复核预览域选择；global 命令集扩充时重估 keyfwd 威胁面（ADR-0021 决策 3）。

**沿革**：2026-09-13 载体决策（独立 WebviewWindow）被 ADR-0021 推翻（拖动跟随延迟结构性不可归零），安全域决策全部保留；CP-033/CP-035（③④）结果登记折叠至 ADR-0018 正文；SEC-02 域级 CSP meta（决策二 9）为落地后追加。2026-09-26 文档收敛——顶部取代 banner 转入 Status 行，独立窗口运维细节压缩。

## 0020 页内分屏（推翻 CP-004/S11「页 = 单组多页签」）

**Status**: accepted（2026-09-12。D3 用户裁决支持页内分屏；D7 仅网格分屏，禁 floating/popout）

**上下文**：CP-004/S11 共享宿主模型登记「页 = 单组多页签」，页内分屏不可用、拖拽拆分产物被回迁守卫清理。但该守卫实际从未对真实拖拽生效（dockview `_moving` 门控吞移动期 onDidAddPanel/onDidRemovePanel，守卫挂错事件源）；真实拖拽产 stray 组后被 sync 末尾无条件 maximizePageGroup 隐藏（面板「消失」，DOM 保留）+ 切片持久化只切主组叶把 stray 组剔除（重启真丢失）。用户裁决（D3）：支持页内分屏，推翻单组限制。

**决策**：

1. **共享宿主 + 派生归属模型**：单一 DockviewReact 不变。页主组 id = `page-{pageId}` 保留为恢复锚点 + Watermark 载体；页内分屏产物 = dockview 自生组（自增 id，无 page- 前缀）。**组页归属不从组 id 断言，从组内首面板 id 前缀派生**（pageGroups.ts `pageIdOfGroup`：id 快车道 ?? 组内首面板页前缀 ?? null；`groupsOfPage` 页内组枚举；`resolvePageGroupForAdd` = 主组 ?? 页内首组 ?? null——null 时调用方显式失败，不落活跃组防错页）。主组可被拖空删除（dockview 自动删空组）——新增面板落组经回退链解析。
2. **可见性机制 = setVisible 逐组显隐（取代 maximize 单组最大化）**：`setActivePageVisibility(api, activePageId)` 遍历 grid 组按派生归属 setVisible——同页多组同隐同显；底层与 maximize 同一 setViewVisible 机制（DOM 保留不卸载，xterm 不重建），且 setViewVisible 首行 exitMaximizedView——弃 maximize 无残留冲突。**红线：dockview setVisible 无条件 fire onDidLayoutChange（等值也 fire）——必须等值跳过**，否则 sync()→setVisible→layoutChange→store 写回→sync() 死循环（jsdom 实证挂死）。
3. **存储形态 = 页子树切片（多叶）**：`OperationPage.layout` root 恒 branch 壳，data = 本页各组节点（单组 = branch[leaf]；分屏 = 多叶或嵌套 branch）——向后兼容天然成立（旧单叶切片 = 新形态子集）；切片剥 visible 标记（toJSON 隐藏叶带 visible:false 入存储会致恢复恒隐藏）、activeView ∉ views 归位 views[0]；activeGroup 保留切片声明值（属本页叶集时）；floatingGroups/popoutGroups 段不存（D7）。
4. **守卫重挂事件源**：`enforcePanelGroupMembership`（挂 onDidAddPanel）删除；`auditGroupMembership(api)` 挂 `onDidMovePanel`（在 movingLock 外 fire，不被吞）+ onDidAddPanel + 恢复后全量一次——自生空壳组 removeGroup、混组以首面板页为属主、少数派面板回迁（模块级重入旗标 + 幂等无环）；restoreGuard 期间跳过。
5. **仅网格分屏（D7）**：DockviewReact `disableFloatingGroups` prop 禁 floating 手势（库内两入口均受其门控）；popout 仅 API 可达，不调用即禁用。
6. **面板实例复用**：dockview 移动复用同一 DockviewPanel 实例、内容 DOM reparent 不重建 → xterm 不二次 open（#4978 安全），终端缓冲跨分屏/切页保留。

**被否决的备选**：

- **onDidAddPanel 守卫维持**：`_moving` 门控吞移动期事件（dockview-core 源码实证）——对真实拖拽从不触发，名存实亡；否决。
- **CSS display 直接操作 dockview 叶元素**：布局管理器不知情会错位（CP-004 红线沿用）。
- **maximize 语义扩展多叶同显**：maximize 单组语义无法表达同页多组同显；否决，改 setVisible 逐组（决策 2）。
- **保留 floating/popout 能力**：归属派生/切片持久化/恢复链对非网格组全部复杂化，无产品需求（D7）；否决。

**后果**：

- 跨页组拖拽禁令不变（审计回迁少数派）；页内拖拽分屏合法化。
- 删页 = `groupsOfPage` 逐组 removeGroup（分屏组一并清除）；页内 addPanel 五入口统一 `resolvePageGroupForAdd` 落组。
- 组对象 identity 跨 whole-grid fromJSON 可变约束不变（CP-004 登记）。
- L4 `workspace-split.e2e.ts` 经 moveTo 等价落点覆盖分屏路径；真实拖拽手势（pointer 序列）自动化豁免登记 test-exemptions.md。

## 0021 预览载体回迁主窗内跨源沙箱 iframe（推翻 ADR-0019 载体决策，保留其安全域决策）

**Status**: accepted（2026-09-13。spike 四过一否 + 用户裁决 D1 路线 A / D2 收窄转发 / D3 frame-src；Q4 失败处置 = 探针断言）

**上下文**：拖动主窗时 md「预览」/html「渲染」页签跟随有延迟（编辑器页签无延迟）。根因结构性：预览 = ADR-0019 独立 OS 窗口，跟随链 = 主窗移动事件 → JS 监听 → 50ms 节流 → invoke IPC → Rust SetWindowPos，三重延迟不可归零（ADR-0019 自登记「拖拽期亚秒级滞后」，force-sync 事件链 + 锚点 ResizeObserver 两轮补丁后仍属轮询/节流范式）。编辑器 = 主窗 WebView2 内 DOM，OS 移窗天然像素级跟随。

**决策**：

1. **预览宿主页改由主窗内跨源沙箱 iframe 承载（路线 A）**：主窗 DOM → `<iframe sandbox="allow-scripts" src="http://slterm-preview.localhost/preview-host.html">`（宿主 iframe，opaque origin）→ 宿主页内嵌 `<iframe sandbox="allow-scripts">` srcdoc = 内容文档。**安全模型不变**（ADR-0019 核心资产保留）：自定义协议域不变；sandbox 无 allow-same-origin（CVE-2024-35222 红线）不变；main_frame_only → 各层 iframe 内均无 tauri IPC 注入不变；域级 CSP meta（SEC-02 指令表）不变；nonce + 类型白名单校验链不变。独立窗口只是载体，换载体不回退威胁模型。
2. **消息桥 = 纯 window.postMessage（三层通道）**：独立窗口时代的 Tauri event/IPC 桥（preview_render/preview_pull/preview_sync/preview_close 四命令 + CONTENT_STORE + SESSION_STATE 会话守卫 + 几何换算 + run_on_main）整体删除——宿主页与主窗同窗口树，postMessage 天然可达。通道分层（previewMessages.ts 单点登记）：文档层（内容 iframe ↔ 主窗，经宿主桥 relay）：上行 {zoom, scroll, nav, font_probe, keyfwd}，下行 {reset, zoom_set, scroll_set}；宿主层（桥生命周期信号）：上行 {host_ready, iframe_loaded}，下行 {host_content}。origin 实证（2026-09-12 spike）：opaque origin 序列化 = "null"——主窗→宿主 targetOrigin 只能 "*"；校验 = `event.source === iframe.contentWindow` + origin "null"（下行侧宿主桥 isMain = source 归属 + 主窗 origin 白名单 tauri.localhost / localhost:1420 双闸）。
3. **键盘语义 = 收窄转发（D2）**：内容 iframe keydown（焦点在 input/textarea/select/contenteditable 时不转发）经 slterm_keyfwd 上行（载荷 = code + 四修饰键，不含 key）→ 主窗合成 KeyboardEvent 经 ShortcutRegistry `resolve(ev, "global")` 解析消费——预览聚焦时全局快捷键（Ctrl+W 等）仍可用；表单键入/系统复制快捷键解禁。不做旧 slterm_key 的命令重放语义复活。**威胁面登记**：nonce 明文内联于注入脚本 → 内容脚本可提取伪造 keyfwd → 触发主窗 global 命令；危害边界 = global 命令集（当前仅 global.closeTab，command-catalog.test.ts 锁死）——global 集扩充须重估本面（previewMessages.ts/buildInjectedScript.ts 注释登记）。
4. **主窗 CSP 新增 `frame-src http://slterm-preview.localhost`（D3，收窄式新增）**：script-src 'self' 终态（CP-012）不动；csp-config.test.ts 锁死 frame-src 恰为该单值。
5. **E2E 驱动契约 = 探针模式（spike Q4 裁决）**：embedded driver frame 内 execute 全灭（同源 about:blank 对照帧同样超时——driver 不接线任何非顶层上下文，与跨源/sandbox 无关）——switchToWindow/switchToFrame 契约整体退役；内容断言走「主窗 E2E 全局」探针（fontProbe 同款，PreviewFrame 经 E2E_ENABLED 门控写 `__slterm_e2e_previewDoc`（panelId → 最近推送产物）/ `__slterm_e2e_iframeLoaded`（panelId → 加载完成计数）——组合 = 旧「宿主页 srcdoc 属性」断言强度 + 真实加载佐证）。

**被否决的备选**：

- **路线 B（Rust 子类化主窗 WM_MOVING 同步位移预览窗）**：保留独立窗口形态，补丁范式极限——只能收窄延迟不能归零（WebView2 合成与 OS 移窗仍不同帧），且引入子类化/消息泵侵入面；否决（D1）。
- **路线 C（预览窗改主窗 WS_CHILD 子窗口）**：WebView2 子窗口与主窗 WebView2 合成冲突面未实证、tauri 无此配置面（须自管 Win32 句柄树）；成本/风险不对称；否决（D1）。
- **switchToFrame 两层切入做 E2E 内容断言**：spike Q4 实证 driver 不接线任何非顶层上下文（同源对照帧同灭）——不可行，改探针模式（决策 5）。

**后果**：

- 拖动/resize/跨屏 HiDPI 场景预览像素级跟随（结构性消除，无轮询/节流/IPC 链）；分屏切页显隐随主窗 DOM 天然正确；md split 预览即刻出现（旧「隐藏态不建窗 + 同步环」类缺陷形态整体消亡）。
- 前端：PreviewFrame 重写（宿主 iframe 渲染 + 消息分派 + 探针），ipc/preview.ts 删除，ipc/window.ts 主窗三事件订阅删除；几何编排/200ms 轮询/force-sync/token 会话守卫前端族消亡。
- Rust：preview.rs 收缩为 scheme 协议 + HOST_PAGE（窗口管理/内容存储/会话守卫/四命令全删）；lib.rs/build.rs/capabilities 清理（preview.json 整删）。
- E2E：specUtils 预览驱动族改探针模式；html.e2e/markdown.e2e 改写（窗口生命周期用例 → 宿主 iframe DOM 生命周期；「主窗移动跟随」用例结构性消除删除）；keyfwd 链路新增正/负两用例。
- **逆转触发点**：预览内容需超出面板矩形（如画中画浮窗）时重估载体（主窗 DOM 无法出窗）；driver 出现帧级寻址能力时 E2E 可回 switchToFrame 直接断言（探针保留无妨）；global 命令集扩充时重估 keyfwd 威胁面（决策 3）。

## 0022 DA1 后端全量接管 + 恢复注入就绪闸门

**Status**: accepted（2026-09-18。grill 轮次裁决：DA1 接管全平台统一 / cmd 恢复注入固定 500ms 兜底 / 验证 = 本机自动化 + Win10 实机人工验收）

**上下文**：Win10（22H2，pwsh 7.6.5）新建终端蜂鸣一声 + 行首出现可编辑 `[?1;2c`；双击历史 session 恢复时注入命令被拼成 `[?1;2cclaude resume xxx` 致恢复失败。Win11 正常。根因链：Win10 捆绑 OpenConsole 1.24 启动握手多发一个 DA1 查询（Win11 inbox conhost 不发）→ 后端启动剥离清单不含 DA1 → 查询透传前端 → xterm.js 核心自动应答 `\x1b[?1;2c` 经 onData 无差别回写 stdin（**作废旧假设「xterm 应答只留前端 DOM 不回灌 PTY」**，实测全量回写）→ PSReadLine 把 ESC 当无效键响铃、`[?1;2c` 落成可编辑文本并与恢复注入竞争 stdin。旧「DA1 模拟响应」（每会话一次 AtomicBool、仅 startup_drained 后检测）同时存在：启动窗口内全剥离块 `continue` 漏检、双重应答身份不一致（`?64;22c` vs `?1;2c`）。

**决策**：

1. **DA1 后端全量接管**：DA1 查询（`ESC[c`/`ESC[0c`）一律后端检测→剥离→代答 `ESC[?64;22c`，永不透传前端，全平台统一（Win11 上 xterm 不再自答，应答身份唯一）。检测前移至剥离前原始字节；解除每会话一次限制（谁问谁得答）；块尾 DA1 前缀扣留 pending 防跨块泄漏。DA2/XTVERSION 同族不接管（未观测受害场景，登记再议）。
2. **恢复注入就绪闸门**：恢复命令在首个提示符渲染完成（OSC 133;A → `TerminalRegistry.promptReady`）之后注入；cmd 无 shell integration → 固定 500ms 延迟兜底；超时（10s）兜底仍注入不劣于现状。pty_spawn 返回扩展为 `SpawnResponse { sessionId, shellKind }` 供闸门分派。（原「输出静默 ≥100ms 沉淀窗」设计已经 Win10 实测证伪撤销——「渲染窗口吞首字节」假设不成立，`lastOutputAt`/`noteOutput` 打点整族删除。）
3. **DSR 按需代答 + 窗口外 OS 门控剥离**：禁止盲注 CPR（原 spawn 盲注 `\x1b[1;1R`——Win10 捆绑 OpenConsole 1.24 握手发 DA1 不发 DSR，盲注 CPR 无人消费，被 conhost 输入状态机解析为 F3 键 → PSReadLine CharacterSearch 吞掉下一个输入字符，实证：Win10 新终端敲 `abc` 显示 `bc`）。处置 = `reader_loop` 启动窗口内检测到 `ESC[6n` 才代答（`should_answer_dsr`）；窗口外 DSR 改 OS 门控剥离不答——`strip_dsr = conhost_input_corrupts_cpr(build)`（阈值 21376，覆盖 Win10 捆绑与回退全路径），Win11+ 维持透传 xterm 实答（主用例零回归）。**关键约束（决定方案形态）：Win10 键事件输入模式下任何 CPR 应答字节写入 stdin 都是毒**（`1;1R`→F3，其他位置形态被键事件引擎丢弃）——「后端代答 CPR」在此类主机上不可行，DSR 只能剥离不答；剥离严格不劣于现状（发起方拿到的本就是 F3 垃圾；DA1 代答字节在 Win10 实质丢失而应用正常为旁证）。
4. **原则沉淀**：握手应答一律按需（问什么答什么），禁止盲注——盲注字节在没有等待者时就是注入应用输入的杂散键；查询处置按传输层能力分叉，「应答会被传输层损坏的查询一律后端接管或剥离」，不透传前端。

**被否决的备选**：

- **修捆绑 conhost 回退（原方案 c）**：诊断证伪前提（该机器 bundled=true，无回退）——撤销。
- **仅 Win10 平台启用接管**：应答身份双轨残留，Win11 仍依赖 xterm 自答时序；全平台统一更简洁。
- **前端 onData 过滤应答**：治标——xterm 仍自答，且其他注入路径（恢复/粘贴）竞争不消除。
- **窗口外 DSR 留 xterm 实答（修订一短暂采用）**：与决策 1 铲除的 DA1 自答回灌完全同构——窗口外 `ESC[6n` 透传前端 → xterm 自动应答 `ESC[{y};{x}R`（渲染期光标 1;1 → 字面 `ESC[1;1R`）→ onData 无过滤回灌 stdin → 同一 F3 吞键链（Win10 实机探针证实）；修订二推翻，改 OS 门控剥离（决策 3）。

**后果**：

- Win10 蜂鸣/`[?1;2c` 污染/恢复失败三症状结构性消除；Win11 行为对齐（DA1 代答统一后端）。
- `pty_spawn` 返回值 `string → SpawnResponse` 为 breaking change（允许，无兼容过渡）；全部 spawn 调用点与测试 mock 同步适配。
- 前端 xterm.js DA1 自答通道失去触发源（查询不再到达）——xterm 升级改变自答行为对本项目无影响面。

**沿革**：2026-09-18 修订一（沉淀窗撤销 + DSR 按需代答）；2026-09-19 修订二（窗口外 DSR 改 OS 门控剥离不答）——两次均为 Win10 验收驳回后定位。2026-09-26 文档收敛折叠入决策 2/3/4 与被否决备选。

## 0023 设置中心组模型改全局/Agent 二分

**Status**: accepted（2026-09-26）

**上下文**：设置中心旧组模型含「项目」组，承载 CLI 别名页与 Hooks 配置 hub 页（hub = CLI 选择行 + 子编辑器容器）。CLI 增多后「项目」组语义失真——别名/hooks 是 agent 维度的配置而非项目维度；hub 选择行在只有两个子页时纯属多余层级。

**决策**：

1. **组模型二分**：`SettingsPageGroup = "global" | "agent"`，删「项目」组；组序 global→agent 固定。
2. **Agent 组按 CLI 分节**：agent 行 = 不可点分节标题（logo + displayName，div 非按钮）+ 缩进子页；页 id 形态 `agent.<cliId>.<page>`。
3. **hooks 页迁入直渲染**：`agent.<cliId>.hooks` 直渲染 profile `configEditor`，删 hub 与 CLI 选择行；页注册经 `capabilities.hooks.hasConfigEditor` 门控（无编辑器能力的 agent 无 hooks 页）。
4. **CLI 别名迁入基础配置页**：`agent.<cliId>.basic` = 「侧栏展示内容」节 +「CLI 别名」节（CliAliasSection 自 CliAliasesPage 抽单 cliId 形态）；别名/ hooks 两旧页删除。
5. **动态注册 profile 补注册契约**：agent 页枚举抽为幂等导出函数 `syncAgentPagesFromProfiles()`（同 id register 幂等覆盖），模块顶层调用一次保 side-effect 语义；E2E 等运行时动态注册 profile 的场景须手动调用补注册（helpers.ts 先例）。

**被否决的备选**：

- **保留 hub 容器 + CLI 选择行**：两级导航重复（设置中心左侧导航本身就是选择器），选择行是纯冗余。
- **agent 分节标题可点（兼作 agent 首页）**：与「分节标题」语义混淆；无内容可放。
- **保留「项目」组**：组语义失真（组内页与项目无关联）。

**后果**：

- 页 id breaking change（`cliAliases`/`hooks` → `agent.<cliId>.basic`/`agent.<cliId>.hooks`，允许，无兼容过渡）；E2E 与 L2 测试同步适配。
- 无 agent 注册时 Agent 组不渲染；selectedPage 失效回退全局首组首页（既有逻辑自然覆盖）。
- claude profile displayName "claude" → "Claude Code"（tabTitle 保持 "claude" 不变——终端页签窄宽度场景）。

## 0024 agent 目录沙箱白名单 + pinned watcher

**Status**: accepted（2026-09-26）

**上下文**：「Agent 全局文件」侧栏视图需读取 `~/.claude`（settings.json 等全局配置文件）。三个结构性障碍：① 路径沙箱 `validate_path_within_root` 仅放行 project_root，agent 目录恒在根外被拒，而编辑器/预览/「在终端打开」链路全部经沙箱命令；② watcher 池 `notify_watch` 内嵌 `pause_all_except`——agent 监听启动会暂停项目 watcher（explorer/commit/编辑器事件全停），项目切换反向暂停 agent watcher，两侧互踩；③ `~/.claude` 在 dotfiles 管理场景下可能是 symlink，字符串前缀比较不可靠。

**决策**：

1. **agent 目录 = 后端静态表唯一真值源**：`src-tauri/src/agent_dirs.rs` 持 cliId → home 相对目录硬编码表（当前仅 claude → `.claude`），`resolve_agent_dirs()` 经 `crate::home::home_dir()` 解析；DTO `AgentGlobalDir { cli_id, path, exists }` 经 ts-rs 生成，`agent_dirs_list` 命令下发。前端永不能注入任意 agent 路径。
2. **沙箱放行域扩展**：`validate_path_within_root` 放行域 = project_root ∪ agent 目录集，canonicalize 双向比较（兼容 symlink）。fs/git/notify/pty 全族命令自动获得 agent 目录访问——编辑器/预览链路零改造，不为 agent 文件建第二套读取通道。
3. **pinned watcher**：`notify_watch(path, pinned?)` 置条目标记——pinned 条目被 `pause_all_except` 跳过（照 touch last_used：项目切换不暂停 agent 监听，agent 监听启动也不暂停项目 watcher，两侧皆免），`evict_lru` 避让（全 pinned 退化全池 LRU）；`notify_stop_watch` 语义不变（移除即清标记）。池容量 8 足够（1 项目 + N agent）。
4. **`agentGlobalFiles` settings 段不设 Rust DTO**：纯透传段（SEC-11 白名单 +1），校验/净化在前端 `features/agentFiles/filtering.ts`——照 ADR-0014 cliAliases 先例：前端消费型配置段 Rust 复刻校验即双源漂移。
5. **树交互共享经 FileTreeExplorer 抽取**：ExplorerPanel 全部树交互抽为 `FileTreeExplorer`（rootPath/rootFilter/eventPathFilter 等入参），ExplorerPanel 收敛薄壳、AgentFilesPanel 复用（红线：explorer 行为零回归）；`useFileTree` 加 `rootFilter`（根层三点统一应用：loadRoot 首帧+续页/loadDirectory 当 dir===root/refreshSubtreeAt 当 target===root）/`eventPathFilter`/内置根前缀过滤（root 外 fs-event 跳过刷新——对 explorer 属严格改进）。

**被否决的备选**：

- **agent 文件走独立 IPC 域/读取通道**：编辑器/预览/终端打开链路要全部分叉，沙箱语义双源——否决，扩展沙箱放行域一处收口。
- **agent watcher 独立池**：池语义重复实现；pinned 标记在既有池内解决互踩更简洁。
- **前端经 IPC 传 agent 目录表**：注入面（前端可声明任意目录进沙箱白名单）——否决，表在后端硬编码。

**后果**：

- 沙箱放行面扩大至 agent 目录集（静态表白名单，无新增注入面）；`project_root=None` 时 agent 目录仍放行（视图在无项目时可用）。
- 新增 agent = agent_dirs.rs 表内追加一行 + profile 声明 `capabilities.globalFiles`（configDir/runtimePaths）。
- `notify_watch` 签名加可选参数为 breaking change（允许，无兼容过渡）。
