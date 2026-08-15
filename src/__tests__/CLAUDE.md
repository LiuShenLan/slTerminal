# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 目录职责

L2 前端单元/集成测试——Vitest + jsdom 环境。覆盖路径工具、HtmlPanel、快捷键转发、注入脚本、CLI profile 注册表与四态策略、agent 状态/历史聚合、IPC 契约与 AC-5 字面量守卫等。

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

## 多 CLI profile 重构——测试文件更名映射全表（MC-8）

Stage 01–07 产生的测试文件更名/合并登记。旧名全部退役、磁盘零残留（Glob 已实查）；语义逐项并入新文件，用例数变化以 `.claude/test-inventory.md` 为准。

| 旧文件（退役） | 新文件（现行） | 语义去向 |
|----------------|----------------|----------|
| `tab-title-registry.test.ts`（13 用例） | `cli-profile-registry.test.ts` | 注册表行为并入（MC-102/108）：register/同 id 覆盖/get/getAll 注册序/matchByCommand（首 token 解析单点化、带参变体/空命令行/未命中）/`_reset`/单例 + logo 资源守卫泛化（遍历注册表全部 profile 断言 iconSrc 磁盘存在 + PNG 魔数） |
| `tab-rules.test.ts`（6 用例） | `cli-profile-claude.test.ts` | side-effect 注册 / 手动注册 / `_reset` 恢复语义并入（MC-104） |
| `cli-icons.test.ts`（12 用例） | `cli-profile-registry.test.ts` + `cli-profile-claude.test.ts` | 首 token 匹配/资源守卫语义拆分并入（MC-108 泛化，含 mockcli.png 先行资源断言） |
| `claude-status.test.ts`（32 用例） | `agent-status-lib.test.ts` + `cli-profile-claude.test.ts` | 拆分（MC-401）：lib 层 6 用例（AgentStatus 四态类型/STATUS_EMOJI/getStatusIcon）留 `agent-status-lib.test.ts`；eventToStatus 26 用例随实现迁 `profiles/claude/` hooks 策略（落点 `cli-profile-claude.test.ts`，语义不丢） |
| `ipc-hooks-contract.test.ts`（22 用例） | `ipc-agent-hooks-contract.test.ts` | 四维同步 + cliId 首参（MC-212）；经共享工厂 `describeIpcContract`（`helpers/ipc-contract.ts`）声明式驱动 |
| `ipc-claude-history-contract.test.ts`（8 用例） | `ipc-agent-history-contract.test.ts` | scan 无参 / delete `{cliId, sessionId}`（MC-306） |
| `claude-history-model.test.ts` | `agent-history-model.test.ts` | 目录迁移 + 复合键 `cliId\|sessionId`（MC-313） |
| `claude-history-hook.test.tsx` | `agent-history-hook.test.tsx` | 目录迁移（MC-310） |
| `claude-history-restore.test.ts` | `agent-history-restore.test.ts` | 目录迁移 + 恢复注入 = `profile.history.buildRestoreInput` 输出（MC-315） |
| `claude-history-row.test.tsx` | `agent-history-row.test.tsx` | 目录迁移 + 行 logo 按 `session.cliId` 查 profile.iconSrc（MC-311） |
| `claude-history-view.test.tsx` | `agent-history-view.test.tsx` | 目录迁移（MC-310） |
| `claude-history-action-dialog.test.tsx` | `agent-history-action-dialog.test.tsx` | 目录迁移（MC-310） |

## 新测试文件登记（Stage 07）

| 文件 | 覆盖 |
|------|------|
| `mock-cli-profile.test.tsx` | AC-4 五点全表 L2 用例：① OSC 133 命中页签标题/logo/agentSession.cliId ② eventToStatus/classifyNotification 被真实调用（spy 入参）③ 历史聚合条目 + 行 logo ④ hub 选择行两枚按钮 + 切换 + selectedCli 持久化 + **双向分派断言（KZ-7）**：选中 mockcli → 桩编辑器标记（data-e2e="mockcli-config-editor"）渲染 + JsonMode 零调用；选中 claude → JsonMode 被调用 + 桩标记不存在（含 claude 保存透传）⑤ 恢复注入 = mock 策略输出 |
| `no-claude-literals.test.ts` | AC-5 字面量守卫（L2 grep 形态）：fs 递归枚举通用层八路径（src/lib、src/panels/terminal、src/features/agentStatus、src/features/agentHistory、src/features/notifications、src/ipc、src/types、src/features/cliProfiles）扫描 .ts/.tsx——零 "claude" 字符串字面量/claude 事件名/`~/.claude` 路径；豁免形态 = `profiles/claude/` 导出常量 import 引用（CLAUDE_CLI_ID/SESSION_END_EVENT/EXIT_EVENT）+ **目录级豁免 `src/features/cliProfiles/profiles/claude/` 整目录（CS-2）**；含 `${}` 的模板字符串按字面量片段拼接后参与判定（CS-1，`cl${''}aude` 自检用例）；新增文件自动纳入 |
| `helpers/mockCliProfile.ts` | mockcli 测试夹具（AC-4 契约，决策 5）：`mockCliProfile` 定义（hooks/history 全能力桩，含 **KZ-7 configEditor 桩组件**——渲染 data-e2e="mockcli-config-editor"、props 签名 = HooksConfigEditorProps + **configLayers 单层桩声明** hint "mock" 区别于 claude 三层）+ `registerMockCliProfile`（claude 基线缺失时补注册）/ `resetCliProfileRegistry`（afterEach `_reset` + 恢复 claude 基线）+ `MOCK_CLI_RESTART_HINT` 桩文案；生产代码零引用 |

## 共享测试工厂（helpers/ + testMocks/ + setup.ts）

| 文件 | 职责 |
|------|------|
| `helpers/vfs.ts` | 虚拟文件系统辅助：makeVfs / mockEntry / findNode（explorer 刷新/清空用例复用） |
| `helpers/workspace-setup.ts` | store 重置 + 种子数据工厂：populateStore / resetStore / seedProject / setupTwoPages（explorer/workspace ~7 文件复用） |
| `helpers/xterm-test-utils.ts` | useXterm 测试工厂：rAF mock / 容器创建 / PTY output spy / ResizeObserver mock / 微任务 flush（use-xterm-* 复用） |
| `helpers/ipc-contract.ts` | IPC 契约共享工厂（IHE-06）：`describeIpcContract` 声明式驱动四维断言（命令名/参数结构/正常返回/异常传播），供 ipc-contract / ipc-hooks-config-contract / ipc-agent-hooks-contract / ipc-agent-history-contract 复用 |
| `helpers/mockCliProfile.ts` | mockcli 夹具（见上节登记） |
| `testMocks/explorerMocks.ts` | 文件浏览器 mock 接口定义（实现注册于 setup.ts 全局函数 `__createFsMocks` 等，供 vi.hoisted 使用） |
| `testMocks/xterm.ts` | xterm.js 共享 mock 工厂：createTerminalMock / createFitAddonMock / createWebglAddonMock（~6 文件复用，vi.mock 回调内正常 import） |
| `setup.ts` | 全局环境：共享工厂经 globalThis 暴露（`__createFsMocks`/`__createGitMocks`/`__createNotifyMocks`）+ 全局 vi.mock（`../ipc/notify`、`../ipc/agentHooks`（onAgentEvent 等，D-01 路径）、`@tauri-apps/api/window`）+ jsdom 补齐（getContext/crypto/ResizeObserver/matchMedia/document.fonts）。⚠️ vi.mock 遮蔽真实实现——需真实 ipc 的测试须在自身文件内 importOriginal 覆盖（照 ipc-contract.test.ts 先例） |

## 测试文件对应源码

| 测试文件 | 被测模块 |
|----------|---------|
| `html-panel.test.tsx` | `src/panels/html/HtmlPanel.tsx`（注入脚本 + postMessage 键盘转发 + origin 校验） |
| `inject-script.test.ts` | `src/lib/injectScript.ts` |
| `csp-config.test.ts` | `src-tauri/tauri.conf.json` |
| `cli-profile-registry.test.ts` | `src/features/cliProfiles/cliProfileRegistry.ts`（注册表五方法 + logo 资源守卫遍历） |
| `cli-profile-claude.test.ts` | `src/features/cliProfiles/profiles/claude/`（身份域 + hooks/history 策略） |
| `agent-status-lib.test.ts` | `src/lib/agentStatus.ts`（四态类型/STATUS_EMOJI/getStatusIcon） |
| `mock-cli-profile.test.tsx` | AC-4 五点全链路（useCommandDetection/useXterm/通知调度/agentHistory/hub 面板） |
| `no-claude-literals.test.ts` | AC-5 通用层八路径字面量守卫（profiles/claude 合法领地目录级豁免） |
| `ipc-agent-hooks-contract.test.ts` | `src/ipc/agentHooks.ts`（四命令 × 四维 + onAgentEvent） |
| `ipc-agent-history-contract.test.ts` | `src/ipc/agentHistory.ts`（scan 无参 / delete `{cliId, sessionId}`） |
| `agent-history-model.test.ts` 等 6 文件 | `src/features/agentHistory/`（model/hook/restore/row/view/action-dialog） |
