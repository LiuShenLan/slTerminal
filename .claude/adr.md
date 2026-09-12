# ADR

## 0001 活动栏 + 共享侧栏区（侧栏视图单槽位状态机）

**Status**: accepted（2026-07-18）

**上下文**：原布局为 Allotment 常驻三栏（项目列表 250px + 文件浏览器 250px + 主区），两栏均不可关闭，挤压主区宽度。需求是两者可关闭、可分区展示，且未来会新增更多同类视图。

**决策**：重构为「活动栏 + 共享侧栏区」——最左新增常驻窄条**活动栏**（~40px，不可关闭），项目列表 📋、文件浏览器 📁 变为**侧栏视图**，经活动栏按钮开关；两视图共享同一**侧栏区**（Allotment 拖宽 160–500 保留），侧栏区垂直分**上区/下区**（Allotment 垂直嵌套，分隔条比例可调），按钮经拖拽归属半区。核心状态机：**每半区最多一个打开的视图（单槽位），无"打开未展示"中间态，无历史记忆**——展示是 `f(按钮归属, 各区打开的视图)` 的纯推导。

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
- 默认态：两按钮均在上区，项目列表打开、文件浏览器关闭。
- 按钮样式：Emoji 图标（项目列表 📋 / 文件浏览器 📁）+ VS Code 风格 active 指示（高亮 + 左侧指示条），配色走 `theme/colors.ts` token（硬约束 #6）。
- 持久化（`~/.slterminal/settings.json` 新增 `sideBar` 段，复用 settings 浅合并，零后端改动）：按钮归属+区内顺序、侧栏区宽度、上下分割比例、各区打开的视图；重启恢复现场。

**被否决的备选**：

- **"打开未展示"态 + 历史记忆**（关闭当前视图后自动回到前一个打开的视图）：用户明确要求"仅考虑当前状态，不要历史记忆"；且多一层状态，与全部给定示例不符（规则 3：同区两点后关闭 → 整体隐藏而非回到前者）。
- **拖拽不跟随**（视图展示位置可与按钮归属临时分离，下次点击才生效）：两个事实脱节，需额外记录"视图实际所在半区"，实现与心智成本都高，易出 bug。
- **关闭即卸载组件**：文件浏览器展开目录、滚动位置等状态丢失；隐藏不卸载与 H6 多实例显隐模式一致，代价仅少量内存。

**后果**：

- 新增 `SideViewRegistry` 模块级单例（同项目现有 panelRegistry / TabTitleRegistry / FileViewerRegistry / ShortcutRegistry 模式）：新增侧栏视图 = 实现组件 + `register({ id, title, icon, component })`，活动栏按钮、侧栏区展示、持久化全部自动生效，无需改动框架代码。
- 状态最少（按钮归属 + 每区打开的视图 id），所有展示由状态纯推导，无 UI 临时态。
- 原 `SidebarTree`、`ExplorerPanel` 组件本体不变，仅宿主从 Allotment 常驻栏变为侧栏区视图槽。
- **换区重建（已确认接受）**：拖拽按钮跨区（上↔下）时，视图组件从上区 pane 移入下区 pane——React 视为不同父节点，触发卸载+重建，组件内部状态（如 explorer 展开状态、rootNodes）丢失。权衡：换区为低频操作（用户通常设定一次后不改），重建成本低于跨父节点保持实例的架构复杂度。

## 0002 配色方案单点（schemes/ + 注册表 + facade + 重载切换）

**Status**: accepted（2026-08-07）

**上下文**：颜色散落在 6+ 条独立色源（colors.ts 32 token、xterm 22 色、oneDark、dockview --dv-*、Allotment、App.css --sl-fg-*、库默认色），改色多点人工同步且有漏网点（数值重复不联动、死 token、1 处真违规硬编码）。需求：全部应用可控色源收拢单点 + 每色注释消费位置 + 为新增方案与一键切换留扩展接口。

**决策**：
- src/theme/ 下设 schemes/（ColorScheme 四段：ui/terminal/editor/libraries + darcula 内置方案）+ SchemeRegistry 模块级单例（项目注册表惯例第 6 例）。
- colors.ts 改 facade：31 个同名导出值代理 active 方案，369 处消费点零改动。
- 方案切换 = settings.json `colorScheme` 段手编 + 重载窗口生效；main.tsx 启动时 React 挂载前解析（App 改动态 import 保证 facade 求值晚于 setActive）。
- 三方库色经 overrides 通道：dockview --dv-* 与 Allotment 变量内联注入挂载点；oneDark 作 editor.theme 引用 + lint/search/背景 token 化覆盖。
- 零视觉变化：所有覆盖值取现行有效值；死配置全清（3 死 token + 2 零消费 CSS 变量 + 1 违规收敛）。

**被否决的备选**：
- 运行期即时切换（token 全面响应式）：369 处常量消费 + xterm/CM 创建期消费需全量改造，代价 vs 暗色系低频切换收益不成比例。
- oneDark 完全 token 化自绘语法色板：每方案需 10+ 语法色定义，工作量与审美风险大；editor 段引用已预留未来自定义。
- 启动链 fail-safe 收编(运行期通道,被否决):index.html/tauri.conf.json 为静态层无法用 TS token。**2026-09 CP-027 修订**:构建期通道成立——`scripts/sync-startup-colors.mjs` 从 linear.ts 提取改写三处消费点,运行期仍不经 facade。

**后果**：
- 新增方案 = schemes/ 新文件 + register 一行，消费方/测试守卫零改动。
- 注释单点在 types.ts 接口槽位（消费位置与方案无关），新方案零注释负担。
- main.tsx 静态 import 图收敛为 react/react-dom/lib/e2eEnabled/theme/startupColors(零依赖常量模块,不触发 facade 求值)；E2E helpers 与 ROOT_CSS_VARS 注入保持原相对顺序。
- **2026-09 CP-039 修订（编辑器侧消除常量化）**：`editorTheme` 模块级常量改 `getEditorTheme()` 函数形，消费点改 `editorThemeSlot`（Compartment + `schemeRegistry.onDidChange` 订阅热重配置）——CM 主题随方案切换即时生效、编辑器不重建（文档/光标/undo 保留）。运行期整体即时切换（壳层 token 全面响应式）仍否决；editorTheme 常量化这一系统性后果已消除，为将来运行期切换移除最后一块编辑器侧障碍。

## 0003 UI 全面重设计（Linear 极黑克制）

**Status**: accepted（2026-08-16）

**上下文**：现状 UI（darcula 时代）被评价为「古板呆滞」——中灰蓝底平铺、实色粗边框、盒式页签、装饰 emoji、侧栏多区块堆叠。用户调研 11 款软件暗黑 UI 后要求整套重设计：仅暗黑、JetBrains Mono 全局、无动效、不做多主题切换。只交付设计产物（设计方案 + 需求规格 + 视觉稿），不含实现。

**决策**：经 13 题澄清（风格锚点/底色温度/强调色/标题栏/图标/页签/密度/IA/内容色/交付流程/规格粒度）+ 6 个候选视觉稿（3 骨架内 + 3 突破型）浏览器实测对比，选定**候选 A「Linear 极黑克制」**，无微调：

- 风格锚点 Linear/Zed 近纯黑现代风；暖黑**明度阶梯** 6 档（`#0a0a0b`→`#2b2b31`），内容区最暗；分隔一律**发丝线**（半透明白 0.055/0.09）。
- 单强调色现代蓝 `#6e9ff2`（<5% 像素占比）；语义色低饱和暖协调；终端 ANSI 16 色与编辑器语法色全量重调（**双轨配色**：壳层 token 与内容色板互不混用）。
- 自绘 34px 一体化标题栏（原生标题栏退役）；扁平页签 + 底部 2px 指示条 + hover 才显关闭钮。
- 侧栏 IA 重构为**统一导航树**（项目 → 页面 → 会话，历史会话折叠计数）；文件浏览器独立为活动栏视图；活动栏固定三槽（导航树/文件/Commit）。
- 装饰图标全部单色线性 SVG（15px/1.5px 描边/currentColor）；状态 emoji → **状态圆点**（绿/黄/灰，语义来源 F3 不变）；CLI 品牌 logo 保留彩色。
- 交付物：`design.md`（设计方案）、`requirements.md`（UI-xxx 编号需求 + 可测验收 + P0/P1 + 对比度自检附录）、`final-mockup.html`（主界面 + 组件集双页静态稿）。

**与 theme 系统对接**（后续实现期）：全部色值经 ADR-0002 配色方案单点落位——新增 scheme 文件替换 darcula 内置方案，需求规格每条色值标注 types.ts 槽位；启动链 fail-safe 静态色经 sync 脚本从 linear.ts 构建期注入(CP-027)；多主题切换机制不建（硬约束）。

**被否决的备选**：
- 候选 B（Zed 实体边框感）/ C（最暖+大圆角）：骨架内变体，层级靠边框/温度而非纯明度差，不如 A 的「界面消失」感。
- 候选 D（紫罗兰强调色+冷黑底）/ E（One Dark 盒式页签）/ F（teal+胶囊页签）：突破型对照组，验证已定决策（现代蓝/暖黑/扁平页签）成立，无需突破。
- 多 UI 模式/亮色系：用户硬约束，明确排除。

**后果**：
- 实现期按 requirements.md 逐条验收（P0 必做）；视觉验收参照 final-mockup.html 组件集页。
- F3 四态 emoji 的**视觉呈现**被状态圆点取代（事件→状态映射逻辑不变）；F9 品牌 logo 保留。
- 现状侧栏（项目列表 + Agent Status 等区块）IA 将随实现重构为统一导航树；行为逻辑（快捷键/右键菜单/会话恢复）不变。

**实现期决策（2026-08）**：Stage 01-08 实施（checklist.md 46 项）期间追加确认的决策，逐条落地于代码：

- **UI-405/406/407 三条剔除**（Agent 面板/composer/状态行）：为远期新功能而非视觉重设计，不属本期范围；2026-08-16 定调——聊天式 Agent 面板为独立产品方向，未来单独立项。requirements.md 三条已补「远期愿景，本期不实施」注记（DOC-04），编号保留。
- **darcula 删除、linear 替换**：新方案 `linear` 替换并删除 darcula（schemes/ 只余 linear.ts）；启动链默认 id 改 `linear`，未知 id 回退机制内建（schemeRegistry.setActive）。
- **配置钮入口唯一化**：活动栏底部「配置」钮 = 打开 hooks 配置面板（唯一入口）；SidebarTree 右键菜单「打开 Hooks 配置」项随 SidebarTree 退役删除。
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

**Status**: accepted（2026-08-18，TE-11）

**上下文**：59 个 npm 依赖版本策略不一致（8 精确 + 51 `^`），生产运行时依赖浮动升级不可控——终端核心路径（xterm 等）的静默 minor/beta 浮动曾引入回归（调查5）。需求：统一版本声明策略，生产与开发工具分流。

**决策**：`package.json` 双区版本策略——

- **dependencies（生产运行时）全精确版本**（无 `^`，锁死当前解析版本）：浮动的任何升级都必须显式改 package.json，进入评审流程。
- **devDependencies（开发工具）全 `^`**：开发工具升级风险低、频次高，允许 minor 浮动。
- **overrides 段保持现状**（`^`），不随本策略调整。成因与『谁钉谁』对齐契约登记于 `e2e-tests/CLAUDE.md`(CP-032);上游放开硬钉后逐条去 overrides 化。
- 精确版本一律以 package-lock.json 当前解析版本为准（pin 不改解析版本本身，`npm install` 刷新 lock）。

**后果**：

- 升级依赖 = 显式改 package.json 版本号 + 跑对应测试门禁，杜绝 `npm install` 静默升级。
- 例外登记：xterm 三件套 beta 保留（ADR-0007）与 notify RC 保持（ADR-0008）为 pre-release 场景既定例外，升级审批见各自条目；Cargo.toml Rust 侧沿用语义化版本区间（不属本策略范围）。

## 0007 xterm 三件套 beta 保留 + 升级审批约定

**Status**: accepted（2026-08-18，TE-03）

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

**Status**: accepted（2026-08-18，TE-04）

**上下文**：Rust 侧 `notify`（文件系统监听核心）与 `notify-debouncer-full` 为 RC 版本。一手证据（`src-tauri/Cargo.toml:36-37、48-49` 跟踪注释）：`notify` 9.0.0 仍为 RC 阶段——最新稳定版 8.2.0（2025-08-03），而 rc.4（2026-05-02）为当前**最新**版本，无更高稳定版可升；降 8.x 属功能回退。watcher 行为有 notify 模块 51 条 L1 回归测试守护。

**决策**：**保持 RC，不降 8.x**；沿用 Cargo.toml 跟踪注释关注 `https://crates.io/crates/notify` 正式发布。上游发布稳定版后升级，升级时跑 notify 模块全量 L1（51 条 watcher 回归）+ 大目录监听实测。

**后果**：

- `notify` 正式版发布即升级触发点（Cargo.toml 注释已登记），升级走常规依赖变更流程。
- RC 风险（API 变动/缺陷）由 51 条 L1 回归守护兜底，与 9.x 前瞻收益（后续版本能力）权衡后接受。

## 0009 review-fix 豁免与决策汇总登记（DOC-10）

**Status**: accepted（2026-08-18，DOC-10）

**上下文**：review 修复（93 项）多处以「登记豁免/保留现状 + 文档登记」关闭，登记点散落各模块文档。此处汇总 root 侧豁免决策，避免各登记点失散；模块内明细以对应模块 CLAUDE.md 为准。

**决策**：

| 标识 | 决策 | 登记点 |
|------|------|--------|
| FE-01 | Workspace 多 Dockview 实例**保持**（H6 终端跨页面存活 + xterm 实例限制，D1）；以页面总数上限 `MAX_PAGES = 20` 防内存/DOM 线性增长（FE-36 跨项目全局计数修订同列）。**已作废（CP-004/S11，2026-09-08）：多实例架构被共享宿主 + 页组模型取代**——单一 DockviewReact，每操作页面 = 宿主内顶级页组（pageGroups.ts 协议：组 id `page-{pageId}`、面板 id 页前缀 `{pageId}:localId`）；页面切换 = 页组容器显隐（dockview 叶可见性，终端不卸载，#4978 约束不变）；`MAX_PAGES` 与超限 toast 删除，上限随实例数线性增长源消亡。**再修订（ADR-0020，2026-09-12）**：「页 = 单组多页签」子约束被推翻——页内分屏合法化，组页归属改派生模型（组 id 快车道 ?? 组内首面板前缀），可见性机制 maximize → setVisible 逐组 | src/workspace/CLAUDE.md、src/stores/CLAUDE.md、src/workspace/pageGroups.ts |
| SEC-09 | CSP `script-src 'unsafe-inline'` **保留**（D4）：srcdoc iframe 继承父 CSP（W3C 行为），HTML 预览注入脚本（锚点拦截/键盘转发/nonce）必须内联，移除即破坏预览。现状 = tauri.conf.json `script-src 'self' 'unsafe-inline'` + `dangerousDisableAssetCspModification: ["script-src"]`。**已被 ADR-0019 取代**（2026-09-08，S10-②：预览迁独立 webview 自定义协议域，主窗口回收 script-src 'unsafe-inline' 与 dangerousDisableAssetCspModification——CP-012） | src-tauri/tauri.conf.json 注释 |
| SEC-06 | 剪贴板读权限 `clipboard-manager:allow-read-text` **保留**（D6）：唯一消费点为 keyboard.ts 的 Ctrl+Shift+V 显式手势，改后端命令不缩小攻击面（前端上下文被注入时同样能 invoke）；grep 级守卫测试锁消费点集合 | src/ipc/CLAUDE.md |
| BE-21 | `fs_read_dir` 返回整目录列表**不分页**（登记豁免）~~已作废~~：**CP-006 已改游标分页（2026-09）**——`(path, cursor?, limit?)` 默认 500/上限 1000，过滤排序后切片、游标 opaque，前端续页拼接；FileTree 虚拟化（FE-30）渲染侧保留 | src-tauri/src/fs/CLAUDE.md |
| FE-31 | CodeMirror 大文件**不虚拟化**（按 D3 关闭）：fs_read_file Channel 分块（BE-03）削峰 + 10MB 上限 + 1MB 警告已覆盖峰值；CM6 文档模型不支持部分加载 | src/panels/editor/CLAUDE.md |
| 09#14 | 后端 Mutex **已换装 parking_lot**（CP-005，2026-09）：中毒攻击面结构性消除，原「保持现状」登记作废 | src-tauri/src/CLAUDE.md |
| TE-03 | xterm 三件套 beta 保留 + 升级审批门禁（L3 + E2E + 真实 claude 实机滚轮 + Win10 21376 阈值核对） | ADR-0007（本文件） |
| TE-04 | notify 9.0.0-rc.4 / notify-debouncer-full 0.8.0-rc.2 保持（rc.4 即最新，无稳定版可升；51 条 L1 watcher 回归守护） | ADR-0008（本文件） |

**后果**：

- 新增豁免须先在本表或对应模块 CLAUDE.md 登记再关闭，禁止只改代码不留档。
- 行为固化点：`MAX_PAGES`（L2 测试断言）、CSP（tauri.conf.json）、剪贴板守卫测试、`GIT_REPO_CACHE_CAPACITY`/`WATCHER_POOL_CAPACITY`/`MAX_PTY_SESSIONS`（L1 契约测试）。

## 0010 review-phase2-fix 决策与债务登记（D12~D20、TE-07、TE-15）

**Status**: accepted（2026-08-22，S10-C）

**上下文**：review-phase2-fix 清单第 0 节决策表 D12~D20（续 review-fix D1~D11，编号规则：未闭环/partial 沿用原 ID，新发现续编 SEC-15~17/BE-22~25/FE-36~48/TE-14~16/DOC-11~14）。此处汇总 root 侧决策、TE-07 妥协结论与 TE-15 工程债务，模块内明细以对应模块 CLAUDE.md 为准。

**决策（D12~D20）**：

| 编号 | 决策点 | 结论 |
|------|--------|------|
| D12 | 修复范围 | 全量修复：P0+P1+P2+未闭环 10 项+fmt 基线，去重合并后 **37 项**（含 FE-39 验证项） |
| D13 | TE-12 knip 门禁 | 方案 A：补 `entry`/`ignoreExports`/`ignoreFiles` 至 `npx knip --production` 退出码 0；不窄化 CI 口径 |
| D14 | TE-07 TS7 声明失真 | 主 `typescript` 字段直改 `^7.0.2`，删 `@typescript/native` 别名与 TS6 包装器；执行前 `npm view typescript-eslint` 实查兼容版，不兼容则升级/overrides 统一或暂停 type-aware 规则并 ADR 登记（**执行结果见下节：三支 fallback 全走尽，妥协为双 TS 并存**） |
| D15 | SEC-15 shell fallback | 收窄为「两侧 canonicalize 均失败且归一化字符串完全相同」才放行，单侧失败即拒绝；`pty/CLAUDE.md` 登记残余风险；补 L1 拒绝用例。不引入 Win32 文件身份比对。**alias 兼容保持**（Store 版 pwsh 场景两侧指向同一路径、双侧均失败，仍走 fallback 放行）。D15 残余风险已销(2026-09):字符串回退改 Win32 句柄级文件身份比对,SEC-15 单侧拒绝保留为纵深 |
| D16 | SEC-04 nonce | 威胁模型登记（HtmlPanel 顶部注释 + `src/panels/CLAUDE.md` 修正失实描述）+ L2 守卫测试锁死 global context 命令集；不加 UI 提示、不移除 nonce。**威胁模型已消除，ADR-0019**（2026-09-08，S10-②：键转发/命令重放通道随 webview 迁移退役——上行终态 = 渲染态集合；nonce 保留为纵深） |
| D17 | SEC-16 root 竞态 | 后端 `tokio::sync::Mutex` 串行化整个 `set_project_root_impl`（Cargo.toml tokio 补 `"sync"` feature）；前端零改动 |
| D18 | FE-37 store IPC | `setProjectRoot` 调用上提调用方（store 纯状态化）；toast 由 `switchToPageShared` 承担（BE-23 同链修）；不登记豁免 |
| D19 | FE-39 嵌套项目 | 接受「最深前缀」语义；实查测试已固化（`nav-tree-history.test.tsx:302-336`），零代码改动，仅 verify 断言确认存在 |
| D20 | FE-40/FE-41/FE-46 | 三项 P2 均实修（滚动跟随 / 空目录行移除 / ErrorBoundary 重试） |

**核验留痕（计划期已实读全部修复点代码原文）**：FE-39 经实查 `nav-tree-history.test.tsx:302-336` 已含嵌套最深前缀用例（Phase 2 04 报告此项失实）——降为「验证已固化，零改动」。FE-45 实查为 **5 处** catch{}（05 报告列 3 处，projects.ts 有 2 处：:254 与 :275）。

**TE-07 执行结果（S02 妥协背书）**：主 typescript 直改 ^7.0.2 **不可行**，D14 三支 fallback 实测走尽（typescript-eslint 最新 8.67.0 peerDependencies `typescript: '>=4.8.4 <6.1.0'` 全系拒绝 TS7、且模块加载期硬校验 `ts.versionMajorMinor >= 7` 崩在加载期，与 type-aware 规则开关无关；overrides 钉兼容组合与根依赖 `^7.0.2` 冲突不可行）。**正式化妥协：双 TS 并存（side-by-side）**——`"typescript": "npm:@typescript/typescript6@^6.0.2"`（TS6 包装器，供 typescript-eslint 8.67.0 消费）+ `"@typescript/native": "npm:typescript@^7.0.2"`（tsc bin = TS7，`npx tsc --version` 7.0.2）。该形态全门禁绿。**升级触发条件(机检,`scripts/check-ts7-trigger.mjs`,CP-001 登记硬化)**:`node scripts/check-ts7-trigger.mjs` 退出码 0 即双条件达成——① issue #10940 `state=closed` ② `typescript` dist-tags.latest = 7.1.x 稳定版;退出码 1 = 未达成,退出码 2 = 查询失败(未知态)。触发后删 TS6 包装器与 `@typescript/native` 别名,`"typescript"` 直改 `^7.1.0`。

**TE-15 工程债务（已知债务登记，代码零改动）**：json-schema-library 9.x/11.x 双 major 并存——codemirror-json-schema@0.8.1 锁 9.x（上游约束），主声明 11.6.2；运行时两实例并存无冲突（JSON Schema 校验各自独立），待上游升级消解（TE-15）。**消解记录（CP-002，2026-09-08）**：codemirror-json-schema 已摘除，自绘 lint/hover 层（jsonSchemaCm.ts）直消费 11.x 编译单例，双 major 并存消亡。

**FE-31 登记点确认**：ADR-0009 表 FE-31 行登记点链接已指向 `src/panels/editor/CLAUDE.md`（新建文件存在，编辑器专属决策已迁入，S10 核对通过）。**FE-36 语义修订**已顺带补入 ADR-0009 表 FE-01 行（MAX_PAGES 跨项目全局计数）。

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

**Status**: accepted（2026-08-30，F11）

**上下文**：活动栏底部「配置」钮直达 hooks 配置面板（单一 hub），而配置项将增长为两类——全局级（套餐余量查询频率、快捷键等）与项目级（Hooks 配置等），「配置钮直达单一面板」的形态无法承载。前端需要统一的设置中心交互形态且新增配置页零框架改动；后端需要配置代码高内聚、低耦合、易扩展。

**决策**：
- **载体 = Dockview 面板**（面板类型 `settings`）：左导航固定 180px（全局/项目两组）+ 右侧配置页槽位，经 **SettingsPageRegistry**（硬约束 #13 注册表家族新成员，side-effect import 注册）分派渲染；页签标题固定「设置」。
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
- **`hooksConfig` 面板类型退役**：注册表与 PANEL_TYPES 移除，老布局由恢复白名单过滤静默丢弃（无迁移映射）；hub 改造为 HooksSettingsPage 迁入设置中心项目组，编辑器归域 `cliProfiles/profiles/claude/configEditor/`。
- **F10 豁免口径更新**：套餐余量轮询间隔从「启动时读一次」改为运行期可改（plan_balance 模块级 static 原子量 + 专用命令写入，每轮末按内存值 sleep），F10 相关豁免登记随之修订。
- 第一期三配置页：Hooks 配置（项目组，迁入）、套餐余量查询频率（全局组）、快捷键（全局组）。

## 0013 后台定时任务双端抽象（ADR-0013：任务元数据单点在后端 + 配置单写通道）

**Status**: accepted（2026-08-30，F12 规格期决策；规格文档已随实施完毕归档删除，实施细节以本 ADR 决策正文与代码为准）

**上下文**：套餐余量查询（F10 后端 tokio poller）之后新增第二个后台定时任务「session 历史刷新」。两任务执行位置本质不同——套餐余量需后端 OS/网络能力且快照预热语义在后端；session 刷新必须与手动刷新钮严格同一代码路径（前端 `scan(true)`），而两任务配置又需统一管理与展示。浅合并写通道（settings.rs 顶层键粒度）下两任务共用 `backgroundTasks` 一段会产生写覆盖冲突（前端写整体替换顶层键，丢对方子键）。

**决策**：

- **双端各自抽象，不设跨端统一调度器**：后端泛化 plan_balance 通用件（间隔内存原子量/每轮末 sleep/读盘初始化/set 命令）为任务骨架（静态切片注册表，照 `SOURCES`/`QUERIES` 先例）；前端新建 backgroundTasks 调度器（#13 注册表家族：全局单例、订阅者计数启停、首轮立即执行、tick 防重入、triggerNow）。
- **任务元数据单点 = 后端注册表**（含前端任务的代管：执行体字段 None 即前端任务，后端只管 id/标题/边界/默认值与配置读写）；设置页与前端调度器统一经 `background_tasks_list()` 读通道取数，前端不复制边界/默认值——DTO `BackgroundTaskInfo` 六键**无 default 字段**（FR-2 写死）的直接后果：行内提示只写范围不写默认值，默认值变更只动后端注册表。taskId 合法值集前后端同步测试锁死（HooksLayer ↔ `Layer` 枚举先例，硬约束 #4）。
- **配置单写通道 = 后端 `background_tasks_set_config` 命令**（taskId 子键读-改-写合并 → 复用 settings.rs 写通道），前端任务的配置也经此命令代管落盘——杜绝浅合并顶层键互覆；前端消费型 `save_settings` 段写不适用于本段。
- **配置结构**：统一顶层段 `backgroundTasks.{taskId}.{enabled,intervalSec}`；白名单 `planBalance` 键替换为 `backgroundTasks`（仍 5 键）；单用户不做旧键迁移。
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

## 0014 CLI 别名持久化走通用 save_settings 段透传（ADR-0014：校验全前端，Rust 白名单加键）

**Status**: accepted（2026-09-05，CLI 别名功能规格期决策；实施细节以代码与 aliasValidation.ts 为准）

**上下文**：CLI 别名（claude 等编码 CLI 的用户自定义启动命令名）需持久化到 settings.json。settings 模块既有两条路径：① 通用 `save_settings` 浅合并顶层段透传（fontSize/keybindings/sideBar 先例，Rust 仅白名单校验）；② 域模块专用命令（backgroundTasks 先例，Rust 端持有任务注册表故校验/合并/emit 收后端）。别名域的语法与 D3「全命名空间唯一」（不得撞任何 profile 内置命令或其它别名）判定需要「全部 CLI 内置命令名集合」——该知识只存在于前端 CliProfileRegistry（注册表 + profiles/* 静态声明），Rust 侧仅有 hooks/history 两个 cliId 键 provider 注册表（无 commands 概念）。若仿 backgroundTasks 走专用命令，Rust 必须复刻一份 CLI 内置命令名单——双源漂移：新增 CLI 需三处同步（前端 profile、hooks provider、history provider）变四处，违背注意 1 的高内聚要求。

**决策**：

- 别名配置存 settings.json `cliAliases` 段（cliId → 别名数组），Rust `SETTINGS_ALLOWED_KEYS` 白名单加键（5→6），内容**纯透传**：不设专用命令/DTO/emit，语法与唯一性校验全前端（cliProfiles 域纯函数 `aliasValidation.ts`），存储走通用 `save_settings` 段写 + 2s debounce（stores/cliAliases.ts，keybindings 模式同构）。
- 手改文件/版本残留产生违例数据 → 前端 loadFromDisk sanitize 兜底（孤儿 cliId 键丢弃、语法不过丢弃、撞内置/重复先到先占）；运行期磁盘改不改内存快照（与 keybindings 行为一致，接受）。
- 未消费方模型：别名运行时经 App 组合层注入注册表别名快照（ADR-0015），后端零感知。

**被否决的备选**：

- **Rust 专用命令校验**（backgroundTasks 形态）：Rust 无 CLI commands 知识源，需复刻注册表或至少内置命令名单 → 知识双源、新 CLI 四处注册，且唯一性跨 cli 判定的真值源（前端注册表）与校验执行地（Rust）分离会随时间漂移。
- **别名并入 profile.commands 静态字段**：见 ADR-0015（独立成案）。
- **后端校验 + 前端注册表推送名单**：为低价值校验域引入跨端同步协议，复杂化无收益。

**后果**：

- 新增 CLI 的步骤不变（前端 profile + 三处后端 provider），别名能力对新 CLI 自动适用（UI 注册表驱动分区）。
- 白名单 6 键需前端 store 与后端双侧测试锁死（save_accepts_cli_aliases_key + 段形态契约）。
- 逆转触发点：未来别名需要后端参与（如 shell 层展开/注入）或出现第二个前端外知识源时，重估专用命令方案。

## 0015 CLI 别名不进 profile.commands，注册表持独立别名快照（ADR-0015：matchByCommand 内置 → 别名回退）

**Status**: accepted（2026-09-05，CLI 别名功能规格期决策；实施以 cliProfileRegistry.ts 为准）

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

## 0016 E2E 假 home 隔离（ADR-0016：USERPROFILE 指向临时假屋，替代备份/还原）

**Status**: accepted（2026-09-05，事故后修复决策；实施以 run-wdio.cjs 与 src-tauri/src/home.rs 为准）

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

## 0017 预览容器信任模型延续：md/html 同态渲染（ADR-0017：sandbox 无 allow-same-origin + global 命令集不扩 + 宿主 script 静态化继承）

**Status**: accepted（2026-09-06，.md 文档面板需求期决策；实施以 docViewer 共享层与 markdown 渲染管线为准）

**上下文**：为 .md 新增预览能力时，渲染容器选型存在两条路：① 与 htmlviewer 同款的 sandbox iframe（opaque origin，注入桥 + postMessage 总线——SEC-03/04 校验体系、zoomRuntime/键转发/HUD 全现成）；② 宿主 DOM 直接注入（dangerouslySetInnerHTML，样式隔离自理）。容器选择连带决定信任模型：md 渲染产物含 raw HTML（用户 md 内嵌 <details>/<table>/<script> 等），htmlviewer 的既有语义是「本地文件全信任」（CSP 'unsafe-inline' + 无消毒，Tauri CVE-2024-35222 红线排除了 allow-same-origin）。跨 html/md/workspace/shortcuts 四模块、与 SEC-03/04 及既有 CVE 决策直接勾连，需要决策记录锚点。

**决策**：

- **md 预览与 html 渲染同态**：iframe sandbox="allow-scripts"（无 allow-same-origin，Tauri CVE-2024-35222）、注入桥（键转发/缩放/滚动/链接路由）与四层 postMessage 校验（origin="null" + source + nonce + type）收 docViewer/PreviewFrame 单点（复用不复制）；raw HTML 透传（markdown-it html:true），事件属性执行、宿主 `<script>` 因 escapeScriptClose 转义纪律与 htmlviewer 同态静态化——**行为继承即预期，不修复存量缺陷**。
- **global 命令集不因 md 扩充**：预览 iframe 键转发只重放 global context 命令（当前仅 global.closeTab，command-catalog.test.ts 锁死）——面板级命令在 iframe 内不可达，扩充须先重评 SEC-04 威胁模型（nonce 明文内联于 srcdoc，防外部伪造不防预览内容自身）。**global 重放通道已退役，ADR-0019**（2026-09-08，S10-②：键盘不跨窗口——预览窗口 focusable=false，键转发通道整体删除；global 命令集保持最小的守卫意图迁移为「预览消息通道不含命令重放」）。
- **链接分派收父侧**：linkRouter 段仅上行 href（slterm_nav），分类（external → 系统浏览器 opener / local → 应用内打开链路）在面板侧 linkPolicy 纯函数做——iframe 内不做任何打开决策。
- **缩放/滚动恢复语义**：keepZoom/keepScrollRatio 重建下行恢复（钳制/比例近似，登记已知行为），缩放状态不跨会话持久化（html 现状语义继承）。

**被否决的备选**：

- **宿主 DOM 直接注入 md 渲染产物**：与 htmlviewer 形成两套渲染面（沙箱/信任/键桥/缩放全复制或抽象重建）；宿主权限执行预览内容扩大信任面；相对资源/CSS 隔离/闪白需自研全套——安全红线登记（SEC-03/04）与威胁模型将分叉。
- **md 渲染做消毒管线（DOMPurify）**：用户需求确认信任边界 = 与 html 渲染同等（md 是用户本地文件，非第三方内容）；消毒引入「html 可显示 md 不可显示」的双轨行为与额外依赖面。

**后果**：

- 新预览型文件（未来 pdf/png 等）扩展路径 = 面板目录 + docViewer 注入段/回调组合，安全面不新开。
- iframe 内宿主 script 静态化为登记存量缺陷的继承行为（不另立缺陷单）；扩充 global 命令前必读本 ADR。
- 逆转触发点：出现预览不可信第三方内容需求（届时重新评估消毒/隔离）；或 escapeScriptClose 存量缺陷修复（宿主 script 恢复执行——需重审 md 侧是否同步放开）。

## 0018 本地资源通道：项目根只读 + data: 内联（ADR-0018：沙箱内二进制入渲染面首例）

**Status**: accepted（2026-09-06，.md 文档面板需求期决策；实施以 fs_read_resource 与 CSP 放行为准）

**上下文**：md/html 预览的本地相对资源（图片等）首次需要「按路径读任意二进制进渲染面」。既有 fs_read_file 只读 UTF-8 文本（编码校验）；iframe 为 opaque origin（无相对路径/asset 协议可达性，CSP font-src 无 data: 放行）。候选通道：① Tauri asset protocol（convertFileSrc）——静态 scope 无法跟随动态项目根，违背「仅项目根沙箱内」用户决策；② blob: URL——无生命周期管理点（iframe 重建即失效需 revoke，CSP 需放行 blob:）；③ 新后端命令 + data: URL 内联。全局 CSP 变更（首个 data: 放行）与「沙箱内二进制入渲染面」通道形态影响所有未来预览型面板，需决策记录锚点。

**决策**：

- **新命令 fs_read_resource（项目根只读通道）**：复用 extract_root 沙箱（root=None 拒绝）+ validate_path_within_root + 10MB 上限（复用 fs_read_file 常量）+ spawn_blocking；256KB 原字节分块 base64 Channel 推送（UTF-8 安全、削峰同 BE-03）；前端 readResourceBase64 聚合，MIME 推断在前端扩展名白名单（png/jpg/jpeg/gif/webp/avif/svg/bmp/ico）——后端保持「读字节」单一职责。三处注册（lib.rs/build.rs/capabilities）。
- **data: URL 内联（非 blob:）**：渲染产物替换为 data: URL——字符串可比（重建去重）、无 revoke 生命周期、CSP 面最小（img-src/font-src 追加 data:，不放行 blob:）；svg 经 `<img>` 惰性上下文加载（内嵌 script 不执行）；资源读取失败回退原 src（缺口不阻塞整篇）；LRU 缓存（50 项）防逐字重渲染反复读盘。
- **KaTeX 字体构建期内联**：katex.min.css woff2 url → data:font/woff2;base64（scripts/gen-katex-inline.mjs 产物提交入库 ~360KB，woff/ttf 回退剔除）；katex 升级重跑脚本 + git diff 审阅——运行时经 asset 协议取字体 CORS 行为未实证，不冒险。
- **渲染执行分层**：mermaid 宿主侧渲染（dynamic import + 按 code Promise 缓存）成 SVG 字符串注入——iframe 内零重排版、CSP 零新增、2MB 库不进主包；暗色主题变量映射 linear 内容色。

**被否决的备选**：

- **Tauri asset protocol + convertFileSrc**：assetProtocol scope 静态配置无法跟随 set_project_root 动态根（静态大 scope 违背最小沙箱）；字体/资源跨源 CORS 在 opaque origin 行为未实证。
- **blob: URL + 逐轮 revoke**：生命周期管理点缺失（iframe 重建竞态）、CSP 需放行 blob:、字符串不可比（无法跳过等值重建）。
- **mermaid 运行时注入 iframe**：2MB+ 源码进注入串违反无 `</script>` 字面量纪律（库内必然出现，需逃逸变换——html 宿主转义缺陷同坑）；300ms 防抖每轮重建全量初始化，成本为宿主渲染数量级倍数。

**后果**：

- 「沙箱内二进制入渲染面」有了统一通道（html/md 预览共用；html 相对图片顺带可用）；未来新资源类型走 MIME 白名单扩展。
- CSP 增两 data: 放行（csp-config.test.ts 守卫）；blob:/connect-src/worker-src 不放行；script-src 政策不变（'unsafe-inline' + nonce 注入关闭为 htmlviewer 既有前置，ADR-0017 继承）。
- 逆转触发点：动态 asset scope 出现（Tauri 支持跟随项目根时重估协议通道）；或需读 >10MB 资源/任意沙箱外路径（重估上限与边界）。
- **已回收（ADR-0019 终步，CP-035，S10-④）**：主窗口 img-src/font-src 的 data: 放行移除（csp-config.test.ts 锁终态）；KaTeX 字体经新预览上下文实证（③ CP-033）后在预览域（当时无 CSP——SEC-02 起为宿主页 meta 域级 CSP，见下落地复核注记）渲染；svg data: 显式禁用（markdown assets 白名单剔除 image/svg+xml——正文「svg 惰性上下文加载」论据随 data: 放行一并失效，处置见下「回收记录（CP-035）」节）。

**维持记录（CP-033，2026-09-08 S10-③ 新 webview 上下文重实证——B2 分支）**：

- **实证结论（数据通道在预览域真实可用）**：markdown.e2e 临时用例（真实 WebView2）渲染含行内 $x^2$ 与块级公式的 md——预览 webview 宿主页 srcdoc iframe 内宿主 `<script>`（CP-031 通道）读公式 DOM `getComputedStyle` font-family 命中 KaTeX 字体族、`document.fonts.check`（KaTeX_Main/Math/Size1）为真、字体集零 error 态 face，结果经 zoom 上行通道编码为主窗 HUD 121% 断言通过——构建期内联 data: 字体在新 webview 上下文真实加载渲染（非 serif 回退），原「opaque origin iframe 内行为未实证」缺口关闭。
- **asset 通道维持否决（不可行证据，两源）**：① 源码实证 tauri 2.11.5 `protocol/asset.rs`：asset 协议所有响应恒带 `Access-Control-Allow-Origin: <webview window_origin>`（manager/webview.rs 按各 webview 自身 URL origin 注册；预览窗口 = `http://slterm-preview.localhost`）——预览内容渲染于 sandbox srcdoc iframe（opaque origin，ADR-0019 决策二），跨源字体/资源请求 Origin 序列化为 null，与 ACAO 固定值不匹配 → 运行时经 asset 协议取字体的 CORS 校验在内容域不可过。② 实测：iframe 内 cors fetch `https://asset.localhost/index.html`（fetch 与字体同为 cors-mode，ACAO 匹配语义一致）→ 拒绝（HUD 110% 证据，2026-09-08）。
- **决策**：保留构建期内联产物（generated/katexInlineCss.ts）与 gen 脚本；CI diff 守卫（.github/workflows/ci.yml，CP-033）成为长期形态——katex 升级漏跑 gen 脚本即红。**S10-④ 执行口径登记**：预览域当前无局部 CSP（自定义协议响应无 CSP 头——ADR-0019）；④ CP-035 若建立预览域局部 CSP，font-src 必须放行 data:（每个渲染文档 head 内嵌 KaTeX data 字体串），主窗口 CSP 届时照 CP-035 回收。

**回收记录（CP-035，S10-④，2026-09-08）**：

- **前置闸判定**：③ B2 实证（KaTeX data 字体在预览域真实加载渲染 ×2 轮、asset 通道否决双证据）通过 → font-src 回收获实证许可——KaTeX 字体渲染只发生预览域（主窗口无 data: 字体消费），无需拆项另行登记。
- **主窗口 CSP 终态**：img-src `'self' asset: https://asset.localhost`（回收 data:）、font-src `'self'`（回收 data:）；style-src 'unsafe-inline' 保留（React inline style / CM6 注入样式，与本族无关）。csp-config.test.ts 三守卫锁死（img-src 恰好三项 / font-src 恰好 ['self'] / data: 不在主窗口任何指令）。
- **执行期发现并处置（img-src data: 的主窗口唯一图像消费点）**：CM6 lint 诊断波浪线——上游 @codemirror/lint baseTheme 与本仓 theme/overrides.ts 旧实现均以 `background-image: url(data:image/svg+xml,…)` 渲染（JsonMode 语法/schema 波浪线，主窗口渲染）——img-src data: 回收会静默遮蔽 lint 波浪线。处置 = 改 text-decoration wavy 技法（非资源 fetch，零 CSP 指令依赖）+ backgroundImage 显式 none 覆盖上游 baseTheme data: svg；色值仍单点于方案 lint 键（波形由 Chromium 绘制，与 6×3 tile 幅度略有差异，D1 已评估接受）。theme-overrides.test.ts 加「规则文本零 data: url」防回潮断言。
- **svg data: 显式禁用**：markdown assets.ts MIME 白名单剔除 image/svg+xml（本地 .svg 引用不再内联——缺口语义，与白名单外扩展同语义）；svg 载体可嵌脚本，预览域（当时无 CSP，同上注记——SEC-02 起为宿主页 meta 域级 CSP）内联风险面大，`<img>` 惰性上下文仅为 W3C 行为单点不作安全边界。html 侧无独立资源内联通道（仅 markdown 管线消费 assets.ts），同口径无代码落点。markdown-assets.test.ts 锁「svg MIME 不在白名单」。
- **预览域 data: 放行口径**：预览 webview CSP 无代码落点——预览域 = 自定义协议宿主页（响应无 CSP 头，host page 无 CSP meta），content iframe（srcdoc）无从继承 → data: img/font 在预览域天然放行（③ 实证通道即此）；「若未来建立预览域局部 CSP，img/font-src 须放行 data:」维持为执行口径（本 ADR-0018 与 ADR-0019 逆转触发点同文登记）。
- **落地复核注记（SEC-02，2026-09-09，原文保留不改写）**：上条「host page 无 CSP meta」已失实——宿主页 HOST_PAGE 现由 meta 承载域级 CSP（`default-src 'none'; script-src/style-src 'unsafe-inline'; img-src data:; font-src data:`），srcdoc iframe 继承宿主 CSP（W3C）→「若未来建立预览域局部 CSP，img/font-src 须放行 data:」触发条件已发生且两项已放行（交叉登记 ADR-0019 决策二 9）。
- **主窗口消费面审计结论**：除 lint 波浪线外主窗口无其它 data: 图像/字体消费者（lucide 内联 svg 元素非 fetch、cli-icons/字体走 self、vite assetsInlineLimit 无 <4KB 资产内联风险）——回收后零静默断图/断字面。

## 0019 预览渲染迁独立 webview（ADR-0019：S10-① spike + S10-② 迁移落地定稿）

**Status**: accepted（2026-09-08。S10-① spike 四问实证（go）；S10-② 迁移落地（CP-012/013/031/044）后定稿；③④ 后续条目结果在逆转记录节追加）

**上下文**：CP-012/013/035/044 同根（预览通道 iframe srcDoc 与主窗口共享 CSP/上下文，SEC-09 结构性问题）。修复方向 = 预览渲染迁出主窗口 CSP 域到独立 Tauri webview，但整个 S10 的 go/no-go 依赖「WDIO（embedded driver：tauri-plugin-wdio-webdriver 1.3.0 内嵌 WebDriver + tauri-service 1.3.0 JS 服务）能否枚举/驱动独立预览 webview」——此前零实证。

**决策一（S10-① spike 实证，2026-09-08 主窗口 + 两个独立预览 WebviewWindow 实测，spike 代码不入库）**：

1. **go——四问全 yes**：① 可枚举（`getWindowHandles()` = webview_windows label 全集，启动即入列、销毁后出列）；② execute 可达（switchToWindow 后 execute/`$` 作用于预览页上下文，含 asset 与 data: 双候选）；③ 焦点语义（driver 命令与 OS 前台焦点解耦，显式 switch 后 `$` 族无 +5s 惩罚）；④ 销毁语义（closeWindow 后句柄收缩、driver 存活，销毁异步需轮询）。
2. **跨独立 WebviewWindow 无 window.postMessage 通道**（实测 main 收不到预览 postMessage）——**消息桥 = Tauri event/IPC**；CP-044 走「通道退役」备选结论分支：不引入 PREVIEW_ORIGIN，targetOrigin 议题随跨窗口 postMessage 退役（iframe ↔ 宿主页的窗口树内 postMessage 保留，targetOrigin "*" 语义不变）。
3. **CSP 无 per-webview 覆盖**（tauri 2.11.5 builder API 无 csp 属性；CSP 为 app 级配置，数据页亦被注入全局 CSP meta——tauri 源码实证：Windows 资产响应带 CSP 头 + 静态内联脚本哈希化，运行时内联脚本在收紧后全灭）。
4. **WDIO 驱动策略**：句柄 = label；`browser.switchToWindow(固定 label)` 切上下文（显式切换抑制焦点自动恢复）；每预览面板 = 独立 WebviewWindow + 固定 label（`preview-<panelId>`）；断言 execute-first、结束切回 `'main'`；TQ-E-10 焦点探针语义限单窗口。（策略全文见 e2e-tests/CLAUDE.md「多 webview WDIO 可达性」节。）

**决策二（S10-② 迁移落地，2026-09-08）**：

1. **预览承载域 = 自定义协议宿主页（② 落地面复核结论，修正 spike 的 asset 首选）**：预览窗口加载 Rust 注册的自定义协议 `slterm-preview`（Windows 映射 `http://slterm-preview.localhost/preview-host.html`，register_uri_scheme_protocol 文档实证；响应不带全局 CSP——域级 CSP 由宿主页 meta 承载，见下 9）。asset 协议页（http://tauri.localhost）恒被注入全局 CSP（响应的 CSP 头）——CP-012 主窗口收紧 script-src 后，资产域内运行时内联注入与宿主自带脚本全灭（静态内联脚本虽经构建期哈希放行，运行时注入产物不可哈希）——asset 候选否决，「预览 CSP 域」= 自定义协议域。
2. **宿主页 = 固定桥接页**（src-tauri/src/preview.rs 内嵌 const）：建 sandbox iframe（allow-scripts、无 allow-same-origin——CVE-2024-35222 红线延续，iframe 无 Tauri IPC 注入——main_frame_only 实证）→ 内容经 `preview_render`（后端存储 + seq + 定向 ping）→ 宿主 `preview_pull` 拉取置 srcdoc → iframe 文档消息（zoom/scroll/nav 上行、reset/zoom_set/scroll_set 下行）经 Tauri event 与主窗中继；事件名单点登记 src/ipc/preview.ts（三处同步：TS / 宿主桥 / 守卫测试）。
3. **注入机制原样迁入**：injectScript + buildInjectedScript + nonce 装配于主窗 PreviewFrame，产物推送预览域执行；**字符串级转义消亡**（escapeScriptClose 删除——宿主 `<script>` 不经转义进入渲染文档且真实执行，CP-031 判定一/二在 ② 达成，判定三由 html.e2e 宿主 script 用例在真实 WebView2 断言）。
4. **上行命令面收窄为零（CP-013）**：keydown 转发段与信任标记整体退役（键盘不跨窗口——预览窗口 focusable(false)，焦点恒在主窗 ShortcutRegistry 域，全局快捷键在预览态可用）；上行终态集合 = {slterm_zoom, slterm_scroll, slterm_nav}（UPLINK_MSG_TYPES 白名单守卫锁死）；下行 = {reset, zoom_set, scroll_set}。
5. **主窗口 CSP 终态（CP-012）**：`script-src 'self'` + 删除 dangerousDisableAssetCspModification 整键（img-src/font-src data: 回收归 CP-035）。
6. **CP-037 复核结论**：预览迁出后 workspace 层 CSS 显隐保活对预览 webview 不适用——CM 保活（面板内 allotment visible=false 恒挂载）**维持**（edit/split/preview 形态往返仍须保留 undo/光标）；预览窗口保活另成机制：**隐藏保活 = 窗口 hide 不销毁**（面板/页面隐藏 → 几何归零 → preview_sync visible=false；恢复 → show）——缩放/滚动态随 iframe 文档存亡，跨显隐保留。两机制并行，各管各层。
7. **面板形态（波及面）**：面板根改「40px 工具条带（切换条/HUD 悬浮带，FloatingArea direction="row"）+ 内容区（PreviewFrame 锚点）」列排——预览窗口几何 = 锚点矩形（主窗 inner 原点 + CSS × scale，PreviewFrame 200ms 轮询 + 主窗移动即时同步）；窗口 owned 无边框、focusable(false)、skip_taskbar；面板卸载 → 窗口销毁（缩放随窗口销毁 = 旧关页签语义）。
8. **键盘边界已知行为（接受）**：预览窗口不可聚焦 → 预览文档内表单键入/系统复制快捷键不可达（鼠标滚动/点击/拖选不受影响）；主窗快捷键在预览态恒可用。若未来需表单键入，须先解决「预览聚焦吞全局快捷键」问题（本决策 4 的逆转触发点）。
9. **域级 CSP 落地（SEC-02，2026-09-09）**：宿主页 HOST_PAGE 加 `<meta http-equiv="Content-Security-Policy">`——指令表 `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:`（`default-src 'none'` 断全部出网；内联 script/style 为宿主桥 + 注入产物必需；img/font `data:` 为 markdown 本地图内联与 KaTeX 内联字体通道，ADR-0018）；srcdoc iframe 继承宿主 CSP（W3C 行为）→ 预览文档同受此约束。tauri 2.11 无 per-webview CSP 配置面 → 域级 CSP 只能由宿主页 meta 承载（响应头形态不变），加宽指令须复核 SEC 面。

**被否决的备选**：

- **同窗口 add_child 子 webview（多 webview-in-window）**：driver 不可枚举且宿主窗口整体消失（实测）——S10 架构不可选。
- **data: URL 注入承载预览**：内容被全局 CSP meta 包裹改写 + opaque origin + 需 webview-data-url feature（实测）——否决。
- **asset 协议宿主页承载预览（spike 期首选）**：CSP 无 per-webview 覆盖下资产域恒带全局 CSP 头 + 静态哈希——收紧后运行时注入/宿主脚本不可行（② 落地面复核否决，见决策二 1）。
- **主窗侧保留 iframe + 运行时经 'self' 外部 js 注入（资产域候选路线）**：运行时代码须运行时生成（nonce/段型/内容变化），无法静态化；且宿主文档 `<script>` 永不可执行（CP-031 判定三不可达）、CP-033/035 的预览域 data: 放行无落点——否决。
- **同态维持（no-go 出口）**：四问实证通过，go 成立——未触发。

**后果**：

- docViewer 六件 + 面板两件重构：PreviewFrame 改窗口编排/事件桥（无 iframe）；buildInjectedScript 去 keydown 段；injectScript 去转义；previewMessages 增白名单；ipc 新增 preview 域；HtmlPanel/MarkdownPanel 工具条带化 + FloatingArea row variant；Rust 新增 preview 模块（scheme 协议 + 4 命令 + 内容存储）与 preview 能力文件（最小权限：事件 + preview_pull）。
- 命令三处注册（lib.rs/build.rs/capabilities）新增 4 条；capabilities 新增 preview.json（windows glob `preview-*`）。
- E2E 可达性落地：html.e2e/markdown.e2e 经 label 切换驱动预览窗口；内容断言 = 宿主页读 iframe srcdoc；HUD 断言在主窗工具条带。
- **已知行为登记**：预览键盘键入不可达（决策二 8）；预览窗口几何跟随为轮询驱动（拖拽期亚秒级滞后）；宿主脚本现可执行（信任模型 = 本地文件全信任延续，ADR-0017 同源）。
- **逆转触发点**：driver 升级出现子 webview/帧级寻址时重估「独立 WebviewWindow」约束；Tauri 提供 per-webview CSP 时复核预览域选择（自定义协议 vs 资产域）；出现「预览内键盘输入」需求时重评 focusable 决策（须先解全局快捷键吞键问题）；CP-033/035（③④）结果在本节追加登记。
- **CP-033（③）结果登记（2026-09-08）**：B2 维持分支——KaTeX data 内联经新 webview 上下文真实 WebView2 实证可用（字体族命中 + 字体真实加载）；asset 通道维持否决（响应 ACAO 固定 webview origin vs 内容 iframe opaque origin null，源码 + 实测双证据）。全文见 ADR-0018「维持记录（CP-033）」；④ 若建立预览域局部 CSP 须放行 font-src data:。
- **CP-035（④）结果登记（2026-09-08，font-src 实证记录归档）**：③ B2 实证通过 → 主窗口 CSP 终态落地——img-src/font-src 双双回收 data:（tauri.conf.json + csp-config.test.ts 三守卫锁死），预览域当时维持无 CSP（data: img/font 天然放行，无代码落点——SEC-02 起宿主页 meta 承载域级 CSP，见 ADR-0018 落地复核注记）；执行期发现主窗口唯一 data: 图像消费点 = CM6 lint 波浪线 svg 背景（JsonMode），改 text-decoration wavy 技法消除（theme/overrides.ts）；svg data: 显式禁用（markdown assets 白名单剔除）。处置全文见 ADR-0018「回收记录（CP-035）」；本决策 5「img-src/font-src data: 回收归 CP-035」至此执行完毕。

## 0020 页内分屏（ADR-0020：推翻 CP-004/S11「页 = 单组多页签」）

**Status**: accepted（2026-09-12。bug 2「拖拽分屏面板消失」修复定稿——D3 用户裁决支持页内分屏；D7 仅网格分屏，禁 floating/popout）

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
