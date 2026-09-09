# 妥协修复 review 问题修复计划 · 执行编排参数

> 只写任务特定编排参数；通用执行规则（resume 语义 / no-return 分流 / 时间盒 / git 操作）见 `/systematic-changes-execute`。
> 清单真值源：`checklist.md`；Stage 分工与要点：`stages.md`；断言真值源：`workflows/verify/stage-NN.md`。

## 执行约束（本任务特有，凌驾模板默认）

1. **所有 agent 严格串行**（用户并发限制约束）：Stage 脚本内 for 循环顺序 `await agent(...)`，不用 `parallel()`；主 agent 调用 Workflow 也逐 Stage 串行，前一个 Stage commit 完成后再起下一个。
2. **全量测试命令逐条串行执行**（CP-007 基准负载敏感）：测试 agent prompt 已写死串行措辞。
3. **L1 定向红线（TQ-COV-06）**：任何 cargo 定向测试一律 `cargo test --test lib_tests <filter> -- --test-threads=1`，禁裸 filter/--lib。
4. **L4 前提**：含 L4 门禁的 Stage（S02/S06）执行前确认 slTerminal 窗口前台聚焦（TQ-E-10 探针）。

## Stage 表

| Stage | 脚本 | verify | 改动项 | agent 顺序 |
|---|---|---|---|---|
| 01 | `workflows/stage-01-toolchain-guards.js` | `workflows/verify/stage-01.md` | TE-01/02/03/07 | ts7-trigger → ci-guard → vitest-exclude |
| 02 | `workflows/stage-02-preview-chain.js` | `workflows/verify/stage-02.md` | SEC-01/02/03+FE-06 | preview-label-csp → preview-frame → injection-lock |
| 03 | `workflows/stage-03-backend-hardening.js` | `workflows/verify/stage-03.md` | BE-01/02/03/04/06 | thread-join → fs-keyset → misc-bench |
| 04 | `workflows/stage-04-frontend-cleanup.js` | `workflows/verify/stage-04.md` | FE-01/02/07/09/10/11 | file-tree → panel-settings → dock-host → shortcuts-retire |
| 05 | `workflows/stage-05-large-file-chain.js` | `workflows/verify/stage-05.md` | BE-05+FE-03/04/05/08 | fs-stat → editor-precheck → large-viewer（顺序依赖写死） |
| 06 | `workflows/stage-06-e2e-infra.js` | `workflows/verify/stage-06.md` | TE-04/05/06/08/09 | plan-env-helper → katex-font-probe → wdio-launcher（顺序依赖写死） |
| 07 | `workflows/stage-07-docs-registry.js` | `workflows/verify/stage-07.md` | DOC-01~11 | compromises-notes → claude-md-registry → exec-doc-replay |

## commit message 表

| Stage | message |
|---|---|
| 01 | `fix(review-fix): S01 工具链与门禁——TS7 触发器 HTTP 状态码守卫+d.mts null 契约+ci.yml src/types 漂移守卫+vitest 坏 exclude 清理（TE-01/02/03/07）` |
| 02 | `fix(review-fix): S02 预览链路——validate_label 放宽 ://HOST_PAGE CSP meta 落地/PreviewFrame catch 可观测/注入纪律测试锁（SEC-01/02/03+FE-06）` |
| 03 | `fix(review-fix): S03 后端加固——join_with_timeout 上提 crate 顶层/fs 游标 keyset 根治/git 注释清理/守卫命令改写/基准两轮制（BE-01/02/03/04/06）` |
| 04 | `fix(review-fix): S04 前端清理——fileTree 挂载竞态+续页失败保留首帧/PANEL_SETTINGS 常量/dock 卸载清理/PageDockviewHost 改名 tabChrome/shortcuts 退役面（FE-01/02/07/09/10/11）` |
| 05 | `fix(review-fix): S05 大文件链路——fs_stat 真实字节/行文本色响应式/块缓存文件变更失效重扫/10MB 检查前置 stat 零读盘（BE-05+FE-03/04/05/08）` |
| 06 | `test(review-fix): S06 e2e 设施——哨兵补 env/探针首 worker fast-fail+降级/writeFakePlanEnv 收编 node-helpers/KaTeX 字体锚点（实证分支 X）/WARN 计数可观测（TE-04/05/06/08/09）`（body 含 TE-08 实证输出 + WARN 基线实测值，分支 X 按实落地替换） |
| 07 | `docs(review-fix): S07 登记收口——compromises 注记回写/根 CLAUDE.md 去计数+L1/L2 串行纪律/test-exemptions 哨兵口径/CP-033 红测演练补做（DOC-01~11）`（body 含红测演练四行输出） |

## git add 路径限定枚举（每 Stage commit 只 add 这些路径）

```
src/
src-tauri/
e2e-tests/
test/
scripts/
.github/
vitest.config.ts
knip.json
.claude/CLAUDE.md
.claude/adr.md
.claude/test-exemptions.md
docs/
```

（相对 config.json `workflow.gitAddPaths` 的新增：`scripts/`、`.github/`、`vitest.config.ts`、`knip.json`（S01/S04 触碰）、`.claude/adr.md`（S02 触碰）——本计划任务特定扩面，登记于此不回写 config。`.temp/node22.bak` 为 untracked 删除，不经 git add。）

## fix-loop args 规范

```
Workflow({ scriptPath: "docs/compromises-fix-review-fix/workflows/fix-loop.js",
  args: { stage: <N>, failedItems: [...], fixContext: <verify details 原文>,
          verifyFile: "docs/compromises-fix-review-fix/workflows/verify/stage-0N.md",
          constraints: <见下> } })
```

各 Stage `constraints` 取值（无特殊纪律传空串）：

| Stage | constraints |
|---|---|
| 01 | `""` |
| 02 | `iframe sandbox 禁 allow-same-origin（CVE-2024-35222）；主窗口 CSP 终态 script-src 'self' 无 data:（csp-config.test.ts 锁死）——修复禁触碰这两面` |
| 03 | `parking_lot 选型不回退（CP-005）；ConPTY flags 禁区之外另：fs_read_dir 前后端契约（cursor opaque）形态不变` |
| 04 | `PageDockviewHost 改名只走 git mv（保历史）；knip.json ignore 键同步改名漏改必红` |
| 05 | `跨边界契约写死（stages.md S05 契约节）：fs_stat/FsMetadata/statFile/LargeFileSignal 形态不得偏离；src/types/fs.ts 为生成物禁手改` |
| 06 | `e2e-tests/ 不在根 tsconfig include——修复后必须实跑对应 spec 验证，不得以 tsc/eslint 绿代替；SEC-18 红线：凭据只用 sk-test 假值` |
| 07 | `本 Stage 只改文档与执行演练动作，禁止改生产代码；历史执行档（docs/compromises-fix/）回写以「落地复核」注记追加，不改写历史断言原文` |

## 进度跟踪表（执行期填记）

| Stage | 状态 | commit | verify 结果 | 备注 |
|---|---|---|---|---|
| 01 | 完成 | 7abc009 | TE-01/02/07 fixed；TE-03 静态断言全过、命令 5 提交后 exit 0 | 门禁 5 时序缺陷：TE-03 自身文档同步改 src/types/CLAUDE.md → 提交前必红，提交后绿 |
| 02 | 完成 | d1a6838 | 首轮 allFixed=false（2 文档项 partial + L4 门禁红）→ fix-loop 1 轮 allFixed=true；L4 补验 build:e2e + html.e2e.ts 绿 | L4 门禁时序缺陷：门禁 7 普通 build tree-shake E2E helper（与 e2e-tests/CLAUDE.md:110 冲突），须 build:e2e 重建后跑 |
| 03 | 完成 | 89988d6 | allFixed=true（BE-01/02/03/04/06 全 fixed） | 连带 settings.rs:531-532 两处测试裸 join 最小换装；BE-02 用例 1201 文件（601 时第二页即末页）；BE-06 保留 samples.sort() |
| 04 | 未开始 | | | |
| 05 | 未开始 | | | TE-03 守卫首次真实触发 |
| 06 | 未开始 | | | TE-08 实证分支入 commit body |
| 07 | 未开始 | | | 红测演练入 commit body |

## 收尾（全 Stage 完成后主 agent 执行）

1. **全量回归兜底**：`npx tsc --noEmit` → `npx eslint src/` → `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` → `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` → `npm test` → `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（逐条串行，禁并行）。
2. **整体人工验证清单**（review 待人工 9 项与本计划联动项，统一交付用户实机验证）：
   - S01：CI 实跑绿（Guard — src/types step 首跑）；
   - S02：预览回归全场景（html/markdown 缩放/滚动/导航）；
   - S04：页签 chrome 行为冒烟（开页/关页/右键菜单/重命名）+ 布局/面板全场景（跨页拖拽/重启恢复）；
   - S05：>10MB 文件实机滚动观感 + 外部修改失效重扫抽查；
   - S06：TE-04 env 哨兵负向验证（改真实屋 env → exit 报红 → 还原）；TE-09 WARN 计数基线填记确认；
   - 既有联动项：Node 26 全量 e2e（.bak 已删前置满足）、CP-029 定责落档复核、Win10 conpty 回退 toast、claude TUI 滚轮实测（ADR-0007 门禁第 3 条）、剪贴板空闲复跑。
