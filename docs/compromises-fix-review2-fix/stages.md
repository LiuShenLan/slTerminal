# compromises-fix-review2 修复 Stage 划分（7 Stage）

> 清单真值源：`docs/compromises-fix-review2-fix/checklist.md`（30 项六段式）。
> 输入 review：`docs/compromises-fix-review2/`（7 份分文件 + summary.md）。

## 通用约定

1. **全程 agent 严格串行**（用户并发限制约束，持续有效）：所有 Stage 脚本内 agents 用 for 循环顺序 `await agent(...)`，**不用 `parallel()`**——偏离 stage-workflow 模板默认并行形态，豁免理由 = 用户显式指令。
2. **全量测试命令逐条串行执行**（CP-007 基准负载敏感，2026-09-08 实测并行抢核致基准中位 410ms 误红 vs 串行 180ms）：测试 agent prompt 写死串行措辞，禁并行——偏离模板「并行启动执行」措辞。
3. **历史执行档修订纪律**：对 `docs/compromises-fix-review-fix/` 与 `docs/compromises-fix/` 历史档一律注记追加（「落地复核注记/批注/更正（review2-fix <ID>，2026-09-11）」），原文保留不改写（.claude/adr.md:461 先例）。
4. **Stage 间允许重复碰同一文件**（Stage 串行 + 每 Stage commit）：`.claude/test-exemptions.md` 先后被 S03（BE-01）/S07（DOC-02）触碰——无冲突。
5. **L1 定向测试红线（TQ-COV-06）**：`cargo test --test lib_tests <filter> -- --test-threads=1`，禁 `--lib` / 裸 filter（0xc0000139，根 CLAUDE.md 红线）。
6. **修复顺序写死的 Stage**：S04（FE-02→01→03→04——FE-01 的 times(1) 断言依赖 FE-02 双提交先收口）、S05（FE-07→09→10→08——FE-10 在 FE-09 落地后的代码形态上改写比对段，FE-08 pipeline 后位因共享 editor/CLAUDE.md）。

## Stage 01 工具链与 e2e 设施

**改动项**：TE-01、TE-02、TE-03、TE-04、FE-06

| label | 负责项 | 触碰文件 |
|---|---|---|
| toolchain-scripts | TE-01、TE-02 | `scripts/check-ts7-trigger.mjs`、`vitest.config.ts` |
| wdio-knip | TE-03、TE-04、FE-06 | `e2e-tests/run-wdio.cjs`、`knip.json` |

（两 agent 文件零重叠；串行执行 toolchain-scripts → wdio-knip。）

**实现要点**：
- TE-03 的 tail 残串必须在 `wireWarnCounting` 函数闭包内（该函数被 stdout/stderr 调两次，模块级共享会串流）——checklist 代码块照抄。
- FE-06 追加 `docs/compromises-fix-review2-fix/workflows/*.js` 前瞻登记（本任务产物落盘即入 git，knip 会扫到）。

**验证项**：见 `workflows/verify/stage-01.md`。

**人工验证点**：TE-03 修复后首轮全量 e2e 的 WARN 计数基线复核（跨 chunk 修复可能使计数微升——此前漏计部分被计入，基线填记时注明）；TE-04 无实机项（信号杀死场景属防御语义）。

**commit message**：
`fix(review2-fix): S01 工具链与 e2e 设施——ts7 日志标签/vitest exclude 死配置/WARN 跨 chunk 计数/fallback 退出码/knip ignore 收窄（TE-01~04+FE-06）`

## Stage 02 预览链路

**改动项**：SEC-01、SEC-04（仅 2 项——豁免理由：预览域主题聚合，与 S03 后端文件面相邻但主题独立，前轮同主题即独立 Stage）

| label | 负责项 | 触碰文件 |
|---|---|---|
| preview-csp-test | SEC-01 | `src-tauri/src/preview.rs` |
| previewframe-comment | SEC-04 | `src/panels/docViewer/PreviewFrame.tsx`（+ 若旧文案被断言则 `src/__tests__/html-panel.test.tsx` 适配——条件触碰，先 rg 自查） |

（两 agent 文件零重叠；串行执行 preview-csp-test → previewframe-comment。）

**实现要点**：
- SEC-01 两断言照 checklist 代码块追加，不动既有四断言。
- SEC-04 先 `rg "下轮轮询自愈" src/__tests__/html-panel.test.tsx` 自查测试断言是否含旧文案，命中则同步适配。

**验证项**：见 `workflows/verify/stage-02.md`。

**人工验证点**：无（测试断言 + 注释修正）。

**commit message**：
`fix(review2-fix): S02 预览链路——CSP 测试补 script/style 断言/sync 失败注释文案失实修正（SEC-01/04）`

## Stage 03 后端加固

**改动项**：BE-02、BE-06、BE-03、BE-05、BE-01

| label | 负责项 | 触碰文件 |
|---|---|---|
| notify-harness | BE-02、BE-06 | `src-tauri/src/notify/mod.rs`、`src-tauri/src/notify/CLAUDE.md` |
| fs-cursor-tests | BE-03 | `src-tauri/src/fs/mod.rs` |
| spawn-exemptions | BE-05、BE-01 | `src-tauri/src/pty/spawn.rs`、`.claude/test-exemptions.md` |

（三 agent 文件零重叠；串行执行 notify-harness → fs-cursor-tests → spawn-exemptions。BE-02/06 同文件故同 agent。）

**实现要点**：
- BE-02 照 settings.rs:518-552 mpsc 回传先例适配（checklist 代码块照抄）；`done_tx.send` 在 event_loop 返回后，panic 时发送端随线程死亡 drop → recv 必 Err。
- BE-06 委托后 stop() 幂等性由 Option::take 双保险保证（notify/mod.rs:167-176 现状）。
- BE-03 三例直接调私有 decode_page_cursor（内嵌测试模块同文件可及），断言 `matches!(err, AppError::Validation(_))`，不锁错误文案。

**验证项**：见 `workflows/verify/stage-03.md`。

**人工验证点**：无。

**commit message**：
`fix(review2-fix): S03 后端加固——notify panic 回传+Drop 委托/游标解码三分支补测/spawn 注释撞号/豁免计数（BE-01/02/03/05/06）`

## Stage 04 explorer 状态机

**改动项**：FE-02 → FE-01 → FE-03 → FE-04（顺序写死）

| label | 负责项 | 触碰文件 |
|---|---|---|
| file-tree-state | FE-02、FE-01、FE-03、FE-04 | `src/features/explorer/useFileTree.ts`、`src/__tests__/use-file-tree.test.ts`、`src/features/explorer/CLAUDE.md` |

（单 agent，项间顺序即上列。）

**实现要点**：
- FE-02 先修：删无快照分支显式 commitViewState（保留 restoringRef 复位）——提交由 commit effect 统一承担（setRootNodes 恒产新数组引用，空目录亦触发渲染，提交必达）。
- FE-01 后修：waitFor 断言改 toHaveBeenCalledTimes(1)——FE-02 未修时必红，顺序不可颠倒。
- FE-03 定时器在 rootPath effect 内创建 + cleanup 清除；deps 加 commitViewState。
- FE-04 注释列举写全 5 个解除点（含 FE-03 兜底）。

**验证项**：见 `workflows/verify/stage-04.md`。

**人工验证点**：实机确认展开态注册表槽行为（review2 待验证清单 R2-FE-02 双提交收口后回归——挂载/切换项目页/槽位回填场景 onViewStateChange 上呼次数与时机）。

**commit message**：
`fix(review2-fix): S04 explorer 状态机——无快照双提交收口/抑制超时兜底/恰好一次断言/注释解除点补齐（FE-01~04）`

## Stage 05 大文件链路

**改动项**：FE-07、FE-09、FE-10 → FE-08（pipeline 串行：editor/CLAUDE.md 两 agent 共享）

| label | 负责项 | 触碰文件 |
|---|---|---|
| largefile-viewer | FE-07、FE-09、FE-10 | `src/panels/editor/largeFileViewer/blockCache.ts`、`src/panels/editor/largeFileViewer/useLineIndex.ts`、`src/panels/editor/largeFileViewer/LargeFileViewer.tsx`、`src/__tests__/large-file-viewer.test.tsx`、`src/panels/editor/CLAUDE.md` |
| codemirror-reload | FE-08 | `src/panels/editor/useCodeMirror.ts`、`src/__tests__/use-code-mirror-reload-error.test.ts`、`src/panels/editor/CLAUDE.md` |

（pipeline 顺序执行 largefile-viewer → codemirror-reload；共享文件 editor/CLAUDE.md 由串行保证无冲突。）

**实现要点**：
- FE-07：invalidateFile 清 inflight 前缀条目；在途调用方仍收响应（Promise 存活），旧任务 finally 的 delete 幂等。
- FE-09：scannedBlocks 渲染期直读 wsRef（fatalError 先例），LargeFileViewer 侧经 ref 转发防闭包过期；fileRev+1 后 scannedBlocks 归零 + stat effect deps 仅 [filePath]——无循环触发。
- FE-10：指纹 = 首/中/末三段各 4KB 拼接文本直接相等比对；首挂基线与事件复核共用 sampleFingerprint；既有 :284 例「mtime/size 未变不失效」夹具需适配指纹取样调用（mock readFileRange 按 offset 分派）。
- FE-08：脏分支确认弹窗先于 stat 预检（用户取消不读 stat），顺序不动；读后复核 TOCTOU 防线与打开路径同语义。

**验证项**：见 `workflows/verify/stage-05.md`。

**人工验证点**：实机「外部修改浏览中的大文件」抽查（含同 size 同 mtime 改写场景——内容改但大小不变的指纹触发路径；review2 待验证清单 R2-FE-01/R2-FE-03 顺带观测收口）。

**commit message**：
`fix(review2-fix): S05 大文件链路——inflight 失效清除/重载 stat 预检/首挂基线竞态封闭/抽样指纹比对（FE-07~10）`

## Stage 06 历史执行档注记

**改动项**：SEC-02、SEC-03、BE-04、FE-05、DOC-03、DOC-04（全部为 docs/compromises-fix-review-fix/ 历史档注记追加，原文不改写）

| label | 负责项 | 触碰文件 |
|---|---|---|
| history-annotations | 全部 6 项 | `docs/compromises-fix-review-fix/checklist.md`、`docs/compromises-fix-review-fix/stages.md`、`docs/compromises-fix-review-fix/execution-plan.md`、`docs/compromises-fix-review-fix/workflows/verify/stage-04.md`、`docs/compromises-fix-review-fix/workflows/verify/stage-07.md` |

（单 agent；注记文本逐条照抄 checklist「修复步骤」段。）

**实现要点**：注记统一前缀「落地复核注记/批注/更正（review2-fix <ID>，2026-09-11）」；插于被更正行下一行（`>` 引用块形态，adr.md:461 先例）。

**验证项**：见 `workflows/verify/stage-06.md`。

**人工验证点**：无。

**commit message**：
`docs(review2-fix): S06 历史执行档注记——checklist/stages/verify/execution-plan 失实断言更正（SEC-02/03+BE-04+FE-05+DOC-03/04）`

## Stage 07 文档登记收口（固定末位）

**改动项**：DOC-01、DOC-02、DOC-05、TE-05

| label | 负责项 | 触碰文件 |
|---|---|---|
| docs-registry | 全部 4 项 | `src/__tests__/markdown-assets.test.ts`、`e2e-tests/html.e2e.ts`、`e2e-tests/markdown.e2e.ts`、`.claude/adr.md`、`.claude/test-exemptions.md`、`.claude/skills/systematic-changes-execute/SKILL.md`、`src/workspace/CLAUDE.md` |

（单 agent；DOC-01 五处注释替换文本照抄 checklist「修复步骤」段逐处对照。）

**实现要点**：
- DOC-01 五处均为注释（零逻辑），替换后 rg 旧表述零命中。
- DOC-02 四处行内注记追加（统一口径「SEC-02 起宿主页 meta 承载域级 CSP」），不另起段落打断 ADR 结构。
- DOC-05 落点 = execute skill 5.6 节列表末尾追加一条（不改既有条目）。
- TE-05 签名与 tabChrome.tsx:254 逐字一致（Read 对照后落笔）。

**验证项**：见 `workflows/verify/stage-07.md`。

**人工验证点**：无。

**commit message**：
`docs(review2-fix): S07 文档登记收口——「无 CSP」失实表述 5 处/adr+exemptions 注记/skill 登记纪律/workspace 签名（DOC-01/02/05+TE-05）`
