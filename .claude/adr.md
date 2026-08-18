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
- 启动链 fail-safe 收编：index.html/tauri.conf.json 为静态层无法用 TS token，保持硬编码 + 注释交叉引用。

**后果**：
- 新增方案 = schemes/ 新文件 + register 一行，消费方/测试守卫零改动。
- 注释单点在 types.ts 接口槽位（消费位置与方案无关），新方案零注释负担。
- main.tsx 静态 import 图收敛为 react/react-dom/lib/e2eEnabled；E2E helpers 与 ROOT_CSS_VARS 注入保持原相对顺序。

## 0003 UI 全面重设计（Linear 极黑克制）

**Status**: accepted（2026-08-16）

**上下文**：现状 UI（darcula 时代）被评价为「古板呆滞」——中灰蓝底平铺、实色粗边框、盒式页签、装饰 emoji、侧栏多区块堆叠。用户调研 11 款软件暗黑 UI（docs/refer/）后要求整套重设计：仅暗黑、JetBrains Mono 全局、无动效、不做多主题切换。只交付设计产物（设计方案 + 需求规格 + 视觉稿），不含实现。

**决策**：经 13 题澄清（风格锚点/底色温度/强调色/标题栏/图标/页签/密度/IA/内容色/交付流程/规格粒度）+ 6 个候选视觉稿（3 骨架内 + 3 突破型）浏览器实测对比，选定**候选 A「Linear 极黑克制」**，无微调：

- 风格锚点 Linear/Zed 近纯黑现代风；暖黑**明度阶梯** 6 档（`#0a0a0b`→`#2b2b31`），内容区最暗；分隔一律**发丝线**（半透明白 0.055/0.09）。
- 单强调色现代蓝 `#6e9ff2`（<5% 像素占比）；语义色低饱和暖协调；终端 ANSI 16 色与编辑器语法色全量重调（**双轨配色**：壳层 token 与内容色板互不混用）。
- 自绘 34px 一体化标题栏（原生标题栏退役）；扁平页签 + 底部 2px 指示条 + hover 才显关闭钮。
- 侧栏 IA 重构为**统一导航树**（项目 → 页面 → 会话，历史会话折叠计数）；文件浏览器独立为活动栏视图；活动栏固定三槽（导航树/文件/Commit）。
- 装饰图标全部单色线性 SVG（15px/1.5px 描边/currentColor）；状态 emoji → **状态圆点**（绿/黄/灰，语义来源 F3 不变）；CLI 品牌 logo 保留彩色。
- 交付物：`docs/ui-redesign/design.md`（设计方案）、`requirements.md`（UI-xxx 编号需求 + 可测验收 + P0/P1 + 对比度自检附录）、`final-mockup.html`（主界面 + 组件集双页静态稿）。

**与 theme 系统对接**（后续实现期）：全部色值经 ADR-0002 配色方案单点落位——新增 scheme 文件替换 darcula 内置方案，需求规格每条色值标注 types.ts 槽位；启动链 fail-safe 三处静态色手动同步 `#0a0a0b`；多主题切换机制不建（硬约束）。

**被否决的备选**：
- 候选 B（Zed 实体边框感）/ C（最暖+大圆角）：骨架内变体，层级靠边框/温度而非纯明度差，不如 A 的「界面消失」感。
- 候选 D（紫罗兰强调色+冷黑底）/ E（One Dark 盒式页签）/ F（teal+胶囊页签）：突破型对照组，验证已定决策（现代蓝/暖黑/扁平页签）成立，无需突破。
- 多 UI 模式/亮色系：用户硬约束，明确排除。

**后果**：
- 实现期按 requirements.md 逐条验收（P0 必做）；视觉验收参照 final-mockup.html 组件集页。
- F3 四态 emoji 的**视觉呈现**被状态圆点取代（事件→状态映射逻辑不变）；F9 品牌 logo 保留。
- 现状侧栏（项目列表 + Agent Status 等区块）IA 将随实现重构为统一导航树；行为逻辑（快捷键/右键菜单/会话恢复）不变。

**实现期决策（2026-08）**：Stage 01-08 实施（docs/ui-redesign-impl/checklist.md 46 项）期间追加确认的决策，逐条落地于代码：

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
- **overrides 段保持现状**（`^`），不随本策略调整。
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

## 0008 notify RC 保持（9.0.0-rc.4 / notify-debouncer-full 0.8.0-rc.2）

**Status**: accepted（2026-08-18，TE-04）

**上下文**：Rust 侧 `notify`（文件系统监听核心）与 `notify-debouncer-full` 为 RC 版本。一手证据（`src-tauri/Cargo.toml:36-37、48-49` 跟踪注释）：`notify` 9.0.0 仍为 RC 阶段——最新稳定版 8.2.0（2025-08-03），而 rc.4（2026-05-02）为当前**最新**版本，无更高稳定版可升；降 8.x 属功能回退。watcher 行为有 notify 模块 51 条 L1 回归测试守护。

**决策**：**保持 RC，不降 8.x**；沿用 Cargo.toml 跟踪注释关注 `https://crates.io/crates/notify` 正式发布。上游发布稳定版后升级，升级时跑 notify 模块全量 L1（51 条 watcher 回归）+ 大目录监听实测。

**后果**：

- `notify` 正式版发布即升级触发点（Cargo.toml 注释已登记），升级走常规依赖变更流程。
- RC 风险（API 变动/缺陷）由 51 条 L1 回归守护兜底，与 9.x 前瞻收益（后续版本能力）权衡后接受。

## 0009 review-fix 豁免与决策汇总登记（DOC-10）

**Status**: accepted（2026-08-18，DOC-10）

**上下文**：review 修复（docs/review-fix/checklist.md 93 项）多处以「登记豁免/保留现状 + 文档登记」关闭，登记点散落各模块文档。此处汇总 root 侧豁免决策，避免各登记点失散；模块内明细以对应模块 CLAUDE.md 为准。

**决策**：

| 标识 | 决策 | 登记点 |
|------|------|--------|
| FE-01 | Workspace 多 Dockview 实例**保持**（H6 终端跨页面存活 + xterm 实例限制，D1）；以页面总数上限 `MAX_PAGES = 20`（src/stores/projects.ts，超限 addPage 拒绝 + toast「页面数已达上限」）防内存/DOM 线性增长 | src/workspace/CLAUDE.md |
| SEC-09 | CSP `script-src 'unsafe-inline'` **保留**（D4）：srcdoc iframe 继承父 CSP（W3C 行为），HTML 预览注入脚本（锚点拦截/键盘转发/nonce）必须内联，移除即破坏预览。现状 = tauri.conf.json `script-src 'self' 'unsafe-inline'` + `dangerousDisableAssetCspModification: ["script-src"]` | src-tauri/tauri.conf.json 注释 |
| SEC-06 | 剪贴板读权限 `clipboard-manager:allow-read-text` **保留**（D6）：唯一消费点为 keyboard.ts 的 Ctrl+Shift+V 显式手势，改后端命令不缩小攻击面（前端上下文被注入时同样能 invoke）；grep 级守卫测试锁消费点集合 | src/ipc/CLAUDE.md |
| BE-21 | `fs_read_dir` 返回整目录列表**不分页**（登记豁免）：懒加载按目录分层 + FileTree 虚拟化（FE-30）覆盖渲染侧，单层万级文件罕见；改分页 = IPC 契约破坏性变更，收益不抵成本 | src-tauri/src/fs/CLAUDE.md |
| FE-31 | CodeMirror 大文件**不虚拟化**（按 D3 关闭）：fs_read_file Channel 分块（BE-03）削峰 + 10MB 上限 + 1MB 警告已覆盖峰值；CM6 文档模型不支持部分加载 | src/panels/editor/CLAUDE.md |
| 09#14 | 后端 Mutex（ring buffer 等 std::sync::Mutex）中毒**保持现状**：等待调用方（reader_loop 持锁写、读侧同锁）同样会 panic，中毒非新增攻击面；parking_lot / catch_unwind 仅作**未来引入高风险外部代码**时的预案，现不引入 | src-tauri/src/CLAUDE.md |
| TE-03 | xterm 三件套 beta 保留 + 升级审批门禁（L3 + E2E + 真实 claude 实机滚轮 + Win10 21376 阈值核对） | ADR-0007（本文件） |
| TE-04 | notify 9.0.0-rc.4 / notify-debouncer-full 0.8.0-rc.2 保持（rc.4 即最新，无稳定版可升；51 条 L1 watcher 回归守护） | ADR-0008（本文件） |

**后果**：

- 新增豁免须先在本表或对应模块 CLAUDE.md 登记再关闭，禁止只改代码不留档。
- 行为固化点：`MAX_PAGES`（L2 测试断言）、CSP（tauri.conf.json）、剪贴板守卫测试、`GIT_REPO_CACHE_CAPACITY`/`WATCHER_POOL_CAPACITY`/`MAX_PTY_SESSIONS`（L1 契约测试）。
