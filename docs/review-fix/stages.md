# slTerminal review 修复——Stage 划分

- **输入**：`docs/review-fix/checklist.md`（93 项：SEC 14 / BE 21 / FE 35 / TE 13 / DOC 10）
- **划分原则**：Stage 内并行 agent 文件零重叠；共享文件项并入同 agent 或 pipeline 串行；Stage 间串行（每 Stage commit 后下一 Stage 可再碰同一文件）；文档同步固定最后 Stage
- **命令数中间态**（verify 计数断言按此推导，不照抄终态）：当前 33 → S03 后 **32**（删 pty_reattach）→ S05 后 **33**（+notify_stop_watch）→ S13 后 **34**（+pty_kill_all）
- **决策约束**：checklist D1~D11 全链路遵守，各 Stage 不再复述

## 总表

| Stage | 名称 | 改动项 | Agents | 人工验证点 |
|-------|------|--------|--------|-----------|
| S01 | 依赖安全排雷 | TE-01/02/05/06/12 | 3 并行 | E2E 全量实跑（WDIO 升级后） |
| S02 | 后端安全 P0 | SEC-01/02、BE-01 | 2 并行 | — |
| S03 | reattach 删除 + 命令白名单 | SEC-03/07 | 2 串行 | 白名单后全功能实测 + E2E |
| S04 | 命令异步化 + 沙箱加固 | BE-04、SEC-14、FE-04 | 2 并行 | — |
| S05 | watcher 排除与生命周期 | BE-02/10/11、SEC-08 | 2 并行 | 大仓库（含 node_modules）事件量实测 |
| S06 | PTY reader 批处理 | BE-05/06/12、FE-18 | 2 并行 | **claude 高输出流畅度 + 滚轮 + kill 实测** |
| S07 | fs_read_file 分块 | BE-03 | 2 并行 | 10MB 大文件打开实测 |
| S08 | 错误处理体系 | FE-02/03/05~10、BE-13/15 | 4 并行 | — |
| S09 | corrupted 契约 + 持久化加固 | BE-14/16、FE-11、SEC-11 | 2 并行 | 损坏 settings/projects 实测 toast |
| S10 | DTO 契约修正 | FE-12/13/14、BE-18 | 2 并行 | — |
| S11 | 前端安全 | SEC-04/06/10 | 1 | — |
| S12 | 前端性能 + 页数上限 | FE-01/15/16/17/19/20/21/29/32/33/34、BE-19 | 4 并行 | 多终端焦点切换 WebGL 实测（FE-34） |
| S13 | 稳定性与生命周期 | FE-22~28、BE-07/08/09 | 3 并行 | 面板崩溃隔离实测；关窗 PTY 全灭实测 |
| S14 | 死代码清理 | FE-35、BE-17/20 | 3 并行 | — |
| S15 | major 升级 | TE-07/08/09/10 | 4 串行 | **dockview 布局拖拽/分屏/恢复实测 + E2E** |
| S16 | 版本策略 + CI 门禁 | TE-03/04/11/13 | 2 并行 | CI 实跑三项新门禁 |
| S17 | hooks 写入校验 | SEC-05/12/13 | 2 并行 | user 层写入确认弹窗实测 |
| S18 | FileTree 虚拟化 | FE-30 | 1 | **大目录（万级节点）滚动/展开实测** |
| S19 | 文档同步 | DOC-01~10 | 3 并行 | — |

---

## S01 依赖安全排雷

**改动项**：TE-01、TE-02、TE-05、TE-06、TE-12
**排最前理由**：依赖变更影响全量测试基线，后续所有 Stage 的测试绿建立在升级后的依赖上

**跨边界契约（写死）**：WDIO 前端 `@wdio/tauri-plugin`/`@wdio/tauri-service` 与 Rust 侧 `tauri-plugin-wdio-webdriver` 版本必须**两侧同为 1.3.0**——A/B 两 agent 分别改 npm/cargo，任一侧偏离即 E2E 挂

| label | 负责项 | 文件 |
|-------|--------|------|
| npm-deps | TE-01（npm 侧）、TE-02 | `package.json`、`package-lock.json` |
| cargo-deps | TE-01（Rust 侧）、TE-05、TE-06 | `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`（TE-05 API 适配时含 `src-tauri/src/git/` 下调用点文件） |
| knip-cfg | TE-12 | `knip.json`（新建） |

**实现要点**：
- npm-deps：`@wdio/*` 9.28.0→9.30.1（含 mocha-framework，serialize-javascript 须升 ≥7.0.5 消 GHSA-5c6j-r48x-rmvq）；`@wdio/tauri-plugin`/`@wdio/tauri-service`→1.3.0；`expect-webdriverio` 随升级；`json-schema`（消费点 `src/panels/hooksConfig/JsonMode.tsx`）、`@lezer/highlight`（消费点 `src/theme/overrides.ts`）按当前 lock 中传递版本显式声明入 dependencies；升完 `npm audit` 复验 WDIO 链路 high=0
- cargo-deps：`git2` 0.20→0.21（vendored-libgit2 保持，status/diff 调用点 API 适配）；`tauri` 2.11.3→2.11.5、`tauri-build` 及各 plugin 按 wanted 升 patch；`tauri-plugin-wdio-webdriver`→1.3.0
- knip-cfg：`knip.json` entry 含 `e2e-tests/**/*.ts`，`npx knip --production` 零误报（@wdio/* 不再误报未使用）
- 版本策略统一（TE-11）在 S16 才做——本 Stage 新声明依赖暂按现状风格写，S16 统一收敛

**验证断言要点**：lock 中 serialize-javascript ≥7.0.5；WDIO 三处版本 = 1.3.0（npm×2 + cargo×1）；git2 = 0.21.x；`json-schema`/`@lezer/highlight` 在 package.json dependencies；knip.json 存在且 `npx knip --production` 无 @wdio 误报
**门禁命令**：全量五条 + `npx vite build` + `npm run build:e2e`（依赖变更构建级验证）
**commit**：`chore(deps): WDIO 9.30.1/git2 0.21/Tauri patch 升级消 serialize-javascript RCE + 补隐式依赖声明 + knip 配置（TE-01/02/05/06/12）`
**人工验证点**：`npm run e2e` 全量实跑通过（WDIO 大版本升级后驱动链路验证，自动化门禁只到 build:e2e）

---

## S02 后端安全 P0

**改动项**：SEC-01、SEC-02、BE-01

| label | 负责项 | 文件 |
|-------|--------|------|
| pty-security | SEC-01、BE-01 | `src-tauri/src/pty/shell.rs`、`src-tauri/src/pty/spawn.rs` |
| signal-security | SEC-02 | `src-tauri/src/hooks/signal.rs`、`src-tauri/src/hooks/watcher.rs` |

**实现要点**：
- SEC-01：用户传入 shell 含路径分隔符时——canonicalize 用户路径，与 `which_full_path(文件名)` 解析结果比对，一致才放行；纯文件名输入维持现状。L1 测试：伪造路径拒绝 / 合法绝对路径放行
- BE-01：`const MAX_PTY_SESSIONS: usize = 32`，`pty_spawn` 在 `sessions.len() >= 32` 时返回 `AppError::Validation`；检查须在 SPAWN_LOCK 内（防并发超发）。L1 测试
- SEC-02：`process_signal_file_with` 与 `collect_signal_files` 改 `fs::symlink_metadata` + `is_symlink()`，symlink 文件仅删除不读取。L1 测试：Windows symlink 需管理员/developer mode——创建失败则 skip 并注释注明
- **禁区**：`compute_conpty_flags` 固定 0x7，本 Stage 任何 agent 不得触碰 ConPTY flags

**验证断言要点**：shell.rs 存在 canonicalize 比对逻辑（语义式，不限变量名）；spawn.rs 存在 32 上限常量与超限 Err；signal.rs/watcher.rs 不再出现 `fs::metadata(`（改 `symlink_metadata`）；新增 L1 用例全绿
**commit**：`fix(security): shell 白名单真实路径校验 + PTY 会话上限 32 + 信号文件 symlink 过滤（SEC-01/02、BE-01）`

---

## S03 reattach 删除 + 命令白名单

**改动项**：SEC-03、SEC-07
**串行理由**：白名单（B）依赖 reattach 删除（A）后的最终 32 条命令清单；两者同碰命令注册面，pipeline 串行

**跨边界契约（写死）**：S03 完成后命令清单终态 32 条——
`ping, get_windows_build_number, set_project_root, pty_spawn, pty_write, pty_resize, pty_kill, fs_read_file, fs_write_file, fs_read_dir, fs_create_dir, fs_delete, fs_rename, save_settings, load_settings, save_projects, load_projects, git_status, git_diff, git_file_at_head, git_rollback, git_unstage, notify_watch, agent_hooks_inject, agent_hooks_uninstall, agent_hooks_injection_status, agent_hooks_restore_statusline, agent_hooks_config_read, agent_hooks_config_write, agent_history_scan, agent_history_delete, agent_history_read_title`
权限命名：`allow-` + 命令名 snake_case 原样（一手证据：tauri-build 2.6.3 `acl.rs:100` 注释「format of allow-$command where $command is the command name in snake_case」）；执行期以 `cargo build` 产物 `src-tauri/gen/schemas/*.json` 实际生成的权限名为准核对

| label | 负责项 | 文件 |
|-------|--------|------|
| A: remove-reattach | SEC-03 | `src-tauri/src/pty/spawn.rs`、`src-tauri/src/lib.rs`、`src/ipc/pty.ts`、`src/__tests__/ipc-contract.test.ts`、`src-tauri/tests/pty_integration_tests.rs` |
| B: command-whitelist | SEC-07 | `src-tauri/build.rs`、`src-tauri/capabilities/default.json` |

**实现要点**：
- A：删 `pty_reattach` 后端命令 + `generate_handler!` 注册 + 前端 wrapper + 关联测试；**ring buffer/channel 替换机制保留**（reader 内部仍用，E1 机制不动）
- B：`build.rs` 配置 `tauri_build::Attributes::new().app_manifest(AppManifest::new().commands(&[...32 条]))`；`capabilities/default.json` 为 32 条命令逐条 `allow-<cmd>`；删 `_p0-07-note` 旧注释
- 前端 `types/` 中 reattach 相关 DTO（若有）一并清理，grep `reattach` 零残留（注释除外）

**验证断言要点**：`grep -ri reattach src/ src-tauri/src/` 零命中（ring buffer 注释除外）；`lib.rs` `generate_handler!` 恰 32 条；capabilities 恰 32 条 `allow-` 自定义命令权限 + 既有插件权限；build.rs 含 `AppManifest::new().commands(`；`cargo build` 后 `gen/schemas` 权限名与 capabilities 一致
**commit**：`fix(security): 删除无消费 pty_reattach + 32 条命令白名单化（SEC-03/07）`
**人工验证点**：白名单启用后实机构建（`npx tauri build --debug --no-bundle`）全功能实测——终端/文件/git/hooks/历史全路径可用；`npm run e2e` 通过（权限缺失会表现为 invoke reject）

---

## S04 命令异步化 + 沙箱加固

**改动项**：BE-04、SEC-14、FE-04

**跨边界契约（写死）**：`set_project_root`/`notify_watch` 改 `async fn`，前端 invoke 调用签名不变（Promise 语义已具备）；`set_project_root` 失败语义 = 返回 Err **且清空旧 root**（SEC-14，防沙箱误放行旧路径）；前端失败仍完成切换（D7/DBG-9 不动）+ toast「项目根路径设置失败，文件操作可能被拒绝」

| label | 负责项 | 文件 |
|-------|--------|------|
| backend-async | BE-04、SEC-14 | `src-tauri/src/state.rs`、`src-tauri/src/notify/mod.rs` |
| frontend-toast | FE-04 | `src/App.tsx`、`src/stores/projects.ts`、`src/workspace/Workspace.tsx`、`src/__tests__/workspace-switch-order.test.tsx` |

**实现要点**：
- backend-async：两命令改 `async fn` + `tokio::task::spawn_blocking` 包裹 canonicalize / FileWatcher::start；set_project_root 失败路径清空 `project_root`（写锁置 None）。L1 测试：失败清空旧 root
- frontend-toast：三处调用点（App.tsx:84-87、projects.ts:154、Workspace.tsx:216）失败时 `toast.show("warning", ...)` 仍继续切换；workspace-switch-order 14 用例补 toast 断言（mock toast，断言失败路径 toast 被调且切换仍发生）

**验证断言要点**：`set_project_root`/`notify_watch` 为 `async fn` 且阻塞段在 `spawn_blocking` 内；失败分支存在 `project_root` 置 None（语义式）；三处前端调用点失败路径均 toast（grep `toast.show` 于三文件）；14 用例全绿且含 toast 断言
**commit**：`fix(backend): set_project_root/notify_watch 异步化 + 失败清空旧 root + 前端失败 toast 降级（BE-04、SEC-14、FE-04）`

---

## S05 watcher 排除与生命周期

**改动项**：BE-02、SEC-08、BE-10、BE-11

**跨边界契约（写死）**：新命令 `notify_stop_watch(path: String) -> Result<(), AppError>`；前端 wrapper `stopWatch(path: string): Promise<void>`（`src/ipc/notify.ts`）；排除目录常量 `WATCH_EXCLUDE_DIRS = ["node_modules", "target", ".venv", "venv", "dist", ".git", "__pycache__"]`（D8 定稿，仅事件侧过滤——notify 不支持目录级排除，watcher 仍注册全树）；本 Stage 后命令数 **33**

| label | 负责项 | 文件 |
|-------|--------|------|
| watcher-backend | BE-02、SEC-08、BE-10、BE-11 | `src-tauri/src/notify/mod.rs`、`src-tauri/src/notify/pool.rs`、`src-tauri/src/lib.rs`、`src-tauri/capabilities/default.json` |
| watcher-frontend | BE-10（前端侧） | `src/ipc/notify.ts`、`src/features/explorer/ExplorerPanel.tsx` + 对应 L2 测试 |

**实现要点**：
- BE-02：事件循环中过滤任一路径分量 ∈ WATCH_EXCLUDE_DIRS 的事件；`need_rescan` 分支不受影响。L1 测试
- SEC-08：事件路径 `symlink_metadata` 检查，命中 symlink 不 emit（need_rescan 只发 watch root，不受影响）。L1 测试（symlink 创建失败 skip 同 SEC-02 约定）
- BE-10：pool 加 `remove(path)`；`notify_stop_watch` 命令 + `lib.rs` 注册 + capabilities `allow-notify_stop_watch`；前端项目移除/切换时调用 stopWatch
- BE-11：`WATCHER_POOL_CAPACITY = 8` 常量 + 注释理由（5→8，覆盖多项目快速切换；pause/resume 既定机制保留，文档登记在 S19）

**验证断言要点**：WATCH_EXCLUDE_DIRS 七元素与契约一致（语义式核对集合）；事件循环存在排除过滤与 symlink 检查；`notify_stop_watch` 在 lib.rs 注册且 capabilities 有对应 allow（命令数 33）；pool.rs 容量常量 = 8；ExplorerPanel 项目移除/切换路径调用 stopWatch
**commit**：`fix(notify): watcher 事件侧排除大目录 + symlink 过滤 + notify_stop_watch 生命周期命令 + 池容量 8（BE-02/10/11、SEC-08）`
**人工验证点**：大仓库（含 node_modules/target）打开文件浏览器，确认 CPU/事件量显著下降且文件变更仍正常刷新

---

## S06 PTY reader 批处理

**改动项**：BE-05、BE-06、BE-12、FE-18
**高风险标注**：终端核心数据路径，流畅度回归只能实测兜底

**跨边界契约（写死）**：微批策略 = 「读到即续读」非定时器——read 成功后非阻塞 try_read 续读，累积至 **64KB** 或无可读数据再一次 `Channel::send` + 一次 `ring_buffer_append`（BE-12 随动：append 调用点仅批量一处）；前端 `usePtyOutput` 直接写阈值 64B→**256B**（2ms 空闲/16ms 强制不变）

| label | 负责项 | 文件 |
|-------|--------|------|
| reader-backend | BE-05、BE-06、BE-12 | `src-tauri/src/pty/reader.rs`、`src-tauri/src/pty/spawn.rs`、`src-tauri/src/state.rs` |
| output-frontend | FE-18 | `src/panels/terminal/usePtyOutput.ts`、`src/panels/terminal/useXterm.ts` + 对应 L2 测试（`src/__tests__/`）、L3 测试（`test/terminal/`，若阈值被断言） |

**实现要点**：
- BE-05：reader_loop try_read 续读微批；**DOC-01 豁免项 1（reader_loop I/O 编排）变动——S19 须同步豁免表**，本 Stage 在 reader.rs 注释更新编排说明
- BE-06：`pty_kill` 检查 `child.kill()` 返回值（失败 `tracing::warn!` 继续）；join 改轮询 `is_finished` 3s 超时（超时记 warn 放弃 join，线程随 Drop 兜底）
- BE-12：append 批量后调用点仅一处；不引入无锁结构
- FE-18：`usePtyOutput` 暴露 `dispose()`（清 idle/max 双定时器 + 清 buffer）；`useXterm` cleanup 调用 `dispose()`；阈值 64B→256B
- **禁区**：不得触碰 ConPTY flags / WebGL 检测逻辑

**验证断言要点**：reader.rs 存在 try_read 续读循环与 64KB 上限（语义式）；reader.rs 中 `ring_buffer_append` 调用点 = 1；spawn.rs kill 返回值被检查且 join 有 3s 超时（语义式）；usePtyOutput 导出 dispose 且 useXterm cleanup 调用；阈值常量 = 256；L2/L3 全绿
**门禁命令**：全量五条 + `npm run test:l3`
**commit**：`perf(pty): reader 微批处理降 IPC 频次 + kill 可靠性加固 + 前端输出 dispose 与阈值上调（BE-05/06/12、FE-18）`
**人工验证点**：**claude 高输出场景（长流式回复/大日志）流畅度实测 + 鼠标滚轮实测 + kill 终端实测**——批处理引入任何可见延迟/吞输入即回滚本 Stage

---

## S07 fs_read_file 分块

**改动项**：BE-03
**独立 Stage 豁免理由**（偏离「每 Stage 3-15 项」）：跨前后端强耦合单一任务，IPC 契约破坏性变更，独立 Stage 便于回滚定位

**跨边界契约（写死）**：
- 后端：`fs_read_file(path: String, onChunk: Channel<FsReadChunk>) -> Result<(), AppError>`；`FsReadChunk { data: String, done: bool }`；块 **256KB**；先 metadata 校验大小 ≤10MB（超限 Err，行为同现状）；发送序列 = 若干 `{data, done:false}` + 终态 `{data:"", done:true}`
- 前端：`readFile(path: string): Promise<string>` **签名不变**——`src/ipc/fs.ts` 内部 Channel 监听拼接，resolve 完整字符串；消费方（DiffPanel.tsx / HtmlPanel.tsx / useCodeMirror.ts）**零适配**

| label | 负责项 | 文件 |
|-------|--------|------|
| fs-backend | BE-03（后端） | `src-tauri/src/fs/mod.rs` |
| fs-frontend | BE-03（前端） | `src/ipc/fs.ts`、`src/__tests__/ipc-contract.test.ts` + 对应 L2 测试 |

**实现要点**：后端分块 read + send（注意 UTF-8 边界——按字节读 256KB 后须落到 char boundary 再转 String，或读 `Vec<u8>` 按 `str::from_utf8` 安全切分；实现取「读字节块，回退到边界」）；前端 ipc-contract 测试更新为新 payload 形态（mock Channel）
**验证断言要点**：`fs_read_file` 签名含 `Channel<FsReadChunk>`；存在 256KB 块常量与 UTF-8 边界处理（语义式）；`readFile` 前端签名不变（`Promise<string>`）；DiffPanel/HtmlPanel/useCodeMirror 三消费方文件本 Stage 零改动（git diff 验证）；L1/L2 全绿
**commit**：`perf(fs): fs_read_file 改 Channel 分块推送削大文件内存/IPC 峰值（BE-03，D3）`
**人工验证点**：打开接近 10MB 大文件，编辑器正常渲染无卡顿；超 10MB 拒绝行为不变

---

## S08 错误处理体系

**改动项**：FE-02、FE-03、FE-05、FE-06、FE-07、FE-08、FE-09、FE-10、BE-13、BE-15

**跨边界契约（写死）**：
- `src/ipc/appError.ts` 导出：`parseAppError(err: unknown): { variant: string; message: string } | null`（按 camelCase 变体名解析后端 AppError 序列化形态）+ `getErrorMessage(err: unknown): string`（提取用户可读消息，兜底盘 `String(err)`）；`src/lib/index.ts` re-export
- 后端新增 `AppError::ConfigParse` 变体（BE-15）——变体总数 **10+1=11**，FE-02 测试覆盖全 11 变体
- toast 契约：`toast.show(type: "success" | "warning" | "error", message: string)`

| label | 负责项 | 文件 |
|-------|--------|------|
| error-infra | FE-02 | `src/ipc/appError.ts`（新建）、`src/lib/index.ts`、`src/__tests__/`（新建测试） |
| error-backend | BE-13、BE-15 | `src-tauri/src/error.rs`、`src-tauri/src/fs/mod.rs`、`src-tauri/src/settings.rs`、`src-tauri/src/projects.rs` |
| error-consumers-app | FE-03、FE-05、FE-06、FE-09 | `src/main.tsx`、`src/App.tsx`、`src/stores/fontSize.ts`、`src/stores/keybindings.ts`、`src/stores/sideBar.ts` + 对应 L2 测试 |
| error-consumers-panels | FE-07、FE-08、FE-10 | `src/features/explorer/useFileTree.ts`、`src/features/explorer/ExplorerPanel.tsx`、`src/panels/terminal/useXterm.ts`、`src/panels/editor/useCodeMirror.ts`、`src/panels/diff/DiffPanel.tsx` + 对应 L2 测试 |

**实现要点**：
- error-infra 先行语义（并行可行——契约已写死，消费方按契约 import，不依赖实现完成）
- BE-13：`From<std::io::Error>` 本身不动；fs/settings/projects 命令内 `map_err` 调用点注入路径上下文，错误消息含路径
- BE-15：`ConfigParse` 变体用于配置 JSON 损坏场景；用户可见消息改业务语义（「保存设置失败」），技术细节进 tracing
- FE-03：main.tsx:38 + App.tsx:44-69 启动链各 catch 至少 `console.warn` 带模块名（降级兜底逻辑不动）
- FE-05：关闭序列 kill 失败收集 → 全部完成后统一一条 `console.error` 汇总（含失败数）
- FE-06：requestUserAttention catch 内 `console.warn`
- FE-07：useFileTree 增加按路径 error 状态；ExplorerPanel 错误占位（消息 + 重试按钮）
- FE-08：非关键路径（resize/kill/openUrl）保留 console.error；关键路径（spawn 失败、write 连续失败 ≥3 次）toast；统一经 getErrorMessage
- FE-09：三 store 保存失败统一 toast「设置保存失败，重启后将丢失」
- FE-10：DiffPanel 失败加「内容可能过时」提示条；useCodeMirror 外部重载失败 console.warn + 状态条提示

**验证断言要点**：appError.ts 导出两函数且 lib re-export；L2 测试覆盖 11 变体；error.rs 含 ConfigParse；fs/settings/projects map_err 含路径注入（语义式）；`grep -n '\.catch(() => {})' src/` 于 FE-05/06/08 涉及行零残留；三 store 保存失败 toast；L2 全绿
**commit**：`feat(error): 统一 AppError 解析器 + ConfigParse 变体 + 启动链/终端/编辑器错误可感知化（FE-02/03/05~10、BE-13/15）`

---

## S09 corrupted 契约 + 持久化加固

**改动项**：BE-14、BE-16、FE-11、SEC-11

**跨边界契约（写死）**：
- `load_settings() -> LoadResult<Value>`：`{ data: Value | null, corrupted: boolean }`（无文件 = `data:null, corrupted:false`；解析失败回退 = `data:默认, corrupted:true`；**.bak 命中也算 corrupted=true**）
- `load_projects() -> LoadResult<String>`：`{ data: String, corrupted: boolean }`（data 为 JSON 字符串，形态同现状）
- 前端 wrapper：`loadSettings(): Promise<{ data: Record<string, unknown> | null; corrupted: boolean }>`；`loadProjects(): Promise<{ data: string; corrupted: boolean }>`
- 大小上限 **1MB**；settings 顶层键白名单 = `fontSize | keybindings | sideBar | colorScheme`（SEC-11）

| label | 负责项 | 文件 |
|-------|--------|------|
| persist-backend | BE-14、BE-16、SEC-11 | `src-tauri/src/settings.rs`、`src-tauri/src/projects.rs`、`src-tauri/src/app_dir.rs`（新建）、`src-tauri/src/lib.rs` |
| persist-frontend | FE-11 | `src/ipc/settings.ts`、`src/ipc/projects.ts`、`src/stores/projects.ts`、`src/stores/fontSize.ts`、`src/stores/keybindings.ts`、`src/stores/sideBar.ts`、`src/main.tsx` + 对应 L2 测试 |

**实现要点**：
- BE-16：`app_data_dir`/`resolve_app_data_dir` 上提 `src-tauri/src/app_dir.rs`，settings/projects 均从新模块导入（消约束 #2 违反）；lib.rs 加 `mod app_dir;`
- BE-14：`.bak` 兜底逻辑保留；corrupted 判定含 bak 命中
- SEC-11：save 侧大小上限 1MB + settings 顶层键白名单校验；projects 结构校验（须为 JSON 对象）
- FE-11：四 store loadFromDisk 消费 corrupted → `toast.show("warning", "配置已损坏，已回退默认值")`；main.tsx 早期 loadSettings 调用适配新返回结构（corrupted 时 console.warn——启动早期 toast 未挂载）

**验证断言要点**：Rust 返回结构序列化形态 = `{ data, corrupted }`（serde 字段名）；app_dir.rs 存在且 settings/projects 从其导入（grep `use crate::app_dir`）；`mod app_dir;` 在 lib.rs；四 store corrupted→toast；main.tsx 适配；L1/L2 全绿
**commit**：`feat(persist): load 返回 corrupted 标志 + app_dir 模块上提 + 保存大小/schema 校验（BE-14/16、FE-11、SEC-11，D11）`
**人工验证点**：手改 settings.json 为非法 JSON → 启动 toast 提示且默认值兜底；bak 恢复路径同样 toast

---

## S10 DTO 契约修正

**改动项**：FE-12、FE-13、FE-14、BE-18

**跨边界契约（写死）**：
- `DirEntry.size: number | null; modified: number | null`（FE-12，运行时实为 null 而非 undefined）
- `FsEventPayload.detail: string`（FE-13，Rust 必填）
- `HooksLayer = "user" | "project" | "local"`（FE-14 收窄；types/CLAUDE.md 登记在 S19）
- `pty_spawn` wrapper 前置校验 `cols/rows ∈ 1..=32767`，越界抛错不 invoke
- 后端 `Layer` 枚举 `User/Project/Local`（serde rename_all snake_case），`parse_layer` 返回枚举

| label | 负责项 | 文件 |
|-------|--------|------|
| dto-frontend | FE-12、FE-13、FE-14 | `src/types/fs.ts`、`src/types/notify.ts`、`src/types/hooksConfig.ts`、`src/types/pty.ts`、`src/types/agentHistory.ts`、`src/types/agent.ts`、`src/ipc/pty.ts` + DirEntry 消费方适配（先 grep 确定，含 `src/__tests__/` 测试工厂） + 对应 L2 测试 |
| dto-backend | BE-18 | `src-tauri/src/hooks/claude/config.rs` |

**实现要点**：
- dto-frontend：u64 字段（agentHistory/agent/pty 类型中）加注释「Rust u64 → JS number，安全整数范围（< 2^53）约定」；DirEntry null 适配须 grep `\.size`/`\.modified` 全部消费点（explorer 排序/显示、测试工厂），含测试文件
- BE-18：hooks 子树结构体（serde 反序列化校验形态）——与 S17 SEC-05 语义校验共用此结构（本 Stage 建结构，S17 加校验规则）

**验证断言要点**：DirEntry 两字段为 `number | null`；detail 无 `?`；HooksLayer 为字面量联合；pty.ts wrapper 含 1..32767 校验；config.rs 存在 Layer 枚举且 parse_layer 返回之；`npx tsc --noEmit` 全绿（null 适配的正确性由 tsc 兜底）
**commit**：`fix(types): DirEntry/detail DTO 与 Rust 真实形态对齐 + HooksLayer 收窄 + pty 参数前置校验 + 后端 Layer 枚举（FE-12/13/14、BE-18）`

---

## S11 前端安全

**改动项**：SEC-04、SEC-06、SEC-10
**单 agent 理由**：三处小改动分散三文件，一 agent 顺序完成

| label | 负责项 | 文件 |
|-------|--------|------|
| frontend-security | SEC-04、SEC-06、SEC-10 | `src/panels/html/HtmlPanel.tsx`、`src/main.tsx` + 新建守卫测试（`src/__tests__/`） |

**实现要点**：
- SEC-04：面板挂载生成随机 nonce（`crypto.getRandomValues`），经注入脚本传入 iframe；父窗口 message 监听校验 nonce，不符静默丢弃；补 L2 测试（伪造消息不触发快捷键）
- SEC-10：main.tsx:28-31 fail-safe 页 `innerHTML` 改 `createElement` + `textContent` + `style` 赋值
- SEC-06（D6 守卫测试）：grep 级 L2 测试——断言 `readText`（clipboard-manager）仅出现于 `src/ipc/clipboard.ts`、`src/panels/terminal/keyboard.ts` 及测试文件；消费点登记（ipc/CLAUDE.md）在 S19

**验证断言要点**：HtmlPanel 存在 nonce 生成/注入/校验三段（语义式）；main.tsx 无 `innerHTML`（fail-safe 块）；守卫测试文件存在且断言上述路径集合；L2 全绿
**commit**：`fix(security): HTML 预览 postMessage nonce 校验 + fail-safe 页去 innerHTML + 剪贴板权限消费点守卫测试（SEC-04/06/10）`

---

## S12 前端性能 + 页数上限

**改动项**：FE-01、FE-15、FE-16、FE-17、FE-19、FE-20、FE-21、FE-29、FE-32、FE-33、FE-34、BE-19

**跨边界契约（写死）**：
- `MAX_PAGES = 20`（stores/projects.ts，超限 addPage 拒绝 + toast「页面数已达上限」）
- `agent_history_scan(cli_id: String, force: Option<bool>) -> ...`（BE-19 缓存 + 显式刷新；前端 wrapper `scanAgentHistory(cliId: string, force?: boolean)`）
- 缓存键 = `(目录 mtime, 文件数)`，命中复用；`force=true` 绕过

| label | 负责项 | 文件 |
|-------|--------|------|
| perf-trees | FE-15、FE-16、FE-19（前端侧） | `src/features/explorer/useFileTree.ts`、`src/features/navTree/useNavTree.ts`、`src/ipc/agentHistory.ts` + 对应 L2 测试 |
| perf-terminal | FE-17、FE-29、FE-32、FE-34 | `src/panels/terminal/TerminalPanel.tsx`、`src/panels/terminal/useXterm.ts` + 对应 L2/L3 测试 |
| perf-workspace | FE-01、FE-20、FE-21、FE-33 | `src/stores/projects.ts`、`src/workspace/Workspace.tsx`、`src/App.tsx`、`src/features/sideViews/SideBarArea.tsx` + 对应 L2 测试 |
| history-backend | BE-19 | `src-tauri/src/agent_history/claude/scan.rs`、`src-tauri/src/agent_history/mod.rs` |

**实现要点**：
- FE-01：保持多 Dockview 实例（D1/H6 不动）；仅加上限；workspace/CLAUDE.md + ADR 豁免登记在 S19
- FE-15：`file-saved` 300ms debounce；已知路径变更只刷新受影响子树（定位最近展开祖先）
- FE-16：历史归属建 `Map<projectId, sessions>` 索引消 O(N×M)；tree 派生 useMemo 依赖精确化
- FE-19：配合后端缓存——挂载一次扫描，展开历史节点不重复 scan（显式刷新/恢复完成时 force）
- FE-17：订阅回调内 `e.panelId === 自身 panelId` 过滤
- FE-29：TerminalPanel.tsx:206 加载遮罩移除 `transition: opacity 0.3s`（ADR-0003 无动效）
- FE-32：useLayout/useFontSize 改 selector 精确订阅
- FE-34：WebGL addon 加载失败回退才重建，焦点切换不主动释放；若实测发现多上下文压力则恢复释放并登记——**本项含实测决策，人工验证点兜底**
- FE-20：字体/快捷键/侧栏三 `loadFromDisk` 改 `Promise.all`（各自独立 try/catch 保留；loadAllProjects 保持在其后，markPersistenceReady 时序不动）
- FE-21：隐藏侧栏视图按需卸载（ADR-0001 已接受状态丢失；导航树滚动位置等轻状态不保活）
- FE-33：pageCallbacksRef 回调按 pageId 惰性创建 + 缓存（getOrCreate）

**验证断言要点**：MAX_PAGES=20 且超限拒绝+toast；useNavTree 存在 Map 索引（语义式）；TerminalPanel 订阅有 panelId 过滤；无 `transition: opacity` 于遮罩；App.tsx 三 loadFromDisk 在 Promise.all；SideBarArea 隐藏视图卸载（条件渲染非 display:none——语义式）；scan.rs 存在缓存键结构与 force 参数；L1/L2/L3 全绿
**门禁命令**：全量五条 + `npm run test:l3`
**commit**：`perf(frontend): 页数上限 20 + 树/订阅/启动加载性能优化 + 历史扫描缓存（FE-01/15/16/17/19/20/21/29/32/33/34、BE-19）`
**人工验证点**：多终端焦点快速切换观察 WebGL 上下文（chrome://gpu 或任务管理器 GPU 内存）；20 页上限 toast 实测；侧栏切换状态丢失符合 ADR-0001 预期

---

## S13 稳定性与生命周期

**改动项**：FE-22、FE-23、FE-24、FE-25、FE-26、FE-27、FE-28、BE-07、BE-08、BE-09

**跨边界契约（写死）**：
- 新命令 `pty_kill_all() -> Result<u32, AppError>`（返回 kill 数；逐 session kill+join 超时语义同 BE-06）；前端 wrapper `ptyKillAll(): Promise<number>`；本 Stage 后命令数 **34**
- `switchToPageAndFocus(pageId: string, signal?: AbortSignal)`；`restoreSession` 的 `waitFor(cond, signal?: AbortSignal)`——可选参数后向兼容，调用处逐步传入
- git_repo_cache LRU 容量 = **8**（零新依赖手实现）

| label | 负责项 | 文件 |
|-------|--------|------|
| boundary | FE-22、FE-28、BE-08（前端侧） | `src/panelRegistry.ts`、`src/App.tsx`、`src/ipc/pty.ts` + 对应 L2 测试 |
| lifecycle | FE-23、FE-24、FE-25、FE-26、FE-27 | `src/features/agentStatus/useAgentStatus.ts`、`src/panels/terminal/useXterm.ts`、`src/panels/hooksConfig/useHooksConfig.ts`、`src/workspace/pageApis.ts`、`src/features/agentHistory/restoreSession.ts` + 调用点适配（grep 确定） + 对应 L2 测试 |
| backend-stability | BE-07、BE-08（后端侧）、BE-09 | `src-tauri/src/notify/mod.rs`、`src-tauri/src/pty/spawn.rs`、`src-tauri/src/lib.rs`、`src-tauri/capabilities/default.json`、`src-tauri/src/state.rs`、`src-tauri/src/git/mod.rs` |

**实现要点**：
- FE-22：panelRegistry components 映射处 HOC 统一包 inline ErrorBoundary（单点）；L2 测试构造抛错面板验证同页其他面板存活
- FE-28：TitleBar / Workspace 容器 / NotificationListener / ConfirmDialogHost / ToastHost 分别包 inline ErrorBoundary（降级占位）
- BE-08：关闭序列 = 先前端 Registry 快速 kill，再 `pty_kill_all` 兜底（App.tsx 关闭链）
- FE-23：useAgentStatus 初始扫描引入 genRef（照 useFileTree 先例），setRows 前检查 generation
- FE-24：useXterm readHistoryTitle 加 isDisposedRef 守卫
- FE-25：setLayer async IIFE 加 try/catch + toast；confirmDiscard timeout id 存 ref + cleanup clearTimeout
- FE-26/27：轮询支持 AbortSignal；调用处（toast 点击/导航树行点击/恢复编排）传 Controller，卸载/新发起时 abort——**先 grep 全部调用点再适配**
- BE-07：fs-event 已有 300ms debounce——补单批 paths 阈值合并为 Rescan；agent-event 低频不节流（评估结论 S19 文档登记）
- BE-09：state.rs git_repo_cache 改简易 LRU（容量 8）；修正「目录切换时清除」失实注释；git/mod.rs 消费点适配

**验证断言要点**：panelRegistry 存在 ErrorBoundary HOC 包裹（语义式）；App.tsx 五个顶层组件各有边界；关闭序列调用 ptyKillAll；`pty_kill_all` 注册 + capabilities allow（命令数 34）；两轮询函数签名含 AbortSignal 且循环检查 aborted；useAgentStatus 有 genRef；git_repo_cache 有容量 8 淘汰逻辑；L1/L2 全绿
**commit**：`fix(stability): 面板级错误边界 + 生命周期守卫（AbortSignal/genRef/dispose）+ pty_kill_all 兜底 + git 缓存 LRU（FE-22~28、BE-07/08/09）`
**人工验证点**：构造渲染抛错面板验证同页存活；关窗后任务管理器确认无残留 pwsh/cmd 子进程

---

## S14 死代码清理

**改动项**：FE-35、BE-17、BE-20

| label | 负责项 | 文件 |
|-------|--------|------|
| deadcode-fe | FE-35 | 删除 `src/features/index.ts`、`src/panels/index.ts`、`src/features/agentHistory/index.ts`、`src/features/commit/index.ts`；改 `src/panelRegistry.ts`、`src/workspace/index.ts`、`src/panels/terminal/index.ts`、`src/ipc/index.ts`、`src/ipc/window.ts` + 消费方/测试适配（grep 确定） |
| deadcode-be | BE-20 | `src-tauri/src/hooks/signal.rs` |
| test-cfg | BE-17 | `src-tauri/src/lib.rs`、`src-tauri/src/fs/mod.rs`、`src-tauri/src/settings.rs`、`src-tauri/src/agent_history/claude/ops.rs` |

**实现要点**：
- FE-35：**每个删除点先 grep 消费方（含 `src/__tests__/`、`e2e-tests/`）再删**；`PANEL_GIT_SHOW`/`PANEL_DIFF`/`PANEL_HOOKS_CONFIG`/`terminalTabConfig` 逐一 grep 后定（有消费改常量引用，零消费删除）；`ping()` 保留但注释「测试专用」；`setFocus()` 删除；4 barrel 删除后若模块索引/文档引用在 S19 同步
- BE-20：移除 `#![allow(dead_code)]`，clippy 零警告（若暴露真 dead_code，删之）
- BE-17（D5）：6 处 `#[cfg(windows)]` 测试 cfg 改运行时 `cfg!(windows)` 分支；无法运行时区分的（如 symlink 特权测试）保留 cfg 并在模块 CLAUDE.md 登记豁免（S19 同步 DOC-02）

**验证断言要点**：4 barrel 文件不存在；`grep -rn "from.*features/index\|from.*panels/index"` 零命中；panelRegistry 非常量残留经 grep 判定；signal.rs 无 `allow(dead_code)`；6 处测试 cfg 改 `cfg!()`（grep `#[cfg(windows)]` 于四文件仅豁免残留）；clippy 零警告；全量测试绿
**commit**：`refactor(deadcode): 删除无消费 barrel/常量/setFocus + 移除 allow(dead_code) + 测试 cfg 改运行时分支（FE-35、BE-17/20）`

---

## S15 major 升级

**改动项**：TE-07、TE-08、TE-09、TE-10
**串行理由**：四 agent 均改 `package.json` + lock，pipeline 严格串行；按风险升序，dockview 最高风险最后

| 序 | label | 负责项 | 文件 |
|----|-------|--------|------|
| 1 | A: test-toolchain | TE-10 | `package.json`、`package-lock.json` + L2 适配（jsdom 30 行为变更） |
| 2 | B: typescript7 | TE-07 | `package.json`、`package-lock.json`、`tsconfig*.json`（如需适配）、`src/` 下源码类型适配文件（tsc 报错逐处修） |
| 3 | C: json-schema | TE-09 | `package.json`、`package-lock.json` + `src/features/hooksConfig/`（API 适配 + 去重评估） |
| 4 | D: dockview8 | TE-08 | `package.json`、`package-lock.json`、`src/workspace/`（layoutSerde/Workspace 适配）、`src/panels/`（如需） |

**实现要点**：
- 每步完成后在步骤内跑 `npx tsc --noEmit` + `npx eslint src/` + `npm test` + `npm run test:l3` 门禁（无 Rust 变更不跑 cargo 系）——依赖升级的兼容性只有测试能暴露，编译级检查不够；四步严格串行无并发冲突。任一步无法修复则回滚该步并记录，后续步骤基于绿态继续
- TE-09 去重评估：能统一则统一到单一 schema 校验库（优先与 CodeMirror 集成更好者，以实测 API 为准）；评估结论写 commit message + S19 登记
- TE-08：breaking changes 逐个适配（布局 serde/组件 API/样式类名）；dockview 8 样式 import 路径变更须核对
- **禁区**：dockview 升级不得改变 H6 多实例架构与 layoutSerde 单点（约束 #7）

**验证断言要点**：package.json 版本 = typescript ^7（或精确，随 S16 策略——本 Stage 先保 `^` 现状风格）/ dockview-react 8.x / json-schema-library 11.x / jsdom 30 / jest-dom 7 / @types/node 26 / cross-env 10；`npx tsc --noEmit` 全绿；layout-serde 测试全绿；L2/L3 全绿
**门禁命令**：全量五条 + `npm run test:l3` + `npx vite build` + `npm run build:e2e`
**commit**：单条 `chore(deps): major 升级——jsdom 30/typescript 7/json-schema-library 11/dockview-react 8（TE-07/08/09/10）`（workflow 原子执行逐步 commit 不可行；逐步门禁已保证中间态绿，被回滚步骤在 commit message 注明）
**人工验证点**：**dockview 8 升级后布局拖拽/分屏/页签恢复实测 + `npm run e2e` 全量**——serde 兼容性只能实测兜底（旧布局 JSON 能 fromJSON 恢复）

---

## S16 版本策略 + CI 门禁

**改动项**：TE-03、TE-04、TE-11、TE-13

| label | 负责项 | 文件 |
|-------|--------|------|
| version-policy | TE-03、TE-04、TE-11 | `package.json`、`.claude/adr.md` |
| ci-gates | TE-13 | `.github/workflows/ci.yml` |

**实现要点**：
- TE-11：生产运行时依赖（dependencies）精确版本、开发工具（devDependencies）`^`；逐条调整 + adr.md 登记约定
- TE-03：xterm beta 保留——adr.md 补升级审批约定（xterm 升级须全量 L3+E2E+实测滚轮）
- TE-04：notify RC 保留——adr.md 登记（rc.4 即最新，无稳定版可升；watcher 51 条 L1 守护）
- TE-13（D10）：ci.yml 增 `npm audit --registry=https://registry.npmjs.org/ --audit-level=high`（high 阻断）、`npx knip --production`、`cargo install cargo-audit && cargo audit`

**验证断言要点**：package.json dependencies 全精确版本（无 `^`）且 devDependencies 全 `^`（xterm beta 等既定例外以 adr 登记为准）；adr.md 含三条登记（版本策略/xterm beta/notify RC）；ci.yml 含三门禁命令
**commit**：`chore(deps): 版本策略统一（生产精确/开发 ^）+ CI 增 audit/knip 门禁 + ADR 登记（TE-03/04/11/13）`
**人工验证点**：push 后 CI 实跑三项新门禁通过

---

## S17 hooks 写入校验

**改动项**：SEC-05、SEC-12、SEC-13

**跨边界契约（写死）**：`agent_hooks_config_write` 后端语义校验规则——事件名 ∈ HOOK_EVENTS（10 事件白名单）、handler `type == "command"`、`command` 为非空字符串；校验失败返回 `AppError::Validation`；**user 层写入时**前端 confirmDialog 二次确认（D9），project/local 层不确认

| label | 负责项 | 文件 |
|-------|--------|------|
| hooks-backend | SEC-05、SEC-12、SEC-13 | `src-tauri/src/hooks/claude/config.rs`、`src-tauri/src/hooks/claude/inject.rs`（SHA-256 如需新依赖则含 `src-tauri/Cargo.toml`） |
| hooks-frontend | SEC-05（前端侧） | `src/panels/hooksConfig/useHooksConfig.ts`、`src/panels/hooksConfig/ClaudeHooksConfigEditor.tsx`（以 grep `agent_hooks_config_write` 调用点为准） + 对应 L2 测试 |

**实现要点**：
- SEC-05：复用 S10 BE-18 所建结构体；user 层判定 = 前端调用处已知 layer，confirmDialog 仅在 layer==="user" 时弹出
- SEC-12：注入/重注入 statusline 时对原命令做模式审查（curl/wget/Invoke-Expression 等可疑模式 `tracing::warn!`），仅记录不阻断（信任边界 S19 登记）
- SEC-13：`include_str!` 内嵌脚本模板，状态检测时对磁盘脚本计算 SHA-256 与模板哈希比对，不一致 → Outdated；sha2 是否已传递可用——优先检查 Cargo.lock 现有依赖树，无则加 `sha2` 直接依赖

**验证断言要点**：config.rs 存在三规则校验（语义式）且 user 层写入路径有确认；inject.rs 存在可疑模式 warn 与 SHA-256 比对；L1（拒绝非法事件名/type/空 command）+ L2（user 层确认弹窗）全绿
**commit**：`fix(security): hooks 写入语义校验 + user 层二次确认 + statusline 审查 warn + 脚本哈希比对（SEC-05/12/13，D9）`
**人工验证点**：面板改 user 层 hooks → 弹确认框；写入非法事件名 → 后端拒绝

---

## S18 FileTree 虚拟化

**改动项**：FE-30
**独立 Stage 豁免理由**（偏离「每 Stage 3-15 项」）：大 UI 改动零新依赖手实现，行为面（键盘导航/右键菜单/选中模型）回归风险高，独立 Stage 便于回滚

| label | 负责项 | 文件 |
|-------|--------|------|
| filetree-virtual | FE-30 | `src/features/explorer/FileTree.tsx` + 对应 L2 测试 |

**实现要点**：扁平化可见节点数组 + 固定行高 + overscan 滚动窗口（零新依赖）；**键盘导航（上下/展开/收起）、右键菜单、选中模型、错误占位（S08 FE-07）行为全部保持**；虚拟化容器与 S12 FE-21 侧栏卸载兼容
**验证断言要点**：FileTree 存在窗口化渲染逻辑（语义式：可见切片 + 偏移）；DOM 节点数与可见行数同量级（L2 测试：构造 1000 节点树，渲染行 << 1000）；键盘导航/右键菜单 L2 用例全绿
**commit**：`perf(explorer): FileTree 虚拟化（零新依赖手实现）（FE-30）`
**人工验证点**：**大目录（node_modules 级，万级节点）展开/滚动/键盘导航/重命名/新建删除刷新实测**

---

## S19 文档同步

**改动项**：DOC-01~DOC-10 + 全模块 CLAUDE.md 随动 + test-inventory.md
**固定最后**：文档反映所有代码 Stage 完成后的最终状态

| label | 负责项 | 文件 |
|-------|--------|------|
| docs-root | DOC-01、DOC-02、DOC-03、DOC-04、DOC-05、DOC-06、DOC-07、DOC-08、DOC-09、DOC-10（root 侧） | `.claude/CLAUDE.md`、`CONTEXT.md`、`README.md`（新建）、`.claude/adr.md` |
| docs-fe-modules | 前端模块 CLAUDE.md 随动 | `src/ipc/CLAUDE.md`（SEC-06 消费点 + 命令数 34 + appError.ts）、`src/types/CLAUDE.md`（HooksLayer 收窄登记）、`src/workspace/CLAUDE.md`（FE-01 豁免 + MAX_PAGES）、`src/panels/editor` 对应 CLAUDE.md（FE-31 决策登记）、`src/features/explorer/CLAUDE.md`（FE-30 虚拟化）、`src/features/agentHistory/CLAUDE.md`（force/缓存）、`src/features/hooksConfig/CLAUDE.md`（SEC-05 确认）、`src/panels/CLAUDE.md`（SEC-04 nonce）、其余按 git diff 随动 |
| docs-be-modules | 后端模块 CLAUDE.md + 测试清单 | `src-tauri/src/CLAUDE.md`（app_dir 新模块 + 09#14 Mutex 登记 + 命令数 34）、`src-tauri/src/pty/CLAUDE.md`（MAX_PTY_SESSIONS + reader 微批 + kill 加固 + reattach 删除）、`src-tauri/src/notify/CLAUDE.md`（WATCH_EXCLUDE_DIRS + 容量 8 + stop_watch + 背压合并）、`src-tauri/src/fs/CLAUDE.md`（BE-21 豁免 + 分块）、`src-tauri/src/hooks/CLAUDE.md`（SEC-02/05/12/13 + BE-20）、`src-tauri/src/agent_history/CLAUDE.md`（缓存/force）、`src-tauri/src/git/CLAUDE.md`（LRU 8）、`.claude/test-inventory.md`（DOC-01 豁免表更新 + 全部新增用例登记） |

**实现要点**：
- DOC-01：约束 #11 修订——可自动化部分必须覆盖；不可自动化部分须在 test-inventory 豁免清单登记并注明原因与兜底层级；**reader_loop 豁免项 1 因 S06 变动，豁免表同步更新**
- DOC-02：约束 #9 修订——业务 cfg 仅 pty/conpty_api/shell/win_build；测试 cfg 原则上 `cfg!()`，例外须模块 CLAUDE.md 登记（S14 豁免残留处登记）
- DOC-03/04/05/06/07：按 checklist 修复要点写入约束正文
- DOC-10 汇总登记：FE-01（Workspace 多实例豁免 + MAX_PAGES）、SEC-09（CSP srcdoc 继承）、SEC-06（剪贴板消费点）、BE-21（read_dir 分页豁免）、FE-31（CM 不虚拟化）、09#14（Mutex 中毒保持现状，parking_lot/catch_unwind 仅作未来预案）、TE-03/TE-04（adr 已有则交叉引用 S16 登记）
- 各 agent 先 `git log --oneline` + 读本计划文档了解全量变更再动笔；test-inventory 登记全部新增/修改用例（逐 Stage grep 新增测试函数）

**验证断言要点**：根 CLAUDE.md 约束 #2/#4/#5/#6/#9/#11 含新措辞；README.md 存在；CONTEXT.md `htmlviewer`；adr.md 含 S16/S19 登记项；各模块 CLAUDE.md 与代码最终态一致（抽查 reader 微批/命令数 34/WATCH_EXCLUDE_DIRS 等关键事实不撒谎——语义式核对）；test-inventory.md 豁免表项 1 更新 + 新增用例全登记
**commit**：`docs: 约束修订 + 豁免/决策汇总登记 + 模块 CLAUDE.md/test-inventory 全量同步（DOC-01~10）`
