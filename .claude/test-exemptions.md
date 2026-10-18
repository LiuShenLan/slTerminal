# 自动化测试豁免与定位

> **本文档职责**：豁免登记处 + 测试定位声明 + 条件跳过说明。用例计数与测试文件导航不属本文档——凡能通过阅读代码直接理解的信息（文件清单、用例数、入口路径）不在此重复（代码自证原则，ADR-0011）。

> **覆盖纪律（硬约束 #11）**：改动的代码可自动化部分必须添加全量自动化测试用例覆盖；不可自动化部分须在本文档既定豁免清单登记（登记处即本文档），注明豁免原因与当前兜底层级，禁止未登记豁免。

## 既定豁免清单（DOC-01）

> 按 D6 分类处理后残余不可自动化项 + 各 Stage 产出收编。对应模块 CLAUDE.md（pty/e2e-tests）保留逐项明细，三列（项目/豁免原因/当前兜底层级）以本表为唯一真值源。

| 项目 | 豁免原因 | 当前兜底层级 | 来源 |
|------|----------|--------------|------|
| `reader_loop` 残余 I/O 编排分支（send 失败/EOF `child.wait()`/DA1+CPR 注入动作/微批续读循环/日志告警/读错误常量） | 依赖 Mutex/RwLock/Channel/系统调用无法纯函数化（决策点已抽 `apply_output_strip`/`strip_da1_queries`/`strip_dsr_queries`/`split_trailing_query_prefix`/`mirror_da1_query`/`mirror_dsr_query`/`should_answer_dsr`/`eof_exit_code`/`micro_batch_tail` + 门控决策 `conhost_input_corrupts_cpr` 补测——ADR-0022 DA1 全量接管后旧 `apply_startup_strip`/`should_inject_da1` 已删除；09-18 DSR 按需代答后 spawn 盲注 CPR 已删除；09-19 修订二窗口外 DSR 改 OS 门控剥离） | `pty_integration_tests`（真实 ConPTY 往返）+ L4 PTY 通信/强杀残留用例；微批上限 64KB 与「读到即续读」语义由 L1 `micro_batch_*` 纯函数用例 + 前端直写阈值 256B（FE-18）双边锁死 | PTY-12 |
| `pty_kill` 超时→监督线程真实阻塞路径 | Win32 阻塞不可注入（`ClosePseudoConsole` 永久阻塞无法在 L1 构造）——兜底 = `plan_cleanup_after_join_timeout` 决策用例 + pty 集成 kill 用例 + Win10 实机人工验证点（杀会话后应用无挂起） | 清理决策由 L1 `plan_cleanup_after_join_timeout` 锁死（reader.rs）+ `join_with_timeout` 用例 + `pty_integration_tests` kill 用例 + Win10 实机人工验证点（杀会话后应用无挂起、3s 内 IPC 返回） | CP-011 |
| `spawn_conpty_child` 纯 Win32 调用部分（AttrList set_pty → CreateProcessW） | Win32 API 组合，参数错误无单测定位价值；可纯化部分已抽 `build_cmdline`/`build_env_block` 补测 | `pty_spawn_custom_conpty` 集成测试 + Windows CI runner | PTY-08 |
| `lib.rs` `run()` | Tauri 运行时胶水，L1 无法直接启动完整应用 | L4 `terminal.e2e.ts` 启动标题等用例 + setup 各副作用的 L1 锁死（`start_signal_watcher_impl` / `reinject_statusline` B15 用例）——setup 本体保持豁免（TQ-COV-02） | SPE-06② |
| `main.rs` fn main 胶水 | 结构性零覆盖：L1 无法启动 tauri 运行时安装 panic hook + run() | L4 `terminal.e2e.ts` 启动链真实执行 + `lib.rs` `run()` 行同构豁免先例 | CP-023 |
| ActivityBar 拖拽 mock 理想化（`getBoundingClientRect` mock + 合成 DragEvent） | jsdom 无法模拟真实 DnD hit-test 与布局矩形 | `activityBar.test.tsx` L2 拖拽全链路 + L4 `sidebar.e2e.ts` 跨区状态机 | SVC-14 |
| `E2E_ENABLED=false` 生产分支 | L2 恒 true，编译期字面量 DCE 结构性缺口 | `e2e-build-config.test.ts` 字面量表达式断言（IHE-04）+ CI 生产 dist grep 守卫 | IHE-04 |
| L3 生产 WebGL renderer / mouse tracking | headless 不跑 GPU；PASSTHROUGH_MODE 滚轮回归无法自动化（行为级测试假阴性） | L4 全屏 TUI 视觉回归（M2 人工确认）+ `compute_conpty_flags` 默认矩阵注入用例（0x7/0x7/0x3 不变）+ 文档红线 | 15-#16 |
| 非默认 flags 矩阵（尤其 0x8）真实滚轮行为（CP-009 人工门禁行） | 自动化不可守卫——0x8 滚轮失效仅真实 claude 场景复现（spawn.rs 实测记录：最小复现实验失败、阻断条件仅 pwsh→claude 进程树 + kitty 协议），矩阵位翻转假阴性 | 默认矩阵零漂移由 L1 `conpty_flags_default_matrix_matches_legacy_tristate` 锁死（0x8 默认置位即红）；任何 0x8 启用/默认翻转兜底 = ADR-0007 门禁第 3 条人工实测（真实 claude 全屏 TUI 滚轮滚动 + Win10 21376 阈值核对），无实测记录禁合入 | CP-009 |
| L4 真实 OS 级按键 | embedded WDIO 无法投递 `browser.keys` 到 WebView2 页面 | 合成事件 + 页面内 dispatch 全链路；terminal.e2e.ts 粘贴用例 = E2E helper 写读往返；Ctrl+Shift+V 消费链路由 L2 keyboard.test.ts + L3 shortcut-dispatch.test.ts（TQ-E-02）覆盖 | 13 P-15 |
| HTML postMessage 真实 WebView2 行为（opaque origin 序列化 / CSP 强制） | jsdom 无法模拟 opaque origin 与 WebView2 CSP；`e.origin === "null"` 为 WHATWG 规范推断 | L4 `html.e2e.ts` Ctrl+W postMessage 往返 + L2 四负面用例（IHE-03） | 13 P-5 |
| `spawn.rs` 容量超限 kill 清理与 `conpty_api.rs` vendor 提取/加载回退的残余 Win32 分支 | 清理段为 I/O + 平台 API 组合，不可纯函数化；上限判定已由 `pty_capacity_*` 用例锁死 | L1 `pty_capacity_*` + `join_with_timeout` 用例 + `pty_integration_tests` 真实 ConPTY 往返 | TQ-COV-03 |
| Rust 行覆盖 89.55%（23309/26029，llvm-cov html Line 列 Totals，含测试代码口径；2026-09-08 采样） | 距 90% 差 0.45pp；残余缺口 = main.rs fn main 结构性零覆盖（本表 fn main 胶水豁免行）+ pty 模块逐项登记（命令层胶水/Job Object Win32 组合/句柄依赖分支/失败 bail/白名单真实 fs 身份判定/vendor 函数指针错误臂，见 pty/CLAUDE.md 豁免表 CP-023 段）+ 编译器生成物计数缺失 | 重点文件已达标或逐条登记豁免（TQ-COV-01/03/06 + git/CLAUDE.md 豁免表）；pty 8 项兜底逐条对应 pty/CLAUDE.md 豁免表 | TQ-COV 收尾 CP-023 |
| plan_balance 真实 provider API 出网（真实账号一轮） | 真实外部 API 依赖（规格 §3 不做 L4） | 请求形态/响应解析/错误映射由本地 HTTP 服务 L1 全覆盖（TcpListener 单请求，不出网）+ 编排/合并/emit/后台线程 L1 全覆盖；真机人工实测（真实账号一轮） | M10.11 |
| win11/win10 真实终端 conda 激活实测（profile 加载链路 + conda 钩子 + prompt 包装链） | 依赖真实 conda/miniforge 环境与交互会话，CI 无此环境 | L1 B17 参数守卫（`pwsh_args_no_noprofile_b17`）+ 双系统 debug build 人工实测 | B17 |
| settings.json corrupted 警示条（L4） | 写坏 settings.json 需沙箱外写文件（E2E 无命令通道），真实损坏无法在 E2E 会话内构造 | L2 覆盖（`settings-panel.test.tsx` loadSettings mock 渲染/关闭）+ 人工实测（手改文件损坏重启） | SC-E2E-02 |
| ~~`cargo test` 门禁（F12 起）~~ **已修复（2026-08-31 当日翻案）** | 初判环境 bug；当日定位真根因：测试二进制无 manifest → SxS 未激活 comctl32 v6 → 系统解析到 v5 → tauri 栈静态导入的 `TaskDialogIndirect`（v6 导出）缺入口 → 0xC0000139 零输出崩溃（与 TQ-COV-06 8-23 预防性登记同因，彼时 `rustc-link-arg-tests` 对默认 lib test 目标不生效故从未真正激活）。修复：Cargo.toml `[lib] test = false` + 显式 `[[test]] lib_tests`（path = src/lib.rs）——使 `cargo:rustc-link-arg-tests`（存量 build.rs）真正作用于 lib 单测目标，manifest 嵌入生效 | 全量 `cargo test -- --test-threads=1` 实测 827 例全绿（711 lib + 116 集成，2026-08-31） | 2026-08-31 实证登记 → 当日翻案 |
| background_tasks spawn/emit 包装层（`spawn_poller` 循环本体与 `background_tasks_set_config` 命令包装层的 emit/重 spawn 分支） | 需 `AppHandle` 与 tauri runtime（async_runtime spawn/事件发射），L1 无法直测；可测部分（`set_config_core` 校验→落盘→内存链、registry 解析钳制）已 L1 全覆盖 | L4 勾选启停端到端（`background-tasks.e2e.ts` C）+ 人工实测（运行中改配置观察 poller 生效） | BE-02 |
| tick 失败静默 E2E 豁免（E2E-03 用例 G） | tick 失败需后端扫描故障注入通道，E2E 沙箱内无可控注入手段 | 调度器 L2 用例（`background-tasks-scheduler.test.ts` 失败处理：tick 失败快照不变/manual 失败置 error）+ 人工观察 | E2E-03 |
| ~~background-tasks.e2e.ts 用例 F 真实 tick 时序豁免（E2E-03）~~ **已修复（2026-09-02 R2a 翻案）** | 根因实证（D1）：E 用例 finally 删除会话 601 后，E 结束→F 开始仅 ~200ms（< E 遗留 scheduler 的 2s tick），删除后重扫未落地，pill 持陈旧值 5（真实 4）→ F 基线取到错误 n=5 → 启用后扫描 = 4 fixture + 602 = 5，断言 n+1=6 永不可达 → 20s 超时。修复：F 取基线前轮询 pill 直至同值持续 ≥3s（覆盖一个 tick 周期，间隔约 1s，上限 6s），以收敛值作基线（n=4），断言目标回归 n+1=5 | 修复后 F 全程真实链路断言（收敛等待 + 落盘 10s / 计数 20s / 勾选态 8s 窗口远超实际 tick 周期）；2026-09-02 单跑与全量 e2e 各 1 次 F 连续通过 | E2E-03 |
| `FileTreeExplorer.handleRename` 同名兜底短路分支（oldPath === newPath） | UI 不可达——同名已在 `FileTree.confirmRename` 拦截（比较 basename），测试无法直传同名进入 onRename；属防御层死代码 | confirmRename 同名短路 L2 用例（explorer-rename-state 用例）+ 后端 src==dst 幂等 L1 用例（`fs_rename_src_equals_dst_*`）双边锁死同层语义 | 修复「重命名取消误删文件」登记 |
| 应用图标视觉质量（1024 母版构图/16px 降采样可辨性/icon.ico 嵌入正确性） | 纯资源替换无可自动化代码逻辑；视觉呈现依赖人眼判定 | 生成脚本 `gen-app-icon.ps1` 后置像素断言（脚本内几何可复算）+ 构建产物人工检查清单（exe 图标属性/任务栏/Alt-Tab 目测，路径 `src-tauri\assets\app-icon\app-icon.png` 与 32px 抽样） | 2026-09-06 图标替换登记 |
| HTML 面板 Ctrl+滚轮缩放的物理滚轮事件与悬停语义（真实 OS 滚轮 delta 设备 / WebView2 物理 wheel / preventDefault 对浏览器缩放的实际效果） | embedded WDIO 无法投递 OS 滚轮；缩放核心行为已由 L2 行为级覆盖（zoomRuntime 桩执行）与 L4 fixture 合成事件全链路覆盖，物理输入路径无法自动化 | L2 `html-zoom-runtime.test.ts`（new Function 桩 doc/win 行为级）+ L4 `html.e2e.ts` Ctrl+滚轮缩放 describe（fixture 合成 WheelEvent → 注入接管 → HUD）+ 下述手工验证清单（build 产物实测：悬停缩放/HUD 续期/重置/切走切回保留/终端 Ctrl+滚轮字号互不干扰/整窗缩放不被触发） | 2026-09-06 htmlviewer 缩放登记 |
| mermaid 图布局与 KaTeX 字形渲染视觉质量（真实 DOM 布局/字体度量） | mermaid v11 渲染与 KaTeX 字体加载依赖真实浏览器布局与字体测量（jsdom 无）；自动断言止于 DOM 存在性 | L2 编排 mock（`markdown-mermaid.test.ts` / `markdown-render-pipeline.test.ts` KaTeX 标记断言）+ L4 `markdown.e2e.ts`（mermaid SVG / KaTeX 类与内联字体 data 前缀）+ 手工视觉清单（图表配色/公式字形/暗色协调） | ADR-0018 预览渲染登记 |
| md 预览物理滚轮与 iframe 重建滚动比例近似误差 | 承接 HTML 缩放豁免语义（同注入接管机制）；滚动恢复为近似语义（文档高度变化后按比例，编辑点恰在视口上方时位置可跳变）——行为级验证上限 | L2 `scrollRuntime` 桩执行（节流上行/下行校验负面）+ L4 `markdown.e2e.ts` 事件属性通道缩放 + 手工清单（编辑预览滚动/重建位置近似） | ADR-0018 预览渲染登记 |
| 大 md 文件渲染性能预算（300ms 防抖下逐键 doc.toString + 渲染主线程占用） | 性能预算需真实 WebView2 计时（jsdom 不具代表性） | 长度守卫登记为 P1 增强（>1MB 关闭 live 刷新）；首次渲染预算 ≤300ms（markdown-it 单趟线性）；真实计时人工清单项 | ADR-0018 预览渲染登记 |
| ~~md 预览 iframe 内 <script> 执行与 html 同态静态化~~ **已销项（2026-09-08，S10-②）** | 存量缺陷（宿主 `</script>` 被无差别转义破坏 → script 吞 EOF）已随预览迁独立 webview 消亡（CP-031：转义函数删除，宿主 `<script>` 于预览域真实执行）——「同态静态化」前提不再成立 | 迁移后宿主 script 执行由 html.e2e 宿主 `<script>` 用例（真实 WebView2）断言；md/html 事件属性通道实证保留（预览域当时无 CSP，SEC-02 起为宿主页 meta 域级 CSP，见 adr.md ADR-0018 落地复核注记） | ADR-0017/0019 登记 |
| CM 字形光栅丢失（GLYPH）的像素断言环境依赖 | 缺陷仅在真实合成/光栅渲染路径与真实截图通道可判（embedded WDIO 无 OS 按键通道、输入走 execCommand；软渲染/非整数 DPI 环境差异会假阴性） | `GLYPH_E2E=1` 像素断言 spec（glyph-repro.e2e.ts：md5-frame/md10/html10/txt10 字形位 PNG 判读全命中）+ L2 repaintGuard 原语/注入契约测试（repaint-guard.test.ts）+ 人工基线（150%/225% 双档 debug build 复现矩阵：5 连输/追加 10/前缀矩阵/选中恢复） | 2026-09-06 GLYPH 取证登记 |
| md/html 编辑 pane Ctrl+滚轮字号的物理滚轮与 split 双通道共存语义 | embedded WDIO 无法投递 OS 滚轮（承接 htmlviewer 缩放豁免 2026-09-06 行）；iframe 内 zoom 与 CM pane 字号双通道的物理合成交互需人手 | L2 字号接线闭环用例（markdown/html-panel.test）+ L4 合成 WheelEvent 字号用例（markdown/html e2e，.cm-scroller 字号 14→15）+ 手工清单（split 态左 pane 字号/右 iframe zoom 互不干扰、整窗缩放不被触发） | 2026-09-06 GLYPH 取证登记 |
| run-wdio.cjs 启动器分支（Node 版本选择/便携预置/键级校验） | 进程编排壳（spawn 外部进程行为不可 jsdom 化），无单测锚点 | L4 全量 e2e（Node 26 直跑）兜底 | CP-003/046 登记 |
| ~~预览窗口 HiDPI/跨屏几何（scale≠1 建窗落点、跨屏拖动 scale 变化跟随）~~ **已销项（2026-09-13，ADR-0021）** | 豁免对象（独立 OS 预览窗建窗/几何换算/跨屏跟随）随载体推翻整体消亡——预览 = 主窗 DOM 内 iframe，HiDPI/跨屏跟随由 WebView2 合成天然承担，无可豁免面 | L1 `compute_physical_rect` 用例族与 L4 主窗移动跟随用例已随被测对象删除；跟随正确性结构性成立（实机验收点保留：拖动/HiDPI 预览跟随目测） | 2026-09 预览几何修复登记 → ADR-0021 销项 |
| dockview 页签真实拖拽分屏手势（pointer down/move/up 序列命中 drop target） | jsdom 无真实 hit-test/布局矩形（同 ActivityBar DnD 豁免 SVC-14 同族）；embedded WDIO 无 OS 级 pointer 投递——真实手势层不可自动化 | L2 `workspace-page-dockview.test.tsx` 页内分屏 describe（真实 dockview `api.addGroup` + `panel.api.moveTo` 等价落点——与真实拖拽同一库内入口：两组同显防复发/切页同隐同显 xterm 不重建/切片两叶持久化/删页杀全组/主组拖空回退链）+ L4 `workspace-split.e2e.ts`（真实二进制 moveTo 等价落点 + 缓冲保留 + 切页显隐 + 切片落盘两叶）+ 实机验收点（分屏手感 + 终端 TUI 保活） | 2026-09-12 ADR-0020 页内分屏登记 |
| Win10 捆绑 conhost（OpenConsole 1.24）握手差异（DA1 多发 + DSR 不发）与键事件输入（CPR 应答 = F3 毒键）的端到端行为 | 本机 Win11 inbox conhost 握手形态与输入解析均不同——差异只在 Win10 路径触发，自动化不可达 | L1 纯函数全分支（`strip_da1_queries`/`strip_dsr_queries`/`split_trailing_query_prefix`/`apply_output_strip` 门控两值/`inject_da1_response` 每次必答回归；`mirror_dsr_query`/`should_answer_dsr` 启动窗口门控/`inject_cpr_response` 按需代答；门控决策 `conhost_input_corrupts_cpr` 边界锁定——防复发锚点：CPR 字节（盲注或 xterm 自答回灌）在 Win10 被解析为 F3 吞首字符）+ L2 就绪闸门用例 + Win10 实机人工验收点（新建终端无蜂鸣无 `[?1;2c` / 新建终端敲键不丢首字符 / 双击恢复无前缀污染不丢首字符 / claude 功能正常——后三点同时覆盖「DSR 剥离不答后发起方缺应答无挂起」） | 2026-09-18 ADR-0022 DA1 接管 + DSR 按需代答登记；2026-09-19 ADR-0022 修订二窗口外 DSR 门控剥离 |
| 分屏落组 L4 用例（点分屏组 + 钮落本组的真实手势） | L2 真实 dockview 集成已等价覆盖（真实 addGroup/moveTo + addTerminalPanel 落组断言）；L4 增量仅是 OS 级点击手势，属「真实手势层不可自动化」同族（页签拖拽分屏豁免行） | L2 `workspace-page-dockview.test.tsx` C4/分屏 describe + `workspace-header-actions.test.tsx` 落触发源组断言 + 实机验收点（分屏两组各自 + 钮落本组） | 2026-09-19 C4 落组统一登记 |
| fs-poll 心跳（10s 周期）与 Rescan 溢出补漏链的 L4 用例 | 心跳周期 10s 过长（E2E 用例等待预算爆炸）且事件丢失/队列溢出在 E2E 无可控注入通道（notify 内部溢出无法从沙箱外构造确定性复现） | L1 `notify/mod.rs` emit 链用例（心跳周期 emit/pause 停发/resume 补发/Rescan 语义不变）+ L2 前端复核用例（editor-fs-poll / diff-panel fs-poll / large-file-viewer Rescan+fs-poll）+ L4 主链已覆盖（editor.e2e 原子替换写 / html.e2e render 态改写——Create 放行与 viewless 回写）+ 人工验收（构建产物观察 10s 心跳兜底生效） | 2026-09-25 外部修改同步修复登记 |
| 回收站真实落桶（`SHFileOperationW` 实际移入系统回收站） | 真实调用会污染开发机/CI 用户回收站，且无法从测试进程断言系统落桶位置；不可重复 | 平台封装用例（`file_operations_tests`）锁死 `FOF_ALLOWUNDO`/静默位、双 NUL 编码、错误码与用户取消映射、缺失路径拒绝；真实落桶由构建产物人工验收（删文件后回收站可见）；M8.5 起菜单入口经注入通道断言路径到达本单点 | 07 篇 M8.4 登记 |
| gutter 行标与编辑器内部滚动的像素级对齐 | 行标同屏位置依赖组件库内部布局（`visible_row_range`/`scroll_offset`/行号留白），虚拟窗口只能断言元素存在与矩形非零，不能断言与文本行像素级重合 | L1 `gutter` 用例锁死状态→行标映射（升序/去重/六值）与 revision 推进；L4 `gutter_click_opens_a_read_only_dual_pane_diff` 断言行标绘制、点击打开 diff；实际对齐由构建产物人工目测（滚动/长文件） | 07 篇 M8.5 登记 |
| 双栏 diff 的像素级行对齐与同步滚动视觉 | 两栏独立 `InputState` 的内部滚动由组件库布局决定，hunk 导航的同步滚动只保证滚动偏移写入，像素级行对位不可在虚拟窗口断言 | L1 `diff_tab` 用例锁死 hunk 计算/只读合同；L4 `diff_hunk_navigation_moves_the_active_hunk` 断言 hunk 集与活动下标；实际双栏对位由构建产物人工目测 | 07 篇 M8.5 登记 |
| 整窗关闭的真实 OS 手势（Alt+F4/系统菜单关闭） | embedded 虚拟窗口无 OS 级关窗注入通道；`simulate_close` 走真实 `on_window_should_close` 回调（含脏守卫与确认框），但 OS 手势到回调的一段不可自动化 | L4 `closing_the_window_with_dirty_files_confirms_first`（拦截/取消/保存落盘）+ `shell_window_mounts_root_and_draws`；真实 Alt+F4 由构建产物人工验收 | 07 篇 M8.5 登记 |
| AI assistant 真实模型 HTTP 请求（`ai_assistant::send_model_request` 四协议族出网） | 真实外部 API + 凭据依赖，测试进程不发出境请求；协议字段/响应解析可纯函数化的部分已覆盖 | L1 `should_suggest`/危险词表/`redact_secrets`/`parse_fix`/`request_target` 与 `fallback_api_key` 用例 + L4 `ai_fix_bar_pastes_only_and_drops_stale_responses`（Pending/Ready/粘贴/过期 seq）+ 构建产物人工实测（配置 provider 后一条失败命令得到建议条） | 08 篇 M9.5 登记 |
| terminal_math 覆盖层上屏后的像素级字形观感（公式落点/抗锯齿/裁剪边） | 位图与布局可在 L1 逐像素/几何断言，但最终屏幕合成由 GPUI 文本与图像管线在真实 DPI 下完成，虚拟窗口不能代表字形观感 | L1 `math_overlay_tests`（扫描/投影/掩码/fit 计划）+ `math_render_tests`（位图合成预算）+ L4 `math_overlay_paint_leaves_grid_and_copy_source_untouched`（真实绘制一帧后网格/复制不变）+ 构建产物人工目测（`$x^2$`、`\\[ \\]` 块、宽字符） | 08 篇 M9.5 登记 |
| 真实系统 toast 投递与 AUMID 注册（WinRT `Show` 跨进程 RPC + `HKCU\Software\Classes\AppUserModelId` 写） | 真实投递会污染开发机通知中心、依赖桌面会话与通知开关，且系统抑制/AUMID 生效不可从测试进程断言；测试构建显式短路真实投递 | 漏斗策略 L1 全量（前台门控/闪烁先行/全局与 pane 节流/失败 episode/160 截断/时长 XML 映射）+ `notify-test` 诊断命令真机逐段打印（注册/时长/投递，构建产物实跑 exit 0）+ 构建产物人工 smoke（系统通知中心可见 slTerminal banner 且点击回窗） | 09 篇 M10.1 登记 |
| 真实任务栏进度投递（`ITaskbarList3::SetProgressState`/`SetProgressValue` 的 COM 调用与视觉呈现） | 真实调用依赖 Explorer 任务栏与桌面会话，条状视觉不可从测试进程断言；虚拟窗口无原生 HWND，且任务栏是纯装饰面，不可重复 | 映射/状态机 L1 全量（五态/未知码/钳制/标志位 + 焦点路由 + TurnDone/SessionEnd/pane 关闭/OSC 0/133;D 清除路径 + 跨 pane 隔离；测试构建 `apply` 只记录调用序列不触 COM）+ 构建产物人工 smoke（程序发 OSC 9;4 时任务栏出现对应条，agent 退出后消失） | 09 篇 M10.2 登记 |
| 真实托盘交互（`Shell_NotifyIconW` 图标挂载/右键 `TrackPopupMenu` 点击/左键点击/通知区图标呈现与橙点视觉） | 依赖 Explorer 通知区、真实鼠标点击与桌面会话；托盘是进程级外部状态不可重复；测试构建显式不启动托盘线程 | 菜单构建/命令 id 映射/1 Hz 去抖/attention 聚合/命令动作映射/橙点像素 L1 全量（含原生 HMENU 结构断言，不开线程）+ 托盘回窗与退出事件 L4（焦点切 tab、退出请求）+ 构建产物人工 smoke（图标出现、右键 agent 直达与「显示」回窗、「退出」删图标且退出）；品牌图标资产已内嵌（C24，M12/R1） | 09 篇 M10.3 登记 |
| 真实窗口还原/前置（`IsIconic`/`SW_RESTORE`/`SetForegroundWindow` 对真实 HWND 的调用与视觉前台化） | 依赖真实桌面会话与窗口管理器（前台锁定/焦点窃取策略），虚拟窗口无原生 HWND 且不可重复；窗口更新回调内不能注入原生窗口 | 还原顺序 L1（`minimized_window_restores_before_foregrounding`）+ 单一实现点门禁 L1（Win32 焦点原语仅 `windowing/focus.rs`）+ 四路回窗 L4（ATTACH/托盘/toast/动作按钮经 facade 落到窗口/tab/pane）+ 构建产物人工 smoke（最小化后托盘/通知点击还原并前置） | 09 篇 M10.4 登记 |
| 真实 setup.exe 覆盖安装与 helper 提权链（PowerShell helper 启动/安装目录写/等待应用退出/Inno 静默安装/版本回读/失败回滚重启） | 依赖安装器管理的真实安装目录、真实 setup.exe 与进程重启；L1 执行会覆盖开发机安装并打断测试进程，不可重复；helper 是独立 PowerShell 子进程，其 Win32 文件锁/进程身份/提权语义无法在测试进程内对偶 | 文件机 L1 全量（plan→ready→commit/cancel 双路径、孤儿/未 commit 事务启动按 cancel 收尾、commit 才授予安装权威、Drop 终止 helper、守卫锁、restore ticket 三态与 3 次尝试上限）+ 假 release 服务器检查→下载→handoff 全链演练 + `update-test-source` 隔离彩排通道 + 构建产物人工验收（真实 setup.exe 安装、版本回读、工作区恢复、失败回滚重启） | 09 篇 M10.5 登记 |
| 真实用户 Startup 文件夹 `.lnk` 写入/删除（`SHGetKnownFolderPath(FOLDERID_Startup)` 定位后落盘） | 真实写入会污染开发机开机启动项且不可重复；测试构建显式短路真实 Startup 路径（`set_launch_at_login` 直接返回 Ok） | COM 写读 round-trip L1 全量（tempdir 假 Startup 目录，`IShellLinkW`/`IPersistFile` 真实接口逐字段保真：目标/参数/工作目录/图标）+ 关闭幂等删除 L1 + Startup 已知文件夹路径解析 L1（品牌文件名 P-8）+ 静默两条件真值表 L1 + 构建产物人工 smoke（开关开机启动后 Startup 文件夹出现 `slTerminal.lnk`，关闭后删除；开机登录仅托盘可见） | 09 篇 M10.6 登记 |
| 真实 DWM 材质渲染与壁纸 GPU 合成（`SetWindowCompositionAttribute`/`DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)`/`DwmEnableBlurBehindWindow` 对真实 HWND 的调用，Mica/MicaAlt/Aero/Acrylic 视觉呈现，壁纸纹理上屏与透明度叠加） | 依赖真实桌面合成器、系统壁纸与 build（Mica 需 22621+）、真实显卡合成，虚拟窗口无原生 HWND；材质是否被系统实际接纳与观感不可从测试进程断言 | 纯函数 gate L1（`background_appearance_gate_matrix_follows_windows_builds`：build×五材质→平台外观；`material_plan_keeps_the_two_native_channels_mutually_exclusive`：WCA/DWMSBT 互斥、Acrylic 回退、旧系统不写不存在属性）+ 壁纸有界加载 L1（64/128 MiB/2048 边/8 MiB 纹理预算、文件戳缓存、代际失配丢弃、BGRA、预览 512 边）+ 透明度纪律 L1（`shell_opacity_only_dims_surfaces_and_is_idempotent`：壳 token alpha 生效、文字前景恒 1、重算幂等）+ 静默缝 L1（`visual_effects_never_show_or_activate_windows`）+ 构建产物人工 smoke（五档材质切换目测、透明度滑块生效、设置壁纸后默认格透出且文字对比度不变、静默启动无窗口闪现） | 09 篇 M10.7 登记 |

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
- `hooks/signal.rs::process_symlink_signal_deletes_without_read`、`hooks/watcher.rs::collect_excludes_symlink_files`、`notify/mod.rs` symlink 用例、`agent_history/claude/ops.rs` symlink 用例（BE-17/D5 豁免先例）

以下用例依赖真实桌面会话与预构建产物，`SLTERM_E2E_EXE` 未设置时打印 SKIP 并计「通过」：
- `runtime_conformance::runtime_api_conformance_and_second_instance_handover`（04 篇 M7.5 / 00-roadmap M7 出口）：真实 GUI 进程 + 双进程移交。兜底 = L1 `server_tests` 移交/stale/Drop 用例 + UI `runtime_control_tests` 执行器用例 + 本机真机实跑（2026-10-18 通过）。串行执行：`SLTERM_E2E_EXE=target/debug/slterm.exe cargo test -p slterm --test runtime_conformance -- --test-threads=1`。

以下用例依赖真实 Windows 凭据管理器（无凭据库的服务账户/CI 会失败），未设 `SLTERM_TEST_CREDENTIAL_STORE=1` 时打印 SKIP 并计「通过」：
- `secrets.rs::credential_three_operations_round_trip_through_windows_manager`（M10.9 / B61）：三原语往返 + 覆盖写 + 幂等删除。兜底 = 内存后端契约用例 + `SecretBytes` 类型边界/日志边界/借出白名单扫描 + 源码门禁（凭据后端仅门面可达）。

本地开发机（已开开发者模式）为真实覆盖来源；CI runner 未开权限时上述分支覆盖记为「不确定」。
