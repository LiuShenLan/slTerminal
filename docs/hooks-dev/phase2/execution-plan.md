# 阶段 2 执行计划

> 本文件只写任务特定编排参数；通用执行规则见 `/systematic-changes-execute`。

---

## Stage 表

| Stage | 内容 | 项数 | 并行 agent 数 | commit message |
|-------|------|------|---------------|----------------|
| 1 | 后端 notification 插件 + hooks_context_usage 命令 | 6 | 3 | `feat: 后端 notification 插件接入 + hooks_context_usage 命令` |
| 2 | 前端 F4 通知调度（toast + 任务栏闪烁） | 6 | 3 | `feat: F4 通知调度——失焦门控 + toast + 任务栏闪烁 + 点击路由` |
| 3 | 前端 F5 Agent Status 侧栏视图 | 8 | 3 | `feat: F5 Agent Status 侧栏视图——运行中会话列表 + 四态 + 用量条` |
| 4 | 测试补全（L1/L2 + L4 骨架） | 6 | 3 | `test: 阶段 2 L1/L2/L4 测试补全` |
| 5 | 文档同步 | 5 | 3 | `docs: 同步阶段 2 IPC/hooks/侧栏视图文档与契约` |

---

## commit message 规范

- 功能：`feat:` 前缀
- 测试：`test:` 前缀
- 文档：`docs:` 前缀
- 每 Stage 一条 commit；禁止 amend，使用 `git add` 限定路径后 `git commit`。

---

## git add 路径枚举

### Stage 1
```
git add src-tauri/Cargo.toml
git add src-tauri/src/lib.rs
git add src-tauri/capabilities/default.json
git add src-tauri/src/hooks/
git add src/types/hooks.ts
```

### Stage 2
```
git add package.json
git add package-lock.json  # 项目实际使用 npm
git add src/ipc/notification.ts
git add src/ipc/hooks.ts
git add src/ipc/index.ts
git add src/features/notifications/useClaudeNotifications.ts
git add src/App.tsx
```

### Stage 3
```
git add src/features/sideViews/sideViewDefs.ts
git add src/features/sideViews/sideBarState.ts
git add src/features/agentStatus/
git add src/theme/colors.ts
```

### Stage 4
```
git add src/__tests__/notifications.test.ts
git add src/__tests__/agent-status-view.test.tsx
git add src/__tests__/agent-status-hook.test.ts
git add src-tauri/src/hooks/        # L1 测试追加
# 或 git add tests/hooks_context_usage_tests.rs（若选择独立集成测试文件）
git add e2e-tests/test.e2e.ts
```

### Stage 5
```
git add src/ipc/CLAUDE.md
git add src-tauri/src/hooks/CLAUDE.md
# 若阶段 1 未创建，则为 src-tauri/src/hooks/ 目录下新建 CLAUDE.md
git add src/features/sideViews/CLAUDE.md
# 若阶段 1/其他未创建，则为新建
git add .claude/test-inventory.md
git add docs/hooks-dev/contract.md
```

---

## fix-loop args 规范

调用 `Workflow({ scriptPath: "docs/hooks-dev/phase2/workflows/fix-loop.js", args })` 时：

```json
{
  "stage": 1,
  "failedItems": ["P2-BE-04"],
  "fixContext": "verify agent 给出的失败证据原文",
  "verifyFile": "docs/hooks-dev/phase2/workflows/verify/stage-01.md",
  "constraints": "Stage 1 后端代码注释用中文；阻塞 I/O 必须用 spawn_blocking；禁止修改 ConPTY flags。"
}
```

- `stage`：1-5
- `failedItems`：非空字符串数组
- `fixContext`：verify agent 返回的 `details` 证据原文
- `verifyFile`：对应 Stage 的 verify 文件全路径
- `constraints`：Stage 特殊纪律（见各 Stage 脚本头部注释）

---

## 进度跟踪表

| Stage | 状态 | 通过项 | 未通过项 | 备注 |
|-------|------|--------|----------|------|
| 1 | 未开始 | - | - | |
| 2 | 未开始 | - | - | 依赖 Stage 1 的 `hooks_context_usage` 编译符号 |
| 3 | 未开始 | - | - | 依赖 Stage 1/2 的 IPC 与事件基础 |
| 4 | 未开始 | - | - | 依赖 Stage 1-3 代码 |
| 5 | 未开始 | - | - | 依赖 Stage 1-4 最终中间态 |

---

## 门禁命令（每 Stage 不同）

### Stage 1
1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

### Stage 2
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`

### Stage 3
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`

### Stage 4
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

### Stage 5
1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `npm test`
5. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

---

## 跨边界契约（执行期不可偏离）

| 契约项 | 固定值 |
|--------|--------|
| 命令名 | `hooks_context_usage` |
| 参数 | `{ transcriptPath: string }` |
| 返回 | `{ inputTokens: number, outputTokens: number } \| null` |
| 事件名 | `hook-event` |
| 侧栏视图 id | `agent-status` |
| 上下文上限常量 | `CLAUDE_CONTEXT_LIMIT = 200_000`（前端单点：`src/features/agentStatus/consts.ts`） |
| 通知权限 | `notification:default` |
| 任务栏闪烁 | `getCurrentWindow().requestUserAttention(UserAttentionType.Critical)`（值 `1`） |
| toast 内容格式 | `<项目名> · <页签标题> · <事件类别>` |
| toast 点击实现 | `new Notification(title, options)` + `onclick`（`sendNotification` Options 不含 `onClick`） |

---

## 待执行期确认清单

| # | 问题 | 推荐值 | 决策位置 |
|---|------|--------|----------|
| 1 | 窗口焦点检测 API | Tauri `getCurrentWindow().onFocusChanged` | Stage 2 实现时 |
| 2 | 任务栏闪烁 API | `requestUserAttention(UserAttentionType.Critical)`（值 `1`） | Stage 2 实现时 |
| 3 | 上下文用量上限 | 200_000 tokens | Stage 3 实现时 |
| 4 | `CLAUDE_CONTEXT_LIMIT` 单点位置 | `src/features/agentStatus/consts.ts` | Stage 3 实现时 |
| 5 | L4 toast 断言是否自动化 | 若通知中心无法稳定访问，则改为“人工验证点” | Stage 4 实现时 |
| 6 | hooks_context_usage tail 读取策略 | 一次性读文件最后 64KB，按行分割后逆行扫描 | Stage 1 实现时 |
| 7 | AgentStatusView 接收 props | `SideViewComponentProps`（switchToPage, onDeletePage） | Stage 3 实现时 |
| 8 | Stop 事件后行行为 | 状态置 `done` 并保留，SessionEnd/exit 后移除 | Stage 3 实现时 |

---

## 通用执行规则引用

- 实际代码修改由 `/systematic-changes-execute` 完成，本计划只落盘文档。
- 每 Stage 完成后由 verify agent 按 `workflows/verify/stage-NN.md` 逐项验证。
- 验证失败进入 fix-loop，最多 3 轮。
- 文档 Stage 固定最后执行。
