# 自动化测试优化执行编排参数（execution-plan）

> 任务特定参数单点。通用执行规则（resume/no-return 分流/时间盒/git 操作）见 `/systematic-changes-execute`，不复制。
> 清单 `docs/text-fix-plan/checklist.md`｜Stage 划分 `docs/text-fix-plan/stages.md`｜脚本 `docs/text-fix-plan/workflows/`｜断言 `docs/text-fix-plan/workflows/verify/`

## Stage 表

| # | 名称 | 项 | 脚本 | verify | agent | 门禁命令（全量测试 agent） |
|---|------|----|------|--------|-------|--------------------------|
| 01 | l1-pty-spawn | PTY-01/02/03/07/08/09/13①（7） | stage-01-l1-pty-spawn.js | verify/stage-01.md | 1 | clippy + rustTest |
| 02 | l1-pty-edge | PTY-04/05/06/10/11/12/13②③（7） | stage-02-l1-pty-edge.js | verify/stage-02.md | 3 | clippy + rustTest |
| 03 | l1-git | GIT-01~12（12） | stage-03-l1-git.js | verify/stage-03.md | 1 先行 + 2 并行（pipeline） | clippy + rustTest |
| 04 | l1-hooks | HUK-01~11（11） | stage-04-l1-hooks.js | verify/stage-04.md | 3 | clippy + rustTest |
| 05 | l1-misc | HFN-01~09 + SPE-01~06（15） | stage-05-l1-misc.js | verify/stage-05.md | 4 | clippy + rustTest |
| 06 | l2-terminal | TRM-01~08 + NAH-02（9） | stage-06-l2-terminal.js | verify/stage-06.md | 2 | tsCheck + eslint + frontendTest |
| 07 | l2-editor | EDF-01~09（9） | stage-07-l2-editor.js | verify/stage-07.md | 2 | tsCheck + eslint + frontendTest |
| 08 | l2-workspace | WRK-01~11（11） | stage-08-l2-workspace.js | verify/stage-08.md | 2 | tsCheck + eslint + frontendTest |
| 09 | l2-explorer | EXP-01~12（12） | stage-09-l2-explorer.js | verify/stage-09.md | 2 | tsCheck + eslint + frontendTest |
| 10 | l2-sideviews | SVC-01~14（14） | stage-10-l2-sideviews.js | verify/stage-10.md | 3 | tsCheck + eslint + frontendTest |
| 11 | l2-hooks-config | HKC-01~10（10） | stage-11-l2-hooks-config.js | verify/stage-11.md | 2 | tsCheck + eslint + frontendTest |
| 12 | l2-shortcuts-theme | STS-01~11（11） | stage-12-l2-shortcuts-theme.js | verify/stage-12.md | 2 | tsCheck + eslint + frontendTest |
| 13 | l2-ipc-html | IHE-01~08（8） | stage-13-l2-ipc-html.js | verify/stage-13.md | 2 | tsCheck + eslint + frontendTest |
| 14 | l2-agent-history | NAH-01,03~11（10） | stage-14-l2-agent-history.js | verify/stage-14.md | 2 | tsCheck + eslint + frontendTest |
| 15 | l3 | E2E-01/02/03/07/08/14（6） | stage-15-l3.js | verify/stage-15.md | 1 | tsCheck + l3Test |
| 16 | l4-e2e | E2E-04/05/06/09/10/11/12/13/15（9） | stage-16-l4-e2e.js | verify/stage-16.md | 1 | e2eBuild + e2eTest |
| 17 | docs | DOC-01~04（4） | stage-17-docs.js | verify/stage-17.md | 2 | 无代码门禁（收尾跑 frontendTest + rustTest 确认零代码副作用） |

**门禁命令精确值**（config.json commands 单点，勿改写）：
- tsCheck：`npx tsc --noEmit`
- eslint：`npx eslint src/`
- clippy：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- frontendTest：`npm test`
- rustTest：`cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
- l3Test：`npm run test:l3`
- e2eBuild：`npx tauri build --debug --no-bundle`
- e2eTest：`npm run wdio`

## commit message（每 Stage 一条，附 Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>）

| # | message |
|---|---------|
| 01 | `test(l1-pty): 补 pty_spawn 校验/SEC-08 归属/Job Object 纯逻辑覆盖 + spawn.rs 可测性重构` |
| 02 | `test(l1-pty): 补 reader strip/ring buffer/shell 回退/沙箱边界覆盖` |
| 03 | `test(l1-git): 五命令命令层测试重写 + 单文件拆分 + 弱断言精确化` |
| 04 | `test(l1-hooks): 补信号链/注入命令/watcher 事件循环 L1 覆盖` |
| 05 | `test(l1-misc): 补 fs/notify/history/settings 命令层与异常分支覆盖` |
| 06 | `test(l2-terminal): useXterm 去重 + webgl 全分支 + mock 清理` |
| 07 | `test(l2-editor): diff 保存链真实断言 + DiffPanel/useCodeMirror 分支补齐` |
| 08 | `test(l2-workspace): 真实 DefaultTab + 启动顺序断言 + pageApis 覆盖` |
| 09 | `test(l2-explorer): OpenInTerminal/CRUD 成功路径/FileIcon 补齐` |
| 10 | `test(l2-sideviews): drop index 断言 + cancelPendingSave + commit-view 拆分` |
| 11 | `test(l2-hooks-config): linter 顺序/generation 竞态/校验链补齐` |
| 12 | `test(l2-shortcuts-theme): colors 断言真实化 + forceContext 反向 + timer 清理` |
| 13 | `test(l2-ipc-html): mockIPC 盲区收口 + notification/postMessage 负面用例` |
| 14 | `test(l2-agent-history): 四态同源回退/classifyEvent 表驱动/恢复守卫补齐` |
| 15 | `test(l3): keyboard 降级标注 + 生产 theme/OSC 覆盖 + 断言精确化` |
| 16 | `test(e2e): hooks 隔离备份扩展 + 真实 reporter 链路 + test.e2e.ts 拆分` |
| 17 | `docs(claude): test-inventory 全量校正 + 豁免清单 + 定位声明同步` |

## git add 路径枚举（config.json workflow.gitAddPaths 单点，逐字）

`src/`、`src-tauri/`、`e2e-tests/`、`test/`、`.claude/CLAUDE.md`、`.claude/test-inventory.md`、`docs/`

## fix-loop 调用规范

脚本：`docs/text-fix-plan/workflows/fix-loop.js`

args 契约（强制校验，缺 failedItems/verifyFile 即抛错）：

| 字段 | 取值规则 |
|------|---------|
| `stage` | Stage 编号（1-17） |
| `failedItems` | verifyResult.failedItems 原文（非空数组） |
| `fixContext` | verifyResult.details 的 JSON 序列化（失败证据线索） |
| `verifyFile` | `docs/text-fix-plan/workflows/verify/stage-NN.md`（与该 Stage 脚本同一真值源） |
| `constraints` | 该 Stage 脚本头注释 `// fix-loop constraints:` 行的值（单点定义，不复制改写；无此行则省略） |

上限 3 轮；3 轮未全绿 → 上报用户人工介入，不继续空转。

## 进度跟踪表（执行期更新）

| Stage | 状态 | 验证轮次 | 备注 |
|-------|------|---------|------|
| 01 | 完成 | 1 | c0dda4e；PTY-03 断言修正（reattach 无 panel_id，SEC-08 仅三命令）；门禁重跑全绿（461+8 passed） |
| 02 | 完成 | 1 | 2ed8ed8；首轮即全绿（allFixed=true）；488 passed；Cargo.toml 行尾差异还原 |
| 03 | 完成 | 2（fix-loop 1 轮） | c8e3604；verify 首轮 GIT-01 失败（rollback UnbornBranch 错误消息缺"HEAD 中不存在"）→ fix-loop 修复 + renamed.path 断言对齐 git2-rs 语义；498 passed |
| 04 | 完成 | 1 | 7e75486；首轮全绿；531 passed；C10 脚本零改动 |
| 05 | 完成 | 3（fix-loop 2 轮） | 8093c9d；轮1 SPE-03 runtime 求值顺序修复；轮2 concurrent_saves_never_torn flaky 修复（persist 重试+守卫上提）；571 passed；还原 agents 越界 cargo fmt 污染 17 文件 |
| 06 | 完成 | 2（fix-loop 1 轮） | 368d8b5；轮1：terminal.test.tsx cleanup/act/kill mock + bootstrap vi.mock 悬空路径修复（存量失败）；2027 passed |
| 07 | 完成 | 2（fix-loop 1 轮） | 213b539；轮1：EDF-07 残留固定延时改 waitFor/同步断言；2049 passed |
| 08 | 完成 | 2（fix-loop 1 轮） | 2ebe7eb；轮1：layoutSerde 条目级容错修复（null 条目跳过）+ T9/T10 改行为断言；2086 passed |
| 09 | 完成 | 2（fix-loop 1 轮） | 61c0296；轮1：canOpenFile 守卫导出 + 无 activePageId 分支直测；2160 passed；fullRefresh 死代码删除 |
| 10 | 完成 | 2（fix-loop 1 轮） | e5b3abe；轮1：typeof 弱断言清理 + commit-view 再拆（132 行）+ commit-open-file mock 缺陷修复；2184 passed |
| 11 | 完成 | 2（fix-loop 1 轮） | 0569453；轮1：注入状态条初始 '--' 用例；2205 passed；pageApis focus?.() 防御 |
| 12 | 完成 | 1 | bc9ce8f；首轮全绿；2217 passed；HEX6_RE 自断言清零 |
| 13 | 完成 | 1 | 9d8ce27；首轮全绿；2236 passed；IPC 契约工厂化 + mockIPC 盲区文档化 |
| 14-17 | 未开始 | - | - |

## 人工验证点（收尾逐项实测，与 stages.md 一致）

M1（Stage 01 后）构建产物实测 claude 会话滚轮/输入/渲染无回归｜M2（Stage 16 后）L4 视觉回归基线人工确认｜M3（Stage 16 后）确认未触碰真实 `~/.claude/projects/` 且 `~/.claude/settings.json` 已还原｜M4（全部后）L1-L4 四级全量绿。
