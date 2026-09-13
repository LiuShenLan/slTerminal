# 自动化测试豁免与定位

> **本文档职责**：豁免登记处 + 测试定位声明 + 条件跳过说明。用例计数与测试文件导航不属本文档——凡能通过阅读代码直接理解的信息（文件清单、用例数、入口路径）不在此重复（代码自证原则，ADR-0011）。

> **覆盖纪律（硬约束 #11）**：改动的代码可自动化部分必须添加全量自动化测试用例覆盖；不可自动化部分须在本文档既定豁免清单登记（登记处即本文档），注明豁免原因与当前兜底层级，禁止未登记豁免。

## 既定豁免清单（DOC-01）

> 按 D6 分类处理后残余不可自动化项 + 各 Stage 产出收编。对应模块 CLAUDE.md（pty/e2e-tests）保留逐项明细，三列（项目/豁免原因/当前兜底层级）以本表为唯一真值源。

| 项目 | 豁免原因 | 当前兜底层级 | 来源 |
|------|----------|--------------|------|
| `reader_loop` 残余 I/O 编排分支（send 失败/EOF `child.wait()`/DA1 注入动作/微批续读循环/日志告警/读错误常量） | 依赖 Mutex/RwLock/Channel/系统调用无法纯函数化（决策点已抽 `apply_startup_strip`/`should_inject_da1`/`eof_exit_code`/`micro_batch_tail` 补测） | `pty_integration_tests`（真实 ConPTY 往返 7 条）+ L4 PTY 通信/强杀残留用例；微批上限 64KB 与「读到即续读」语义由 L1 `micro_batch_*` 6 条纯函数用例 + 前端直写阈值 256B（FE-18）双边锁死 | PTY-12 |
| `pty_kill` 超时→监督线程真实阻塞路径 | Win32 阻塞不可注入（`ClosePseudoConsole` 永久阻塞无法在 L1 构造）——兜底 = `plan_cleanup_after_join_timeout` 决策用例 + pty 集成 kill 用例 + Win10 实机人工验证点（杀会话后应用无挂起） | 清理决策由 L1 `plan_cleanup_after_join_timeout` 2 例锁死（reader.rs）+ `join_with_timeout` 3 例 + `pty_integration_tests` kill 用例 + Win10 实机人工验证点（杀会话后应用无挂起、3s 内 IPC 返回） | CP-011 |
| `spawn_conpty_child` 纯 Win32 调用部分（AttrList set_pty → CreateProcessW） | Win32 API 组合，参数错误无单测定位价值；可纯化部分已抽 `build_cmdline`/`build_env_block` 补测 | `pty_spawn_custom_conpty` 集成测试 + Windows CI runner | PTY-08 |
| `lib.rs` `run()` | Tauri 运行时胶水，L1 无法直接启动完整应用 | L4 `terminal.e2e.ts` 启动标题等用例 + setup 两副作用各自的 L1 锁死（`start_signal_watcher_impl` 4 例 / `reinject_statusline` B15 用例）——setup 本体保持豁免（TQ-COV-02） | SPE-06② |
| `main.rs` fn main 3 行胶水 | 结构性零覆盖：L1 无法启动 tauri 运行时安装 panic hook + run() | L4 `terminal.e2e.ts` 启动链真实执行 + `lib.rs` `run()` 行同构豁免先例 | CP-023 |
| ActivityBar 拖拽 mock 理想化（`getBoundingClientRect` mock + 合成 DragEvent） | jsdom 无法模拟真实 DnD hit-test 与布局矩形 | `activityBar.test.tsx` L2 拖拽全链路 + L4 `sidebar.e2e.ts` 跨区状态机 | SVC-14 |
| `E2E_ENABLED=false` 生产分支 | L2 恒 true，编译期字面量 DCE 结构性缺口 | `e2e-build-config.test.ts` 字面量表达式断言（IHE-04）+ CI 生产 dist grep 守卫 | IHE-04 |
| L3 生产 WebGL renderer / mouse tracking | headless 不跑 GPU；PASSTHROUGH_MODE 滚轮回归无法自动化（行为级测试假阴性） | L4 全屏 TUI 视觉回归（M2 人工确认）+ `compute_conpty_flags` 默认矩阵注入用例（0x7/0x7/0x3 不变）+ 文档红线 | 15-#16 |
| 非默认 flags 矩阵（尤其 0x8）真实滚轮行为（CP-009 人工门禁行） | 自动化不可守卫——0x8 滚轮失效仅真实 claude 场景复现（spawn.rs 实测记录：最小复现实验失败、阻断条件仅 pwsh→claude 进程树 + kitty 协议），矩阵位翻转假阴性 | 默认矩阵零漂移由 L1 `conpty_flags_default_matrix_matches_legacy_tristate` 锁死（0x8 默认置位即红）；任何 0x8 启用/默认翻转兜底 = ADR-0007 门禁第 3 条人工实测（真实 claude 全屏 TUI 滚轮滚动 + Win10 21376 阈值核对），无实测记录禁合入 | CP-009 |
| L4 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys` 到 WebView2 页面 | 合成事件 + 页面内 dispatch 全链路；terminal.e2e.ts 粘贴用例 = E2E helper 写读往返；Ctrl+Shift+V 消费链路由 L2 keyboard.test.ts + L3 shortcut-dispatch.test.ts（TQ-E-02）覆盖 | 13 P-15 |
| HTML postMessage 真实 WebView2 行为（opaque origin 序列化 / CSP 强制） | jsdom 无法模拟 opaque origin 与 WebView2 CSP；`e.origin === "null"` 为 WHATWG 规范推断 | L4 `html.e2e.ts` Ctrl+W postMessage 往返 + L2 四负面用例（IHE-03） | 13 P-5 |
| `spawn.rs` 容量超限 kill 清理与 `conpty_api.rs` vendor 提取/加载回退的残余 Win32 分支 | 清理段为 I/O + 平台 API 组合，不可纯函数化；上限判定已由 `pty_capacity_*` 用例锁死 | L1 `pty_capacity_*` 3 例 + `join_with_timeout` 3 例 + `pty_integration_tests` 真实 ConPTY 往返 | TQ-COV-03 |
| Rust 行覆盖 89.55%（23309/26029，llvm-cov html Line 列 Totals，含测试代码口径） | 距 90% 差 0.45pp；残余缺口 = main.rs fn main 结构性零覆盖（本表 fn main 3 行胶水豁免行）+ pty 模块 8 项逐条登记（命令层胶水/Job Object Win32 组合/句柄依赖分支/失败 bail/白名单真实 fs 身份判定/vendor 函数指针错误臂，见 pty/CLAUDE.md 豁免表 CP-023 段）+ 编译器生成物计数缺失 | 重点文件已达标或逐条登记豁免（TQ-COV-01/03/06 + git/CLAUDE.md 豁免表）；pty 8 项兜底逐条对应 pty/CLAUDE.md 豁免表 | TQ-COV 收尾 CP-023 |
| plan_balance 真实 HTTP 查询（ureq fetch）与 tokio 轮询任务本体（含动态间隔内存读取 POLL_INTERVAL_SEC 与 set_interval 落盘/内存一致链，F11 扩注） | 真实外部 API 依赖 + Tauri 运行时（规格 §3 不做 L4） | 解析与状态机 L1 全覆盖（罐装 JSON/参数化编排 + 间隔内存默认值/四维 set_interval 直调用例）+ L2 UI 四场景 + L4 频率页真实后端落盘（settings.e2e.ts ④⑤）+ 人工实测（真实账号一轮） | F10/F11 |
| win11/win10 真实终端 conda 激活实测（profile 加载链路 + conda 钩子 + prompt 包装链） | 依赖真实 conda/miniforge 环境与交互会话，CI 无此环境 | L1 B17 参数守卫（`pwsh_args_no_noprofile_b17`）+ 双系统 debug build 人工实测 | B17 |
| settings.json corrupted 警示条（L4） | 写坏 settings.json 需沙箱外写文件（E2E 无命令通道），真实损坏无法在 E2E 会话内构造 | L2 覆盖（`settings-panel.test.tsx` loadSettings mock 渲染/关闭）+ 人工实测（手改文件损坏重启） | SC-E2E-02 |
| ~~`cargo test` 门禁（F12 起）~~ **已修复（2026-08-31 当日翻案）** | 初判环境 bug；当日定位真根因：测试二进制无 manifest → SxS 未激活 comctl32 v6 → 系统解析到 v5 → tauri 栈静态导入的 `TaskDialogIndirect`（v6 导出）缺入口 → 0xC0000139 零输出崩溃（与 TQ-COV-06 8-23 预防性登记同因，彼时 `rustc-link-arg-tests` 对默认 lib test 目标不生效故从未真正激活）。修复：Cargo.toml `[lib] test = false` + 显式 `[[test]] lib_tests`（path = src/lib.rs）——使 `cargo:rustc-link-arg-tests`（存量 build.rs）真正作用于 lib 单测目标，manifest 嵌入生效 | 全量 `cargo test -- --test-threads=1` 实测 827 例全绿（711 lib + 116 集成，2026-08-31） | 2026-08-31 实证登记 → 当日翻案 |
| background_tasks spawn/emit 包装层（`spawn_poller` 循环本体与 `background_tasks_set_config` 命令包装层的 emit/重 spawn 分支） | 需 `AppHandle` 与 tauri runtime（async_runtime spawn/事件发射），L1 无法直测；可测部分（`set_config_core` 校验→落盘→内存链、registry 解析钳制）已 L1 全覆盖 | L4 勾选启停端到端（`background-tasks.e2e.ts` C）+ 人工实测（运行中改配置观察 poller 生效） | BE-02 |
| tick 失败静默 E2E 豁免（E2E-03 用例 G） | tick 失败需后端扫描故障注入通道，E2E 沙箱内无可控注入手段 | 调度器 L2 用例（`background-tasks-scheduler.test.ts` 失败处理：tick 失败快照不变/manual 失败置 error）+ 人工观察 | E2E-03 |
| ~~background-tasks.e2e.ts 用例 F 真实 tick 时序豁免（E2E-03）~~ **已修复（2026-09-02 R2a 翻案）** | 根因实证（D1）：E 用例 finally 删除会话 601 后，E 结束→F 开始仅 ~200ms（< E 遗留 scheduler 的 2s tick），删除后重扫未落地，pill 持陈旧值 5（真实 4）→ F 基线取到错误 n=5 → 启用后扫描 = 4 fixture + 602 = 5，断言 n+1=6 永不可达 → 20s 超时。修复：F 取基线前轮询 pill 直至同值持续 ≥3s（覆盖一个 tick 周期，间隔约 1s，上限 6s），以收敛值作基线（n=4），断言目标回归 n+1=5 | 修复后 F 全程真实链路断言（收敛等待 + 落盘 10s / 计数 20s / 勾选态 8s 窗口远超实际 tick 周期）；2026-09-02 单跑与全量 e2e 各 1 次 F 连续通过 | E2E-03 |
| `ExplorerPanel.handleRename` 同名兜底短路分支（oldPath === newPath） | UI 不可达——同名已在 `FileTree.confirmRename` 拦截（比较 basename），测试无法直传同名进入 onRename；属防御层死代码 | confirmRename 同名短路 L2 用例（explorer-rename-state 3 例）+ 后端 src==dst 幂等 L1 用例（`fs_rename_src_equals_dst_*` 2 例）双边锁死同层语义 | 修复「重命名取消误删文件」登记 |
| 应用图标视觉质量（1024 母版构图/16px 降采样可辨性/icon.ico 嵌入正确性） | 纯资源替换无可自动化代码逻辑；视觉呈现依赖人眼判定 | 生成脚本 `gen-app-icon.ps1` 后置像素断言（脚本内几何可复算）+ 构建产物人工检查清单（exe 图标属性/任务栏/Alt-Tab 目测，路径 `src-tauri\assets\app-icon\app-icon.png` 与 32px 抽样） | 2026-09-06 图标替换登记 |
| HTML 面板 Ctrl+滚轮缩放的物理滚轮事件与悬停语义（真实 OS 滚轮 delta 设备 / WebView2 物理 wheel / preventDefault 对浏览器缩放的实际效果） | embedded WDIO 无法投递 OS 滚轮；缩放核心行为已由 L2 行为级覆盖（zoomRuntime 桩执行 15 例）与 L4 fixture 合成事件全链路覆盖，物理输入路径无法自动化 | L2 `html-zoom-runtime.test.ts`（new Function 桩 doc/win 行为级）+ L4 `html.e2e.ts` Ctrl+滚轮缩放 describe（fixture 合成 WheelEvent → 注入接管 → HUD）+ 下述手工验证清单（build 产物实测：悬停缩放/HUD 续期/重置/切走切回保留/终端 Ctrl+滚轮字号互不干扰/整窗缩放不被触发） | 2026-09-06 htmlviewer 缩放登记 |
| mermaid 图布局与 KaTeX 字形渲染视觉质量（真实 DOM 布局/字体度量） | mermaid v11 渲染与 KaTeX 字体加载依赖真实浏览器布局与字体测量（jsdom 无）；自动断言止于 DOM 存在性 | L2 编排 mock（`markdown-mermaid.test.ts` / `markdown-render-pipeline.test.ts` KaTeX 标记断言）+ L4 `markdown.e2e.ts`（mermaid SVG / KaTeX 类与内联字体 data 前缀）+ 手工视觉清单（图表配色/公式字形/暗色协调） | ADR-0018 预览渲染登记 |
| md 预览物理滚轮与 iframe 重建滚动比例近似误差 | 承接 HTML 缩放豁免语义（同注入接管机制）；滚动恢复为近似语义（文档高度变化后按比例，编辑点恰在视口上方时位置可跳变）——行为级验证上限 | L2 `scrollRuntime` 桩执行（节流上行/下行校验负面）+ L4 `markdown.e2e.ts` 事件属性通道缩放 + 手工清单（编辑预览滚动/重建位置近似） | ADR-0018 预览渲染登记 |
| 大 md 文件渲染性能预算（300ms 防抖下逐键 doc.toString + 渲染主线程占用） | 性能预算需真实 WebView2 计时（jsdom 不具代表性） | 长度守卫登记为 P1 增强（>1MB 关闭 live 刷新）；首次渲染预算 ≤300ms（markdown-it 单趟线性）；真实计时人工清单项 | ADR-0018 预览渲染登记 |
| ~~md 预览 iframe 内 <script> 执行与 html 同态静态化~~ **已销项（2026-09-08，S10-②）** | 存量缺陷（宿主 `</script>` 被无差别转义破坏 → script 吞 EOF）已随预览迁独立 webview 消亡（CP-031：转义函数删除，宿主 `<script>` 于预览域真实执行）——「同态静态化」前提不再成立 | 迁移后宿主 script 执行由 html.e2e 宿主 `<script>` 用例（真实 WebView2）断言；md/html 事件属性通道实证保留（预览域当时无 CSP，SEC-02 起为宿主页 meta 域级 CSP，见 adr.md ADR-0018 落地复核注记） | ADR-0017/0019 登记 |
| CM 字形光栅丢失（GLYPH）的像素断言环境依赖 | 缺陷仅在真实合成/光栅渲染路径与真实截图通道可判（embedded WDIO 无 OS 按键通道、输入走 execCommand；软渲染/非整数 DPI 环境差异会假阴性） | `GLYPH_E2E=1` 像素断言 spec（glyph-repro.e2e.ts：md5-frame/md10/html10/txt10 字形位 PNG 判读全命中）+ L2 repaintGuard 原语/注入契约测试（repaint-guard.test.ts）+ 人工基线（150%/225% 双档 debug build 复现矩阵：5 连输/追加 10/前缀矩阵/选中恢复） | 2026-09-06 GLYPH 取证登记 |
| md/html 编辑 pane Ctrl+滚轮字号的物理滚轮与 split 双通道共存语义 | embedded WDIO 无法投递 OS 滚轮（承接 htmlviewer 缩放豁免 2026-09-06 行）；iframe 内 zoom 与 CM pane 字号双通道的物理合成交互需人手 | L2 字号接线闭环用例（markdown/html-panel.test）+ L4 合成 WheelEvent 字号用例（markdown/html e2e，.cm-scroller 字号 14→15）+ 手工清单（split 态左 pane 字号/右 iframe zoom 互不干扰、整窗缩放不被触发） | 2026-09-06 GLYPH 取证登记 |
| run-wdio.cjs 启动器分支（Node 版本选择/便携预置/键级校验） | 进程编排壳（spawn 外部进程行为不可 jsdom 化），无单测锚点 | L4 全量 e2e（Node 26 直跑）兜底 | CP-003/046 登记 |
| ~~预览窗口 HiDPI/跨屏几何（scale≠1 建窗落点、跨屏拖动 scale 变化跟随）~~ **已销项（2026-09-13，ADR-0021）** | 豁免对象（独立 OS 预览窗建窗/几何换算/跨屏跟随）随载体推翻整体消亡——预览 = 主窗 DOM 内 iframe，HiDPI/跨屏跟随由 WebView2 合成天然承担，无可豁免面 | L1 `compute_physical_rect` 用例族与 L4 主窗移动跟随用例已随被测对象删除；跟随正确性结构性成立（实机验收点保留：拖动/HiDPI 预览跟随目测） | 2026-09 预览几何修复登记 → ADR-0021 销项 |
| dockview 页签真实拖拽分屏手势（pointer down/move/up 序列命中 drop target） | jsdom 无真实 hit-test/布局矩形（同 ActivityBar DnD 豁免 SVC-14 同族）；embedded WDIO 无 OS 级 pointer 投递——真实手势层不可自动化 | L2 `workspace-page-dockview.test.tsx` 页内分屏 describe（真实 dockview `api.addGroup` + `panel.api.moveTo` 等价落点——与真实拖拽同一库内入口：两组同显防复发/切页同隐同显 xterm 不重建/切片两叶持久化/删页杀全组/主组拖空回退链）+ L4 `workspace-split.e2e.ts`（真实二进制 moveTo 等价落点 + 缓冲保留 + 切页显隐 + 切片落盘两叶）+ 实机验收点（分屏手感 + 终端 TUI 保活） | 2026-09-12 ADR-0020 页内分屏登记 |

> 原豁免表中 `FileWatcher::start`/`notify_watch` 与 `claude_history` 命令包装两项已按 D6 从豁免重分类为补测，不再列入豁免表。  
> **SEC-17 豁免已撤销（TQ-COV-05 翻案）**：`tracing::warn!(target: "audit")` 已由 `tracing-test` 断言锁死，豁免行删除。

## 定位声明（DOC-02）

| # | 层级 | 定位声明 | 来源 |
|---|------|----------|------|
| ① | L3 | **网格状态正确性，非渲染正确性**——headless 不跑 WebGL/GPU/onContextLoss，生产 `terminalOptions`/OSC handler 的验证限于 headless 网格可观察语义 | E2E-04 |
| ② | L4 | **半端到端/部分端到端**——键盘（合成 keydown 非 OS 按键）、拖拽（store helper 非真实 DnD）、恢复编排（断言到 `pty.write` 注入，不含真实进入会话）三类用例唯一不真实处是事件来源/前置动作 | E2E-11 |
| ③ | L2 | **jsdom 模拟**——postMessage origin/source 校验在 jsdom 无法代表真实 WebView2，负面用例守护 JS 侧逻辑，真实行为由 L4 验收 | IHE-03 |
| ④ | L2 | **term.input 间接验证**——`term.input(...)` → onData 断言是 xterm.js 内部转换的间接验证，键盘链路由 L2 `attachCustomKeyEventHandler` 委托 + L4 半端到端兜底 | 06 #14 |
| ⑤ | L2 | **E2E helper 行为契约**——验证 `__slterm_e2e_createProject` 等 helper 的契约，非真实 App 初始化逻辑 | 13 P-14 |
| ⑥ | L2 | **浅层组件定位**——`editor.test.tsx` mock `useCodeMirror`，定位为组件集成契约测试，真实编辑器行为由 `use-code-mirror.test.ts` 等覆盖 | 07 G1/G2 |

## 条件跳过用例（有效覆盖依赖 runner 环境）

以下用例依赖 Windows 应用执行别名或 symlink 创建权限，环境不满足时跳过但仍计「通过」：
- `pty/shell.rs::allowlist_accepts_real_alias_when_present`
- `hooks/signal.rs::process_symlink_signal_deletes_without_read`、`hooks/watcher.rs::collect_excludes_symlink_files`、`notify/mod.rs` symlink 两用例、`agent_history/claude/ops.rs` symlink 三用例（BE-17/D5 豁免先例）

本地开发机（已开开发者模式）为真实覆盖来源；CI runner 未开权限时上述分支覆盖记为「不确定」。

## L4 运行机制

### 用户目录隔离机制（ADR-0016 假 home，替代 FIX-TE-04 + E2E-05 备份/还原）

`run-wdio.cjs` 建临时假屋（`<tmp>/slterm-e2e-home-<pid>`）并注入 `USERPROFILE`——e2e 全部用户目录写入（hooks 注入/statusLine 桥接/假 env/信号文件）落假屋，真实 `~/.claude` 与 `~/.slterminal` 零接触；exit 时对真实屋做哨兵键级比对（~/.claude/settings.json 的 hooks/statusLine/env 存在性+值快照比对；~/.slterminal/statusline-backup.json 维持文件 sha256、hooks/ 维持整树快照——任何泄漏独立报红 exitCode=1）。旧备份/还原机制（`.e2e-bak`）已退役。E2E 不触碰真实 `~/.claude/projects/`（`SLTERM_CLAUDE_PROJECTS_DIR` 指向临时副本）。详见 `e2e-tests/CLAUDE.md`。

### E2E 键盘输入限制（半端到端，TE-17）

embedded WDIO 无法投递 OS 级按键；所有键盘用例改用页面内 dispatch 合成事件 → ShortcutRegistry → 命令 handler → 真实 IPC。唯一不真实处是事件来源。
