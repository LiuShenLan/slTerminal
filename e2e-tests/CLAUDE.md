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

### E2E helper 命名与挂载位置

- `__slterm_e2e_*`：挂载在 `window` 全局；
- `__e2e_*`：挂载在终端容器 DOM 元素上（局部，随面板销毁而消失）。

两套命名反映挂载位置不同，禁止把 `__e2e_*` 当 window 全局使用。

### 用户目录隔离（FIX-TE-04 + E2E-05）

`run-wdio.cjs` 启动时备份，exit 时同步还原：
- `~/.slterminal/settings.json`
- `~/.claude/settings.json`
- `~/.slterminal/hooks/`
- `~/.slterminal/statusline-backup.json`
- `~/.slterminal/hooks-events/`（exit 时直接清理）

恢复失败收集为清单并设 `process.exitCode = 1`，禁止静默吞错。

### Spec 级项目/设置重置（TQ-E-08）

wdio 单 session 共享 app 实例。`wdio.conf.ts` 的 `beforeSuite` 调 `__slterm_e2e_resetProjects()` + `__slterm_e2e_resetSettings()`，防止跨 spec 累积触发 `MAX_PAGES=20` 上限。**不用 `beforeTest`**，否则会清掉 spec 内 `before()` 建的项目。`resetSettings` 不清 hooks 注入状态（hooks.e2e.ts 依赖 ensureHooksInjected 幂等）。

### 用例级重试

`wdio.conf.ts` 的 mocha `retries` 由 `WDIO_RETRIES` 驱动，默认 1；`WDIO_RETRIES=0` 用于 CI flakiness 观察面（TQ-E-09）。E2E-12 杀 app 用例在用例内显式 `this.retries(0)`。

## 外部坑/红线

- **禁止直接 `tauri build --debug` 跑 E2E**：必须 `VITE_E2E=1`。
- **禁止 build:e2e 与 wdio 并行**：cargo 无法覆写运行中的 exe。
- **fixture 缺失必须终止**：`fixtures/claude-projects/` 缺失时 `run-wdio.cjs` 直接 `process.exit(1)`，禁止自动兜底到真实 `~/.claude/projects`。
- **DOM 选择器必须用 `data-e2e`**：禁止 CSS 内联样式选择器。
- **helper 是测试后门而非用户路径**：真实用户交互由对应 L2 组件测试覆盖。

## 测试模式

- **运行**：`npm run e2e`（= `build:e2e && wdio`）。
- **单实例串行**：`maxInstances: 1`。
- **选择器**：`data-e2e` 属性。
- **通信**：测试代码在 Node 进程，通过 `browser.execute()` 调用应用侧注入的 window/容器全局 helper。
- **重试**：`WDIO_RETRIES` 环境变量控制，默认 1。

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
