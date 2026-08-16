# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 目录职责

E2E 端到端测试 — 使用 WDIO + `@wdio/tauri-service` 1.1.0 + `tauri-plugin-wdio-webdriver` 1.1.0，embedded driver 模式（`webview2-com` COM 直连 `ICoreWebView2`，零 msedgedriver 依赖）。

## 命令

```bash
# 完整验收流程（必须先 build 再 wdio，二进制过期会导致结果不可靠）
npm run build:e2e     # = cross-env VITE_E2E=1 tauri build --debug --no-bundle
npm run wdio          # → node ./e2e-tests/run-wdio.cjs
# 或一步：npm run e2e（= build:e2e && wdio）
```

> **build:e2e 与 wdio 必须串行**：`npm run e2e` 的 `&&` 已保证；手动或自动化并行（如 workflow 测试 agent 并行跑多命令）时，cargo 无法覆写被 wdio 占用的 `slterminal.exe`——报 `failed to remove file ... slterminal.exe`（os error 5 拒绝访问），wdio 实际运行在旧二进制上结果不可靠（2026-08-08 ACC-05 fix-loop 实证）。多命令并行执行时排除此两命令或显式串行。

`npm run wdio` 由 `run-wdio.cjs` 启动：Node >= 26 时自动下载便携 Node 22（undici 8 与 webdriverio 不兼容），Node 22 直接运行。

> **必须 `VITE_E2E=1` 构建**：E2E helper 由 `E2E_ENABLED`（`src/lib/e2eEnabled.ts`）门控。`tauri build` 的前端恒走 `vite build`（production，`import.meta.env.DEV=false`，与 `--debug`/`--mode` 无关——`--debug` 只管 Rust 壳），故必须经 `VITE_E2E=1` 才能保留 helper。直接 `tauri build --debug` 会 tree-shake 掉 helper，wdio 全部卡在"Workspace 未就绪"。

## 文件结构

| 文件 | 用途 |
|------|------|
| `wdio.conf.ts` | WDIO 配置：local runner、mocha BDD、embedded driverProvider、单实例端口 4445、60s 超时、**用例级重试（E2E-15，mocha `retries: 1`；E2E-12 杀 app 用例在用例内 `this.retries(0)` 显式关闭）**。specs 通配 `./*.e2e.ts`（E2E-09 拆分后自动纳入），同一 worker 顺序执行（maxInstances=1，字母序即执行序——terminal.e2e.ts 末位承载 E2E-12 杀 app 用例） |
| `terminal.e2e.ts` | 终端 spec（7 条 active）：启动标题、PTY 通信+缓冲断言、E2E helper 写入读取、terminal-N 标题、**H6 跨页面存活**、**全屏 TUI 大负载 + 切页签往返（E2E-04 视觉回归——M2 人工验证点）**、**强杀 slterminal.exe → 子进程树无残留（E2E-12，KILL_ON_JOB_CLOSE 真实验证）** |
| `editor.e2e.ts` | 编辑器 spec（5 条 active）：编辑器标题 basename/同名冲突相对路径/关闭后重算、Ctrl+S 真实写盘（mtime 断言）、外部修改触发 reload 后保存（dirty→clean） |
| `html.e2e.ts` | HTML 面板 spec（1 active + 1 skip）：iframe Ctrl+W postMessage 转发关闭（真实二进制全链路）、**内联脚本/事件 CSP 执行验证**（skip，执行断言不稳定） |
| `sidebar.e2e.ts` | 侧栏视图 spec（2 条 active）：点击开关（R1/R2）、**跨区移动状态机（R6/R7——经 store helper，非真实 DnD，见定位声明）** |
| `commit.e2e.ts` | Commit 视图 spec（2 条 active）：真实 git 仓库（`gitScaffold.ts` 脚手架）变更列表渲染、双击 modified 打开 diff 页签 |
| `hooks.e2e.ts` | hooks spec（5 条 active）：注入/卸载/状态三态、信号文件驱动页签 emoji、**真实 hook reporter 链路（E2E-06：node 执行脚本 + stdin JSON + SLTERM_PANEL_ID → 信号文件产生/消费 + 非法 JSON exit 0 的 C10 守卫）**、hooksConfig project 层保存写盘 + merge 保留其他字段、**hub 注入按钮三态（Stage 06 D-14 段：hub 注入/卸载按钮 → 状态条流转并恢复；cliId 实参 = hub 选中态，E2E 构建仅 claude 一个 hasConfigEditor CLI）** |
| `agent.e2e.ts` | Agent 状态 spec（6 active + 1 skip）：视图存在性、纯 shell 终端无行、动态四态（agent-event 信号即建行→⚡→✅→行消失）、**R2 变体 = ContextUsage 信号通道全链路（官方 used_percentage 口径——PreToolUse 信号建行 → ContextUsage 信号 50% → 切项目往返行重建 → 再发信号 50% 恢复；原 transcript/contextUsage 链路已退役）**、R3/R4 变体、toast 触发链路（skip，权限弹窗需用户交互） |
| `history.e2e.ts` | 历史会话 spec（8 条 active）：fixture 6 行展示 + 排除规则、标题回退链、搜索过滤、复制恢复命令（剪贴板断言，`buildResumeCommand` 输出 = `cd '<cwd>' && claude --resume <id>`）、孤儿行 ✗、删除（ConfirmDialog 确认 + 副本删除）、历史区四态同源、**恢复编排（部分端到端：断言到 pty.write 注入 `profile.history.buildRestoreInput` 输出，不含真实进入会话；B14 追加真实渲染断言——主区 dockview 存在本页 terminal 前缀面板 + title=claude + 容器 offsetParent 非 null，修复 E2E 文本缓冲在 visible 门控前填充的黑屏脱靶盲区）** |
| `mockcli.e2e.ts` | mock profile spec（3 条 active：Stage 07 AC-4 ① 冒烟 + Stage 05 review-fix CS-3 两条关键路径）：冒烟 = `__slterm_e2e_registerMockCliProfile` 注册 mock 夹具 → OSC 133 C 注入（`__e2e_writeToTerminal` 走真实 parser + useCommandDetection → matchByCommand 命中 mockcli）→ 页签标题 "mockcli"（profile.tabTitle）+ 16×16 logo（profile.iconSrc）+ 🟡 attention → OSC 133 D 退出恢复（标题还原 + logo/图标双清）；**CS-3 ① agent-event 注入 = Node 侧原子写信号文件（cliId="mockcli" 显式，9 字段契约）→ 页签 ⚡ + Agent Status 活跃区建行（真实 watcher → agent-event → resolvePayloadCliId 三级解析 → 桩策略全链）**；**CS-3 ② hub 分派/保存透传 = hooksConfig 选择行 mockcli 按钮（`data-e2e="hooks-cli-mockcli"`）→ 点击渲染桩编辑器（`data-e2e="mockcli-config-editor"`）→ 桩保存触发真实 `writeHooksConfig("mockcli", ...)` → 后端「未知 cliId: mockcli」错误透传展示**。按 `data-panel-id` 精确定位面板（app 恢复用户布局多终端，防全局首匹配错位） |
| `specUtils.ts` | spec 共享工具（Node 侧，E2E-09）：Workspace/Dockview 就绪等待、项目/终端创建、PTY session 等待、hooks 注入（泛化命令 `agent_hooks_*` 六命令全表，一律经 window helper 调用、spec 侧无命令名字面量）、信号文件原子写与消费等待、页签 emoji 参数断言（`waitForPanelTabIcon`，F3 四态）、页面切换等待（waitUntil 替代 pause，E2E-10）、共享 setup `withProjectAndTerminal`。**与应用侧 `helpers.ts` 相互独立，二者禁止互相 import** |
| `gitScaffold.ts` | git 仓库脚手架（Node 侧）：`makeGitRepo` 按场景描述初始化真实 git 仓库（init/commit/modified/untracked），tempdir 隔离，execSync 调系统 git CLI |
| `fixtures/claude-projects/` | agent_history claude provider 的扫描 fixture（`SLTERM_CLAUDE_PROJECTS_DIR` env 覆盖指向的副本，env 覆盖留 provider 内部）：7 形态会话文件（custom-title/ai-title/prompt 回退/无 cwd/孤儿/agent-* 平铺/subagents 子目录）+ **README.md（E2E-13③）说明编码目录名/UUID 与 agent_history claude provider 排除规则的同步关系** |
| `run-wdio.cjs` | Node 版本兼容启动器 + 用户目录隔离备份/还原（见下节） |
| `helpers.ts` | 应用侧 E2E 辅助（`installAllE2eHelpers()` 统一注入 window 全局对象，见通信方式表；含 `__slterm_e2e_registerMockCliProfile`——mockcli 测试 profile 注册入口，Stage 07 AC-4；mockcli 定义含 **configEditor 桩**（CS-3：React.createElement 构造——.ts 无 JSX，`data-e2e="mockcli-config-editor"` 标记与 L2 桩同口径 + 保存按钮触发真实 `writeHooksConfig("mockcli", ...)`，错误展示于 `data-e2e="mockcli-config-error"`）+ **configLayers 桩**（单层 user）） |

## 配置要点

- 被测二进制路径：`./src-tauri/target/debug/slterminal.exe`（相对 `wdio.conf.ts`）
- `driverProvider: 'embedded'` — Tauri 内嵌 WebDriver，不走 msedgedriver
- `maxInstances: 1` — 单实例串行执行

## 加载机制

E2E helpers 通过 `main.tsx` 中 `E2E_ENABLED`（`src/lib/e2eEnabled.ts`）条件动态导入：dev serve 时 `import.meta.env.DEV=true`，E2E 构建时经 `VITE_E2E=1` 打开；生产发布构建二者皆 false → tree-shake 排除。同一开关门控 Workspace 就绪标志与终端级 helper（`useTerminalInstance`/`useXterm` 共 6 处）。CI 有生产 dist grep 守卫强制"生产不含 helper"。

## DOM 选择器约定

使用 `data-e2e` 属性定位元素（如 `data-e2e='terminal-container'`），禁止 CSS 内联样式选择器。

## 测试与应用的通信方式

测试代码运行在 Node 进程，通过 `browser.execute()` 在 WebView2 内执行 JS，依赖应用侧注入的 window 全局对象：

> 以下对象均由 `helpers.ts` 的 `installAllE2eHelpers()` 在 `E2E_ENABLED` 时（DEV serve 或 `VITE_E2E=1` 构建）动态挂载

| 全局对象 | 用途 |
|----------|------|
| `__slterm_e2e_workspaceReady` | Workspace 就绪标志 |
| `__slterm_e2e_createProject(path)` | 程序化创建测试项目（绕过原生对话框），**async**——返回 `Promise<string>`，内部先 `await setProjectRoot` 再 `setActivePage` |
| `__slterm_e2e_addPage(projId, name, rootPath)` | 在已有项目中创建新操作页面 |
| `__slterm_e2e_switchToPage(pageId)` | 切换活跃页面，**async**——返回 `Promise<void>`，内部先查 rootPath → `await setProjectRoot` → `setActivePage` |
| `__slterm_e2e_getProjectIdForPage(pageId)` | pageId 反查所属 projectId |
| `__slterm_e2e_getActivePageInfo()` | 获取活跃页面 ID 和 rootPath |
| `__slterm_e2e_registerAndRecompute` | 注册编辑器并重算标题（workspace 层辅助） |
| `__slterm_e2e_writeClipboard(text)` | 写入剪贴板（绕过 browser.execute 中裸模块解析） |
| `__slterm_e2e_shortcutDebug()` | 诊断：返回 `{ stack, commands }`（ShortcutRegistry 上下文栈 + 已注册命令 id） |
| `__slterm_e2e_getSideBarState()` | 返回 `useSideBar` 纯数据快照（`{zones, open, width, splitRatio, loaded}`），可安全经 `browser.execute` 序列化 |
| `__slterm_e2e_toggleSideView(id)` | 等价点击活动栏按钮，走 `store.toggleView(id)`（委托 `toggleViewPure`） |
| `__slterm_e2e_moveSideViewButton(id, zone, index)` | 等价拖拽落点，走 `store.moveButton(id, zone, index)`（委托 `moveButtonPure`）。zone 为 `"top"` 或 `"bottom"` |
| `__slterm_e2e_injectHooks` / `__slterm_e2e_uninstallHooks` / `__slterm_e2e_getHookInjectionStatus` | hooks 注入/卸载/状态三态。底层泛化命令 `agent_hooks_inject` / `agent_hooks_uninstall` / `agent_hooks_injection_status`（六命令全表），**cliId 实参固定 "claude"**——E2E 辅助属测试基建，字面量合法；随第二 CLI 接入再扩展 cliId 参数 |
| `__slterm_e2e_registerMockCliProfile` | mockcli 测试 profile 注册（Stage 07 AC-4）：mock 夹具 profile 进 `CliProfileRegistry`（register 幂等，同 id 覆盖）；hooks 全能力含 **configEditor 桩**（CS-3：`data-e2e="mockcli-config-editor"` 标记 + 保存按钮触发真实 `writeHooksConfig("mockcli", ...)`——mockcli 无后端 provider，错误透传即 cliId 全链携带证据）+ **configLayers 桩**（单层 user）；仅 E2E_ENABLED 构建存在，生产构建整块 tree-shake |
| `__dockviewApi` | Dockview 布局 API（`addPanel` 等） |
| `__e2e_sessionReady` | PTY session 就绪标志（挂载在终端容器 DOM 元素上，非 window 全局） |
| `__e2e_writeToPty(text)` | 向 PTY 写入文本（挂载在终端容器 DOM 元素上） |
| `__e2e_writeToTerminal(text)` | 直接向 xterm 缓冲写入（绕过 PTY）（挂载在终端容器 DOM 元素上） |
| `__e2e_getTerminalText()` | 读取 xterm 终端缓冲内容（挂载在终端容器 DOM 元素上） |

> **命名约定说明**：`__slterm_e2e_*` 前缀的对象挂载在 `window` 上（全局）；`__e2e_*` 前缀的对象挂载在终端容器 DOM 元素上（局部）。两套命名反映挂载位置不同——`__e2e_*` 对象随面板销毁而消失，不能作为 `window` 全局。未来可统一为 `__slterm_e2e_*` 前缀。

## 用户目录隔离机制（FIX-TE-04 + E2E-05 扩展）

`run-wdio.cjs` 启动时备份以下用户目录内容，`process.on('exit')` 中同步还原（还原前先 `rmSync` 删产物再 rename 备份，防残留 bak 致还原失败；node22 直跑 / 便携下载 / fallback 三路径均受 `exit` 钩子覆盖）：

| 目标 | 备份方式 | 说明 |
|------|----------|------|
| `~/.slterminal/settings.json` | 复制为 `.e2e-bak` | FIX-TE-04 原有——侧栏视图状态等 |
| `~/.claude/settings.json` | 复制为 `.e2e-bak` | E2E-05 新增——`agent_hooks_inject` 会写入 slterm matcher + statusLine 桥接，异常退出残留会污染用户配置 |
| `~/.slterminal/hooks/` | 整目录复制为 `.e2e-bak` | E2E-05 新增——注入的 reporter + statusline 桥接脚本目录；备份失败（目录占用）降级为 exit 时跳过还原 |
| `~/.slterminal/statusline-backup.json` | 复制为 `.e2e-bak` | statusline 桥接新增——注入备份的原 statusLine 配置，注入/卸载会写删此文件 |
| `~/.slterminal/hooks-events/` | exit 时清理 | 信号文件目录，运行产物直接删除 |

还原语义：原文件存在 → 删 E2E 产物后 rename 备份回来；原文件不存在 → 删产物 + 残留 bak。

> **决策 4（multi-cli 重构）**：E2E-05 备份集合保持 claude 硬编码（`~/.claude/settings.json` 等不按 CLI 泛化）——规格「二选一」取后者降范围；`run-wdio.cjs` 对应注释「随第二 CLI 接入扩展」。

> **AQ-4（fixture 缺失终止而非降级）**：`fixtures/claude-projects/` 缺失时 `run-wdio.cjs` 在 wdio 启动前以 `console.error` 明确文案 + `process.exit(1)` 终止——不设 `SLTERM_CLAUDE_PROJECTS_DIR` 会令后端回落真实 `~/.claude/projects`（生产默认），历史会话用例有触碰真实用户目录风险。禁止引入新降级路径（自动创建空 fixture、临时目录兜底等）。

## 已知无害噪声

`Tauri core.invoke not available after 5s timeout` — embedded 模式下降级到 WebDriver HTTP 协议时的日志，不影响测试结果。

## 定位声明：L4 = 半端到端 / 部分端到端（DOC-02，E2E-11 收编）

> 键盘、拖拽、恢复编排三类用例不是完整 OS 级端到端，统一定位为**半端到端**（应用内监听/匹配/命令 handler/真实 IPC/写盘全链路在真实二进制执行，唯一"不真实"处是事件来源或前置动作）。标注以下逐项边界，勿把这类用例视为真实用户操作路径的完整验证：

| 用例类别 | 实际路径 | 不真实处 | 兜底 |
|----------|----------|----------|------|
| 键盘（Ctrl+S / Ctrl+W / 终端按键） | 页面内 dispatch 合成 `keydown` → ShortcutRegistry window capture 真实捕获 → 命令 handler → 真实 IPC | 事件来源是 JS dispatch 而非 OS 键盘（embedded WDIO 无法投递 `browser.keys`，见下节） | L2 keyboard 系测试 + 真实 OS 按键豁免（见豁免表） |
| 侧栏视图拖拽跨区（R6/R7） | `__slterm_e2e_moveSideViewButton` 走 store 纯函数（`moveButtonPure`），等价拖拽落点 | 未触发真实 HTML5 DnD 事件链（jsdom/驱动能力所限） | `activityBar.test.tsx` L2 拖拽用例（含 drop index 断言） |
| 历史会话恢复编排 | 双击普通行 → 项目入列 + 页面切换 + 终端注入 `profile.history.buildRestoreInput` 输出（claude provider 策略 = `claude --resume <id>`，断言到 `pty.write` 注入） | 不断言 CLI 真实进入会话（fixture id 非真实） | 真实进入会话属人工验证（M 系列人工验证点） |
| E2E helper 类用例（`__slterm_e2e_createProject` 等） | 页面内直接调 store/workspace 层函数 | 绕过用户真实交互（对话框/拖拽/点击） | 对应组件 L2 测试覆盖交互路径 |

**应用侧 helpers 是测试后门而非用户路径**：`app.test.tsx` / `e2e-create-project.test.ts` 等 L2 用例验证的是 **E2E helper 行为契约**（pending 标记、localStorage 恢复交互），不是应用用户路径——它们守护的是 E2E 基建本身不漂移。

## 键盘输入限制（重要）

embedded WDIO 驱动**无法把 OS 级按键（`browser.keys`）投递进 WebView2 页面** —— `browser.keys` 发出的 keydown 不会到达页面 DOM。因此：

- 终端 `Ctrl+Shift+V` 用例发完按键后**直接写标记**（`__e2e_writeToTerminal`）验证终端可操作，并不断言真实按键效果。
- 编辑器 `Ctrl+S` 保存用例：`.click()` 聚焦 CodeMirror 在 headless WebView2 中不稳定，故改用**合成 `focusin` 事件**（`usePanelFocus` 监听的即是 focusin）激活 "editor" 上下文 + 设为聚焦编辑器，再在页面内 `browser.execute` dispatch 合成 `keydown` 到 `window`（重试循环直到写盘），由 `ShortcutRegistry` window capture 真实捕获 → `editor.save` → 真实 IPC `fs.writeFile`，以文件 **mtime 变化** 断言。dirty→clean 保存用例在此基础上先外部写盘触发 auto-reload，再修改内容后保存验证新内容已写盘。
- HTML `Ctrl+W` 用例：iframe sandbox 为 `allow-scripts`（不含 `allow-same-origin`），键盘转发经注入脚本 `postMessage({type:"slterm_key",...})` 到父 window。E2E 测试通过 `window.postMessage(...)` 模拟此路径（不访问 `iframe.contentDocument`）→ 父窗口 message handler → `exportContextBindings("global")` 比对 → `dispatchEvent(合成 KeyboardEvent)` → `ShortcutRegistry` window capture 真实捕获 → `global.closeTab` 关活跃面板，断言 `__dockviewApi.getPanel(id)` 变 `undefined`。`forwardGlobalShortcuts.ts` 已随 FE-13 删除。
- 唯一"不真实"处是事件来源为 JS dispatch 而非 OS 键盘（驱动能力所限）；监听/上下文匹配/命令 handler/写盘/转发全链路均在真实二进制中执行。
- 相关诊断 helper：`window.__slterm_e2e_shortcutDebug()` 返回 `{ stack, commands }`（上下文栈 + 已注册命令 id），用于断言快捷键设置正确。

## 豁免登记（DOC-01，L4 相关项）

> 完整豁免表见 `.claude/test-inventory.md`「既定豁免清单」——此处为 L4 侧相关项的模块级补充，三列（项目/豁免原因/当前兜底层级）以 test-inventory 为唯一真值源。

| 项目 | 豁免原因 | 当前兜底层级 |
|------|----------|--------------|
| L4 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys` 到 WebView2 页面（驱动能力所限） | 合成事件 + 页面内 dispatch 全链路（监听/上下文匹配/命令 handler/写盘真实执行）；未来 WDIO 支持真实输入时替换 |
| HTML postMessage 真实 WebView2 行为 | jsdom 无法模拟 opaque origin 序列化与 WebView2 CSP 强制（L2 只守 JS 侧形状） | `html.e2e.ts` Ctrl+W postMessage 往返（真实二进制）+ L2 四负面用例 |
| L3 生产 WebGL renderer / mouse tracking | headless 不跑 GPU；PASSTHROUGH_MODE 滚轮回归无法自动化（行为级测试假阴性） | `terminal.e2e.ts` 全屏 TUI 视觉回归（M2 人工确认）+ `compute_conpty_flags` 4 条守卫锁 0x7 |
| `E2E_ENABLED=false` 生产分支 | L2 恒 true，编译期字面量 DCE 结构性缺口 | CI 生产 dist grep 守卫 + `e2e-build-config.test.ts` 字面量断言（IHE-04） |

**视觉回归基线（E2E-04，M2 人工验证点）**：`terminal.e2e.ts` 全屏 TUI 大负载 + 切页签往返用例断言内容完整性与渲染器存活，但"WebGL→DOM 回退不白屏"属视觉判定——Stage 16 收尾时人工确认截图/渲染基线，此后回归由该用例持续守卫。
