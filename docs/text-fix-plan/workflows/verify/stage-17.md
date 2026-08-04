# Stage 17 逐项验证断言（唯一真值源）

> stage-17 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；全量测试任一命令失败则相关项判 not_fixed。
> 本 Stage 无代码门禁——收尾跑 frontendTest + rustTest 确认文档 Stage 零代码副作用。

<!--
生成纪律：
1. 逐 ID 对照 checklist 原文写断言，禁止凭记忆——断言内容与 checklist 条目一一对应
2. 每条断言必须可机械检验（grep 模式 / Read 确认 / 测试命令），不写"检查是否合理"
3. "禁止存在 X"类断言写语义式，防"改名迎合"；正向意图断言同样写语义式，防"字面通过"
4. 本文件由 stage 脚本与 fix-loop 共用——断言变更只改本文件，两处自动生效
5. 门禁命令按 Stage 触碰文件选择——触及 tsc/eslint 覆盖外文件时补 npx vite build 构建级兜底
6. 断言与该 Stage 完成后的真实中间态一致——计数/枚举类按中间态推导，不照抄终态（用例数 = 各 Stage 完成后实际统计值）
7. 断言证据在本 Stage 门禁命令产出内——不可得则收窄取数口径（静态 grep 计数）或补门禁命令
-->

## 断言清单

- **DOC-01**：`.claude/test-inventory.md` 含豁免表（项目/豁免原因/当前兜底层级三列）且覆盖范围与 checklist DOC-01 一致（reader_loop 残余、spawn_conpty_child Win32 部分、lib.rs run()、ActivityBar 拖拽 mock 理想化、E2E_ENABLED=false 生产分支、L3 WebGL/mouse tracking、L4 真实 OS 按键、HTML postMessage 真实 WebView2 行为；Read 确认各豁免项登记）
- **DOC-02**：定位声明落位——`e2e-tests/CLAUDE.md` 含半端到端/部分端到端声明（L4 键盘/拖拽/恢复）；`test/terminal/` 文件头或 README 含"网格状态正确性，非渲染正确性"声明；L2 jsdom postMessage 模拟标注（Read 确认）
- **DOC-03**：test-inventory.md 全量校正——①"notification 权限声明"等 stale 条目零残留（grep 确认）；②各域用例数与 `npm test`/`cargo test`/`test:l3` 实际统计一致（静态口径：逐域 grep `it(`/`#[test]` 计数，与各 Stage 完成后的实际值比对）；③豁免表与定位声明已登记
- **DOC-04**：子路径 CLAUDE.md 测试模式章节同步——测试拆分（GIT-12/SVC-14/E2E-09）、新增测试文件（HKC-08/IHE-02/IHE-06 helper）、测试模式变化（block_on 命令层模式/EventEmitter trait/ScanRootGuard）、git CLI 最低版本声明（GIT-08 产出）同步到对应模块 CLAUDE.md；抽查 git/hooks/fs 三模块 CLAUDE.md 测试文件表与磁盘实际一致（Glob 对照）；claude_history/CLAUDE.md 已纳入同步范围
- **零代码副作用**：`npm test` + `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 全绿；git diff 确认本 Stage 只改文档（无 src/ src-tauri/src/ e2e-tests/ test/ 下代码变更）

## 全量测试（全部通过为门禁）

1. `npm test`
2. `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
