# Commit 视图计划文档 Review

> Review 对象：`docs/commit-view/`（checklist.md / stages.md / execution-plan.md）+ `docs/workflows/`（stage-01~07 脚本、fix-loop.js、verify/stage-01~07.md）
> 对照规范：`.claude/skills/systematic-changes-plan/SKILL.md`（Step 3/4/5）+ 需求真值源 `docs/commit-view/requirements.md`
> 结论：**高危 2 / 中危 6 / 低危 4**。脚本语法预检 8/8 实跑通过（execution-plan 的"已预检"声称属实）；逐 ID 对照无错位；测试命令三方（脚本↔verify↔stages 门禁列）一致。

---

## 高危

### H1：renamed 检测前提不成立——`git_status` 未开启 rename 检测，CV-BE-02 oldPath 恒为 null，FR-7 renamed / B14 落空

**位置**：`docs/commit-view/checklist.md` CV-BE-02；`docs/workflows/stage-01-git-backend.js` CV-BE-02 段；`docs/workflows/verify/stage-01.md` CV-BE-02 断言

**问题**：计划全链（需求 FR-3/FR-6/FR-7/B14 → checklist CV-BE-02 → stage-01 脚本 → verify 断言）假定 `INDEX_RENAMED`/`WT_RENAMED` 状态会出现并从中取 `oldPath`，但现状 `git_status` 的 `StatusOptions` 链仅 `include_untracked(true).include_unreadable(true).include_unreadable_as_untracked(true)`（`src-tauri/src/git/mod.rs:143-146`），**未开启 `renames_head_to_index(true)` / `renames_index_to_workdir(true)`**。git2 status 默认不做 rename 检测——重命名表现为 deleted + untracked 两条独立条目，renamed 状态位运行时永不置位（`status_to_str` 的 renamed 分支 `mod.rs:46-47` 现状即死代码，其测试 `mod.rs:441-442` 直接构造状态位、不经过真实 status 计算，故测试绿但行为不存在）。

**后果**：oldPath 恒为 null；renamed 文件在 Changes 列表显示为 deleted（灰）+ untracked（红）两条而非 renamed（青）一条；双击走 deleted/untracked 分派而非 `(git diff)`；B14（HEAD 侧取旧路径）无从谈起。若执行 agent 写 L1 renamed 测试时发现拿不到 renamed 状态，可能自行加选项（计划外改动）或直接构造状态位绕过（逻辑永不真实触发）。

**修复建议**：
1. CV-BE-01 或 CV-BE-02 增补显式改动项：`StatusOptions` 链追加 `.renames_head_to_index(true).renames_index_to_workdir(true)`（评估是否需 `renames_from_rewrites` 及相似度阈值开销），checklist / stage-01 脚本 / verify stage-01 三处同步。
2. verify stage-01 的 CV-BE-02 断言补"grep `renames_head_to_index` 命中 `git_status` 选项链"。
3. L1 renamed 测试要求构造真实 rename 场景（`git mv` 或高相似度重写），禁止直接构造状态位。

### H2：execution-plan 的 fix-loop args 规范与 fix-loop.js / stage-06 / stage-07 头部注释矛盾

**位置**：`docs/commit-view/execution-plan.md`「fix-loop args 规范」段；对照 `docs/workflows/fix-loop.js:14-16`、`docs/workflows/stage-06-e2e.js:9`、`docs/workflows/stage-07-docs.js:5`

**问题**：execution-plan 写 `constraints: ""  // 各 Stage 无特殊约束（无"只改测试"类纪律）`，示意主 agent 对所有 Stage 传空串；但 fix-loop.js 注释明确 Stage 06 应传 `"本 Stage 只改 e2e-tests/ 下文件，禁止改 src/ 与 src-tauri/ 生产代码"`、Stage 07 应传 `"本 Stage 只改 *.md 文档，禁止改代码"`，两个 stage 脚本头部注释亦同。

**后果**：主 agent 以 execution-plan 为编排依据照填 `""`，Stage 06 修复循环 agent 无边界约束、可能改生产代码使修复面扩散；Stage 07 修复循环可能改代码而非文档——与 verify stage-07 语义断言"所有改动文件为 *.md（`git diff --name-only` 确认）"直接冲突，修复循环将反复失败空转。

**修复建议**：execution-plan fix-loop args 规范改为按 Stage 给值（Stage 06/07 传上述原文，其余传 `""`），或改为"constraints 值以各 Stage 脚本头注释为准"，与 fix-loop.js 注释对齐。

---

## 中危

### M1：测试文件名全链写错——`panelRegistry.test.ts` 实为 `panel-registry.test.ts`

**位置**（8 处）：`docs/commit-view/checklist.md` CV-FE-04、CV-FE-09；`docs/commit-view/stages.md` Stage 03 / Stage 04 分工表；`docs/workflows/stage-03-gitshow-panel.js` CV-FE-04 段；`docs/workflows/stage-04-diff-panel.js` CV-FE-09 段；`docs/workflows/verify/stage-03.md` 回归条目；`docs/workflows/verify/stage-04.md` CV-FE-09 条目

**问题**：实际文件为 `src/__tests__/panel-registry.test.ts`（连字符），全链 8 处均写 `panelRegistry.test.ts`（驼峰）。

**后果**：执行 agent 按错误路径找不到文件，浪费轮次定位，或错误新建 `panelRegistry.test.ts` 造成注册表测试分裂成两个文件。

**修复建议**：8 处统一改为 `src/__tests__/panel-registry.test.ts`。

### M2：Stage 03 漏列 `workspace-file-panel-types.test.ts`——其断言 `FILE_PANEL_TYPES.size === 2`，Stage 03 后必红

**位置**：`docs/commit-view/checklist.md` CV-FE-04；`docs/commit-view/stages.md` Stage 03 分工表；`docs/workflows/stage-03-gitshow-panel.js` CV-FE-04 段；`docs/workflows/verify/stage-03.md` 回归条目

**问题**：`src/__tests__/workspace-file-panel-types.test.ts:31` 断言 `expect(FILE_PANEL_TYPES.size).toBe(2)`。Stage 03 将 FILE_PANEL_TYPES 扩为 4（+gitshow+diff）后该断言必失败，但 checklist / 分工表 / 脚本 / verify 均未列此文件（只提了 panel-registry.test.ts）。

**后果**：Stage 03 全量测试直接红，进入修复循环才发现——属计划层面本应列明的波及面（对照 SKILL Step 4"共享常量/token 改动：先查全部消费方再动笔"，FILE_PANEL_TYPES 的消费方排查遗漏了一个测试文件）。

**修复建议**：checklist CV-FE-04、Stage 03 分工表、stage-03 脚本 prompt 补列 `src/__tests__/workspace-file-panel-types.test.ts`（size 2→4 + has 断言更新）；verify stage-03 回归条目同步覆盖。

### M3：Stage 03 分工表漏列 `src/panels/editor/useCodeMirror.ts`（需加 export）

**位置**：`docs/commit-view/stages.md` Stage 03 分工表；`docs/commit-view/checklist.md` CV-FE-03 位置行

**问题**：stage-03 脚本 CV-FE-03 prompt 要求"`MAX_FILE_SIZE_BYTES`/`LARGE_FILE_WARN_BYTES` 当前是 useCodeMirror.ts 模块内 const，需在该文件加 export 后 import 复用"——已核实两常量确为未导出模块内 const（`useCodeMirror.ts:42/44`）。但 stages 分工表与 checklist 文件清单均不含 `src/panels/editor/useCodeMirror.ts`。SKILL Step 4 要求分工表"证明无文件重叠"，漏列文件使该证明失真。

**修复建议**：stages.md Stage 03 分工表与 checklist CV-FE-03 位置行补列 `src/panels/editor/useCodeMirror.ts`（改动范围注明"仅加 export，不改逻辑"）。

### M4：verify stage-03 回归条目措辞自相矛盾

**位置**：`docs/workflows/verify/stage-03.md` 回归条目

**问题**：原文"断言与五类型现状一致（gitshow 已注册、diff 尚未注册属预期……）"——Stage 03 完成后 PANEL_TYPES 为 4 类型（terminal/editor/htmlviewer/gitshow），"五类型"与"diff 尚未注册"自相矛盾；且 stage-03 脚本 CV-FE-04 写的是"三面板→四面板断言"，verify 与脚本口径不一致。verify agent 无法判定该按 4 还是 5 断言，可能误判或放过。

**修复建议**：改为"panel-registry.test.ts 断言与四类型现状一致（terminal/editor/htmlviewer/gitshow）；diff 于 Stage 04 注册，本 Stage 断言中不应出现"（同步修正 M1 文件名）。

### M5：diff 占位对齐 / 滚动同步属"无法自动化验证的假设"，stages 未标注人工验证点

**位置**：`docs/commit-view/stages.md` Stage 04；`docs/workflows/verify/stage-04.md`

**问题**：SKILL Step 4 规则："无法自动化验证的假设（规范推断、平台行为）：代码注释记录假设来源 + stages 文档标注人工验证点，由收尾实测兜底"。FR-7 的占位对齐（`Decoration.widget` 空白行高度=文本行高、双侧视觉平齐）与滚动同步真实行为在 jsdom（L2）无法渲染验证；Stage 06 E2E 仅断言 `diff-left`/`diff-right` 存在，不验证对齐视觉与滚动跟随。stages.md 全文档无任何人工验证点标注。

**后果**：Stage 04/06 全绿不代表对齐与滚动真实行为正确，缺陷将漏到交付后。

**修复建议**：stages.md Stage 04 标注人工验证点（占位行高与文本行高一致、未改动行双侧平齐、垂直滚动跟随、水平滚动独立），并列入 Step 6 收尾实测清单；stage-04 脚本 PREAMBLE 要求 agent 在占位 widget / 滚动同步代码处注释假设来源。

### M6：Stage 07 verify 断言所需数据（cargo/L3/L4 用例数）超出本 Stage 门禁产出

**位置**：`docs/workflows/stage-07-docs.js` CV-DOC-03 段 + 测试命令；`docs/workflows/verify/stage-07.md` CV-DOC-03 断言

**问题**：verify CV-DOC-03 断言"test-inventory 各层级用例数与 `npm test` / `cargo test` 实际输出一致（对照全量测试结果中的统计行核实）"，stage-07 prompt 也要求"按 npm test 与 cargo test 实际输出的用例数更新"——但 Stage 07 门禁仅 tsc + eslint + npm test，**无 cargo test 统计行**；L4 用例数（Stage 06 新增 2 条后 14→16）的数据在 Stage 06 的 wdio 输出中，Stage 07 不可得。verify prompt 又声明"测试类断言据此判定（无需重跑）"——数据源缺失使该断言不可核实，只能判 partial 或被迫重跑。

**修复建议**（二选一）：
1. Stage 07 门禁补 `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`（文档 Stage 顺手跑全量 L1，与 Step 6 收尾前奏一致），测试 prompt 要求附 cargo 统计行；L4 计数改为"对照 `e2e-tests/test.e2e.ts` 用例静态计数"。
2. verify CV-DOC-03 收窄取数方式：L2 数对照本 Stage npm test 统计行；L1/L3/L4 数对照源文件静态用例计数（不依赖实跑输出）。

---

## 低危

### L1：CV-BE-01 未提示 `recurse_untracked_dirs` 对 explorer 侧的波及面

**位置**：`docs/commit-view/checklist.md` CV-BE-01；`docs/workflows/stage-01-git-backend.js` CV-BE-01 段

**问题**：需求 FR-4 已明示已知副作用"文件浏览器中新目录下每个文件都会标红（属缺陷修正，预期接受）"，但 checklist 与脚本均未提示 agent 该改动波及 ExplorerPanel git 着色行为及相关测试（explorer 系列测试若断言目录级 untracked 聚合将失败）。虽然全量测试门禁兜底，但 agent 无预期地遭遇跨模块测试失败会拉长修复循环。

**修复建议**：checklist CV-BE-01 补一句"已知副作用（FR-4）：explorer 新目录子文件逐个标红；agent 需 grep explorer git 着色相关测试断言并按新语义同步更新"。

### L2：gitshow/diff 经 `isAlwaysRenderPanel` 自动获得 `renderer: "always"`，与 editor 排除理由矛盾，决策未显式化

**位置**：`docs/commit-view/stages.md` Stage 03 实现要点（"现有逻辑自动返回 true（面板少无需改），确认即可"）

**问题**：`panelRegistry.ts:59-62` `isAlwaysRenderPanel = type !== "editor" && isValidPanelType(type)`——gitshow（Stage 03）/diff（Stage 04）注册后自动返回 true，Stage 05 openCommitFile 照 ExplorerPanel 流程将为其设置 `renderer: "always"`（页签切换不卸载）。但该函数注释明示 editor 被排除是因"大文件编辑器始终挂载显著增加内存开销"——gitshow/diff 同样承载大文件 CM（FR-11 沿用 >10MB 阈值），always 挂载与排除理由相悖。"确认即可"把决策留给执行 agent 却未给判断依据。

**修复建议**：stages.md Stage 03/04 显式决策并记录理由：接受 always（如"git 页签低频且数量少，内存可接受"），或将 gitshow/diff 排除出 always 集合（`isAlwaysRenderPanel` 改显式白名单）；Stage 05 openCommitFile 的 renderer 设置按决策执行。

### L3：Stage 02 仅 2 项且两项不强耦合，偏离"每 Stage 3-15 项"

**位置**：`docs/commit-view/stages.md` Stage 02

**问题**：SKILL Step 4 规定"每 Stage 3-15 项——强耦合大项（跨前后端的单一任务）可 2-3 项独立成 Stage"。Stage 02 含 CV-FE-01（titleManager 后缀）+ CV-FE-02（DEFAULT_ZONES）共 2 项，两者文件零重叠、互不依赖，不属"强耦合大项"例外。

**修复建议**：现状可豁免（基础设施须先于 Stage 03/05 就绪，且并入他 Stage 会引入文件重叠）——在 stages.md Stage 02 补一句划分理由即可；无需重新划分。

### L4：checklist 无 P0-P4 优先级组织

**位置**：`docs/commit-view/checklist.md`

**问题**：SKILL Step 3 要求清单"按优先级（P0-P4）+ 模块组织"。checklist 仅按模块（CV-BE/CV-FE/CV-TE/CV-DOC）组织，无优先级标注，亦未说明替代约定。

**修复建议**：checklist 头部补一句"本清单为 feature 开发，实施优先级由 Stage 依赖顺序表达（BE → FE 基础 → 面板 → 视图 → E2E → 文档）"，或逐项补 P 级标注。

---

## 核验结论与处置（2026-07-19）

**12 条全部核验为正确**，无 review 错误需补充。逐条证据：

| # | 核验证据 | 处置 |
|---|---------|------|
| H1 | `src-tauri/src/git/mod.rs:143-146` StatusOptions 链仅 `include_untracked/include_unreadable/include_unreadable_as_untracked`，无 `renames_*`；git2 默认不做 rename 检测 | checklist CV-BE-02 加前提 + stages Stage 01 要点 + stage-01 脚本 CV-BE-02 段 + verify/stage-01 断言，四处同步 |
| H2 | execution-plan `constraints: ""` ↔ fix-loop.js:14-16 / stage-06:9 / stage-07:5 矛盾属实 | execution-plan fix-loop args 规范改为按 Stage 取值 |
| M1 | Glob 证实实际文件 `src/__tests__/panel-registry.test.ts`（连字符） | 8 处统一修正（checklist×2、stages×2、stage-03/04 脚本、verify×2） |
| M2 | `workspace-file-panel-types.test.ts:31` 断言 `size).toBe(2)` 属实 | checklist CV-FE-04 + stages Stage 03 分工表 + stage-03 脚本 + verify/stage-03 回归条目，四处补列 |
| M3 | Grep 证实 `useCodeMirror.ts:42/44` 两常量无 export | stages Stage 03 分工表 + checklist CV-FE-03 位置行补列（仅加 export） |
| M4 | verify/stage-03 "五类型"与"diff 尚未注册"自相矛盾属实。**补充发现（review 漏列）**：stages.md Stage 03 验证项摘要同样误写"PANEL_TYPES 五类型" | verify/stage-03 回归条目重写为四类型；stages.md Stage 03 验证项摘要一并修正 |
| M5 | stages.md 全文无人工验证点标注属实 | stages.md Stage 04 新增人工验证点段 + Step 6 收尾插入人工实测项 + stage-04 PREAMBLE 加假设来源注释要求 |
| M6 | Stage 07 门禁仅 tsc+eslint+npm test，无 cargo 统计行属实 | 选方案 2（收窄取数）：stage-07 脚本 + verify/stage-07 + stages.md Stage 07 三处同步 |
| L1 | checklist CV-BE-01 与 stage-01 脚本均未提示 explorer 波及面属实 | 两处补"已知副作用"提示 |
| L2 | `panelRegistry.ts:57` editor 排除理由注释属实 | 决策：改显式白名单（terminal + htmlviewer），gitshow/diff 不 always——已写入 stages Stage 03/04 + stage-03 脚本 + verify/stage-03 |
| L3 | Stage 02 仅 2 项属实 | stages.md Stage 02 补划分理由（不重新划分） |
| L4 | checklist 无优先级组织属实 | checklist 头部补 Stage 依赖顺序替代约定 |
