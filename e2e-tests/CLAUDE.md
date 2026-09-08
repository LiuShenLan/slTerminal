# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`e2e-tests/` 是 L4 端到端测试层，用 WDIO + Tauri embedded driver 在真实构建二进制上验证关键链路。embedded driver、构建门控、用户目录隔离、半端到端边界等事项无法从代码本身读出，必须文档化。

## 关键约束与决策

### Embedded driver 零 msedgedriver

WDIO 使用 `driverProvider: 'embedded'`，通过 `webview2-com` COM 直连 `ICoreWebView2`，不依赖外部 WebDriver server。

### 必须 `VITE_E2E=1` 构建

E2E helper 由 `E2E_ENABLED`（`src/lib/e2eEnabled.ts`）门控。`tauri build` 的前端恒走 production `vite build`，与 `--debug` 无关，必须经 `VITE_E2E=1` 才能保留 helper；否则 helper 被 tree-shake，wdio 全部卡在「Workspace 未就绪」。

### build:e2e 与 wdio 必须串行

`npm run e2e` 的 `&&` 已保证串行。手动或 CI 并行会导致 cargo 无法覆写被 wdio 占用的 `slterminal.exe`（os error 5），wdio 实际跑在旧二进制上（ACC-05 实证）。

### Node 版本（CP-003）

`npm run wdio` 由 `run-wdio.cjs` 启动，Node >= 22 直跑（webdriverio 9.30.0+ 已修复 Node 26 undici 8 兼容，webdriverio#15265）；`.temp/node22` 显式预置便携 Node 22 时优先使用（不自动下载，预置方法自行从 nodejs.org 取 node.exe 放入）。版本约束经根 package.json `engines.node ">=22"` 纳入主 toolchain。

### wdio 版本矩阵与 overrides 对齐契约（CP-032）

真值源：主声明在根 package.json devDependencies（@wdio/* `^9.30.1`、@wdio/globals `^9.31.0`、expect-webdriverio `^6.0.5`）；`@wdio/tauri-service@1.3.0` **硬钉** @wdio/* `9.29.1` + webdriverio `9.30.0`（package-lock 实测，上游约束）；overrides 段把家族强扭到主声明线——e2e 版本真值源 = 主声明 + overrides 两处，缺一即漂移。

各 override 成因（2026-09-06 挖掘补登记，来源 git log -S）：

| override | 成因 | 引入提交 |
|---|---|---|
| `serialize-javascript` `^7.0.5` | 消 mocha 传递依赖 RCE（npm audit high 阻断），经 @wdio/mocha-framework 传递 | a027b17（TE-01 批） |
| `deepmerge-ts` `^8.0.1` | @wdio/config/@wdio/utils/webdriver 传递依赖版本统一（dedupe） | a027b17 |
| `@puppeteer/browsers` `^3.2.1` | @wdio/utils 传递依赖版本统一（dedupe） | a027b17 |
| `glob` `^10.5.0` | @wdio/config/mocha/archiver-utils 传递依赖版本统一（dedupe） | a027b17 |
| `@wdio/globals` `^9.31.0` | 对齐 tauri-service 硬钉 9.29.1 → 主声明 | 1233336（TE-06/07/14） |
| `expect-webdriverio` `^6.0.5` | dedupe 对齐主声明 | 1233336 |
| `webdriverio` `^9.30.1` | 对齐 tauri-service 硬钉 9.30.0 → 主声明 | 1233336 |

**对齐契约**：
1. 升 `@wdio/tauri-service` 或任一 `@wdio/*` 主声明时，先 `npm view @wdio/tauri-service@latest dependencies` 查新版硬钉；硬钉与主声明不一致 → 更新 overrides 对应条目保持家族单实例；一致 → 删对应 override 条目（去 overrides 化）。
2. 版本评审看两处：主声明 `^` 浮动结果 + overrides 是否仍与主声明同线；`npm ls webdriverio @wdio/globals` 输出单实例即健康态。
3. 前 4 项（serialize-javascript/deepmerge-ts/@puppeteer/browsers/glob）为传递依赖治理，与 tauri-service 无关——wdio 升 major 时逐条重估是否仍需。

### 多 webview WDIO 可达性（S10-① spike 结论，2026-09-08 实测）

预览渲染迁独立 webview（CP-012 方向）的 E2E 可行性 spike 结论——embedded driver（tauri-plugin-wdio-webdriver 1.3.0 内嵌 WebDriver，会话/窗口模型 = app `webview_windows()` 注册表）对多窗口上下文的实测四问：

| 问 | 结论 | 证据 |
|---|---|---|
| ① webview 可枚举 | **yes** | `getWindowHandles()` = webview_windows label 全集；独立预览 WebviewWindow 启动即入列、销毁后出列（句柄 = label，实时反映注册表） |
| ② `browser.execute` 在预览上下文可达 | **yes** | `switchToWindow(preview-label)` 后 execute/`$` 作用于预览页上下文——asset 宿主页（href/title/内联脚本置位）与 data: 页均实测；预览页 `$('h1')` 取文 5ms |
| ③ 焦点语义 | **yes（命令与 OS 前台焦点解耦）** | 预览窗口 show+set_focus 后前台在预览、main `document.hasFocus()=false`，主窗口 execute/getTitle 仍正常；显式 switch 后 `$` 族无 +5s 焦点惩罚（实测 findMs=5ms） |
| ④ 销毁语义 | **yes** | `closeWindow(预览)` → 句柄集收缩、driver 存活；销毁为异步（注册表滞后瞬时，实测销毁后立即 enumerate 仍偶见残影）——后续命令先 `switchToWindow('main')` 即恢复，无挂死 |

**加载方式结论：asset 协议宿主页为 spike 期首选，② 落地面复核后改走自定义协议域（S10-② 定稿，ADR-0019）**——asset 页（`http://tauri.localhost`）在 tauri 2.11 无 per-webview CSP 下恒被注入全局 CSP（响应的 CSP 头 + 静态脚本哈希化），主窗口收紧 script-src（CP-012）后运行时内联注入与宿主自带脚本在 asset 域全灭；故预览宿主页改由 Rust 注册的自定义协议 `slterm-preview`（Windows 实为 `http://slterm-preview.localhost/…`）承载——响应不带全局 CSP →「预览 CSP 域」成立（origin 仍确定，注入机制与宿主脚本在该域宽松执行）。**data: 注入否决**（三重：全局 CSP meta 包裹改写 / origin opaque / 需 webview-data-url feature）与**跨窗口无 postMessage** 结论不变。

**架构约束（实测写死，S10-② 不得违背）**：
- **预览必须是独立 WebviewWindow（label 即驱动句柄）**；同窗口 `add_child` 子 webview 不仅不可枚举，且实测把宿主窗口整体挤出 `webview_windows()`（t+60s add_child 后 handles 变 `[]`）——多 webview-in-window 形态驱动侧无解（另需 tauri "unstable" feature）。
- **跨独立 WebviewWindow 无 window.postMessage 通道**（opener=null、无 WindowProxy，实测 main 收不到预览 postMessage）——消息桥只能走 Tauri event/IPC；CP-044 targetOrigin 议题随 postMessage 通道退役（备选结论分支落地，不引入 PREVIEW_ORIGIN）。
- **CSP 无 per-webview 覆盖**：tauri 2.11.5 builder API 无 csp 属性（CSP 为 app 级配置，data: 页同样被注入全局 CSP meta）——② 落地面复核结论：预览宿主页改走自定义协议域（`http://slterm-preview.localhost`，响应无 CSP），注入机制原样迁入域内（ADR-0019），替代原「'self' 可加载外部 js（asset 同源）」路线。
- **hasFocus 探针语义限单窗口**：`document.hasFocus()` 只反映被查窗口自身；预览可见/聚焦时 main 恒 false——TQ-E-10 类探针只适用于「将被真实交互的窗口」；预览相关用例一律 execute-first，不依赖探针。

**WDIO selector/驱动策略（预览相关 spec 编写契约）**：句柄 = label（embedded 模式）；`browser.switchToWindow('<固定 label>')` 切上下文（用户显式切换 → 服务侧抑制焦点自动恢复 → `$` 族命令不再吃 +5s）；每个预览面板 = 独立 WebviewWindow + 固定 label（如 `preview-<panelId>`），断言 execute-first，操作完切回 `'main'`；销毁预览 = `closeWindow()` 后先切回 main 再继续；全程不做任何 OS 级聚焦操作。

### E2E helper 命名与挂载位置

- `__slterm_e2e_*`：挂载在 `window` 全局；
- `__e2e_*`：挂载在终端容器 DOM 元素上（局部，随面板销毁而消失）。

两套命名反映挂载位置不同，禁止把 `__e2e_*` 当 window 全局使用。

### glyph-repro 门控 spec 与参数透传（2026-09-06）

`glyph-repro.e2e.ts` 是 CM 字形绘制丢失取证/防复发 spec——现象只发生在真实 GPU 合成渲染路径（非整数 DPI 场景），默认套件**跳过**（`describe.skip`），取证/回归期显式启用：

```
GLYPH_E2E=1 node e2e-tests/run-wdio.cjs --spec glyph-repro.e2e.ts
```

`run-wdio.cjs` 支持把 CLI 参数透传 wdio（`--spec` 等）——单 spec 运行必须经启动器（Node >= 22 直跑，裸 `npx wdio` 缺启动器的数据/假屋隔离链（SLTERM_DATA_DIR/USERPROFILE），禁止绕过启动器）。启用时先读 spec 文件头注释（取证通道/断言层/环境探针语义——"e2e 不复现 ≠ 修复完成"的裁量依据）。

### 用户目录隔离（ADR-0016：假 home，替代 FIX-TE-04/E2E-05 备份/还原）

**数据目录隔离（SLTERM_DATA_DIR）**：`run-wdio.cjs` 启动时注入 `SLTERM_DATA_DIR = <os.tmpdir()>/slterm-e2e-data`（env 链式继承：run-wdio → npx wdio → tauri driver → slterminal.exe），应用全部数据写入（settings.json / slterminal-projects.json 等）落在临时目录，与日常使用数据完全隔离；退出时清理临时目录。

**假 home 隔离（USERPROFILE）**：E2E 会真实写盘用户 home 配置（hooks 注入 / statusLine 桥接 / 假 env），旧「备份 → run 后还原」机制存在窗口期污染（真实 claude 会话启动即读假 token，曾致 API 401 事故）与残留固化风险。现改为 `run-wdio.cjs` 建临时假屋 `<os.tmpdir()>/slterm-e2e-home-<pid>`（per-pid 唯一名，免启动清空——句柄占用残留无害，OS 回收）并把 `USERPROFILE` 指向它——Node `os.homedir()`（libuv，每调重读）与 Rust 侧 `crate::home` 共享解析（env-first；dirs 6.0 Windows **不读 env**，收敛纪律见 src-tauri 侧文档）全链跟随，**真实用户目录零接触**，窗口期污染在机制层面消失。

**防复发校验（CP-046 键级断言）**：覆盖 USERPROFILE 前对真实屋快照——`~/.claude/settings.json` 取哨兵键（`hooks`/`statusLine`，本套件唯一可能写入的键）做存在性 + 值快照，exit 时键级比对，不做整文件 diff；`~/.slterminal/statusline-backup.json` 维持文件 sha256、`~/.slterminal/hooks/` 维持整树快照、`~/.slterminal/hooks-events/` 仅当启动时不存在才校验 exit 仍不存在（存在 = 用户会话在用，跳过防误报）。任何泄漏（Rust 收敛遗漏/新裸 `dirs::home_dir()` 消费点）独立报红并 `exitCode = 1`。哨兵键集合 = run-wdio.cjs `SETTINGS_SENTINEL_KEYS`——新增 settings.json 消费点时须同步向该数组加键（登记进本防复发节口径）。

**历史会话扫描根隔离（fixture 通道）**：run-wdio.cjs 每次运行从 `fixtures/claude-projects/`、`fixtures/mockcli-projects/`（CP-041 起）重建 `e2e-tests/.tmp-claude-projects/`、`e2e-tests/.tmp-mockcli-projects/` 副本，并把后端历史扫描根 env（`SLTERM_CLAUDE_PROJECTS_DIR` / `SLTERM_MOCKCLI_PROJECTS_DIR`——mockcli 为注册表 env 门控，仅 E2E 注入）指向副本——历史会话用例只触碰副本，不触真实 `~/.claude/projects`。fixture 会话 cwd 占位符 `__E2E_PROJECT_DIR__` 复制时替换为 E2E 临时项目目录（`SLTERM_E2E_PROJECT_DIR`，归属匹配）。fixture 维护说明与 UUID 常量同步纪律见各 fixture README。

**真实屋零接触承诺范围** = `~/.claude` + `~/.slterminal`（假屋机制覆盖）。`%LOCALAPPDATA%` 的 WebView2 数据不在此承诺内（环境变量未动，与手动运行行为一致）。

**已知并发误报面（已收窄，CP-046）**：开发者跑 e2e 的同时自己开真实 claude/slterminal 会合法改写 `~/.claude/settings.json` 的非哨兵键（env/permissions/用户配置）→ 键级校验放行，不再报红。并发 hooks 注入/卸载（写 `hooks`/`statusLine` 键）仍会命中哨兵键报红——属真实泄漏信号，排查方向照旧（Rust 侧 home 解析收敛），不静默。

### Spec 级项目/设置重置（TQ-E-08）

wdio 单 session 共享 app 实例。`wdio.conf.ts` 的 `beforeSuite` 调 `__slterm_e2e_resetProjects()` + `__slterm_e2e_resetSettings()`，防止跨 spec 累积触发 `MAX_PAGES=20` 上限。**不用 `beforeTest`**，否则会清掉 spec 内 `before()` 建的项目。`resetSettings` 不清 hooks 注入状态（hooks.e2e.ts 依赖 ensureHooksInjected 幂等）。

### 用例级重试

`wdio.conf.ts` 的 mocha `retries` 由 `WDIO_RETRIES` 驱动，默认 1；`WDIO_RETRIES=0` 用于 CI flakiness 观察面（TQ-E-09）。E2E-12 杀 app 用例在用例内显式 `this.retries(0)`。

## 外部坑/红线

- **禁止直接 `tauri build --debug` 跑 E2E**：必须 `VITE_E2E=1`。
- **target/debug 的 exe 可能是 E2E 构建**：`npm run e2e` 覆盖构建产物（`VITE_E2E=1`，helper 被 tree-shake 与否以产物为准），日常使用该 exe 会跳过项目加载且带 E2E 后门——日常使用前须以普通 `npx tauri build --debug --no-bundle` 覆盖。
- **禁止 build:e2e 与 wdio 并行**：cargo 无法覆写运行中的 exe。
- **fixture 缺失必须终止**：`fixtures/claude-projects/`、`fixtures/mockcli-projects/`（CP-041）缺失时 `run-wdio.cjs` 直接 `process.exit(1)`，禁止自动兜底——claude 兜底 = 回落真实 `~/.claude/projects`；mockcli 兜底 = provider 不注册（env 门控），mockcli 历史链路用例整组静默失效。
- **DOM 选择器必须用 `data-e2e`**：禁止 CSS 内联样式选择器。
- **helper 是测试后门而非用户路径**：真实用户交互由对应 L2 组件测试覆盖。
- **`$()`/elementClick 触发 tauri-service 焦点检查（focusCommands）**：`$`/`$$`/`findElement`/`findElements`/`elementClick`/`getTitle` 命令前 `ensureActiveWindowFocus` 经 core.invoke 查窗口状态，查询不可用（WARN "core.invoke not available after 5s timeout"）时每命令 +5-15s（2026-09-06 实测：cli-aliases 真实手势版步骤 2 四个 focus 命令吃 40-60s，长链用例被拖出 mocha 60s 上限多轮失败）；executeScript **豁免**不触发。spec 编写优先 execute 内 helper；新引入 `$()` 元素命令前评估焦点检查成本。运行前提 = 窗口前台聚焦——wdio.conf beforeSuite TQ-E-10 探针 fast-fail 保证（失焦即报错退出，不静默吃延迟）；前提满足后 `$` 族命令正常速度，cli-aliases 已回归真实手势（CP-030）。**2026-09-08 排查修正**：聚焦前提满足（TQ-E-10 通过）时窗口态查询通道仍可能不可用——本 app 构建页面 `__TAURI_INTERNALS__` 无 `.core` 成员，tauri-service 以 `__TAURI_INTERNALS__.core.invoke("plugin:wdio|get_window_states")` 查询恒失败（实测 TypeError + 5s 超时 WARN），每 focus 命令确定性 +5s、与窗口是否聚焦无关；cli-aliases 真实手势长链实测单轮 13 次 WARN（含 suite 清理）≈ +65s，经 suite 级 `this.timeout(120000)` 放宽预算（见失败排查提示节 timeout 生效形态）。新增含 `$`/click 的长链 spec 须照此预算。
- **合成 JS click 无焦点语义**：`browser.execute(() => el.click())` 不转移焦点、不触发 blur，测不到焦点转移类竞态；embedded driver 唯一真实输入 = elementClick（同受 focusCommands 惩罚）。交互时序断言（blur→click 竞态等）归 L2——jsdom `fireEvent` 可编排完整手势序列，见 `src/__tests__/CLAUDE.md`「blur/焦点时序竞态复现」。alias 添加链例外——blur→click 竞态经真实 elementClick 覆盖（CP-030），其余焦点类竞态仍归 L2。

## 测试模式

- **运行**：`npm run e2e`（= `build:e2e && wdio`）。
- **单实例串行**：`maxInstances: 1`。
- **定向运行官方形态 = 单 `--spec`**（2026-09-08 实证）：`run-wdio.cjs --spec a.e2e.ts --spec b.e2e.ts` 会为每个 spec 起独立 worker/独立应用实例——后起的应用实例在 Windows 前台锁定期内无法取得前台焦点，beforeSuite TQ-E-10 探针确定性快失败（与 spec 内容无关，换序后「谁在第 2 位谁失败」实证）；config `specs` 数组全量形态 = 单 worker 单 app 会话内顺序跑各 spec（无重启聚焦问题，S12 全量回归走此形态）。多 spec 验证请分次单 spec 调用。
- **选择器**：`data-e2e` 属性。
- **通信**：测试代码在 Node 进程，通过 `browser.execute()` 调用应用侧注入的 window/容器全局 helper。
- **重试**：`WDIO_RETRIES` 环境变量控制，默认 1。

### 失败排查提示（2026-09-06 实证）

- **mocha retry 吞首跑错误**：默认 `WDIO_RETRIES=1` 下报告只显示重跑失败的最后一个错误——重跑叠加首跑残留态（面板/别名/项目），错误行号与消息往往 ≠ 首跑真实失败点，会误导归因。暴露首跑真实错误用 `WDIO_RETRIES=0` 跑一轮（与 TQ-E-09 观察面同法）。
- **裸 "Error: Timeout"（@wdio/utils `executeAsync`）≠ waitUntil 超时**：前者是 mocha runnable 超时包装（`runnableTimeout - TIME_BUFFER` 后 reject），= 用例总时长超限（命令堆积/环境延迟），无 timeoutMsg；waitUntil 超时必带 timeoutMsg。长链用例放宽须在 **suite 级**设 `this.timeout(N)`（describe 回调内、用例执行前生效）；**用例体内 `this.timeout()` 无效**——@wdio/utils `executeAsync` 在用例体开始前一次性采样 runnable 超时并单独立竞时定时器，体内延长不改变该定时器（2026-09-08 实测，先例 E2E-12 `this.retries(0)` 同理须在 runnable 创建时生效）。
- **空数据目录启动无默认项目/页面 → Dockview 不挂载（2026-09-08 实证）**：`window.__dockviewApi` 恒指向活跃页面的 Dockview API；run-wdio 每轮清空数据目录，启动后须先创建项目/页面才挂载。`waitForDockviewApi()` 必须排在 `createProject()` 之后（先例 agent/mockcli；cli-aliases 2026-09-08 修正前在 createProject 前等待 → 确定性 20s 超时「Dockview API 未就绪」）。

### 半端到端边界声明（DOC-02）

以下用例不是完整 OS 级操作路径，但在真实二进制内跑通了监听/匹配/命令 handler/IPC/写盘全链路：
- **键盘**：合成 `keydown` dispatch → ShortcutRegistry window capture → 命令 handler → IPC。不真实处 = 事件来源不是 OS 按键（embedded WDIO 无法投递 `browser.keys`）。
- **侧栏视图拖拽跨区**：`__slterm_e2e_moveSideViewButton` 走 store 纯函数，等价落点，未触发真实 HTML5 DnD。
- **历史会话恢复编排**：断言到 `pty.write` 注入 `profile.history.buildRestoreInput` 输出，不进入真实 CLI 会话。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys` 到 WebView2 | 合成事件 + 页面内 dispatch 全链路 |
| 预览渲染往返（窗口创建/宿主页桥/iframe 执行/事件中继） | jsdom 无窗口/CSP/真实 iframe 执行——L2 止于 ipc mock 边界 | `html.e2e.ts`/`markdown.e2e.ts` 真实二进制 + switchToWindow 预览窗口往返（S10-② 驱动契约）+ L2 负面用例 |
| WebGL / mouse tracking 回归 | headless 不跑 GPU；PASSTHROUGH_MODE 滚轮自动化假阴性 | `terminal.e2e.ts` 全屏 TUI 视觉回归 + L1 flags 守卫 |
| `E2E_ENABLED=false` 生产分支 | L2 恒 true，字面量 DCE 结构性缺口 | CI 生产 dist grep 守卫 + `e2e-build-config.test.ts` |
