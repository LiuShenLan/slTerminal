# 章二「后端架构与平台」修复清单(CP-004/005/006/007/008/009/010/011/034)

真值源:`docs/compromises.md` 章二(第 32-51 行)。本清单每条均经现状代码原文实读核对(2026-09-06),漂移点见末尾「起草附注」。

---

## CP-004 · 多 Dockview 实例 + MAX_PAGES=20——共享宿主 + 页组分组模型 [Stage S11,独立]

1. **位置**:
   - `src/stores/projects.ts:14-15`——`export const MAX_PAGES = 20;`(FE-01/D1 契约)
   - `src/stores/projects.ts:61-62`——`addPage` 超限拒绝返回 false 契约
   - `src/workspace/CLAUDE.md:11-17`——「多 Dockview 实例(H6)」节,**:15 笔误**「豁免登记见 ADR-0001 配套豁免表」(实为 ADR-0009,adr.md:202 FE-01 行)
   - `src/workspace/Workspace.tsx:72,99-113,146,152,229-230`——`initializedPages` Set + `ensurePageInitialized` 惰性初始化
   - `src/workspace/PageDockviewHost.tsx:721-735`——`display: visible ? "block" : "none"` 实例级 CSS 显隐 + `<DockviewReact>`
   - `src/workspace/layoutSerde.ts:10-12,77-105`——`saveLayout`/`loadLayout`(toJSON/fromJSON 单点,硬约束 #7)
   - `src/panelRegistry.ts:54-76`——`panelRegistry` 面板组件注册表
   - e2e:`e2e-tests/helpers.ts:260-301`(`__slterm_e2e_resetProjects`/`__slterm_e2e_addPage`/`__slterm_e2e_switchToPage`)、`e2e-tests/wdio.conf.ts:72-79`(beforeSuite 双 reset)、`e2e-tests/CLAUDE.md:58`(resetProjects 防 `MAX_PAGES=20` 触发的登记)
2. **现状**:
   - projects.ts:14 注释:「页面总数上限(FE-01/D1 契约)——多 Dockview 实例架构每页一实例,上限防内存/DOM 线性增长」;stores/CLAUDE.md FE-36 节:上限为跨项目全局计数(`Object.values(projects).flatMap(p => p.pages).length`)。
   - workspace/CLAUDE.md:13:「每个操作页面拥有独立 `<DockviewReact>` 实例。页面切换通过 CSS `display:none/block`,终端不销毁。根因:xterm.js 不支持二次 `open()`(Issue #4978)」。
   - workspace/CLAUDE.md:15 原文:「**FE-01 豁免登记**:多实例架构不变(H6 + xterm #4978 是硬约束),仅加**页面总数上限 `MAX_PAGES = 20`**……豁免登记见 ADR-0001 配套豁免表。」——ADR-0001 是 CSP 档,FE-01 实登记于 ADR-0009(adr.md:202),笔误实存。
3. **修复步骤**(本项为最大架构项,给到模块/契约级设计;执行 agent 不得另起方向):
   0. **笔误先行**(独立小步):workspace/CLAUDE.md:15 `ADR-0001 配套豁免表` → `ADR-0009 配套豁免表`。
   1. **共享宿主结构**:Workspace 渲染**单一** `<DockviewReact>` 共享宿主;删除 `Workspace.tsx:72` 的 `initializedPages` state 与 `ensurePageInitialized`(`:99-113`)——多实例 map 消亡,惰性初始化语义随实例消亡;`PageDockviewHost.tsx` 从「每页一实例组件」改造为「页组渲染组件」或删除,`window.__dockviewApi` 重指不变量(workspace/CLAUDE.md:97-99 三允许位置收敛为「宿主唯一,指向即宿主」)。
   2. **页组分组模型**:每操作页面 = 宿主内一个顶级 branch group,`groupId = page-${pageId}` 由纯函数产出:
      ```ts
      // src/workspace/pageGroups.ts(新)
      export const pageGroupId = (pageId: string): string => `page-${pageId}`;
      export const panelIdInPage = (pageId: string, localId: string): string => `${pageId}:${localId}`;
      export const pageOfPanelId = (panelId: string): string | null => panelId.includes(":") ? panelId.slice(0, panelId.indexOf(":")) : null;
      export function panelsOfPage(api: DockviewApi, pageId: string): IDockviewPanel[];
      ```
      **panelId 页前缀协议**(全仓承重契约,执行 agent 逐点核改):panelId 从「nextPanelId() 裸值」改为「页前缀 + localId」;冲击面 = titleManager 终端编号(`terminal-N`,每页从 0 起计数契约不变——编号仍 local)、tabClose 守卫 settings- 前缀判据(workspace/CLAUDE.md:39,判据不变,dirtyRegistry 键同 params.panelId)、TerminalRegistry 键、SEC-08 `PtySession.panel_id` 归属校验(存全量前缀 id,校验逻辑零改)、DefaultTab params、文件型面板 findExistingEditor 查重键。
      跨页组拖拽禁止:宿主 `onDidAddPanel` 校验「panel 页前缀 == 目标 group 页前缀」,越界即回迁原页组(纯函数 `panelBelongsToGroup(panelId, groupId)`,L2 锁死)。
   3. **面板生命周期契约**(写死):
      - 新增面板:addPanel 时 options.group 显式指定目标页组;
      - 页面切换:显隐从「实例级 display」改为「页组容器 display」——每页组 DOM 外包容器 div,切页只切页容器 `display:none/block`(现状心智平移);**终端面板不随切页卸载/重建,xterm 实例只 open 一次(#4978 约束不变)**;`renderer="always"` 白名单(panelRegistry.ts:111-117)语义不变——恒挂载面板在隐藏页组内保持挂载,fit/resize 仅在页组可见时执行(visible effect 单点);
      - 页面删除:先 kill 页组内全部终端面板,再移除空页组;
      - 布局事件:`onDidLayoutChange` 恢复守卫(`restoreGuardRef`)语义不变,单宿主下全页组变更统一写回。
   4. **layoutSerde 契约演进**(硬约束 #7 单点不破):
      - `saveLayout(api)` 不变(单宿主全量 toJSON),存储形态从「每页一份 SerializedDockview」改为「单宿主 JSON 一份 + 页组 id 索引」;projects store `OperationPage.layout` 字段语义改为「该页在宿主内的页组子树切片」;
      - `loadLayout` 增页组提取:`loadPageGroup(api, pageId, saved)`——从宿主 JSON 提取 `page-${pageId}` 子树 fromJSON(`reuseExistingPanels: true` 语义不变),白名单过滤与 patchLegacyLayout 照旧;
      - `patchLegacyLayout` 增迁移:识别旧多实例格式(每页独立 layout.grid 无 page- 前缀组)→ 包成页组子树合入宿主;无法归组的面板丢弃 + console.error(不阻断启动,Watermark 接管空页语义不变)。
   5. **MAX_PAGES 消亡**:删 projects.ts:14-15 常量与 `addPage` 上限判定(含 toast「页面数已达上限」调用);上限消亡理由写进 stores/CLAUDE.md(实例数不再随页线性增长,容器/渲染管线共享)。
   6. **e2e 适配**:wdio.conf.ts:72-79 beforeSuite 双 reset 保留(隔离语义不变),e2e-tests/CLAUDE.md:58「防止跨 spec 累积触发 `MAX_PAGES=20` 上限」句改为「跨 spec 状态隔离」;helpers.ts `__slterm_e2e_addPage`/`__slterm_e2e_switchToPage`/`__slterm_e2e_getProjectIdForPage` 若经 pageId→api map 取数,改为 getPageApi(pageId) 页组查询(协议骨架随步骤 2)。
4. **测试同步**:
   - 改:L2 workspace 系全量——「多实例各自存活」「CSS 显隐」「initializedPages 惰性初始化」用例(workspace/CLAUDE.md:130 登记)改为页组语义(页组存活/页组显隐/未初始化页无 DOM);stores/projects 上限用例(FE-36 跨项目计数、超限 false + toast)删除并替换为「addPage 无上限」用例;layoutSerde 旧格式用例保留 + 新增迁移用例(见下);
   - 加:`pageGroups.ts` 纯函数用例(`panelIdInPage`/`pageOfPanelId` 往返、`panelBelongsToGroup` 越界判定)、`patchLegacyLayout` 旧多实例→页组迁移用例(归组成功/脏面板丢弃两分支)、跨页拖拽回迁用例;
   - 既有用例适配逐一点名:`src/__tests__/` 下 workspace-*.test.tsx(多实例/显隐/初始化)、projects-store 测试、layoutSerde 测试、settings 面板跨页单例用例(openSettingsPanel 同页单例契约,F11 语义不变);L4:terminal.e2e.ts H6 跨页存活用例、settings.e2e.ts `__slterm_e2e_getSettingsPanelCount` 跨页用例(helpers.ts 适配后全量回归)。
5. **文档同步**:
   - workspace/CLAUDE.md「多 Dockview 实例(H6)」节(:11-17)整节重写为共享宿主页组模型(含 #4978 约束不变、跨页拖拽禁止、面板生命周期契约);:15 笔误随步骤 0 修正;
   - stores/CLAUDE.md「FE-01 / FE-36 页面总数上限」节改写为「上限随多实例架构消亡(S11 共享宿主)」;
   - adr.md ADR-0009 FE-01 行(adr.md:202)改写:多实例保持决策被本项取代(记「CP-004 修复:转共享宿主 + 页组模型,上限消亡」);
   - panels/CLAUDE.md「renderer=always 白名单」语义若因页组显隐有措辞关联,同步核修;e2e-tests/CLAUDE.md:58 句改(见步骤 6)。
6. **验证**:
   - `rg "MAX_PAGES|initializedPages|ensurePageInitialized" src/` 零命中(退出码 1);
   - `npx tsc --noEmit`、`npx eslint src/` 退出码 0;
   - `npm test` 全绿;`npm run e2e` 全绿(H6 跨页存活、settings 跨页计数用例必须绿);
   - 人工:双页各开终端 + 编辑器,切页 20 次往返,终端输出不丢、编辑器无重建闪屏。

---

## CP-005 · std::sync::Mutex 中毒保持现状——全仓换装 parking_lot [Stage S04-4a,全仓单 agent 先行]

1. **位置**(grep 实读全量,锁定决策「约 10 文件」实扩为下列清单):
   - `src-tauri/Cargo.toml:30-67`——[dependencies] 无 parking_lot
   - `src-tauri/src/state.rs:4,15,18,20,24,26,28,49,52,135,137,142,259-260,286,316-335`——use 行 + PtyState/AppState 九字段 + ring_buffer_append
   - `src-tauri/src/settings.rs:37,101-103`——`SETTINGS_SAVE_LOCK`(map_err 形态)
   - `src-tauri/src/background_tasks/mod.rs:89,115-117`——`CONFIG_WRITE_LOCK`(map_err 形态)
   - `src-tauri/src/hooks/mod.rs:18,62,80-86`——`WATCHER`(match 中毒分支形态)
   - `src-tauri/src/plan_balance/mod.rs:12,65,69,146,156,174`——`SNAPSHOT`(.lock().unwrap() 形态)
   - `src-tauri/src/agent_history/claude/scan.rs:17,78,95`——`SCAN_CACHE`
   - `src-tauri/src/git/mod.rs:88,96-98,121-123`——`&std::sync::Mutex<GitRepoCache>` 全路径签名
   - `src-tauri/src/pty/spawn.rs:19,42,224,341,1207,1213-1215,1239-1244,1260,1282,2076`——use + ConPtyInner/OwnedHandle 字段 + writer/child/exit_code/output_ring 构造(:42 为测试 import)
   - `src-tauri/src/pty/reader.rs:23,72-76,88-99,104,156-167,179-182,232,238`
   - `src-tauri/src/notify/mod.rs:13,89,212,240`——生产(`watch_paths` + emit_rescan_overflow 签名)
   - `#[cfg(test)]` 槽位:`src-tauri/src/home.rs:21,32,42,54`、`src-tauri/src/app_dir.rs:50,60,70`
   - 测试局部:`src-tauri/src/hooks/watcher.rs:346,353,474,547`、`src-tauri/src/hooks/signal.rs:329`、`src-tauri/src/fs/mod.rs:872,882`、`src-tauri/src/notify/mod.rs:882,942`、`src-tauri/src/notify/pool.rs:145`
2. **现状**:
   - state.rs:4:`use std::sync::{Arc, Mutex, RwLock};`;src-tauri/src/CLAUDE.md:53 登记:「`state.rs` 等处的 `Arc<Mutex>` 保持标准库 `std::sync::Mutex`。持锁临界区均为短小无 panic 路径,中毒实际不可达,换 `parking_lot` 是零收益依赖变更」——本条即翻此登记。
   - 两种降级形态并存:settings.rs:101-103 `SETTINGS_SAVE_LOCK.lock().map_err(|_| AppError::Unknown("settings 保存锁中毒"))?`;hooks/mod.rs:80-86 `match WATCHER.lock() { Ok(g)=>g, Err(e)=>{ error; return; } }`;plan_balance/mod.rs:146/156/174 与 scan.rs:95 裸 `.lock().unwrap()`(中毒即 panic)。
   - 2026-09-06 核查:8 处登记低估,生产站点实含 notify/mod.rs;`Cargo.toml` 无 parking_lot 依赖。
3. **修复步骤**(4a 先行,4b 各项以本项完成后为基线;全部照抄):
   1. `src-tauri/Cargo.toml:56`(ureq 行后)加:
      ```toml
      # CP-005: 消除 std Mutex 中毒攻击面——锁内 panic 不再连锁 panic 等待方;
      # parking_lot lock() 无 Result,全仓 map_err/unwrap/match 降级站点一并清除
      parking_lot = "0.12"
      ```
   2. 全仓类型替换:`std::sync::Mutex` → `parking_lot::Mutex`、`std::sync::RwLock` → `parking_lot::RwLock`(use 语句与全路径书写站点——git/mod.rs:88 签名、`state.rs:259,260,286` 参数签名——一并替换)。
   3. 锁获取形态统一清除(parking_lot `.lock()/.read()/.write()` 不返回 Result,直接返回 guard):
      - settings.rs:101-103 → `let _guard = SETTINGS_SAVE_LOCK.lock();`(map_err 与「锁中毒」错误消息删除);
      - background_tasks/mod.rs:115-117 → `let _guard = CONFIG_WRITE_LOCK.lock();`(锁序注释 :88「锁序单向:CONFIG_WRITE_LOCK → SETTINGS_SAVE_LOCK」保留);
      - hooks/mod.rs:80-86 → `let mut guard = WATCHER.lock();`(match 中毒分支删除,start_signal_watcher_impl 行为零变更);
      - state.rs `apply_project_root`:293-299 写锁中毒分支删除 → `project_root.write()` 直取;:303-306 read map_err 删除;:46-52 fs extract_root 同款 map_err 删除;
      - pty/spawn.rs pty_kill(:1435-1439)/pty_kill_all(:1497-1500) sessions 写锁 map_err → `.write()` 直取;ConPtyMaster::resize(:231-233)、writer CPR(:1213-1215)同;
      - reader.rs:88-100 child.lock() match → `let mut c = child.lock();`;channel.read() 三处(EOF :108、数据 :156、Err :182)→ `let ch = channel.read();`;
      - git/mod.rs:96-98、121-123 cache lock map_err → `cache.lock()` 直取;
      - scan.rs:95 → `let mut guard = cache.lock();`。
   4. `#[cfg(test)]` 槽位(home.rs:21/32/42/54、app_dir.rs:50/60/70)与测试局部 Mutex/RwLock(hooks/watcher.rs、hooks/signal.rs、fs/mod.rs、notify/mod.rs、notify/pool.rs)一并换装——`parking_lot::Mutex::new` 为 const fn,static 声明形态不变;目标:`rg "std::sync::(Mutex|RwLock)" src-tauri/` 全仓零命中。
   5. 注释同步删改:settings.rs:36「中毒不可达;map_err 兜底防御」句删除;各文件「锁中毒」相关注释(如 state.rs:295-297 BE-24 登记)随分支删除同步清理——BE-24 语义(失败时旧 root 未清)在 parking_lot 下消亡,src-tauri/src/CLAUDE.md 对应句同步(见文档同步)。
4. **测试同步**:
   - 适配(逐一点名):`state.rs` `mod state_tests`(ring_buffer 五用例 :372-476 与 :341-369 构造用例的 `.lock().unwrap()/.read().unwrap()` → `.lock()/.read()`)、`mod project_root_tests`(:871 等 `read().unwrap()` → `.read()`);`settings.rs` 内嵌测试(并发用例 :520 起,锁调用形态若涉 unwrap 同步);`plan_balance/mod.rs` `reset_snapshot_for_test`(:69 `.lock().unwrap().take()` → `.lock().take()`);`notify/mod.rs` 测试(:882、:942);`pty/spawn.rs` 测试(:42 import 换 parking_lot);`agent_history/claude/scan.rs` scan_tests(若涉锁直接调用);`src-tauri/tests/` 集成测试侧经 `make_app_state` 透传,零改动(grep 确认无直接 lock 调用);
   - 新增防复发用例:`hooks/mod.rs` `mod watcher_tests` 增 `start_signal_watcher_locks_without_poison_path`(连续启动两次,第二次命中「已启动跳过」分支——锁获取不再走 Result 形态由编译器保证,行为用例锁死幂等语义不回归);settings.rs 增 `save_settings_lock_section_comment_removed`?——否,注释无测试价值;防复发主体 = 编译期(锁无 Result)+ 下列验证节 grep 守卫;
   - PTY 相关用例全部 `--test-threads=1` 串行(纪律不变)。
5. **文档同步**:
   - src-tauri/src/CLAUDE.md「std Mutex 中毒保持现状(DOC-10)」节(:53)整节重写为:「**parking_lot 换装(CP-005)**:`state.rs` 等全部持锁站点用 `parking_lot::Mutex/RwLock`,中毒攻击面消除(锁内 panic 不再连锁 panic 等待方);新建持锁临界区一律 parking_lot,禁止再引入 std::sync::Mutex/RwLock(grep 守卫)」;同文件「既定豁免」表「Mutex 中毒分支」行删除;
   - adr.md ADR-0009 表 09#14 行(:207)改写:「后端 Mutex **已换装 parking_lot**(CP-005,2026-09):中毒攻击面结构性消除,原『保持现状』登记作废」;
   - pty/CLAUDE.md「既定豁免」表「Mutex 中毒分支」行删除;git/CLAUDE.md「既定豁免」表「仓库缓存 Mutex 中毒分支」行删除;plan_balance/CLAUDE.md:17「照 hooks/mod.rs WATCHER 先例」句保留(先例本身换装,语义不冲突,不强制改);
   - agent_history/CLAUDE.md、notify 相关若涉「锁中毒」措辞,grep 一并清理。
6. **验证**:
   - `rg "std::sync::(Mutex|RwLock)" src-tauri/` 零命中(退出码 1);
   - `rg "锁中毒|poison" src-tauri/src` 零命中(测试名/注释残留即红);
   - `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests -- --test-threads=1` 全绿;
   - `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 与 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 退出码 0。

---

## CP-006 · fs_read_dir 不分页——契约按理想终态重设计(游标分页) [Stage S06,前置 S05 ts-rs 完成]

1. **位置**:
   - `src-tauri/src/fs/mod.rs:15-29`——`DirEntry` DTO
   - `src-tauri/src/fs/mod.rs:334-405`——`fs_read_dir` 命令 + `fs_read_dir_impl`(:348-405,整表返回,签名无分页/游标参数)
   - `src-tauri/src/fs/CLAUDE.md`——「`fs_read_dir` 不分页(BE-21)」节 + 红线「**禁止给 `fs_read_dir` 加分页**:除非同步改前端 FileTree 虚拟化与 IPC 契约」
   - `src/types/fs.ts:4-15`——前端 DirEntry DTO(硬约束 #4 双边)
   - `src/ipc/fs.ts:77-78`——`readDir(path)` 唯一通道
   - 前端消费:`src/features/explorer/useFileTree.ts:37,432`(`gitStatusMap`/`rootNodes`,FileTree 虚拟化 FE-30 承接渲染)
2. **现状**:
   - fs/mod.rs:348-405:`fs_read_dir_impl` 一次性 `Vec<DirEntry>` 全量返回,排序(:396-400 文件夹→文件、同类型小写名称序)与过滤(`.git`,:362-364)在返回前完成;无 cursor/limit 参数。
   - fs/CLAUDE.md 红线原话:「禁止给 `fs_read_dir` 加分页:除非同步改前端 FileTree 虚拟化与 IPC 契约」——本项即同步执行该红线的前置条件。
3. **修复步骤**(前置:S05 ts-rs 已入仓;新契约 DTO 用 ts-rs 定义,全部照抄):
   1. 后端契约重定义(ts-rs DTO,双边导出):
      ```rust
      // src-tauri/src/fs/mod.rs(:15-29 后追加)
      /// 目录分页读取结果(CP-006:游标契约)
      #[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
      #[serde(rename_all = "camelCase")]
      #[ts(export)]
      pub struct FsReadDirPage {
          /// 本页条目(排序与过滤语义同旧整表契约)
          pub entries: Vec<DirEntry>,
          /// 下一页游标;None = 无更多(末页)
          pub next_cursor: Option<String>,
      }

      /// 单页默认/上限条目数(CP-006 写死)
      const READ_DIR_PAGE_DEFAULT: u32 = 500;
      const READ_DIR_PAGE_MAX: u32 = 1000;
      ```
      `fs_read_dir` 命令签名改:
      ```rust
      #[tauri::command]
      pub async fn fs_read_dir(
          path: String,
          cursor: Option<String>,
          limit: Option<u32>,
          state: State<'_, AppState>,
      ) -> Result<FsReadDirPage, AppError>
      ```
      内核实现写死:过滤 + 排序**全量完成后**再按游标切片(排序契约跨页稳定——游标 = 排序后序号的 base64 编码 `start..end` 形态,opaque 不透明);`limit` 越界钳制到 `[1, READ_DIR_PAGE_MAX]`,缺省 `READ_DIR_PAGE_DEFAULT`;`.git` 过滤与文件夹→文件排序语义零变更;**不采用 Channel 增量推送**——拉取式分页已削峰,推送式增加前端状态机复杂度,若执行期实测首帧仍不达标,另行立项(不属本条目)。
   2. 前端双边:`src/types/fs.ts` 增 `FsReadDirPage` 接口(`entries: DirEntry[]; nextCursor: string | null`,camelCase);`src/ipc/fs.ts:77-78` 改:
      ```ts
      export async function readDirPage(path: string, cursor?: string, limit?: number): Promise<FsReadDirPage> {
        return invoke<FsReadDirPage>("fs_read_dir", { path, cursor, limit });
      }
      ```
   3. FileTree 迁移:`useFileTree.ts` `loadRoot` 改首帧拉首页(`limit=READ_DIR_PAGE_DEFAULT`)+ `nextCursor` 非空则后台续页拉取拼接(增量拉取,gen 计数丢弃旧代际响应的竞态语义不变,:13 explorer CLAUDE.md 登记的 rootPath 变化清空语义保留);FileTree 虚拟化层(FE-30)对窗口数据天然透明,无改动;`refresh`/`refreshExpanded` 路径同步走分页聚合。
   4. 红线改写(见文档同步)。
4. **测试同步**:
   - L1(fs/mod.rs 内嵌测试或 tests/fs_* 件,照现有命令内核直测模式):新增 `read_dir_first_page_has_cursor_when_overflow`(溢出目录首页 next_cursor 非空)、`read_dir_last_page_null_cursor`、`read_dir_cursor_resume_mid_list`(第二页接续无重复无遗漏)、`read_dir_page_limit_clamped_to_max`(limit=5000 → 钳 1000)、`read_dir_sort_order_stable_across_pages`(跨页拼接后整体序 = 文件夹→文件 + 小写名称序,锁死排序契约)、`read_dir_git_filter_still_applied`;
   - 既有用例适配:fs 测试中断言整表返回的用例改「分页遍历聚合后断言」(逐一点名:fs_read_dir 相关 L1 用例与 `src-tauri/tests/` 下涉 fs_read_dir 的集成用例);
   - L2:`use-file-tree.test.ts` 首帧 + 续页拼接用例、`explorer-virtualization.test.tsx` 数据供给适配、`src/__tests__/ipc-fs-contract.test.ts` 参数结构改 `{ path, cursor, limit }` 键集合精确断言;
   - 防复发:排序跨页稳定用例(上列)即防「分页破坏排序契约」回归。
5. **文档同步**:
   - fs/CLAUDE.md「`fs_read_dir` 不分页(BE-21)」节重写为「`fs_read_dir` 游标分页(CP-006):`cursor`/`limit` 参数,默认 500/上限 1000,排序过滤后切片,游标 opaque;增量拉取由前端续页拼接」;红线「禁止给 `fs_read_dir` 加分页」改写为「**禁止无游标全量返回**:新增目录读取通道必须走游标分页契约」;
   - adr.md ADR-0009 BE-21 行(:205)改写:「`fs_read_dir` ~~不分页~~ → CP-006 已改游标分页(2026-09)」;
   - src/types/CLAUDE.md 契约对照节补 `FsReadDirPage`;src/ipc/CLAUDE.md fs 通道描述同步。
6. **验证**:
   - `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests read_dir -- --test-threads=1` 全绿;
   - `rg "禁止给 .fs_read_dir. 加分页" src-tauri/src/fs/CLAUDE.md` 零命中;
   - ts-rs 导出产物 `src/types/generated/FsReadDirPage.ts`(路径以 ts-rs 配置为准)存在且与手写 DTO 键集合一致(契约测试锁死);
   - `npx tsc --noEmit` + `npm test` 全绿;万级单目录实测:首帧条目 = 500,交互无阻塞。

---

## CP-007 · session 扫描缓存死机制——先实测,达标删缓存层 [Stage S06]

1. **位置**:
   - `src-tauri/src/agent_history/claude/scan.rs:53-126`——BE-19 缓存(`SCAN_CACHE` :78、`ScanCacheKey` :60-65、`cached_scan` :84-110、`cache_key_of` :113-126)
   - `src-tauri/src/agent_history/mod.rs:88-122`——`agent_history_scan(cli_id, force)` 命令 + `run_scan`(force 分发 :114-122)
   - `src-tauri/src/agent_history/CLAUDE.md`——「扫描缓存 + force 通道(BE-19)」节 + 红线「**缓存键语义勿改**」
   - `src/src/features/backgroundTasks/sessionRefreshTask.ts:26-27`——唯一生产调用点恒 `force=true`(头注释 :4-5 登记理由)
   - `src/features/backgroundTasks/CLAUDE.md:41`——「扫描执行体 force 恒 true」节
   - `src/src/ipc/agentHistory.ts:16-20`、`src/src/ipc/CLAUDE.md:64`、`src/src/features/agentHistory/CLAUDE.md:32`——force 契约三面登记
2. **现状**:
   - scan.rs:57-58 注释:「缓存键 = (目录 mtime, 文件数)……目录内会话文件的增删改不影响根键——由前端显式刷新(force=true)兜底」;前端唯一生产调用点(sessionRefreshTask.ts:27)恒 `scanAgentHistory(p.id, true)`——缓存永不命中,BE-19 沦为死机制(:41 背景Tasks CLAUDE.md 理由「对进行中会话不敏感」即力证)。
   - 现状成本双付:死缓存维护(键计算 + 单槽 Mutex + clone 回填)+ 每次全量读盘。
3. **修复步骤**(先实测,门槛写死;两分支均已写死,执行 agent 按实测结果照抄对应分支,不得自作主张):
   0. **基准实测(本 Stage 第一步)**:scan.rs `mod scan_bench` 新增基准用例(非 `#[ignore]`,全量门禁执行):
      ```rust
      /// CP-007: 1000 会话目录全扫性能门槛——写死,防「无缓存」回归慢化
      #[test]
      fn scan_bench_1000_sessions_median_under_50ms() {
          // 构造:1000 个编码目录 × 每目录 1 个 UUID jsonl(head+tail 真实内容,
          // 照 write_valid_session 夹具形态);样本 20 次取中位
          // 门槛:中位 < 50ms(决策 CP-007 写死);max < 200ms(尾部门槛防 flaky)
      }
      ```
   1. **分支判定**(机械):跑上例——绿 → 走删除分支(步骤 2);红 → 走指纹分支(步骤 3)。
   2. **删除分支(达标,主起草)**:
      1. scan.rs 删 `SCAN_CACHE`(:78)、`ScanCacheEntry`(:68-75)、`ScanCacheKey`(:60-65)、`cache_key_of`(:113-126)、`cached_scan`(:84-110);`scan_sessions()`(:41-43)与 `scan_sessions_with_force`(:49-51)合并为单一 `pub(crate) fn scan_sessions() -> Vec<AgentHistorySession>` = 原 `scan_sessions_uncached(&root)` 直扫;
      2. agent_history/mod.rs:`agent_history_scan` 签名删 `force: Option<bool>` 参数(:97-106);`run_scan` 删 `force` 参数与 `is_claude_provider` 身份比对分支(:114-122)→ `pub(crate) fn run_scan(provider: &dyn CliHistoryProvider) -> Vec<AgentHistorySession> { provider.scan() }`;
      3. `src/ipc/agentHistory.ts:16-20` `scanAgentHistory(cliId, force?)` 删 force 参数;`src/types/agentHistory.ts` 契约注释同步;
      4. sessionRefreshTask.ts:27 `scanAgentHistory(p.id, true)` → `scanAgentHistory(p.id)`;文件头注释 :4-5(恒 force=true 理由段)整段改写为「扫描无缓存全量直扫(CP-007),无需 force」;
      5. 文件头 doc 注释(scan.rs:7-8)删「BE-19 缓存」句。
   3. **指纹分支(不达标,备选,同样写死)**:
      1. `ScanCacheKey` 改目录内容指纹:`root` 一级目录逐项 `(file_name, mtime_ms, len)` 收集 → 按 file_name 排序 → FNV-1a 64 位哈希 + 一级条目数(替代现 mtime+count);`cache_key_of` 重写为指纹计算(每次扫描 O(一级条目数) stat,成本与重扫比可忽略);
      2. force 通道、命令签名、`sessionRefreshTask.ts` 恒 true 全保留;
      3. agent_history/CLAUDE.md BE-19 节改写为指纹口径,红线「缓存键语义勿改」改写为「指纹算法勿改——失效精度承重」。
4. **测试同步**(按分支):
   - 删除分支:scan.rs `mod scan_tests` 删五用例——`scan_cache_hit_returns_stale_without_reread`(:571)、`scan_cache_invalidated_when_file_count_changes`(:591)、`scan_force_true_bypasses_cache`(:614)、`scan_cache_key_tracks_dir_mtime_and_file_count`(:631)、`scan_cache_isolated_per_root`(:659);agent_history/mod.rs 删 `command_scan_force_true_bypasses_cache`(:577)与 `run_scan_force_true_on_non_claude_falls_back_to_trait_scan`(:598)(签名消亡);新增防回归 `scan_reflects_deletion_immediately`(删会话文件后立即重扫结果为空——锁死「无缓存直扫」语义,防 BE-19 复活);agent_history/mod.rs 增 `command_scan_without_force_param`(命令壳参数契约);L2:`ipc-agent-history-contract.test.ts` scanAgentHistory 参数结构删 `force` 键(:69-114 全组适配)、`background-tasks-session-refresh.test.ts`「force=true 各调一次」断言改无第二实参、`agent-history-hook.test.tsx` 同步;
   - 指纹分支:上述缓存用例保留并改指纹口径(`scan_cache_hit_returns_stale_without_reread` 语义不变;`scan_cache_key_tracks_dir_mtime_and_file_count` 改写为 `scan_cache_key_tracks_dir_content_fingerprint`——新增目录指纹变、目录内会话 mtime 变→键变(精度提升的核心用例));新增 `scan_cache_invalidated_when_session_file_modified`(目录内文件 mtime 变化 → 指纹变 → 失效,锁死精度修复);
   - 两分支共有:基准用例(scan_bench_1000_sessions_median_under_50ms)常驻。
5. **文档同步**:
   - agent_history/CLAUDE.md「扫描缓存 + force 通道(BE-19)」节按分支重写(删除分支:「扫描无缓存全量直扫——CP-007 实测 1000 会话中位 <50ms(基准用例常驻守卫);旧 BE-19 缓存层已删,force 参数已移出契约」);红线「缓存键语义勿改」删除分支中整行删除;
   - src/ipc/CLAUDE.md:64 `scanAgentHistory(cliId, force?)` 描述同步;src/types/CLAUDE.md agentHistory 条同步;backgroundTasks/CLAUDE.md:41「扫描执行体 force 恒 true」节改写;features/agentHistory/CLAUDE.md:32 同步;
   - compromises.md 本条销项时勾选,无其它登记点(BE-19 不在 ADR-0009 表,grep 确认)。
6. **验证**:
   - 删除分支:`rg "SCAN_CACHE|ScanCacheKey|scan_sessions_with_force|force" src-tauri/src/agent_history` 零命中(退出码 1);`rg "scanAgentHistory\([^)]*, ?true\)" src/` 零命中;
   - 指纹分支:`rg "dir_mtime_ms" src-tauri/src/agent_history` 零命中(旧键消亡);
   - 共有:`cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests scan -- --test-threads=1` 全绿(含基准用例);`npm test` 全绿。

---

## CP-008 · git_status is_ignored() 死代码——语义对齐验证 + 删死分支 [Stage S04-4b]

1. **位置**:
   - `src-tauri/src/git/mod.rs:50-76`——`status_to_str`(:68-69 `is_ignored()` 死分支);`:26` `GitStatusEntry.status` 注释含 `| ignored`
   - `src-tauri/src/git/CLAUDE.md:21`——「`git_status` 不再扫描被忽略文件」节(末句「`status_to_str` 的 `is_ignored()` 分支保留为无害死代码」);`:65` 红线「不要恢复 `include_ignored(true)`」
   - `src-tauri/tests/git_status_tests.rs:30-56`——B1 纯函数映射用例(:48 `(git2::Status::IGNORED, Some("ignored"))`)
   - 前端容错消费面(实查结论,登记只要求「grep 前端是否消费」):`src/types/git.ts:7` status 注释值集、`src/features/explorer/FileIcon.tsx:28-35`(`ignored: GIT_FILE_COLORS.ignored` 映射)、`src/features/commit/CommitFileList.tsx:277-279`(`GIT_FILE_COLORS[status] ?? fg` 兜底)、theme `GIT_FILE_COLORS.ignored` token、`src/__tests__/commit-context-menu.test.ts:115-119`("ignored → 空(不弹菜单)")、`src/__tests__/commit-open-file.test.ts:295`(`getPanelDispatch("ignored")` → null)、`file-icon.test.tsx:108`、`explorer-git-status.test.tsx:348,411`
2. **现状**:
   - git/mod.rs:68-69:
     ```rust
     } else if status.is_ignored() {
         Some("ignored")
     ```
   - 唯一调用路径 `git_status_impl`(:155-161)的 `StatusOptions` 不含 `include_ignored` → ignored 标志永不置位 → 分支不可达;前端消费面全部为**渲染容错**(颜色映射/菜单空分支/打开拒绝),非功能消费——生产永不收到 `"ignored"` 条目,容错路径是死防御但**保留无害**(对任意未知 status 的兜底行为)。
3. **修复步骤**(语义对齐验证已随起草完成,结论写死:无功能消费,可删):
   1. git/mod.rs:68-69 删除 `} else if status.is_ignored() { Some("ignored") }` 两分支行;`:26` 注释 `modified | added | deleted | renamed | untracked | conflict | ignored` 删 `| ignored`(改后:`modified | added | deleted | renamed | untracked | conflict`);
   2. git/mod.rs:50 `status_to_str` doc 注释「返回 None 表示无变更(Current)」保留;IGNORED 标志落入末支 `None`——与 Current 同语义(跳过),注释补一句:「`IGNORED` 落入 None:include_ignored 恒关,永不置位(CP-008 删死分支)」;
   3. 前端零改动(渲染容错保留);`src/types/git.ts:7` 注释值集删 `| ignored` 双边同步(硬约束 #4 注释对齐)。
4. **测试同步**:
   - 改:`src-tauri/tests/git_status_tests.rs:48` `(git2::Status::IGNORED, Some("ignored"))` → `(git2::Status::IGNORED, None)`(用例名 `test_status_to_str_all_flags` 不变);`:174-190` include_ignored(false) 行为用例保留(语义不变,ignored 文件不出现);
   - 加(防复发):git_status_tests.rs 增 `status_to_str_ignored_returns_none`(显式锁死 IGNORED → None,防 is_ignored 分支回潮);`git_status_ignored_file_never_emitted`(仓库含 .gitignore 忽略文件 → `git_status_impl` 结果无 ignored 条目——命令层锁死);
   - L2 零改动(前端容错用例不涉后端分支)。
5. **文档同步**:
   - git/CLAUDE.md:21 节末句改写:「`status_to_str` 无 `is_ignored()` 分支(CP-008 已删——include_ignored 恒关,ignored 永不置位);未来若确需 ignored 感知,走独立轻量通道(.gitignore 判定),**禁止**恢复全量扫描」;
   - git/CLAUDE.md:65 红线保留并追加同句引用(「.gitignore 轻量通道」为唯一未来路径)。
6. **验证**:
   - `rg "is_ignored" src-tauri/src` 零命中(退出码 1);
   - `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests git_status -- --test-threads=1` 全绿;
   - `npx tsc --noEmit` 退出码 0;clippy/fmt 通过。

---

## CP-009 · PASSTHROUGH_MODE(0x8)永久禁用——模式能力矩阵可配置化 [Stage S08]

1. **位置**:
   - `src-tauri/src/pty/spawn.rs:52-55`——flag 常量(`FLAG_RESIZE_QUIRK` 0x2、`FLAG_WIN32_INPUT_MODE` 0x4;0x1 = `PSEUDOCONSOLE_INHERIT_CURSOR`)
   - `src-tauri/src/pty/spawn.rs:59-87`——`compute_conpty_flags(build_number, bundled)` 三态 + 0x8 禁用注释块(:71-79 实测记录)
   - `src-tauri/src/pty/spawn.rs:1153`——`create_conpty_pair(cols, rows, build)` flags 消费点
   - `src-tauri/src/settings.rs:22-29`——`SETTINGS_ALLOWED_KEYS` 六键白名单(SEC-11)
   - `src-tauri/src/pty/CLAUDE.md`——「PASSTHROUGH_MODE (0x8) 永久禁用」节 + 红线「**永不启用 0x8**」+「改 flags 必须实测真实 claude 滚轮」
   - `.claude/adr.md:167-177`——ADR-0007 审批门禁(:171 第 3 条「真实 claude 实机滚轮测试」)
2. **现状**:
   - spawn.rs:80-87:
     ```rust
     pub fn compute_conpty_flags(build_number: u32, bundled: bool) -> u32 {
         let base = PSEUDOCONSOLE_INHERIT_CURSOR | FLAG_RESIZE_QUIRK;
         if bundled || build_number >= CONPTY_WIN11_MIN_BUILD {
             base | FLAG_WIN32_INPUT_MODE
         } else {
             base // 系统老 conhost 回退:去 WIN32_INPUT_MODE
         }
     }
     ```
   - 三态输出恒不含 0x8;0x8 致 claude 全屏 TUI 滚轮失效(:71-74 实测,Win11 build 26200 双向实测),且「最小复现实验失败……阻断条件仅真实 claude 场景复现」——自动化不可守卫,重开必须人工门禁。
3. **修复步骤**(全部照抄;重开 0x8 永远走人工门禁,本步骤只落可配置化 + 默认矩阵守卫):
   1. pty 域新增设置键常量(spawn.rs :52 前):
      ```rust
      /// CP-009: ConPTY 输入模式能力矩阵设置键(段形态,照 background_tasks::SETTINGS_KEY 先例——
      /// 后端消费型域键名归域模块;默认矩阵 = 现状三态,零默认漂移)
      pub const SETTINGS_KEY: &str = "conptyInputModes";
      /// 模式矩阵 DTO(serde + ts-rs 双边,字段缺省走 Default)
      #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS, PartialEq)]
      #[serde(rename_all = "camelCase", default)]
      #[ts(export)]
      pub struct ConptyInputModes {
          pub inherit_cursor: bool,   // 0x1
          pub resize_quirk: bool,     // 0x2
          pub win32_input_mode: bool, // 0x4
          pub passthrough_mode: bool, // 0x8——默认 false,启用须过 ADR-0007 门禁第 3 条
      }
      impl Default for ConptyInputModes {
          fn default() -> Self {
              Self { inherit_cursor: true, resize_quirk: true, win32_input_mode: true, passthrough_mode: false }
          }
      }
      ```
   2. `compute_conpty_flags` 改签名(三态决策逐位显式化,默认矩阵输出与原三态恒等):
      ```rust
      /// 计算 ConPTY flags(CP-009:能力矩阵可配置化;默认矩阵 = 旧三态)
      pub fn compute_conpty_flags(build_number: u32, bundled: bool, modes: &ConptyInputModes) -> u32 {
          let mut flags = 0;
          if modes.inherit_cursor { flags |= PSEUDOCONSOLE_INHERIT_CURSOR; }
          if modes.resize_quirk { flags |= FLAG_RESIZE_QUIRK; }
          // WIN32_INPUT_MODE 维持原门控:捆绑 conhost 或 Win11 才置位(老 conhost 不识别)
          if modes.win32_input_mode && (bundled || build_number >= CONPTY_WIN11_MIN_BUILD) {
              flags |= FLAG_WIN32_INPUT_MODE;
          }
      ```
      (补 FLAG_PASSTHROUGH_MODE: `const FLAG_PASSTHROUGH_MODE: u32 = 0x8;`,末行 `if modes.passthrough_mode { flags |= FLAG_PASSTHROUGH_MODE; }`)
   3. spawn 路径接线(:1153 调用点):`pty_spawn` 在 spawn_blocking 内经 `crate::settings::read_existing_settings` 读 `conptyInputModes` 段(缺失/解析失败 → `ConptyInputModes::default()` 并 debug 记日志,不阻塞 spawn),矩阵随 `create_conpty_pair` 传入;
   4. 白名单:settings.rs:22-29 `SETTINGS_ALLOWED_KEYS` 六键改七键,追加 `crate::pty::spawn::SETTINGS_KEY`(:27 background_tasks 先例形态);
   5. 前端:`src/types` 增 `ConptyInputModes`(ts-rs 导出双边);settingsCenter 新增「终端输入模式」设置页(四开关;passthrough 开关旁常驻警示「启用将导致 claude 等全屏 TUI 鼠标滚轮失效,变更须实测验证」);store 段 `conptyInputModes` 走既有 settings 保存通道(段形态,照 fontSize 先例);
   6. **人工验证点(标注,不可自动化)**:任何 `passthrough_mode: true` 默认值变更或矩阵位默认翻转,合并前必须完成——ADR-0007 门禁第 3 条(真实 claude 实机滚轮:全屏 TUI + 滚轮滚动)+ Win10 21376 阈值核对;门禁记录在 PR 描述留证。
4. **测试同步**:
   - 适配:spawn.rs 现有 compute_conpty_flags 用例(ADR-0005 登记 L1 7 条)全部注入 `ConptyInputModes::default()` 适配新签名,期望输出不变(0x7/0x7/0x3 三态);
   - 加:`conpty_flags_default_matrix_matches_legacy_tristate`(bundled/Win11/Win10 三输入 × 默认矩阵 → 0x7/0x7/0x3,锁死零默认漂移——防复发主用例)、`conpty_flags_passthrough_mode_adds_0x8`(矩阵 passthrough=true → 输出含 0x8)、`conpty_flags_win32_input_still_gated_by_build`(win32_input_mode=true + Win10 回退 → 0x4 不置位,原门控语义不丢)、settings.rs 白名单用例增第七键(`SETTINGS_ALLOWED_KEYS` 长度 6→7 断言同步);
   - L2:settingsCenter 新页用例(四开关渲染 + passthrough 警示文案断言)、`ipc-settings` 契约(段键 `conptyInputModes` 白名单);
   - 人工门禁登记:.claude/test-exemptions.md 增行:「非默认 flags 矩阵(尤其 0x8)真实滚轮行为——自动化不可守卫(假阴性,spawn.rs:76-79 实证)——兜底 = ADR-0007 门禁第 3 条人工实测」。
5. **文档同步**:
   - pty/CLAUDE.md「PASSTHROUGH_MODE (0x8) 永久禁用」节改写:「0x8 默认禁用 + 能力矩阵可配置化(CP-009)——`conptyInputModes` 设置段四开关,默认矩阵 = 旧三态;**启用 0x8 必须过 ADR-0007 门禁第 3 条真实 claude 滚轮实测**,否则默认 false 不动」;
   - 红线「永不启用 0x8」改写:「默认矩阵不含 0x8;任何 0x8 启用/默认翻转须过 ADR-0007 门禁第 3 条人工实测,无实测记录禁合入」;「改 flags 必须实测真实 claude 滚轮」保留;
   - adr.md ADR-0007「后果」节(:174-177)追加:「CP-009:0x8 自永久禁用转默认禁用 + 设置项可配置;0x8 启用变更除本门禁第 3 条外,还须 `conpty_flags_default_matrix_matches_legacy_tristate` 守卫用例绿(默认零漂移)」;
   - settings.rs:14-21 白名单注释(:16「后端消费型域键名归域模块」)补 conptyInputModes 先例实例。
6. **验证**:
   - `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests conpty -- --test-threads=1` 全绿(含三态等价用例);
   - `rg "永不启用 0x8" src-tauri/src/pty/CLAUDE.md` 零命中(红线措辞已换);
   - `npx tsc --noEmit` + `npm test` 绿;
   - 人工:门禁记录(ADR-0007 第 3 条实测留证)出现在合并说明——无 0x8 默认值变更时本项豁免执行。

---

## CP-010 · Win10 conpty 静默回退——一次性状态查询命令暴露 [Stage S04-4b,跨端]

1. **位置**:
   - `src-tauri/src/pty/conpty_api.rs:195-207`——`build_conpty_api`(回退仅 `tracing::warn!` :203)
   - `src-tauri/src/pty/conpty_api.rs:38-65,128-131`——`ConptyApi`/`Backend`/`is_bundled()`/`resolve_conpty_api`(OnceLock 单例)
   - `src-tauri/src/lib.rs:105-113`——`generate_handler!` 注册点
   - 三处注册红线:src-tauri/src/CLAUDE.md「新增命令必须三处注册」(`lib.rs` `generate_handler!`、`build.rs` `AppManifest::new().commands(...)`、`capabilities/default.json` `allow-<cmd>`,SEC-07)
   - `src-tauri/src/pty/CLAUDE.md`——「Win10 捆绑 conhost(ADR-0005)」节 + 豁免表「conpty_api vendor 提取/加载回退」行
   - 前端启动序列挂点:`src/App.tsx`(「启动对账 reconcile」先例,commit e99524f 同形态)
2. **现状**:
   - conpty_api.rs:200-205:
     ```rust
     match try_bundle() {
         Ok(backend) => ConptyApi { backend },
         Err(e) => {
             tracing::warn!("Win10 捆绑 ConPTY 加载失败,回退系统 conhost(滚轮不可用): {e:#}");
             ConptyApi::system()
         }
     }
     ```
   - 捆绑/回退状态无任何前端暴露通道(:206-207 核查一致);用户无感知落入 0x3 老 conhost(无鼠标滚轮转发)。
3. **修复步骤**(观测性增强,与 ADR-0005 部署形态红线不冲突;全部照抄):
   1. conpty_api.rs 增状态 DTO 与记录槽(:38 前):
      ```rust
      /// ConPTY 后端状态(CP-010:一次性查询,启动 toast 数据源)
      #[derive(Debug, Clone, serde::Serialize)]
      #[serde(rename_all = "camelCase")]
      pub struct ConptyStatus {
          /// 是否尝试捆绑(仅 Win10 build < 21376)
          pub attempted: bool,
          /// 实际是否走捆绑 conhost
          pub bundled: bool,
          /// 回退原因(attempted && !bundled 时有值,与 warn 日志同源)
          pub fallback_reason: Option<String>,
      }
      static STATUS: std::sync::OnceLock<ConptyStatus> = std::sync::OnceLock::new();
      ```
      `build_conpty_api` 改形态:成功 → `STATUS.set(ConptyStatus{attempted:true,bundled:true,fallback_reason:None})`;回退 → warn 文案与 `fallback_reason` **同一变量**(接上抛,:203 的 `{e:#}` 存入 `fallback_reason: Some(format!("{e:#}"))`,日志与命令暴露同源零漂移);Win11/未尝试 → `ConptyStatus{attempted:false,bundled:false,fallback_reason:None}`。查询入口:
      ```rust
      /// 查询 ConPTY 后端状态(一次性;Win11 恒 attempted=false)
      pub fn conpty_status() -> &'static ConptyStatus {
          // resolve_conpty_api 先行的前提下 STATUS 必已初始化;防御分支走 should_bundle 推导
      }
      ```
   2. 新命令(lib.rs :113 后注册,三处同步):
      ```rust
      /// 查询 ConPTY 后端状态(CP-010:Win10 回退可观测)
      #[tauri::command]
      pub async fn pty_conpty_status() -> Result<ConptyStatus, AppError> {
          Ok(conpty_api::conpty_status().clone())
      }
      ```
      三处注册照抄:`lib.rs` `generate_handler!` 增 `pty::conpty_api::pty_conpty_status`;`build.rs` `AppManifest::new().commands(...)` 增 `"pty_conpty_status"`;`capabilities/default.json` 增 `"allow-pty_conpty_status"`。
   3. 前端:`src/types/pty.ts` 增 `ConptyStatus` 接口(`attempted: boolean; bundled: boolean; fallbackReason: string | null`,双边);`src/ipc/pty.ts` 增:
      ```ts
      export async function getConptyStatus(): Promise<ConptyStatus> {
        return invoke<ConptyStatus>("pty_conpty_status");
      }
      ```
      App.tsx 启动序列(紧随「启动对账 reconcile」)调一次:`attempted && !bundled` → `toast.show("warning", "终端已回退到系统控制台,鼠标滚轮转发不可用")`(单次,不重复弹);`bundled` → debug 日志静默。
4. **测试同步**:
   - L1(conpty_api.rs `mod conpty_api_tests`,现有 5 条保留):增 `conpty_status_win11_not_attempted`(build ≥ 21376 → attempted=false/bundled=false)、`conpty_status_bundled_on_win10`(build < 21376 + try_bundle 成功注入 → attempted=true/bundled=true)、`conpty_status_fallback_reason_matches_warn`(失败注入 → fallback_reason=Some 且与 warn 同文案——失败注入点照 `ensure_extracted` 幂等用例的注入先例);STATUS 单例跨用例污染 → 各用例独立进程或由 `conpty_status()` 防御推导分支保证;
   - 命令壳测试:tauri `mock_builder` 先例(pty_conpty_status 转发契约);
   - L2:`src/__tests__/ipc-pty-contract.test.ts` 增命令名 + 返回键集合 camelCase 精确断言;
   - 既有用例适配:无(build_conpty_api 仅增 STATUS 记录,签名不变);
   - 人工验证点(标注):Win10 实机删除 `%LOCALAPPDATA%\slterminal\conpty\` 触发回退 → 启动 toast 出现(ADR-0005 实机红线同批次执行)。
5. **文档同步**:
   - pty/CLAUDE.md「Win10 捆绑 conhost(ADR-0005)」节补:「回退状态经 `pty_conpty_status` 一次性查询暴露,启动 toast 提示降级后果(CP-010);warn 日志与 fallback_reason 同源」;
   - pty/CLAUDE.md 豁免表「conpty_api vendor 提取/加载回退」行「当前兜底层级」列补「回退状态可观测(pty_conpty_status + 启动 toast)」;
   - src-tauri/src/CLAUDE.md 无新增红线(三处注册红线已覆盖);src/types/CLAUDE.md 契约对照 pty 条补 `ConptyStatus`;src/ipc/CLAUDE.md pty 通道描述同步;
   - adr.md ADR-0005 不动(仅观测性增强,不触部署形态红线——compromises.md 本条原文已载明)。
6. **验证**:
   - `rg "pty_conpty_status" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json` 三处全命中(缺一则 invoke reject,SEC-07);
   - `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests conpty -- --test-threads=1` 全绿;
   - `npx tsc --noEmit` + `npm test` 绿;
   - 人工:Win10 实机删提取目录 → 回退 + 启动 toast;Win11 恒静默(attempted=false)。

---

## CP-011 · pty_kill 3s 超时仅 warn——超时后显式清理(监督线程) [Stage S04-4b,与 CP-034 同 agent]

1. **位置**:
   - `src-tauri/src/state.rs:38-44`——`impl Drop for PtySession`(无超时 `handle.join()`,注释失真点)
   - `src-tauri/src/pty/spawn.rs:1418-1475`——`pty_kill`(超时分支 :1463-1468)
   - `src-tauri/src/pty/spawn.rs:1477-1538`——`pty_kill_all`(:1519-1526 超时分支)+ `KILL_JOIN_TIMEOUT`(:1538)/`KILL_JOIN_POLL_INTERVAL`(:1541)/`join_with_timeout`(:1543-)
   - `src-tauri/src/pty/spawn.rs:440-448`——`impl Drop for ConPtyInner`(先 drop writer :443,再 `ClosePseudoConsole` :446——真实无保护阻塞点)
   - `src-tauri/src/pty/CLAUDE.md:52-54`——「pty_kill 异步销毁」节(「超时放弃 join 记 warn,线程随 PtySession Drop 兜底」——失真登记)
2. **现状**:
   - state.rs:38-44:
     ```rust
     impl Drop for PtySession {
         fn drop(&mut self) {
             if let Some(handle) = self.reader_handle.take() {
                 let _ = handle.join();
             }
         }
     }
     ```
     超时路径下 `reader_handle` 已被 `pty_kill`/`pty_kill_all` take(:1463/:1519)→ 本 Drop join 不可达;可达场景(如进程退出清空 sessions)是无超时 join——无界阻塞风险同款。
   - spawn.rs:1466-1468 超时仅 warn「放弃 join(随 Drop 兜底)」;随后 `session` 随闭包尾 drop → master drop → `ConPtyInner::drop`(:440-448)→ `ClosePseudoConsole` 无任何超时保护;上游已证实阻塞等待自 Win11 24H2 移除、Win10 永不修复(microsoft/terminal Discussion #17716)——Win10 永久形态,须应用侧规避。
3. **修复步骤**(采用决策「master drop 移入带超时监督路径」支;「先关输出管道句柄」支因读端句柄由 reader 线程独占持有、kill 路径不可直达而不采用,记录于此;全部照抄):
   0. `join_with_timeout` 与两超时常量上提至 `src-tauri/src/pty/reader.rs`(纯函数无状态;state.rs 经 `crate::pty::reader::join_with_timeout` 引用——state.rs 已有引 `crate::pty::spawn::PtyEvent` 先例(:10),无分层破坏);spawn.rs 改 import。
   1. spawn.rs 增常量(:1538 旁):
      ```rust
      /// CP-011: ClosePseudoConsole 监督超时(上游 Discussion #17716:pre-24H2 可永久阻塞,Win10 永不修复)
      const CLOSE_PSEUDO_CONSOLE_TIMEOUT: Duration = Duration::from_secs(3);
      ```
   2. `pty_kill` 超时分支改写(:1463-1469):
      ```rust
      if let Some(handle) = session.reader_handle.take() {
          if !join_with_timeout(handle, KILL_JOIN_TIMEOUT) {
              // CP-011: reader 未退出(管道未排空高危窗口)——禁止无界阻塞 IPC 线程。
              // reader detach(随进程退出回收);session 移入监督线程执行 drop:
              // ConPtyInner::drop 先关 writer 再 ClosePseudoConsole,监督超时则清理线程一并 detach。
              tracing::warn!("pty_kill: reader 线程 3s 内未退出,detach 并移交监督线程清理");
              let session = session; // move 入监督线程
              match std::thread::Builder::new().name("pty-cleaner".into()).spawn(move || drop(session)) {
                  Ok(cleaner) => {
                      if !join_with_timeout(cleaner, CLOSE_PSEUDO_CONSOLE_TIMEOUT) {
                          // JoinHandle 按值 drop = detach:ClosePseudoConsole 永久阻塞仅泄漏一线程,
                          // 进程退出时 OS 回收全部句柄(Job Object 已保证子进程先死)
                          tracing::error!("pty_kill: ClosePseudoConsole 监督超时,清理线程 detach(OS 兜底回收)");
                      }
                  }
                  Err(e) => tracing::error!("pty_kill: 监督线程启动失败,session 就地 drop(可能阻塞): {e}"),
              }
          }
      }
      ```
      (`pty_kill_all` :1519-1527 同形态逐 session 套用。)
   3. state.rs Drop 修正(:38-44):
      ```rust
      impl Drop for PtySession {
          fn drop(&mut self) {
              // CP-011: 无超时 join 失真修正——本 Drop 仅在 reader_handle 未被
              // pty_kill/pty_kill_all take 时可达(如进程退出清空 sessions);
              // 可达场景同样禁止无界阻塞:带超时 join,超时 detach(进程退出时 OS 回收)。
              if let Some(handle) = self.reader_handle.take() {
                  if !join_with_timeout(handle, KILL_JOIN_TIMEOUT) {
                      tracing::warn!("PtySession drop: reader 未退出,detach(进程退出回收)");
                  }
              }
          }
      }
      ```
   4. 注释失真修正:spawn.rs:1424-1425、1464-1465、1520-1524 三处「超时放弃 join 记 warn,线程随 PtySession Drop 兜底」改为「超时 detach reader,master drop 移交监督线程(CP-011)」;pty/CLAUDE.md:52-54 节同步(见文档同步);`// session drop → master drop → ClosePseudoConsole` 注释行(:1470、:1528)保留并补「正常路径;超时路径见上监督线程」。
   5. 决策抽纯函数(可测性,照 `eof_exit_code` 先例):reader.rs 增
      ```rust
      /// CP-011: reader 未退出后的清理决策(纯函数,L1 锁死两分支)
      pub(crate) enum CleanupPlan { NormalDrop, DetachReaderSupervisedDrop }
      pub(crate) fn plan_cleanup_after_join_timeout(reader_finished: bool) -> CleanupPlan {
          if reader_finished { CleanupPlan::NormalDrop } else { CleanupPlan::DetachReaderSupervisedDrop }
      }
      ```
      pty_kill/pty_kill_all 按 plan 分支执行。
4. **测试同步**:
   - 加:reader.rs `mod reader_tests` 增 `cleanup_plan_finished_reader_normal_drop` / `cleanup_plan_timeout_reader_supervised_drop`(两分支锁死);`join_with_timeout_timeout_returns_false`(mock 永不结束线程 + 短超时——10ms 级,`KILL_JOIN_POLL_INTERVAL` 同缩,防 flaky);PTY 集成(spawn.rs `mod spawn_tests` 区,SPAWN_LOCK 串行)既有 kill 用例全量回归;
   - 既有适配:spawn.rs 涉 KILL_JOIN_TIMEOUT/join_with_timeout 用例改 import(reader.rs);state.rs Drop 用例无直接构造(豁免表不收,drop 行为由集成路径覆盖);
   - 防复发:`rg "\.join\(\)" src-tauri/src` 仅余 `join_with_timeout` 内部与 spawn_blocking 的 `.await` join error(map_err 形态,非线程 join)——裸 `JoinHandle::join` 零命中;
   - 不可自动化登记:ClosePseudoConsole 真实阻塞窗口无法确定性注入 → .claude/test-exemptions.md 增行「`pty_kill` 超时→监督线程真实阻塞路径——Win32 阻塞不可注入——兜底 = `plan_cleanup_after_join_timeout` 决策用例 + pty 集成 kill 用例 + Win10 实机人工验证点(杀会话后应用无挂起)」。
5. **文档同步**:
   - pty/CLAUDE.md「pty_kill 异步销毁」节(:52-54)重写:「`ClosePseudoConsole` 在 pre-Win11 24H2 上可能永久阻塞(上游 Discussion #17716,Win10 永不修复)。`pty_kill` 先提取 session 释放写锁,再在 `spawn_blocking` 中执行 `kill → join reader(3s)`:正常路径随闭包尾 drop;超时路径 reader detach、session 移入监督线程执行 drop(关 writer + ClosePseudoConsole),监督 3s 超时则清理线程 detach,进程退出时 OS 回收句柄(Job Object 保证子进程先死)」;
   - pty/CLAUDE.md 豁免表增行(见测试同步);
   - state.rs:21「reader 线程句柄,pty_kill 时 join 回收」字段注释补「超时/监督语义见 spawn.rs CP-011」。
6. **验证**:
   - `rg "随 (PtySession )?Drop 兜底|随 Drop 兜底" src-tauri/src` 零命中(失真注释全清);
   - `rg "handle\.join\(\)|\.join\(\)\s*;" src-tauri/src` 零命中(裸 join 全清);
   - `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests -- --test-threads=1`(pty 系重点:kill/join/reader 过滤)全绿;
   - clippy/fmt 通过;人工:Win10 实机高负载会话 kill,应用不挂起、3s 内 IPC 返回。

---

## CP-034 · ring buffer 半死机制——删替换层 + ring,reader 收敛单路径 [Stage S04-4b,与 CP-011 同 agent;基线 = CP-005(4a)完成后]

1. **位置**:
   - `src-tauri/src/pty/CLAUDE.md:58`——「Channel 可替换 + ring buffer 回放(E1)」登记行;`:115` 豁免表「`reader_loop` 残余 I/O 编排」行(RwLock 措辞)
   - `src-tauri/src/pty/reader.rs:1-3`(E1 头注释)、`:58-69`(reader_loop 文档)、`:70-78`(签名)、`:108-120`(EOF 分支 channel.read)、`:156-174`(数据分支 channel.read + ring append)、`:182-193`(Err 分支 channel.read);`:392-438`(M11 分析块)
   - `src-tauri/src/state.rs:23-28`(`channel`/`output_ring` 字段)、`:311-335`(`RING_BUFFER_CAPACITY` + `ring_buffer_append`)、`:38-44`(Drop,与 CP-011 同改)
   - `src-tauri/src/pty/spawn.rs:1238-1244`(channel/output_ring 构造)、`:1256-1261`(reader 传参克隆)、`:1269-1279`(reader 线程 spawn)、`:1281-1292`(PtySession 构造)、`:2091`(测试直接构造 PtySession)
   - `.claude/test-exemptions.md:13`——DOC-01 豁免行含「channel 锁/…/ring buffer 批量写入」
   - 与 CP-005 冲突点:reader.rs:23 `use std::sync::{Arc, Mutex, RwLock};`——本条全部锁形态以 4a 换装后 parking_lot 为基线照抄
2. **现状**:
   - reader.rs:63-64 文档:「channel: 可替换的 Channel 引用,pty_reattach 通过写锁替换;ring: ring buffer,总是缓存最近输出供 reattach 回放」——对外重连命令已随 SEC-03 删除,替换与回放均无入口;断开时 ring 写入永不回放(:163-169「总是先缓存到 ring buffer……失败路径(Channel 断连)ring buffer 已有数据」)。
   - pty/CLAUDE.md:58:「该机制保留于内部,对外重连命令已随 SEC-03 删除」——机制活着但无人受益。
3. **修复步骤**(全部照抄;确认无未来 reattach 规划——SEC-03 删除即终态,不留无入口机制):
   1. `PtySession` 删两字段(state.rs:23-26):`channel: Arc<RwLock<Option<Channel<PtyEvent>>>>` 与 `output_ring: Arc<Mutex<VecDeque<u8>>>` 全删(channel 全仓消费方经 grep 确认仅 reader 一线,:24 字段除 spawn 传递外零读取——pty_reattach 已删);
   2. state.rs 删 `RING_BUFFER_CAPACITY`(:312)与 `ring_buffer_append`(:316-335);`use` 行清 VecDeque(:1)与 RwLock/Mutex 涉 ring 的引用(4a 基线上按实际残留清);
   3. `reader_loop` 收敛(签名与三分支):
      ```rust
      /// reader 线程主循环(CP-034:Channel 直写 + 断开退出,单路径)
      pub fn reader_loop(
          mut input: PtyReaderInput,
          channel: tauri::ipc::Channel<PtyEvent>, // 直写,无替换层;自 reader.rs 取类型
          child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,
          exit_code: Arc<Mutex<Option<i32>>>,
          writer: Arc<Mutex<Box<dyn Write + Send>>>,
          da1_injected: Arc<AtomicBool>,
      ) {
      ```
      - 数据分支:删 `channel.read()` 段(:156-162)与 `ring_buffer_append` 调用(:163-169),改为:
        ```rust
        if let Err(e) = channel.send(PtyEvent::Output { bytes: batch }) {
            // CP-034: Channel 断开(前端已卸载)——单路径语义:退出,不缓冲
            tracing::debug!("Channel send 失败(前端已断开),reader 退出: {e}");
            break;
        }
        ```
      - EOF 分支:删 `channel.read()`(:108-114),Exit 事件 send 失败仅 debug 记日志后 break(:115-120 语义不变,去锁);
      - Err 分支:同 EOF 处理(:182-193);
      - 头注释(:1-3 E1 句)、函数文档(:58-69)、M11 分析块(:392-438)同步删 RwLock/ring/reattach 全部措辞,改写为「Channel 直写 + 断开退出」;
   4. spawn.rs 适配:`:1239-1241` 删 channel/output_ring 构造(channel 直接用 `on_output` 形参);`:1256-1257` 删 `reader_channel`/`reader_ring` 克隆(`:1266-1267` writer/da1 克隆保留);`:1270-1278` reader 调用删 ring 实参;`:1281-1292` PtySession 构造删 `channel`/`output_ring` 两字段;`use std::collections::VecDeque` 与 RwLock import 清理;
   5. pty/CLAUDE.md:58 登记行整行删除;:115 豁免表行「依赖 `RwLock<Option<Channel>>`/Mutex/管道系统调用」措辞改「依赖 Channel/管道系统调用」;
   6. test-exemptions.md:13 DOC-01 豁免行删「channel 锁/」「ring buffer 批量写入/」措辞。
4. **测试同步**:
   - 删:state.rs `mod state_tests` ring 用例六条全删——`ring_buffer_append_fifo`(:372)、`ring_buffer_eviction`(:382)、`ring_buffer_eviction_at_newline_boundary`(:395)、`ring_buffer_eviction_long_line_exact_1024`(:413)、`ring_buffer_eviction_long_line_exceed_1024`(:430)、`ring_buffer_eviction_long_line_with_newline`(:452);
   - 改:spawn.rs `:2091` 测试 PtySession 构造删 `channel`/`output_ring` 字段(spawn.rs `mod spawn_tests` 内,逐点核改);reader.rs `mod reader_tests` 零直接 ring 用例(经实读确认),M11 块文本随步骤 3 更新;
   - 加(防复发):`rg` 级守卫之外,L1 增 `scan_...`?——否;防复发 = 本条目验证节 grep 零命中 + 既有 PTY 集成用例(spawn.rs 集成区,SPAWN_LOCK 串行,`pty_integration_tests` 7 条)全绿锁死链路无回归;`reader_loop` 本体残余 I/O 编排豁免照旧(test-exemptions.md:13 措辞更新后),「断开退出」分支不可 L1 构造(Channel send 失败需真实 IPC 对端),由 L4 PTY 通信用例 + L3 headless 覆盖;
   - 前端零改动(PtyEvent 载荷形态 `Output { bytes }`/`Exit { code }` 不变,Channel 订阅协议不变)。
5. **文档同步**:
   - pty/CLAUDE.md:58 删行(见步骤 5);:115 豁免表行措辞改;「reader_loop I/O 编排残余豁免(DOC-01 引用)」相关句同步;
   - test-exemptions.md:13 措辞同步(见步骤 6);
   - state.rs `PtySession` 字段 doc 删 E1 两行注释(:23-26);reader.rs 头注释与 M11 块(见步骤 3);
   - compromises.md 本条销项勾选,无 ADR 登记点(E1 未入 ADR-0009 表,grep 确认)。
6. **验证**:
   - `rg "output_ring|ring_buffer_append|RING_BUFFER_CAPACITY|RwLock<Option<Channel|pty_reattach|reattach" src-tauri/src` 零命中(退出码 1);
   - `rg "ring" src-tauri/src/pty/` 零命中(注释残留即红);
   - `cargo test --manifest-path src-tauri/Cargo.toml --test lib_tests -- --test-threads=1` 全绿(pty 集成串行);
   - clippy/fmt 通过;L4 `npm run e2e` terminal spec 末位杀 app 用例绿(销毁路径端到端)。

---

## 起草附注

1. **CP-004 e2e 触点比登记广**:compromises 登记只点名「e2e resetProjects 冲击」,实读还有三处——wdio.conf.ts:72-79 beforeSuite 双 reset、e2e-tests/CLAUDE.md:58 的「防 MAX_PAGES 触发」句、helpers.ts 三个 helper(resetProjects/addPage/switchToPage);全部已写入步骤。笔误实证:workspace/CLAUDE.md:15「ADR-0001」实指 ADR-0009(adr.md:202 FE-01 行,ADR-0001 是 CSP 档)。
2. **CP-005 站点数漂移**:登记「8 处生产使用点」低估——实读生产站点含 notify/mod.rs(:89,212,240)、plan_balance、agent_history scan、settings、background_tasks、hooks/mod.rs、state.rs 九字段、git/mod.rs 签名、pty spawn/reader,且 hooks/watcher.rs、hooks/signal.rs、fs/mod.rs、notify/pool.rs 的 **test 局部** Mutex 与 home.rs/app_dir.rs 的 `#[cfg(test)]` 槽位须一并换装才能收敛 grep 至零。清单以本条目第 1 节 grep 清单为准,执行 agent 不得只改登记点名的文件。
3. **CP-005 与 CP-034 的 reader.rs 冲突**:两批同改 reader.rs(:23 import、:72-76 签名、:156-174 数据分支)。**4a(CP-005)先行**,本条全部锁形态以 parking_lot 换装后为基线照抄;若编排上 4a 未完成而 034 先行启动,034 步骤 3 的代码块按 std 形态先行、4a 收尾时再统一换 parking_lot(不推荐,双写成本)。
4. **CP-007 触点扩面**:登记只点名 sessionRefreshTask.ts:27,实读 force 契约还有三面文本登记——src/ipc/CLAUDE.md:64、features/backgroundTasks/CLAUDE.md:41、features/agentHistory/CLAUDE.md:32,删除分支须同步;BE-19 **未**登记于 ADR-0009 表(grep 确认),文档面只动 agent_history/CLAUDE.md + 上述三面,无 ADR 改写。
5. **CP-008 测试断言实证**:登记未提 tests/git_status_tests.rs:48 有 `(git2::Status::IGNORED, Some("ignored"))` 直接映射断言——删分支必改该用例,已写入步骤;前端消费实查结论 = **纯渲染容错**(GIT_FILE_COLORS token 映射/菜单空分支/打开拒绝),非功能消费,前端零改动、容错保留(对未知 status 的兜底行为不动)。
6. **CP-009 白名单连锁**:加 `conptyInputModes` 键即动 SEC-11 白名单(settings.rs:22-29)与对应白名单测试;ADR-0007 门禁第 3 条实读 = adr.md:171「真实 claude 实机滚轮测试」。flags 消费点实读为 spawn.rs:1153 `create_conpty_pair(cols, rows, build)`(flags 经 build 号内传),步骤 3 已按此锚定。
7. **CP-010 三处注册与「接上抛」实证**:命令注册红线(src-tauri/src/CLAUDE.md)要求 lib.rs + build.rs + capabilities 三处;`is_bundled()` 已存在(conpty_api.rs:63)复用;「warn 保留并接上抛」落法 = warn 文案与 `fallback_reason` 同一 `format!("{e:#}")` 变量,零漂移。前端启动挂点建议锚定 App.tsx「启动对账 reconcile」旁(e99524f 先例同形态)。
8. **CP-011 失真注释三处实位**:登记只写「state.rs:38-44 不可达」,实读失真注释共四处——spawn.rs:1424-1425(doc 块)、:1464-1467(pty_kill 内)、:1520-1524(pty_kill_all 内)、pty/CLAUDE.md:52-54,全部列入步骤;真实阻塞点在 `ConPtyInner::drop`(spawn.rs:440-448,ClosePseudoConsole :446),登记所指「spawn.rs:1470」是注释行,步骤已锚真点。「先关输出管道句柄」支不采用的理由:读端句柄由 reader 线程经 `clone_reader_with_pending_check` 独占 move,kill 路径无句柄可达,记录于步骤 3 首句。
9. **CP-034 测试构造点实扩**:登记只点 reader.rs:58-66 与 state.rs,实读还有 spawn.rs:2091(测试直接构造 PtySession,含 channel/output_ring 两字段)与 test-exemptions.md:13(DOC-01 行含 ring 措辞),已写入步骤;`PtySession.channel` 字段全仓消费方 grep 确认仅 reader 一线(pty_reattach 已删),字段随 output_ring 一并删除不留死字段。
10. **Stage 编排文件重叠线索**:
    - S04-4a(CP-005)先行单 agent,触及 src-tauri 全仓——4b 全部项(CP-008/010/011/034)以其完成后 git 状态为基线;
    - S04-4b 内部:CP-011 与 CP-034 **必须同 agent 串行**(共改 spawn.rs:1239-1292/1418-1538、state.rs:13-44、reader.rs 全文件、pty/CLAUDE.md);CP-008(仅 git/mod.rs + git 测试 + git/CLAUDE.md)与 CP-010(仅 conpty_api.rs + lib.rs/build.rs/capabilities + App.tsx)与 011/034 零文件重叠,可同批并行;
    - CP-011 的 `join_with_timeout`/常量上提 reader.rs 与 CP-034 的 reader_loop 收敛同文件——同 agent 内先 034 删字段再 011 移函数,或反之,同 commit 序列内完成即可,无跨 agent 冲突;
    - S06(CP-006 fs / CP-007 agent_history)不同文件可并行;CP-007 改 ipc/agentHistory.ts 与 sessionRefreshTask.ts,CP-006 改 ipc/fs.ts,零重叠;
    - S08(CP-009)触 settings.rs 白名单——与 S04-4b 的 state.rs/settings 无重叠(settings.rs 唯 CP-005 动锁),但白名单测试与 CP-005 的 settings 测试同文件,若 S08 先于 S04 执行需注意测试文件合并冲突,建议 S04 先行;
    - S11(CP-004)为独立大架构项,与 S04/S06/S08 全零重叠(纯前端 + e2e helpers)。
11. **ts-rs 前置实证**:CP-006 前置「S05 ts-rs 已完成」——2026-09-06 实读 Cargo.toml 与 src-tauri/src 均无 ts-rs(grep 零命中),即 S05 尚未执行;CP-006 执行前必须确认 S05 已落仓,否则步骤 1 的 `#[derive(ts_rs::TS)]` 无法编译。
12. **CP-007 基准门槛的机器可检性**:`scan_bench_1000_sessions_median_under_50ms` 为真实计时用例,CI 机 IO 抖动可能 flaky——尾部门槛 max < 200ms 即为此设;若执行期实测 CI 不稳定,放宽顺序为「先 max、后中位」,任何放宽须在用例注释内留数字依据,禁止静默删除。
