# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 目录职责

L2 前端单元/集成测试——Vitest + jsdom 环境。覆盖路径工具、HtmlPanel、快捷键转发、注入脚本等。

> 所有用例数为快照，最新计数以 `.claude/test-inventory.md` 为准。

## HTML 面板测试（`html-panel.test.tsx`）

测试 HtmlPanel 组件的渲染状态、iframe 属性、竞态取消、注入脚本内容、postMessage 键盘转发。

### Mock 策略

- `src/ipc/fs`（readFile）、`features/shortcuts/ShortcutRegistry`（exportContextBindings）在 test 文件中 `vi.mock`
- mock 状态在 `vi.hoisted()` 创建，确保模块级 mock 执行前就绪
- 测试使用真实 `HtmlPanel` 导入（mock 之后），通过 `React.createElement` 渲染

### 关键测试模式

- **渲染状态**：readFile pending→loading、resolve→iframe（srcDoc）、reject→error（红色文字）
- **iframe 属性**：sandbox="allow-scripts"、srcDoc 含注入脚本、src 为 null
- **注入脚本验证**：srcDoc 字符串含 `slterm_key`、`scrollIntoView`、`classList.add/remove`、`dataset.sltermHash`、`createElement("style")`
- **postMessage 转发**：`window.postMessage({type:"slterm_key",fingerprint:"Ctrl+KeyW",...})` → `window.dispatchEvent` spy 验证
- **边界**：null data 不抛异常、缺 fingerprint 忽略、非 Error 类型 reject 走 String(err)、缺修饰键字段用 `?? false`

## injectScript 测试（`inject-script.test.ts`）

测试 `src/lib/injectScript.ts` 的 `injectScript(html, script, marker)` 纯函数——向 HTML 字符串注入脚本标签。

### 测试模式

- 纯函数测试：无 mock、无 jsdom、无 React，直接调用断言
- 覆盖三种注入策略：`</head>` → `<body` → `</html>` → 追加
- 幂等：已含 marker 不重复注入
- 边界：空字符串、null、空白字符、超大输入（500KB）、空 marker、空 script

## 快捷键转发与注入脚本验证

> `forwardGlobalShortcuts.ts` 已于 Stage 8 FE-13 删除。iframe 键盘转发改为 postMessage 方案（`HtmlPanel.tsx` 注入脚本 + 父窗口 `handleMessage` + `ShortcutRegistry` 分发）。

### INJECTED_SCRIPT 结构验证

`html-panel.test.tsx` 的注入脚本断言覆盖：
- 键盘转发：postMessage 目标 origin `"null"`、capture phase、keystroke 格式
- 片段拦截：`closest("a")`、`preventDefault`、`scrollIntoView`
- class toggle：`classList.add/remove`、`dataset.sltermHash`、同片段 toggle
- CSS 注入：`createElement("style")`、`.slterm-target`
- postMessage origin 校验 + source 校验 + 信任标记
- **注意**：测试中 INJECTED_SCRIPT 结构断言需与 `HtmlPanel.tsx` 注入脚本保持同步，否则测试通过但生产失败

## CSP 配置测试（`csp-config.test.ts`）

读取 `tauri.conf.json` 解析 CSP 策略字符串，断言关键决策：
- `script-src 'self' 'unsafe-inline'`（内联脚本/事件放行）
- `dangerousDisableAssetCspModification: ["script-src"]`（关 nonce 注入）
- `default-src 'self'`（严格同源）
- 未放宽到 `https:` 或 `*`

## 测试文件对应源码

| 测试文件 | 被测模块 |
|----------|---------|
| `html-panel.test.tsx` | `src/panels/html/HtmlPanel.tsx`（注入脚本 + postMessage 键盘转发 + origin 校验） |
| `inject-script.test.ts` | `src/lib/injectScript.ts` |
| `csp-config.test.ts` | `src-tauri/tauri.conf.json` |
