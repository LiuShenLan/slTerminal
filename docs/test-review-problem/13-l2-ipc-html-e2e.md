# L2 前端测试审查：IPC 层 + File Viewer/HTML + E2E 辅助门控

## 元信息

- **审查范围**：`src/ipc/` 及其 L2 契约测试、`src/features/fileViewers/`、`src/panels/html/`、E2E 辅助门控相关测试
- **测试文件**：按 `.claude/test-inventory.md` 口径，本次实际覆盖 15 个文件（用户原述 14 个，差异说明见下方）
  - IPC 契约：`src/__tests__/ipc-contract.test.ts`（65）、`src/__tests__/ipc-hooks-contract.test.ts`（22）、`src/__tests__/ipc-ping.test.ts`（1）、`src/__tests__/ipc-claude-history-contract.test.ts`（8）
  - HTML / File Viewer：`src/__tests__/html-panel.test.tsx`（40）、`src/__tests__/file-viewer-registry.test.ts`（25）、`src/__tests__/csp-config.test.ts`（4）
  - E2E 辅助 / 门控：`src/__tests__/e2e-enabled.test.ts`（2）、`src/__tests__/e2e-build-config.test.ts`（6）、`src/__tests__/app.test.tsx`（5）、`src/__tests__/e2e-clipboard-helper.test.ts`（3）、`src/__tests__/e2e-create-project.test.ts`（3）、`src/__tests__/dialog-e2e-hook.test.ts`（3）、`src/__tests__/e2e-gating-workspace.test.tsx`（2）、`src/__tests__/error-boundary.test.tsx`（3）、`src/__tests__/e2e-gating-terminal.test.ts`（5）
- **用例数**：约 196 条（与清单口径一致）
- **被测源码**：`src/ipc/*.ts`、`src/panels/html/HtmlPanel.tsx`、`src/features/fileViewers/FileViewerRegistry.ts`、`src/lib/e2eEnabled.ts`、`src/lib/ErrorBoundary.tsx`、`e2e-tests/helpers.ts`
- **说明**：仓库中另有 `src/__tests__/ipc-hooks-config-contract.test.ts`（12 用例）与 `src/__tests__/e2e-gating-terminal.test.ts`（5 用例）同处本领域；前者未在用户列出的 14 个文件中，但因它测试 `src/ipc/hooksConfig.ts`，本次作为相邻文件纳入观察但不计入核心评分；后者已在清单中。

---

## 覆盖率缺口（按业务风险分级）

| 源文件 | 行覆盖率 | 未覆盖分支 / 函数 | 风险等级 | 说明 |
|--------|----------|-------------------|----------|------|
| `src/ipc/index.ts` | 0% | `ping()` | 🟢 低 | barrel 导出的健康检查包装函数无人调用；`ipc-ping.test.ts` 直接 `invoke('ping')`，未测 wrapper。 |
| `src/ipc/notification.ts` | 14.3% | `sendToastNotification` 异常分支、`ensureNotificationPermission` 拒绝/异常分支 | 🟡 中 | toast 失败静默吞错，单元测试从未触发 catch；权限被拒绝路径未验证。 |
| `src/ipc/window.ts` | 58.8% | `onFocusChanged`、`setFocus` | 🟡 中 | 窗口 focus 事件监听与设置函数未测；若 Tauri API 行为变化无法早期发现。 |
| `src/ipc/dialog.ts` | 100% 行 | line 27 `if (E2E_ENABLED)` false 分支 | 🟢 低 | 生产构建路径（E2E_ENABLED=false）无法在 L2 命中，属结构性 DCE 缺口；L4 间接覆盖。 |
| `src/panels/html/HtmlPanel.tsx` | 100% 行 | line 140 `err instanceof Error` false 分支 | 🟢 低 | 异常为非 `Error` 实例时回退 `String(err)` 的分支未命中。 |
| `src/features/fileViewers/FileViewerRegistry.ts` | 95.7% | `_reset()` | 🟢 低 | 仅测试隔离使用的 `_reset` 在常态测试中被调用，但覆盖率工具显示其体未命中。 |
| `src/lib/e2eEnabled.ts` | 100% 行 | 二元表达式 false 分支 | 🟢 低 | `computeE2eEnabled` 的 false 分支由常量一致性测试覆盖，生产 tree-shake 非 L2 能测。 |
| `src/lib/ErrorBoundary.tsx` | 90.9% | line 52 `inline` variant 分支 | 🟡 中 | `variant="inline"` 渲染路径未测；该组件在 UI 中存在被 inline 使用的可能。 |

> 覆盖率数据由 `node docs/test-review-problem/coverage/extract-uncovered.cjs <path>` 提取。

---

## 问题列表

### 一、IPC 契约测试存在“mock 代替实现”的结构性盲区

🔴 **P1：mockIPC 绕过真实 Tauri 序列化，camelCase/snake_case、Channel、Uint8Array 等转换只在 mock 层断言**

- **位置**：`src/__tests__/ipc-contract.test.ts`、`src/__tests__/ipc-hooks-contract.test.ts`、`src/__tests__/ipc-claude-history-contract.test.ts`
- **表现**：测试通过 `mockIPC` 拦截 `invoke`，断言 `command` 名字符串、`args` 结构。例如 `pty.write` 断言 `data: [0x48, 0x69]` 是 `number[]`，但并未验证真实 Tauri IPC 对 `Uint8Array` 的处理；`pty.spawn` 断言 `onOutput` 是 `Channel` 实例并绑定 `onmessage`，但并未验证真实 `Channel<T>` 的序列化/反序列化。
- **风险**：若某次升级 Tauri 或写错 wrapper，mock 层仍会通过，真实 IPC 在 L4 才能暴露。`ipc-contract.test.ts` 的 DBG-4 守卫本身很薄弱——它只检查 mock 返回的字段名，不检查端到端字段映射。
- **建议**：L2 无法替代 L4，但应在 L2 增加“wrapper 行为契约”测试：例如手动构造一个最小对象传入 wrapper，验证它确实调用 `invoke` 并传入预期结构；同时明确文档化 IPC 契约测试只能防“wrapper 写错命令名/参数结构”，不能防“Tauri 序列化变化”。

### 二、IPC 源文件存在未覆盖或浅覆盖模块

🟡 **P2：`src/ipc/notification.ts` 仅被直接 import，异常与权限分支未测**

- **位置**：`src/ipc/notification.ts:38-47`
- **表现**：`sendToastNotification` 对 `sendNotification` 做 try/catch，catch 分支未命中；`ensureNotificationPermission` 的 `requestPermission` 返回 false 或抛异常的路径未测。
- **风险**：toast 失败静默吞错是设计选择，但若未来引入“通知失败回退到任务栏闪烁”等业务逻辑，无测试守卫。
- **建议**：补充 `notification.test.ts`，mock `@tauri-apps/plugin-notification` 返回拒绝/异常，验证行为符合预期。

🟡 **P3：`src/ipc/window.ts` 的 focus 相关 API 完全未测**

- **位置**：`src/ipc/window.ts:13-44`
- **表现**：`registerCloseHandler` 有测试覆盖（通过 `src/__tests__/app.test.tsx` 或 `ipc-contract` 中的 `window.registerCloseHandler`），但 `onFocusChanged`、`setFocus` 无任何调用点。
- **风险**：Tauri 2 Window API 若变更（如 `onFocusChanged` 返回 unlisten 结构变化），L2 无法发现；功能本身也缺乏回归用例。
- **建议**：若 `onFocusChanged`/`setFocus` 当前未被使用，考虑删除或明确标记为预留；若保留，补充最小契约测试。

🟢 **P4：`src/ipc/index.ts` 的 `ping()` wrapper 0% 覆盖**

- **位置**：`src/ipc/index.ts:19-21`
- **表现**：`ipc-ping.test.ts` 直接调用 `invoke('ping')`，未使用 `src/ipc/index.ts` 导出的 `ping()`。
- **风险**：barrel 文件导出路径若被删除或改名，只有 import 侧编译错误能发现，L2 无覆盖。
- **建议**：`ipc-ping.test.ts` 改为 `import { ping } from '@/ipc'` 并调用 `ping()`，直接覆盖 wrapper。

🟢 **P5：生产构建分支无法被 L2 命中**

- **位置**：`src/ipc/dialog.ts:27`、`src/lib/e2eEnabled.ts:9`
- **表现**：`E2E_ENABLED` 在 L2 测试环境中恒为 `true`（`import.meta.env.DEV=true`），`dialog.ts` 中 `if (E2E_ENABLED)` 的 false 分支、`e2eEnabled.ts` 中 `import.meta.env.VITE_E2E === "1"` 的 false 分支覆盖率均为 0。
- **风险**：这是结构性缺口，不是测试写得不好。真正的风险在于 L2 测试的是“dev/E2E 模式”，不是“生产模式”。
- **建议**：文档化说明；由 CI 的 `dist grep` 和生产构建 L4 兜底，无需在 L2 硬补。

### 三、HTML 面板测试深度不足，真实 WebView2 行为难以在 jsdom 验证

🟡 **P6：postMessage origin/source 三层校验的 L2 测试使用 jsdom 模拟，无法代表真实 WebView2**

- **位置**：`src/panels/html/HtmlPanel.tsx:117-156`、`src/__tests__/html-panel.test.tsx`
- **表现**：SEC-03 要求 `e.origin === "null"`、`e.source === iframe.contentWindow`、信任标记三层校验。L2 用 `new MessageEvent('message', { origin: 'null', source: ... })` 模拟，且未覆盖 `source` 不匹配、`origin` 为其他值、`source` 为 `null`、多 iframe 共存等场景。
- **风险**：真实 WebView2 中 opaque origin 序列化是否真的是 `"null"`、跨 iframe 攻击模型、Tauri 是否仍可能注入脚本等，L2 无法给出结论。文档已说明由 L4 验收，但 L2 的“三层校验”测试给人虚假安全感。
- **建议**：在 L2 明确标注这些用例为“jsdom 模拟，非真实 WebView2”；增加负面用例覆盖 `origin !== 'null'`、`source !== contentWindow`、`type !== 'slterm_key'`、未知 fingerprint 等路径。

🟢 **P7：注入脚本逻辑仅做字符串包含检查，未验证注入位置与执行语义**

- **位置**：`src/panels/html/HtmlPanel.tsx:30-75`、`src/lib/injectScript.ts`
- **表现**：`html-panel.test.tsx` 的 E5/E6/E7 断言 `srcDoc` 包含 `"slterm_key"`、`"scrollIntoView"` 等字符串，但未验证：
  - `injectScript` 是否把脚本放到了正确的位置（`</head>` 前、`<body` 前、追加末尾）；
  - `</script>` 转义是否正确；
  - 注入脚本内部事件监听器的绑定顺序、阻止默认行为、postMessage 字段构造是否正确。
- **风险**：重构 `INJECTED_SCRIPT` 或 `injectScript` 时，只要保留关键词就可能让测试通过而实际行为破坏。
- **建议**：`html-panel.test.tsx` 应断言完整注入脚本片段（或至少关键控制流）；`inject-script.test.ts`（如果存在）应补充位置/转义/幂等用例。若尚无 `inject-script.test.ts`，应创建。

🟢 **P8：HtmlPanel 异常分支 `err instanceof Error` false 路径未覆盖**

- **位置**：`src/panels/html/HtmlPanel.tsx:140`
- **表现**：`E12` 用例抛字符串 `"权限不足"`，理论上会命中 `String(err)` 分支，但覆盖率报告显示 false 分支命中 0 次。
- **风险**：可能是测试未真正走到该分支（例如被外层 catch 提前处理），或覆盖率工具有误。需要确认。
- **建议**：复跑 `html-panel.test.tsx` 的 E12/E13 并确认覆盖率；若确实未命中，修复用例断言。

🟢 **P9：CSP 配置测试只检查 script-src，未覆盖 style-src / connect-src / img-src 等**

- **位置**：`src/__tests__/csp-config.test.ts`、`src-tauri/tauri.conf.json`
- **表现**：测试断言 `default-src 'self'`、`script-src 'self' 'unsafe-inline'`、`dangerousDisableAssetCspModification: ["script-src"]`，但未断言 `style-src`、`connect-src`、`img-src` 等。
- **风险**：如果未来收紧 `style-src` 导致 CSS 内联失效，或放宽 `connect-src` 引入远程连接，L2 不会报警。
- **建议**：扩展测试为“完整 CSP 字段快照”或至少断言关键字段不变；如果某些字段允许演进，应使用白名单/黑名单组合。

### 四、File Viewer Registry 测试遗漏边界与异常输入

🟢 **P10：`FileViewerRegistry` 对非法/边界扩展名输入缺乏测试**

- **位置**：`src/features/fileViewers/FileViewerRegistry.ts`
- **表现**：测试覆盖了大小写、隐藏文件、带参路径、覆盖注册，但未覆盖：
  - 路径以 `.` 结尾或只有 `.` 的文件名；
  - 注册空字符串扩展名 `extensionStrategy.register("", ...)`；
  - `_reset()` 在测试间的行为（覆盖率显示未命中）。
- **风险**：`_reset()` 是测试隔离依赖，如果它意外被删除或行为变化，测试可能互相污染。
- **建议**：补充 `_reset()` 用例；补充 `resolve(".gitignore")`、`resolve("file.")` 等边界。

### 五、E2E 辅助 / 门控测试多为“配置读取”与“mock 验证”，深度不足

🟡 **P11：E2E_ENABLED 门控无法在生产构建路径上验证 tree-shake**

- **位置**：`src/lib/e2eEnabled.ts`、`src/__tests__/e2e-enabled.test.ts`、`src/__tests__/e2e-build-config.test.ts`
- **表现**：L2 只能验证 `computeE2eEnabled` 真值表和 `E2E_ENABLED` 常量一致性；真正的“生产二进制不含 helper”由 CI grep 守卫。
- **风险**：若 `E2E_ENABLED` 不再是编译期常量（例如被包装成函数调用），Rollup 无法 DCE，生产可能带测试后门，L2 不会报警。
- **建议**：增加一个 L2 测试，断言 `e2eEnabled.ts` 中 `E2E_ENABLED` 是字面量表达式（可通过 AST 或简单正则）；该测试已在 `e2e-build-config.test.ts` 部分体现，但可更明确。

🟡 **P12：`e2e-gating-terminal.test.ts` 的 mock 包含模块未导出的 `hooks` 字段**

- **位置**：`src/__tests__/e2e-gating-terminal.test.ts:20`
- **表现**：`vi.mock('@/lib/e2eEnabled', ...)` 返回 `{ E2E_ENABLED: true, hooks: { onHookEvent: vi.fn() } }`，但 `src/lib/e2eEnabled.ts` 实际只导出 `E2E_ENABLED` 和 `computeE2eEnabled`。
- **风险**：mock 中的 `hooks` 是虚假字段，可能误导后续维护者认为 `e2eEnabled.ts` 导出了 hooks； TypeScript 若严格检查会报错。
- **建议**：删除 mock 中的 `hooks` 字段，仅保留真实导出的字段。

🟡 **P13：`error-boundary.test.tsx` 未覆盖 `variant="inline"` 分支**

- **位置**：`src/lib/ErrorBoundary.tsx:51-62`
- **表现**：仅测试了 `variant="fullscreen"`（默认），未测试 inline 渲染路径。
- **风险**：若 inline 模式的样式/文案被改错，只有 UI 回归或 L4 能发现。
- **建议**：补充 inline variant 的渲染用例。

🟢 **P14：`app.test.tsx` 与 `e2e-create-project.test.ts` 测试的是 helper 模拟函数，不是真实 App 初始化逻辑**

- **位置**：`src/__tests__/app.test.tsx`、`src/__tests__/e2e-create-project.test.ts`
- **表现**：`app.test.tsx` mock 了整个 `App.tsx` 模块并验证 `__slterm_e2e_createProject` 的行为；`e2e-create-project.test.ts` 测试 `simulateInitRestore` 辅助函数与 localStorage 交互。
- **风险**：这些测试无法发现真实 `App.tsx` 在 E2E 模式下是否真正挂载 helper、localStorage 键名是否一致、`projectPending` 竞态是否真实存在。
- **建议**：文档化这些是“helper 行为契约”测试；若可能，增加对 `main.tsx` E2E 导入路径的静态断言。

🟢 **P15：dialog E2E hook 测试无法覆盖生产构建的 false 分支**

- **位置**：`src/__tests__/dialog-e2e-hook.test.ts`、`src/ipc/dialog.ts`
- **表现**：与 P5 同构；测试只能验证 E2E_ENABLED=true 且 hook 已挂载/未挂载两条路径。
- **建议**：同 P5，由 L4/CI 兜底，L2 标注限制。

### 六、可维护性与重复代码

🟢 **P16：四个 IPC 契约测试文件高度重复，可参数化**

- **位置**：`src/__tests__/ipc-*.test.ts`
- **表现**：每个测试文件都重复“mockIPC → commandSpy → 调用 wrapper → 断言命令名/参数/返回值/异常”的模式。
- **风险**：新增一个 IPC wrapper 需要复制粘贴大量模板，容易漏测异常传播或参数结构。
- **建议**：抽取 `test/helpers/ipc-contract.ts` 工厂函数，每个 wrapper 只需声明 schema。

🟢 **P17：`html-panel.test.tsx` 存在大量重复 waitFor 与选择器模式**

- **位置**：`src/__tests__/html-panel.test.tsx`
- **表现**：多处 `await waitFor(() => expect(...).toBe(...), { timeout: 3000 })`、`getByTitle('加载中')` 等重复。
- **风险**：测试可读性差，维护成本高。
- **建议**：提取 `waitForLoaded`、`waitForError` 等局部 helper。

---

## 已做变异推演的用例清单

以下对核心用例做“假设代码被改错，测试是否仍能发现”的推演。通过者标注 ✅，漏网者标注 ⚠️ 并说明补充方向。

### IPC 契约测试

| 用例 | 变异假设 | 推演结果 | 说明 |
|------|----------|----------|------|
| `ipc-contract: pty.spawn 绑定 Channel` | 删除 `channel.onmessage = onOutput` | ✅ 能发现 | 测试断言 `channelArg.onmessage === onOutput` |
| `ipc-contract: pty.write Uint8Array 转 number[]` | 把 `Array.from(data)` 改回 `data` | ✅ 能发现 | 测试断言 `data` 是数组 |
| `ipc-contract: 命令名 snake_case` | `pty_spawn` 误写成 `ptySpawn` | ✅ 能发现 | 测试断言 command 字符串 |
| `ipc-contract: camelCase payload 键` | `sessionId` 写成 `session_id` | ✅ 能发现 | 测试断言 args 结构 |
| `ipc-contract: onFsEvent payload 解包` | `listen` 回调不解包 `event.payload` | ⚠️ 可能漏网 | mockIPC 不调用真实 listen，只能验证 callback 被传入，无法验证运行时解包 |
| `ipc-hooks-contract: onHookEvent 解包 | 同上 | ⚠️ 可能漏网 | 同 onFsEvent |
| `ipc-hooks-contract: contextUsage 参数 camelCase` | `transcriptPath` 写成 `transcript_path` | ✅ 能发现 | 测试断言 payload 键名 |
| `ipc-claude-history-contract: delete sessionId 校验` | 删除 UUID 校验 | ⚠️ 可能漏网 | L2 只验证 wrapper 传入 `sessionId`，真实校验在 Rust 端，需 L1/L4 |

### HTML 面板测试

| 用例 | 变异假设 | 推演结果 | 说明 |
|------|----------|----------|------|
| `html-panel: sandbox="allow-scripts"` | 改成 `allow-scripts allow-same-origin` | ✅ 能发现 | 测试断言 sandbox 属性字符串 |
| `html-panel: postMessage origin 校验` | 把 `e.origin === "null"` 改成 `"*"` | ✅ 能发现 | 测试验证非 null origin 不 dispatch |
| `html-panel: 注入脚本包含 slterm_key` | 把 `type: "slterm_key"` 改成 `"key"` | ⚠️ 可能漏网 | 测试只检查字符串包含，不检查事件对象结构 |
| `html-panel: handleMessage 信任标记` | 删除 `__slterm_postMessage` 属性定义 | ✅ 能发现 | 测试断言 event 上该属性为 true |
| `html-panel: 片段拦截 scrollIntoView` | 删除 `scrollIntoView` 调用 | ⚠️ 可能漏网 | 测试未 spy scrollIntoView，只检查 classList |
| `html-panel: 错误态显示` | 错误信息文案改错 | ✅ 能发现 | 测试断言具体文案 |

### File Viewer Registry 测试

| 用例 | 变异假设 | 推演结果 | 说明 |
|------|----------|----------|------|
| `file-viewer-registry: .html → htmlviewer` | 把扩展名映射改成 `.htm` | ✅ 能发现 | 测试覆盖 `.html`/`.htm` 两者 |
| `file-viewer-registry: 大小写不敏感` | 改成大小写敏感 | ✅ 能发现 | 有 `.HTML` 用例 |
| `file-viewer-registry: 隐藏文件排除` | 删除 `name.startsWith('.')` 分支 | ✅ 能发现 | 有 `.gitignore` 用例 |
| `file-viewer-registry: 链式短路` | 把所有策略返回 null 才回退改成取第一个非 null 与 null 合并 | ✅ 能发现 | 测试覆盖注册多个策略时的短路行为 |

### E2E 门控测试

| 用例 | 变异假设 | 推演结果 | 说明 |
|------|----------|----------|------|
| `e2e-enabled: 真值表` | `computeE2eEnabled` 逻辑改错 | ✅ 能发现 | 测试覆盖 DEV/VITE_E2E/生产三态 |
| `e2e-enabled: 常量与函数一致` | `E2E_ENABLED` 不再调用 `computeE2eEnabled` 等价表达式 | ✅ 能发现 | 测试断言二者相等 |
| `e2e-build-config: package.json script 存在` | 删除 `build:e2e` | ✅ 能发现 | 测试断言脚本存在 |
| `e2e-build-config: ci.yml grep 守卫` | 删除生产不含 helper 的 grep | ✅ 能发现 | 测试断言 CI step 存在 |
| `dialog-e2e-hook: 未挂载 hook 时透传真实 ask` | hook 存在时仍返回 hook 结果 | ✅ 能发现 | 测试区分 `window.__slterm_e2e_dialogAsk` 存在与否 |

---

## 结论

- **高风险问题 1 个（P1）**：IPC 契约测试过度依赖 `mockIPC`，真实 Tauri 序列化与运行时行为难以在 L2 暴露。这是该领域测试设计上的根本局限，需要文档化并在 L4 重点补强。
- **中风险问题 6 个（P2/P3/P6/P11/P12/P13）**：`notification.ts`、`window.ts`、HTML postMessage 真实环境、E2E 生产 tree-shake、错误边界 inline 分支等存在覆盖不足或测试质量隐患。
- **低风险 / 可维护性问题 10 个（P4/P5/P7/P8/P9/P10/P14/P15/P16/P17）**：多为边界分支未命中、生产构建路径无法 L2 覆盖、重复代码等，可通过补充用例、重构 helper、文档化限制来改进。

**最优先修复建议**：

1. 补充 `notification.test.ts` 与 `window.test.ts`，把未覆盖的 IPC 分支纳入 L2。
2. 修复 `ipc-ping.test.ts` 使其直接调用 `src/ipc/index.ts` 的 `ping()`，消除 0% 覆盖。
3. 修复 `e2e-gating-terminal.test.ts` 中虚假的 `hooks` mock 字段。
4. 扩展 `html-panel.test.tsx` 的负面用例（origin/source 不匹配、未知 type、fingerprint 错误），并验证注入脚本的关键控制流。
5. 补充 `error-boundary.test.tsx` 的 inline variant 用例。
6. 对四个 IPC 契约测试做参数化重构，降低维护成本。
