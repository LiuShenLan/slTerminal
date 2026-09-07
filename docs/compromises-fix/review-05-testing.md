# 章五「测试覆盖缺口」修复清单(CP-023/024/028/029/030/040/041/045/046)

真值源:`docs/compromises.md` 章五(第 97-116 行)。本清单每条均经现状代码原文实读核对(2026-09-06/07),漂移点见末尾「起草附注」。

---

## CP-023 · Rust 行覆盖 88.20% 收尾——生产代码口径重测 + pty 纯逻辑抽取收敛 [Stage S12]

1. **位置**:
   - `.claude/test-exemptions.md:25`——TQ-COV 收尾登记行(88.20% 含测试代码口径,目标 90% 差 1.8pp)
   - `src-tauri/src/main.rs:1-7`——可覆盖行 = 4/5/6 共 3 行(fn main 壳 + install_panic_hook + run 调用),结构性零覆盖
   - pty 模块既有纯逻辑抽取先例:`src-tauri/src/pty/spawn.rs:95`(`build_cmdline`)、`:121`(`build_env_block`,各自带 `mod conpty_custom_tests`/`spawn_tests` 用例,`spawn.rs:573/1661`)
   - 测试模块分布(全部 `#[cfg(test)] mod <领域>_tests`,待加 `#[coverage(off)]`):home.rs / app_dir.rs / state.rs / settings.rs / fs/mod.rs(6 处)/ error.rs / hooks/{watcher,signal,provider,mod×2}.rs / hooks/claude/{config,mod×2,inject}.rs / agent_history/{provider,mod}.rs / agent_history/claude/{mod×4,scan,ops,jsonl}.rs / projects.rs / lib.rs(:157)/ pty/{spawn×2,shell,conpty_api,reader}.rs / background_tasks/{registry,mod×2}.rs / plan_balance/{deepseek,mod×2,kimi,query,source}.rs / notify/{mod,pool}.rs
2. **现状**:
   - test-exemptions.md:25 原文:「Rust 行覆盖 88.20%(llvm-cov 含测试代码口径)| 目标 90% 差 1.8pp;残余缺口集中 PTY Win32 分支 + main.rs 结构性零覆盖 + 编译器生成物计数缺失」。
   - main.rs 全文 7 行,其中 4-6 行 = `fn main() {` / `slterminal_lib::install_panic_hook();` / `slterminal_lib::run()`——L1 无法启动 tauri 运行时,与豁免表 `lib.rs run()` 行(test-exemptions.md:15)同构。
   - 本机已装 cargo-llvm-cov 0.9.0;`--help` 仅有文件级 `--ignore-filename-regex`,无 cfg(test) 代码级排除开关——口径变更须走 Rust 侧 `#[coverage(off)]` 属性(rustc 1.96 已稳定支持,模块级生效)。
3. **修复步骤**:
   1. **生产代码口径改造(机械批处理)**:对上述测试模块清单中每个**文件级** `#[cfg(test)] mod <名> {` 紧跟一行加 `#[coverage(off)]`(属性写在 `mod` 上,模块内全部测试代码退出计数;模块内的 `#[cfg(test)]` 子项与内联 guard 不动)。示例:
      ```rust
      #[cfg(test)]
      #[coverage(off)]
      mod spawn_tests {
      ```
      完成后 `grep -rn "#\[cfg(test)\]" src-tauri/src | wc -l` 与改造前相同(`#[coverage(off)]` 不增删 cfg(test) 行)。
   2. **基线重测**:S02(CP-040)已恢复默认 lib test target,直接全量:
      ```
      cargo llvm-cov -- --test-threads=1
      ```
      记录摘要行覆盖率 P_new(生产代码口径)。全量测试必须全绿(exit 0)——覆盖率数字只在全绿时有效。
   3. **目标重定(写死分支)**:
      - 若 P_new ≥ 90%:删 test-exemptions.md:25 整行,在豁免表「原豁免表脚注」区(:43 一带)补一行销记「TQ-COV 收尾已达成(生产口径 P_new,YYYY-MM-DD 实测)」;本条完成。
      - 若 P_new < 90%:把 :25 行改写为「Rust 行覆盖 P_new(生产代码口径,llvm-cov + #[coverage(off)] 于测试模块)| 目标 90% 差 X.pp;残余缺口 = 本表 pty/ 各行逐条登记项之和」,X 写实测值;继续步骤 4。
   4. **pty 缺口系统性收敛**:用 `cargo llvm-cov --html -- --test-threads=1` 出报告,逐文件扫 `src-tauri/src/pty/{spawn,reader,conpty_api,shell,win_build}.rs` 未覆盖区(不含豁免表已登记行:reader_loop 残余 I/O 编排、spawn_conpty_child 纯 Win32 调用、容量超限 kill 清理、conpty_api vendor 回退——test-exemptions.md:13-14/23 与 pty/CLAUDE.md 既定豁免表)。对每个未覆盖**判定/计算分支**,照 `build_cmdline`/`build_env_block` 先例抽为不依赖 Win32 句柄/运行时状态的纯函数(同文件就近,`fn` 私有),补 `mod <领域>_tests` 用例;确属 Win32 API 组合无法纯化的,逐条进 pty/CLAUDE.md 既定豁免表(项目/原因/兜底层级三列)+ 同步 test-exemptions.md 汇总行,禁止再新开「收尾」型笼统登记。
   5. **main.rs 3 行处理(写死,不评估别的形态)**:在 test-exemptions.md 豁免表新增一行:
      | `main.rs` fn main 3 行胶水 | 结构性零覆盖:L1 无法启动 tauri 运行时安装 panic hook + run() | L4 `terminal.e2e.ts` 启动链真实执行 + `lib.rs run()` 行同构豁免先例 |
      不给 fn main 加 `#[coverage(off)]`(生产代码语义,登记豁免更诚实,与 lib.rs run() 豁免先例一致)。
4. **测试同步**:
   - 步骤 4 每抽一个纯函数,新增用例名 = 「对象_行为_场景」snake_case 裸名,落在所属文件的领域测试模块(如 `micro_batch_stops_at_limit` 先例);改动可自动化部分必须全量覆盖(硬约束 #11)。
   - 既有用例零适配(`#[coverage(off)]` 不改变测试行为;`cargo test` 照常全绿)。
   - 防复发维度:本次为覆盖补齐而非 bugfix,无「改动前老代码」回归面;抽取过程不得改变任何生产行为(纯机械移动,签名/语义逐字保持)。
5. **文档同步**:
   - `.claude/test-exemptions.md:25`:按步骤 3.3 写死口径改写或销记;步骤 5 新增 main.rs 行(位置在 lib.rs run() 行之后)。
   - `src-tauri/src/pty/CLAUDE.md`「既定豁免」表:步骤 4 逐条新增项同步此表;表头注记补「生产代码口径 = 测试模块 #[coverage(off)](TQ-COV 收尾,YYYY-MM)」。
   - 根 CLAUDE.md「测试策略」表:不改(命令形态与口径无关)。
6. **验证**:
   - `cargo test -- --test-threads=1` exit 0,用例数与改造前全量相等(`cargo test -- --test-threads=1 2>&1 | tail` 对比)。
   - `cargo llvm-cov -- --test-threads=1` exit 0 且摘要百分比 = 登记新值(±0.1pp)。
   - 机械断言:`grep -rn "#\[coverage(off)\]" src-tauri/src | wc -l` ≥ 40(文件级测试模块数,实测清单在步骤 1);且 `grep -rn -B1 "^mod \|^    mod " src-tauri/src --include="*.rs" | grep -c "coverage(off)"` 与文件级测试模块清单逐一对应(抽查 pty 5 文件 + lib.rs 必须命中)。
   - `grep -c "main.rs" .claude/test-exemptions.md` ≥ 1(豁免行已登记)。

---

## CP-024 · IPC DTO 契约漂移——ts-rs 单源化(Rust derive 生成,删手写 src/types 对应面) [Stage S05,独立大项,依赖 S02 先行]

1. **位置**:
   - `src/types/`——10 文件 317 行,手写 DTO;导出面 `src/types/index.ts:1-20`;域文件 pty.ts/fs.ts/git.ts/notify.ts/agent.ts/agentHistory.ts/hooksConfig.ts(101 行,含 GUI 模型)/backgroundTasks.ts/planBalance.ts
   - 消费面:全仓 58 文件 74 处 `from "../types(/域)"` 形态 import(e2e-tests 0 处;深导入如 `src/ipc/agentHistory.ts:4` `from "../types/agentHistory"`、`src/features/navTree/NavTree.tsx` `from "../../types/agentHistory"`)
   - Rust DTO 真身清单:`pty/spawn.rs:969`(PtyEvent)、`pty/spawn.rs:1020`(SpawnRequest)、`fs/mod.rs:19`(DirEntry)、`git/mod.rs:19/36`(GitStatusEntry/DiffHunk)、`notify/mod.rs:54`(FsEventPayload)、`hooks/signal.rs:28`(AgentEventPayload)、`hooks/mod.rs:33/47`(AgentInjectionStatus/AgentHookInjectionStatus)、`agent_history/mod.rs:35/62`(AgentHistorySession/AgentHistoryTitle)、`hooks/claude/config.rs:62/72/86`(HooksSubtree/MatcherGroup/HookHandler)、`background_tasks/mod.rs:24`(BackgroundTaskInfo)、`plan_balance/mod.rs:23-50`(PlanBalanceInfo/AmountInfo/WindowsInfo/WindowInfo)
   - 契约测试:`src/__tests__/helpers/ipc-contract.ts:29,83-85`(expectExactKeys)、`src/__tests__/ipc-*-contract.test.ts` 六件
   - 登记点:`src/types/CLAUDE.md:11-19`(双边对应契约)、`src/ipc/CLAUDE.md:94-95`(mockIPC 盲区红线)、根 CLAUDE.md 硬约束 #4
2. **现状**:
   - src/types/agent.ts:18-34 `AgentEventPayload` 手写十字段,可选字段 `usageSourcePath?: string | null`(`?` 源于旧信号缺键 serde default)——Rust `hooks/signal.rs:28` 同名字段为 `Option<String>` + serde default,**TS 的 `?` 与 Rust default 缺省语义当前靠人肉维持**。
   - hooksConfig.ts:26-51 `HookHandlerJson` 为 C13-3 全字段矩阵(command/args/url/headers/server/tool/input/prompt/model/timeout 等),而 Rust `hooks/claude/config.rs:86-93` `HookHandler` **只声明 type+command 两字段**(后端 serde 忽略未知键)——手写面宽于 Rust 面,单源化必须先扩 Rust DTO。
   - hooksConfig.ts:57-100 `HooksConfigGui` 家族(4 类型)是前端 GUI 模型,**Rust 无对应**,不属于迁移面。
   - backgroundTasks.ts:16-22 `BACKGROUND_TASK_IDS` 等常量前端专有;agentHistory.ts:6 `TitleSource = string` 开放字符串(Rust `agent_history/claude/mod.rs:25` 是枚举,语义有意放宽为开放串,不生成)。
   - PtyEvent 双侧 serde 形态一致:`#[serde(tag = "type", content = "data", rename_all = "camelCase")]`(spawn.rs:968)↔ TS 判别联合 `{ type: "output"; data: { bytes: number[] } } | ...`。
3. **修复步骤**(阶段化,执行 agent 只抄写适配):
   1. **阶段 0(S02 前置)**:CP-040 落地、默认 lib test target 恢复。ts-rs 的 `#[ts(export)]` 会在 derive 侧自动生成 `export_bindings_<类型名>` 内嵌单测(落位 lib 单测 target),**无需专用 [[test]] 结构**——S02 只需保证默认 target 可跑。
   2. **阶段 1(依赖接线)**:`src-tauri/Cargo.toml` [dependencies] 段(`serde` 行后)加一行:
      ```toml
      # CP-024:DTO 单源化——Rust derive 生成 TS 类型(serde-compat 默认特性解析
      # rename_all/tag/content/serde(default),与现有 serde 属性零冲突)
      ts-rs = "10"
      ```
      执行时 `cargo add ts-rs` 取解析 major 并锁 Cargo.lock;`cargo check` 通过。
   3. **阶段 2(Rust derive 接线,代表块)**:对上列 DTO 逐个加 `TS` derive + `#[ts(export, export_to = "../src/types/<域>.ts")]`(`export_to` 相对 src-tauri/CARGO_MANIFEST_DIR;同域多类型写同一路径,ts-rs 合并导出)。代表形态:
      ```rust
      use ts_rs::TS;

      /// spawn 参数
      #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
      #[serde(rename_all = "camelCase")]
      #[ts(export, export_to = "../src/types/pty.ts")]
      pub struct SpawnRequest {
          // 字段块原样不动
      }
      ```
      ```rust
      /// PTY 输出事件 — 通过 Channel 推送到前端
      #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
      #[serde(tag = "type", content = "data", rename_all = "camelCase")]
      #[ts(export, export_to = "../src/types/pty.ts")]
      pub enum PtyEvent {
          Output { bytes: Vec<u8> },
          Exit { code: Option<i32> },
      }
      ```
      serde-default 可选字段(对应 TS `field?: T | null` 形态)补 `#[ts(optional)]`,代表:
      ```rust
      /// 用量来源路径(旧信号无此字段——serde default,TS 侧 `?: string | null`)
      #[serde(default)]
      #[ts(optional)]
      pub usage_source_path: Option<String>,
      ```
      先行扩 DTO(写死):`hooks/claude/config.rs:86` `HookHandler` 扩至 C13-3 全字段矩阵(command: Option<String>、args: Option<Vec<String>>、async/async_rewake: Option<bool>、shell: Option<String>、url: Option<String>、headers: Option<BTreeMap<String,String>>、allowed_env_vars: Option<Vec<String>>、server: Option<String>、tool: Option<String>、input: Option<serde_json::Value>、prompt: Option<String>、model: Option<String>、continue_on_block: Option<bool>、if: Option<String>、timeout: Option<u64>、status_message: Option<String>,全部 `#[serde(default, skip_serializing_if = "Option::is_none")]`),校验层(SEC-05 type/command 审查)语义不变;`MatcherGroup.matcher` 同理已 `#[serde(default, skip_serializing_if = "Option::is_none")]` → 补 `#[ts(optional)]`。
      首次接线后跑一次 `cargo test export_bindings -- --test-threads=1` 生成 9 个文件到 `src/types/`,逐文件与手写版 diff:**只允许注释与等价写法差异**;任何字段名/可选性/联合判别差异一律在 Rust 侧修(`#[ts(optional)]`/`#[ts(rename = "...")]`),不回改生成物。
   4. **阶段 3(前端残面收口)**:生成物物理覆盖同名文件(pty.ts/fs.ts/git.ts/notify.ts/agent.ts/agentHistory.ts/hooksConfig.ts/backgroundTasks.ts/planBalance.ts 九个);新建 `src/types/local.ts` 收容手写残面(TitleSource 开放串别名、BACKGROUND_TASK_IDS/BackgroundTaskId/PLAN_BALANCE_TASK_ID/SESSION_REFRESH_TASK_ID 常量族、ContextUsageSignal 前端窄视图——Rust 对应字段已生成,此接口保留为别名);`hooksConfigGui` 家族(GUI 模型)迁出为 `src/types/hooksConfigGui.ts`(含原文件 :57-100 四类型,头部注释保留「前端 GUI 模型,非后端 DTO」);`src/types/index.ts` 重写为:
      ```ts
      // DTO 单源化(CP-024):本目录 9 个域文件由 ts-rs 从 Rust derive 生成
      // (export_to = "../src/types/<域>.ts"),禁手改——改 DTO 走 Rust + 重跑
      // cargo test export_bindings。残面见 local.ts / hooksConfigGui.ts。
      export * from "./pty";
      export * from "./fs";
      export * from "./git";
      export * from "./notify";
      export * from "./agent";
      export * from "./agentHistory";
      export * from "./hooksConfig";
      export * from "./backgroundTasks";
      export * from "./planBalance";
      export * from "./local";
      export type { HooksConfigGui, HookEventGroup, HookMatcherGroup, HookHandlerGui } from "./hooksConfigGui";
      ```
      消费方 58 文件 import 路径与导入名**全部不动**(同名同路径生成);GUI 类型原经 `types/hooksConfig` 导入者改指 `types/hooksConfigGui`(grep `HooksConfigGui\|HookEventGroup\|HookMatcherGroup\|HookHandlerGui` 逐一改,预计仅 settings 页族)。
   5. **阶段 4(契约测试同步)**:`src/__tests__/ipc-*-contract.test.ts` 与 `helpers/ipc-contract.ts` **零改动**(expectExactKeys 守 JS 侧 payload 形状,与类型来源无关);L2 全量原样绿即形状等价证明。新增生成物漂移守卫(操作指令登记,见文档同步):提交前 `cargo test export_bindings -- --test-threads=1 && git diff --exit-code -- src/types`——漂移即红。
4. **测试同步**:
   - 新增(derive 自动生成,无需手写):每个 `#[ts(export)]` 类型一条 `export_bindings_<类型名>` 单测,落位 lib 内嵌单测;步骤 = `cargo test export_bindings -- --test-threads=1` 全绿。
   - Rust 侧 HookHandler 扩字段后:补 L1 serde 用例 `hook_handler_full_matrix_roundtrip`(全 16 字段序列化/反序列化逐字段断言,锁 C13-3 矩阵)于 hooks/claude/config.rs 领域测试模块;BE-18 形态校验既有用例原样保留(逐一点名:`config.rs` 测试模块内 type/command 校验用例,语义不变应全绿)。
   - 既有用例适配逐一点名:`src/__tests__/ipc-*-contract.test.ts` 六件零改动;`background-tasks-session-refresh.test.ts`/`agent-history-hook.test.tsx` 等 mock scan 用例零改动(类型形状等价);若 `nav-tree*.test.tsx` 对 DTO 字面量有 toEqual 断言,生成物字段序差异不影响 toEqual——保持零改动,红了才查形状不等价(属缺陷)。
5. **文档同步**:
   - `src/types/CLAUDE.md`:「双边对应契约」节改写为「单源化(ts-rs)」——Rust derive 为唯一真源;9 域文件为生成物(禁手改,生成/刷新指令 = `cargo test export_bindings -- --test-threads=1`);残面清单(local.ts / hooksConfigGui.ts)与各自理由;「修改注意事项」段改口:改 DTO = 改 Rust derive → 跑导出测试 → `git diff --exit-code -- src/types` 守卫。
   - 根 CLAUDE.md 硬约束 #4:改写为「DTO 单源:Rust `#[derive(TS)]` 经 ts-rs 生成 `src/types/` 对应文件,禁止手写第二份;字段语义值集同步登记与双侧字面量测试契约不变」。
   - `src/ipc/CLAUDE.md:94-95` mockIPC 盲区红线:补一句「DTO 形状真值源 = Rust ts-rs 生成,JS 侧不得另造结构」。
   - `src/types/CLAUDE.md:13-19` 域对照表(ptys.ts ↔ spawn.rs 等):改为生成关系对照(类型 → export_to 文件),不再写「改一边必须改另一边」。
6. **验证**(可机检迁移完成判据,全部满足才销项):
   1. `npx tsc --noEmit` exit 0 且 `npx eslint src/` exit 0。
   2. `grep -rliE "ts-rs|generated" src/types/*.ts | wc -l` = 9(九个域文件全含生成头注);`ls src/types` = 9 域文件 + index.ts + local.ts + hooksConfigGui.ts + CLAUDE.md。
   3. `grep -rnE 'from "(\.\./)+types(/[a-zA-Z]+)?"' src --include="*.ts" --include="*.tsx" | wc -l` = 74(与迁移前同数;每个导入名可解析由 tsc 保证);`grep -rn "from .*types/hooksConfig\"" src --include="*.ts" --include="*.tsx" | grep -i gui` 零命中(GUI 导入已全部改指 hooksConfigGui)。
   4. `grep -rn "export interface AgentHistorySession\|export interface SpawnRequest\|export type PtyEvent" src/types` 的命中文件全部含生成头注(即无手写残留);手写残面仅存于 local.ts/hooksConfigGui.ts。
   5. `cargo test export_bindings -- --test-threads=1` exit 0;随后 `git status --porcelain -- src/types` 输出为空(生成物与提交一致)。
   6. `npm test` exit 0;`npm run test:l3` exit 0。
   7. L4 抽验:`node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts` 全绿(DTO 消费链终端侧)。
   8. `grep -rn "ts-rs" src-tauri/Cargo.toml` 命中且 `cargo tree -p ts-rs` 成功。

---

## CP-028 · e2e 导航树展开循环奇偶风险——NavTree 暴露 aria-expanded 探针 + 两处 6 轮循环收敛为单次确定性操作 [Stage S03]

1. **位置**:
   - `e2e-tests/agent.e2e.ts:57-110`——`ensureTreeExpanded`,6 轮循环(62-109),children 计数判定
   - `e2e-tests/mockcli.e2e.ts:206-255`——CS-3 用例①内联 6 轮循环(209-255),注释自认「奇数次翻转必然到达展开稳态」
   - `e2e-tests/history.e2e.ts:79-118`——`ensureProjectPagesExpanded` **已改单次点击版(先例)**
   - 行组件(展开态 props 现状):`src/features/navTree/NavProjectRow.tsx:47-49`(行根 div `data-e2e="nav-row-project"`,`:62` chevron 按 expanded 换向)、`src/features/navTree/NavPageRow.tsx:69-71`(行根 `nav-row-page`)、`src/features/navTree/NavHistoryNode.tsx:38-40`(节点根 `nav-history-node`,`:62` `expanded &&` 子容器)
   - 展开态真值:`src/features/navTree/useNavTree.ts:103-104`(`expanded`/`expandedHist` 两 Set,默认空 = 全收起)、`:175-191`(`toggleExpand`/`toggleHist`);`NavTree.tsx:430/494/528` 传 `expanded` prop
2. **现状**:
   - agent.e2e.ts:89-94 注释:「页面行无会话时展开不渲染子级容器(DOM 无变化),故以项目展开为统一收敛点;…每轮点击各自提交后奇数次翻转必然到达展开稳态」——**无会话页面行的展开态 DOM 不可判**,靠翻转次数奇偶假设;用例结构变动(多页面行/复用项目)即可能偶数翻转终态收起。
   - 行组件当前无任何展开态属性可探(children 计数是唯一间接信号)。
3. **修复步骤**:
   1. **NavTree 暴露 aria-expanded 探针**(三处行根,照写):
      - `NavProjectRow.tsx` 行根 div(现 :47-49)加属性:`aria-expanded={expanded}`(props 已有,直接引用)。
      - `NavPageRow.tsx` 行根 div(现 :69-71)加:`aria-expanded={expanded}`。
      - `NavHistoryNode.tsx` 节点根 div(现 :38-40,`data-e2e="nav-history-node"`)加:`aria-expanded={expanded}`。
      语义 = 与 chevron 方向同源(同一 `expanded` prop),不新增状态;搜索态 `expanded = searching ? hasChildren : …`(NavTree.tsx:430/494/528)下属性随渲染态如实反映。
   2. **agent.e2e.ts `ensureTreeExpanded` 整体替换为单次确定性版**(照 history.e2e.ts:79-118 先例,写死):
      ```ts
      /** 展开导航树到会话行可见(单次确定性,CP-028):aria-expanded 探针——只点击
       *  aria-expanded !== "true" 的行,每行至多一次点击,无奇偶翻转窗口。前提 =
       *  行初始收起且同一 NavTree 挂载内不重复调用本函数。 */
      async function ensureTreeExpanded(): Promise<void> {
        await browser.execute(() => {
          const proj = Array.from(
            document.querySelectorAll('[data-e2e="nav-row-project"]'),
          ).find((p) => (p.textContent ?? "").includes("当前"));
          if (!proj) return;
          const container = proj.parentElement as HTMLElement | null;
          if (!container) return;
          if (proj.getAttribute("aria-expanded") !== "true") {
            (proj as HTMLElement).click();
          }
          for (const pg of Array.from(
            container.querySelectorAll('[data-e2e="nav-row-page"]'),
          )) {
            if (pg.getAttribute("aria-expanded") !== "true") {
              (pg as HTMLElement).click();
            }
          }
        });
        await browser.waitUntil(
          async () =>
            await browser.execute(() => {
              const proj = Array.from(
                document.querySelectorAll('[data-e2e="nav-row-project"]'),
              ).find((p) => (p.textContent ?? "").includes("当前"));
              if (!proj) return false;
              const container = proj.parentElement as HTMLElement | null;
              if (!container) return false;
              if (proj.getAttribute("aria-expanded") !== "true") return false;
              return Array.from(
                container.querySelectorAll('[data-e2e="nav-row-page"]'),
              ).every((pg) => pg.getAttribute("aria-expanded") === "true");
            }),
          { timeout: 5000, interval: 100, timeoutMsg: "树节点展开超时" },
        );
      }
      ```
      原函数体(57-110)及顶部「展开态判定(DOM 结构)」长注释一并删除,替换为上述块。
   3. **mockcli.e2e.ts 用例①内联循环(206-255)替换为同构单次版**:与步骤 2 同形(单 execute 点未展开行 + waitUntil 全展开),写死代码同上(操作域 = 含「当前」pill 的项目容器);原 6 轮循环与「奇数次翻转」注释删除。
   4. 不改 `useNavTree` 状态语义、不改 NavTree 渲染结构;`waitForSessionRow`(agent.e2e.ts:117-156)内对 `ensureTreeExpanded` 的调用点不变。
4. **测试同步**:
   - L2:`src/features/navTree` 行结构若有渲染断言(`nav-tree.test.tsx`/`nav-tree-history.test.tsx`),在两用例各补一条 `expect(row).toHaveAttribute("aria-expanded", "false")` 级断言(初始收起)与点击后 `"true"`——点名为**建议新增**,非阻塞。
   - L4 适配用例(逐一点名,全绿即过):agent.e2e.ts「nav 视图可通过活动栏按钮打开」「纯 shell 终端无活跃会话行」「动态四态」「R2 变体」「R3 变体」「R4 变体」;mockcli.e2e.ts CS-3 用例①;history.e2e.ts 回归(共用行组件,ensureProjectPagesExpanded/ensureAllProjectsExpanded 形态未动,应原样绿——红了即探针语义错误)。
   - 既有用例适配:无删除用例;两处循环收敛后原「6 轮」注释引用清零。
5. **文档同步**:
   - `src/features/navTree/CLAUDE.md`「数据属性契约(写死)」节(:66 一带)补:「行根/历史节点根挂 `aria-expanded`(展开态探针,与 chevron 同源,E2E 契约 CP-028)——值 ∈ "true"/"false",禁移除」。
   - `e2e-tests/CLAUDE.md`:无需改(无展开循环登记)。
6. **验证**:
   - `grep -c "aria-expanded" src/features/navTree/NavProjectRow.tsx src/features/navTree/NavPageRow.tsx src/features/navTree/NavHistoryNode.tsx` 各 ≥ 1。
   - `grep -c "for (let i = 0; i < 6" e2e-tests/agent.e2e.ts e2e-tests/mockcli.e2e.ts` 各 = 0(两处 6 轮循环已消;history.e2e.ts:124 的 ensureAllProjectsExpanded 循环不在本条范围,见附注)。
   - `npm run e2e` 全绿;重点 `WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec agent.e2e.ts` 与 `--spec mockcli.e2e.ts` 各 1 轮 exit 0。
   - `npx tsc --noEmit` exit 0。

---

## CP-029 · editor.e2e.ts auto-reload 用例多轮确定性失败——二分定责后修环境或修产品 [Stage S03,定序②(CP-003 合并体落地后)]

1. **位置**:
   - `e2e-tests/editor.e2e.ts:283-386`——用例 "should persist modified content to disk after external change triggers reload then Ctrl+S save"(步骤 6 外部写盘 :324,步骤 7 等 auto-reload :327-337,**15s 超时确定性失败**)
   - 前端 reload 链:`src/panels/editor/useCodeMirror.ts:426-504`(`onFsEvent` → 路径规范化比较 :441-446 → kind==="Modify" 过滤 :449 → 无 dirty 自动重载 :484-503;`justSavedRef` 自写抑制 :431-437)
   - watcher 注册链:`src/workspace/Workspace.tsx:238-269`(SEC-01 effect,FE-38 `setProjectRoot` 成功后才 `startWatch`,`:262` `void startWatch(targetRoot)`)
   - 后端链:`src-tauri/src/notify/mod.rs:355-418`(notify_watch 三阶段)、`:491+`(L1 notify_tests,L1 同机通过是既定事实);`src-tauri/src/notify/pool.rs`(LruWatcherPool)
   - E2E 环境注入:`e2e-tests/run-wdio.cjs:34-37`(SLTERM_DATA_DIR)、`:44-47`(假 home USERPROFILE per-pid)
   - 归因冲突登记:`.claude/test-exemptions.md:24`(定性「Windows notify 环境级故障,非代码缺陷」)↔ `docs/compromises.md:105-106` CP-029(定性「待专项排查」,以本条为准)
2. **现状**:
   - 用例保持启用,每轮确定性失败并消耗 mocha retry(run-wdio 默认 retries=1)——失败信号被重试稀释。
   - 豁免表 :24 归因「同机 L1 notify 测试通过,页面内写入不产生 fs-event」,承诺「修复环境后复跑验收」未兑现。
   - Workspace.tsx:235 注释「E2E editor auto-reload 失败根因修复」指 SEC-01 上提已完成,但失败仍在——上提不是根因或根因未除尽。
3. **修复步骤**(二分定责,执行 agent 只取证不预判):
   1. **基线复现**:`WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec editor.e2e.ts`——确认唯一失败用例 = dirty→clean(:283),其余用例全绿;记录失败输出。
   2. **探针 A(事件投递链,取证用代码不入库)**:临时新 spec `e2e-tests/probe-fsevent.e2e.ts`(登记 wdio.conf specs 数组跑完即删):
      ```ts
      it("probe: fs-event 投递链", async () => {
        await waitForWorkspaceReady();
        const dir = mkdtempSync(join(tmpdir(), "slterm-probe-"));
        await createProject(dir);
        await waitForDockviewApi();
        await browser.execute(() => {
          (window as any).__probe = [];
          void window.__TAURI_INTERNALS__.core.listen("fs-event", (e: unknown) => {
            (window as any).__probe.push((e as { payload: unknown }).payload);
          });
        });
        writeFileSync(join(dir, "probe.txt"), "x", "utf8");
        await browser.waitUntil(
          async () =>
            (await browser.execute(
              () => (window as any).__probe.length,
            )) > 0,
          { timeout: 10000, timeoutMsg: "fs-event 未到达页面(投递链断)" },
        );
      });
      ```
      判定:**收到 Modify 事件 → 嫌疑转产品前端链**(步骤 3);**收不到 → 探针 B**。
   3. **探针 B(watcher 注册态)**:同 spec 内追加——`await browser.execute((p) => window.__TAURI_INTERNALS__.core.invoke("notify_watch", { path: p }), dir)` 手动注册后再次写文件:
      - 手动注册后**收到** → SEC-01 自动注册链在 E2E 环境未生效(setProjectRoot then 回调未跑/startWatch 未调/路径不匹配)→ 按产品缺陷修 Workspace.tsx:238-269(补注册态可观测:then 内 `catch` 已有 toast,startWatch 失败静默 `void`——改 startWatch 挂 `.catch(err => console.error + toast)` 后再二分);
      - 手动注册仍**收不到** → 后端 watcher 在 E2E 进程环境失效 → 探针 C。
   4. **探针 C(环境消融)**:同机**不经 run-wdio** 手动起 `target/debug/slterminal.exe`(普通构建,不设 SLTERM_DATA_DIR/假 home)重复「建项目于临时目录 → 外部改文件 → 编辑器 auto-reload」人工验证:
      - 手动通过 → E2E 环境注入致失效;逐次消融:仅设 SLTERM_DATA_DIR 跑一次 → 加假 USERPROFILE 跑一次 → 加 SLTERM_CLAUDE_PROJECTS_DIR 跑一次,定位令投递失效的注入项;
      - 手动亦失败 → 产品缺陷(与 E2E 环境无关),回到步骤 3 产品链修。
   5. **定责收尾(写死两条出口)**:
      - **环境缺陷出口**:修 run-wdio.cjs 注入方式(按消融定位的项);用例保持启用;**修订 test-exemptions.md:24** 归因行为实际根因(删「Windows notify 环境级故障」笼统措辞,写实测根因 + 修复方式 + 「复跑验收:YYYY-MM-DD WDIO_RETRIES=0 连续 3 轮通过」);销 CP-029 登记。
      - **产品缺陷出口**:修 useCodeMirror.ts:426-504 或 Workspace.tsx:238-269 实际缺陷;**删除 test-exemptions.md:24 豁免行**(链路由 L4 用例正常覆盖);按 bugfix 纪律补 L2 回归用例(use-code-mirror-reload 系,对照改动前老代码);销 CP-029 登记。
   6. 修后用例原文不改(editor.e2e.ts:283-386),复跑 `WDIO_RETRIES=0` 连续 3 轮全绿;再跑全量 `npm run e2e` 绿。
4. **测试同步**:
   - editor.e2e.ts:283 用例**保持启用零改动**(豁免表 :24 原本就保持启用)。
   - 探针 spec(probe-fsevent.e2e.ts)取证完删除,不入库、不进 wdio.conf 终态。
   - 产品缺陷出口:新增 L2 回归用例落 `src/__tests__/use-code-mirror-reload-error.test.ts` 或 `editor-confirm.test.ts`(按实际缺陷点二选一,用例名 snake 描述缺陷场景);环境出口:无新用例。
   - 既有用例适配:notify L1 用例(notify/mod.rs notify_tests)零改动——不得为迁就 E2E 改后端 notify 语义。
5. **文档同步**:
   - `.claude/test-exemptions.md:24`:按步骤 5 两出口之一改写(环境出口:留行但归因改写;产品出口:删行)。
   - `e2e-tests/CLAUDE.md`「失败排查提示」节:若根因属环境注入,补一条注记(实测根因一句话,防再排查)。
   - `docs/compromises.md` CP-029 条目:销项勾选 + 一行修复注记(定责结论 + 修复面),与 CP-015 销项格式一致。
6. **验证**:
   - `WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec editor.e2e.ts` 连续 3 轮 exit 0(同一命令复跑)。
   - `npm run e2e` 全绿。
   - 环境出口:`grep "Windows notify 环境级故障" .claude/test-exemptions.md` 零命中;产品出口:`grep "editor.e2e.ts" .claude/test-exemptions.md` 零命中。
   - 探针清理:`ls e2e-tests/probe-fsevent.e2e.ts` 不存在;`grep probe e2e-tests/wdio.conf.ts` 零命中。

---

## CP-030 · tauri-service 焦点检查 +5-15s——beforeSuite 聚焦探针 fast-fail + cli-aliases 真实手势断言回归 [Stage S03]

1. **位置**:
   - `e2e-tests/wdio.conf.ts:69-81`——beforeSuite(现有双 reset,TQ-E-08)
   - tauri-service 本地源码实证:`node_modules/@wdio/tauri-service/dist/cjs/index.js:3037`(`focusCommands = ['getTitle','findElement','findElements','$','$$','elementClick']`)、`:3133`(`core.invoke not available after 5s timeout`)、`:2987-2990`(getWindowStates 经 `plugin:wdio|get_window_states`)
   - 回退后的合成驱动:`e2e-tests/cli-aliases.e2e.ts:77-88`(setInputValue execute 合成)、`:184-256`(用例主流程,添加/删除全走 `browser.execute` 内 `.click()`)
   - 交互时序语义源:`src/panels/settings/pages/CliAliasesPage.tsx:174-179`(blur 不清空不提交,注释明示「真实鼠标点添加别名时 mousedown 先失焦」竞态)、`:58-72`(成功添加后 `setInputs(prev => ({...prev,[cliId]:""}))` 清空输入)
   - 登记点:`e2e-tests/CLAUDE.md` 外部坑节(`$()`/elementClick 触发 focusCommands 条目)、`e2e-tests/CLAUDE.md`「合成 JS click 无焦点语义」节
2. **现状**:
   - e2e CLAUDE.md 实测登记:cli-aliases 真实手势版步骤 2 四个 focus 命令吃 40-60s,长链用例被拖出 mocha 60s 上限多轮失败——上游 1.3.0(2026-08-03)即最新,focusCommands 纯 WARN 无失败语义。
   - 现 spec 全部合成驱动:fC blur→click 竞态(:174-179 修复的目标)在 L4 侧零覆盖(交互时序断言当时回退,登记归 L2 FC-01)。
3. **修复步骤**:
   1. **beforeSuite 聚焦探针(wdio.conf.ts,:70 双 reset 的 browser.execute 之前插入,写死)**:
      ```ts
      // TQ-E-10(CP-030):窗口前台聚焦 fast-fail 探针——$ 元素命令族(findElement/
      // $/elementClick 等)触发 tauri-service ensureActiveWindowFocus,窗口未聚焦时
      // 每命令 +5s 且交互时序断言失真。探针失败即报错退出,不静默吃延迟。
      const focused = await browser.execute(() => document.hasFocus());
      if (focused !== true) {
        throw new Error(
          "[wdio] 应用窗口未前台聚焦——E2E 运行前提不满足(TQ-E-10 探针)。请先聚焦 slTerminal 窗口再跑 npm run e2e;CI 环境请确认 embedded driver 启动后窗口置前。",
        );
      }
      ```
   2. **cli-aliases 真实手势回归(cli-aliases.e2e.ts 用例 184-256,四处替换写死)**:
      - 输入(:202-204)合成 `setInputValue` → 元素命令:`await $('[data-e2e="cli-aliases-input-claude"]').setValue("cc");`(真实焦点转移,blur 竞态真实触发)
      - 添加钮(:205-207)→ `await $('[data-e2e="cli-aliases-add-claude"]').click();`
      - chip 出现等待(:208-214)保持 execute 轮询(executeScript 豁免 focusCommands,不动)
      - 删除钮(:260-262)→ `await $('[data-e2e="cli-aliases-remove-claude-cc"]').click();`
      - **恢复交互时序断言(写死,紧跟添加钮点击之后、落盘等待之前)**:
        ```ts
        // 交互时序断言(CP-030 回归):真实指针序列(mousedown→input blur→mouseup→click)
        // 驱动——blur 不清空(CliAliasesPage blur 语义)+ 成功提交清空输入两语义同时落位。
        const inputAfterAdd = await $('[data-e2e="cli-aliases-input-claude"]');
        expect(await inputAfterAdd.getValue()).toBe("");
        ```
        (chip 出现断言沿用 :208-214 既有块,不断言顺序调整。)
   3. **不 fork tauri-service、不改 node_modules**(锁定决策);其余 spec 不动(execute 内 helper 优先策略不变)。
4. **测试同步**:
   - 新增强约束:所有 spec 的 beforeSuite 自动获得探针(wdio.conf 单点);探针自身无独立测试(行为 = 失焦时 beforeSuite 抛错,人工验证一次:故意 Alt-Tab 离窗跑单 spec,应首条用例前报错退出)。
   - cli-aliases.e2e.ts 用例名不变("添加别名 cc → 落盘 → 终端 OSC 133 C 命中别名…"),内部驱动换真实手势;L2 `settings-cli-aliases.test.tsx` 首条用例(FC-01 blur 编排先例)零改动——L4 版与其构成同源双锁。
   - 既有用例适配逐一点名:无其它 spec 改动;`mockcli.e2e.ts`/hooks 系 spec 的 execute 内 .click() 不强制升级(探针只保证前提,不强制元素命令)。
5. **文档同步**:
   - `e2e-tests/CLAUDE.md` 外部坑节 focusCommands 条目(:一行):补「运行前提 = 窗口前台聚焦,wdio.conf beforeSuite TQ-E-10 探针 fast-fail 保证;前提满足后 $ 族命令正常速度,cli-aliases 已回归真实手势」。
   - `e2e-tests/CLAUDE.md`「合成 JS click 无焦点语义」节:补一句「alias 添加链例外——blur→click 竞态经真实 elementClick 覆盖(CP-030),其余焦点类竞态仍归 L2」。
6. **验证**:
   - `npm run e2e` 全绿(cli-aliases spec 应无 5s 级命令延迟;日志无 `core.invoke not available after 5s timeout` WARN:`grep "not available after 5s" <run log>` 零命中)。
   - 探针负向验证(一次):跑 e2e 时 Alt-Tab 使 slTerminal 失焦 → beforeSuite 抛错、进程非零退出。
   - `WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec cli-aliases.e2e.ts` 1 轮 exit 0。
   - `npx tsc --noEmit` exit 0。

---

## CP-040 · 默认 lib test target 0xc0000139——经 embed-manifest 注入 comctl32 v6 manifest,拆 [lib] test=false 重组与 TQ-COV-06 红线 [Stage S02,独立]

1. **位置**:
   - `src-tauri/Cargo.toml:10-25`——`[lib] test = false`(:16)+ F12 注释块(:17-21)+ `[[test]] lib_tests path = "src/lib.rs"`(:23-25)
   - `src-tauri/build.rs:2-16`——`cargo:rustc-link-arg-tests=/MANIFEST:EMBED`(:12)+ `/MANIFESTINPUT:tests-comctl6.manifest`(:13-16)+ `rerun-if-changed=icons`(:11)
   - `src-tauri/tests-comctl6.manifest`——comctl32 v6 依赖清单(现经 link-arg-tests 嵌入)
   - `.claude/test-exemptions.md:29`——~~cargo test 门禁~~ 已修复(2026-08-31 翻案)整行
   - 根 `CLAUDE.md:77`——「L1 定向测试红线(TQ-COV-06)」整条
   - `.cargo/config.toml`——**不存在**(Glob 零命中;rustflags 通道评估见步骤 2)
   - 波及引用:`src-tauri/src/lib.rs:157`(`mod lib_tests` 内嵌测试模块,与已删 [[test]] 目标同名,仅名字偶合,不动)、`src-tauri/src/git/CLAUDE.md:74`(命令壳测试 TQ-COV-06 标签)、`src-tauri/tests/*.rs` 头部 TQ-COV-06 注释标签
2. **现状**:
   - 根 CLAUDE.md:77 原文:「`cargo test` 带 filter 或 `--lib` 会绕过 `[lib] test=false` 重建默认 lib 单测 target——该 target 收不到 build.rs `rustc-link-arg-tests` 的 comctl32 v6 manifest(SxS),静态导入的 `TaskDialogIndirect` 解析到 v5 → 启动即 0xc0000139…定向测试一律 `cargo test --test lib_tests <filter> -- --test-threads=1`」。
   - 2026-08-31 全量实测 827 例(711 lib + 116 集成)全绿——link-arg-tests 通道对显式 [[test]] 与集成目标生效,唯独默认 lib test target 收不到(cargo 行为,与代码/环境无关)。
3. **修复步骤**(通道二选一已评估定案,**写死 embed-manifest 方案**):
   1. **方案评估结论(理由写死)**:
      - `.cargo/config.toml` `[build] rustflags = ["-Clink-arg=/MANIFEST:EMBED", "-Clink-arg=/MANIFESTINPUT:..."]`:**拒绝**——rustflags 作用于全部构建产物(slterminal.exe bin / cdylib / staticlib 与全部 test target),主 exe 的 tauri 生成 manifest 会被链接参数覆盖(clobber DPI/兼容性声明),且 rustflags 通道无法区分 test/非 test 目标;`.cargo/config.toml` 当前不存在,为它引入全局副作用不值。
      - **embed-manifest crate:采用**——`embed_manifest::embed_manifest_file!` 经 `#[link_section = ".rsrc"]` 直接把 RT_MANIFEST 资源编进**调用处所在产物**:放在 `src/lib.rs` 且以 `#[cfg(all(windows, test))]` 门控后,仅 lib 单测 target 编译该代码 → 只有 lib 单测 exe 获得 comctl6 manifest;bin/cdylib/集成测试不编译 cfg(test) 代码,零副作用;不依赖 `rustc-link-arg-tests` 的 cargo target 分类行为,默认 lib test target 天然覆盖。
   2. **加 dev-dependency**:`src-tauri/Cargo.toml` [dev-dependencies] 段(:97-99)加:
      ```toml
      # CP-040(S02):lib 单测 exe 内嵌 comctl32 v6 manifest——#[cfg(all(windows, test))]
      # 门控下经 link_section(.rsrc)嵌入,补 build.rs rustc-link-arg-tests 覆盖不到
      # 默认 lib test target 的缺口;非 test 构建不编译,主 exe manifest 无冲突。
      embed-manifest = "1"
      ```
   3. **src/lib.rs 注入**(crate 根 item 位,紧邻 `#[cfg(test)]` 区(:156)之前):
      ```rust
      // CP-040(S02):默认 lib test target 的 manifest 注入通道——link_section 资源随
      // rlib 链入单测 exe;link-arg-tests 仅覆盖显式/集成 test target(2026-08-31 实证),
      // 故双通道并存。windows-only;test-only,生产与集成构建零影响。
      #[cfg(all(windows, test))]
      embed_manifest::embed_manifest_file!("tests-comctl6.manifest");
      ```
      (路径相对 src-tauri/CARGO_MANIFEST_DIR;manifest 文件沿用现 tests-comctl6.manifest 不新建。)
   4. **拆除重组(Cargo.toml 一次编辑)**:`[lib]` 段删 `test = false` 行及 :17-21 整段 F12 注释,替换为一句:「CP-040(S02):lib 单测走默认 target,manifest 经 embed-manifest(link_section)注入——定向 `cargo test <filter>`/`--lib` 恢复可用,勿再 test=false + [[test]] 重组」;`[[test]] lib_tests` 块(:23-25)整段删除。
   5. **build.rs 保留**:`:12-16` 两行 `rustc-link-arg-tests` 保留(集成测试目标仍走该通道,2026-08-31 实证其生效);:2-10 注释更新为「link-arg-tests 覆盖显式/集成 test target;lib 单测经 src/lib.rs embed-manifest 注入(S02)」。
   6. **拆除红线**:根 CLAUDE.md:77 整条删除(「L1 定向测试红线(TQ-COV-06…)」bullet,含「核心原则」段内该子项);`.claude/test-exemptions.md:29` 整行删除;该表「原豁免表脚注」区补销记一句:「TQ-COV-06 红线已拆除(S02,YYYY-MM-DD):默认 lib test target 经 embed-manifest 注入 manifest 恢复可用,重组结构与定向红线退役」。
   7. **验证为先**:以下验证全过才允许提交;若步骤 4 后 `cargo test --lib` 仍 0xc0000139(embed-manifest 未生效信号),**停止回退**:恢复 [lib] test=false + [[test]] 重组与红线(本步骤的逆操作),并把实证登记为 compromises.md CP-040 触发条件(上游 cargo issue 路线),不得带病前进。
4. **测试同步**:
   - 零新增/零删除测试用例(本条为构建通道修复);lib 内嵌单测(含 `src/lib.rs:157 mod lib_tests`)与 `src-tauri/tests/*.rs` 六件原样。
   - 计数等价断言:改造前 `cargo test -- --test-threads=1` 全量计数 N1(应 = 827 或实测当前值),改造后 N2 必须 **N2 == N1 且全绿**——用例数不得变化(重组只动 target 形态)。
   - 既有用例适配:无;`git_command_shell_tests.rs` 等头部 TQ-COV-06 注释标签保留(历史出处标签,语义不受影响)。
5. **文档同步**:
   - 根 `CLAUDE.md:77`:删除红线 bullet;「测试策略」表 L1 行命令不变(`cargo test`)。
   - `.claude/test-exemptions.md`:删 :29 行 + 补销记(步骤 3.6)。
   - `src-tauri/Cargo.toml`/`src-tauri/build.rs`/`src-tauri/tests-comctl6.manifest` 头注:按步骤 3.2/3.5 改口径(manifest 文件头注补「lib 单测经 embed-manifest 引用本文件;集成 test target 经 build.rs link-arg 引用」)。
   - `src-tauri/src/git/CLAUDE.md:74`「命令壳测试(TQ-COV-06)」标签行:标签文字保留,补半句「(target 形态已复原,S02)」——只改注释不改约定。
   - 跨章一致性:docs/compromises-fix/review-02/03/04-*.md 验证节中 `cargo test --test lib_tests <filter>` 形态随本条失效(S02 先行),后续 Stage 执行时一律按等价形态 `cargo test <filter> -- --test-threads=1` 适配——由 Stage 编排统一定调,不回改已起草清单(见附注)。
6. **验证**(机械断言):
   - `cargo test --lib -- --test-threads=1` exit 0(默认 lib target 带 filter/--lib 形态,原 0xc0000139 通道)。
   - `cargo test validate_spawn_request -- --test-threads=1` exit 0(带 filter 定向形态)。
   - `cargo test -- --test-threads=1` 全量 exit 0 且计数 = N1。
   - `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` exit 0;`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` exit 0。
   - `grep -n "test = false\|\[\[test\]\]" src-tauri/Cargo.toml` 零命中;`grep -n "lib_tests" .claude/CLAUDE.md` 零命中;`grep -c "lib_tests" .claude/test-exemptions.md` = 0。
   - 主 exe manifest 无冲突:`npx tauri build --debug --no-bundle` 成功且产物启动正常(人工一次,窗口正常出现)。

---

## CP-041 · mockcli 无后端 history provider——补 L4 专用测试 provider(env 门控注册),豁免两行销项 [Stage S03]

1. **位置**:
   - `.claude/test-exemptions.md:21-22`——两条豁免:「mockcli 历史条目展示(L4)」「mockcli 双击恢复注入(L4)」,原因均为「mockcli 无后端 provider」,兜底 L2 AC-4③/⑤
   - 后端注册表:`src-tauri/src/agent_history/provider.rs:46-62`(`static REGISTRY: &[ProviderEntry] = &[("claude", &CLAUDE_PROVIDER)]`、`resolve_provider` 未知 cliId → Validation)
   - 聚合层:`src-tauri/src/agent_history/mod.rs:20-26`(mod 声明)、`:114-133`(run_scan/is_claude_provider 遍历 REGISTRY)、L1 用例引用 REGISTRY(:348,469 等)
   - claude 扫描根 env 先例:`SLTERM_CLAUDE_PROJECTS_DIR`(claude/scan.rs `resolve_projects_root` 内部自管,MC-305)
   - 前端扫描面:`src/features/backgroundTasks/sessionRefreshTask.ts:27` 遍历全部 history 能力 profile 逐个 `scanAgentHistory(p.id, true)`——mockcli profile(helpers.ts:580-612)声明 history 能力,E2E 中每 tick 对 mockcli 发 scan
   - 前端恢复桩:`e2e-tests/helpers.ts:604-610`(buildResumeCommand/buildRestoreInput,输出 `mockcli --resume <id>`);L4 恢复断言先例 `e2e-tests/history.e2e.ts:535-596`
   - run-wdio fixture 通道:`e2e-tests/run-wdio.cjs:181-228`(claude-projects 副本 + 占位符替换 + 缺失即 exit(1))
2. **现状**:
   - E2E 中 `agent_history_scan("mockcli", …)` 恒 Validation「未知 cliId: mockcli」→ sessionRefreshTask 聚合/mockcli 历史行/双击恢复链全断,两条豁免以此为唯一出口。
   - mockcli.e2e.ts 用例② 依赖「未知 cliId: mockcli」错误透传(hooks 配置写命令)——该错误出自 **hooks 模块独立注册表**(`src-tauri/src/hooks/provider.rs:88`),与 history 注册表互不相通,本条不影响该用例。
3. **修复步骤**:
   1. **新建 `src-tauri/src/agent_history/mock.rs`**(L4 专用测试 provider,写死骨架):
      ```rust
      //! mockcli 历史 provider —— L4 专用测试 provider(CP-041)
      //
      //! 仅当 env SLTERM_MOCKCLI_PROJECTS_DIR 存在时注册进 REGISTRY(run-wdio.cjs 注入;
      // 生产/日常二进制无该 env → 注册表与现状一致,agent_history_scan("mockcli") 仍
      // Validation)。扫描根/命名/解析自管(MC-305 先例:env 不上提聚合层)。会话文件
      //! 形态与 claude jsonl 相同(fixture 由 e2e-tests/fixtures/mockcli-projects/ 复制,
      //! 占位符替换同 claude 通道)。

      use std::path::PathBuf;

      use crate::agent_history::claude::ScanRootGuard; // 不使用;占位防误引,实现时删
      use crate::agent_history::{is_uuid_filename, AgentHistorySession, AgentHistoryTitle};
      use crate::error::AppError;

      use super::CliHistoryProvider;

      /// mockcli 测试 provider(无状态单元结构体,注册表静态引用)
      pub struct MockCliHistoryProvider;

      /// 扫描根解析:env 优先;未设置 → None(生产形态,provider 注册亦被 env 门控)
      fn scan_root() -> Option<PathBuf> {
          std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR").map(PathBuf::from)
      }

      impl CliHistoryProvider for MockCliHistoryProvider {
          fn scan(&self) -> Vec<AgentHistorySession> {
              // 形态照 claude scan:遍历扫描根一级编码目录,UUID 主干 .jsonl,
              // 解析 cwd/summary 首行(复用 claude::jsonl 可复用解析助手,若可见性
              // 不足则将其 pub(crate) 化——不得复刻第二份解析),条目打标 cli_id = "mockcli"。
              // env 缺失/目录不存在 → 空 Vec(无 Err 通道,照 trait 契约降级)。
              // 实现体由执行 agent 照 claude/scan.rs 同构抄写适配。
              todo!()
          }
          fn delete(&self, session_id: &str) -> Result<(), AppError> {
              // 定位扫描根内 UUID jsonl 并删除;不存在 → Io NotFound 语义照 claude ops
              todo!()
          }
          fn validate_session_id(&self, session_id: &str) -> Result<(), AppError> {
              // 照 trait 契约:UUID 形态(is_uuid_filename)——与 claude 校验同口径
              if is_uuid_filename(session_id) { Ok(()) } else { Err(AppError::Validation(format!("非法 sessionId: {session_id}"))) }
          }
          fn read_title(&self, session_id: &str) -> Result<AgentHistoryTitle, AppError> {
              // 回退链照 claude:summary > firstPrompt;文件缺失 → Ok(title: None)
              todo!()
          }
      }
      ```
      (骨架中 `todo!()` 两处由执行 agent 按注释照 claude/scan.rs、claude/ops.rs 同构填充;`ScanRootGuard` 误引行删除。)
   2. **注册表 env 门控(provider.rs:46-62 改写)**:
      ```rust
      /// mockcli provider 静态实例(CP-041:L4 专用,env 门控注册)
      static MOCK_PROVIDER: MockCliHistoryProvider = MockCliHistoryProvider;

      /// 注册表条目:cliId → history provider
      pub(crate) type ProviderEntry<'a> = (&'static str, &'a dyn CliHistoryProvider);

      /// 基础注册表(生产形态,与现状逐字一致)
      static BASE_REGISTRY: &[ProviderEntry<'static>] = &[("claude", &CLAUDE_PROVIDER)];

      /// L4 形态注册表(env SLTERM_MOCKCLI_PROJECTS_DIR 存在时启用——run-wdio.cjs 注入)
      static E2E_REGISTRY: &[ProviderEntry<'static>] = &[
          ("claude", &CLAUDE_PROVIDER),
          ("mockcli", &MOCK_PROVIDER),
      ];

      /// 当前生效注册表(env 门控,CP-041)
      pub(crate) fn registry() -> &'static [ProviderEntry<'static>] {
          if std::env::var_os("SLTERM_MOCKCLI_PROJECTS_DIR").is_some() {
              E2E_REGISTRY
          } else {
              BASE_REGISTRY
          }
      }
      ```
      `resolve_provider` 体内 `REGISTRY.iter()` 改 `registry().iter()`;**mod.rs 同步**:`use provider::{…, REGISTRY}`(:26)改 `registry`;`is_claude_provider`(:125-133)内 `REGISTRY.iter()` 改 `registry().iter()`(注释「注册表恒含 claude 条目」语义不变,两表均含 claude)。provider.rs 既有 L1 用例(resolve_provider_known/unknown)断言对象不变,全绿即过;新增用例:`registry_env_absent_returns_base_only`(env 未设 → 仅 claude 条目)、`registry_env_present_includes_mockcli`(env 设置 → 含 mockcli 条目;env 修改用 `std::env::set_var` 后须串行恢复——L1 门禁 `--test-threads=1` 保证,用例内 set/remove 成对)。
   3. **mod.rs 声明**:`pub mod claude; pub mod provider;`(:20-21)后加 `mod mock;`(crate 内私有,不对 crate 外暴露)。
   4. **run-wdio.cjs 注入**(claude fixture 块(:211-228)之后追加,写死):
      ```js
      // ── mockcli 历史会话 fixture 副本 + env 注入(CP-041) ──
      // 后端 mockcli provider 扫描根(注册表 env 门控——仅 E2E 注入该 env)。
      // 缺失同 claude 通道硬失败(红线条款「fixture 缺失必须终止」扩列 mockcli)。
      const mockFixturesDir = path.join(__dirname, 'fixtures', 'mockcli-projects');
      const tmpMockProjectsDir = path.join(__dirname, '.tmp-mockcli-projects');
      if (!fs.existsSync(mockFixturesDir)) {
        console.error('[wdio-launcher] fixtures/mockcli-projects 缺失,E2E 终止(mockcli provider fixture 必须存在)');
        process.exit(1);
      }
      copyFixtureTree(
        mockFixturesDir,
        tmpMockProjectsDir,
        '__E2E_PROJECT_DIR__',
        e2eProjectDir.replace(/\\/g, '\\\\'),
      );
      process.env.SLTERM_MOCKCLI_PROJECTS_DIR = tmpMockProjectsDir;
      console.log(`[wdio-launcher] 已重建 mockcli-projects 副本 → ${tmpMockProjectsDir}`);
      ```
      文件头注(:5-21 防复发校验段后)补一行 mockcli env 说明。
   5. **fixture 新建 `e2e-tests/fixtures/mockcli-projects/`**:形态照 `fixtures/claude-projects/`——一级编码目录(如 `C--Users-e2e-mockcli-a/`)+ UUID 主干 .jsonl(summary 首行 + cwd 占位符 `__E2E_PROJECT_DIR__`,归属 E2E 项目目录),README 一行维护说明(照 claude fixture README 口径)。fixture 会话 ≥1 条(恢复用例目标)。
   6. **L4 用例(mockcli.e2e.ts 新增第三 describe,写死流程)**:
      ```ts
      describe("mockcli 历史链路(CP-041 L4:展示 + 双击恢复注入)", () => {
        it("mockcli 历史条目展示:provider 打标条目渲染于导航树历史节点(cliId=mockcli)", async () => {
          // 1. setupTerminal(注册 mockcli profile——history 能力声明已随 helpers.ts)
          // 2. createProject(e2eProjectDir)——fixture cwd 占位符已替换为该目录,归属匹配
          // 3. openNavView + ensureAllProjectsExpanded/ensureHistoryExpanded 模式照 history.e2e.ts
          //    (触发/等待 sessionRefresh 聚合扫描含 mockcli provider)
          // 4. 断言:nav-history-node 内行文本含 fixture 标题 + 行内 img src 含 "/cli-icons/mockcli.png"
          //    (NavHistoryRow 按 session.cliId 查 profile.iconSrc)
        });
        it("mockcli 双击恢复:恢复编排 → 终端注入 `mockcli --resume <id>`(部分端到端,E2E-11)", async () => {
          // 1-3 同上;4. dblclick fixture 行 → 断言(照 history.e2e.ts:557-596 四步):
          //    activePage rootPath = e2eProjectDir → 终端容器就绪 → __e2e_getTerminalText
          //    含 `mockcli --resume <fixture UUID>`(buildRestoreInput 桩输出,helpers.ts:608)
        });
      });
      ```
      展开辅助若 spec 内无 ensureAllProjectsExpanded 则内联单次确定性版(CP-028 同口径;S03 内 CP-028 先落地时直接复用其辅助形态)。
4. **测试同步**:
   - L1 新增(provider.rs 领域测试模块):`registry_env_absent_returns_base_only`、`registry_present_e2e_env_includes_mockcli`(env 门控两态);mock.rs 新领域测试模块 `mock_provider_tests`:scan 空 env → 空 Vec、validate UUID 两态、read_title 文件缺失 → Ok(None)、fixture tempdir 单条目 scan 打标 `cli_id == "mockcli"`、delete 落盘真删(照 claude ops 用例口径,mk tempdir 隔离)。
   - L4 新增:上列两用例(mockcli.e2e.ts);既有 mockcli 用例①②零改动(用例②「未知 cliId」走 hooks 注册表,不受影响,逐一点名)。
   - 既有用例适配:mod.rs 既有 `command_scan_unknown_cli_id_returns_validation` 用例 cliId "nope" 仍未知,零改动;sessionRefreshTask L2 用例(`background-tasks-session-refresh.test.ts` 125/144 行)零改动(mock scan 不经后端注册表)。
   - 豁免销项:删 test-exemptions.md:21-22 两行;其 L2 兜底(AC-4③/⑤)保留原位(它们本就独立存在,非豁免行附属)。
5. **文档同步**:
   - `.claude/test-exemptions.md`:删 :21-22 两行;脚注区(:43 一带)无需新增(mockcli 两豁免已销,无残余)。
   - `src-tauri/src/agent_history/CLAUDE.md`:「CliHistoryProvider trait + cliId 注册表」节补:「注册表 env 门控扩展(CP-041):`registry()` 为当前生效表,`SLTERM_MOCKCLI_PROJECTS_DIR` 存在时追加 mockcli 条目——L4 专用测试 provider,env 命名/解析自管(MC-305 先例)」;「既定豁免」表不变。
   - `e2e-tests/CLAUDE.md`:「用户目录隔离」节 fixture 通道段补 mockcli-projects 副本 + env 注入一句;「fixture 缺失必须终止」红线清单扩列 mockcli-projects。
   - `e2e-tests/mockcli.e2e.ts` 文件头注:补第三 describe 的存在与数据隔离语义(扫描根 = .tmp-mockcli-projects 副本,SLTERM_MOCKCLI_PROJECTS_DIR,不触真实 ~/.claude)。
6. **验证**:
   - L1:`cargo test --test lib_tests mock -- --test-threads=1`(mock provider + registry 门控用例)exit 0;全量 `cargo test -- --test-threads=1` 计数 = 基线 + 新增数,全绿。
   - 生产形态守卫:不设 env 的机器上 `cargo test --test lib_tests registry_env_absent -- --test-threads=1` 过;`resolve_provider("mockcli")` 仍 Validation 由用例锁死。
   - L4:`WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts` 三轮 exit 0(含新增两用例);全量 `npm run e2e` 绿。
   - 机检:`grep -c "mockcli" .claude/test-exemptions.md` = 0(两豁免行已销);`grep -n "SLTERM_MOCKCLI_PROJECTS_DIR" e2e-tests/run-wdio.cjs src-tauri/src/agent_history/*.rs` 各 ≥ 1。

---

## CP-045 · e2e 假屋清理残留——文档同步销项(代码已是 per-pid 唯一命名) [Stage S03,与 CP-003/046 同 agent(run-wdio.cjs 合并体)]

1. **位置**:
   - `e2e-tests/run-wdio.cjs:44`——`const fakeHomeDir = path.join(os.tmpdir(), "slterm-e2e-home-" + String(process.pid));`(per-pid 唯一命名,评审实证;原文为模板字符串形态)
   - `e2e-tests/run-wdio.cjs:39-43`——唯一名免清空语义注释块(IME/遥测句柄占用根因登记)
   - **漂移点** `e2e-tests/run-wdio.cjs:13`——文件头注仍写旧固定名「启动时建临时假屋(os.tmpdir()/slterm-e2e-home)」(老注释残留)
   - `e2e-tests/CLAUDE.md:48`——「假 home 隔离(USERPROFILE)」节仍写「临时假屋 `<os.tmpdir()>/slterm-e2e-home`」(固定名)
   - `.claude/test-exemptions.md:69`——已写 `<tmp>/slterm-e2e-home-<pid>`(正确,无需动)
2. **现状**:
   - run-wdio.cjs:40-43 注释原文:「假屋目录每次运行唯一(pid 后缀):IME/遥测组件…长驻句柄——固定名假屋的启动清空必撞 EPERM。唯一名免清空;exit 清理 best-effort,残留目录留在 tmp 由 OS 回收(无害——隔离目标已达成,真实屋零接触)」——**修改方向二选一中的「按运行实例唯一命名」已落地**;166-171 行 exit 清理保持 best-effort 属该方案既定语义。
   - 唯一残留 = 两处文档仍写固定名(:13 头注、CLAUDE.md:48)。
3. **修复步骤**(纯文档/注释,写死):
   1. `run-wdio.cjs:13` 头注改写为:「启动时建临时假屋(`os.tmpdir()/slterm-e2e-home-<pid>`,per-pid 唯一——IME/遥测句柄占用根因见 :39-43 注释)并把 USERPROFILE 指向它」。
   2. `e2e-tests/CLAUDE.md:48` 句中「`run-wdio.cjs` 建临时假屋 `<os.tmpdir()>/slterm-e2e-home`」改写为「`<os.tmpdir()>/slterm-e2e-home-<pid>`(per-pid 唯一名,免启动清空——句柄占用残留无害,OS 回收)」。
   3. 销项:`docs/compromises.md` CP-045 勾选 `[x]` 并按 CP-015 先例补修复销项注记(「run-wdio.cjs:44 已是 per-pid 唯一命名(评审实证);残留 = 头注 + e2e-tests/CLAUDE.md:48 固定名表述,已同步;残留目录无害语义已在头注」)。
   4. 不改动任何清理逻辑(44/168-171 行原样)。
4. **测试同步**:无(纯文档);L4 全量回归一轮即可。
5. **文档同步**:即步骤 1-3 本身;`e2e-tests/CLAUDE.md` 其它节(防复发校验 :50)不涉及假屋命名,不动。
6. **验证**:
   - `grep -rn "slterm-e2e-home" e2e-tests/CLAUDE.md e2e-tests/run-wdio.cjs .claude/test-exemptions.md` 所有命中均带 pid/唯一名语义(不再有裸固定名表述);机检:「os.tmpdir()/slterm-e2e-home」后无 `<pid>`/`${process.pid}` 字样的命中 = 0。
   - `node -e "const s=require('fs').readFileSync('e2e-tests/run-wdio.cjs','utf8'); if(/slterm-e2e-home(?!-)/.test(s.replace(/\$\{process\.pid\}/,''))) process.exit(1)"` exit 0(头注清洗后)。
   - `npm run e2e` 一轮全绿(注释改动不应影响运行,顺带验证)。

---

## CP-046 · e2e 真实屋校验并发误报——settings.json 键级断言改写(只校对本套件应写入的键) [Stage S03,与 CP-003/045 同 agent(run-wdio.cjs 合并体)]

1. **位置**:
   - `e2e-tests/run-wdio.cjs:66-75`——`snapFile`(整文件 sha256)、`:78-98`——`snapDir`(目录树 sha)、`:100-108`——`snapshotUserHome`、`:110-139`——`verifyRealHomeUnchanged`(整文件/整树 diff 判定)
   - `e2e-tests/CLAUDE.md:50`——「防复发校验」节(存在性 + sha256 快照口径)、`:54`——「已知并发误报面」节(整文件 diff 无法区分外部并发合法修改)
   - `e2e-tests/run-wdio.cjs:17-21`——头注防复发校验说明
2. **现状**:
   - verifyRealHomeUnchanged:118-127 对 `~/.claude/settings.json` 与 `~/.slterminal/statusline-backup.json` 做整文件 sha 前后相等判定;`:128-132` 对 hooks/ 目录做整树相等;`:134-137` hooks-events 存在性校验(启动不存在才校)。
   - 误报面原文(CLAUDE.md:54):开发者并发跑真实 claude/slterminal 合法改写 settings.json → exit 校验报红;报红被习惯性忽略后真回归也会被放过。
3. **修复步骤**(run-wdio.cjs:66-139 改写,写死):
   1. **哨兵键集合与键级快照**(新增,替换 settings.json 的整文件 snap):
      ```js
      /** 本套件泄漏判定哨兵键——E2E 唯一可能写入真实屋 settings.json 的键
       *  (hooks 注入 matcher / statusLine 桥接;其余键(env/permissions/用户配置)
       *  外部并发修改合法,不做整文件 diff——CP-046 键级断言口径) */
      const SETTINGS_SENTINEL_KEYS = ["hooks", "statusLine"];

      /** settings.json 哨兵键快照:{ [key]: { existed: boolean, json: unknown } }
       *  (文件不存在/解析失败 → 全键 { existed: false, json: null }) */
      function snapSettingsSentinels(p) {
        let parsed = null;
        try {
          parsed = JSON.parse(fs.readFileSync(p, "utf8"));
        } catch {
          // 不存在/损坏:按全键不存在处理
        }
        const out = {};
        for (const k of SETTINGS_SENTINEL_KEYS) {
          const has = parsed !== null && typeof parsed === "object" && k in parsed;
          out[k] = has ? { existed: true, json: parsed[k] ?? null } : { existed: false, json: null };
        }
        return out;
      }
      ```
   2. **snapshotUserHome 改写**(:101-108):`claudeSettings: snapFile(...)` 改为 `claudeSettings: snapSettingsSentinels(path.join(realHome, '.claude', 'settings.json'))`;`statuslineBackup`/`hooksDir`/`hooksEventsExisted` 三字段保留原 snap 形态(这两个对象非用户高频并发改写面,维持文件级/目录级)。
   3. **verifyRealHomeUnchanged 改写**(:113-127 的 expectFile 调用段):
      ```js
      // settings.json 键级校验(CP-046):只比对哨兵键——本测试应写入的键存在且值
      // 与启动快照一致;外部并发改写其它键合法放行。
      const settingsPath = path.join(realHome, '.claude', 'settings.json');
      const curSentinels = snapSettingsSentinels(settingsPath);
      for (const k of SETTINGS_SENTINEL_KEYS) {
        const before = snap.claudeSettings[k];
        const cur = curSentinels[k];
        if (before.existed !== cur.existed) {
          problems.push(`真实屋 ${settingsPath} 哨兵键 "${k}" 存在性变化(启动 ${before.existed ? '存在' : '不存在'} → 当前 ${cur.existed ? '存在' : '不存在'})——疑似 E2E 泄漏`);
        } else if (cur.existed && JSON.stringify(cur.json) !== JSON.stringify(before.json)) {
          problems.push(`真实屋 ${settingsPath} 哨兵键 "${k}" 值在 E2E 期间被修改(泄漏)`);
        }
      }
      ```
      statusline-backup.json 的 expectFile(:123-127)与 hooksDir 整树(:128-132)、hooks-events(:134-137)原样保留。
   4. **删除**:`snapFile` 若改写后无其它调用方则删除(检索 `snapFile(` 命中 = 仅 snapDir 内部一处时保留给 snapDir;snapDir 仍服务 hooks/ 目录树)。
   5. 防复发语义不变:任何 Rust 侧收敛遗漏导致 hooks/statusLine 落真实屋,仍独立报红 exitCode=1;哨兵键外的泄漏通道(未来新消费点)由哨兵键集合评审维护(新增消费点 = 本数组加键,登记进 e2e-tests/CLAUDE.md 防复发节)。
4. **测试同步**:
   - L4 层无新用例(校验属 runner 退出路径);**人工负向验证一次**(写死步骤):临时在 exit 校验前向真实屋 settings.json 写入 `hooks` 键(测试后立即撤销)跑一次单 spec → 退出码 1 且 stderr 含「哨兵键 "hooks"」;再删除该键复跑 → 通过。验证完撤销临时代码。
   - 既有用例:无适配(spec 不感知 runner 内部)。
5. **文档同步**:
   - `e2e-tests/CLAUDE.md:50`「防复发校验」节改写:「对真实屋 `~/.claude/settings.json` 做**哨兵键级**快照比对(键 = hooks/statusLine——E2E 唯一可能写入的键,CP-046);`~/.slterminal/statusline-backup.json` 与 hooks/ 目录维持整文件/整树 sha256;hooks-events 存在性校验保留。外部并发改写 settings.json 其它键不再报红」。
   - `e2e-tests/CLAUDE.md:54`「已知并发误报面」节改写:「已收窄(CP-046):校验 = settings.json 哨兵键级断言,用户并发编辑 env/permissions 等其它键不再误报;并发 hooks 注入/卸载仍会命中哨兵键报红——属真实泄漏信号,排查方向照旧」。
   - `run-wdio.cjs:17-21` 头注同步一句键级口径。
6. **验证**:
   - 机检:`grep -n "SETTINGS_SENTINEL_KEYS\|snapSettingsSentinels" e2e-tests/run-wdio.cjs | wc -l` ≥ 4;`grep -n "sha256" e2e-tests/run-wdio.cjs` 命中 ≥ 2 处且 settings.json 不再走 snapFile(expectFile 调用只剩 statuslineBackup 一处:grep 校验 `expectFile(` 命中 = 1)。
   - 正向:`node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts`(任一短 spec)exit 0,日志含「真实用户目录校验通过(零接触)」。
   - 负向:步骤 4 的人工注入验证记录 exit 1 + stderr 哨兵键文案。
   - 文档:`grep "哨兵键" e2e-tests/CLAUDE.md` ≥ 2 命中(:50/:54 两节)。

---

## 起草附注

### 漂移点清单(实读核对 vs compromises.md/锁定决策转述)

1. **CP-045「唯一名免清空语义已在头注」部分失真**:run-wdio.cjs 唯一名注释块在 :39-43(新增注释),但文件头注 :13 仍写旧固定名「os.tmpdir()/slterm-e2e-home」——销项步骤须连头注一起改(已写入步骤)。
2. **CP-028 登记外存在第三处同构循环**:history.e2e.ts:123-148 `ensureAllProjectsExpanded` 仍是 6 轮 children 计数循环。项目行展开恒渲染子容器(历史节点,NAV-10),DOM 可判、奇偶风险面与页面行不同,不在 CP-028 登记范围;本清单未将其列入验收,建议 S03 同 agent 顺手同改(单次 aria-expanded 版),不改不算本条未完成。
3. **CP-024「58 文件」实测 = 58 文件 74 处 import**(grep 证据:`grep -rE 'from "(\.\./)+types(/[a-zA-Z]+)?"' src` = 74 行 / 58 文件;e2e-tests 0 处);且存在深导入 `types/<域>` 形态 → 生成物必须**同名同路径**覆盖,不能迁子目录(已写死)。
4. **CP-024 Rust HookHandler 面窄于 TS**:hooks/claude/config.rs:86-93 仅 type+command 两字段,TS HookHandlerJson 为 C13-3 全矩阵 → 单源化前置「扩 Rust DTO」必须发生(已写死进阶段 2,BE-18/SEC-05 校验语义不变)。
5. **CP-024 TitleSource 双边语义有意不一致**:Rust 枚举(agent_history/claude/mod.rs:25)vs TS 开放字符串(agentHistory.ts:6)——按现状保留 TS 别名于 local.ts,不生成(已写死)。
6. **CP-040 波及既有清单**:docs/compromises-fix/review-02/03/04-*.md 验证节多处 `cargo test --test lib_tests <filter>` 形态,S02 拆除 [[test]] 后该形态失效——不回改已起草清单,后续 Stage 执行统一按 `cargo test <filter> -- --test-threads=1` 等价适配(比 `--test lib_tests` 少一级 target 选择,行为等价)。
7. **CP-040 附加事实**:`.cargo/config.toml` 不存在;src/lib.rs:157 存在内嵌 `mod lib_tests` 测试模块(与 [[test]] 目标同名偶合,执行 agent 勿混淆——拆的是 Cargo.toml 目标,不是该模块);tauri-service 本地 dist 与 review 登记逐点吻合(:3037 focusCommands 名单、:3133 5s 超时)。
8. **CP-023 工具事实**:本机 cargo-llvm-cov 0.9.0,`--help` 无 cfg(test) 代码级排除开关——生产口径只能走 Rust 侧 `#[coverage(off)]`(rustc 1.96 稳定);llvm-cov 全量命令必须在 S02 之后跑(默认 lib target 才能承载)。
9. **CP-029/041 事实补充**:Workspace.tsx:235 注释「E2E editor auto-reload 失败根因修复」指 SEC-01 上提(已完成但失败仍在);hooks 配置写命令的「未知 cliId」与 history 注册表相互独立(hooks/provider.rs:88),CP-041 不破坏 mockcli.e2e.ts 用例②。

### Stage 编排与文件重叠线索

- **S02(CP-040)先行**:S05(CP-024)的 export_bindings 内嵌单测与 S12(CP-023)的 llvm-cov 全量都依赖默认 lib test target;S02 拆除红线后,review-02/03/04 清单的定向命令形态按附注 6 适配。
- **S03 内 run-wdio.cjs 合并体(CP-003/045/046)**:CP-041 亦改 run-wdio.cjs(mockcli fixture 块,追加于 claude 块后)——建议 CP-041 并入同 agent 或紧随其后提交,避免合并体二次冲突;CP-029 若定责为环境出口,改 run-wdio.cjs 注入,同样并入合并体序列。
- **S03 内 e2e spec 分布**:CP-028 触碰 agent.e2e.ts/mockcli.e2e.ts + NavProjectRow/NavPageRow/NavHistoryNode 三个组件(及 L2 nav-tree*.test.tsx 断言适配);CP-029 触碰 editor.e2e.ts(仅取证,终态零改动或仅豁免修订);CP-030 触碰 wdio.conf.ts(beforeSuite 探针)+ cli-aliases.e2e.ts;CP-041 触碰 mockcli.e2e.ts(第三 describe)——**CP-028 与 CP-041 同改 mockcli.e2e.ts,须同 agent 串行或明确前后序**;CP-041 的展开辅助复用 CP-028 的单次确定性形态(若 CP-028 先落地)。
- **S03 定序②**:CP-029 在 CP-003(Node 26 工具链合并体)落地后执行(锁定决策);CP-030 探针与 CP-003 无文件重叠,顺序不限。
- **S05(CP-024)文件面**:src-tauri(Cargo.toml + 11 个 DTO 定义文件)/src/types(9 生成 + 2 残面 + index)/58 消费文件(预期零改动)/六件 ipc-*-contract.test.ts(零改动);与 S02 的唯一交接点 = 默认 lib target。
- **S12(CP-023)依赖**:llvm-cov 口径改造为全仓测试模块批注(~40 处 `#[coverage(off)]`),与 S02 后的 `cargo test` 形态天然兼容;pty 缺口收敛参照物 = test-exemptions.md:13-14/23 已登记行,禁止新开笼统登记。
