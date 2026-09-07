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

### Node 版本兼容启动器

`npm run wdio` 实际由 `run-wdio.cjs` 启动。Node >= 26 时自动下载便携 Node 22（undici 8 与 webdriverio 不兼容），Node 22 直接运行。

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

### E2E helper 命名与挂载位置

- `__slterm_e2e_*`：挂载在 `window` 全局；
- `__e2e_*`：挂载在终端容器 DOM 元素上（局部，随面板销毁而消失）。

两套命名反映挂载位置不同，禁止把 `__e2e_*` 当 window 全局使用。

### glyph-repro 门控 spec 与参数透传（2026-09-06）

`glyph-repro.e2e.ts` 是 CM 字形绘制丢失取证/防复发 spec——现象只发生在真实 GPU 合成渲染路径（非整数 DPI 场景），默认套件**跳过**（`describe.skip`），取证/回归期显式启用：

```
GLYPH_E2E=1 node e2e-tests/run-wdio.cjs --spec glyph-repro.e2e.ts
```

`run-wdio.cjs` 支持把 CLI 参数透传 wdio（`--spec` 等）——单 spec 运行必须经启动器（Node 26→便携 22 切换兜底，裸 `npx wdio` 会踩版本坑）。启用时先读 spec 文件头注释（取证通道/断言层/环境探针语义——"e2e 不复现 ≠ 修复完成"的裁量依据）。

### 用户目录隔离（ADR-0016：假 home，替代 FIX-TE-04/E2E-05 备份/还原）

**数据目录隔离（SLTERM_DATA_DIR）**：`run-wdio.cjs` 启动时注入 `SLTERM_DATA_DIR = <os.tmpdir()>/slterm-e2e-data`（env 链式继承：run-wdio → npx wdio → tauri driver → slterminal.exe），应用全部数据写入（settings.json / slterminal-projects.json 等）落在临时目录，与日常使用数据完全隔离；退出时清理临时目录。

**假 home 隔离（USERPROFILE）**：E2E 会真实写盘用户 home 配置（hooks 注入 / statusLine 桥接 / 假 env），旧「备份 → run 后还原」机制存在窗口期污染（真实 claude 会话启动即读假 token，曾致 API 401 事故）与残留固化风险。现改为 `run-wdio.cjs` 建临时假屋 `<os.tmpdir()>/slterm-e2e-home` 并把 `USERPROFILE` 指向它——Node `os.homedir()`（libuv，每调重读）与 Rust 侧 `crate::home` 共享解析（env-first；dirs 6.0 Windows **不读 env**，收敛纪律见 src-tauri 侧文档）全链跟随，**真实用户目录零接触**，窗口期污染在机制层面消失。

**防复发校验**：覆盖 USERPROFILE 前对真实屋（`~/.claude/settings.json`、`~/.slterminal/statusline-backup.json`、`~/.slterminal/hooks/`）做存在性 + sha256 快照，exit 时逐项比对——任何泄漏（Rust 收敛遗漏/新裸 `dirs::home_dir()` 消费点）独立报红并 `exitCode = 1`。`~/.slterminal/hooks-events/` 仅当启动时不存在才校验 exit 仍不存在（存在 = 用户会话在用，跳过防误报）。

**真实屋零接触承诺范围** = `~/.claude` + `~/.slterminal`（假屋机制覆盖）。`%LOCALAPPDATA%` 的 WebView2 数据不在此承诺内（环境变量未动，与手动运行行为一致）。

**已知并发误报面**：开发者跑 e2e 的同时自己开真实 claude/slterminal 会合法改写 `~/.claude/settings.json` → exit 校验报红。属真实告警（提示「外部进程并发写真实屋」），错误消息列排查方向，不静默。

### Spec 级项目/设置重置（TQ-E-08）

wdio 单 session 共享 app 实例。`wdio.conf.ts` 的 `beforeSuite` 调 `__slterm_e2e_resetProjects()` + `__slterm_e2e_resetSettings()`，防止跨 spec 累积触发 `MAX_PAGES=20` 上限。**不用 `beforeTest`**，否则会清掉 spec 内 `before()` 建的项目。`resetSettings` 不清 hooks 注入状态（hooks.e2e.ts 依赖 ensureHooksInjected 幂等）。

### 用例级重试

`wdio.conf.ts` 的 mocha `retries` 由 `WDIO_RETRIES` 驱动，默认 1；`WDIO_RETRIES=0` 用于 CI flakiness 观察面（TQ-E-09）。E2E-12 杀 app 用例在用例内显式 `this.retries(0)`。

## 外部坑/红线

- **禁止直接 `tauri build --debug` 跑 E2E**：必须 `VITE_E2E=1`。
- **target/debug 的 exe 可能是 E2E 构建**：`npm run e2e` 覆盖构建产物（`VITE_E2E=1`，helper 被 tree-shake 与否以产物为准），日常使用该 exe 会跳过项目加载且带 E2E 后门——日常使用前须以普通 `npx tauri build --debug --no-bundle` 覆盖。
- **禁止 build:e2e 与 wdio 并行**：cargo 无法覆写运行中的 exe。
- **fixture 缺失必须终止**：`fixtures/claude-projects/` 缺失时 `run-wdio.cjs` 直接 `process.exit(1)`，禁止自动兜底到真实 `~/.claude/projects`。
- **DOM 选择器必须用 `data-e2e`**：禁止 CSS 内联样式选择器。
- **helper 是测试后门而非用户路径**：真实用户交互由对应 L2 组件测试覆盖。
- **`$()`/elementClick 触发 tauri-service 焦点检查（focusCommands）**：`$`/`$$`/`findElement`/`findElements`/`elementClick`/`getTitle` 命令前 `ensureActiveWindowFocus` 经 core.invoke 查窗口状态，查询不可用（WARN "core.invoke not available after 5s timeout"）时每命令 +5-15s（2026-09-06 实测：cli-aliases 真实手势版步骤 2 四个 focus 命令吃 40-60s，长链用例被拖出 mocha 60s 上限多轮失败）；executeScript **豁免**不触发。spec 编写优先 execute 内 helper；新引入 `$()` 元素命令前评估焦点检查成本。
- **合成 JS click 无焦点语义**：`browser.execute(() => el.click())` 不转移焦点、不触发 blur，测不到焦点转移类竞态；embedded driver 唯一真实输入 = elementClick（同受 focusCommands 惩罚）。交互时序断言（blur→click 竞态等）归 L2——jsdom `fireEvent` 可编排完整手势序列，见 `src/__tests__/CLAUDE.md`「blur/焦点时序竞态复现」。

## 测试模式

- **运行**：`npm run e2e`（= `build:e2e && wdio`）。
- **单实例串行**：`maxInstances: 1`。
- **选择器**：`data-e2e` 属性。
- **通信**：测试代码在 Node 进程，通过 `browser.execute()` 调用应用侧注入的 window/容器全局 helper。
- **重试**：`WDIO_RETRIES` 环境变量控制，默认 1。

### 失败排查提示（2026-09-06 实证）

- **mocha retry 吞首跑错误**：默认 `WDIO_RETRIES=1` 下报告只显示重跑失败的最后一个错误——重跑叠加首跑残留态（面板/别名/项目），错误行号与消息往往 ≠ 首跑真实失败点，会误导归因。暴露首跑真实错误用 `WDIO_RETRIES=0` 跑一轮（与 TQ-E-09 观察面同法）。
- **裸 "Error: Timeout"（@wdio/utils `executeAsync`）≠ waitUntil 超时**：前者是 mocha runnable 超时包装（`runnableTimeout - TIME_BUFFER` 后 reject），= 用例总时长超限（命令堆积/环境延迟），无 timeoutMsg；waitUntil 超时必带 timeoutMsg。长链用例按需 `(this as any).timeout(N)` 放宽（function 声明取 this，先例 E2E-12 `this.retries(0)`）。

### 半端到端边界声明（DOC-02）

以下用例不是完整 OS 级操作路径，但在真实二进制内跑通了监听/匹配/命令 handler/IPC/写盘全链路：
- **键盘**：合成 `keydown` dispatch → ShortcutRegistry window capture → 命令 handler → IPC。不真实处 = 事件来源不是 OS 按键（embedded WDIO 无法投递 `browser.keys`）。
- **侧栏视图拖拽跨区**：`__slterm_e2e_moveSideViewButton` 走 store 纯函数，等价落点，未触发真实 HTML5 DnD。
- **历史会话恢复编排**：断言到 `pty.write` 注入 `profile.history.buildRestoreInput` 输出，不进入真实 CLI 会话。

### 既定豁免

| 豁免项 | 原因 | 当前兜底 |
|--------|------|---------|
| 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys` 到 WebView2 | 合成事件 + 页面内 dispatch 全链路 |
| HTML postMessage 真实 WebView2 行为 | jsdom 无法模拟 opaque origin 与 CSP | `html.e2e.ts` 真实二进制往返 + L2 四负面用例 |
| WebGL / mouse tracking 回归 | headless 不跑 GPU；PASSTHROUGH_MODE 滚轮自动化假阴性 | `terminal.e2e.ts` 全屏 TUI 视觉回归 + L1 flags 守卫 |
| `E2E_ENABLED=false` 生产分支 | L2 恒 true，字面量 DCE 结构性缺口 | CI 生产 dist grep 守卫 + `e2e-build-config.test.ts` |
