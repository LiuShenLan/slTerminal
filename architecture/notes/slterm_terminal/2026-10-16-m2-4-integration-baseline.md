# M2.4 集成层与真机基线执行记录

## Status

已执行（2026-10-16，本机 Win11 Pro build 26300，pwsh 7.6.7 商店版 + powershell 5.1 桌面版）。逐项门禁结果见 Evidence；未执行项附移交登记。

## Context

M2.4 出口 = ConPTY 集成用例真机 `--test-threads=1` 全绿 + 真机门禁清单（Win10 捆绑、flags 三态、claude 全屏滚轮、DA1 身份验证）执行记录。集成层落 `slterm_terminal/tests/pty_integration_tests.rs`（独立 target，与 lib_tests 并存），本 note 记录真机侧无法进代码的判定与实测事实。

## Evidence

集成用例六条（全部真进程、串行）：

- `conpty_spawn_write_read_resize_close_full_chain` — cmd 全链：EventLoop 形态 spawn → 写命令读输出 → resize 80→47 列经 Win32 探针（`AttachConsole` + `GetConsoleScreenBufferInfo`，与生产光标对账同形态）直读 conhost 窗口宽度 → Shutdown 关闭链。顺带钉死回退臂可观测性：`conpty_status` 的 `attempted/bundled/fallback_reason` 自洽断言。
- `default_shell_resolve_chain_osc_marks` — `Options::default()` 走生产 `resolve_shell` 探测链（本机命中商店版 pwsh，AEL alias 白名单身份比对放行）：集成脚本 bringup 发射 OSC 7/9;9 → `Event::CwdReport`、OSC 133;A → `slterm_prompt_active` 置位、包装 prompt 的 OSC 133;D → `Event::CommandDone`。
- `pwsh_integration_script_osc_semantics` — 上游 `run_powershell_integration_case` 范式：prelude（自定义 prompt）+ `shell-integration.ps1` 本体 + checks 合体经 B17 EncodedCommand 真跑；自定义 prompt 不被覆盖（exit 31 闸），stdout 含 133;D/A/B 与 OSC 7。
- `da1_unified_identity_bundled_host` — bundled 子进程（exe 旁文件对侧载 OpenConsole 1.24）：pwsh 发 DA1 查询，stdin 读回应答含统一身份 `\x1b[?64;22c`（priming 残留 + 代答各一份），无 `?61;` 等异身份混入。D02-1 真机钉死。
- `bundled_conpty_resources_return_after_close_and_failed_spawn` — 上游 `windows_pty_lifecycle` 语义：warm 基线 87 句柄，idle×8 → 88、busy（输出背压）×8 → 88、失败 spawn（假 cwd 走 CreateProcessW 失败臂）×8 → 89，均在 +2 预算内归还；45→90s 完成预算为负载裕度。失败 spawn 臂改用合法 shell + 不存在 cwd 的原因：白名单外程序在 `validate_shell_allowlist` 即被拒，测不到 conpty 层失败路径。
- `spawn_lock_serializes_and_capacity_32_enforced` — 32 会话占满 → 第 33 个被拒（消息含上限值）；释放 1 名额后 4 线程 barrier 并发抢，恰 1 成 3 拒；全 drop 后名额归还。走 in-box 臂（容量语义与后端无关，避开 OpenConsole 启动税）。

真机实测发现（探测期，已并入用例注释）：

1. in-box conhost（build 26300）启动发 DSR 查询（`\x1b[6n`）等 CPR——裸 Pty 无应答方，宿主握手永不放行输出；驱动形态必须经 EventLoop（启动窗口内代答 `\x1b[1;1R`）。窗口外 DSR 交 Term 自答，应答经 `Event::PtyWrite` 发出——测试驱动必须回环到 `Msg::Input`（壳语义），否则 conhost 挂住行编辑（实测：输入回显停在首字符）。
2. in-box conhost 收到输出流中的 DA1 查询会自答（实测 31 字节 `\x1b[?61;6;7;21;22;23;24;28;32;42c`）；bundled OpenConsole 握手发 DA1+DSR，应答唯一来源是 priming/代答——DA1 身份判别力只在 bundled 臂，故用例走子进程侧载形态。
3. `$Host.UI.Write` 的输出 writer 在首次调用时定型：集成脚本 bringup 已定型为真实 stdout，checks 内 `[Console]::SetOut` 捕获不到后续 OSC（二分实证）——集成脚本用例的 OSC 断言直接落合体脚本 stdout。
4. conhost 重绘编排是黑盒：powershell 子命令输出的 `47` 曾被 ConPTY 发出（pty_trace 实证 chunk 存在）但被后续 CUP 重绘同行覆盖，grid（含 history）断言落空——resize 真验改走 Win32 探针直读 conhost 几何。
5. 间歇性根因案例：用例 panic 时 EventLoop 线程（JoinHandle drop = detach）持有 Pty 不归还会话名额——`DrivenTerm` 加 Drop 兜底（Shutdown+join），串行尾部容量实测回到 32 全可用。

## Decision

门禁清单逐项结论：

- **flags 三态**：本机两臂实测——in-box 系统臂（build 26300 ≥ 21376）默认矩阵 0x7（0x1|0x2|0x4，CreatePseudoConsole 无 E_INVALIDARG 重试，0x4 生效）；bundled OpenConsole 臂 0x7（侧载恒启用 0x4）。老 Win10 系统臂 0x3 本机不可实测——L1 矩阵守卫（`flags_win10_19041_system_returns_0x3` 等）钉死，移交 Win10 真机复测（发布核验链）。
- **Win10 捆绑**：本机 Win11 不激活提取臂（`should_bundle(26300)=false`）。回退臂存在性真机实证：exe 旁无文件对时 `conpty_status` = attempted+bundled:false+reason（用例 1 断言钉死）；提取链幂等（`ensure_extracted`/`write_if_size_differs`）由 L1 tempdir 用例覆盖。Win10 真机激活（提取→侧载→Defender 首扫税）移交 Win10 实机复测。
- **DA1 身份**：集成用例实证 bundled 臂应答 = `\x1b[?64;22c`（D02-1 统一身份钉死，priming/Term 自答/代答三处同值）。in-box 臂 conhost 自答为宿主行为，不经 core。
- **claude 全屏滚轮**：未执行。M2 全程纯库态，无可驱动 exe；滚轮链路（壳输入翻译 → SGR mouse report → 0x8 passthrough 门禁）须 M3 壳层就位后真机人工复测。本机 claude.exe 已在（`C:\Users\liush\.local\bin\claude.exe`），M3 复测环境就绪。0x8 默认关的守卫在 L1（`conpty_flags_default_matrix_matches_legacy_tristate`），启用 0x8 的人工实测门禁不变。
- **win32_input_matrix 基线语义**：脚本本体归 M3 输入矩阵夹具。基线语义登记——三编码路径：kitty 协议 / Win32 input mode（0x4，键事件 INPUT_RECORD）/ legacy VT；chord（组合键）人工步骤保留。门禁点：0x4 门控链（bundled 恒开、系统臂 build ≥ 21376、`E_INVALIDARG` 重试臂降级自门控）；本机两臂 0x4 均生效（集成用例真跑实证）。

## Rejected alternatives

- 裸 Pty 直读输出做断言（前两任探测草稿形态）：in-box 宿主握手无人应答不放行输出（Evidence 1），且 `UnblockedReader` 空读返回 `Ok(0)` 非 EOF，探测草稿误当 EOF 产生「秒退」假象。
- resize 断言走子进程输出文本（`$Host.UI.RawUI...Width` 进 grid）：conhost 重绘黑盒会同行覆盖（Evidence 4），间歇红不可治理——改 Win32 探针直读。
- DA1 身份在 in-box 臂验证：conhost 自答污染（Evidence 2），无判别力。

## Consequences

- 集成层六个用例串行约 10s；真机负载敏感段均有「负载裕度」注释登记。
- `Event::PtyWrite` 回环是壳的硬职责：M3 壳输入层必须实现该回环，否则任何发 DSR 的交互组件（PSReadLine 等）在窗口外卡死——此语义由集成层 `relay_pty_writes` 先行验证。
- 测试进程内 `AttachConsole` 探针与生产光标对账共用形态；测试串行纪律（`--test-threads=1` + `INTEGRATION_LOCK` 兜底）保证无并发 attach。

## Validation

- `cargo test -p slterm_terminal --test pty_integration_tests -- --test-threads=1`：6 例全绿，三连复跑稳定（9.7s×3）。
- `cargo test -p slterm_terminal --test lib_tests -- --test-threads=1`：394 例全绿，基线不回退。
- `python scripts/check_architecture.py` 零 ERROR；`check_brand_regress.py staged` 零命中；`cargo fmt --all -- --check` 过。

## Supersedes

None。

## Revisit when

- M3 壳层就位后：claude 全屏滚轮真机实测、win32_input_matrix 脚本建基（本 note 登记的门禁点即比对基准）。
- Win10 真机可用时：捆绑提取臂激活验证 + flags 0x3 回退臂实测。
- 若 `Event::PtyWrite` 回环语义变更（如 core 内自回环），集成层 DrivenTerm 的 relay 同步删除。
