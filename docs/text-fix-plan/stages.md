# 自动化测试优化 Stage 划分（stages）

> 164 项 → 17 Stage | Stage 串行执行、每 Stage commit | 清单原文见 `docs/text-fix-plan/checklist.md`
> 编号引用一律用 ID（PTY-01 等），禁用"第几个问题"

## 全局约定

- **禁区（全 Stage 生效，写入各脚本 PREAMBLE）**：
  1. `compute_conpty_flags` 固定 0x7（含 4 条守卫测试），任何 agent 不得修改 ConPTY flags——PASSTHROUGH_MODE (0x8) 吞 claude TUI 鼠标滚轮
  2. L4 E2E 不得触碰真实 `~/.claude/projects/`（`SLTERM_CLAUDE_PROJECTS_DIR` env 隔离 fixture）
  3. C10 契约不可改：`slterm-hook-reporter.js` 任何代码路径必须 `process.exit(0)`
  4. cargo test 恒 `--test-threads=1`（ConPTY 并发 spawn 死锁）
- **生产代码改动边界**：仅 checklist 标注的最小可测性重构（抽纯函数/参数注入/导出符号，D2 零行为变更）；其余一律只改测试与文档
- **并行 agent 测试纪律**：同一 Stage 并行 agent 不跑共享资源测试——Rust 侧只做 `cargo test --no-run` 编译级检查，真实执行由全量测试 agent 单点跑；cargo 命令共享 target 锁排队属正常，勿中止
- **命名错位说明**：checklist 定稿编号与早期草案有两处移位，以 `checklist.md` 为准：PTY-12=reader_loop I/O（reader.rs）、PTY-13=spawn.rs 清理+state/shell 边界（跨三文件，拆分执行）
- **偏离规则豁免**：
  - Stage 03 用 pipeline（拆分先行）——GIT-12 改变测试文件布局，后续项依赖新布局
  - Stage 01/15/16 单 agent——同文件串行（01）、L3 同配置同目录（15）、WDIO 单实例+拆分需全局视野（16）
  - PTY-13、SVC-13 跨文件项按文件拆分到多 agent，verify 合并断言
  - Stage 05 为 15 项满格——文件天然四分无重叠，不再拆 Stage

## 人工验证点（执行收尾逐项实测）

| # | 时点 | 验证内容 |
|---|------|---------|
| M1 | Stage 01 完成后 | `npx tauri build --debug --no-bundle` 构建产物实测真实 claude 会话：滚轮/键盘输入/Ink 渲染无回归（spawn.rs 可测性重构零行为变更的实证兜底，自动化无法守卫） |
| M2 | Stage 16 完成后 | L4 视觉回归用例（E2E-04）截图/渲染基线人工确认——WebGL→DOM 回退不白屏属视觉判定 |
| M3 | Stage 16 完成后 | 确认 E2E 运行未触碰真实 `~/.claude/projects/`；`~/.claude/settings.json` 已还原、`~/.slterminal/hooks*/` 已清理 |
| M4 | 全部 Stage 完成后 | L1 `cargo test -- --test-threads=1` + L2 `npm test` + L3 `npm run test:l3` + L4 `npm run e2e` 四级全量绿 |

---

## Stage 01 L1-PTY：spawn.rs 校验与可测性重构（7 项）

- **项**：PTY-01、PTY-02、PTY-03、PTY-07、PTY-08、PTY-09、PTY-13①（spawn.rs 清理部分）
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| pty-spawn | PTY-01/02/03/07/08/09/13① | `src-tauri/src/pty/spawn.rs`、`src-tauri/tests/pty_integration_tests.rs` |

- **实现要点**：单 agent 同文件串行。抽 `validate_spawn_request`（尺寸/白名单/cwd 三校验）、`validate_session_ownership`（SEC-08）纯函数；Job Object 抽 job_name/limit flags 纯逻辑；`build_cmdline` 引号、`ConPtyMaster::resize` invalid 分支、`spawn_conpty_child` 可纯化部分（命令行/环境块构造）补测；测试清理抽 helper。**M1 人工验证点**。
- **门禁**：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` + `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
- **验证项**（详见 `verify/stage-01.md`）：
  1. `spawn.rs` 存在 `validate_spawn_request` 与 `validate_session_ownership` 纯函数且被对应命令调用（Read 确认，不限签名细节）
  2. 新增用例覆盖：尺寸超限拒绝、shell 白名单拒绝、cwd 沙箱拒绝、SEC-08 归属放行/拒绝——均调真实命令或抽出纯函数
  3. `compute_conpty_flags` 及其 4 条守卫测试零改动（git diff 无命中）
  4. `cargo test pty -- --test-threads=1` 全绿，pty 域用例数 ≥ 基线 105+新增
  5. clippy 零警告；无 `#[cfg(windows)]` 新增到 spawn.rs 以外（硬约束 #9）

## Stage 02 L1-PTY：reader/shell/state 边界（7 项）

- **项**：PTY-04、PTY-05、PTY-06、PTY-10、PTY-11、PTY-12、PTY-13②③（state/shell 边界部分）
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| pty-reader | PTY-04、PTY-12 | `src-tauri/src/pty/reader.rs` |
| pty-shell | PTY-06、PTY-10、PTY-13③ | `src-tauri/src/pty/shell.rs` |
| pty-state | PTY-05、PTY-11、PTY-13② | `src-tauri/src/state.rs` |

- **实现要点**：PTY-12 按 D6 评估 reader_loop 可抽决策点（channel 断开→ring buffer 分流、EOF 处理），能抽则注入参数补 L1，残余不可抽分支在 pty/CLAUDE.md 留豁免标注草稿（Stage 17 统一收编）。shell 回退用 tempdir 放假 exe 构造可控 PATH。
- **门禁**：同 Stage 01
- **验证项**：
  1. OSC 1/3/4/9 保留、CSI 3J、非 Windows 原样返回分支（或 cfg 守护标注）各有用例
  2. ring_buffer 无换行淘汰三边界（恰好 1024/超 1024/含换行）有用例
  3. shell 回退顺序（pwsh→powershell→cmd）经可控 PATH 验证；白名单 PATH 解析后仍非法拒绝有用例
  4. `..` 穿越沙箱拒绝用例存在且调 `validate_path_within_root`
  5. reader_loop 抽取决策点有用例，或 pty/CLAUDE.md 出现豁免标注草稿（二选一必有其一，Read 确认）

## Stage 03 L1-GIT：命令层重写 + 拆分（12 项）

- **项**：GIT-01~12
- **分工表**（pipeline，stage1 先行）：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| git-restructure（先行） | GIT-12、GIT-11、GIT-06 | `src-tauri/src/git/mod.rs`（测试区拆分）、拆分新测试文件、`src-tauri/tests/ci_config_tests.rs`（新建）、共享 test_utils |
| git-commands | GIT-01、GIT-02、GIT-03、GIT-09、GIT-10 | `src-tauri/src/git/mod.rs`（源码最小抽函数）、拆分后 rollback/unstage/at_head 测试文件 |
| git-units | GIT-04、GIT-05、GIT-07、GIT-08 | 拆分后 status/diff 测试文件（不碰 git/mod.rs 源码） |

- **实现要点**：先拆分（按命令分文件 + `init_temp_repo`/`commit_file` 提取 test_utils + 仓库局部 git config 隔离），再并行：commands agent 用 `block_on` 调真实五命令（每命令 happy/沙箱拒绝/错误契约 ≥3 条）+ 删除/重写 two_step 废弃测试 + 重写 old_path 假测试；units agent 补 conflict/hunks 边界/弱断言精确化/改名。8.3 短名坑：`init_temp_repo` 保持 `dunce::canonicalize`。
- **门禁**：同 Stage 01
- **验证项**：
  1. 五命令各有经 `block_on` 调真实命令的用例（grep `block_on` 命中各命令测试文件）
  2. `git_rollback_two_step_` 前缀用例不复存在（grep 零命中）或显式标注已废弃（二选一）
  3. `non_renamed_old_path` 用例重写为构造非 renamed/renamed 两路断言（Read 确认，不接受恒真条件）
  4. `git/mod.rs` 测试区 `#[test]` 计数下降、拆分后新文件计数上升，git 域总用例数 ≥ 基线 88−删除数+新增数（inventory 记录口径）
  5. `ci_l1_uses_single_test_thread` 位于 `tests/ci_config_tests.rs`（grep 命中），git/mod.rs 零命中

## Stage 04 L1-hooks：信号链与注入命令（11 项）

- **项**：HUK-01~11
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| hooks-signal | HUK-01、HUK-04、HUK-09 | `src-tauri/src/hooks/signal.rs`、`src-tauri/src/hooks/mod.rs` |
| hooks-inject | HUK-02、HUK-03、HUK-08、HUK-11 | `src-tauri/src/hooks/inject.rs`、`src-tauri/src/hooks/watcher.rs` |
| hooks-usage-config | HUK-05、HUK-06、HUK-07、HUK-10 | `src-tauri/src/hooks/usage.rs`、`src-tauri/src/hooks/config.rs` |

- **实现要点**：`process_signal_file` emit 抽注入参数，tempdir 验证读→emit→删 + emit 失败仍删；三注入命令抽路径可注入 impl，tempdir 驱动注入/幂等/非法中止/卸载混组保用户 handler；watcher 拆 `run_one_tick` 或临时目录真实启动验证轮询补漏 + 目录重建恢复；`start_signal_watcher` 加 `#[cfg(test)]` 重置钩子。**不改 `assets/slterm-hook-reporter.js`（C10）**。
- **门禁**：同 Stage 01
- **验证项**：
  1. `process_signal_file` 全流程用例存在（读→emit→删断言，emit 可注入）
  2. 注入三命令经 impl 路径的 L1 场景用例存在（含非法 JSON 中止、版本三态）
  3. watcher 轮询补漏消费残留 + 目录重建恢复有用例（集成或 run_one_tick）
  4. `inject_adds_10_events` 改结构断言（type/matcher/timeout/command 键集合）
  5. serde 测试改往返精确断言（`contains` 弱断言零残留于 hooks/mod.rs、signal.rs）
  6. `slterm-hook-reporter.js` 零改动（git diff 无命中）

## Stage 05 L1-外围：fs/notify/history/settings（15 项）

- **项**：HFN-01~09、SPE-01~06
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| l1-fs | HFN-01、HFN-04、HFN-08 | `src-tauri/src/fs/mod.rs` |
| l1-notify | HFN-02、HFN-03、HFN-07、HFN-09① | `src-tauri/src/notify/mod.rs`、`src-tauri/src/notify/pool.rs` |
| l1-history | HFN-05、HFN-06、HFN-09②③ | `src-tauri/src/claude_history/scan.rs`、`src-tauri/src/claude_history/ops.rs` |
| l1-settings | SPE-01~06 | `src-tauri/src/settings.rs`、`src-tauri/src/projects.rs`、`src-tauri/src/error.rs` |

- **实现要点**：fs 写测试改调真实 `fs_write_file` 命令（消除同构循环断言）+ transmute 移除（抽纯函数）；notify 抽 `EventEmitter` trait 补事件循环 L1 + pool 替换分支去手动 remove + Drop 测试改轮询；history 引入 `ScanRootGuard` RAII + 命令包装与 IO 降级用例；settings `block_on` 调真实命令 + `app_data_dir` 注入 tempdir + error.rs 三 From + persist 失败映射。SLTERM_CLAUDE_PROJECTS_DIR env 测试依赖 `--test-threads=1`。
- **门禁**：同 Stage 01
- **验证项**：
  1. fs 写文件用例调真实命令（grep `fs_write_file` 命中测试区），无重写 `use_crlf` 检测逻辑残留（Read 确认）
  2. `EventEmitter` trait 存在且 L1 用 mock emitter 驱动（Read 确认）；pool 替换测试无手动 remove（grep 确认）
  3. `ScanRootGuard`（或等价 RAII）存在，scan.rs env 测试经 guard（Read 确认）
  4. settings/projects 用例经 `block_on` 调真实命令（grep 命中）；error.rs 三 From 各有用例
  5. 全部测试 `cargo test -- --test-threads=1` 绿；无新增 transmute（grep 零命中于 fs 测试区）

## Stage 06 L2-terminal：去重 + webgl + mock 清理（9 项）

- **项**：TRM-01~08、NAH-02
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| term-xterm | TRM-01、TRM-02、TRM-03、TRM-04 | `src/__tests__/use-xterm-lifecycle.test.ts`、`use-xterm-output.test.ts`、`e2e-gating-terminal.test.ts`、`src/__tests__/helpers/xterm-test-utils.ts` |
| term-panel | TRM-05、TRM-06、TRM-07、TRM-08、NAH-02 | `src/__tests__/terminal.test.tsx`、`detectWebgl.test.ts`（及新增 webgl 测试）、useTerminalInstance 测试、`terminal-registry.test.ts` |

- **实现要点**：14 条重复用例去重归位（合帧→output、生命周期→lifecycle），`await Promise.resolve()` 时序改 fake timers/显式 flush；删 setBufferType 虚假用例与死辅助；清三文件 mock `hooks:` 字段；webgl 退避/耗尽/cancel 用 fake timers 全分支；setClaudeSession merge 语义（undefined 不覆盖/lastEventAt 自动填/null 清空）。
- **门禁**：`npx tsc --noEmit` + `npx eslint src/` + `npm test`
- **验证项**：
  1. lifecycle 与 output 间重复用例消除（同名/同断言用例只存一处，Read 抽查）
  2. `setBufferType` 在测试与 helper 中零残留（grep 零命中）
  3. 三文件 mock 中 `hooks:` 字段零残留（grep 零命中）
  4. webgl 退避/耗尽/cancel 分支各有用例（Read 确认）；usePtyOutput 64KB 淘汰有用例
  5. terminal-registry 测试含 merge 语义三条（增量保留/lastEventAt/null 清空）
  6. `npm test` 全绿，终端域用例数变化与 inventory 口径一致

## Stage 07 L2-editor/diff：保存链与分支补齐（9 项）

- **项**：EDF-01~09
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| ed-diff | EDF-01、EDF-02、EDF-06、EDF-07 | `src/__tests__/diff-panel.test.tsx`、`diff-alignment.test.ts` |
| ed-cm | EDF-03、EDF-04、EDF-05、EDF-08、EDF-09 | `src/__tests__/useCodeMirror.test.ts`、`gitshow-panel.test.tsx`、`gitGutter.test.ts` |

- **实现要点**：保存链真实触发（writeFile→gitDiff 重调→gutter/占位刷新断言）；DiffPanel 五分支（占位/.git 刷新/脏确认/滚动重绑/大文件）补测；滚动同步去 200ms 固定等待；大文件拒绝/警告/保存失败 mock fs+confirm；gitshow 切换断言 EditorView identity。
- **门禁**：同 Stage 06
- **验证项**：
  1. 保存链用例断言 `fs.writeFile` → `gitDiff` 重调 → gutter 更新全链（非 toBeDefined）
  2. DiffPanel 五分支（refreshPlaceholders/.git/脏确认/滚动重绑/大文件）各有用例
  3. diff 测试中固定 `200ms` 延时零残留（grep 零命中）
  4. useCodeMirror 大文件三分支（>10MB 拒绝/>1MB 取消/writeFile reject）有用例
  5. gitGutter 四 wrapper 各有直接调用用例

## Stage 08 L2-workspace：真实组件与启动顺序（11 项）

- **项**：WRK-01~11
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| wk-host | WRK-01、WRK-02、WRK-05、WRK-06、WRK-09 | PageDockviewHost/DefaultTab/multi-instance/switch-order/pageApis 相关测试文件 |
| wk-shell | WRK-03、WRK-04、WRK-07、WRK-08、WRK-10、WRK-11 | startup-restore/app/ipc-window/layout-serde/close-handler/main/titleManager/panel-registry 相关测试文件 + `src/ipc/window.ts`（WRK-04 决策：删除或标注预留） |

- **实现要点**：真实 DefaultTab（tabIcon emoji/img 分支，事件结构 `event.tabIcon` 非 `event.params`）；Watermark/RightHeader/handleReady 空布局不兜底；`switchToPageShared` 断言 setProjectRoot→setActivePage 顺序 + `__dockviewApi` 重指（D7 时序断言）；switchToPageAndFocus 轮询命中/超时降级；启动恢复 spy 顺序断言；layout-serde mock 对齐真实 6 种 PANEL_TYPES；WRK-04 先查消费方再定删除/预留/补测。
- **门禁**：同 Stage 06
- **验证项**：
  1. DefaultTab 用例渲染生产组件（非手写 Mock，Read 确认）
  2. switchToPageShared 用例含调用顺序断言（spy invocationCallOrder 或等价）
  3. layout-serde mock 的合法面板类型与真实 `PANEL_TYPES` 一致（Read 对照 panelRegistry.ts）
  4. 启动恢复用例含 setProjectRoot 先于 setActivePage 断言
  5. WRK-04 处置三选一落实（删除/标注预留/契约测试），ipc/window.ts 与测试一致
  6. multi-instance 用例含实例 identity 断言（非仅 CSS display）

## Stage 09 L2-explorer/sidebar：高频路径补齐（12 项）

- **项**：EXP-01~12
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| ex-panel | EXP-01、EXP-02、EXP-03、EXP-04、EXP-06、EXP-07、EXP-10、EXP-11 | `src/__tests__/explorer-*.test.tsx`（含 useFileTree/FileTree 相关） |
| ex-side | EXP-05、EXP-08、EXP-09、EXP-12 | `src/__tests__/sidebar-actions.test.ts`、FileIcon 测试、`file-viewer-registry.test.ts` |

- **实现要点**：OpenInTerminal 断言 addPanel 全参数（cwd 文件取父目录/panelId 格式/renderer always）；CRUD 成功路径补 IPC+refresh+状态重置断言；fullRefresh 死代码确认后删除或接线；generation 过期丢弃照系统性改法（旧请求延迟 resolve）。
- **门禁**：同 Stage 06
- **验证项**：
  1. OpenInTerminal 用例断言 addPanel 参数含 `component:"terminal"`、正确 cwd、`renderer:"always"`
  2. 删除/重命名/新建三 CRUD 各有成功路径断言（IPC+refresh+状态重置）
  3. fullRefresh 处置落实（删除或接线+用例，二选一）；F8 用例名实一致
  4. FileIcon 表驱动覆盖 .pyw/.markdown/.less/.scss/.gitattributes
  5. E6 标题与断言一致；explorer 测试用例编号无重复

## Stage 10 L2-sideviews/commit：交互精度与拆分（14 项）

- **项**：SVC-01~14
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| sv-activity | SVC-01、SVC-05、SVC-06、SVC-07、SVC-10、SVC-13①（sideBarState 部分） | `src/__tests__/activityBar.test.tsx`、`sideBarArea.test.tsx`、`workspace-sideviews.test.tsx`、`sideBarState.test.ts` |
| sv-store | SVC-02、SVC-13②（stores/sideBar clamp 部分） | `src/__tests__/sideBar.test.ts`、`font-size.test.ts`、`keybindings.test.ts` |
| sv-commit | SVC-03、SVC-04、SVC-08、SVC-09、SVC-11、SVC-12、SVC-14 | `src/__tests__/commit-view.test.tsx`（拆三文件）、commitContextMenu 测试、openCommitFile 测试 |

- **实现要点**：每个 drop 用例追加 index 断言；cancelPendingSave 三 store 各一条（timer 活跃时取消→推进→saveSettings 未调）；resolveTargetZone 中点 ±1 边界；commit-view 拆状态机/分派去重/右键菜单三文件，B10 反向用例改经 openCommitFile；fake timers 与 waitFor 不混用。**font-size/keybindings 测试在 Stage 12 还会再碰（STS-06），本 Stage 只加 cancelPendingSave 用例**。
- **门禁**：同 Stage 06
- **验证项**：
  1. activityBar 全部 drop 用例含第三参数 index 断言（Read 抽查 ≥3 处）
  2. sideBar/fontSize/keybindings 各有 cancelPendingSave 活跃 timer 取消用例
  3. resolveTargetZone 含中点恰好值与中点 -1 两边界用例
  4. commit-view.test.tsx 不复存在或 ≤200 行（拆分完成，新文件落位）
  5. commit 测试中 fake timers 与 waitFor 混用消除（Read 确认）

## Stage 11 L2-hooks-config：竞态与校验链（10 项）

- **项**：HKC-01~10
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| hk-data | HKC-02、HKC-03、HKC-07、HKC-08、HKC-09 | hooks-config-panel/sync 相关测试、`src/__tests__/hooks-config-schema.test.ts`（新建）、hooks-config-entry 测试 |
| hk-ui | HKC-01、HKC-04、HKC-05、HKC-06、HKC-10 | hooks-config-jsonmode/gui/handlerform/eventtree 相关测试 |

- **实现要点**：linter 顺序身份断言（linterCalls[0][0]/[1][0]）；load() 过期请求丢弃（延迟 resolve 旧请求）；非法 JSON 输入断言 configJson 快照不变+保存禁用；validateHooksJson 新建独立测试文件直测边界。
- **门禁**：同 Stage 06
- **验证项**：
  1. JsonMode 用例含 linter 顺序身份断言（非仅 options）
  2. load() 竞态用例：旧请求延迟 resolve 后 configJson 仍为目标层数据
  3. handleJsonChange 非法 JSON 用例存在（快照不变+保存禁用）
  4. `hooks-config-schema.test.ts` 存在且直测 validateHooksJson 边界
  5. GuiMode 删除选中项选中态回退空态有用例

## Stage 12 L2-shortcuts/theme/store：断言真实化（11 项）

- **项**：STS-01~11
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| st-theme | STS-01、STS-02、STS-03、STS-04、STS-05、STS-08、STS-09、STS-11① | `src/__tests__/colors.test.ts`、`theme.test.ts`、`claude-status.test.ts`、`global-commands.test.ts`、`shortcuts.test.ts`、`command-catalog.test.ts`、`inject-script.test.ts` |
| st-store | STS-06、STS-07、STS-10、STS-11② | `src/__tests__/projects.test.ts`、`font-size.test.ts`、`keybindings.test.ts` |

- **实现要点**：colors 五组表驱动全部改真实导出值比对（`expect(GIT_FILE_COLORS[key]).toBe(expected)`）+ 补 EXPLORER_SELECTION_BG 等 token；forceContext 反向注册顺序；afterEach 统一 cancelPendingSave（**本 Stage 在 Stage 10 基础上改，不冲突**）；codify 注释"已知当前行为"。
- **门禁**：同 Stage 06
- **验证项**：
  1. colors.test.ts 中 `expect(expected).toMatch(HEX6_RE)` 式自断言零残留（grep 确认），五组改读真实导出
  2. shortcuts 测试含 forceContext 反向（global 先注册）用例
  3. `theme.test.ts` 含 `kittyKeyboard` 断言
  4. 三 store 测试 afterEach 含 cancelPendingSave 或等价清理
  5. commandFromMeta 参数化覆盖全 9 命令

## Stage 13 L2-ipc/html：盲区收口与参数化（8 项）

- **项**：IHE-01~08
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| ih-ipc | IHE-01、IHE-02、IHE-04、IHE-06、IHE-07①④ | `src/__tests__/ipc-*.test.ts`、`notification.test.ts`（新建）、`src/__tests__/helpers/ipc-contract.ts`（新建）、`e2e-build-config.test.ts`、`csp-config.test.ts`、`src/ipc/CLAUDE.md`（盲区文档化） |
| ih-html | IHE-03、IHE-05、IHE-07②③、IHE-08 | `src/__tests__/html-panel.test.tsx`、`error-boundary.test.tsx` |

- **实现要点**：mockIPC 盲区文档化（文件头 + ipc/CLAUDE.md："只防 wrapper 写错，真实序列化 L4 守卫"）+ listen 回调解包行为契约用例；四契约文件走新建工厂参数化；E2E_ENABLED AST/正则字面量断言；postMessage 四负面用例（origin/source/type/fingerprint）。
- **门禁**：同 Stage 06
- **验证项**：
  1. ipc/CLAUDE.md 含 mockIPC 盲区说明段（Read 确认）
  2. `notification.test.ts` 存在且含拒绝/异常分支用例
  3. e2e-build-config 或 e2e-enabled 测试含 E2E_ENABLED 字面量表达式断言（AST/正则）
  4. html-panel 测试含 origin/source/type/fingerprint 四负面用例 + jsdom 局限标注
  5. 四 IPC 契约文件经共享工厂（新 helper 被四文件 import，grep 确认）
  6. ipc-ping 改调导出的 `ping()`（grep 确认非裸 invoke）

## Stage 14 L2-agent/history：跨模块同源（10 项）

- **项**：NAH-01、NAH-03~11
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| ah-notify | NAH-01、NAH-03、NAH-04 | `src/__tests__/notifications.test.ts`、`claude-history-model.test.ts`、`src/features/notifications/useClaudeNotifications.ts`（classifyEvent 导出，D2） |
| ah-view | NAH-05、NAH-06、NAH-07、NAH-08、NAH-09、NAH-10、NAH-11 | `src/__tests__/agent-status-view.test.tsx`、`claude-history-{restore,hook,view,row,action-dialog}.test.ts(x)` |

- **实现要点**：sessionId null 回退 transcriptPath basename；classifyEvent 导出（D2）表驱动全事件 × notificationType；去重缓存 250 事件截断；AgentStatusView 标题覆盖用真实/受控 history 集成；restoreSession 防重入 + cwd null；scan generation 延迟 resolve。
- **门禁**：同 Stage 06
- **验证项**：
  1. claude-history-model 含 `sessionId: null` 回退用例（断言 basename 去 .jsonl 键命中）
  2. classifyEvent 表驱动覆盖 PermissionRequest/Notification(两型)/Stop/StopFailure/PostToolUseFailure/未识别
  3. 去重缓存截断用例存在（>200 事件 → 截断 100 → 最旧可再弹）
  4. restoreSession 含防重入（连调两次仅一次编排）+ cwd null 抛错两用例
  5. AgentStatusView 标题覆盖用真实/受控 history 数据（非纯 mock useClaudeHistory，Read 确认）

## Stage 15 L3：生产配置覆盖与断言精确化（6 项）

- **项**：E2E-01、E2E-02、E2E-03、E2E-07、E2E-08、E2E-14
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| l3-terminal | 全部 6 项 | `test/terminal/keyboard.test.ts`、`terminal-serialize.test.ts`、`ansi-correctness.test.ts`、`osc.test.ts`、新增 production 配置/OSC/negative 测试文件 |

- **实现要点**：keyboard 文件头+describe 降级标注（D4）；新文件用生产 `terminalOptions` 建 headless Terminal（16 色/kitty/scrollback/drawBoldTextInBrightColors）；OSC 52/133/8 触发 + mock IPC 断言；CUP/reflow/SGR 改 getLine/getCell 精确断言；256 色按 SerializeAddon.ts:259-262 实际优化行为断言；负面 ANSI 用例。
- **门禁**：`npm run test:l3` + `npx tsc --noEmit`
- **验证项**：
  1. keyboard.test.ts 含降级标注（文件头/describe，Read 确认"xterm.js 基础行为回归"字样）
  2. 存在用生产 `terminalOptions` 创建 Terminal 的用例文件（grep `theme` import 命中）
  3. OSC 52/133/8 三 handler 各有触发+断言用例（mock writeText/onTabStateChange/openUrl）
  4. CUP 用例含行列位置断言（getLine/translateToString 或 getCell）
  5. 256 色用例断言基本 SGR 30-37/90-97（grep `\x1b[3Xm`/`\x1b[9Xm` 命中）且误导注释已修正
  6. `npm run test:l3` 全绿

## Stage 16 L4：隔离、真实链路与拆分（9 项）

- **项**：E2E-04、E2E-05、E2E-06、E2E-09、E2E-10、E2E-11、E2E-12、E2E-13、E2E-15
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| e2e-full | 全部 9 项 | `e2e-tests/test.e2e.ts`（拆分）、拆分新 spec 文件、`e2e-tests/helpers.ts`、`e2e-tests/run-wdio.cjs`、`e2e-tests/wdio.conf.ts`、`e2e-tests/fixtures/claude-projects/README.md`（新建） |

- **实现要点**：run-wdio.cjs 备份扩展（~/.claude/settings.json + hooks*/，exit 还原，三启动路径全覆盖）+ renameSync 前 rmSync；新增真实 hook reporter 用例（node 执行脚本 + stdin JSON + SLTERM_PANEL_ID → 信号被消费；非法 JSON exit 0）；test.e2e.ts 按领域拆 spec + withProjectAndTerminal 提取；browser.pause→waitUntil；视觉回归用例（**M2 人工验证点**）；Job Object 杀父进程用例；wdio 重试配置；拖拽改名+恢复标注。**helpers.ts 在 tsc include 外——门禁靠 e2eBuild 构建级验证**。
- **门禁**：`npm run build:e2e` + `npm run wdio`
- **验证项**：
  1. run-wdio.cjs 备份范围含 `~/.claude/settings.json` 且 exit 还原 + hooks*/ 清理（Read 确认，三路径覆盖）
  2. 还原前存在 `rmSync(..., {force:true})` 或等价（grep 命中）
  3. 真实 reporter 用例存在：node 执行脚本路径 + stdin 写 JSON + 断言信号消费 + 非法 JSON exit 0（Read 确认）
  4. test.e2e.ts 拆分完成（单文件 ≤800 行，新 spec 文件被 wdio.conf specs 覆盖）
  5. `browser.pause(` 在 spec 中零残留（grep 零命中，waitUntil 替代）
  6. wdio.conf 含重试配置（retries/specFileRetries）
  7. `npm run e2e` 全量绿；真实 `~/.claude/projects/` 零触碰（M3 人工确认）

## Stage 17 文档同步（4 项）

- **项**：DOC-01~04
- **分工表**：

| label | 负责项 | 触碰文件 |
|-------|--------|---------|
| doc-inventory | DOC-01、DOC-02、DOC-03 | `.claude/test-inventory.md`、`e2e-tests/CLAUDE.md`（定位声明 + L4 测试模式同步段）、`src-tauri/src/pty/CLAUDE.md`（豁免段收编 Stage 02 草稿） |
| doc-modules | DOC-04 | 其余 14 个子路径 CLAUDE.md + 根 `.claude/CLAUDE.md`（如需） |

- **实现要点**：豁免表与 00-summary 5.3 对齐 + 各 Stage 产出收编；定位声明（L3 网格状态/L4 半端到端/jsdom 模拟/helper 契约）；test-inventory 全量校正（stale 清理 + 用例数按各 Stage 实际变更更新 + 豁免表登记）；测试模式章节同步（block_on 命令层模式/EventEmitter trait/ScanRootGuard/拆分后文件清单/git CLI 最低版本）。**e2e-tests/CLAUDE.md 归 doc-inventory 独占，doc-modules 不碰**。
- **门禁**：无代码门禁（grep 断言 + 人工通读）；收尾跑 `npm test` + `cargo test -- --test-threads=1` 确认文档 Stage 零代码副作用
- **验证项**：
  1. test-inventory.md 含豁免表（项目/原因/兜底层级三列）且与 DOC-01 范围一致
  2. test-inventory.md 各域用例数与 `npm test`/`cargo test`/`test:l3` 实际统计一致（静态口径：逐域 grep `it(`/`#[test]` 计数）
  3. "notification 权限声明"等 stale 条目零残留（grep 确认）
  4. e2e-tests/CLAUDE.md 含半端到端/部分端到端定位声明
  5. 子路径 CLAUDE.md 测试文件表与磁盘实际一致（抽查 git/hooks/fs 三模块，Glob 对照）
