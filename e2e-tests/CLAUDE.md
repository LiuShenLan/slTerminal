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

`npm run wdio` 由 `run-wdio.cjs` 启动：Node >= 26 时自动下载便携 Node 22（undici 8 与 webdriverio 不兼容），Node 22 直接运行。

> **必须 `VITE_E2E=1` 构建**：E2E helper 由 `E2E_ENABLED`（`src/lib/e2eEnabled.ts`）门控。`tauri build` 的前端恒走 `vite build`（production，`import.meta.env.DEV=false`，与 `--debug`/`--mode` 无关——`--debug` 只管 Rust 壳），故必须经 `VITE_E2E=1` 才能保留 helper。直接 `tauri build --debug` 会 tree-shake 掉 helper，wdio 全部卡在"Workspace 未就绪"。

## 文件结构

| 文件 | 用途 |
|------|------|
| `wdio.conf.ts` | WDIO 配置：local runner、mocha BDD、embedded driverProvider、单实例端口 4445、60s 超时 |
| `test.e2e.ts` | 测试用例（12 条，7 describe）：启动标题、终端 PTY 通信+缓冲断言、终端写入读取（E2E helper）、**H6 终端跨页面存活**、页签标题+冲突、**编辑器 dirty→clean 保存**、HTML iframe Ctrl+W 转发关闭、**HTML 内联脚本/事件 CSP 执行验证** |
| `run-wdio.cjs` | Node 版本兼容启动器 |
| `helpers.ts` | E2E 辅助函数（`installAllE2eHelpers()` 统一注入全局对象） |

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

> 以下对象均由 `helpers.ts` 的 `installAllE2eHelpers()` 在 DEV 模式下动态挂载

| 全局对象 | 用途 |
|----------|------|
| `__slterm_e2e_workspaceReady` | Workspace 就绪标志 |
| `__slterm_e2e_createProject(path)` | 程序化创建测试项目（绕过原生对话框） |
| `__slterm_e2e_addPage(projId, name, rootPath)` | 在已有项目中创建新操作页面 |
| `__slterm_e2e_switchToPage(pageId)` | 切换活跃页面 |
| `__slterm_e2e_getProjectIdForPage(pageId)` | pageId 反查所属 projectId |
| `__slterm_e2e_getActivePageInfo()` | 获取活跃页面 ID 和 rootPath |
| `__slterm_e2e_registerAndRecompute` | 注册编辑器并重算标题（workspace 层辅助） |
| `__slterm_e2e_writeClipboard(text)` | 写入剪贴板（绕过 browser.execute 中裸模块解析） |
| `__slterm_e2e_shortcutDebug()` | 诊断：返回 `{ stack, commands }`（ShortcutRegistry 上下文栈 + 已注册命令 id） |
| `__dockviewApi` | Dockview 布局 API（`addPanel` 等） |
| `__e2e_sessionReady` | PTY session 就绪标志（挂载在终端容器 DOM 元素上，非 window 全局） |
| `__e2e_writeToPty(text)` | 向 PTY 写入文本（挂载在终端容器 DOM 元素上） |
| `__e2e_writeToTerminal(text)` | 直接向 xterm 缓冲写入（绕过 PTY）（挂载在终端容器 DOM 元素上） |
| `__e2e_getTerminalText()` | 读取 xterm 终端缓冲内容（挂载在终端容器 DOM 元素上） |

> **命名约定说明**：`__slterm_e2e_*` 前缀的对象挂载在 `window` 上（全局）；`__e2e_*` 前缀的对象挂载在终端容器 DOM 元素上（局部）。两套命名反映挂载位置不同——`__e2e_*` 对象随面板销毁而消失，不能作为 `window` 全局。未来可统一为 `__slterm_e2e_*` 前缀。

## 已知无害噪声

`Tauri core.invoke not available after 5s timeout` — embedded 模式下降级到 WebDriver HTTP 协议时的日志，不影响测试结果。

## 键盘输入限制（重要）

embedded WDIO 驱动**无法把 OS 级按键（`browser.keys`）投递进 WebView2 页面** —— `browser.keys` 发出的 keydown 不会到达页面 DOM。因此：

- 终端 `Ctrl+Shift+V` 用例发完按键后**直接写标记**（`__e2e_writeToTerminal`）验证终端可操作，并不断言真实按键效果。
- 编辑器 `Ctrl+S` 保存用例：`.click()` 聚焦 CodeMirror 在 headless WebView2 中不稳定，故改用**合成 `focusin` 事件**（`usePanelFocus` 监听的即是 focusin）激活 "editor" 上下文 + 设为聚焦编辑器，再在页面内 `browser.execute` dispatch 合成 `keydown` 到 `window`（重试循环直到写盘），由 `ShortcutRegistry` window capture 真实捕获 → `editor.save` → 真实 IPC `fs.writeFile`，以文件 **mtime 变化** 断言。dirty→clean 保存用例在此基础上先外部写盘触发 auto-reload，再修改内容后保存验证新内容已写盘。
- HTML `Ctrl+W` 用例：因 `allow-same-origin` 与父同源，经 `iframe.contentDocument.dispatchEvent(合成 keydown)` 触发**真实的** `attachGlobalShortcutForwarder`（生产同一 handler）→ preventDefault + 重放到父 `window` → `global.closeTab` 关活跃面板，断言 `__dockviewApi.getPanel(id)` 变 `undefined`。
- 唯一"不真实"处是事件来源为 JS dispatch 而非 OS 键盘（驱动能力所限）；监听/上下文匹配/命令 handler/写盘/转发全链路均在真实二进制中执行。
- 相关诊断 helper：`window.__slterm_e2e_shortcutDebug()` 返回 `{ stack, commands }`（上下文栈 + 已注册命令 id），用于断言快捷键设置正确。
