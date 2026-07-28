# Phase 2-fix Stage 划分

> 5 个 Stage，串行执行，每 Stage commit。Stage 内文件不重叠；Stage 间允许先后碰同一文件（`useClaudeNotifications.ts`、`AgentStatusView.tsx`、`agent-status-view.test.tsx` 被 Stage 01/02/03/04 先后触碰，已逐处标注）。
> 验证断言唯一真值源：`docs/hooks-dev/phase2-fix/workflows/verify/stage-NN.md`。

---

## 跨边界契约汇总（执行期不可偏离）

### C1 `src/workspace/pageApis.ts`（新建）——页面 API 注册表 + 共享切换

```ts
import type { DockviewApi } from "dockview-react";

registerPageApi(pageId: string, api: DockviewApi): void
unregisterPageApi(pageId: string): void
getPageApi(pageId: string): DockviewApi | undefined

/** 切换活跃页面：setProjectRoot 前置 await → setActivePage → 已注册 api 立即重指向 __dockviewApi */
switchToPageShared(pageId: string): Promise<void>
// - useLayout.getState().activePageId === pageId 时直接返回
// - 经 useProjects.getState() 查 pageId 所属项目 rootPath，await setProjectRoot（失败 console.error 降级继续）
// - useLayout.getState().setActivePage(pageId)
// - getPageApi(pageId) 命中 → window.__dockviewApi = api（未初始化页面由 Workspace.handlePageApiReady 兜底重指向）

/** 切换页面并聚焦面板：await switchToPageShared → 有限轮询面板挂载（100ms×50=5s 上限）→ focus() */
switchToPageAndFocus(pageId: string, panelId: string): Promise<void>
```

- `window.__dockviewApi` 重指向只允许出现在三站点：`Workspace.switchToPage`（经 switchToPageShared）、`Workspace.onDeletePage`、`Workspace.handlePageApiReady`。
- `Workspace.tsx` 组件内 `switchToPage` 保留 `ensurePageInitialized(pageId)` 调用后委托 `switchToPageShared(pageId)`（ensure 依赖组件 setState，不下放）。

### C2 `TerminalRegistry.subscribe`（新增）

```ts
type RegistryEvent = { type: "register" | "remove"; panelId: string };
subscribe(listener: (e: RegistryEvent) => void): () => void  // 返回退订函数
// register/remove 内同步通知全部 listener；通知在 Map 变更之后
```

### C3 `src/ipc/notification.ts` 契约

```ts
sendClickableNotification(title: string, options: { body: string }, onClick: () => void): Notification | null
// 成功路径返回 Notification 实例；catch 回退 Tauri sendNotification 路径返回 null
export { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
// ensureNotificationPermission 保留；sendSilentNotification 删除
```

### C4 `src/lib/panelId.ts`（新建）

```ts
/** 从终端 panelId 解析 pageId。格式 terminal-{pageId}-{seq}，seq 必须为全数字 */
parseTerminalPageId(panelId: string): string | null
// "terminal-page1-0" → "page1"；"terminal-my-page-2" → "my-page"；
// "terminal-abc"（两段）/ "terminal-foo-bar"（尾段非数字）/ "editor-x-1"（非 terminal 前缀）→ null
```

---

## Stage 01：共享页面切换基础设施 + 路由修复（高风险）

**改动项**：FIX-FE-01、FIX-FE-02、FIX-FE-10

**Agent 分工**（pipeline：A 先行，B/C 依赖 A 的 pageApis 模块后并行）：

| label | 负责项 | 文件全集 |
|-------|--------|---------|
| infra | pageApis 新建 + Workspace 改造 + E2E helper 清扫 | `src/workspace/pageApis.ts`（新）、`src/workspace/Workspace.tsx`、`e2e-tests/helpers.ts` |
| notify-consumer | FIX-FE-01/10：routeToPanel + findPanelTitle + 注释 | `src/features/notifications/useClaudeNotifications.ts`、`src/__tests__/notifications.test.ts` |
| view-consumer | FIX-FE-02：handleFocus 改造 | `src/features/agentStatus/AgentStatusView.tsx`、`src/__tests__/agent-status-view.test.tsx` |

无文件重叠 ✓。B/C 不碰 `Workspace.tsx`/`pageApis.ts`（只 import）。

**实现要点**：
- infra：`pageApis.ts` 按 C1 实现；Workspace 三站点改造（switchToPage 委托、onDeletePage 经 unregisterPageApi + getPageApi、handlePageApiReady 经 registerPageApi）；`helpers.ts:211-227` `__slterm_e2e_switchToPage` 改委托 `switchToPageShared`（其余 helper 不动）。**helpers.ts 在根 tsconfig include 外——本 Stage 门禁补 `npx vite build`**。
- notify-consumer：routeToPanel 删除 :116-143 自复制逻辑，改 `await switchToPageAndFocus(pageId, panelId)`（pageId 用现有本地 parsePageId，lib 收敛属 Stage 02）；findPanelTitle 改经 `getPageApi(parsePageId(panelId) ?? "")?.getPanel(panelId)?.title ?? panelId`；FIX-FE-10 注释修正；notifications.test.ts 新增 routeToPanel 守卫用例（mock `../../workspace/pageApis`，断言 onClick 触发后 switchToPageAndFocus 被调、setActivePage 未被 routeToPanel 直接调）。
- view-consumer：handleFocus 改 async——保留内联 pageId 解析（Stage 02 收敛），删除项目查找循环与 `props.switchToPage` 调用，改 `await switchToPageAndFocus(pageId, panelId)`；agent-status-view.test.tsx :229 点击用例改写（mock pageApis 断言参数）。

**验证项**：见 `verify/stage-01.md`（含语义式断言：不存在 routeToPanel/handleFocus 内直接调用 `setActivePage` 的残留——须 Read 确认；`__dockviewApi` 重指向仅三站点——grep 全仓 `__dockviewApi =` 枚举确认）。

**人工验证点**（收尾实测）：① 真实 claude 失焦 toast 点击 → 跨页面落到正确页签（F4 验收要点 1）；② F5 行点击跨页面聚焦。

**commit**：`fix: F4/F5 路由复用共享 switchToPage——修复跨页面 focus 失败与 __dockviewApi 悬挂`

**门禁**：`npx tsc --noEmit`、`npx eslint src/`、`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`npm test`、`npx vite build`

---

## Stage 02：Agent Status 数据层修复

**改动项**：FIX-FE-03、FIX-FE-04、FIX-FE-05、FIX-FE-06、FIX-FE-07

**Agent 分工**（pipeline：A 先行，B 依赖 A 的 subscribe + panelId）：

| label | 负责项 | 文件全集 |
|-------|--------|---------|
| registry-lib | FIX-FE-03 前半 + FIX-FE-07 前半 | `src/panels/terminal/TerminalRegistry.ts`、`src/lib/panelId.ts`（新）、`src/__tests__/terminal-registry-subscribe.test.ts`（新）、`src/__tests__/panelId.test.ts`（新） |
| hook-consumer | FIX-FE-03/04/05/06 + FIX-FE-07 引用替换 | `src/features/agentStatus/useAgentStatus.ts`、`src/__tests__/agent-status-hook.test.ts`、`src/features/notifications/useClaudeNotifications.ts`、`src/features/agentStatus/AgentStatusView.tsx` |

无文件重叠 ✓。B 碰 Stage 01 已改的 `useClaudeNotifications.ts`/`AgentStatusView.tsx`——Stage 串行无冲突。

**实现要点**：
- registry-lib：C2 subscribe（listeners Set + register/remove 内通知）；C4 parseTerminalPageId；两个新测试文件。
- hook-consumer：
  - 订阅增删（FE-03）：useEffect 内 `TerminalRegistry.subscribe`——register 且属当前项目 → 插 🟡 行；remove → 删行。
  - 标题（FE-04）：两处 `终端 ${pageId}` 改 `getPageApi(pageId)?.getPanel(panelId)?.title ?? \`终端 ${pageId}\``；事件路径顺带刷新已有行标题。
  - null 跳过（FE-05）：`eventToStatus` 返回 null → 不更新 status（仍刷新 lastEventAt/transcriptPath/拉用量）；删 `Stop ? "done"` 特判（真实映射已含）。
  - useMemo 稳定（FE-06）：`projectPageIds = useMemo(() => new Set(...), [activeProject])`。
  - parsePageId 收敛（FE-07）：三处删除本地副本，统一 `import { parseTerminalPageId } from "../../lib/panelId"`（路径按所在文件调整）；注意 parseTerminalPageId 返回 `string | null`——调用点处理 null（原本地实现不返回 null 的路径按类型收窄处理）。
  - agent-status-hook.test.ts：删 claudeStatus mock 改用真实映射；mock 补 `TerminalRegistry.subscribe` 与 `../workspace/pageApis`；审计 21 用例对真实映射断言；新增 null 跳过/register 增行/remove 删行/标题查找/重订阅计数用例。

**验证项**：见 `verify/stage-02.md`（含语义式断言：useAgentStatus 中不存在 `newStatus === null` 时写入 status 的路径——须 Read 确认；grep 全仓 `function parsePageId` 零命中）。

**commit**：`fix: F5 数据层——行生命周期订阅/标题查找/null 语义/重订阅修复/parsePageId 收敛`

**门禁**：`npx tsc --noEmit`、`npx eslint src/`、`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`npm test`

---

## Stage 03：IPC 契约对齐

**改动项**：FIX-FE-08、FIX-FE-09、FIX-TE-01

**Agent 分工**（并行，无依赖）：

| label | 负责项 | 文件全集 |
|-------|--------|---------|
| notification-contract | FIX-FE-08 | `src/ipc/notification.ts`、`src/features/notifications/useClaudeNotifications.ts`、`src/__tests__/notifications.test.ts` |
| hooks-contract | FIX-FE-09 + FIX-TE-01 | `src/ipc/hooks.ts`、`src/__tests__/setup.ts`、`src/__tests__/ipc-hooks-contract.test.ts` |

无文件重叠 ✓。

**实现要点**：
- notification-contract：按 C3 改签名/返回值/re-export/删 sendSilentNotification；调用点 :226 改 `{ body: bodyParts }`；notifications.test.ts mock 与第二参数断言同步（`{ body }` 对象）。
- hooks-contract：删 `getContextUsage`；setup.ts:98 mock 键改 `contextUsage`；ipc-hooks-contract.test.ts 追加 contextUsage 四维用例（命令名/参数/透传/异常，16→20）。

**验证项**：见 `verify/stage-03.md`（grep `sendSilentNotification`/`getContextUsage` 全仓零命中；notification.ts re-export 三函数命中；contract 测试 20 用例绿）。

**commit**：`fix: notification/hooks IPC 契约对齐 P2 计划——签名/返回值/re-export/别名清理/合约用例`

**门禁**：`npx tsc --noEmit`、`npx eslint src/`、`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`npm test`

---

## Stage 04：测试补全 + E2E 隔离

**改动项**：FIX-TE-02、FIX-TE-03、FIX-TE-04、FIX-TE-05

**Agent 分工**（并行）：

| label | 负责项 | 文件全集 |
|-------|--------|---------|
| l2-tests | FIX-TE-02 + FIX-TE-05 | `src/__tests__/colors.test.ts`、`src/theme/index.ts`、`src/__tests__/agent-status-view.test.tsx`、`src/__tests__/diff-panel.test.tsx` |
| e2e-tests | FIX-TE-03 + FIX-TE-04 | `e2e-tests/test.e2e.ts`、`e2e-tests/run-wdio.cjs` |

无文件重叠 ✓。

**实现要点**：
- l2-tests：colors.test.ts 新增 AGENT_STATUS_USAGE_COLORS describe（3 token hex + 精确值，从 `../theme` 导入——先把 token 追加进 `src/theme/index.ts` re-export 列表）；view 测试用量条 describe 补三段颜色断言；diff-panel :353 waitFor 补 `[data-e2e="diff-left"] .cm-content` 存在断言 + 审计用例 13-15 同模式。
- e2e-tests：
  - run-wdio.cjs 备份/还原：启动时 `~/.slterminal/settings.json` → `settings.json.e2e-bak`（存在才复制）；`process.on('exit')` 同步还原（原无文件则删产物，原有则移回 + 清 bak）；exit 钩子覆盖 node22 直跑/下载/fallback 三路径。
  - 侧栏两用例前置重置 zones：经 `__slterm_e2e_moveSideViewButton` 逐个将 `projects/explorer/commit/agent-status` 归位 `top` 对应序位、bottom 清空，再 toggle 重置 open。
  - Agent Status 用例 2（:1612 it.skip）拆两个 active 用例：静态行渲染（复用本体 :1615-1696）+ 信号文件驱动四态（照 :1357 先例原子写信号文件，panelId 同静态用例的 `terminal-{pageId}-0`，PreToolUse→⚡ / Stop→✅ / SessionEnd→行消失逐态断言）；删除"hook 事件不可用"失实注释。用例 3 维持 it.skip。
  - **E2E 实跑为本 Stage 门禁**（build:e2e + wdio），不是可选。

**验证项**：见 `verify/stage-04.md`（含 E2E 跑后 `~/.slterminal/settings.json` 与跑前 diff 无变化——测试 agent 执行前后各取一次快照比对）。

**commit**：`test: P2 测试补全——颜色断言/L4 行渲染启用/动态四态/E2E settings 隔离/flaky 加固`

**门禁**：`npx tsc --noEmit`、`npx eslint src/`、`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`npm test`、`npm run build:e2e`、`npm run wdio`

---

## Stage 05：文档同步

**改动项**：FIX-DOC-01、FIX-DOC-02

**Agent 分工**（单 agent——多文档间口径需一致，拆分收益低）：

| label | 负责项 | 文件全集 |
|-------|--------|---------|
| docs | FIX-DOC-01/02 | `.claude/test-inventory.md`、`src/ipc/CLAUDE.md`、`src/lib/CLAUDE.md`、`src/workspace/CLAUDE.md`、`src/panels/CLAUDE.md`、`src/features/sideViews/CLAUDE.md`、`e2e-tests/CLAUDE.md`、`.claude/CLAUDE.md` |

**实现要点**：按 checklist 两条目逐项落实；test-inventory 用例数以 Stage 1-4 完成后**实跑统计**为准（npm test 尾行 + cargo test 尾行 + L4 active 计数），禁凭估计；CLAUDE.md 逐文件对照真实代码写。

**验证项**：见 `verify/stage-05.md`（失实关键词 grep 零命中；文档描述对照代码语义式核实）。

**commit**：`docs: P2-fix 文档同步——test-inventory 失实重写 + CLAUDE.md 对齐最终代码`

**门禁**：`npx tsc --noEmit`、`npx eslint src/`、`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`、`npm test`

---

## 人工验证点汇总（执行收尾实测）

| # | 项 | 关联 |
|---|----|------|
| 1 | 真实 claude 失焦 toast 点击 → 跨页面落到正确页签 | FIX-FE-01 |
| 2 | F5 行点击跨页面聚焦到对应终端 | FIX-FE-02 |
| 3 | 关闭终端页签 → F5 行消失；新开终端（未注入）事件到达 → 行出现 | FIX-FE-03 |
| 4 | F5 行标题与页签标题一致（terminal-N） | FIX-FE-04 |
| 5 | E2E 跑完后用户 settings.json 无变化（执行期已自动 diff，人工复核一次） | FIX-TE-04 |

原阶段 2 的 7 条人工验证点（stages.md「人工验证点汇总」）中与此相关的项由本修复覆盖后一并复核。

## 收尾

全部 Stage 完成 + 人工验证点复核后：`git push origin phase2`。
