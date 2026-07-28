# Phase 2-fix 执行计划

> 本文件只写任务特定编排参数；通用执行规则见 `/systematic-changes-execute`。

---

## Stage 表

| Stage | 内容 | 项数 | 并行 agent 数 | commit message |
|-------|------|------|---------------|----------------|
| 1 | 共享页面切换基础设施 + 路由修复（高风险） | 3 | 3（pipeline：infra 先行，notify/view 并行） | `fix: F4/F5 路由复用共享 switchToPage——修复跨页面 focus 失败与 __dockviewApi 悬挂` |
| 2 | Agent Status 数据层修复 | 5 | 2（pipeline：registry-lib 先行） | `fix: F5 数据层——行生命周期订阅/标题查找/null 语义/重订阅修复/parsePageId 收敛` |
| 3 | IPC 契约对齐 | 3 | 2（并行） | `fix: notification/hooks IPC 契约对齐 P2 计划——签名/返回值/re-export/别名清理/合约用例` |
| 4 | 测试补全 + E2E 隔离 | 4 | 2（并行） | `test: P2 测试补全——颜色断言/L4 行渲染启用/动态四态/E2E settings 隔离/flaky 加固` |
| 5 | 文档同步 | 2 | 1 | `docs: P2-fix 文档同步——test-inventory 失实重写 + CLAUDE.md 对齐最终代码` |

---

## Git 策略

- 直接在当前 `phase2` 分支上每 Stage 一条 commit；禁止 amend；`git add` 限定路径后 `git commit`。
- 全部 Stage 完成 + 人工验证点复核后：`git push origin phase2`（收尾步骤）。
- commit message 尾部追加 `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`。

---

## git add 路径枚举

### Stage 1
```
git add src/workspace/pageApis.ts
git add src/workspace/Workspace.tsx
git add e2e-tests/helpers.ts
git add src/features/notifications/useClaudeNotifications.ts
git add src/__tests__/notifications.test.ts
git add src/features/agentStatus/AgentStatusView.tsx
git add src/__tests__/agent-status-view.test.tsx
```

### Stage 2
```
git add src/panels/terminal/TerminalRegistry.ts
git add src/lib/panelId.ts
git add src/__tests__/terminal-registry-subscribe.test.ts
git add src/__tests__/panelId.test.ts
git add src/features/agentStatus/useAgentStatus.ts
git add src/__tests__/agent-status-hook.test.ts
git add src/features/notifications/useClaudeNotifications.ts
git add src/features/agentStatus/AgentStatusView.tsx
```

### Stage 3
```
git add src/ipc/notification.ts
git add src/ipc/hooks.ts
git add src/features/notifications/useClaudeNotifications.ts
git add src/__tests__/notifications.test.ts
git add src/__tests__/setup.ts
git add src/__tests__/ipc-hooks-contract.test.ts
```

### Stage 4
```
git add src/__tests__/colors.test.ts
git add src/theme/index.ts
git add src/__tests__/agent-status-view.test.tsx
git add src/__tests__/diff-panel.test.tsx
git add e2e-tests/test.e2e.ts
git add e2e-tests/run-wdio.cjs
```

### Stage 5
```
git add .claude/test-inventory.md
git add .claude/CLAUDE.md
git add src/ipc/CLAUDE.md
git add src/lib/CLAUDE.md
git add src/workspace/CLAUDE.md
git add src/panels/CLAUDE.md
git add src/features/sideViews/CLAUDE.md
git add e2e-tests/CLAUDE.md
```

---

## fix-loop args 规范

调用 `Workflow({ scriptPath: "docs/hooks-dev/phase2-fix/workflows/fix-loop.js", args })` 时：

```json
{
  "stage": 1,
  "failedItems": ["FIX-FE-01"],
  "fixContext": "verify agent 给出的失败证据原文",
  "verifyFile": "docs/hooks-dev/phase2-fix/workflows/verify/stage-01.md",
  "constraints": "见对应 Stage 脚本头部注释的 Stage 特殊纪律"
}
```

- `stage`：1-5
- `failedItems`：非空字符串数组（FIX-* ID）
- `verifyFile`：对应 Stage 的 verify 文件全路径
- `constraints`：值见各 Stage 脚本头部注释（单一真值源，不复制）

---

## 进度跟踪表

| Stage | 状态 | 通过项 | 未通过项 | 备注 |
|-------|------|--------|----------|------|
| 1 | 未开始 | - | - | 高风险：触及 Workspace 切换核心 |
| 2 | 未开始 | - | - | 依赖 Stage 1 的 pageApis |
| 3 | 未开始 | - | - | 依赖 Stage 1/2 最终中间态 |
| 4 | 未开始 | - | - | 含 E2E 实跑门禁（build:e2e + wdio） |
| 5 | 未开始 | - | - | 用例数以实跑统计为准 |
| push | 未开始 | - | - | `git push origin phase2`（人工验证点复核后） |

---

## 门禁命令（每 Stage 不同）

### Stage 1（helpers.ts 在 tsc include 外 → 补 vite build）
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `npx vite build`

### Stage 2 / Stage 3 / Stage 5
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`

### Stage 4（E2E 实跑为门禁）
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `npm run build:e2e`
6. `npm run wdio`

> 注：E2E 构建必须走 `npm run build:e2e`（含 `VITE_E2E=1`），不可用 config.json 的 `e2eBuild`（裸 `tauri build` 会 tree-shake E2E helper）。

---

## 跨边界契约（执行期不可偏离）

见 `docs/hooks-dev/phase2-fix/stages.md`「跨边界契约汇总」C1-C4（pageApis / TerminalRegistry.subscribe / notification / panelId）——单一真值源在 stages.md，本文件不复制。

---

## 通用执行规则引用

- 实际代码修改由 `/systematic-changes-execute` 完成，本计划只落盘文档。
- 每 Stage 完成后由 verify agent 按 `workflows/verify/stage-NN.md` 逐项验证。
- 验证失败进入 fix-loop，最多 3 轮。
- 文档 Stage 固定最后执行。
