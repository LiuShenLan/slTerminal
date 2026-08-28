# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目性质

**slTerminal** — 面向 Windows 10/11、专为 Claude Code CLI 调优的通用终端模拟器。

定位约束（贯穿全程，不可违背）：
- Windows 原生跑 `claude`（不走 WSL）；单窗口单实例；仅暗色模式；渲染 GPU 加速。
- 默认 shell：PowerShell 7（`pwsh.exe` → `powershell.exe` → `cmd.exe` 回退）。
- 复制 = `Ctrl+Shift+C`（`Ctrl+C` 保留为中断，供 claude 取消）。

## 文档规范

- **代码自证原则（ADR-0011）**：凡能通过阅读代码直接理解的信息（模块职责、文件清单、入口路径、接口签名、数据流、技术栈、现状描述等）一律不写入文档——此类内容必然腐化为失真文档。文档只记录代码无法自证的信息：设计决策与原因（why）、外部依赖的坑与红线、操作指令、约定与豁免登记。
- **渐进式披露**：读取某目录代码后，才需要读取该目录的 CLAUDE.md。
- **收录判定**：跨 ≥2 个模块适用、或每次会话必需的指令入根文件；仅触碰某模块才需要的归该模块子路径 CLAUDE.md。
- **子文件创建**：新建模块目录时同步创建该路径 CLAUDE.md，并登记下方模块索引。
- **子文件模板**：存在理由（一段话：为何需要此模块/不这么设计会怎样）→ 关键约束与决策（why）→ 外部坑/红线 → 测试模式（仅非显而易见部分）。不写「职责」「文件表」（代码自证）。
- **及时同步**：改动约定/决策/红线时同步对应 CLAUDE.md；只改实现不改约定时无需动文档。
- 配套文档：领域术语表 `@../CONTEXT.md`；架构决策记录 `@.claude/adr.md`；测试用例清单 `@.claude/test-inventory.md`。

## 架构（两进程模型）

**Rust 后端拥有一切 OS 访问；Web 前端只做 UI，经 IPC 调用后端。**（技术选型清单读 package.json / Cargo.toml 即得，不复述。）

目录结构原则：实现落到既定分层，不另起炉灶。现行模块清单以下方「模块索引」为准。

## 硬性开发约束（新增功能必须遵守）

1. **前端绝不直接碰 OS/文件/进程**：`invoke` 只允许出现在 `src/ipc/`；其它文件只调用 `ipc/` 暴露的领域函数。
2. **后端按功能分模块**（现行清单见模块索引）：模块间不互相穿透，共享只经 `state.rs` 的 `AppState`。
3. **命令统一注册**于 `lib.rs` 的 `generate_handler!`；一律返回 `Result<_, AppError>`；阻塞 I/O 用 `spawn_blocking`。
4. **DTO 双边对应**：`src/types/` ↔ Rust 模块 DTO 一一对应；Rust `snake_case` ↔ JS `camelCase`，改一边必须改另一边。字段类型泛化后，其语义值集（枚举字面量联合等）须在 CLI profile（前端 `cliProfiles` 能力声明）与后端对应 provider（Rust 枚举）同步登记，并配合同步测试锁死一致性（先例：HooksLayer `"user"|"project"|"local"` ↔ 后端 `Layer` 枚举，FE-14/BE-18）。
5. **面板封闭**：Dockview 面板只能是 `panels/` 下注册过的类型；新增类型 = 加目录 + 在 `panelRegistry.ts` 注册。合法形态含「hub 容器 + 注册表分派子编辑器」——hub 面板本身是普通注册面板，内部经注册表分派渲染子编辑器组件（hooksConfig 先例：hub 经 CLI profile 的 `configEditor` 字段分派，新增 CLI 自带编辑器组件即可接入，hub 零改动，KZ-1）。
6. **配色单点**：颜色定义于 `theme/schemes/<scheme>.ts`（配色方案值文件），组件经 `theme/colors.ts` facade token 引用，禁止硬编码颜色。既定例外完整清单：
   - 启动链 fail-safe 三处静态色（index.html body 背景 / tauri.conf.json 窗口背景 / main.tsx 超时错误页）——先于方案加载，改 linear 对应 ui 值须手动同步（→ ../src/theme/CLAUDE.md）
   - 终端 adapter（panels/terminal/theme.ts 展开 active 方案 terminal 段）——例外范围仅 terminal 段，不扩新（→ ../src/theme/CLAUDE.md）
   - FileIcon 六色盘（→ ../src/features/explorer/CLAUDE.md）
   - 项目行文件夹蓝（→ ../src/features/navTree/CLAUDE.md）
   新增例外须同步登记对应模块 CLAUDE.md（注明范围与同步义务），禁止只改代码不留档。
7. **布局单点**：操作页面布局只经 `workspace/layoutSerde.ts` 用 Dockview `toJSON/fromJSON` 存取。
8. **会话元数据单点**：PTY 进程映射仅在 `panels/terminal/TerminalRegistry`（模块级 Map）管理，前端会话元数据已合并。面板只订阅，不自存。
9. **平台分支收敛**：业务 `#[cfg(windows)]` 仅允许出现在 pty 模块（spawn.rs / shell.rs / win_build.rs / conpty_api.rs 等 Windows 专用 API 编译期分支），业务逻辑不撒 cfg（详见 ../src-tauri/src/pty/CLAUDE.md）。测试 `#[cfg(windows)]` 原则上改运行时 `cfg!(windows)` 分支；依赖 Windows 编译期 API（symlink 等）无法运行时区分的例外保留 cfg，须在所属模块 CLAUDE.md 登记豁免（BE-17/D5 先例：state.rs、agent_history/ops.rs、hooks/signal.rs、hooks/watcher.rs、notify/mod.rs 的 symlink 特权测试）。
10. **权限最小化**：Tauri 2 自定义命令默认放行，`capabilities/` 只管插件权限；不追加通配 `*`。
11. **测试覆盖**: 改动的代码可自动化部分必须添加全量自动化测试用例覆盖；不可自动化部分（平台 API 直调、真实 ConPTY/OS 交互、人工实测场景等）须在 `.claude/test-inventory.md` 既定豁免清单登记，注明豁免原因与当前兜底层级（L1/L4 集成用例、人工验证手册等），禁止未登记豁免。
12. **store 纯状态**：`src/stores/` 只存状态与状态转换，不存业务逻辑（校验/映射/编排放注册表、纯函数或上层组件）；持久化一律经 `src/ipc/` 对应领域函数（settings 类经 ipc/settings、项目数据经 ipc/projects），禁止在 store 内直接调用 `invoke`；禁止跨 store 隐式依赖——store 间协调在上层组件/命令中完成（详见 ../src/stores/CLAUDE.md）。
13. **注册表家族通用契约**：注册表类模块统一形态——模块级单例（模块内实例化并导出，或 getXxxRegistry() 惰性获取）、`register(...)` / `getAll()`（按注册序）接口、`_reset()`（仅测试用，清空全部条目）；注册经 side-effect import 触发（import 即注册，触发点登记于所属模块 CLAUDE.md，禁止隐式初始化）；测试在 beforeEach/afterEach 调 `_reset()` 保证用例隔离。先例：panelRegistry / SchemeRegistry / FileViewerRegistry / ShortcutRegistry / SideViewRegistry / CliProfileRegistry / TerminalRegistry。
14. **git 追踪文件凭据红线（SEC-18）**：真实凭据值（API token/key、Authorization 头实际值）禁止写入任何 git 追踪文件——代码、测试夹具、文档、脚本一律不行；测试与文档仅允许假值占位符（`sk-test` 形态）。真实凭据只存 user 层 `~/.claude/settings.json`（仓库外，读取侧在 plan_balance/source.rs）。自查：`git grep -nE "sk-[A-Za-z0-9_-]{16,}"` 应零命中（2026-08-29 全仓排查实证）。

## Windows 关键坑

每条只留红线规则，机制与背景在链接的子文件。

- **spawn 串行化**：并发 spawn 会卡死 ConPTY 输出管道——`pty_spawn` 必须握 `SPAWN_LOCK`。详见 ../src-tauri/src/pty/CLAUDE.md
- **ConPTY flags 三态**：PASSTHROUGH_MODE (0x8) 吞全屏 TUI 鼠标滚轮输入（禁用红线）；捆绑新 conhost（ADR-0005，仅 Win10）/系统 Win11 → 0x7、系统 Win10 回退 → 0x3；自动化测试无法守卫（假阴性），改 flags 必须实测真实 claude 滚轮。详见 ../src-tauri/src/pty/CLAUDE.md
- **cwd 反斜杠**：传给 ConPTY 前把 cwd 规范化成 `\`（`CreateProcessW` 对 `/` 行为异常）。详见 ../src-tauri/src/pty/CLAUDE.md
- **cwd / 命令边界跟踪**：portable-pty 在 Windows 不返回 cwd——靠集成脚本注入的 OSC 序列跟踪，禁止解析提示符。详见 ../src-tauri/src/pty/CLAUDE.md
- **键盘 / IME**：Shift+Tab、Ctrl 组合键用 xterm.js `attachCustomKeyEventHandler` 接管；中文 IME 合成要尽早实测。详见 ../src/panels/CLAUDE.md
- **E2E 用不了 Playwright**（Tauri 非 Chromium）：用 embedded driver，零 msedgedriver 依赖。详见 ../e2e-tests/CLAUDE.md
- **watcher 不频繁重建**：`notify` 递归注册大目录耗时——用 watcher 池缓存 + pause/resume 切换，禁止 stop/start 轮换。详见 ../src-tauri/src/notify/CLAUDE.md
- **HTML 预览 iframe sandbox**：sandboxed iframe 中 `#fragment`/`:target` 彻底失效，`allow-same-origin` 会致 Tauri 向 iframe 注入 App JS——固定 `sandbox="allow-scripts"` + 注入脚本拦截锚点点击。详见 ../src/panels/CLAUDE.md
- **测试 tempdir 8.3 短名**：CI runner 的 `%TEMP%` 是 8.3 短名——Rust 测试路径比较前用 `dunce::canonicalize` 统一长名，否则 CI 失败而本地不复现。详见 ../src-tauri/src/git/CLAUDE.md

## 命令

- 开发运行：`npm run tauri dev`
- 构建：`npx tauri build --debug --no-bundle`
- **测试/使用流程（用户固定习惯）**：永远用 `npx tauri build --debug --no-bundle` 构建产物测试与使用，**不使用 dev 模式**（`npm run tauri dev` 仅保留为开发兜底）。产出 exe + dll（debug 模式），部署到本机或 win10 另一台 PC 使用

## 测试策略

四级测试金字塔，按执行速度和隔离度分层。完整用例清单 → `@.claude/test-inventory.md`。

| 层级 | 名称 | 技术栈 | 运行命令 |
|------|------|--------|----------|
| L1 | Rust 单元/集成 | `cargo test`、`tempfile` 隔离 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` |
| L2 | 前端单元/集成 | Vitest + jsdom | `npm test` |
| L3 | 终端 headless 渲染 | Vitest + `xterm/headless` | `npm run test:l3` |
| L4 | 端到端 (E2E) | WDIO + embedded driver | `npm run e2e`（= `build:e2e` + `wdio`） |

核心原则：
- **隔离优先**：L1 用 `tempfile::tempdir()` 隔离文件系统、`SPAWN_LOCK` 串行化 PTY；L2 用 `vi.mock()` 隔离 IPC/终端库；L4 用 embedded driver 隔离浏览器依赖
- **L1/L2 覆盖所有 PR**，L3/L4 覆盖关键路径变更
- **bugfix 须附防复发测试**：修复缺陷时同步提交常驻回归用例，防同一缺陷重现
- **用例清单同步**：新增/修改/删除用例须同步更新 `.claude/test-inventory.md`
- **L1 必须 `--test-threads=1`**：ConPTY 并发 spawn 会死锁
- **L4 必须 `VITE_E2E=1` 构建**（用 `npm run e2e`/`build:e2e`）：E2E helper 由 `E2E_ENABLED` 门控，`tauri build` 前端恒为 production `vite build`（`DEV=false`），不设开关则 helper 被 tree-shake、wdio 全部卡"Workspace 未就绪"
- **模块测试模式见各子路径 CLAUDE.md**，不在根文件展开

静态检查门禁：
- TypeScript：`npx tsc --noEmit`
- ESLint：`npx eslint src/`
- Clippy：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- rustfmt：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

### 发布打包

**一键打包**：`.\.claude\package.ps1 -Version "0.1.0"`（release 模式，单文件 exe → zip）
加 `-Debug` 用 debug 模式（exe + dll 两个文件）。

## 模块索引

登记规则：增删模块时同步增删行。职责与入口读代码即得（代码自证原则），此表仅作子 CLAUDE.md 导航。

| 模块 | CLAUDE.md |
|------|-----------|
| src/ipc | ../src/ipc/CLAUDE.md |
| src/types | ../src/types/CLAUDE.md |
| src/stores | ../src/stores/CLAUDE.md |
| src/workspace | ../src/workspace/CLAUDE.md |
| src/panels | ../src/panels/CLAUDE.md |
| src/lib | ../src/lib/CLAUDE.md |
| src/theme | ../src/theme/CLAUDE.md |
| src/features/explorer | ../src/features/explorer/CLAUDE.md |
| src/features/fileViewers | ../src/features/fileViewers/CLAUDE.md |
| src/features/shortcuts | ../src/features/shortcuts/CLAUDE.md |
| src/features/sidebar（已退役并入 navTree，仅存历史文档） | ../src/features/sidebar/CLAUDE.md |
| src/features/navTree | ../src/features/navTree/CLAUDE.md |
| src/features/titleBar | ../src/features/titleBar/CLAUDE.md |
| src/features/sideViews | ../src/features/sideViews/CLAUDE.md |
| src/features/commit | ../src/features/commit/CLAUDE.md |
| src/features/agentStatus | ../src/features/agentStatus/CLAUDE.md |
| src/features/notifications | ../src/features/notifications/CLAUDE.md |
| src/features/hooksConfig | ../src/features/hooksConfig/CLAUDE.md |
| src/features/cliProfiles | ../src/features/cliProfiles/CLAUDE.md |
| src/features/agentHistory | ../src/features/agentHistory/CLAUDE.md |
| src/__tests__ | ../src/__tests__/CLAUDE.md |
| src/panelRegistry.ts（共享配置层，硬约束 #5） | — |
| test/（L3 headless） | — |
| src-tauri/src/pty | ../src-tauri/src/pty/CLAUDE.md |
| src-tauri/src/fs | ../src-tauri/src/fs/CLAUDE.md |
| src-tauri/src/git | ../src-tauri/src/git/CLAUDE.md |
| src-tauri/src/notify | ../src-tauri/src/notify/CLAUDE.md |
| src-tauri/src/plan_balance | ../src-tauri/src/plan_balance/CLAUDE.md |
| src-tauri/src/hooks | ../src-tauri/src/hooks/CLAUDE.md |
| src-tauri/src/agent_history | ../src-tauri/src/agent_history/CLAUDE.md |
| src-tauri/src 顶层单文件模块（lib/app_dir/settings/projects/state/error） | ../src-tauri/src/CLAUDE.md |
| e2e-tests | ../e2e-tests/CLAUDE.md |

## 需求编号索引

代码和文档中引用的短标识符规则：

**前缀语义**：

| 前缀 | 含义 | 示例 |
|------|------|------|
| H | 需求（早期高层需求清单） | H6 |
| E | 工程机制需求 | E1 |
| P | 问题（阶段-序号） | P1-19 |
| F | 特性 | F3、F5 |
| SEC | 安全约束 | SEC-01 |
| DBG | 调试调查 | DBG-5 |
| B | 缺陷 | B10 |
| FIX | 修复项 | FIX-TE-04 |
| ADR | 架构决策记录 | ADR-0001 |
| L | 测试层级——免登记，定义见「测试策略」 | L1–L4 |
| R | 回归变体——免登记，模块内就近定义 | R2–R4 |

**登记规则**：跨模块引用的标识符首次使用时登记到下表；仅模块内部使用的就近定义，不登记。

**未列入的编号家族免登记**：阶段项目代号（如 MC-*（multi-cli profile 重构）、C13-*、DOC-*、E2E-*、HFN-*、HUK-*、IHE-*、SVC-*、WRK-*、TE-* 等）登记于所属模块文档与 `.claude/test-inventory.md`，免入根表。

| 标识符 | 类型 | 含义 |
|--------|------|------|
| H6 | 需求 | 终端跨页面存活——页面切换不杀 PTY 进程 |
| E1 | 需求 | Channel 可替换 + ring buffer 回放——PTY 重连机制 |
| P1-19 | 问题 | 窗口关闭前杀子进程——前端 registerCloseHandler 遍历 kill + 后端 Job Object KILL_ON_JOB_CLOSE 兜底 |
| SEC-01 | 安全 | project_root 是页面切换前置条件（路径沙箱） |
| SEC-03 | 安全 | HTML postMessage origin/source/信任标记三层校验 |
| SEC-05 | 安全 | agent_history_delete 的 sessionId 校验 + 定位不信托前端 |
| SEC-08 | 安全 | PTY write/resize/kill 的 panelId 归属校验 |
| SEC-15 | 安全 | shell 白名单 fallback 收窄——双侧 canonicalize 均失败才回退归一化字符串比较，单侧失败即拒绝 |
| SEC-16 | 安全 | set_project_root 经 tokio Mutex 串行化——慢 canonicalize 不得后写覆盖 |
| SEC-17 | 安全 | hooks user 层写入后端审计日志（`tracing::warn! target:"audit"`）兜底 |
| SEC-18 | 安全 | git 追踪文件禁止真实凭据值——测试/文档仅允许假值占位符（sk-test 形态）；真实凭据只存 user 层 settings.json（仓库外） |
| DBG-6 | 调试调查 | 启动恢复 lastPage 先 await setProjectRoot 再 setActivePage |
| B10 | 缺陷 | 编辑器去重聚焦须匹配 suffix（普通编辑器与 git 页签互不误聚焦） |
| B11 | 缺陷 | statusline 注入须递归解包自有脚本包裹；桥接脚本容忍引号 + 剥引号后 ~ 展开 + 透传失败 stdout 占位 |
| B12 | 缺陷 | 恢复时无 customTitle 的终端面板经 titleManager 重算 terminal-N；F8 customTitle 保留 |
| B13 | 缺陷 | 标题恢复只由真退出信号（OSC 133 D / PTY EXIT）承担；SessionEnd 不恢复，SessionStart 补 title 重设 |
| B14 | 缺陷 | panelId 生成/解析单点收口（makeTerminalPanelId/parseTerminalPageId 成对）；恢复面板 id 不含 Date.now 段 |
| B15 | 缺陷 | reinject_statusline 须传 statusline_script_path() 作桥接路径（误传 reporter 路径致重启后空白行） |
| B16 | 缺陷 | 桥接 .sh 经 bashCandidates 试错定位（PATH→where git 上溯→固定路径）+ 反斜杠转正斜杠 |
| B17 | 缺陷 | spawn PowerShell 携带 -NoProfile 致用户 profile 不加载——conda activate 失效（win11 CondaError / win10 conda.bat 静默空转）；修复 = 删 -NoProfile 恢复 profile 原生加载 |
| ADR-0001 | 架构决策 | 侧栏视图换区重建丢失组件内部状态（已确认接受）（→ .claude/adr.md） |
| ADR-0003 | 架构决策 | UI 全面重设计「Linear 极黑克制」（→ .claude/adr.md） |
| F2 | 特性 | hooks 注入入口（F6 面板工具栏并入；与功能键 F2 区分） |
| F3 | 特性 | 终端页签四态状态指示（agent-event + OSC 133 合成；视觉呈现为状态圆点） |
| F5 | 特性 | agentSession 契约行建模（双通道建行/三通道删行） |
| F6 | 特性 | hooks 双模式配置面板（JSON/GUI 编辑 hooks 子树，user/project/local 三层，F2 并入） |
| F7 | 特性 | 历史会话查询与恢复（扫描/恢复/删除 + 四步恢复编排；重命名已移除） |
| F8 | 特性 | 终端页签自定义重命名（customTitle 随布局持久化；claude 运行中禁用；不影响 terminal-N 递增） |
| F9 | 特性 | 终端页签/侧栏 CLI 品牌 logo（按命令行首 token 匹配 profile.iconSrc；页签 logo 会话绑定，侧栏行存在即显示） |
| F10 | 特性 | 编码套餐余量展示（导航树底部；user 层 settings.json env 判定套餐 → 后端定时查询推送；规格与计划产物已随实施完毕归档删除） |

> 测试策略概览见上方「测试策略」章节；完整用例清单见 `.claude/test-inventory.md`。
