# multi-cli profile 重构 — 收尾报告

> 执行方式：`/systematic-changes-execute`，2026-08-10。8 个 Stage 全部完成，全量门禁绿。

## Stage commit 列表

| Stage | commit | 内容 | fix-loop 轮次 |
|-------|--------|------|---------------|
| 01 | `9898faa` | 前端 CliProfileRegistry + 身份域迁移（MC-101~108） | 1 |
| 02 | `afdd74d` | 前端状态域去 claude 化——四态策略入 profile（MC-401~422） | 1 |
| 03 | `a574752` | 后端 hooks 信号链路泛化 + CliHooksProvider 下沉 claude（MC-201~215） | 1 |
| 04 | `bca4be3` | 后端历史会话泛化 + CliHistoryProvider 下沉 claude（MC-301~306） | 1 |
| 05 | `1c59c4d` | 前端历史聚合 UI 泛化 + 复合键（MC-310~317） | 1 |
| 06 | `904f169` | hub 面板 + CLI 选择行（MC-501~508） | 0 |
| 07 | `bb82b15` | mock profile 全链路验收 + AC-5 字面量守卫（AC-4/AC-5） | 0 |
| 08 | `25e97cb` | 文档同步——模块索引/CLAUDE.md/test-inventory（MC-8/109/110/318） | 0 |

## 最终测试用例数（全量门禁实测）

| 层级 | 用例数 | 说明 |
|------|--------|------|
| L1（cargo test） | 592 | 486 单元 + 1 ci_config + 32 + 8 + 10 + 41 + 6 + 8 集成 |
| L2（npm test） | 2408 | 139 文件 |
| L3（test:l3） | 138 | 7 文件 |
| L4（npm run e2e） | 39 | 9 spec，37 active + 2 skip |
| **全量** | **3177** | test-inventory 已回写实跑值 |

## 未修复项

无。8 个 Stage 全部 verify `allFixed: true`（Stage 02/05 的 partial 为执行证据缺失，主 agent 补跑门禁后闭合）。

## 各 Stage 修复摘要（fix-loop 轮次 ≥1 的根因）

- **Stage 01**：并行 agent import 路径笔误（`../../cliProfiles` → `../cliProfiles`）；资源守卫用例落点与断言文件不符；cargo fmt 历史债（工具链版本差异，18 文件纯格式）
- **Stage 02**：分工表遗漏 claudeHistory 3 文件 import 断裂；HUK12 测试 mock 未对齐生产 merge 语义；文档残留旧 API 名
- **Stage 03**：cliProfiles 3 文件残留 ipc/hooks import；terminal.test.tsx mock 键未更名；watcher 注释/CLAUDE.md 引用残留
- **Stage 04**：6 处 types/claudeHistory 残留 import；test-inventory 分布数字失实
- **Stage 05**：useXterm/useAgentStatus 3 处事件名字面量（SessionEnd/Exit）→ SESSION_END_EVENT/EXIT_EVENT 常量下沉 profiles/claude（AC-5 预检）
- **Stage 07**：mockCliProfile.ts 桩参数 eslint 未用；mock-cli-profile.test.tsx 2 用例（ensureNotificationPermission 桩非 Promise + activeStatuses 复合键）
- **E2E mockcli 根因链**（Stage 07 无 fix-loop 但主 agent 深度排障）：app 启动恢复用户布局 ~30 个终端面板（run-wdio 只备份不清理）→ `__e2e_writeToTerminal` 首匹配注入用户残留面板与断言对象不一致 → TerminalPanel 容器挂 `data-panel-id` 定位锚点 + 循环注入等待 OSC 133 handler 注册（waitForPtySessionReady 可能命中残留面板的 ready 标志）

## 人工验证点（待真机实测，结果回填）

| Stage | 验证点 | 状态 |
|-------|--------|------|
| 01 | 真实 claude 会话页签：OSC 133 C 命中 → 标题 `claude` + 🟡 + logo；退出恢复 | 待实测 |
| 02 | 四态全链路（⚡→✅、页签 emoji、会话行/用量条）；F4 通知（失焦 toast + 任务栏闪烁） | 待实测 |
| 03 | hooks 注入/三态/卸载；**已注入用户升级后「版本过旧」→ 重新注入恢复**（决策 7，SCRIPT_VERSION 1→2 波及）；信号文件→emoji 链路 | 待实测 |
| 04 | 历史区两区展示、删除、孤儿 ✗ | 待实测 |
| 05 | 恢复编排（双击恢复/分支恢复/复制命令剪贴板） | 待实测 |
| 06 | hub 面板（选择行/切换/保存提示/注入按钮/双模式） | 待实测 |
| 08 | 终验人工走查：真实 claude 全功能回归 | 待实测 |
