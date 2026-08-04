# 自动化测试优化清单（checklist）

> 输入：`docs/test-review-problem/` 00-summary + 15 份领域报告（195 条原始发现）
> 去重合并后 **164 项** | 生成日期 2026-08-04 | 本清单只列问题与修复要点，执行归 `/systematic-changes-execute`

## 决策记录（grill 已确认，执行期不再重议）

| # | 决策 |
|---|------|
| D1 | **全量修复**：🔴🟡🟢 所有级别 + 结构重构项（test.e2e.ts 拆分、git 测试拆分、IPC 契约参数化等）全部纳入 |
| D2 | 允许**最小可测性重构**：抽纯函数 / 参数注入 / 导出符号，零行为变更；除此之外不改生产代码 |
| D3 | 矛盾项**测试对齐实现**：锁定现状 + 注释标明"已知当前行为"，生产行为一律不变 |
| D4 | L3 keyboard ~30 条同义反复 → **降级标注保留**（重定位"xterm.js 基础行为回归"），不删除；另补生产 theme/OSC 覆盖 |
| D5 | L4 hooks 隔离 → **备份扩展**（`~/.claude/settings.json` 备份还原 + 清理 `~/.slterminal/hooks/` 与 `hooks-events/`）+ 新增 1 条真实 hook reporter 链路用例 |
| D6 | 豁免**分类处理**：物理不可自动化项维持豁免 + 文档化（L4 真实 OS 按键、L3 WebGL/mouse tracking、HTML postMessage 真实 WebView2、E2E_ENABLED=false 生产分支、lib.rs run()）；「无 AppHandle」类（FileWatcher::start/notify_watch、claude_history 命令包装、reader_loop I/O、process_signal_file、HookSignalWatcher 事件循环）**抽 trait/参数注入纳入 L1 补测**，残余不可抽部分文档化 |
| D7 | **防复发测试纳入**：payload 键集合精确匹配、时序/顺序断言、SEC-01/SEC-08 拒绝分支、C10 守卫、变异推演漏网分支 |

## 组织约定

- **ID 编号**：模块前缀 + 两位序号。Stage 划分按 ID 引用。
- **优先级**：不使用 P0-P4；以严重级别（🔴 > 🟡 > 🟢）+ Stage 依赖顺序表达。
- **合并留痕**：跨报告重复的条目在「来源」行标注全部并入来源（格式 `报告号-条目号`）。
- **事实核验**：引用路径均已 Glob 命中；库行为断言附一手证据 `file:line`。15-P9（256 色）报告引用失实已更正（见 E2E-08）。

## ID 前缀与统计

| 前缀 | 领域 | 项数 | 🔴 | 🟡 | 🟢 |
|------|------|-----|----|----|----|
| PTY | L1 pty + state | 13 | 3 | 9 | 1 |
| GIT | L1 git | 12 | 3 | 7 | 2 |
| HUK | L1 hooks | 11 | 4 | 6 | 1 |
| HFN | L1 history/fs/notify | 9 | 0 | 8 | 1 |
| SPE | L1 settings/projects/error | 6 | 1 | 4 | 1 |
| TRM | L2 终端面板 | 8 | 3 | 4 | 1 |
| EDF | L2 编辑器/diff/gitshow | 9 | 4 | 5 | 0 |
| WRK | L2 workspace/启动关闭 | 11 | 4 | 6 | 1 |
| EXP | L2 explorer/sidebar | 12 | 2 | 8 | 2 |
| SVC | L2 sideviews/commit | 14 | 3 | 9 | 2 |
| HKC | L2 hooks 配置面板 | 10 | 3 | 6 | 1 |
| STS | L2 shortcuts/theme/store | 11 | 1 | 7 | 3 |
| IHE | L2 ipc/html/e2e 门控 | 8 | 1 | 4 | 3 |
| NAH | L2 通知/agent/历史 | 11 | 3 | 6 | 2 |
| E2E | L3 headless + L4 E2E | 15 | 6 | 6 | 3 |
| DOC | 文档同步 | 4 | 0 | 0 | 4 |
| **合计** | | **164** | **41** | **95** | **28** |

---

## A. L1 Rust 后端（51 项）

### A1. PTY（13 项）

#### PTY-01 🔴 Job Object 孤儿防护零 L1 覆盖
- **位置**：`src-tauri/src/pty/spawn.rs:1185-1263`（add_to_job_object/create_and_assign_job）、`706-714`（JobHandle::drop）
- **问题**：`CreateJobObjectW`/`SetInformationJobObject`/`AssignProcessToJobObject` 参数与 `KILL_ON_JOB_CLOSE` 设置、JobHandle Drop 全部无测试；父进程崩溃时子进程残留无回归。
- **修复**：按 D2 抽纯逻辑（job_name 构造、limit flags 计算）补 L1 单测；L4 新增"杀 slterminal.exe 后检查子进程残留"用例 → E2E-12。
- **来源**：01 P-1

#### PTY-02 🔴 pty_spawn 校验路径零覆盖
- **位置**：`src-tauri/src/pty/spawn.rs:756-970`（762-767 尺寸超限、770-772 shell 白名单、775-781 cwd 沙箱）
- **问题**：尺寸超限、shell 白名单、cwd 沙箱三条拒绝路径 L1 全未验证，非法请求可穿透。
- **修复**：抽 `validate_spawn_request` 纯函数（尺寸/白名单/cwd 三校验），补边界用例；命令层最小集成测试（构造 AppState + await 调用）。
- **来源**：01 P-2

#### PTY-03 🔴 pty_write/resize/kill/reattach + SEC-08 归属校验零覆盖
- **位置**：`src-tauri/src/pty/spawn.rs:977-1183`
- **问题**：四命令的 SEC-08 panelId 归属校验（放行/拒绝）L1 全未验证。
- **修复**：抽 `validate_session_ownership` 纯函数，补归属放行/拒绝用例（D7 防复发）。
- **来源**：01 P-3

#### PTY-04 🟡 strip_conpty_startup 未覆盖分支
- **位置**：`src-tauri/src/pty/reader.rs:166-235`
- **问题**：非 Windows 原样返回分支（`cfg!(windows)` 运行时分支）、OSC 1/3/4/9 保留、CSI 3J 未测。
- **修复**：非 Windows 分支标注"由 cfg 守护，Windows CI 不可达"并补 `cfg!(windows)` 常量断言；OSC 1/3/4/9、CSI 3J 补保留用例。
- **来源**：01 P-4

#### PTY-05 🟡 ring_buffer_append 无换行长行淘汰边界
- **位置**：`src-tauri/src/state.rs:201-218`
- **问题**：`map_or` 右侧 or 分支（1024 字节内无换行则按 1024 原量淘汰）未测；超长单行可能死循环/淘汰错误。
- **修复**：补无换行超长行淘汰用例（恰好 1024、超 1024、含换行三边界）。
- **来源**：01 P-5

#### PTY-06 🟡 resolve_shell_info 自动检测回退顺序未测
- **位置**：`src-tauri/src/pty/shell.rs:94-127`
- **问题**：pwsh→powershell→cmd 回退顺序未验证（PATH 构造三场景）。
- **修复**：构造可控 PATH（tempdir 放假 exe）验证三档回退顺序与命中。
- **来源**：01 P-6

#### PTY-07 🟡 build_cmdline 引号处理未测
- **位置**：`src-tauri/src/pty/spawn.rs:81-99`
- **问题**：程序路径/参数含空格时加引号逻辑未验证。
- **修复**：补含空格路径、含空格参数、无空格不加引号三用例。
- **来源**：01 P-7

#### PTY-08 🟡 spawn_conpty_child 仅集成覆盖
- **位置**：`src-tauri/src/pty/spawn.rs:398-459`
- **问题**：AttrList set_pty → CreateProcessW 组合只在集成测试端到端覆盖，参数错误无单测定位。
- **修复**：可纯化部分（命令行/环境块构造）抽函数补单测；纯 Win32 调用部分标注"由 pty_spawn_custom_conpty 集成测试 + CI 守卫"。
- **来源**：01 P-8

#### PTY-09 🟡 ConPtyMaster::resize HPCON invalid 分支未覆盖
- **位置**：`src-tauri/src/pty/spawn.rs:201-217`
- **问题**：HPCON 已关闭后 resize 应静默更新 size 不调 Win32 API，该分支未测。
- **修复**：构造 invalid HPCON 状态断言 resize 静默成功且 size 更新。
- **来源**：01 P-9

#### PTY-10 🟡 resolve_shell 回退 + 白名单 PATH 解析后仍非法
- **位置**：`src-tauri/src/pty/shell.rs:68-86`、`283-322`
- **问题**：`resolve_shell` 回退路径、用户指定 shell 经 PATH 解析后仍不在白名单的拒绝分支未测。
- **修复**：补回退顺序用例 + 白名单拒绝用例（解析成功但非 pwsh/powershell/cmd）。
- **来源**：01 P-10

#### PTY-11 🟡 validate_path_within_root 相对路径 `..` 穿越未测
- **位置**：`src-tauri/src/state.rs:138-177`
- **问题**：相对路径含 `..` 穿越沙箱根的拒绝分支未测（SEC-01 防线）。
- **修复**：补 `..` 穿越拒绝、相对路径正常放行两用例（D7 防复发）。
- **来源**：01 P-11

#### PTY-12 🟡 reader_loop I/O 编排——评估抽取 + 残余豁免文档化
- **位置**：`src-tauri/src/pty/reader.rs:31-154`
- **问题**：channel send vs ring buffer 回退、EOF `child.wait()`、tracing 告警等 I/O 编排分支零覆盖（模块 CLAUDE.md 已标"已尽力"纯函数化）。
- **修复**：按 D6 评估可抽取的决策点（如"channel 断开→写 ring buffer"分流表、EOF 处理决策），能抽为注入参数的补 L1；确不可抽的残余分支在 pty/CLAUDE.md 补豁免文档（→ DOC-01 引用）。
- **来源**：01 P-12（按 D6 从豁免重分类为补测）

#### PTY-13 🟢 spawn.rs 测试清理重复 + canonicalize/which 边界
- **位置**：`src-tauri/src/pty/spawn.rs:1365-1371` 等三处、`src-tauri/src/state.rs:100-131`、`src-tauri/src/pty/shell.rs:173-183`
- **问题**：①spawn.rs 测试三处清理代码重复未抽 helper；②`canonicalize_or_ancestor` relative 路径分支、`which_full_path` PATH 顺序未测。
- **修复**：①抽测试清理 helper；②补两函数边界用例。
- **来源**：01 P-13、01 P-15（合并；01 P-14 集成测试平台限制说明 → DOC-01）

### A2. GIT（12 项）

#### GIT-01 🔴 五命令补命令层测试 + 重写 inline 假测试
- **位置**：`src-tauri/src/git/mod.rs:127-582`（五命令）、`2136-2703`（git_rollback_*/git_unstage_*/git_file_at_head_* 测试）
- **问题**：①`git_status`/`git_diff`/`git_file_at_head`/`git_rollback`/`git_unstage` 的 State 注入、路径沙箱、`spawn_blocking`、错误消息契约零命令层测试；②15 条测试 inline 重写 git2 调用序列，不调被测函数，守护的是测试副本。
- **修复**：构造最小 AppState + `block_on` await 真实命令（每命令 ≥3 条：happy/沙箱拒绝/错误契约）；inline 重写测试改为调真实命令，保留的 git2 行为测试标注"底层原语"。
- **来源**：02 P-1、02 P-4（合并）

#### GIT-02 🔴 git_rollback_two_step_* 7 条验证已废弃实现
- **位置**：`src-tauri/src/git/mod.rs:2421-2621`
- **问题**：生产已改为 `std::fs::write(blob) + index.add_path + index.write`，测试仍验证废弃的 `reset_default + checkout_index` 两步法。
- **修复**：删除或重写为当前命令路径（D3 测试对齐实现）。
- **来源**：02 P-2

#### GIT-03 🔴 git_status_non_renamed_old_path_is_none 假测试
- **位置**：`src-tauri/src/git/mod.rs:1700-1739`
- **问题**：循环内 `continue` 跳过 renamed 条目后再断言 oldPath 为 none——条件恒真，永不可失败。
- **修复**：重写为构造非 renamed 条目断言 `oldPath === null`、renamed 条目断言 oldPath 为旧路径（一手证据：生产已开 renames 检测，`git/mod.rs:145-151`）。
- **来源**：02 P-3

#### GIT-04 🟡 status_to_str conflict 分支未覆盖
- **位置**：`src-tauri/src/git/mod.rs:42-43`
- **修复**：补 `git2::Status::CONFLICTED` → `"conflict"` 表驱动用例。
- **来源**：02 P-5

#### GIT-05 🟡 compute_diff_hunks 三处边界未覆盖
- **位置**：`src-tauri/src/git/mod.rs:315-320`（修改后多余新增行）、`361-363`（prev_was_del flush）、`264-265`（非 UnbornBranch HEAD 错误）
- **修复**：三处各补一条精确 hunk 断言用例。
- **来源**：02 P-6

#### GIT-06 🟡 测试未隔离系统 git 全局配置
- **位置**：`src-tauri/src/git/mod.rs:594-636`（init_temp_repo/commit_file）
- **问题**：依赖 runner 全局 git 配置（autocrlf/safecrlf/defaultBranch），换环境结果漂移。
- **修复**：`init_temp_repo` 内设仓库局部 `core.autocrlf=false`、`core.safecrlf=false`、`init.defaultBranch=main`。
- **来源**：02 P-7

#### GIT-07 🟡 git_status 弱断言 `any(...)` 精确化
- **位置**：`src-tauri/src/git/mod.rs:691-862`（五条）
- **问题**：`entries.iter().any(...)` 只验证存在性，路径/状态串/条目数错误不红。
- **修复**：改精确断言（路径集合 + 状态 + 数量，D7 payload 键集合精确匹配同款思路）。
- **来源**：02 P-8

#### GIT-08 🟡 名实不符改名 + .gitignore 时序 + git 版本声明
- **位置**：`src-tauri/src/git/mod.rs:1109-1262`（git_diff_returns_hunks 等四条）、`771-862`（.gitignore 磁盘时序）
- **问题**：①四条 diff 测试名暗示精确验证实为存在性断言；②.gitignore 用例依赖磁盘写入时序，CI 偶发；③系统 git CLI 最低版本未声明。
- **修复**：①改名或补精确断言；②改 `git2` 内存 ignore 规则（add_ignore_rule）消除时序；③模块 CLAUDE.md 声明 git CLI 最低版本（→ DOC-04 同步）。
- **来源**：02 P-9、02 P-10、02 P-13（合并）

#### GIT-09 🟡 git_file_at_head_unborn_branch_err 未调被测函数
- **位置**：`src-tauri/src/git/mod.rs:2145-2157`
- **问题**：只验证 `git2::Repository::head()` 返回 UnbornBranch，未验证命令错误消息契约（"HEAD 中不存在"）。
- **修复**：改调真实 `git_file_at_head` 命令，断言 AppError::Git 消息含"HEAD 中不存在"。
- **来源**：02 P-14

#### GIT-10 🟡 五命令沙箱拒绝分支未覆盖
- **位置**：`src-tauri/src/git/mod.rs:139`、`229-231`、`407`、`472`、`550`
- **修复**：随 GIT-01 命令层测试补齐五命令 `validate_path_within_root` 拒绝用例（SEC-01，D7）。
- **来源**：02 P-15

#### GIT-11 🟢 ci_l1_uses_single_test_thread 领域污染迁移
- **位置**：`src-tauri/src/git/mod.rs:2000-2011`
- **修复**：迁移至 `tests/ci_config_tests.rs`（新建），git 域测试文件只留 git 用例。
- **来源**：02 P-11

#### GIT-12 🟢 88 条单文件拆分 + 工厂提取
- **位置**：`src-tauri/src/git/mod.rs:584-2718`
- **修复**：按命令拆分为独立测试文件（status/diff/at_head/rollback/unstage），`init_temp_repo`/`commit_file` 提取共享 test_utils；本项先行为 GIT-01/02/03/07 等新测试落位。
- **来源**：02 P-12

### A3. HUK（11 项）

#### HUK-01 🔴 process_signal_file 全流程零覆盖
- **位置**：`src-tauri/src/hooks/signal.rs:52-79`
- **问题**：读文件 → parse → emit("hook-event") → 删文件全链路（含 emit 失败仍删文件）未测。
- **修复**：按 D6 将 emit 抽为注入参数（闭包/trait），tempdir 构造信号文件验证读→emit→删全流程 + emit 失败仍删除 + 非法 JSON 降级。
- **来源**：03 P-01

#### HUK-02 🔴 hooks_inject/uninstall/injection_status 三命令零 L1
- **位置**：`src-tauri/src/hooks/inject.rs:191-274`、`280-351`、`358-423`
- **问题**：settings.json merge、非法 JSON 中止、版本比对、目录删除等核心逻辑无 L1 回归（仅靠纯函数拼装测试）。
- **修复**：按 D2/D6 抽 `inject_impl(settings_path, script_dir)` 等路径可注入的同步函数，tempdir 驱动三命令场景（注入/幂等/非法中止/卸载混组保用户 handler/状态三态）。
- **来源**：03 P-02

#### HUK-03 🔴 HookSignalWatcher::start 双通道事件循环零 L1
- **位置**：`src-tauri/src/hooks/watcher.rs:46-136`
- **问题**：notify 实时 + 3s 轮询补漏、notify 降级 warn、目录删除重建恢复等 win10 实证兜底逻辑未测。
- **修复**：按 D6 拆 `run_one_tick` 可测单元或写临时目录真实启动 watcher 的集成测试（轮询补漏消费残留文件、目录重建后恢复）。
- **来源**：03 P-03

#### HUK-04 🔴 start_signal_watcher 全局启动零 L1
- **位置**：`src-tauri/src/hooks/mod.rs:63-84`
- **问题**：幂等启动（已启动跳过）、WATCHER 静态实例管理未测。
- **修复**：加 `#[cfg(test)]` 重置钩子，补首次启动/重复启动幂等用例。
- **来源**：03 P-04

#### HUK-05 🟡 hooks_context_usage 命令包装未覆盖
- **位置**：`src-tauri/src/hooks/usage.rs:34-42`
- **修复**：补命令包装层用例（参数透传 transcriptPath、None/Some 返回映射）。
- **来源**：03 P-05

#### HUK-06 🟡 config 读写包装 + IO 异常分支未覆盖
- **位置**：`src-tauri/src/hooks/config.rs:66-68`、`94`、`121`、`145-147`、`154-176`、`182-207`
- **问题**：`home_dir()` 失败、persist 失败、命令包装参数透传未测。
- **修复**：补 IO 异常分支（注入失败点或用不可写路径）+ 包装层透传用例。
- **来源**：03 P-06

#### HUK-07 🟡 config user 层测试依赖真实 home 目录
- **位置**：`src-tauri/src/hooks/config.rs:233-238`
- **问题**：user 层路径解析测试读真实 `dirs::home_dir()`，环境污染风险。
- **修复**：home 解析抽为可注入参数，测试注入 tempdir。
- **来源**：03 P-07

#### HUK-08 🟡 inject_adds_10_events 弱断言
- **位置**：`src-tauri/src/hooks/inject.rs:651-661`
- **问题**：只检查事件键存在，未断言 handler 的 `type`/`matcher`/`timeout`/`command` 字段。
- **修复**：改结构断言（每事件 handler 数组含 `{type:"command", timeout:5, command 含 slterm-hook-reporter}`，D7 键集合精确匹配）。
- **来源**：03 P-08

#### HUK-09 🟡 serde camelCase `contains` 弱断言
- **位置**：`src-tauri/src/hooks/mod.rs:98-144`、`signal.rs:144-174`
- **问题**：`json.contains("panelId")` 不防字段值/类型错误。
- **修复**：改序列化→反序列化往返精确断言 + 键集合精确匹配。
- **来源**：03 P-09

#### HUK-10 🟡 P2-TE-05 与 scan_transcript_usage 用例重复
- **位置**：`src-tauri/src/hooks/usage.rs:371-483` vs `256-318`
- **问题**：两组用例覆盖同一纯函数路径，重复维护。
- **修复**：去重合并；保留的一组改为经命令包装层调用（与 HUK-05 协同）。
- **来源**：03 P-10

#### HUK-11 🟢 watcher stop 无结束断言 + handler_contains_slterm 非字符串分支
- **位置**：`src-tauri/src/hooks/watcher.rs:337-348`、`inject.rs:98-102`
- **修复**：①stop 测试补 `thread.is_finished()` 断言；②补 command 为非字符串（number/null）时 `handler_contains_slterm` 返回 false 用例。
- **来源**：03 P-11、03 P-12（合并；03 P-13 test-inventory stale → DOC-03）

### A4. HFN（9 项）

#### HFN-01 🟡 fs write_file_tests 与实现同构
- **位置**：`src-tauri/src/fs/mod.rs:492-632`
- **问题**：测试重写 `use_crlf` 检测与行尾转换逻辑，生产改为恒 CRLF/LF 时期望跟随变（循环断言）。
- **修复**：改直接调 `fs_write_file` 命令，用固定输入/输出字节断言（CRLF 保持/LF 保持/新文件平台默认/混合归一）。
- **来源**：04 P-1

#### HFN-02 🟡 pool.rs:66 替换分支未真正覆盖
- **位置**：`src-tauri/src/notify/pool.rs:66`（测试在 pool.rs 测试模块）
- **问题**：p10 测试先手动 `pool.remove(&path)` 再 insert，`insert` 内部"已存在→stop 旧 watcher"分支未执行。
- **修复**：去掉手动 remove，同 path 直接两次 insert，断言旧 watcher 被 stop。
- **来源**：04 P-2

#### HFN-03 🟡 FileWatcher::start / notify_watch 零 L1——抽 EventEmitter trait 补测
- **位置**：`src-tauri/src/notify/mod.rs:62-157`、`214-270`
- **问题**：debouncer 创建、watch 注册、事件循环、pause/resume、emit 全部无 L1（原豁免：无 AppHandle）。
- **修复**：按 D6 抽 `EventEmitter` trait（生产实现包 AppHandle emit），L1 用 mock emitter 驱动事件循环；notify_watch 的沙箱校验/pool 交互分支补用例。
- **来源**：04 P-3（按 D6 从豁免重分类为补测）

#### HFN-04 🟡 fs 异常路径未覆盖
- **位置**：`src-tauri/src/fs/mod.rs:221`（fs_delete 不存在）、create_dir/delete 沙箱拒绝、TaskJoin panic 映射
- **修复**：补删除不存在路径、root 外拒绝、spawn_blocking panic → AppError 映射三用例。
- **来源**：04 P-4

#### HFN-05 🟡 claude_history 命令包装 + IO 降级路径未覆盖
- **位置**：`src-tauri/src/claude_history/scan.rs:42,49,54,58`、`ops.rs:43,48,73`
- **问题**：命令包装层（spawn_blocking/参数透传）与 metadata 失败 → mtimeMs=0 等 IO 降级分支未测（原豁免"命令包装不直测"）。
- **修复**：按 D6 补包装层最小用例 + IO 降级用例（不可读文件 → 降级条目）。
- **来源**：04 P-5（按 D6 从豁免重分类为补测）

#### HFN-06 🟡 scan.rs env 无 RAII 清理
- **位置**：`src-tauri/src/claude_history/scan.rs:163-169`
- **问题**：`SLTERM_CLAUDE_PROJECTS_DIR` set 后 panic 会残留污染环境变量，后续用例被波及。
- **修复**：引入 `ScanRootGuard`（Drop 时恢复 env）替换手动 set/unset。
- **来源**：04 P-6

#### HFN-07 🟡 notify Drop 测试固定 sleep(100ms)
- **位置**：`src-tauri/src/notify/mod.rs:567`
- **修复**：改轮询等待 `thread.is_finished()`（2s 超时），消除慢 CI flaky。
- **来源**：04 P-7

#### HFN-08 🟡 fs 测试 as_tauri_state transmute
- **位置**：`src-tauri/src/fs/mod.rs:285-288`
- **问题**：测试用 transmute 构造 `State<AppState>`，UB 风险且脆弱。
- **修复**：抽命令内核为纯函数（State 仅做提取），测试调纯函数；或改安全构造。
- **来源**：04 P-8

#### HFN-09 🟢 pool p9_drop 无断言 + scan 命名误导 + ops 空串恒真断言
- **位置**：`src-tauri/src/notify/pool.rs:303-307`、`claude_history/scan.rs:240-260`、`claude_history/ops.rs:139-148`
- **修复**：①p9 drop 测试补线程退出断言；②`scan_multiple_sessions_sorted_input_order` 改名（不验证顺序）；③ops 空串 UUID 用例改断言错误消息含具体校验文案（`msg.contains(bad)` 空串恒真）。
- **来源**：04 P-9、04 P-10、04 P-11（合并）

### A5. SPE（6 项）

#### SPE-01 🔴 settings 全部核心用例未调真实命令
- **位置**：`src-tauri/src/settings.rs:114-498`
- **问题**：`.bak` 备份恢复、原子写入、浅合并、`spawn_blocking`、TaskJoin 全在 inline 重写测试中虚构，真实 `save_settings`/`load_settings` 命令从未被调用——命令路径架空。
- **修复**：`tokio::runtime::Runtime::block_on` 调真实命令；`app_data_dir()` 抽为可注入（测试注 tempdir）；覆盖备份恢复/浅合并不擦他段/原子写。
- **来源**：05 P-1、05 P-2（合并）

#### SPE-02 🟡 projects.rs 命令包装层未覆盖
- **位置**：`src-tauri/src/projects.rs:64-81`
- **修复**：新增用例直接 `block_on` 调 `save_projects`/`load_projects`（app_data_dir 注入 tempdir）。
- **来源**：05 P-3

#### SPE-03 🟡 error.rs 三个 From 实现未覆盖
- **位置**：`src-tauri/src/error.rs:49-63`
- **修复**：补 `serde_json::Error`/`git2::Error`/`tokio::task::JoinError` → AppError 三转换用例（变体 + 消息契约）。
- **来源**：05 P-4

#### SPE-04 🟡 app_data_dir() 错误分支未覆盖
- **位置**：`src-tauri/src/settings.rs:10-20`
- **问题**：`current_exe` 失败、exe 无父目录两分支未测。
- **修复**：路径解析抽纯函数注入可失败点，补两错误分支。
- **来源**：05 P-5

#### SPE-05 🟡 persist 失败映射未覆盖
- **位置**：`src-tauri/src/projects.rs:25-28`、`settings.rs:63-64`
- **修复**：补 `NamedTempFile::persist` 失败 → AppError 映射用例（目标路径只读/冲突构造）。
- **来源**：05 P-6

#### SPE-06 🟢 settings 边界 + current_exe 依赖说明
- **位置**：`src-tauri/src/settings.rs:482-497`
- **修复**：①补并发写/只读文件/超大 JSON 边界用例（可行范围内）；②`app_data_dir` 依赖真实 current_exe 的测试加注释说明；lib.rs `run()` 维持豁免 → DOC-01。
- **来源**：05 P-7、05 P-8（合并；05 P-9 → DOC-01）

---

## B. L2 前端（94 项）

### B1. TRM（8 项）

#### TRM-01 🔴 use-xterm-lifecycle 与 use-xterm-output 14 条重复用例
- **位置**：`src/__tests__/use-xterm-lifecycle.test.ts`、`use-xterm-output.test.ts`（cancelPendingFlush/ResizeObserver 合帧等 14 条近逐字复制）
- **问题**：重复用例双倍维护，且 `await Promise.resolve()` 微任务时序假设脆弱（改实现调度即 flaky）。
- **修复**：去重归位（合帧属 output、生命周期属 lifecycle）；时序统一改用 fake timers 或显式 flush helper 替代裸 `await Promise.resolve()`。
- **来源**：06 #1、06 #3、06 #5（合并）

#### TRM-02 🔴 setBufferType("alternate") 虚假测试 + 死辅助删除
- **位置**：`src/__tests__/use-xterm-lifecycle.test.ts`、`src/__tests__/helpers/xterm-test-utils.ts`
- **问题**：源码从不读 `terminal.buffer.type`，测试仅给 mock 挂不会被读取的属性（换实现也能通过）；`xterm-test-utils.setBufferType` 是与之配套的脱节奏死辅助。
- **修复**：删除虚假用例与死辅助；交替缓冲行为改由 resize/fit 链路断言（真实读取路径）。
- **来源**：06 #2、06 #15（合并）

#### TRM-03 🟡 mock 混入不属于目标模块的 `hooks:` 字段（3 文件）
- **位置**：`src/__tests__/use-xterm-output.test.ts:136,162,175`、`src/__tests__/e2e-gating-terminal.test.ts:20,113,121,134`
- **问题**：`@xterm/addon-fit`/`TerminalRegistry`/`e2eEnabled` mock 被 copy-paste 混入 `hooks:` 虚假字段，误导维护者且 TS 严格检查会报错。
- **修复**：删除全部 mock 中目标模块未导出的字段；确需的依赖在测试内单独 mock。
- **来源**：06 #4、06 #7、13 P-12（合并）

#### TRM-04 🟡 usePtyOutput 64KB 淘汰 + 退出码分支 + E2E 缓冲截断
- **位置**：`src/panels/terminal/usePtyOutput.ts:191-217`
- **问题**：`pendingBufSizeRef` 超 `MAX_PENDING_BYTES`(64KB) 丢弃最旧块、PTY 退出码传递、`isCommandRunningRef=false` 时 E2E 缓冲行数截断均未测。
- **修复**：补 64KB 淘汰（恰好/超过/多块）、退出码透传、缓冲截断三用例。
- **来源**：06 #6、06 #12、06 #13（合并）

#### TRM-05 🟡 TerminalPanel 分支覆盖 42.85%
- **位置**：`src/panels/terminal/TerminalPanel.tsx`
- **问题**：1.5s 超时隐藏加载遮罩、`handleTabStateChange` active=false 恢复原标题、`windowsPty` 更新分支未测。
- **修复**：fake timers 补超时遮罩；补 active=false 标题恢复与 windowsPty 更新断言。
- **来源**：06 #8

#### TRM-06 🔴 webgl.ts 26.4%——setupWebglWithRetry 核心路径零覆盖
- **位置**：`src/panels/terminal/webgl.ts`
- **问题**：context loss 指数退避、重试耗尽回退 DOM、`cancel()` 清定时器全部未测（GPU 渲染稳定性无回归）。
- **修复**：fake timers 补退避序列/耗尽回退/cancel 清理全分支；L4 真实 context loss 场景归 E2E-04 视觉回归。
- **来源**：06 #9、15 附录（合并）

#### TRM-07 🟡 useTerminalInstance 多分支未覆盖
- **位置**：`src/panels/terminal/useTerminalInstance.ts`
- **问题**：`fonts.ready` catch、fontSize undefined、prevFontSize 相同跳过、webglAddon 已存在不重复加载分支未测。
- **修复**：四分支各补一条。
- **来源**：06 #10

#### TRM-08 🟢 TerminalRegistry getAll/_size/_dump 未覆盖
- **位置**：`src/panels/terminal/TerminalRegistry.ts`
- **修复**：terminal-registry.test.ts 补轻量断言（getAll 只读视图、_size 计数、_dump 不抛）；或 JSDoc `@internal` 标注仅供测试/调试。
- **来源**：06 #11、14 #12（合并）

### B2. EDF（9 项）

#### EDF-01 🔴 diff-panel 保存链用例名实不符
- **位置**：`src/__tests__/diff-panel.test.tsx:169-194`
- **问题**：注释声称验证 `writeFile → gitDiff → updateDiffGutter` 刷新链，实际只断言 mock 函数 toBeDefined——永不可失败。
- **修复**：真实触发保存（dispatch Ctrl+S 或调 handler），断言 `fs.writeFile` → `gitDiff` 重调 → 双侧 gutter/占位刷新全链。
- **来源**：07 R1

#### EDF-02 🔴 DiffPanel.tsx 63.9% 关键路径大面积缺失
- **位置**：`src/panels/diff/DiffPanel.tsx:239-250`（大文件）、`275-302`（refreshPlaceholders）、`306-347`（滚动重绑定）、`457-468`（脏文件确认）、`483-501`（.git 刷新）
- **修复**：按源码分支补 L2 用例：占位刷新同步、左侧 .git 变更重取 HEAD、外部修改净重载/脏弹窗、滚动同步重绑定、大文件阈值。
- **来源**：07 R2

#### EDF-03 🔴 useCodeMirror 大文件拒绝/警告/保存失败无直接回归
- **位置**：`src/panels/editor/useCodeMirror.ts:150-210`、`259-276`
- **问题**：>10MB 拒绝、>1MB confirm 取消、`fs.writeFile` reject 失败 alert 均无编辑器侧直接测试。
- **修复**：补三分支用例（mock fs + dialog.confirm）。
- **来源**：07 R3

#### EDF-04 🔴 gitshow 大文件警告断言薄弱 + params 切换断言无法区分
- **位置**：`src/__tests__/gitshow-panel.test.tsx:269-279`、`319-387`
- **问题**：大文件警告 header 断言薄弱；params.filePath 切换时"旧 view 销毁新 view 创建"的断言无法区分两者。
- **修复**：警告 header 精确断言；切换用例断言 EditorView 实例identity 变化（或销毁/创建计数）。
- **来源**：07 R4

#### EDF-05 🟡 gitGutter 四个 dispatch wrapper 未直接测试
- **位置**：`src/panels/editor/gitGutter.ts:261-328`（updateDiffGutter/clearDiffGutter/updateHeadDiffGutter/clearHeadDiffGutter）
- **修复**：补四 wrapper 直接调用用例（dispatch 的 StateEffect 类型与 RangeSet 结果）。
- **来源**：07 Y1

#### EDF-06 🟡 alignment key>=0 false 分支未覆盖
- **位置**：`src/panels/diff/alignment.ts:38,44`
- **修复**：补 key<0 过滤分支用例。
- **来源**：07 Y2

#### EDF-07 🟡 diff-panel 滚动同步固定 200ms 延时
- **位置**：`src/__tests__/diff-panel.test.tsx:198-291`
- **修复**：改 fake timers 或轮询断言，消除固定等待。
- **来源**：07 Y3

#### EDF-08 🟡 justSavedRef Set 多实例语义未测
- **位置**：`src/panels/editor/useCodeMirror.ts:143`、`365-369`
- **问题**：模块级 Set 在多编辑器实例并存时的隔离/清理语义未测。
- **修复**：补双实例保存-重载互不影响用例。
- **来源**：07 Y4

#### EDF-09 🟡 gitshow 字号 reconfigure 未覆盖
- **位置**：`src/panels/gitshow/GitShowPanel.tsx:172-180`
- **修复**：补 editorFontSize 变化 → fontCompartment.reconfigure 调用断言。
- **来源**：07 Y5

### B3. WRK（11 项）

#### WRK-01 🔴 PageDockviewHost.tsx 44.8%/12.19% 真实组件零覆盖
- **位置**：`src/workspace/PageDockviewHost.tsx`（DefaultTab/Watermark/RightHeader/handleReady/onSaveAs）
- **修复**：补真实 DefaultTab 渲染（tabIcon emoji/img 分支）、Watermark 按钮 addPanel、RightHeader、handleReady 空布局不兜底创建终端、onSaveAs 重算标题用例。
- **来源**：08 R1

#### WRK-02 🔴 pageApis.ts 42.2% 页面切换核心无 L2
- **位置**：`src/workspace/pageApis.ts`（switchToPageShared/switchToPageAndFocus）
- **问题**：`setProjectRoot` 先于 `setActivePage` 的 DBG-5/9 契约、轮询聚焦与超时降级无回归。
- **修复**：直接调用 `switchToPageShared` 断言 await 顺序 + `__dockviewApi` 重指（D7 时序断言）；`switchToPageAndFocus` 补轮询命中/超时降级。
- **来源**：08 R2

#### WRK-03 🔴 App.tsx 启动恢复顺序未断言
- **位置**：`src/App.tsx:76-84`、`183-186`（requestUserAttention catch）
- **问题**：startup-restore 验证了状态流转但未锁定 `setProjectRoot` 先于 `setActivePage`（DBG-6）；通知 catch 分支未测。
- **修复**：spy 断言两调用顺序（D7 时序断言）；补 requestUserAttention reject 静默 catch。
- **来源**：08 R3

#### WRK-04 🔴 ipc/window.ts onFocusChanged/setFocus 未覆盖
- **位置**：`src/ipc/window.ts:13-44`
- **问题**：focus 监听与设置函数零调用零测试。
- **修复**：确认无消费方则删除或标注"预留"；保留则补最小契约测试（命令名/参数/异常传播四维）。
- **来源**：08 R4、13 P-3（合并）

#### WRK-05 🟡 workspace-defaulttab 手写 MockDefaultTab 漂移风险
- **位置**：`src/__tests__/workspace-defaulttab.test.tsx`
- **问题**：测的是手写 Mock 而非生产 DefaultTab，`event.params.tabIcon` vs `event.tabIcon` 漂移无法发现。
- **修复**：改用生产 DefaultTab 渲染断言（params 变化 → 图标切换）。
- **来源**：08 Y1

#### WRK-06 🟡 workspace-switch-order 时序契约是手动模拟
- **位置**：`src/__tests__/workspace-switch-order.test.tsx`
- **问题**：手动模拟 setProjectRoot/setActivePage 顺序而非真实驱动；另有 3000ms 超时。
- **修复**：真实驱动 `Workspace.switchToPage`/`switchToPageShared` 断言顺序；超时收敛。
- **来源**：08 Y2

#### WRK-07 🟡 layout-serde mock isValidPanelType 仅 3 种 vs 真实 6 种
- **位置**：`src/__tests__/layout-serde.test.ts`
- **问题**：mock 白名单与真实 PANEL_TYPES 漂移，新面板类型过滤未验证。
- **修复**：改用真实 `PANEL_TYPES`（6 种）或断言 mock 与真实一致。
- **来源**：08 Y3

#### WRK-08 🟡 close-handler 未验证阻止默认关闭
- **位置**：`src/__tests__/`（close-handler 相关）
- **修复**：补关窗拦截（preventDefault/二次确认）行为断言。
- **来源**：08 Y4

#### WRK-09 🟡 workspace-multi-instance 仅 CSS display 断言 H6
- **位置**：`src/__tests__/workspace-multi-instance.test.tsx`
- **问题**：只断言 display none/block，未验证 Dockview 实例存活（H6 核心语义）。
- **修复**：补实例 identity 断言（同一 api 对象跨切换）+ 终端不 dispose。
- **来源**：08 Y5

#### WRK-10 🟡 main.tsx bootstrap catch 未覆盖
- **位置**：`src/main.tsx`
- **修复**：补 init 失败 catch 分支断言（错误展示/不白屏）。
- **来源**：08 Y6

#### WRK-11 🟢 残留行 + makeEmptyLayout 使用验证 + FILE_PANEL_TYPES 重复断言
- **位置**：`src/workspace/titleManager.ts`、`layoutSerde.ts`、`src/__tests__/panel-registry.test.ts`、`workspace-file-panel-types.test.ts`
- **修复**：①titleManager/layoutSerde 覆盖残留行补测或标注；②default-layout-format 补"SidebarTree 实际使用 makeEmptyLayout"断言；③FILE_PANEL_TYPES 两处重复断言合并为单点。
- **来源**：08 Y7、08 G1、08 G2（合并）

### B4. EXP（12 项）

#### EXP-01 🔴 handleOpenInTerminal 零覆盖
- **位置**：`src/features/explorer/ExplorerPanel.tsx:251-262`
- **修复**：右键"在终端中打开"触发后断言 `addPanel` 参数：`component="terminal"`、`params.cwd` 为目录（文件取父目录）、panelId 格式、`renderer:"always"`。
- **来源**：09 H1

#### EXP-02 🔴 CRUD 成功路径未断言
- **位置**：`src/features/explorer/ExplorerPanel.tsx:321,335,347,349-350`
- **问题**：删除/重命名/新建成功后 `refresh()`、`setRenamingPath(null)`、`setNewFileName(null)` 未断言——静默失败不红。
- **修复**：每个 CRUD 操作补成功路径断言（IPC 调用 + refresh 触发 + 状态重置）。
- **来源**：09 H2

#### EXP-03 🟡 fullRefresh 未调用 + F8 命名误导
- **位置**：`src/features/explorer/useFileTree.ts:191-206`
- **问题**：`fullRefresh` 定义但无调用方无测试；F8 用例断言的是初始 mount 的 gitStatus 而非 fullRefresh 结果。
- **修复**：确认 fullRefresh 语义（死代码则删或接线）；F8 改名或重写为真实 fullRefresh 驱动。
- **来源**：09 M1

#### EXP-04 🟡 焦点/失活/hover/错误横幅 dismiss 链路
- **位置**：`src/features/explorer/ExplorerPanel.tsx:91-157`
- **修复**：补 focusin/focusout 上下文栈、hover 高亮（非选中态）、错误横幅 dismiss 按钮用例。
- **来源**：09 M2、09 L2（合并）

#### EXP-05 🟡 FileIcon 扩展名分支未覆盖
- **位置**：`src/features/explorer/FileIcon.tsx`（.pyw/.markdown/.less/.scss/.gitattributes 等）
- **修复**：表驱动补未覆盖扩展名 → emoji 映射用例。
- **来源**：09 M3

#### EXP-06 🟡 FileTree 输入框边界
- **位置**：`src/features/explorer/FileTree.tsx`（重命名 input：Escape/空名/重名/失焦）
- **修复**：补四边界用例。
- **来源**：09 M4

#### EXP-07 🟡 useFileTree 竞态清理分支
- **位置**：`src/features/explorer/useFileTree.ts:65-66,139-140,232,241`
- **修复**：补 generation 过期丢弃、卸载清理分支（照模式二系统性改法：旧请求延迟 resolve → 断言丢弃）。
- **来源**：09 M5

#### EXP-08 🟡 SidebarTree 错误降级分支
- **位置**：`src/features/sidebar/SidebarTree.tsx:55-56,342,369,484`
- **修复**：补 dialog 取消/IPC 失败降级（console.error + 状态不变）用例。
- **来源**：09 M6

#### EXP-09 🟡 SidebarTree hover/stopPropagation 未覆盖
- **位置**：`src/features/sidebar/SidebarTree.tsx`（行 hover、按钮 stopPropagation）
- **修复**：补 hover 样式与按钮点击不触发行选择用例。
- **来源**：09 M7

#### EXP-10 🟡 handleOpenFile 防御分支
- **位置**：`src/features/explorer/ExplorerPanel.tsx:185,187,227`
- **修复**：补无活跃页/无 dockviewApi/重复打开去重聚焦防御用例。
- **来源**：09 M8

#### EXP-11 🟢 E6 标题矛盾 + 用例编号重复
- **位置**：`src/__tests__/explorer-delete.test.tsx:536-547`
- **修复**：E6 标题（handler 返回 false）与断言（deleteSelected 被调一次）对齐；全文用例编号去重统一。
- **来源**：09 L1、09 L3（合并）

#### EXP-12 🟢 FileViewerRegistry 单例 side-effect 恢复
- **位置**：`src/__tests__/file-viewer-registry.test.ts`
- **问题**：`_reset()` 后模块级单例预注册（html/htm）丢失，影响后续测试。
- **修复**：`_reset()` 后恢复预注册或改 per-test 新实例；并补 `_reset` 用例 + `resolve(".gitignore")`/`resolve("file.")` 边界。
- **来源**：09 L4、13 P-10（合并）

### B5. SVC（14 项）

#### SVC-01 🔴 activityBar drop 不校验 moveButton index
- **位置**：`src/__tests__/activityBar.test.tsx:260-495`
- **问题**：全部 drop 用例只断言 zone，`computeDropTarget` 落点 index 零守卫（落点偏移不红）。
- **修复**：每个 drop 用例追加 `expect(moveSpy.mock.calls[0][2]).toBe(expectedIndex)`。
- **来源**：10 P1

#### SVC-02 🔴 sideBar.ts cancelPendingSave 零覆盖（含三 store 活跃 timer 分支）
- **位置**：`src/stores/sideBar.ts:143-149`、`fontSize.ts:82-85`、`keybindings.ts:85-89`
- **问题**：关窗冲刷依赖 cancelPendingSave，活跃 timer 取消分支全未测（关窗竞态写盘）。
- **修复**：触发变更产生 timer → 调 cancelPendingSave → 推进 2s → 断言 saveSettings 未再调用（三 store 各一条）。
- **来源**：10 P2、12 P-10（合并）

#### SVC-03 🔴 useCommitStatus debounce 清理与去抖未覆盖
- **位置**：`src/features/commit/useCommitStatus.ts:88-108`
- **修复**：连续 fs-event 仅 1 次 gitStatus（200ms 去抖）；激活 timer 后 unmount 断言 clearTimeout。
- **来源**：10 P3

#### SVC-04 🟡 openCommitFile 四条守卫路径未覆盖
- **位置**：`src/features/commit/openCommitFile.ts:47,65,112,122`
- **修复**：补无 pageApi/未知状态/去重命中 focus/addPanel 失败降级用例。
- **来源**：10 P4

#### SVC-05 🟡 resolveTargetZone 中点边界未锁定
- **位置**：`src/features/sideViews/ActivityBar.tsx:93-99`
- **问题**：现有用例 clientY 远离中点，阈值 `>= rect.top + height/2` 边界未测。
- **修复**：补 clientY 恰好等于中点（→bottom）、中点 -1（→top）边界用例。
- **来源**：10 P5

#### SVC-06 🟡 moveButtonPure R7 目标区非空场景未测
- **位置**：`src/features/sideViews/sideBarState.ts:105-151`
- **修复**：补"跨区拖拽未打开视图且目标区已有打开视图"用例（仅归属变化，open 不动）。
- **来源**：10 P6

#### SVC-07 🟡 SideBarArea total<=0 除零守卫未覆盖
- **位置**：`src/features/sideViews/SideBarArea.tsx:75-82`
- **修复**：构造 total=0 场景断言不 NaN/不崩溃。
- **来源**：10 P7

#### SVC-08 🟡 CommitFileList 菜单交互 + oldPath 回退未覆盖
- **位置**：`src/features/commit/CommitFileList.tsx:130,133,253`
- **修复**：补右键菜单打开/项点击/oldPath ?? undefined 回退用例。
- **来源**：10 P8

#### SVC-09 🟡 commitContextMenu 删除 catch 未覆盖
- **位置**：`src/features/commit/commitContextMenu.ts:76,85`
- **修复**：补 gitUnstage/deleteEntry 失败 catch（静默/console.error）用例。
- **来源**：10 P9

#### SVC-10 🟡 workspace-sideviews props typeof 弱断言
- **位置**：`src/__tests__/workspace-sideviews.test.tsx:284-300`
- **修复**：`typeof props.switchToPage === "function"` 改引用断言（toBe 传入函数）。
- **来源**：10 P11

#### SVC-11 🟢 B10 反向用例错位——改经 openCommitFile
- **位置**：`src/__tests__/commit-view.test.tsx:488-497`
- **问题**：B10（suffix 去重）反向用例直测 titleManager 而非 commit 分派路径。
- **修复**：改经 `openCommitFile` 驱动验证"同文件不同 suffix 不误聚焦"。
- **来源**：10 P12

#### SVC-12 🟡 commit-view fake timers 混 waitFor
- **位置**：`src/__tests__/commit-view.test.tsx:532-642`
- **修复**：rootPath 切换用例统一计时策略（fake timers + advanceTimersByTimeAsync），不混 waitFor。
- **来源**：10 P13

#### SVC-13 🟢 sanitizeSideBar NaN/Infinity 分支未覆盖
- **位置**：`src/features/sideViews/sideBarState.ts:67-70`、`src/stores/sideBar.ts:59`
- **修复**：补 width/splitRatio 为 NaN/Infinity 时 clamp 回退用例。
- **来源**：10 P14、12 报告 sideBar clamp（合并）

#### SVC-14 🟢 commit-view.test.tsx 850+ 行拆分
- **位置**：`src/__tests__/commit-view.test.tsx`
- **修复**：拆分为状态机/分派去重/右键菜单三文件；ActivityBar 拖拽 mock 理想化局限标注 → DOC-01。
- **来源**：10 P15（10 P10 → DOC-01）

### B6. HKC（10 项）

#### HKC-01 🔴 JsonMode linter 包装顺序未锁定
- **位置**：`src/__tests__/hooks-config-jsonmode.test.tsx:158-181`
- **问题**：只断言 options，未锁定 `[0]=jsonParseLinter`、`[1]=jsonSchemaLinter`——交换后语法错误进 schema linter，误报误导。
- **修复**：追加 `linterCalls[0][0]`/`linterCalls[1][0]` 身份断言。
- **来源**：11 R1

#### HKC-02 🔴 useHooksConfig.load() generation 竞态无守卫
- **位置**：`src/panels/hooksConfig/useHooksConfig.ts:110-129`
- **修复**：模拟旧请求延迟 resolve，断言最终 configJson 为目标层数据（过期结果被丢弃）。
- **来源**：11 R2

#### HKC-03 🔴 HooksConfigPanel.handleJsonChange 非法 JSON catch 无回归
- **位置**：`src/panels/hooksConfig/HooksConfigPanel.tsx:146-155`
- **修复**：onChange 传非法文本，断言 configJson 保持原快照、保存按钮禁用、不崩溃。
- **来源**：11 R3

#### HKC-04 🟡 HandlerForm record/stringArray 清空删键未覆盖
- **位置**：`src/panels/hooksConfig/HandlerForm.tsx:337-342`
- **修复**：补字段清空 → 对象中删键（非置空）用例。
- **来源**：11 Y1

#### HKC-05 🟡 GuiMode 删除选中项后选中态重置未覆盖
- **位置**：`src/panels/hooksConfig/GuiMode.tsx:229-244`、`267-293`
- **修复**：补删除当前选中事件/handler → 选中态回退空态用例。
- **来源**：11 Y2

#### HKC-06 🟡 EventTree 未知事件分组未覆盖
- **位置**：`src/panels/hooksConfig/EventTree.tsx:143-154`
- **修复**：补配置含未知事件 → 归「未知事件」组渲染用例。
- **来源**：11 Y3

#### HKC-07 🟡 handleUninstall 失败分支未覆盖
- **位置**：`src/panels/hooksConfig/HooksConfigPanel.tsx:239-252`
- **修复**：补 uninstall reject → 错误提示 + 状态条不变用例。
- **来源**：11 Y4

#### HKC-08 🟡 validateHooksJson 直接边界未覆盖
- **位置**：`src/features/hooksConfig/schema/index.ts:61-79`
- **修复**：新建 `hooks-config-schema.test.ts`，直测 validateHooksJson（合法/缺 hooks 键/非法 matcher/未知事件告警边界）。
- **来源**：11 Y5

#### HKC-09 🟡 open-hooks-config-panel getPanel 无 focus 降级未覆盖
- **位置**：`src/__tests__/`（open-hooks-config-panel 相关）
- **修复**：补 getPanel 命中但无 focus 方法时的降级路径断言。
- **来源**：11 Y6

#### HKC-10 🟢 展示分支杂项
- **位置**：`src/__tests__/hooks-config-*.test.tsx`
- **修复**：补 JsonMode schema hover、注入状态条初始 "--"、MatcherTester placeholder 三处展示断言。
- **来源**：11 G1、11 G2、11 G3（合并）

### B7. STS（11 项）

#### STS-01 🔴 colors.test.ts 循环断言——改读 colors.ts 实际值
- **位置**：`src/__tests__/colors.test.ts:61-67,81-87,104-109,125-130,218-227`
- **问题**：`expect(expected).toMatch(HEX6_RE)` 断言的是测试文件内硬编码字面量自身，token 漂移/拼错永不红（变异推演已证）。
- **修复**：改 `expect(GIT_FILE_COLORS[key]).toBe(expected)` 等真实导出值比对（GIT_FILE/GIT_GUTTER/EXPLORER/SIDEBAR/AGENT_STATUS_USAGE 五组全改）。
- **来源**：12 P-1

#### STS-02 🟡 global-commands 用例名与断言不符
- **位置**：`src/__tests__/global-commands.test.ts:166-174`
- **问题**：名"handler 不传播异常"实只 `toBeDefined`（handler 从未调用）。
- **修复**：真调 handler 断言行为，或改名"factory 在 getter 抛异常时仍能创建命令对象"。
- **来源**：12 P-2

#### STS-03 🟡 forceContext 平局 tie-breaker 反向分支未覆盖
- **位置**：`src/features/shortcuts/ShortcutRegistry.ts:236-242`
- **修复**：补注册顺序 `global` 在前、`terminal` 在后 + `forceContext="terminal"` 用例（覆盖 aForced=0,bForced=1 方向）。
- **来源**：12 P-3

#### STS-04 🟡 getStatusIcon(null) 分支未覆盖
- **位置**：`src/lib/claudeStatus.ts:23-26`
- **修复**：补 `getStatusIcon(null)===""` 与 `getStatusIcon("working")==="⚡"`。
- **来源**：12 P-4

#### STS-05 🟡 theme.test.ts 未断言 kittyKeyboard
- **位置**：`src/panels/terminal/theme.ts:43`
- **修复**：补 `terminalOptions.vtExtensions?.kittyKeyboard === true` 断言（与 E2E-02 的 L3 主题加载互补）。
- **来源**：12 P-5

#### STS-06 🟡 store debounce 测试 afterEach 未清活跃 timer
- **位置**：`src/__tests__/projects.test.ts:580-668`、`font-size.test.ts:33-35`、`keybindings.test.ts:29-31`
- **修复**：afterEach 统一调 `cancelPendingSave()`（或 `vi.runOnlyPendingTimers()` + `vi.clearAllTimers()`）。
- **来源**：12 P-6

#### STS-07 🟡 projects codify 可疑行为需注释
- **位置**：`src/__tests__/projects.test.ts:179-189`、`358-371`
- **问题**：不存在 pageId 操作仍递增 version 被锁为强契约，阻塞未来优化。
- **修复**：按 D3 保留断言 + 注释"已知当前行为（无影响操作仍 bump version），非强契约"。
- **来源**：12 P-7

#### STS-08 🟢 command-catalog commandFromMeta 仅 5/9
- **位置**：`src/__tests__/command-catalog.test.ts:76-135`
- **修复**：改参数化遍历 EXPECTED_IDS 全 9 条，统一断言 id/context/defaultKey/handler。
- **来源**：12 P-8

#### STS-09 🟢 colors 缺 EXPLORER_SELECTION_BG 等 token
- **位置**：`src/theme/index.ts:23-28`
- **修复**：EXPLORER_SELECTION_BG/HTML_PANEL_LOADING_FG/HTML_PANEL_IFRAME_BG 加入 uiTokenCases（配合 STS-01 真实值断言）。
- **来源**：12 P-9

#### STS-10 🟢 renamePage 不存在 projectId 守卫 + 名实不符改名
- **位置**：`src/stores/projects.ts:175-179`、`src/__tests__/projects.test.ts:339-342`
- **修复**：①补 renamePage 对不存在 projectId 状态不变用例；②"markPersistenceReady 应允许后续 save"补实际 save 断言或改名。
- **来源**：12 P-11、12 P-12（合并）

#### STS-11 🟢 inject-script 性能断言删除 + 同引用快照改深拷贝
- **位置**：`src/__tests__/inject-script.test.ts:222-228`、`src/__tests__/projects.test.ts:173-177,290-300`
- **修复**：①删 `elapsed < 500ms` 时间断言（保留结果断言）；②状态不变断言改 `structuredClone` 快照比对。
- **来源**：12 P-13、12 P-14（合并）

### B8. IHE（8 项）

#### IHE-01 🔴 mockIPC 结构性盲区——文档化 + wrapper 行为契约
- **位置**：`src/__tests__/ipc-contract.test.ts`、`ipc-hooks-contract.test.ts`、`ipc-claude-history-contract.test.ts`
- **问题**：mockIPC 只守 JS 侧形状——camelCase/snake_case 真实转换、Channel 序列化、Uint8Array 处理、listen 回调运行时解包均不在 mock 层验证（后端必填参数缺失时 invoke 必 reject 且被 catch 吞 = 测试全绿运行时静默失败）。
- **修复**：①文件头注释 + ipc/CLAUDE.md 文档化"契约测试只防 wrapper 写错命令名/参数结构，真实序列化由 L4 守卫"（→ DOC-01/02 引用）；②补"wrapper 行为契约"用例（listen 回调解包 event.payload 的模拟驱动断言）。
- **来源**：13 P-1

#### IHE-02 🟡 notification.ts 14.3% 分支未测
- **位置**：`src/ipc/notification.ts:38-47`
- **修复**：新建 `notification.test.ts`：mock plugin-notification 拒绝/异常，验证 sendToastNotification catch 静默、ensureNotificationPermission 拒绝路径。
- **来源**：13 P-2

#### IHE-03 🟡 HTML postMessage 负面用例缺失 + jsdom 局限标注
- **位置**：`src/panels/html/HtmlPanel.tsx:117-156`、`src/__tests__/html-panel.test.tsx`
- **修复**：补负面用例（origin ≠ "null"、source ≠ contentWindow、type ≠ "slterm_key"、未知 fingerprint 均不 dispatch）；用例标注"jsdom 模拟，真实 WebView2 由 L4 验收"（→ DOC-02）。
- **来源**：13 P-6

#### IHE-04 🟡 E2E_ENABLED tree-shake 字面量断言
- **位置**：`src/lib/e2eEnabled.ts`、`src/__tests__/e2e-build-config.test.ts`
- **问题**：`E2E_ENABLED` 若非编译期字面量（被包成函数调用），Rollup 无法 DCE，生产带测试后门且 L2 不报警。
- **修复**：补 AST/正则断言 `E2E_ENABLED` 定义为内联 `import.meta.env` 字面量表达式（不得调用 computeE2eEnabled）。
- **来源**：13 P-11

#### IHE-05 🟡 error-boundary inline variant 未覆盖
- **位置**：`src/lib/ErrorBoundary.tsx:51-62`
- **修复**：补 `variant="inline"` 渲染用例。
- **来源**：13 P-13、12 报告 ErrorBoundary 行（合并）

#### IHE-06 🟢 四 IPC 契约文件参数化
- **位置**：`src/__tests__/ipc-*.test.ts`（contract/hooks-contract/hooks-config-contract/claude-history-contract）
- **修复**：抽 `src/__tests__/helpers/ipc-contract.ts` 工厂（声明式 schema：命令名/参数/返回/异常），四文件重走工厂；四维断言不丢。
- **来源**：13 P-16

#### IHE-07 🟢 ipc/html 边界杂项
- **位置**：`src/__tests__/ipc-ping.test.ts`、`html-panel.test.tsx`、`csp-config.test.ts`、`src/panels/html/HtmlPanel.tsx:140`
- **修复**：①ipc-ping 改调 `src/ipc/index.ts` 导出的 `ping()` wrapper；②注入脚本断言关键控制流（postMessage 字段构造/preventDefault/监听绑定），非仅字符串包含；③复跑确认 HtmlPanel `err instanceof Error` false 分支命中，未中则修用例；④CSP 测试扩展 style-src/connect-src/img-src 关键字段快照。
- **来源**：13 P-4、13 P-7、13 P-8、13 P-9（合并）

#### IHE-08 🟢 html-panel waitFor helper 提取
- **位置**：`src/__tests__/html-panel.test.tsx`
- **修复**：提取 `waitForLoaded`/`waitForError` 局部 helper 消除重复。
- **来源**：13 P-17

### B9. NAH（11 项）

#### NAH-01 🔴 deriveActiveSessionStatuses sessionId 缺失回退未覆盖
- **位置**：`src/features/claudeHistory/historyModel.ts:131`
- **修复**：注册表条目 `claudeSession: { sessionId: null, transcriptPath: "C:/x/abc.jsonl", status: "working" }`，断言 `deriveActiveSessionStatuses().get("abc") === "working"`。
- **来源**：14 #1

#### NAH-02 🔴 TerminalRegistry.setClaudeSession merge 语义未断言
- **位置**：`src/panels/terminal/TerminalRegistry.ts`（setClaudeSession）
- **问题**：`undefined` 字段不覆盖旧值、缺 `lastEventAt` 自动填充、null 清空三语义未锁（F5 双通道建行/三通道删行核心保证）。
- **修复**：先全量 set 再增量 `{ status: "working" }`，断言 transcriptPath 保留 + lastEventAt 更新；null 清空单独断言。
- **来源**：14 #2

#### NAH-03 🔴 useClaudeNotifications classifyEvent 表驱动缺失
- **位置**：`src/features/notifications/useClaudeNotifications.ts:76,131,139,143`
- **修复**：导出 classifyEvent（或拆纯函数），事件 × notificationType 表驱动断言返回类别 + toast 触发与否 + 标题/正文。
- **来源**：14 #3

#### NAH-04 🟡 通知去重缓存 200→100 截断未覆盖
- **位置**：`src/features/notifications/useClaudeNotifications.ts:132-133`
- **修复**：构造 250 个不同事件推进时间，断言缓存截断为 100；最旧事件重新触发应再弹 toast。
- **来源**：14 #4

#### NAH-05 🟡 AgentStatusRow 行 2 未断言
- **位置**：`src/features/agentStatus/AgentStatusRow.tsx:65-66,50`
- **修复**：渲染完整 usage 行，断言 outputTokens 文本与 formatRelativeTime 相对时间出现。
- **来源**：14 #5

#### NAH-06 🟡 AgentStatusView 标题覆盖用 mock history
- **位置**：`src/features/agentStatus/AgentStatusView.tsx:118`
- **修复**：集成测试：真实 useClaudeHistory（或受控数据）含 rename 后 title，断言活跃区行标题被覆盖；无匹配回退原标题。
- **来源**：14 #6

#### NAH-07 🟡 restoreSession 防重入 / cwd null 守卫未进入
- **位置**：`src/features/claudeHistory/restoreSession.ts:34,36`
- **修复**：①同步连调两次断言四步编排仅执行一次；②`cwd: null` 断言抛 "cwd required"。
- **来源**：14 #7

#### NAH-08 🟡 useClaudeHistory.scan generation 竞态未覆盖
- **位置**：`src/features/claudeHistory/useClaudeHistory.ts:35,60`
- **修复**：首次 scan 延迟 resolve + 二次立即 resolve，断言 sessions 来自第二次。
- **来源**：14 #8

#### NAH-09 🟡 HistorySessionList 默认折叠 / 右键回调未完整覆盖
- **位置**：`src/features/claudeHistory/HistorySessionList.tsx`
- **修复**：①断言 expandedGroups 初始为空、点击组标题后含该组 key；②右键触发断言 onCopy/onFork/onDelete 回调参数（fork 标志 true）。
- **来源**：14 #9

#### NAH-10 🟢 HistorySessionRow 图标优先级未覆盖
- **位置**：`src/features/claudeHistory/HistorySessionRow.tsx:50`
- **修复**：`status="working" && orphan=true` 断言渲染 ⚡ 而非 ✗。
- **来源**：14 #10

#### NAH-11 🟢 SessionActionDialog 空 actions 防御未覆盖
- **位置**：`src/features/claudeHistory/SessionActionDialog.tsx:42`
- **修复**：`actions={[]}` 断言不渲染弹窗。
- **来源**：14 #11

---

## C. L3 headless + L4 E2E（15 项）

#### E2E-01 🔴 L3 keyboard ~30 条同义反复降级标注
- **位置**：`test/terminal/keyboard.test.ts:72-317`
- **问题**：`term.input('\x01')` → 断言 onData 收到 `\x01`，等价"输入=输出"（一手证据：xterm `input()` = `triggerDataEvent` 纯透传，`node_modules/@xterm/xterm/src/common/CoreTerminal.ts:183-185`），不经生产 attachCustomKeyEventHandler → ShortcutRegistry 链路。
- **修复**：按 D4 文件头 + describe 标注降级为"xterm.js 基础行为回归（非 slTerminal 键盘链路）"，用例保留；生产键盘链路归 L2 已有覆盖。
- **来源**：15 P-1

#### E2E-02 🔴 L3 未覆盖生产 theme.ts
- **位置**：`src/panels/terminal/theme.ts`（colors/cursorStyle/scrollback/vtExtensions/drawBoldTextInBrightColors）
- **修复**：L3 新增用生产 `terminalOptions` 创建 headless Terminal 的用例：16 色 ANSI 与主题色板一致、`CSI>1u` 可激活 Kitty、scrollback 容量生效、drawBoldTextInBrightColors 亮色映射。
- **来源**：15 P-2

#### E2E-03 🔴 L3 未覆盖生产 OSC 52/133/8 handler
- **位置**：`src/panels/terminal/useClipboardHandler.ts`、`useCommandDetection.ts`、`useXterm.ts`（OSC 8）
- **修复**：headless 触发：①`\x1b]52;c;<base64>\x07` → mock `src/ipc/clipboard` writeText 断言调用 + CJK 解码；②`\x1b]133;C;<cmd>\x1b\\` → onTabStateChange 参数（icon/title）；③`\x1b]8;;<url>\x1b\\` → mock `src/ipc/shell` openUrl。
- **来源**：15 P-3

#### E2E-04 🔴 L3 headless ≠ 生产渲染器——L4 视觉回归 + 定位声明
- **位置**：`test/terminal/`（整体定位）、`src/panels/terminal/webgl.ts`
- **问题**：headless 不跑 WebGL/GPU/onContextLoss，"渲染正确性"代表性有限；detectWebgl/setupWebglWithRetry/onContextLoss 在 L3 不可触发。
- **修复**：①L4 新增真实 WebView2 视觉/功能回归（全屏 TUI 输出后 resize、切页签往返、WebGL→DOM 回退不白屏）——**人工验证点**（截图基线人工确认）；②L3 定位声明"网格状态正确性，非渲染正确性"（→ DOC-02）。
- **来源**：15 P-4

#### E2E-05 🔴 L4 hooks 注入污染 ~/.claude/settings.json 无备份
- **位置**：`e2e-tests/run-wdio.cjs:11-33`（一手证据：FIX-TE-04 仅备份 `~/.slterminal/settings.json`）
- **问题**：`hooks_inject` 真实写 `~/.claude/settings.json` + `~/.slterminal/hooks/`，E2E 异常退出残留 slterm matcher 污染用户配置。
- **修复**：按 D5 扩展备份：启动时备份 `~/.claude/settings.json`（存在时），exit 还原；同时清理 `~/.slterminal/hooks/` 与 `hooks-events/`；三启动路径（node22 直跑/便携/fallback）均覆盖。
- **来源**：15 P-6

#### E2E-06 🔴 L4 新增真实 hook reporter 链路用例
- **位置**：`e2e-tests/test.e2e.ts`（信号文件用例群：1378-1532/1688-1856/3019-3165）
- **问题**：现有用例 Node 侧直接写 `.json` 绕过 `slterm-hook-reporter.js`（stdin 解析、SLTERM_PANEL_ID 路由、C10 恒 exit 0 均无 L4 覆盖）。
- **修复**：按 D5 新增 1 条：真实执行 `node ~/.slterminal/hooks/slterm-hook-reporter.js` 向 stdin 写 JSON（含 SLTERM_PANEL_ID env），断言信号文件产生且被消费（页签 emoji 变化）；另断言非法 JSON 输入脚本 exit 0（C10 守卫，D7）。
- **来源**：15 P-7

#### E2E-07 🟡 L3 断言粒度过粗（文本存在 → 行列精确）
- **位置**：`test/terminal/terminal-serialize.test.ts:83-95,146-174,188-222,297-306`
- **修复**：CUP/reflow/SGR 用例改 `term.buffer.active.getLine(y).translateToString()` 按行断言 + `getCell(x,y).getFgColorMode()` 单元格属性断言。
- **来源**：15 P-8

#### E2E-08 🟡 256 色用例名实不符修正
- **位置**：`test/terminal/ansi-correctness.test.ts:70-81`
- **问题**：注释声称"0-15 优化为基本色"但断言只有 `C256_0`/`C256_15`/`\x1b[0m` 文本存在——优化行为无断言验证（**核验更正**：报告引用的 `toContain('\x1b[38;5;0m')` 实际不存在）。
- **修复**：按 D3 对齐实现：一手证据 `node_modules/@xterm/addon-serialize/src/SerializeAddon.ts:259-262`——palette 0-15 优化为基本 SGR（`30+(c&7)` / `90+(c&7)`），补断言 `\x1b[30m`/`\x1b[97m` 等优化后序列 + 删除/修正误导注释。
- **来源**：15 P-9

#### E2E-09 🟡 test.e2e.ts 3236 行拆分 + setup 提取
- **位置**：`e2e-tests/test.e2e.ts`（全长）、`e2e-tests/helpers.ts`
- **修复**：提取 `withProjectAndTerminal({ hooks?: boolean })` 等共享 setup 到 helpers；按领域拆 spec（terminal/sidebar/agent/history/hooks 等），wdio.conf specs 通配覆盖。
- **来源**：15 P-10

#### E2E-10 🟡 browser.pause(500) 固定等待替换
- **位置**：`e2e-tests/test.e2e.ts:1669,2012,2018,2184,2190`
- **修复**：替换为 `browser.waitUntil` 轮询具体状态（DOM/store/文件 mtime）。
- **来源**：15 P-11

#### E2E-11 🟡 拖拽跨区改名 + 恢复编排标注部分端到端
- **位置**：`e2e-tests/test.e2e.ts:1018-1143`、`3169-3235`
- **修复**：①"拖拽跨区"标题改"侧栏视图跨区移动状态机（R6/R7）"（实际走 store helper）；②恢复编排用例注释标注"部分端到端（断言到 pty.write 命令注入，不含真实进入会话）"（→ DOC-02）。
- **来源**：15 P-12、15 P-13（合并）

#### E2E-12 🟡 L4 Job Object 杀父进程检查子进程残留
- **位置**：`e2e-tests/test.e2e.ts`（新增用例）
- **修复**：新增：spawn 终端（跑持久子进程）→ 强杀 slterminal.exe → 断言子进程无残留（Job Object KILL_ON_JOB_CLOSE 真实验证，PTY-01 的 L4 部分）。
- **来源**：01 P-1（L4 部分）、15 附录

#### E2E-13 🟢 run-wdio 健壮性 + fixture 维护说明
- **位置**：`e2e-tests/run-wdio.cjs:24-33,102-134`、`e2e-tests/fixtures/claude-projects/`
- **修复**：①Node 22 便携版预置 `.temp/node22` 或 CI 固定 Node 22 跳过外网下载；②还原前先 `rmSync(settingsPath, {force:true})` 再 rename/copy，防残留 bak 致还原失败；③fixtures 目录加 README 说明编码目录名/UUID 与 claude_history 排除规则的同步关系。
- **来源**：15 P-14、15 P-15、15 覆盖率缺口 #20（合并）

#### E2E-14 🟡 L3 反向/异常 ANSI 用例缺失
- **位置**：`test/terminal/`（新增）
- **修复**：补非法 ANSI、截断多字节序列、嵌套 OSC、异常 resize（0×0）等负面用例（headless 不崩溃 + 状态可恢复）。
- **来源**：15 覆盖率缺口 #10

#### E2E-15 🟢 L4 WDIO 无重试机制
- **位置**：`e2e-tests/wdio.conf.ts`
- **修复**：配置 spec/用例级重试（mocha retries 或 specFileRetries），单条 flaky 不拖垮整轮。
- **来源**：15 覆盖率缺口 #17

---

## D. 文档同步（4 项，最后 Stage 执行）

#### DOC-01 既定豁免清单文档化
- **范围**：reader_loop 残余不可抽分支（PTY-12 产出）、spawn_conpty_child 纯 Win32 调用部分（PTY-08 产出）、lib.rs `run()`、ActivityBar 拖拽 mock 理想化（SVC-14 产出）、E2E_ENABLED=false 生产分支（IHE-04 互补）、L3 WebGL/mouse tracking（15-#16）、L4 真实 OS 按键、HTML postMessage 真实 WebView2 行为
- **修复**：在 `.claude/test-inventory.md` + 对应模块 CLAUDE.md 统一登记豁免表（项目/豁免原因/当前兜底层级），与 00-summary 5.3 表对齐。
- **来源**：00-summary 5.3、01 P-14、05 P-9、10 P10、13 P-5/P-15、15-#16

#### DOC-02 定位声明（半端到端 / 网格状态 / helper 契约）
- **范围**：①L3 = 网格状态正确性非渲染正确性（E2E-04 产出）；②L4 键盘/拖拽/恢复 = 半端到端/部分端到端（E2E-11 产出）；③L2 jsdom postMessage 模拟（IHE-03 产出）；④term.input 间接验证（TRM 域）；⑤app.test/e2e-create-project = E2E helper 行为契约（13 P-14）；⑥editor.test.tsx 浅层定位（07 G1/G2）
- **修复**：e2e-tests/CLAUDE.md + test/terminal README 或文件头 + test-inventory.md 补定位声明。
- **来源**：13 P-14、15 P-5/P-13、06 #14、07 G1/G2

#### DOC-03 test-inventory.md 全量校正
- **位置**：`.claude/test-inventory.md`
- **修复**：①stale 条目清理（hooks 模块"notification 权限声明"等）；②各 Stage 完成后同步用例数（新增/删除/拆分/改名全量反映）；③登记 DOC-01 豁免表与 DOC-02 定位声明。
- **来源**：03 P-13、各 Stage 用例变更

#### DOC-04 子路径 CLAUDE.md 测试模式章节同步
- **位置**：15 个 claudeMdFiles（pty/git/hooks/notify/fs/claude_history/ipc/panels/workspace/stores/lib/features 各 CLAUDE.md）
- **修复**：测试拆分（GIT-12/SVC-14/E2E-09）、新增测试文件（HKC-08/IHE-02/IHE-06 helper）、测试模式变化（命令层 block_on 模式/EventEmitter trait/ScanRootGuard）、git CLI 最低版本声明（GIT-08 产出）同步到对应模块 CLAUDE.md。
- **来源**：结构性变更项的文档收尾
