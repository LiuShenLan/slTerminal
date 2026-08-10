# multi-cli profile 重构 — 执行编排参数

> 本文件只写任务特定编排参数；通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）单一真值源在 `/systematic-changes-execute`，不复制。
> 真值源：`docs/multi-cli/checklist.md`（条目）+ `docs/multi-cli/stages.md`（划分与契约）。

## 1. Stage 表

| Stage | 内容 | agent 数 | commit message |
|-------|------|---------|----------------|
| 01 | 前端 profile 注册表 + 身份域（MC-101~108 + D-02/07/08/13 + mockcli.png） | 3 | `refactor(cli-profiles): 前端 CliProfileRegistry + 身份域迁移（MC-101~108）` |
| 02 | 前端状态域（MC-401~422 + MC-107/205/206 + D-02/04/06/12/14） | 5 | `refactor(agent-status): 前端状态域去 claude 化——四态策略入 profile（MC-401~422）` |
| 03 | 后端 hooks 泛化 + 前端 ipc/types（MC-201~215 + D-01/03/09/10/11/14） | 3 | `refactor(hooks): 后端 hooks 信号链路泛化 + CliHooksProvider 下沉 claude（MC-201~215）` |
| 04 | 后端历史泛化 + 前端 ipc/types（MC-301~306 + D-03/14） | 3 | `refactor(agent-history): 后端历史会话泛化 + CliHistoryProvider 下沉 claude（MC-301~306）` |
| 05 | 前端历史聚合 UI（MC-310~317 + D-05/14） | 3 | `refactor(agent-history): 前端历史聚合 UI 泛化 + 复合键（MC-310~317）` |
| 06 | hub 面板（MC-501~508 + MC-220~223 + D-14/15 + 中间态回收） | 3 | `refactor(hooks-config): hub 面板 + CLI 选择行（MC-501~508）` |
| 07 | mock profile 验收 + AC-5 守卫（AC-4/AC-5 + MC-4/6） | 2 | `test(cli-profiles): mock profile 全链路验收 + AC-5 字面量守卫（AC-4/AC-5）` |
| 08 | 文档同步（MC-8/109/110/318/223 + AC-6） | 4 | `docs(multi-cli): 文档同步——模块索引/CLAUDE.md/test-inventory（MC-8/109/110/318）` |

脚本路径：`docs/multi-cli/workflows/stage-NN-*.js`；verify 断言：`docs/multi-cli/workflows/verify/stage-NN.md`；修复循环：`docs/multi-cli/workflows/fix-loop.js`。

## 2. 统一门禁命令（每 Stage 全量，决策 9）

| 序 | 命令 | 用途 |
|---|------|------|
| 1 | `npx tsc --noEmit` | TS 静态检查 |
| 2 | `npx eslint src/` | ESLint |
| 3 | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Clippy |
| 4 | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | rustfmt |
| 5 | `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | L1（**必须单线程**——ConPTY 并发 spawn 死锁 + env 测试污染） |
| 6 | `npm test` | L2 |
| 7 | `npm run test:l3` | L3 |
| 8 | `npm run e2e` | L4（= `build:e2e` + `wdio` 串行；**禁拆分、禁与其他命令并行**——cargo 无法覆写被 wdio 占用的 slterminal.exe；config.json 的 `e2eBuild` 不带 `VITE_E2E=1`，直接使用会 tree-shake helper 全卡「Workspace 未就绪」） |

## 3. git add 路径枚举（Stage commit 限定）

照 config.json `workflow.gitAddPaths` 原样 + 本任务补充一行：

```
src/
src-tauri/
e2e-tests/
test/
.claude/CLAUDE.md
.claude/test-inventory.md
docs/
public/cli-icons/          # 本任务补充：Stage 01 的 mockcli.png（config 原清单未含 public/）
```

## 4. fix-loop args 规范

`fix-loop.js` 的 args 由执行期组装（模板强制校验：failedItems 非空数组 + verifyFile 非空字符串，缺失即 throw）：

| 字段 | 取值规范 |
|------|---------|
| `stage` | Stage 编号（1–8） |
| `failedItems` | verify agent 返回的 `failedItems` 原样透传（与 `verify/stage-NN.md` 同一真值源） |
| `fixContext` | Stage 脚本头部的跨边界契约段（profile 接口/泛化命令/DTO/禁区）原样 + 本 Stage 实现要点 |
| `verifyFile` | `docs/multi-cli/workflows/verify/stage-NN.md`（与 Stage 脚本同一断言文件） |
| `constraints` | stages.md「禁区」六条原样（ConPTY 0x7 / C10 / 轮询补漏 / SEC-05 / E2E 隔离 / E2E_ENABLED 内联） |

重试上限：`maxFixRetries = 3`（config.json）。

## 5. 并行 agent 测试纪律（生成期约定，执行期提醒）

- 同一 Stage 并行 agent **不跑资源共享型测试**（PTY/端口/全局锁）——重构阶段只做编译级检查（`cargo test --no-run` / `npx tsc --noEmit`），真实执行由全量测试 agent 单点跑
- cargo 系命令共享 target 目录锁，并行时排队属正常——勿中止
- Stage 03/04 后端 agent 改 Rust：编译级检查用 `cargo check --manifest-path src-tauri/Cargo.toml`（不跑测试）

## 6. test-inventory 就近同步纪律

各 Stage agent 在改动用例（增/删/更名）时**就近更新** `.claude/test-inventory.md` 对应行（同 Stage commit）；Stage 08 做总数对齐核对。Stage 间串行 + commit，同文件无冲突。

## 7. 进度跟踪表（执行期填写）

| Stage | 状态（待跑/进行中/已提交/已验证） | commit hash | verify allFixed | fix-loop 轮次 | 备注（人工验证点确认） |
|-------|------|------------|-----------------|--------------|----------------------|
| 01 | 已提交/已验证 | 9898faa | true | 1 | import 路径笔误修复（S01-01/05/09）+ 守卫用例落点（S01-08）+ cargo fmt 历史债（gate:fmt）；e2e 补跑 8 spec 全绿 |
| 02 | 已提交/已验证 | afdd74d | true | 1 | claudeHistory 3 文件 import 断裂（分工表遗漏，S02-01/09）+ HUK12 mock 未对齐 merge 语义（S02-06/05）+ 文档残留（S02-03）+ strategies.ts 注释字面命中 + npm test 证据缺失（测试 agent no-return，主 agent 补跑）；cargo test 需 Bash 跑（PowerShell 工具 env 缺 SYSTEMROOT） |
| 03 | 已提交/已验证 | a574752 | true | 1 | cliProfiles 3 文件残留 ipc/hooks import（S03-02/08）+ terminal.test.tsx mock 未更名 + 3 测试文件 onHookEvent 键 + watcher 注释/CLAUDE.md 引用（S03-02）；e2e 被 tsc 阻断 → 修复后全绿 |
| 04 | 待跑 | — | — | 0 | |
| 05 | 待跑 | — | — | 0 | |
| 06 | 待跑 | — | — | 0 | |
| 07 | 待跑 | — | — | 0 | |
| 08 | 待跑 | — | — | 0 | |

## 8. 人工验证点汇总（收尾实测项）

| Stage | 验证点 |
|-------|--------|
| 01 | 真实 claude 会话页签：OSC 133 C 命中 → 标题 `claude` + 🟡 + logo；退出恢复 |
| 02 | 四态全链路（⚡→✅、页签 emoji、会话行/用量条）；F4 通知（失焦 toast + 任务栏闪烁） |
| 03 | hooks 注入/三态/卸载；**已注入用户升级后「版本过旧」→ 重新注入恢复**（决策 7 波及）；信号文件→emoji 链路 |
| 04 | 历史区两区展示、删除、孤儿 ✗ |
| 05 | 恢复编排（双击恢复/分支恢复/复制命令剪贴板） |
| 06 | hub 面板（选择行/切换/保存提示/注入按钮/双模式） |
| 07 | 无（全自动化） |
| 08 | 终验人工走查：真实 claude 全功能回归 |
