# Phase 2 修复清单（checklist）

> 输入：`docs/review-phase2/`（00-汇总 + 01~07 分组报告，2026-08-21）。
> 组织方式：按 Stage 组（S01~S10）排列，级别（P0/P1/P2）仅标注，执行顺序由 stages.md 表达。
> 每项含：ID / 级别 / 来源 / 位置（已实读核验 file:line）/ 现状 / 编号修复步骤（含可照抄代码块）/ 测试同步 / 文档同步 / 验证断言。
> Stage 划分、分工与门禁见 `docs/review-phase2-fix/stages.md`。

## 第 0 节：决策登记（D12~D20，续 review-fix/checklist.md D1~D11）

| 编号 | 决策点 | 结论 |
|------|--------|------|
| D12 | 修复范围 | 全量修复：P0+P1+P2+未闭环 10 项+fmt 基线，去重合并后 **37 项**（含 FE-39 验证项） |
| D13 | TE-12 knip 门禁 | 方案 A：补 `entry`/`ignoreExports`/`ignoreFiles` 至 `npx knip --production` 退出码 0；不窄化 CI 口径 |
| D14 | TE-07 TS7 声明失真 | 主 `typescript` 字段直改 `^7.0.2`，删 `@typescript/native` 别名与 TS6 包装器；执行前 `npm view typescript-eslint` 实查兼容版，不兼容则升级/overrides 统一或暂停 type-aware 规则并 ADR 登记 |
| D15 | SEC-15 shell fallback | 收窄为「两侧 canonicalize 均失败且归一化字符串完全相同」才放行，单侧失败即拒绝；`pty/CLAUDE.md` 登记残余风险；补 L1 拒绝用例。不引入 Win32 文件身份比对。**alias 兼容保持**（Store 版 pwsh 场景两侧指向同一路径、双侧均失败，仍走 fallback 放行） |
| D16 | SEC-04 nonce | 威胁模型登记（HtmlPanel 顶部注释 + `src/panels/CLAUDE.md` 修正失实描述）+ L2 守卫测试锁死 global context 命令集；不加 UI 提示、不移除 nonce |
| D17 | SEC-16 root 竞态 | 后端 `tokio::sync::Mutex` 串行化整个 `set_project_root_impl`（Cargo.toml tokio 补 `"sync"` feature）；前端零改动 |
| D18 | FE-37 store IPC | `setProjectRoot` 调用上提调用方（store 纯状态化）；toast 由 `switchToPageShared` 承担（BE-23 同链修）；不登记豁免 |
| D19 | FE-39 嵌套项目 | 接受「最深前缀」语义；实查测试已固化（`nav-tree-history.test.tsx:302-336`），零代码改动，仅 verify 断言确认存在 |
| D20 | FE-40/FE-41/FE-46 | 三项 P2 均实修（滚动跟随 / 空目录行移除 / ErrorBoundary 重试） |

**编号规则**：未闭环/partial 沿用原 ID（TE-06/07/12/13、SEC-04、BE-10、FE-08/10/31/35）；新发现续编（SEC-15~17、BE-22~25、FE-36~48、TE-14~16、DOC-11~14）。

**核验留痕（计划期已实读全部修复点代码原文）**：FE-39 经实查 `nav-tree-history.test.tsx:302-336` 已含嵌套最深前缀用例（Phase 2 04 报告此项失实）——降为「验证已固化，零改动」。FE-45 实查为 **5 处** catch{}（05 报告列 3 处，projects.ts 有 2 处：:254 与 :275）。

---

## 1. S01 组：基线 fmt + knip P0（TE-16、TE-12、TE-13）

### TE-16（P0 基线）cargo fmt 失败修复
- **来源**：00-汇总 4.3 B-0
- **位置**：`src-tauri/src/pty/shell.rs` 3 处、`src-tauri/src/pty/spawn.rs` 2 处（格式偏差，非逻辑问题）
- **修复步骤**：
  1. 仓库根执行 `cargo fmt --manifest-path src-tauri/Cargo.toml`（仅格式化，零逻辑改动——禁止顺手改任何代码）
  2. 执行 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 确认退出码 0
- **测试**：无新测试；复跑 `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` 确认全绿
- **文档**：无
- **根因留痕**：Stage 收尾 commit 后未全量复跑门禁——execution-plan 进度表强制「每 Stage commit 前跑全门禁」
- **验证**：`cargo fmt --check` 退出码 0

### TE-12（P0）knip --production 零误报
- **来源**：01-依赖升级（not-fixed）；00-汇总 P0#1；D13
- **位置**：`knip.json`（现状仅 11 行：`entry: ["index.html","src/main.tsx","e2e-tests/**/*.ts"]` + `ignoreDependencies` 5 个 @wdio 包）
- **现状**：`npx knip --production` 退出码 1（38 unused files / 146 unused exports）
- **修复步骤**（迭代法，每轮改一处跑一次）：
  1. 跑 `npx knip --production` 收集输出，按类别分组：unused files / unused exports / unused dependencies
  2. **unused files → 入 `entry`**：本项目注册表家族（硬约束 #13）靠 side-effect import 触发注册，触发点文件对 knip 不可见。已知形态（以实际输出为准逐一核对）：`src/features/sideViews/sideViewDefs.ts`、`src/features/cliProfiles/profiles/**/index.ts`、`src/features/fileViewers/` 注册触发点、`src/theme/schemes/` 注册触发点等
  3. **unused exports → 入 `ignoreExports`**：测试专用导出（文件内有「测试专用」注释者），已知：`src/ipc/index.ts` 的 `ping`（FE-35 注释标注）、`src/features/agentHistory/restoreSession.ts` 的 `waitFor`（注释标注「导出为测试专用」）等；逐条核对——**无「测试专用」注释的 unused export 不得入 ignoreExports，须人工判断是否真死代码**（真死代码删除之，并在条目备注）
  4. **unused dependencies → 判断**：真实未用则从 package.json 移除；工具链隐式使用（如 @wdio/* 先例）入 `ignoreDependencies`
  5. 重复 1-4 至 `npx knip --production` **退出码 0**
- **测试**：knip 本身即门禁；无 L2 用例
- **文档**：knip.json 不写注释（JSON 无注释——配置即文档）；若新增大类 ignore，在条目备注原因供 S10 写 ADR
- **验证**：`npx knip --production` 退出码 0（verify 断言退出码，非项数）
- **联动**：S09 FE-35 删除 `terminalTabConfig` 后，若本项为其加了 ignoreExports 条目，S09 负责清理（写入 S09 断言）

### TE-13（P0 连带）CI knip 门禁验证
- **来源**：01-依赖升级（not-fixed，依赖 TE-12）
- **位置**：`.github/workflows/ci.yml:44-46`（`Dead code check (knip)` step 已存在）
- **修复步骤**：TE-12 通过后本项自然闭环——**零改动**，仅人工核对 ci.yml 含 `run: npx knip --production` 行
- **验证**：grep ci.yml 含 `npx knip --production`；本地 knip 退出码 0（CI 环境等效）

## 2. S02 组：依赖升级（TE-06、TE-07、TE-14）

### TE-06（P1）@tauri-apps/plugin-dialog npm 升 2.7.2
- **来源**：01 报告 partial；00-汇总 P1#3
- **位置**：`package.json:39`（现状 `"@tauri-apps/plugin-dialog": "2.7.1"`；cargo 侧已 2.7.2）
- **修复步骤**：
  1. `package.json:39` 改 `"2.7.2"`
  2. `npm install` 刷新 package-lock.json
  3. `npm ls @tauri-apps/plugin-dialog` 确认单版本 2.7.2
- **测试**：`npm test` 全量过（dialog 消费点经 `src/ipc/dialog.ts` re-export，契约测试覆盖）
- **验证**：`npm ls @tauri-apps/plugin-dialog` 输出 2.7.2

### TE-07（P1）TypeScript 主字段直改 ^7.0.2
- **来源**：01 报告 partial；00-汇总 P1#2；D14
- **位置**：`package.json:80`（`"typescript": "npm:@typescript/typescript6@^6.0.2"`）、`:59`（`"@typescript/native": "npm:typescript@^7.0.2"`）、`:81`（`"typescript-eslint": "^8.61.1"`）
- **修复步骤**：
  1. **实查兼容**（禁凭印象）：`npm view typescript-eslint version` 与 `npm view typescript-eslint@<latest> peerDependencies`——确认其支持的 TypeScript 版本范围含 7.x
  2. **确认别名零消费**：grep 全仓 `@typescript/native`（除 package.json/lock 外应零命中——若有引用先评估）
  3. `package.json:80` 改 `"typescript": "^7.0.2"`；删除 `:59` 整行
  4. `npm install` 刷 lock；`npm ls typescript` 确认单实例 7.x（无 typescript6 残留）
  5. 全量门禁：`npx tsc --noEmit` → `npx eslint src/` → `npm test`
  6. **若 eslint 报 TS 版本不兼容**：升级 `typescript-eslint` 至步骤 1 查实的兼容版；仍不行则 `overrides` 钉 typescript-eslint 兼容组合，并在条目记录妥协方案
  7. 结果（含任何妥协）写入 S10 的 ADR 登记（S10-C 负责，本 Stage 在 commit message/进度表留结论）
- **测试**：tsc + eslint + L2 全量即验证
- **验证**：`npm ls typescript` 单版本 7.x；三门禁全过
- **人工验证点**：本 Stage 完成后 `npx tauri build --debug --no-bundle` 构建真实产物，人工冒烟（开终端/编辑器/hooks 面板各一次）

### TE-14（P2）WDIO 工具链同包多版本收敛
- **来源**：01 报告 P2#1
- **位置**：`package-lock.json`（`@wdio/globals` 9.31/9.29.1、`expect-webdriverio` 6/5.7、`webdriverio` 9.30.1/9.30.0）；package.json:67-73 声明为 `^9.30.1`/`^9.31.0`/`^6.0.5`
- **修复步骤**：
  1. `npm ls @wdio/globals expect-webdriverio webdriverio` 记录多版本现状
  2. `npm dedupe` 后重跑步骤 1——收敛则完成
  3. 未收敛：package.json `overrides`（现有 4 项，:85-90）追加 `"@wdio/globals": "^9.31.0"`、`"expect-webdriverio": "^6.0.5"`、`"webdriverio": "^9.30.1"` → `npm install` → 重跑步骤 1 确认单版本
  4. `npm run build:e2e` 构建通过（E2E helper 链路编译验证；全量 wdio 留 S10 终跑/人工）
- **验证**：`npm ls` 三包各单版本；build:e2e 退出码 0

## 3. S03 组：后端安全（SEC-15、SEC-17、BE-22、BE-24、BE-25）

### SEC-15（P1）shell 白名单 paths_match fallback 收窄
- **来源**：02-安全 P1#4；D15
- **位置**：`src-tauri/src/pty/shell.rs:105-122`（`paths_match`）、`:98-104`（函数文档注释）
- **现状**：`if let (Ok(cp), Ok(cr)) = (canonicalize, canonicalize)` 双成功精确比较；**任一侧**失败即落入 fallback 字符串比对（注释声称「安全语义不弱化」——失实）
- **修复步骤**：
  1. `paths_match` 整体改为 match 三臂（改后形态，可照抄）：
     ```rust
     fn paths_match(program: &str, resolved: &str) -> bool {
         match (
             std::fs::canonicalize(program),
             std::fs::canonicalize(resolved),
         ) {
             // 1) canonicalize 双成功 → 精确比较（8.3 短名/`..`/symlink 差异由系统拉平）
             (Ok(cp), Ok(cr)) => {
                 if cfg!(windows) {
                     cp.to_string_lossy().eq_ignore_ascii_case(&cr.to_string_lossy())
                 } else {
                     cp == cr
                 }
             }
             // 2) 双侧均失败（应用执行别名/特殊 ACL——CreateProcess 可运行但普通文件 API
             //    打开失败，os error 1920 场景；alias 两侧指向同一路径）→ 回退归一字符串比较
             (Err(_), Err(_)) => {
                 let a = normalize_for_compare(program);
                 let b = normalize_for_compare(resolved);
                 if cfg!(windows) {
                     a.eq_ignore_ascii_case(&b)
                 } else {
                     a == b
                 }
             }
             // 3) SEC-15：单侧失败即拒绝——字符串比对无法证明文件身份，
             //    reparse point/执行别名组合可绕过，从严
             _ => false,
         }
     }
     ```
  2. 函数文档注释（:98-104）改写：删「安全语义不弱化」失实表述，写「双侧失败才回退字符串比对（残余风险：此时仅剩字符串证据，理论上可构造同名字符串绕过——alias 兼容与风险的权衡，D15 登记）；单侧失败即拒绝」
  3. **测试**：`shell.rs` 现有 31 条测试中 paths_match 相关 5 条逐一核对——若有断言「单侧失败放行」的用例改为断言 false；**新增** `paths_match_single_side_failure_rejected`：一侧为真实存在路径（如 `%SystemRoot%\System32\cmd.exe`）、一侧为不存在路径 → 断言 false
  4. **文档**：`src-tauri/src/pty/CLAUDE.md`「白名单真实路径校验（SEC-01，S02）」段——「canonicalize 失败…时**不拒绝**，回退归一字符串比较」改为「**双侧**失败才回退；单侧失败即拒绝（SEC-15 收窄，残余风险登记：双侧失败仅剩字符串比对，reparse point 组合理论上可绕过——概率极低，D15 接受）」
- **验证**：`cargo test --manifest-path src-tauri/Cargo.toml shell -- --test-threads=1` 全绿；grep shell.rs 无「安全语义不弱化」残留
- **人工验证点**：真实 claude spawn 实测（Win11 本机 + Win10 另一台）——系统 pwsh/PowerShell/cmd 三 shell 均能正常启动

### SEC-17（P2）hooks user 层写入后端审计 + 威胁模型登记
- **来源**：02 报告 P2#3
- **位置**：`src-tauri/src/hooks/claude/config.rs:271-295`（`config_write_sync`）
- **修复步骤**：
  1. `:293`（`let path = resolve_config_path(...)?;`）之后、`:294`（`write_hooks_subtree`）之前插入：
     ```rust
     // SEC-17：user 层写入审计——二次确认仅前端门控（UX 层），后端日志兜底可观测
     if matches!(l, Layer::User) {
         tracing::warn!(target: "audit", "hooks user 层配置写入: {}", path.display());
     }
     ```
  2. **文档**：`src-tauri/src/hooks/CLAUDE.md`「写入语义校验（SEC-05，S17，D9）」段末补：「**威胁模型登记（SEC-17）**：user 层二次确认 = UX 层非安全边界（同进程信任模型，恶意前端代码本可绕过任何后端门控）；后端以 `tracing::warn!(target: "audit")` 记录 user 层写入供事后审计」
  3. **测试豁免登记**：`.claude/test-inventory.md` 豁免清单补一行（审计日志输出，人工可观测，L1 不可测 tracing 副作用）
- **验证**：`cargo test hooks -- --test-threads=1` 全绿（行为不变）；grep config.rs 含 `target: "audit"`

### BE-22（P2）notify_watch 前置校验移 spawn_blocking
- **来源**：03 报告 P2#4
- **位置**：`src-tauri/src/notify/mod.rs:356-363`（`notify_watch` 前置校验块）；`validate_watch_path` 定义 :310-318
- **现状**：async 命令内直接调 `validate_watch_path`（内含 `watch_path.exists()` + `validate_path_within_root`（内含 canonicalize））——同步磁盘 I/O 占 IPC worker
- **修复步骤**（照 :379-391 BE-04 spawn_blocking 先例）：
  1. `:356-363` 块替换为：
     ```rust
     // 路径前置校验（存在性 + 沙箱），短暂持有 project_root 锁取快照；
     // BE-22: 校验本身（exists/canonicalize 磁盘 I/O）移入 spawn_blocking，不占 IPC worker
     let root_snapshot = {
         let root = state
             .project_root
             .read()
             .map_err(|e| AppError::Notify(format!("获取 project_root 锁失败: {e}")))?;
         root.clone()
     };
     let watch_path_for_validate = watch_path.clone();
     match tokio::task::spawn_blocking(move || {
         validate_watch_path(&watch_path_for_validate, &root_snapshot)
     })
     .await
     {
         Ok(inner) => inner?,
         Err(e) => return Err(AppError::TaskJoin(e.to_string())),
     }
     ```
  2. `validate_watch_path` 函数本体不动
- **测试**：`notify_watch` 命令层沙箱三分支用例（`validate_watch_path_nonexistent_rejected`/`outside_root_rejected`/`inside_root_ok`）保持绿即行为不变
- **验证**：`cargo test notify -- --test-threads=1` 全绿

### BE-24（P2）SEC-14 锁中毒分支可观测化
- **来源**：03 报告 P2#6
- **位置**：`src-tauri/src/state.rs:283-288`（`apply_project_root` Err 臂）
- **修复步骤**：
  1. `if let Ok(mut root) = project_root.write() { *root = None; }` 改为：
     ```rust
     // SEC-14: 失败时清空旧 root，防止沙箱继续放行已失效的旧路径
     match project_root.write() {
         Ok(mut root) => *root = None,
         // BE-24：锁中毒时旧 root 无法清空——接受语义偏差但可观测化（登记见 src-tauri/CLAUDE.md）
         Err(lock_err) => {
             tracing::warn!("project_root 写锁中毒，旧 root 未能清空: {lock_err}");
         }
     }
     ```
  2. **文档**：`src-tauri/src/CLAUDE.md`「std Mutex 中毒保持现状」节末补：「BE-24 例外：`apply_project_root` SEC-14 清空旧 root 在写锁中毒时不生效（旧 root 残留至进程退出）——已 `tracing::warn!` 可观测化，接受偏差（中毒本身不可达，见上节）」
- **验证**：`cargo test --manifest-path src-tauri/Cargo.toml state -- --test-threads=1` 全绿

### BE-25（P2）watcher 排除目录大小写不敏感
- **来源**：03 报告 P2#7
- **位置**：`src-tauri/src/notify/mod.rs:199-203`（`is_excluded_path`）
- **修复步骤**：
  1. `.any(|seg| WATCH_EXCLUDE_DIRS.contains(&seg))` 改为：
     ```rust
     // BE-25：大小写不敏感——Windows 文件系统不区分大小写，
     // `Node_Modules`/`Target` 变体同样排除（WATCH_EXCLUDE_DIRS 全小写定义）
     .any(|seg| WATCH_EXCLUDE_DIRS.iter().any(|d| seg.eq_ignore_ascii_case(d)))
     ```
  2. **测试**：`is_excluded_path_matches_all_seven_dirs` 增补大小写变体断言（`Node_Modules`、`TARGET`、`DIST` 命中）；函数头注释「整分量比较」段同步补大小写说明
- **验证**：`cargo test notify -- --test-threads=1` 全绿

## 4. S04 组：root 竞态（SEC-16，独立 Stage）

### SEC-16（P1）set_project_root tokio::Mutex 串行化
- **来源**：03 报告 P1#6；D17
- **位置**：`src-tauri/src/state.rs:132-140`（AppState 定义）、`:148-157`（`AppState::new`）、`:249-273`（`set_project_root` + `set_project_root_impl`）；`src-tauri/Cargo.toml:43`（`tokio = { version = "1", features = ["rt"] }`）
- **修复步骤**：
  1. `Cargo.toml:43` features 改 `["rt", "sync"]`
  2. AppState 加字段（:137 后）：
     ```rust
     /// set_project_root 串行化锁（SEC-16：A→B 快速切换时慢 canonicalize 的 A
     /// 不得后写回覆盖 B——整个 canonicalize+apply 过程互斥）
     pub project_root_lock: tokio::sync::Mutex<()>,
     ```
     `AppState::new` 对应初始化 `project_root_lock: tokio::sync::Mutex::new(()),`
  3. `set_project_root_impl` 签名加 `lock: &tokio::sync::Mutex<()>` 参数；函数体首行 `let _guard = lock.lock().await;`（持锁至函数尾——canonicalize 与 apply 全程互斥）
  4. 命令层 `set_project_root`（:249-251）调用改 `set_project_root_impl(&state.project_root, &state.project_root_lock, path).await`
  5. **测试适配**：state.rs 现有直测 `set_project_root_impl` 的用例——调用点补传 `&tokio::sync::Mutex::new(())`
  6. **新增 L1 用例** `set_project_root_serializes_concurrent_calls`：tokio runtime 下并发 `join!` 两个 impl 调用（两个 tempdir 路径），断言：两调用均 Ok、最终 root 为其中之一且非 None、再顺序调用 B 后 root == B
- **文档**：`src-tauri/src/CLAUDE.md` state.rs 节补一句（AppState 字段清单加 project_root_lock + SEC-16 串行化语义）
- **验证**：`cargo clippy -- -D warnings` + `cargo test state -- --test-threads=1` 全绿
- **人工验证点**：A→B 快速连切页面，旧项目文件操作应被沙箱拒绝（root 不串）

## 5. S05 组：watcher 生命周期前端（BE-10、FE-38）

### BE-10（P1）空页面路径 stopWatch
- **来源**：03 报告 partial；00-汇总 P1#7
- **位置**：`src/workspace/Workspace.tsx:237-258`（SEC-01 effect）；`stopWatch` wrapper 已存在（`src/ipc/notify.ts:18-20`）
- **修复步骤**：
  1. effect 首行 `if (!activePageId) return;`（:239）改为：
     ```ts
     // BE-10：activePageId 置 null（删除末页/移除活跃项目）→ 停掉旧项目 watcher，
     // 防 OS 句柄残留至 LRU 淘汰（两条置 null 链：onDeletePage 删末页、NavTree removeProject）
     if (!activePageId) {
       if (prevRootRef.current) {
         void stopWatch(prevRootRef.current);
         prevRootRef.current = null;
       }
       return;
     }
     ```
  2. **测试**：`workspace-switch-order.test.tsx`（或新用例）——种子活跃项目触发 effect 后，将 activePageId 置 null，断言 `stopWatch` 以旧 rootPath 被调用
- **文档**：`src/workspace/CLAUDE.md`「文件监听跟随项目激活」段补「activePageId 置 null → stopWatch（BE-10）」
- **验证**：L2 对应用例绿

### FE-38（P2）SEC-01 effect await 成功后再 startWatch
- **来源**：03 报告 P2#8
- **位置**：`src/workspace/Workspace.tsx:244-254`（同 BE-10 同一 effect，同一 agent 一并改）
- **现状**：`setProjectRoot(...).catch(...)` 与 `void startWatch(...)` 并排放火——setProjectRoot 未 resolve 即 startWatch（沙箱竞态）
- **修复步骤**：
  1. `:248-253` 改为：
     ```ts
     // FE-38：setProjectRoot 成功后才 startWatch（失败不启动 watcher）；
     // 过期守卫：then 回调时 prevRootRef 已指向其他项目（快速连切）→ 丢弃
     const targetRoot = proj.rootPath;
     setProjectRoot(targetRoot)
       .then(() => {
         if (prevRootRef.current !== targetRoot) return;
         void startWatch(targetRoot);
       })
       .catch((err) => {
         console.error("[slTerminal] 设置项目根路径失败:", err);
         // FE-04（D7）：SEC-01 兜底失败时 toast 告警，不阻断切换
         toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝");
       });
     ```
     （其上一行 `prevRootRef.current = proj.rootPath;` 保持不动；`if (prev) void stopWatch(prev);` 保持不动）
  2. **测试**：断言 setProjectRoot resolve 前 startWatch 未被调用、reject 时 startWatch 不调用且 toast 出现
- **验证**：L2 对应用例绿

## 6. S06 组：store 纯状态 + 页面切换链（FE-37、FE-36、BE-23）

### FE-37（P1）switchToPage 剥离 IPC 上提调用方
- **来源**：04 报告 P1；00-汇总 P1#9；D18
- **位置**：`src/stores/projects.ts:159-186`（`switchToPage`）；`:8`（`setProjectRoot` import）
- **现状**：store action 内 `setProjectRoot(project.rootPath).catch(...)` + toast（违反硬约束 #12）
- **修复步骤**：
  1. 删除 `:163-170` 整块（`// SEC-01: 通知后端当前项目根路径…` 注释 + `if (project.rootPath) { setProjectRoot(...).catch(...) }`），`switchToPage` 变为纯 `set(...)` 状态转换
  2. 检查 `setProjectRoot` import（:8）——本文件其他位置无消费则删除该 import（grep 文件内确认；`toast` import 保留——:113/:239 仍有消费）
  3. **生产调用点零改动确认**：grep 全仓 `.switchToPage(`——生产代码仅 `Workspace.tsx:110-113`（委托 `switchToPageShared`，内含 await setProjectRoot）与 `NavTree.tsx:222-230`（缺省回退 `switchToPageShared`）——两者天然含 setProjectRoot，无需改
  4. **测试适配**：直接调 store `switchToPage` 的 6 个测试文件（`layout-switch.test.ts`、`projects.test.ts`、`workspace-multi-instance.test.tsx` 等）——不再期待 `setProjectRoot` 被调用；若有断言「切换后 setProjectRoot 调用」的用例，改为经 `switchToPageShared` 驱动或删除该断言（切换链 setProjectRoot 覆盖由 pageapis.test.ts 承担）
  5. **文档**：`src/stores/CLAUDE.md` projects.ts 节补「switchToPage 为纯状态转换（FE-37：setProjectRoot 已上提调用方 switchToPageShared，约束 #12 合规）」
- **验证**：`npm test` 全绿；grep `src/stores/` 无 `setProjectRoot` 命中
- **人工验证点**：页面切换后文件树/终端 cwd 正常（setProjectRoot 链路经 switchToPageShared 无回归）

### FE-36（P1）MAX_PAGES 全局总数校验
- **来源**：04 报告 P1#8；00-汇总 P1#8
- **位置**：`src/stores/projects.ts:107-115`（`addPage`）
- **修复步骤**：
  1. `:112` `if (project.pages.length >= MAX_PAGES)` 改为：
     ```ts
     // FE-36（D1 契约名实相符）：页面总数上限 = 跨项目全局计数
     // （原按项目计数——多项目下 Dockview 实例仍可无界增长）
     const totalPages = Object.values(get().projects).flatMap((p) => p.pages).length;
     if (totalPages >= MAX_PAGES) {
     ```
     `:108-109` 注释（FE-01 契约说明）同步补「FE-36 全局化」一句
  2. **测试**：`projects.test.ts:356-387` MAX_PAGES 用例适配（按项目构造 → 按全局构造）；新增跨项目用例：项目 A 15 页 + 项目 B 5 页时，B addPage 拒绝 + toast
  3. **文档**：`src/stores/CLAUDE.md`「页面总数上限（FE-01）」段补「跨项目全局计数（FE-36）」；ADR-0009 语义修订由 S10 统一登记
- **验证**：`npm test` 全绿

### BE-23（P2）switchToPageShared setProjectRoot 失败 toast
- **来源**：03 报告 P2#5
- **位置**：`src/workspace/pageApis.ts:49-73`（`switchToPageShared` catch 块 :60-62）
- **修复步骤**：
  1. catch 块补 toast（文案照 Workspace.tsx:251 既有先例）：
     ```ts
     } catch (err) {
       console.error("[slTerminal] 设置项目根路径失败:", err);
       // BE-23：与 FE-04 三处一致——失败 toast 可感知（原仅 console.error）
       toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝");
     }
     ```
  2. 文件头 import 区加 `import { toast } from "../lib";`
  3. **测试**：`pageapis.test.ts` 增用例——`setProjectRoot` mock reject → 断言 toast.show 以 warning 调用且切换仍完成
- **验证**：`npm test` 全绿

## 7. S07 组：错误处理一致性 + nonce 威胁模型（SEC-04、FE-08、FE-10、FE-42、FE-43、FE-44、FE-45）

### SEC-04（P1）nonce 威胁模型登记 + global 命令集守卫
- **来源**：02 报告 partial；00-汇总 P1#5；D16
- **位置**：`src/panels/html/HtmlPanel.tsx:56-64`（buildInjectedScript 文档注释）、`:132-134`（handleMessage nonce 注释——「拿不到本面板 nonce」失实）；`src/features/shortcuts/commandCatalog.ts:27-34`（global context 仅 `global.closeTab`）
- **修复步骤**：
  1. HtmlPanel.tsx `buildInjectedScript` 文档注释（:56-64）追加威胁模型段：
     ```
     * 【SEC-04 威胁模型（D16 登记）】nonce 明文内联于 srcDoc——iframe 内任意脚本可读取
     * 文档中的注入脚本提取 nonce 并伪造 slterm_key 消息。nonce 仅防「不知密钥的外部伪造」，
     * 不防被预览 HTML 自身。真正防线 = global context 命令集最小化（当前仅 global.closeTab
     * 关页签，低风险）——守卫测试 command-catalog.test.ts 锁死该集合，扩充 global 命令
     * 必须先评估本威胁模型。
     ```
  2. `:132-134` 注释修正：删「拿不到本面板 nonce，伪造消息在此被拦截」失实表述，改「iframe 内脚本可提取 nonce 伪造（见上方威胁模型）——nonce 拦截外部伪造，内部伪造由 global 命令集最小化兜底」
  3. `src/panels/CLAUDE.md`「postMessage origin 校验与威胁模型（SEC-03/SEC-04）」节：「无 nonce 校验时」段与 nonce 行「防御目标」列中的失实描述（「攻击者 HTML 无法读取 buildInjectedScript 产出」）修正为明文可读的准确表述 + 补「防线分层：nonce（外部）+ global 命令集最小化（内部，守卫测试锁死）」结论
  4. **守卫测试**：`src/__tests__/command-catalog.test.ts` 追加：
     ```ts
     // SEC-04（D16）：global context 命令集守卫——iframe 内脚本可提取 nonce 伪造
     // 全局快捷键消息（HtmlPanel 威胁模型），global 命令集必须保持最小低风险；
     // 扩充即红，迫使先评估威胁模型
     it("global context 命令集恒为 [global.closeTab]", () => {
       const globals = COMMAND_CATALOG.filter((m) => m.context === "global").map((m) => m.id);
       expect(globals).toEqual(["global.closeTab"]);
     });
     ```
- **验证**：`npm test` 全绿；grep HtmlPanel.tsx 无「拿不到本面板 nonce」残留

### FE-08（P2）终端粘贴 readText 失败可观测
- **来源**：05 报告 partial + N1；00-汇总 P2#25
- **位置**：`src/panels/terminal/keyboard.ts:35-38`
- **修复步骤**：`.catch(() => {});` 改为（照 :25-27 copy 分支 console.error 先例）：
  ```ts
  .catch((err) => {
    console.error("[slTerminal] 读取剪贴板失败:", err);
  });
  ```
- **测试**：`keyboard.test.ts` 视既有覆盖——粘贴失败路径断言 console.error（无则补一条 mock readText reject 用例）
- **验证**：grep keyboard.ts 无 `.catch(() => {})` 残留；`npm test` 全绿

### FE-10（P2）Diff 右栏外部修改重载失败提示条
- **来源**：05 报告 partial + N3；00-汇总 P2#26
- **位置**：`src/panels/diff/DiffPanel.tsx:478-491`（两处 catch 仅 console.warn）；`:655-677`（diffStale 提示条已存在）
- **修复步骤**：两处 catch（:483、:490）各补 `setDiffStale(true);`（保留 console.warn 一行）：
  ```ts
  }).catch((err) => {
    console.warn("[slTerminal] 外部修改重载失败:", err);
    // FE-10：复用 diffStale 提示条——重载失败内容可能过时，用户可感知
    setDiffStale(true);
  });
  ```
- **测试**：`diff-panel-stale-banner.test.tsx` 增用例——外部修改重载 readFile reject → `data-testid="diff-stale-banner"` 出现
- **验证**：`npm test` 全绿

### FE-42（P2）关窗监听 cleanup 静默吞错
- **来源**：05 报告 N2；00-汇总 P2#12
- **位置**：`src/ipc/window.ts:55-58`
- **修复步骤**：`.catch(() => {});` 改为 `.catch((err) => { console.warn("[slTerminal] 取消关窗监听失败:", err); });`；:56 注释「兜底吞掉」改「兜底记录」
- **验证**：grep window.ts 无 `.catch(() => {})` 残留；`npm test` 全绿

### FE-43（P2）DiffPanel 保存失败 toast 统一 getErrorMessage
- **来源**：05 报告 N4；00-汇总 P2#13
- **位置**：`src/panels/diff/DiffPanel.tsx:372-376`（`:50` 已 import getErrorMessage）
- **修复步骤**：`toast.show("error", \`保存失败: ${err}\`)` → `toast.show("error", \`保存失败: ${getErrorMessage(err)}\`)`
- **测试**：`diff-panel.test.tsx` 保存失败用例断言文案为解析后消息
- **验证**：`npm test` 全绿

### FE-44（P2）编辑器保存失败 toast 统一 getErrorMessage
- **来源**：05 报告 N5；00-汇总 P2#14
- **位置**：`src/panels/editor/useCodeMirror.ts:178-181`（`:33` 已 import getErrorMessage）
- **修复步骤**：同 FE-43 改法
- **测试**：`use-code-mirror.test.ts` 保存失败用例断言文案
- **验证**：`npm test` 全绿

### FE-45（P2）stores loadFromDisk catch{} 空块补日志（5 处）
- **来源**：05 报告 N6；00-汇总 P2#15；**实查补正：projects.ts 有 2 处（:254、:275），共 5 处**
- **位置**：`src/stores/fontSize.ts:64`、`keybindings.ts:71`、`sideBar.ts:122`、`projects.ts:254`、`projects.ts:275`
- **修复步骤**：5 处 `} catch {` 统一改为（保留原注释行）：
  ```ts
  } catch (err) {
    // 首次启动或 IPC 失败，保持默认值（原注释保留）
    console.warn("[slTerminal] <store名> loadFromDisk 失败:", err);
  }
  ```
  `<store名>` 分别 = `fontSize`/`keybindings`/`sideBar`/`projects`/`loadAllProjects`
- **测试**：`startup-store-fail-warn.test.tsx` 既有告警路径用例核对；必要时补 console.warn 断言
- **验证**：grep `src/stores/` 无 `} catch {` 空块残留；`npm test` 全绿

## 8. S08 组：explorer/navTree 增强（FE-39 验证、FE-40、FE-41）

### FE-39（P2）projectIdForCwd 最深前缀语义固化 —— **验证项，零改动**
- **来源**：04 报告 P2#9；D19；**实查补正：`nav-tree-history.test.tsx:302-336` 已含嵌套最深前缀用例（「cwd 命中嵌套 rootPath → 归属最深前缀项目（根项目计数 0）」），Phase 2 04 报告此项失实**
- **修复步骤**：零代码改动——仅 verify 断言该 describe/用例存在且绿
- **验证**：grep `nav-tree-history.test.tsx` 含「最深前缀」；`npx vitest run nav-tree-history` 全绿

### FE-40（P2）FileTree 虚拟化选中滚动跟随
- **来源**：04 报告 P2#10；D20
- **位置**：`src/features/explorer/FileTree.tsx`（props `selectedPath: string | null` :164；rows 扁平化 :599-602；scrollRef :606；start/end :636-646；ROW_HEIGHT=24 :56；FlatRow `{ key, node, depth, kind, parentPath? }` :277-288——节点行 `kind === "node"` 且 `row.node.entry.path`）
- **修复步骤**：
  1. `visibleRows` 计算（:646）之后插入：
     ```tsx
     // FE-40：程序式选中视口外行时滚动跟随——selectedPath 变化且对应行不在
     // [start, end] 窗口内 → scrollTop 定位使该行可见（虚拟化常见缺口补齐；
     // 鼠标点击天然落在可见区，本 effect 服务 explorer.open 等程序式选中路径）
     useLayoutEffect(() => {
       if (!selectedPath) return;
       const index = rows.findIndex(
         (row) => row.kind === "node" && row.node?.entry.path === selectedPath,
       );
       if (index < 0 || (index >= start && index < end)) return;
       const el = scrollRef.current;
       if (el) el.scrollTop = index * ROW_HEIGHT;
     }, [selectedPath, rows, start, end]);
     ```
  2. **测试**：`explorer-virtualization.test.tsx` 增用例——构造大行数树 + mock 容器高度（jsdom clientHeight=0 退化为全量渲染，需按现有「高度测得」用例模式 mock）→ 程序式设视口外 selectedPath → 断言 scrollRef scrollTop 被设置
- **验证**：`npx vitest run explorer-virtualization` 全绿

### FE-41（P2）refreshSubtreeAt 目标已删空目录行移除
- **来源**：04 报告 P2#11；D20
- **位置**：`src/features/explorer/useFileTree.ts:230-298`（`refreshSubtreeAt`）；`loadDirectory` :56-86（失败记 dirErrors 返回 []）
- **修复步骤**：
  1. `loadDirectory` 不动；`refreshSubtreeAt` 中 `:258` `const fresh = await loadDirectory(targetPath);` 改为自行 try/catch 直调 readDir 链路，以区分「目标已删除」与「空目录」：
     ```ts
     // FE-41：直调 readDir 以区分「目标目录已删除」（readDir 抛错）与「空目录」（返回 []）——
     // loadDirectory 容错返回 [] 无法区分两者；目标已删时须从父层移除该目录行
     let fresh: TreeNode[];
     let targetMissing = false;
     try {
       const entries = await readDir(targetPath);
       fresh = entries.map((entry) => ({ entry, expanded: false, children: [], loading: false }));
     } catch {
       targetMissing = true;
       fresh = [];
     }
     if (targetMissing && targetPath !== rp) {
       // 目标目录本身已被删除 → 从父层 children 移除该目录行（不留空目录行残留）
       setRootNodes((prev) => {
         const removeNode = (nodes: TreeNode[]): TreeNode[] =>
           nodes
             .filter((n) => n.entry.path !== targetPath)
             .map((n) =>
               n.expanded && n.children.length > 0
                 ? { ...n, children: removeNode(n.children) }
                 : n,
             );
         return removeNode(prev);
       });
       return true;
     }
     ```
     （`readDir` import 已存在于文件头——loadDirectory 在用；TreeNode 映射形态照 :67-72）
  2. 目标已删且 `targetPath === rp`（根被删）→ 走原 mergeLayer 空合并路径（根行保留，现状语义不动）
  3. **测试**：`use-file-tree.test.ts` 增用例——vfs 中删除目标目录后触发 refreshSubtreeAt → 断言该目录行从树中消失
- **验证**：`npx vitest run use-file-tree` 全绿

## 9. S09 组：稳定性与死代码（FE-35、FE-46、FE-47、FE-48）

### FE-35（P2）terminalTabConfig 死代码删除
- **来源**：06 报告 partial + P2#16
- **位置**：`src/panelRegistry.ts:72-75`（`terminalTabConfig` 常量）；`src/__tests__/panel-registry.test.ts:6`（import）、`:63-75`（describe 块 3 断言）
- **修复步骤**：
  1. 删 panelRegistry.ts:72-75（含 `:72` 注释行）
  2. panel-registry.test.ts：删 :6 import 中 `terminalTabConfig,`、删 :63-75 整个 describe 块（29 用例 → 26）
  3. **knip 联动**：检查 knip.json——若 TE-12 为 terminalTabConfig 加了 ignoreExports 条目，一并删除
- **验证**：grep 全仓 `terminalTabConfig` 零命中；`npm test` 全绿；`npx knip --production` 仍 0

### FE-46（P2）ErrorBoundary inline 重试按钮
- **来源**：06 报告 P2#17；D20
- **位置**：`src/lib/ErrorBoundary.tsx:51-115`（inline variant）
- **修复步骤**：
  1. 「查看错误详情」`details`（:86）之前插入：
     ```tsx
     <button
       onClick={() => this.setState({ error: null })}
       style={{
         marginTop: 4,
         padding: "4px 16px",
         background: SECONDARY_BG,
         color: PLACEHOLDER_FG,
         border: `1px solid ${SEPARATOR_BG}`,
         borderRadius: 4,
         cursor: "pointer",
         fontSize: 12,
         fontFamily: "inherit",
       }}
     >
       重试
     </button>
     ```
     （token 复用文件已有 import——SECONDARY_BG/SEPARATOR_BG/PLACEHOLDER_FG :11-12；禁硬编码色值，约束 #6）
  2. **测试**：`error-boundary.test.tsx` 增用例——抛错面板渲染占位 → 点击「重试」→ 断言子树重新渲染（子组件渲染计数 +1；若仍抛错则再次落占位，用「首次抛错二次正常」桩验证恢复路径）
- **验证**：`npx vitest run error-boundary` 全绿

### FE-47（P2）关窗 ptyKillAll 包总超时
- **来源**：06 报告 P2#18
- **位置**：`src/App.tsx:156-163`（SHUTDOWN_TIMEOUT_MS 既有常量，:140-143 有同形 race 先例）
- **修复步骤**：`:157` `const killed = await pty.ptyKillAll();` 改为：
  ```ts
  // FE-47：ptyKillAll 包总超时——后端逐 session 3s 串行 kill+join，
  // 极端多 session 场景防拖长关窗（与上方 Registry kill 同形 race）
  const killed = await Promise.race([
    pty.ptyKillAll(),
    new Promise<null>((resolve) =>
      setTimeout(() => resolve(null), SHUTDOWN_TIMEOUT_MS),
    ),
  ]);
  if (killed !== null && killed > 0) {
  ```
  （`:158-160` 的 `if (killed > 0)` 分支条件照此调整，其后逻辑不动）
- **测试**：`close-handler.test.ts` 适配/增用例——ptyKillAll 永不 resolve 时关窗流程在超时后继续
- **验证**：`npm test` 全绿
- **人工验证点**：多 session 场景关窗实测（总时长有界）

### FE-48（P2）waitFor 轮询 setTimeout abort 清理（2 处）
- **来源**：06 报告 P2#19
- **位置**：`src/workspace/pageApis.ts:98`（`switchToPageAndFocus` 轮询）；`src/features/agentHistory/restoreSession.ts:48`（`waitFor`，间隔常量 `POLL_INTERVAL_MS`）
- **修复步骤**（两处同形）：
  ```ts
  await new Promise<void>((resolve) => {
    const timer = setTimeout(resolve, 100); // restoreSession 用 POLL_INTERVAL_MS
    // FE-48：abort 感知——立即 clearTimeout + resolve，不等下一 tick
    signal?.addEventListener(
      "abort",
      () => { clearTimeout(timer); resolve(); },
      { once: true },
    );
  });
  ```
  循环顶部已有 `signal?.aborted` 检查（pageApis:92 静默 return / restoreSession:43 throw 已取消）——abort 后 resolve 落入下轮迭代顶部退出，行为正确，无需再加分支
- **测试**：`pageapis.test.ts` / `agent-history-restore.test.ts` 既有 abort 用例适配——增断言「abort 后轮询 Promise 在 <100ms 内 settle」（fake timers 下 advance 0 即完成）
- **验证**：`npm test` 全绿

## 10. S10 组：文档同步（FE-31、DOC-11、DOC-12、DOC-13、DOC-14、TE-15，固定最后 Stage）

### FE-31（P2）新建 src/panels/editor/CLAUDE.md
- **来源**：07 报告 M1（partial 残留）
- **位置**：`src/panels/editor/`（新建）；`src/panels/CLAUDE.md`（「编辑器：大文件不虚拟化」等相关节）；`.claude/adr.md` ADR-0009
- **修复步骤**：
  1. 新建 `src/panels/editor/CLAUDE.md`（模板：职责 → 架构决策 → 文件表 → 测试模式）：迁入 panels/CLAUDE.md 的编辑器专属节（大文件不虚拟化 FE-31/D3 决策全文、Compartment 语言/换行、滚动委托 .cm-scroller、Ctrl+S 注册表、CM6 主题层叠、useCodeMirror/gitGutter/EditorPanel/keyboard/activeEditor 文件表、编辑器测试模式表）
  2. `src/panels/CLAUDE.md` 被迁走节改一行交叉引用（`详见 @editor/CLAUDE.md`），面板通用决策保留
  3. `.claude/adr.md` ADR-0009 FE-31 行登记点链接确认指向新文件（S10-C 一并）
- **验证**：Glob `src/panels/editor/CLAUDE.md` 存在；panels/CLAUDE.md 无大段编辑器细节残留

### DOC-11（P2）ipc/CLAUDE.md 删 setFocus + 用例数 9
- **来源**：07 报告 M2；06 组去重合并
- **位置**：`src/ipc/CLAUDE.md:24`（window.ts 行「七个 wrapper…`setFocus`（预留，当前无消费方）…」）、测试模式节（`ipc-window-contract.test.ts（10 用例，七个 wrapper…）`）
- **修复步骤**：window.ts 行「七个 wrapper」→「六个 wrapper」，删 `setFocus` 描述；测试节 `（10 用例，七个 wrapper…含 setFocus（预留）…）` → `（9 用例，六个 wrapper…）`
- **验证**：grep ipc/CLAUDE.md 无 `setFocus` 残留、含「9 用例」

### DOC-12（P2）ipc-agent-history-contract 用例数 14→18
- **来源**：07 报告 M3
- **位置**：`src/ipc/CLAUDE.md` 测试模式节（`ipc-agent-history-contract.test.ts（14 用例…）`）；`src/__tests__/ipc-agent-history-contract.test.ts` 文件头注释（14 条）
- **修复步骤**：两处 14 → 18（真值源 `.claude/test-inventory.md:100`）
- **验证**：grep 两处均为 18

### DOC-13（P2）panels/CLAUDE.md 4 处终端用例数校正
- **来源**：07 报告 M4
- **位置**：`src/panels/CLAUDE.md` 测试模式节
- **修复步骤**（以 test-inventory 为真值源）：`detect-webgl.test.ts` 3→4、`terminal-instance.test.ts` 7→6、`use-xterm-lifecycle.test.ts` 80→86、`terminal.test.tsx` 19→27
- **验证**：grep 四处数字与 test-inventory 一致

### DOC-14（P2）sideViews/CLAUDE.md 用例数统一
- **来源**：07 报告 M5
- **位置**：`src/features/sideViews/CLAUDE.md` 测试模式节
- **修复步骤**：`sideBarState.test.ts` 53→54、`activityBar.test.tsx` 38→40（按 test-inventory:226-227）
- **验证**：grep 两处数字与 test-inventory 一致

### TE-15（P2）json-schema-library 双 major 债务登记
- **来源**：01 报告 P2#2
- **位置**：`package-lock.json`（9.x 由 codemirror-json-schema 锁定 / 11.x 主声明）；代码零改动
- **修复步骤**：
  1. `.claude/adr.md` 已知债务节登记：「json-schema-library 9.x/11.x 双 major 并存——codemirror-json-schema@0.8.1 锁 9.x（上游约束），主声明 11.6.2；运行时两实例并存无冲突（JSON Schema 校验各自独立），待上游升级消解（TE-15）」
  2. `src/features/hooksConfig/CLAUDE.md` schema 节补一句同义引用
- **验证**：grep adr.md 含「json-schema-library」

---

## 11. 去重留痕

| 报告编号 | 归并入 | 说明 |
|----------|--------|------|
| 00-汇总 P2#16 | FE-35 | terminalTabConfig 死代码（partial 残留与新发现同一问题） |
| 00-汇总 P2#20 | FE-31 | editor/CLAUDE.md 缺失（partial 残留与 07-M1 同一问题） |
| 00-汇总 P2#25 | FE-08 | 终端粘贴静默（partial 残留与 05-N1 同一问题） |
| 00-汇总 P2#26 | FE-10 | Diff 右栏重载失败（partial 残留与 05-N3 同一问题） |
| 01 报告 TE-13 | TE-12 | CI knip 门禁依赖 TE-12 联动闭环（验证项） |
| 00-汇总 4.3 B-0 | TE-16 | 基线 fmt 失败编号 |
| 07-M1 | FE-31 | 同上 |
| 07-M2 | DOC-11 | 06 组「setFocus 文档残留」与 07-M2 同题合并 |
| 07-M3 | DOC-12 | ipc-agent-history-contract 用例数 |
| 07-M4 | DOC-13 | panels/CLAUDE.md 4 处数字 |
| 07-M5 | DOC-14 | sideViews/CLAUDE.md 数字 |
| 04 报告 P2#9 | FE-39 | 实查已固化（报告失实），降为验证项 |

**总计 37 项**：S01×3、S02×3、S03×5、S04×1、S05×2、S06×3、S07×7、S08×3（含 1 验证项）、S09×4、S10×6。
