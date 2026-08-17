# Stage 19 逐项验证断言（唯一真值源）

> stage-19 脚本与 fix-loop 的 verify agent 均以本文件为准。
> 方法：用 Grep/Read 逐条核实，给出证据（文件+行号）；门禁任一失败则相关项判 not_fixed。
> **文档类断言总纲**：文档描述必须对照当前代码核实——与代码矛盾即判 not_fixed（防文档撒谎），不接受「措辞合理」。

## 断言清单

- **DOC-01**：`.claude/CLAUDE.md` 约束 #11 含新措辞（语义式：可自动化部分必须覆盖 + 不可自动化部分须 test-inventory 豁免清单登记并注明原因与兜底层级——Read 确认）
- **DOC-02**：`.claude/CLAUDE.md` 约束 #9 区分业务 cfg（pty/conpty_api/shell/win_build）与测试 cfg（原则上 cfg!()，例外须模块 CLAUDE.md 登记）——Read 确认
- **DOC-03**：`.claude/CLAUDE.md` 含 store 约束（语义式：只存状态不存业务逻辑、持久化经指定 IPC、禁止跨 store 隐式依赖）
- **DOC-04**：`.claude/CLAUDE.md` 含注册表家族通用契约（语义式：模块级单例、register/getAll/_reset、side-effect import、_reset 测试隔离）
- **DOC-05**：`.claude/CLAUDE.md` 约束 #5 含 hub 容器 + 注册表分派子编辑器形态条款（hooksConfig 先例）
- **DOC-06**：`.claude/CLAUDE.md` 约束 #4 含语义值集同步登记条款
- **DOC-07**：`.claude/CLAUDE.md` 约束 #6 含完整例外清单 + 新增例外须同步登记条款
- **DOC-08**：`README.md` 存在于项目根，含项目定位、构建/测试命令、文档链接（CONTEXT.md/adr.md/test-inventory）——Read 确认三要素
- **DOC-09**：`CONTEXT.md` 面板类型表述为 `htmlviewer`（grep 命中）；对照 `src/panelRegistry.ts` 注册 id 核实一致
- **DOC-10**：`.claude/adr.md` 含 FE-01（Workspace 多实例 + MAX_PAGES=20，对照 `src/stores/projects.ts` 核实上限值）、SEC-09（CSP unsafe-inline 保留，对照 `src-tauri/tauri.conf.json` 核实）、09#14（Mutex 中毒保持现状）登记——逐条 grep + 对照代码核实
- **DOC-10**：模块 CLAUDE.md 关键事实抽查（逐条对照代码，任一失实判 not_fixed）：① 命令数 34（`src/ipc/CLAUDE.md` 与 `src-tauri/src/CLAUDE.md` 对照 `lib.rs` generate_handler! 计数）；② `src-tauri/src/pty/CLAUDE.md` 含 MAX_PTY_SESSIONS=32 / reader 微批 / pty_kill_all（对照 spawn.rs/reader.rs）；③ `src-tauri/src/notify/CLAUDE.md` 含 WATCH_EXCLUDE_DIRS 七元素与容量 8（对照 notify/mod.rs/pool.rs）；④ `src-tauri/src/CLAUDE.md` 含 app_dir 模块登记（对照 app_dir.rs 存在）；⑤ `src/types/CLAUDE.md` 含 HooksLayer 收窄登记；⑥ `src/features/explorer/CLAUDE.md` 含 FileTree 虚拟化
- **DOC-10**：`.claude/test-inventory.md` 豁免表 reader_loop 项已按 S06 后现状重写（对照 `src-tauri/src/pty/reader.rs` 微批实现核实描述不撒谎）；S01~S18 新增用例已登记（抽查 S02 安全用例、S05 watcher 用例、S08 appError 用例、S13 pty_kill_all 用例在清单中）

## 门禁（全部通过为准）

1. `npx tsc --noEmit`（静态兜底，确认文档 Stage 未误伤类型）
2. `git diff --name-only HEAD` 输出只含文档类文件（.claude/*.md、src/**/CLAUDE.md、src-tauri/**/CLAUDE.md、CONTEXT.md、README.md）——发现代码文件（.ts/.tsx/.rs/.toml/.json/.yml）变更即判失败并列出文件名
