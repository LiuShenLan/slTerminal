# 妥协修复 review 问题修复计划 · Stage 划分

> 输入：`docs/compromises-fix-review-fix/checklist.md`（40 项六段式，唯一真值源）。
> 本文只写 Stage 分工/实现要点/验证/commit message；执行编排参数见 `execution-plan.md`。

## 全局纪律（偏离登记）

1. **全程 agent 严格串行**（用户并发限制约束，持续有效）：所有 Stage 脚本内 agents 用 for 循环顺序 `await agent(...)`，**不用 `parallel()`**——偏离 stage-workflow 模板默认并行形态，豁免理由 = 用户显式指令。
2. **全量测试命令串行执行**（CP-007 基准负载敏感，2026-09-08 实测并行抢核致基准中位 410ms 误红 vs 串行 180ms）：测试 agent 逐条串行跑命令，禁并行——偏离模板「并行启动执行」措辞；该纪律的文档登记在 DOC-09（Stage 07），纪律本身自 Stage 01 起生效。
3. **文档同步固定末位**：Stage 07 = 纯文档登记（DOC-01~11），反映全部代码 Stage 完成后的终态。
4. **Stage 间允许重复碰同一文件**（Stage 串行 + 每 Stage commit）：src-tauri/src/CLAUDE.md 先后被 S01/S02/S03 触碰、e2e-tests/CLAUDE.md 归 S06、PreviewFrame.tsx 先后被 S02/S06(分支 b) 触碰——均无冲突。
5. **每 Stage 3-15 项要求**：S07 为 11 项文档项（纯注记改写，单项粒度小）；其余 Stage 2-6 项，均按文件零重叠划分。

## Stage 总表

| Stage | 名称 | 改动项 | agent 数 | 脚本 |
|---|---|---|---|---|
| 01 | 工具链与门禁 | TE-01/02/03/07 | 3 | `workflows/stage-01-toolchain-guards.js` |
| 02 | 预览链路 | SEC-01/02/03 + FE-06 | 3 | `workflows/stage-02-preview-chain.js` |
| 03 | 后端加固 | BE-01/02/03/04/06 | 3 | `workflows/stage-03-backend-hardening.js` |
| 04 | 前端清理 | FE-01/02/07/09/10/11 | 4 | `workflows/stage-04-frontend-cleanup.js` |
| 05 | 大文件链路 | BE-05 + FE-03/04/05/08 | 3（顺序 A→B→C） | `workflows/stage-05-large-file-chain.js` |
| 06 | e2e 设施 | TE-04/05/06/08/09 | 3（顺序 B→C→A） | `workflows/stage-06-e2e-infra.js` |
| 07 | 文档登记 | DOC-01~11 | 3 | `workflows/stage-07-docs-registry.js` |

排序依据：S01 门禁先行（TE-03 守卫落地后后续 Stage 的 DTO 改动即被守护——S05 改 src/types/fs.ts 受益）；S02-S05 按「后端契约 → 前端消费」依赖排序（S02/S03 独立可互换，S05 的 fs_stat 依赖 S03 的 fs/mod.rs keyset 落定避免同文件交叉）；S06 依赖 S02 预览链路落地（TE-08 探针以 SEC-02 CSP meta 后的预览域为前提）；S07 文档末位。

---

## Stage 01 · 工具链与门禁（TE-01/02/03/07）

**改动项**：TE-01（check-ts7-trigger HTTP 状态码守卫）、TE-02（d.mts 入参 null）、TE-03（ci.yml src/types 漂移守卫）、TE-07（vitest 坏 exclude 清理）

### 分工表（文件零重叠）

| label | 负责项 | 文件 |
|---|---|---|
| ts7-trigger | TE-01 + TE-02 | `scripts/check-ts7-trigger.mjs`、`scripts/check-ts7-trigger.d.mts`、`src/__tests__/deps-ts7-trigger.test.ts` |
| ci-guard | TE-03 | `.github/workflows/ci.yml`、`src/types/CLAUDE.md`、`src-tauri/src/CLAUDE.md` |
| vitest-exclude | TE-07 | `vitest.config.ts` |

### 实现要点

- TE-01：getJson 加非 2xx reject + http/https 协议分派（http 分支仅供测试本地桩）+ 导出；测试用本地 http server 桩返 403/200，不打真实网络。
- TE-02：d.mts 两参改 `string | null | undefined`；测试删 `as unknown as string` cast。
- TE-03：ci.yml 照 KaTeX 守卫先例（ci.yml:59-63）插 Guard step；定向命令形态 = `--test lib_tests export_bindings`（TQ-COV-06 红线不踩）。
- TE-07：删 `vitest.config.ts:7` 的 `'datalearncodeterax-ai-temp'`。

### 验证项

→ `workflows/verify/stage-01.md`。关键断言：getJson 导出+状态码守卫存在、d.mts 含 null 声明、ci.yml Guard step 存在、本地导出+diff 两命令绿、坏 exclude 零命中、npm test 全绿（基线 3236 + TE-01 新增 2 例 = 3238 例）。

### 门禁命令

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests export_bindings -- --test-threads=1`
5. `git diff --exit-code -- src/types`

（4/5 = TE-03 守卫的本地等价验证；本 Stage 无 Rust 源码改动，收窄不跑 clippy/fmt/L1 全量——收窄理由：触碰面 = scripts/.github/vitest.config/一个 L2 测试文件。）

### commit message

`fix(review-fix): S01 工具链与门禁——TS7 触发器 HTTP 状态码守卫+d.mts null 契约+ci.yml src/types 漂移守卫+vitest 坏 exclude 清理（TE-01/02/03/07）`

### 人工验证点

- CI 实跑绿（Guard — src/types step 在真实 CI 首跑确认；本地两命令绿但 CI 环境差异无自动化覆盖）。

---

## Stage 02 · 预览链路（SEC-01/02/03 + FE-06）

**改动项**：SEC-01（validate_label 放宽 `:` `/`）、SEC-02（HOST_PAGE CSP meta）、SEC-03（PreviewFrame catch 可观测）、FE-06（注入纪律测试锁）

### 分工表（文件零重叠）

| label | 负责项 | 文件 |
|---|---|---|
| preview-label-csp | SEC-01 + SEC-02（含 src-tauri 侧文档段） | `src-tauri/src/preview.rs`、`src-tauri/src/CLAUDE.md` |
| preview-frame | SEC-03 + SEC-02（前端侧三处文档段） | `src/panels/docViewer/PreviewFrame.tsx`、`src/__tests__/html-panel.test.tsx`、`src/panels/docViewer/CLAUDE.md`、`src/panels/CLAUDE.md`、`.claude/adr.md` |
| injection-lock | FE-06 + SEC-01（L4 用例面） | `src/__tests__/doc-viewer-injection.test.ts`、`e2e-tests/html.e2e.ts` |

SEC-02 文档同步四处按归属拆分：src-tauri/src/CLAUDE.md 归 preview-label-csp；docViewer/panels/CLAUDE.md + adr.md 归 preview-frame（同 Stage 禁双 agent 碰同一文件）。

### 实现要点

- SEC-01：字符判定加 `b == b':' || b == b'/'`——与 tauri-runtime-2.11.3 window.rs:534 合法字符集（alphanumeric + `-` `/` `:` `_`）对齐；测试加页前缀形态正例、移 `"preview-a/b"` 出反例；L4 用例落 html.e2e.ts（e2e 裸 id 掩盖问题——新用例用页前缀形态 `preview-page-1:html-2` 走真实 label 路径）。
- SEC-02：HOST_PAGE `<meta charset>` 后插 CSP meta（`default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:`）；新增 L1 用例 host_page_carries_domain_csp；红线：iframe sandbox 禁 allow-same-origin（CVE-2024-35222）不动。
- SEC-03：sync catch 加一次性旗标（成功复位）；close catch 直接 console.warn。
- FE-06：段组合三矩阵断言 `<script>`/`</script>` 各恰好 1 个。

### 验证项

→ `workflows/verify/stage-02.md`。关键断言：validate_label 含 `:`/`/` 放行、HOST_PAGE 含 CSP meta、PreviewFrame catch 均带 console.warn、注入矩阵测试存在、L4 页前缀用例存在、L1/L2/L4 门禁绿。

### 门禁命令

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
7. `npx tauri build --debug --no-bundle`
8. `node e2e-tests/run-wdio.cjs --spec html.e2e.ts`（前提：窗口前台聚焦——TQ-E-10 探针）

### commit message

`fix(review-fix): S02 预览链路——validate_label 放宽 ://HOST_PAGE CSP meta 落地/PreviewFrame catch 可观测/注入纪律测试锁（SEC-01/02/03+FE-06）`

### 人工验证点

- 预览回归抽查：html/markdown 面板实机打开预览确认渲染/缩放/滚动/导航正常（联动 review 待人工清单 S10 项——本次修复后预览链路方恢复可验）。

---

## Stage 03 · 后端加固（BE-01/02/03/04/06）

**改动项**：BE-01（join_with_timeout 上提 crate 顶层）、BE-02（fs_read_dir 游标 keyset 根治）、BE-03（git 注释删 ignored）、BE-04（守卫命令改写）、BE-06（基准两轮制加固）

### 分工表（文件零重叠）

| label | 负责项 | 文件 |
|---|---|---|
| thread-join | BE-01 | `src-tauri/src/thread_join.rs`（新建）、`src-tauri/src/pty/reader.rs`、`src-tauri/src/pty/spawn.rs`、`src-tauri/src/state.rs`、`src-tauri/src/notify/mod.rs`、`src-tauri/src/hooks/watcher.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/pty/CLAUDE.md` |
| fs-keyset | BE-02 | `src-tauri/src/fs/mod.rs`、`src-tauri/src/fs/CLAUDE.md`、`src/ipc/CLAUDE.md` |
| misc-bench | BE-03 + BE-04 + BE-06 | `src-tauri/src/git/mod.rs`、`src-tauri/src/CLAUDE.md`、`src-tauri/src/agent_history/claude/scan.rs` |

### 实现要点

- BE-01：硬约束 #2（模块不穿透）→ 上提落 crate 顶层 `thread_join.rs`（app_dir/home 同形态）；KILL_JOIN_TIMEOUT 全改 JOIN_TIMEOUT 不留别名；notify/watcher 三处裸 join 换装（超时 tracing::warn + detach）；测试两裸 join（notify:968、watcher:448）改 join_with_timeout assert；两测试组（spawn.rs:2156-2186、reader.rs:882-907）合并随迁 mod join_tests。
- BE-02：keyset 游标——sort_key=(is_dir?0:1, name.to_lowercase())，base64("D\0"+lower/"F\0"+lower) 编码，`partition_point` 起始索引；前端 opaque 契约零改动；边界（大小写变体同名）登记 fs/CLAUDE.md。
- BE-04：守卫行在 src-tauri/src/CLAUDE.md:64（实读行号，review 报告 :53 已漂移——按实读改）。
- BE-06：两轮制（首轮越门槛 → sleep 2s → 重采样 20 次 → 仍越才 panic，消息附两轮中位）；头注加固留痕。

### 验证项

→ `workflows/verify/stage-03.md`。关键断言：`rg "\.join\(\)" src-tauri/src` 仅命中 thread_join.rs、游标编码形态、两用例（growth_no_dup_no_hole/beyond_end_empty_page）存在、`ignored` 注释零命中、守卫命令与文档逐字一致、重采样路径存在、L1 全量绿。

### 门禁命令

1. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
2. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
3. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

（本 Stage 触碰面 = 纯 Rust + 后端 CLAUDE.md，收窄不跑 tsc/eslint/npm test——收窄理由：零前端文件改动。）

### commit message

`fix(review-fix): S03 后端加固——join_with_timeout 上提 crate 顶层/fs 游标 keyset 根治/git 注释清理/守卫命令改写/基准两轮制（BE-01/02/03/04/06）`

### 人工验证点

无（全自动化覆盖；基准两轮制效果由 L1 全量绿承载）。

---

## Stage 04 · 前端清理（FE-01/02/07/09/10/11）

**改动项**：FE-01（fileTree 挂载竞态抑制）、FE-02（PANEL_SETTINGS 常量）、FE-07（loadRoot 续页失败双分支）、FE-09（WorkspaceDockHost 卸载清理）、FE-10（PageDockviewHost 改名 tabChrome）、FE-11（shortcuts 退役面清理）

### 分工表（文件零重叠）

| label | 负责项 | 文件 |
|---|---|---|
| file-tree | FE-01 + FE-07 | `src/features/explorer/useFileTree.ts`、`src/__tests__/use-file-tree.test.ts`、`src/features/explorer/CLAUDE.md` |
| panel-settings | FE-02 | `src/panels/panelRegistry.ts` |
| dock-host | FE-09 + FE-10 | `src/workspace/WorkspaceDockHost.tsx`、`src/workspace/PageDockviewHost.tsx`（git mv → `tabChrome.tsx`）、`src/workspace/Workspace.tsx`、`src/lib/panelId.ts`、`src/workspace/tabClose.ts`、`src/theme/schemes/types.ts`、`src/theme/schemes/linear.ts`、`knip.json`、`src/__tests__/terminal-rename-apply.test.ts`、`src/__tests__/workspace-defaulttab.test.tsx`、`src/__tests__/workspace-header-actions.test.tsx`、`src/__tests__/workspace-host-pages.test.tsx` |
| shortcuts-retire | FE-11 | `src/features/shortcuts/ShortcutRegistry.ts`、`src/features/shortcuts/types.ts`、`src/features/shortcuts/index.ts`、`src/features/shortcuts/CLAUDE.md`、`src/__tests__/shortcuts.test.ts`、`src/__tests__/html-panel.test.tsx`、`src/__tests__/markdown-panel.test.tsx` |

> 落地复核更正（review2-fix FE-05，2026-09-11）：FE-02 实际路径为 src/panelRegistry.ts（无 src/panels/ 前缀），原文保留不改写；按正确路径实跑 3 命中通过（review-04 实证）。

### 实现要点

- FE-01/FE-07 联动闭环：rootPath effect 置 restoringRef=true；restoreExpanded 三分支收口解除；loadRoot catch 双分支——续页失败 console.error 后 return（首帧保留，抑制经 restoreExpanded 路径解除），首帧失败维持现状 + restoringRef=false。
- FE-02 收窄：只改 review 点名的 :86/:118 两处字面量；组件映射表 :73 键与 pageApis.ts 不动（对象键字面量是一致形态，收窄理由已写死 checklist）。
- FE-10：`git mv` 后五处 import + 四处注释 + 两处测试头注 + **knip.json:211 ignore 键**同步（漏改则 knip 报新红）。
- FE-11：删 exportContextBindings 家族；shortcuts.test.ts 甄别（listCommands 用例保留，describe 改名）；两 panel 测试 mock 块先 grep 生产零消费再整删。

### 验证项

→ `workflows/verify/stage-04.md`。关键断言：restoringRef 置位/解除路径、PANEL_SETTINGS 常量引用、disposables 卸载消费、PageDockviewHost 全仓零命中（含 knip.json）、exportContextBindings 家族绝迹、knip 无新增红、L2 全绿。

### 门禁命令

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npx knip --production`

（FE-11 验证含 knip 断言故补第 4 条；无 Rust 改动收窄不跑 clippy/fmt/L1。）

### commit message

`fix(review-fix): S04 前端清理——fileTree 挂载竞态+续页失败保留首帧/PANEL_SETTINGS 常量/dock 卸载清理/PageDockviewHost 改名 tabChrome/shortcuts 退役面（FE-01/02/07/09/10/11）`

### 人工验证点

- 页签 chrome 行为冒烟：开页/关页/页签右键菜单/重命名实机确认无回归（改名+卸载清理的行为不变量，联动 review 待人工清单 S11 项）。

---

## Stage 05 · 大文件链路（BE-05 + FE-03/04/05/08）

**改动项**：BE-05（fs_stat 命令全链）、FE-03（行文本色响应式）、FE-04（信息条真实字节）、FE-05（块缓存失效重扫）、FE-08（10MB 预检前置 stat）

**跨边界契约写死**（agent 不各自推断）：

- IPC 命令：`fs_stat({ path: string }) -> FsMetadata`；`FsMetadata = { sizeBytes: number, mtimeMs: number | null }`（Rust `FsMetadata` derive TS，snake_case→camelCase 经 ts-rs）。
- 前端 wrapper：`src/ipc/fs.ts` 增 `statFile(path: string): Promise<FsMetadata>`。
- 信号形态：`LargeFileSignal = { filePath: string }`（删 sizeBytes）；`LargeFileViewerProps` 删 fileSizeBytes。
- `useLineIndex` 签名：`(filePath: string, fileRev: number)`；fileKey 复合 `${filePath}#${fileRev}`。

### 分工表（顺序 A→B→C，文件零重叠）

| label | 负责项 | 文件 |
|---|---|---|
| fs-stat | BE-05 | `src-tauri/src/fs/mod.rs`、`src-tauri/src/lib.rs`、`src-tauri/build.rs`、`src-tauri/capabilities/default.json`、`src/types/fs.ts`（生成物，禁手改——export_bindings 生成）、`src/ipc/fs.ts`、`src-tauri/src/fs/CLAUDE.md`、`src/ipc/CLAUDE.md` + L1/L2 测试落点（按 checklist BE-05 测试同步节） |
| editor-precheck | FE-08 + FE-04（信号侧） | `src/panels/editor/useCodeMirror.ts`、`src/panels/editor/EditorPanel.tsx`、`src/panels/gitshow/GitShowPanel.tsx`、`src/panels/diff/DiffPanel.tsx`、`src/panels/editor/CLAUDE.md`、`src/panels/CLAUDE.md` + 测试落点（按 checklist FE-08/FE-04 测试同步节） |
| large-viewer | FE-03 + FE-04（viewer 侧）+ FE-05 | `src/panels/editor/largeFileViewer/LargeFileViewer.tsx`、`src/panels/editor/largeFileViewer/blockCache.ts`、`src/panels/editor/largeFileViewer/useLineIndex.ts`、`src/__tests__/large-file-viewer.test.tsx` |

顺序依据：editor-precheck 消费 fs-stat 的 statFile wrapper（FE-08 预检）；large-viewer 消费 statFile（FE-04 fileMeta）+ editor-precheck 的信号形态（LargeFileSignal 删 sizeBytes 双侧同步）。

### 实现要点

- BE-05：三处注册（generate_handler + build.rs commands 42→43 + capabilities allow-fs-stat）；DTO 生成后 `git diff --exit-code -- src/types` 必绿（TE-03 守卫已落地，本 Stage 首次真实触发表单）。
- FE-08：读盘前 stat 预检（>MAX 零读盘返回；>WARN confirmDialog 真实字节文案；`filePathRef.current === undefined` 判取消）；读后保留 doc.length 复核（TOCTOU 防线）。
- FE-05：fileGen 代际 Map + invalidateFile；onFsEvent 订阅照 useCodeMirror.ts:587-623 形态（Modify 过滤 + 路径归一化命中 → 重 stat 比对 → invalidateFile + setFileRev+1）；静默重载（只读无 dirty）。
- FE-04 收窄登记：GitShow/Diff 的 10MB 判定保留 text.length（blob 无 stat 通道，checklist 已登记）。

### 验证项

→ `workflows/verify/stage-05.md`。关键断言：fs_stat 三处注册齐全、FsMetadata DTO 生成物形态、statFile wrapper、stat 预检在 readFile 之前、LargeFileSignal 仅 filePath、fileGen/invalidateFile 存在、`src/types` diff 绿、L1/L2 全绿。

### 门禁命令

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
4. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
5. `npm test`
6. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`

### commit message

`fix(review-fix): S05 大文件链路——fs_stat 真实字节/行文本色响应式/块缓存文件变更失效重扫/10MB 检查前置 stat 零读盘（BE-05+FE-03/04/05/08）`

### 人工验证点

- >10MB 文件实机滚动/渲染观感抽查（联动 review 待人工清单 S09/CP-022 项）；
- 外部修改浏览中的大文件 → viewer 失效重扫行为实机抽查（FE-05 行为面）。

---

## Stage 06 · e2e 设施（TE-04/05/06/08/09）

**改动项**：TE-04（哨兵补 env）、TE-05（TQ-E-10 探针首 worker fast-fail+其余降级）、TE-06（writeFakePlanEnv 收编 node-helpers.ts）、TE-08（KaTeX 字体真实加载锚点）、TE-09（CP-030 矛盾句改写+WARN 计数可观测化）

### 分工表（顺序 B→C→A，文件零重叠）

| 顺序 | label | 负责项 | 文件 |
|---|---|---|---|
| 1 | plan-env-helper | TE-06 | `e2e-tests/node-helpers.ts`（新建）、`e2e-tests/settings.e2e.ts`、`e2e-tests/background-tasks.e2e.ts` |
| 2 | katex-font-probe | TE-08 | `e2e-tests/markdown.e2e.ts`；分支 b 时追加 `src/panels/docViewer/buildInjectedScript.ts`、`src/panels/docViewer/previewMessages.ts`、`src/panels/docViewer/PreviewFrame.tsx`、`src/panels/markdown/MarkdownPanel.tsx`、`src/__tests__/doc-viewer-preview-messages.test.ts`、`src/__tests__/markdown-panel.test.tsx`、`src/__tests__/doc-viewer-injection.test.ts`、`src/__tests__/html-panel.test.tsx` |
| 3 | wdio-launcher | TE-04 + TE-05 + TE-09 + e2e-tests/CLAUDE.md 全部登记 | `e2e-tests/run-wdio.cjs`、`e2e-tests/wdio.conf.ts`、`e2e-tests/CLAUDE.md` |

顺序依据：e2e-tests/CLAUDE.md 统一归 wdio-launcher（同 Stage 禁双 agent 碰同一文件）——含 TE-06 helper 分工条与 TE-08 实证结论登记，均由 wdio-launcher 读代码现状后按实写（`rg "fonts.check|fontProbe" e2e-tests/markdown.e2e.ts src/panels/docViewer/` 判定分支），不依赖前序 agent 报告转述。katex-font-probe 先于 wdio-launcher 跑，使其落地形态可被读。

### 实现要点

- TE-04：`SETTINGS_SENTINEL_KEYS` 加 `"env"`；两处「唯一可能写入」声明改写；误报面登记（e2e 期间勿动 claude env 配置）。
- TE-05：`WDIO_WORKER_ID`（实证 @wdio/local-runner build/index.js:257，cid 形如 `"0-0"`）——首 worker（undefined 或 `0-` 前缀）维持 fast-fail，其余降级 warn 继续。
- TE-06：收编点 = 新建 node-helpers.ts（helpers.ts 是浏览器侧 VITE_E2E 打包件，Node fs 入包即破构建——偏离 review 建议的留痕理由）；以 settings 版为基（含 mkdirSync）。
- TE-08 **实证驱动**：先跑分支 a 探针（宿主页 document.fonts.check 双对照）——`katex===true && control===false` 则分支 a 成立固化为常驻断言；否则转分支 b（fontProbe 段+上行白名单第四类型+PreviewFrame 收束）。实证输出记入 Stage commit body。
- TE-09：run-wdio.cjs 两通道（runWdio execSync + fallback spawn）统一改 spawn `stdio:['inherit','pipe','pipe']` + FORCE_COLOR=1 + 转发正则计数 + close 时打印计数行；CLAUDE.md:116 矛盾句改写 + WARN 基线登记（基线数字执行期实跑填记）。

### 验证项

→ `workflows/verify/stage-06.md`。关键断言：哨兵键含 env、探针降级分支存在、writeFakePlanEnv 单点、markdown.e2e.ts 字体锚点断言存在（分支 a）或 fontProbe 链路齐全（分支 b）、WARN 计数行输出、CLAUDE.md 矛盾句绝迹、`npm run e2e` 全量绿。

### 门禁命令

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `npm test`
4. `npm run e2e`（= build:e2e + wdio 全量；e2e-tests/ 不在根 tsconfig include，构建级门禁 = 实跑兜底；前提：窗口前台聚焦）

### commit message

`test(review-fix): S06 e2e 设施——哨兵补 env/探针首 worker fast-fail+降级/writeFakePlanEnv 收编 node-helpers/KaTeX 字体锚点（实证分支 X）/WARN 计数可观测（TE-04/05/06/08/09）`

（commit body 须含 TE-08 探针实证输出与 WARN 计数基线实测值。）

### 人工验证点

- TE-04 负向验证：改真实屋 `~/.claude/settings.json` 的 env 哨兵键 → 跑 `node e2e-tests/run-wdio.cjs --spec settings.e2e.ts` → exit 校验报红（联动 review 待人工清单 S03/CP-046 项；验证后还原真实屋）。

---

## Stage 07 · 文档登记（DOC-01~11）

**改动项**：DOC-01~11（compromises.md 注记回写 ×8、根 CLAUDE.md 去计数+串行纪律、test-exemptions 哨兵口径、.temp/node22.bak 删除、compromises-fix 执行档回写、CP-033 红测演练补做）

### 分工表（文件零重叠）

| label | 负责项 | 文件 |
|---|---|---|
| compromises-notes | DOC-01、DOC-02（注记面）、DOC-05、DOC-06（1）、DOC-07、DOC-08、DOC-11 | `docs/compromises.md` |
| claude-md-registry | DOC-09、DOC-10、DOC-02（删 .bak 动作） | `.claude/CLAUDE.md`、`.claude/test-exemptions.md`、`.temp/node22.bak`（删除，untracked 纯文件系统动作） |
| exec-doc-replay | DOC-03、DOC-04、DOC-06（2/3） | `docs/compromises-fix/checklist.md`、`docs/compromises-fix/workflows/verify/stage-10.md`、红测演练执行（临时改 `src/panels/markdown/generated/katexInlineCss.ts` → `git checkout` 还原，零持久变更） |

### 实现要点

- 全部注记按 checklist DOC 条目原文改写；历史执行档回写以「落地复核」注记形式追加，**不改写历史断言原文**（防篡改历史记录语义）。
- 红测演练四行（改一字符 → diff 非 0 → 还原 → diff 归 0）输出入 Stage commit body（verify 据此断言）。
- DOC-09 串行纪律条自 Stage 01 已在执行，此处仅登记。

### 验证项

→ `workflows/verify/stage-07.md`。关键断言：各注记新表述命中+旧失实表述零命中、.temp/node22.bak 不存在、721 计数绝迹、哨兵键级口径、commit body 含红测演练记录、演练终态 diff 归 0。

### 门禁命令

1. `npx tsc --noEmit`
2. `npx eslint src/`
3. `node scripts/gen-katex-inline.mjs && git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts`（演练终态归位确认）

（纯文档 Stage 收窄门禁——无 npm test/cargo test：触碰面为 md 文件 + untracked 目录删除 + 演练还原零持久变更；tsc/eslint 防文档 agent 误碰代码。）

### commit message

`docs(review-fix): S07 登记收口——compromises 注记回写/根 CLAUDE.md 去计数+L1/L2 串行纪律/test-exemptions 哨兵口径/CP-033 红测演练补做（DOC-01~11）`

（commit body 须含红测演练四行输出记录。）

### 人工验证点

无（文档 Stage）；全部 Stage 完成后的整体人工验证清单见 execution-plan.md 收尾节。
