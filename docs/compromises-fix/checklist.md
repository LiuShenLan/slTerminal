# 妥协修复总清单(checklist)

定稿日期:2026-09-07。真值源 = 六份章级报告(review-01~06,同目录,起草期 2026-09-06/07 实读核对);本文件为按 Stage 重排的汇总定稿,执行 agent 以本文件条目为准。

## 约定说明

- **ID 沿用 CP 编号**(compromises.md 原编号),不重排;CP-015 已修复销项,不在本册。
- **优先级由 Stage 依赖顺序表达**(S01→S12 串行,Stage 内并行位见 stages.md 分工表),不设 P0-P4 标签。
- **条目结构 = 六段式**:位置(file:line)/现状摘录/修复步骤(含可照抄代码块)/测试同步/文档同步/验证断言。执行 agent 照抄步骤,不另起方向。
- **合并关系**(一个 Stage 位承载多个 CP 项):
  | 合并体 | 成员 | 原因 |
  |---|---|---|
  | CP-026+CP-021 | S01 同 agent | CP-021 为契约占位(章四),实现归 CP-026(章六);拆分会造成编译断裂窗口 |
  | CP-038+CP-027 | S01 同 agent | 共改 package.json(scripts 段 vs devDependencies 字段级错峰) |
  | CP-003+CP-045+CP-046 | S03 同 agent | 共改 e2e-tests/run-wdio.cjs;CP-045 降级为文档销项(run-wdio.cjs:44 已是 per-pid 唯一命名) |
  | CP-011+CP-034 | S04-4b 同 agent | 共改 spawn.rs/state.rs/reader.rs |
  | CP-017+CP-036 | S07 同 agent | 共改 tabClose.ts;CP-036 依赖 CP-017 第 4 步先行 |
  | CP-039+CP-002 | S08 同 agent | 共碰 JsonMode.tsx;CP-039 先于 CP-002 |
  | CP-012/013/031/044 | S10-② 同卡 | 共改 docViewer 注入/消息面六文件;CP-031 为消亡判定(零独立代码) |
- **CP-031 处置**:零独立代码,S10 收尾执行三条消亡判定(条目见 S10 节);若 S10 架构评审结论为「仍同态」则翻案重新起草。
- **验证命令形态注意(全册适用)**:S02(CP-040)执行**前**,L1 定向测试 = `cargo test --test lib_tests <filter> -- --test-threads=1`(TQ-COV-06 红线);S02 **后**该形态失效,恢复 `cargo test <filter> -- --test-threads=1`。章一/二/三/四/五报告条目内写 `--test lib_tests` 的断言属起草时形态,执行时按 Stage 时点适配(S01/S02 用旧形态,S03 起用新形态)。
- **行号时效**:条目行号为 2026-09-06/07 工作区实态;执行 Stage 时以锚点文本/符号名为准,行号仅定位辅助。
- **人工验证点**(不可自动化,Stage 验收硬门槛):CP-009(claude 实机滚轮)、CP-010(Win10 回退 toast)、CP-003(Node 26 e2e 全量)、CP-029(定责结论)、CP-007(扫描成本门槛)、S10(WDIO 多 webview spike + 预览回归)、S11(布局全场景人工演练)。
- **文档同步前置**:各 Stage 同步本 Stage 触及的登记点原文(ADR/模块 CLAUDE.md),S12 只做 compromises.md 销项总扫 + 一致性抽查。

## 导航

| Stage | 条目 | CP 覆盖 |
|---|---|---|
| S01 低风险清理与登记硬化 | CP-025;CP-026+CP-021;CP-038+CP-027;CP-032;CP-001 | 7 项 |
| S02 L1 测试基建 | CP-040 | 1 项 |
| S03 e2e 基建族 | CP-003+CP-045+CP-046;CP-029;CP-030;CP-028;CP-041 | 7 项 |
| S04 后端基础原语 | CP-005(4a);CP-008/CP-010/CP-011+CP-034/CP-014/CP-043(4b) | 7 项 |
| S05 DTO ts-rs 单源化 | CP-024 | 1 项 |
| S06 后端契约重设计 | CP-006;CP-007 | 2 项 |
| S07 面板状态解耦 | CP-017;CP-036;CP-042;CP-016;CP-037;CP-019 | 6 项 |
| S08 终端与主题体验 | CP-020;CP-018;CP-039+CP-002;CP-009 | 5 项 |
| S09 编辑器大文件 | CP-022 | 1 项 |
| S10 预览通道根治 | ①spike;②CP-012/013/031/044;③CP-033;④CP-035 | 6 项 |
| S11 Dockview 重设计 | CP-004 | 1 项 |
| S12 收尾 | CP-023 + 销项总扫 + 全量回归 | 1 项 |

合计 44 CP 项 ✓

---

# S01 低风险清理与登记硬化(7 项,并行 ≤4)

agent 分工:CP-025 独立;CP-026+CP-021 单 agent;CP-038+CP-027 单 agent;CP-032+CP-001 可并行(注意 CP-032/CP-038/CP-027/CP-001 均触及 package.json,字段级错峰:scripts 段归 CP-027、devDependencies pin 归 CP-038、overrides 段归 CP-032——由 CP-038+CP-027 agent 统一收口 package.json 写入或明确串行)。

## CP-025 · 退役 sidebar 目录物理删除(原章六)

1. **位置**:
   - `src/features/sidebar/CLAUDE.md`(目录唯一存活文件,全 25 行)
   - 误导注释:`src/__tests__/agent-history-restore.test.ts:4`
   - 悬空引用登记:`.claude/skills/systematic-changes-plan/config.json:52`
2. **现状**:
   - `src/features/sidebar/CLAUDE.md:7`:「`src/features/sidebar/` 目录仅剩本文件——`SidebarTree.tsx` 及其配套已于 NAV-06 整体删除。本目录待清理:目录删除时本文件一并删除。」
   - `agent-history-restore.test.ts:4` 注释原文:`//   stores/projects(useProjects.getState + ID 生成)、features/sidebar(makeEmptyLayout)、`——而实际 mock 本体在 :63-66 早已指向 navTree:
     ```ts
     // NAV-06：makeEmptyLayout 随 SidebarTree 退役迁入 navTree(restoreSession 消费点改引用)
     vi.mock("../features/navTree/NavTree", () => ({
       makeEmptyLayout: () => ({}),
     }));
     ```
   - 全仓 `features/sidebar` 引用 grep 归零面:`src/`(除该注释)、`knip.json`、`e2e-tests/`、`src-tauri/` 均无引用;仅存 `config.json:52` 与 compromises 文档自身。
3. **修复步骤**:
   1. 物理删除目录:`git rm -r src/features/sidebar`(仅含 CLAUDE.md 一个文件)。
   2. 修正 `src/__tests__/agent-history-restore.test.ts:4` 注释,改后原文:
      ```ts
      //   stores/projects(useProjects.getState + ID 生成)、features/navTree(makeEmptyLayout,
      //   NAV-06 随 SidebarTree 退役迁入——mock 目标即 ../features/navTree/NavTree)、
      ```
      (:5-6 行其余 mock 清单行不动。)
   3. 删除 `.claude/skills/systematic-changes-plan/config.json` 的 `"src/features/sidebar/CLAUDE.md",` 行(:52,`claudeMdFiles` 数组内)——目录删除后该登记即悬空。
4. **测试同步**:无行为变化(mock 本体早已指向 navTree),不新增用例;`agent-history-restore.test.ts` 全部既有用例原样通过即回归锁。防复发断言:验证节 grep 锁 `features/sidebar` 在 `src/` 零命中。
5. **文档同步**:`src/features/sidebar/CLAUDE.md` 随目录删除即闭合,无迁移内容(navTree/CLAUDE.md「CRUD 迁移承接(NAV-06)」已承载其有效约定);其余 CLAUDE.md 无涉。
6. **验证**:
   - `test ! -e src/features/sidebar`(退出码 0)。
   - `grep -rn "features/sidebar" src/ .claude/skills/` 退出码 1(零命中;compromises 文档与 knip.out 为历史产物不计)。
   - `npx vitest run agent-history-restore` 全绿。

---

## CP-026+CP-021 · useAgentStatus 迁入 navTree + 返回面收窄 + 60s ticker 移交宿主(原章六 CP-026 实现 + 章四 CP-021 契约,单 agent)

**CP-021 契约锚(章四占位条目,本节实现必须满足)**:
- ticker 落点 = `src/features/navTree/NavTree.tsx` 宿主单一 60s ticker(渲染层与数据层节奏解耦——sessionRefresh 禁用/慢档时相对时间仍自动刷新);
- `NavHistoryRowProps` 新增必填 `now: number`;`NavHistoryRow.tsx:38` 的 `formatRelativeTime(session.mtimeMs, Date.now())` 改为用 prop `now`;
- NavTree 渲染 NavHistoryRow 处传 `now={now}`;useAgentStatus 的 60s ticker 一并移交宿主(由本条目定稿);
- 文档锚:`src/features/agentHistory/CLAUDE.md:89` MC-318 改写为「相对时间由 navTree 宿主 60s ticker 驱动重算(CP-021,与 sessionRefresh 数据层节奏解耦)」。

**CP-026 实现条目**:

1. **位置**:
   - 迁移源:`src/features/agentStatus/useAgentStatus.ts`(死面::55-59 `AgentStatusState`、:61-68 `AgentStatusResult`、:90+ :92-97 `now` state 与 60s ticker、:109 `currentProjectName`、:413-421 state 派生、:423 return)
   - 迁移目标:`src/features/navTree/useAgentStatus.ts`(新)
   - 消费改接线:`src/features/navTree/useNavTree.ts:24-25,99`、`src/features/navTree/NavSessionRow.tsx:14`
   - ticker 落点(CP-021 契约):`src/features/navTree/NavTree.tsx`(:193 `const nav = useNavTree();` 后;渲染点 :511-517)、`src/features/navTree/NavHistoryRow.tsx:21-38`(props 接口 + `Date.now()`)
   - 目录删除:`src/features/agentStatus/`(useAgentStatus.ts 迁出后仅剩 `CLAUDE.md`)
   - 测试:`src/__tests__/agent-status-hook.test.ts:144,309-387`、`src/__tests__/nav-tree.test.tsx:51,138,261-266,307-310,357,400,430,551,742,988,1329,1367`、`src/__tests__/nav-tree-history.test.tsx:35,210-215`、`src/__tests__/nav-history-row.test.tsx:78-96,107,193`
   - 配置:`knip.json:22-24`
2. **现状**:
   - `useAgentStatus.ts:62-68`:`AgentStatusResult` 含 `state`/`rows`/`currentProjectName`/`now` 四面;生产唯一消费方 `useNavTree.ts:99` 为 `const { rows } = useAgentStatus();`——其余三面仅测试 mock 形状消费(nav-tree.test.tsx:264 等 10 处 `mockReturnValue({ state, rows, currentProjectName, now })`);`knip.out:169` 亦标 `AgentStatusState` 未使用(死面佐证),`knip.json:22-24` 以 `types` 豁免压制。
   - `useAgentStatus.ts:90-97`:
     ```ts
     const [now, setNow] = useState(() => Date.now());
     // 相对时间定时刷新(问题 1b 修复):formatRelativeTime 渲染时计算,
     // 无 hook 事件时组件不重渲染 → 时间文本永久冻结;60s ticker 强制重算
     useEffect(() => {
       const timer = setInterval(() => setNow(Date.now()), 60_000);
       return () => clearInterval(timer);
     }, []);
     ```
   - `NavHistoryRow.tsx:38`:`const timeStr = formatRelativeTime(session.mtimeMs, Date.now());`(渲染层无自主 ticker,历史区相对时间冻结——CP-021)。
   - `NavSessionRow` 无相对时间消费(仅用量条 + 百分比)——`now` 移交后活跃会话行无涉。
   - `agentStatus/CLAUDE.md` 载有效约束:行建模双/三通道(F5)、行 cliId MC-205 三级解析(ZQ-2 契约 4)/MC-206、ZQ-3 决策 2 建行 status、ContextUsage 信号分支(AC-5)、竞态双保险、行 title 动态跟随页签、FE-23 generation、项目域过滤。
3. **修复步骤**:
   1. 迁移:`git mv src/features/agentStatus/useAgentStatus.ts src/features/navTree/useAgentStatus.ts`。
   2. 返回面收窄——`src/features/navTree/useAgentStatus.ts` 全文照抄章六报告 review-06 的 CP-026 条目步骤 3.2 完整代码块(约 390 行,含头注「CP-026:返回面收窄为 AgentSessionRow[]——state/currentProjectName/now 为已退役视图时代死面,删除;相对时间 60s ticker 移交 NavTree 宿主(CP-021),now 经 prop 注入 NavHistoryRow」;迁移 + 死面删除一步完成;import 深度不变,两目录同为 `src/features/<x>/`,`../cliProfiles` 等相对路径原样保留;`useAgentStatus()` 返回类型由 `AgentStatusResult` 收窄为 `AgentSessionRow[]`)。
   3. 改接线 `src/features/navTree/useNavTree.ts`:
      - :24-25 两行改为:
        ```ts
        import { useAgentStatus } from "./useAgentStatus";
        import type { AgentSessionRow } from "./useAgentStatus";
        ```
      - :99 改为 `const rows = useAgentStatus();`;文件头 :5 注释「活跃会话:useAgentStatus(rows——...)」不变(hook 名未变)。
   4. 改 `src/features/navTree/NavSessionRow.tsx:14`:`import type { AgentSessionRow } from "./useAgentStatus";`
   5. ticker 移交宿主——`src/features/navTree/NavTree.tsx`,在 :193 `const nav = useNavTree();` 之后插入:
      ```tsx
      // CP-021:历史区相对时间 60s ticker——navTree 宿主单点持有,渲染层与数据层节奏解耦
      // (sessionRefresh 禁用/慢档时相对时间仍自动刷新);ticker 随 CP-026 返回面收窄
      // 自 useAgentStatus 移交至此
      const [now, setNow] = useState(() => Date.now());
      useEffect(() => {
        const timer = setInterval(() => setNow(Date.now()), 60_000);
        return () => clearInterval(timer);
      }, []);
      ```
      `renderHistory` 内 NavHistoryRow 渲染(:511-517)改为:
      ```tsx
      {model.history.sessions.map((session) => (
        <NavHistoryRow
          key={keyOf(session.cliId, session.sessionId)}
          session={session}
          status={nav.activeStatuses.get(keyOf(session.cliId, session.sessionId))}
          now={now}
          onDoubleClick={handleHistoryDoubleClick}
          onContextMenu={handleHistoryContextMenu}
        />
      ))}
      ```
   6. `src/features/navTree/NavHistoryRow.tsx`:
      - props 接口(:21-27)改为:
        ```ts
        interface NavHistoryRowProps {
          session: AgentHistorySession;
          /** 运行中会话四态(activeStatuses 复合键查询;null → 无圆点) */
          status?: AgentStatus | null;
          /** 相对时间基准(navTree 宿主 60s ticker 注入——CP-021,替换原内部 Date.now()) */
          now: number;
          onDoubleClick(session: AgentHistorySession): void;
          onContextMenu(session: AgentHistorySession, pos: { x: number; y: number }): void;
        }
        ```
      - 解构(:29-34)补 `now,`;:38 改为 `const timeStr = formatRelativeTime(session.mtimeMs, now);`
   7. 删除 `src/features/agentStatus/CLAUDE.md`(有效约束随迁,见文档同步),目录即空闭合。
4. **测试同步**:
   - `src/__tests__/agent-status-hook.test.ts`::144 import 改 `../features/navTree/useAgentStatus`;「状态机派生」组(:305-333)三用例改写为 rows-only 断言(`result.current` 现为数组);「now ticker」组(:335-387)三用例**删除**(ticker 已移交宿主);新增用例「返回面收窄回归(CP-026)」:`expect(Array.isArray(result.current)).toBe(true)` + `state`/`now`/`currentProjectName` 三键不存在;其余 `result.current.rows` 统一改 `result.current`(:402-1441 逐处适配)。
   - `src/__tests__/nav-tree.test.tsx`::51 mock 路径与 :138 type import 改指 `../features/navTree/useAgentStatus`;mock 形状收窄——删全部 `state: { kind: "ready" },` / `currentProjectName: ...,` / `now: ...,` 三行,保留 `rows`(resetAll :261-266 及 :307-310、:357、:400、:430、:551、:742、:988、:1329、:1367 共 10 处逐一适配)。
   - `src/__tests__/nav-tree-history.test.tsx`::35 mock 路径同上;:210-215 resetAll 形状改 `mockUseAgentStatus.mockReturnValue({ rows: [] })`——**注意**:收窄后返回值为数组本体,此处置应为 `mockUseAgentStatus.mockReturnValue([])`(执行时以收窄后真实签名校准);新增宿主 ticker 用例组「CP-021 宿主 60s ticker(渲染层与数据层节奏解耦)」:fake timers 下 base-5min 历史行渲染「5 分钟前」→ 推进 59s 不变 → 再推进 1s(累计 60s)变「6 分钟前」(完整代码块见 review-06 CP-026 步骤 4,`act` 自 `@testing-library/react` 导入需补)。
   - `src/__tests__/nav-history-row.test.tsx`:`renderRow`(:78-96)改签名,`now` 显式传入(props `{ status?: AgentStatus | null; now?: number }`,默认 `Date.now()`,完整代码块见 review-06);:107 与 :193 两处 `formatRelativeTime(session.mtimeMs, Date.now())` 改为用 renderRow 返回值 `now`;新增用例「rerender 传 now+60s → 相对时间文本随 prop 重算(无内部 Date.now)」(代码块见 review-06)。
5. **文档同步**:
   - 删除 `src/features/agentStatus/CLAUDE.md`;其有效约束随迁 `src/features/navTree/CLAUDE.md`——「层级与数据源」节后新增「活跃会话行数据 hook(useAgentStatus,CP-026 自 agentStatus 迁入)」节(行建模双/三通道、行 cliId MC-410/MC-205/ZQ-2、建行 status ZQ-3 决策 2、ContextUsage 信号分支 AC-5、行 title 动态跟随页签、FE-23 generation 防竞、项目域过滤、返回面契约——完整文本照抄 review-06 CP-026 步骤 5 首条)。
   - `src/features/navTree/CLAUDE.md`「硬约束」节「数据 hook 不自建订阅」条补:「相对时间 60s ticker 由 NavTree 宿主单点持有(CP-021,now 经 prop 注入 NavHistoryRow),数据 hook 不自建 ticker」。
   - `src/features/agentHistory/CLAUDE.md:89` MC-318 整条改写为:「1. **历史区相对时间刷新(CP-021 已修)**:`formatRelativeTime` 渲染时计算,相对时间基准 `now` 由 navTree 宿主 60s ticker 驱动重算——与 sessionRefresh 数据层节奏解耦,禁用/慢档不冻结。」(标题可保留作历史锚。)
   - `knip.json`:删除 `ignoreIssues` 中 `"src/features/agentStatus/useAgentStatus.ts": ["types"]` 条目(:22-24,含尾逗号行)。
6. **验证**:
   - `test -f src/features/navTree/useAgentStatus.ts && test ! -e src/features/agentStatus`(退出码 0)。
   - `grep -rn "features/agentStatus" src/ knip.json` 退出码 1(零命中)。
   - `grep -n "Date.now()" src/features/navTree/NavHistoryRow.tsx` 退出码 1(零命中)。
   - `grep -n "60_000" src/features/navTree/NavTree.tsx` ≥ 1 命中。
   - `grep -n "currentProjectName\|AgentStatusState\|AgentStatusResult" src/features/navTree/useAgentStatus.ts` 退出码 1(零命中)。
   - `npx tsc --noEmit` 绿;`npx eslint src/` 绿。
   - `npx vitest run agent-status-hook nav-tree nav-history-row` 全绿。
   - `npx knip` 无新增 issue(agentStatus 条目已删,AgentSessionRow 有生产消费)。

---

## CP-038+CP-027 · @types/markdown-it pin 改回 ^14.2.0 + 启动链 fail-safe 三处静态色构建期注入(原章一 CP-038 + 章六 CP-027,单 agent 串行改 package.json)

**CP-038(先执行,纯 pin)**:

1. **位置**:`package.json:67`(`"@types/markdown-it": "14.2.0",`);ADR-0006 依赖版本策略 `.claude/adr.md:143-159`(:152「devDependencies(开发工具)全 `^`」)
2. **现状**:pin 自 `78cb1b8`(md 渲染纯管线 S5,首次引入即精确形态 `14.2.0`)至今零改动;`git log -S '"@types/markdown-it": "^14.2.0"'` 全历史无命中,即**从未存在过 ^ 形态,也无 pin 成因登记**——无成因路径坐实(markdown-it 本体 15.0.1 与 @types 14.x 的 major 错位是 DefinitelyTyped 版本号常态,非 pin 理由)。
3. **修复步骤**:
   1. `package.json:67` 改为 `"@types/markdown-it": "^14.2.0",`
   2. `npm install` 刷新 lockfile(`^14.2.0` 解析至 14.2.x 最新;若上游已发 14.3.x 亦合法,ADR-0006 允许 dev `^` 浮动);
   3. 若刷新后 `npx tsc --noEmit` 报 @types/markdown-it 相关新错误 → **pin 成因浮现路径**:不改回,在 ADR-0006「后果」节补例外登记(形态照 :159 xterm 例外句:「例外登记:@types/markdown-it 精确 pin(类型定义与 markdown-it 15.x 不兼容,<错误摘要>),升级 markdown-it/types 时重估」),compromises.md CP-038 条目按「有成因」改写;
   4. 无错误 → 无需任何登记(改回即符合 ADR-0006 现状口径)。
4. **测试同步**:无新用例;**回归面** = L2 markdown 系六件全量:`src/__tests__/markdown-assets.test.ts`、`markdown-links.test.ts`、`markdown-mermaid.test.ts`、`markdown-panel.test.tsx`、`markdown-render-async.test.ts`、`markdown-render-pipeline.test.ts`。
5. **文档同步**:无(正常路径零文档变更);仅步骤 3 异常分支触发 ADR-0006 例外登记。
6. **验证**:`grep -n '"@types/markdown-it": "\^14.2.0"' package.json` 命中;`npm ls @types/markdown-it` 退出码 0 且版本 ∈ 14.2.x+;`npx tsc --noEmit` / `npx eslint src/` 退出码 0;`npx vitest run src/__tests__/markdown-` 六文件全绿。

**CP-027(后执行,scripts 段接线)**:

1. **位置**:
   - `index.html:10`:`<body style="margin: 0; padding: 0; overflow: hidden; background: #0a0a0b;">`
   - `src-tauri/tauri.conf.json:21`:`"backgroundColor": "#0a0a0b"`
   - `src/main.tsx:28-47`(fail-safe 页;:30 注释含「既定例外」字样;:36 `background = "#0a0a0b"`、:37 `color = "#ece9e4"`、:43 `messageSpan.style.color = "#d9706b"`)
   - 色源:`src/theme/schemes/linear.ts:74`(`appBgPrimary: "#0a0a0b"`)、:77(`sidebarFg: "#ece9e4"`)、:78(`errorFg: "#d9706b"`)
   - 登记/交叉引用待删:`linear.ts:6-11` 文件头 fail-safe 交叉引用块、`src/theme/CLAUDE.md:54` 与 `:62`、`main.tsx:30`、`.claude/adr.md` ADR-0002 :58 被否决备选、ADR-0003 :80 对接段
   - 打包脚本:`.claude/package.ps1:23`(`npx tauri build`——经 tauri.conf.json `beforeBuildCommand: npm run build` 触发 prebuild 钩子)
2. **现状**:三处硬编码与 linear.ts 现值一致(2026-09-06 核查实证);`linear.ts:6-11` 为交叉引用块(其中 `main.tsx:28` 为陈旧行号,实际色值行 :36-43);`main.tsx:30` 有「色值属启动链 fail-safe 既定例外」注释。
3. **修复步骤**:
   1. 新建 `src/theme/startupColors.ts`(构建期生成物,照 `src/panels/markdown/generated/katexInlineCss.ts` 先例入库):
      ```ts
      // startupColors.ts — 启动链 fail-safe 静态色(构建期生成物,勿手改)
      // 色源单一 = schemes/linear.ts(appBgPrimary/sidebarFg/errorFg)——生成器 scripts/sync-startup-colors.mjs(CP-027),
      // 经 package.json predev/prebuild 接线;改 linear.ts 三槽位后跑 npm run sync:startup-colors 即同步三处消费点。
      export const STARTUP_FAIL_SAFE_BG = "#0a0a0b";
      export const STARTUP_FAIL_SAFE_FG = "#ece9e4";
      export const STARTUP_FAIL_SAFE_ERROR_FG = "#d9706b";
      ```
   2. 新建生成器 `scripts/sync-startup-colors.mjs`(零依赖纯 Node ESM;`extractStartupColors` 锚定 `appBgPrimary/sidebarFg/errorFg` 三键小写 6 位 hex 正则提取,失败即抛错禁自估;`renderStartupColorsModule` 渲染生成物全文;主流程改写 index.html body background、tauri.conf.json backgroundColor、startupColors.ts 三处,`rewriteIfChanged` 幂等;`process.argv[1]` 门控仅直跑执行 main——**完整全文照抄 review-06 CP-027 步骤 3.2**)。
   3. `package.json` scripts 段(:6-18)追加三行(与 CP-038 同 agent 串行改同一文件):
      ```json
      "sync:startup-colors": "node scripts/sync-startup-colors.mjs",
      "predev": "npm run sync:startup-colors",
      "prebuild": "npm run sync:startup-colors",
      ```
      (npm 生命周期:predev 挂既有 `dev`、prebuild 挂既有 `build`;`npx tauri build` 经 `beforeBuildCommand` 触发 prebuild,`.claude/package.ps1` 与用户 `npx tauri build --debug --no-bundle` 习惯路径同链路覆盖。)
   4. `src/main.tsx`:顶部静态 import 追加 `import { STARTUP_FAIL_SAFE_BG, STARTUP_FAIL_SAFE_ERROR_FG, STARTUP_FAIL_SAFE_FG } from "./theme/startupColors";`(注释注明零依赖不触发 facade 求值);:30 注释改「色值经 startupColors 常量,构建期自 linear.ts 注入——CP-027」;:36/:37/:43 三处字面量替换为对应常量。
   5. 删交叉引用与例外登记:`linear.ts` 删文件头 :6-11 整块(保留 :1-5 与 :12-13);`src/theme/CLAUDE.md:54` 改写为构建期注入口径(「硬约束 #6 自此无例外」,文本照抄 review-06);:62 红线条删除,补「startupColors.ts 为生成物禁手改」一句;`.claude/adr.md` ADR-0002 :58 被否决备选整条改写为「2026-09 CP-027 修订:构建期通道成立——sync 脚本提取改写三处消费点,运行期仍不经 facade」、:63 后果条收敛为「react/react-dom/lib/e2eEnabled/theme/startupColors」、ADR-0003 :80 改为「构建期注入(CP-027)」。
4. **测试同步**:新增 `src/__tests__/startup-colors-sync.test.ts`(L2;import 生成器两导出,断言:linear.ts 现值三槽位提取、槽位缺失抛错、index.html/tauri.conf.json/startupColors.ts 三处与提取值一致、main.tsx 无三字面量且含 `STARTUP_FAIL_SAFE_BG`——完整代码块照抄 review-06 CP-027 步骤 4)。既有 `theme-scheme-registry.test.ts:160-161` 与 `theme-colors.test.ts` 原样通过即回归。
5. **文档同步**:见步骤 5;根 CLAUDE.md 硬约束 #6 文本不破(例外清单机制保留,清单归零)。
6. **验证**:
   - `npm run sync:startup-colors` 退出码 0 且输出「无改动」(幂等)。
   - 提取链路演练:临时改 `linear.ts` `appBgPrimary` 为 `#111111` → 跑 sync → 三处命中 → `git checkout -- .` 还原(演练后必须还原)。
   - `grep -n "#0a0a0b" index.html src-tauri/tauri.conf.json src/main.tsx` 仅前两文件各 1 命中、main.tsx 0 命中。
   - `grep -rn "既定例外" src/main.tsx src/theme/schemes/linear.ts` 退出码 1。
   - `grep -n "fail-safe" src/theme/CLAUDE.md` 命中为「无例外」新口径。
   - `npx vitest run startup-colors-sync` 全绿;`npx tsc --noEmit` 绿;`npx tauri build --debug --no-bundle` 成功(prebuild 触发,端到端验证)。

---

## CP-032 · wdio overrides 成因登记 + 「谁钉谁」对齐契约(原章一)

1. **位置**:`package.json:92-100`(overrides 段 7 条目);`package-lock.json:4796-4815`(@wdio/tauri-service@1.3.0 硬钉 `@wdio/* 9.29.1` + `webdriverio 9.30.0`);`.claude/adr.md:153`(ADR-0006 仅状态无成因);成因一手来源:`a027b17`(2026-08-18,TE-01/02/05/06/12 批,前 4 项)、`1233336`(2026-08-22,TE-06/07/14「WDIO dedupe」,后 3 项)。
2. **现状**:`npm ls` 实证——serialize-javascript ← mocha 10.8.2;deepmerge-ts ← @wdio/config/@wdio/utils/webdriver;@puppeteer/browsers ← @wdio/utils;glob ← @wdio/config/archiver-utils/mocha。家族解析:@wdio/globals 9.31.1 / webdriverio 9.30.1 顶层单实例(dedupe 生效),tauri-service 子树硬钉经 overrides 强扭对齐。
3. **修复步骤**:
   1. `e2e-tests/CLAUDE.md` 在「### E2E helper 命名与挂载位置」节(:27)之前插入「### wdio 版本矩阵与 overrides 对齐契约(CP-032)」新节——内容照抄 review-01 CP-032 步骤 3.1(真值源两处 = 主声明 + overrides;七 override 成因表:serialize-javascript `^7.0.5` 消 mocha RCE(a027b17)、deepmerge-ts `^8.0.1` dedupe(a027b17)、@puppeteer/browsers `^3.2.1` dedupe(a027b17)、glob `^10.5.0` dedupe(a027b17)、@wdio/globals `^9.31.0` 对齐 tauri-service 硬钉(1233336)、expect-webdriverio `^6.0.5` dedupe(1233336)、webdriverio `^9.30.1` 对齐硬钉(1233336);对齐契约三条:升 tauri-service 先 `npm view` 查硬钉再动 overrides、版本评审看两处 + `npm ls` 单实例即健康、前 4 项 wdio 升 major 时逐条重估)。
   2. **评估动作(当场执行)**:`npm view @wdio/tauri-service@latest version dependencies --json`——若 latest > 1.3.0 且硬钉已放开 → 升 tauri-service、删 overrides 中 `@wdio/globals`/`webdriverio` 两条、`npm install` 后 `npm ls webdriverio @wdio/globals` 断言单实例、跑 `npm run e2e` 全量;若 latest = 1.3.0 → 维持不动作,契约第 1 条成为后续同步点。
4. **测试同步**:纯登记 + 条件触发依赖变更;升级分支以 `npm run e2e` 全量为回归面,无新单测。
5. **文档同步**:`e2e-tests/CLAUDE.md` 新增节;`.claude/adr.md:153` 该句后追加「成因与『谁钉谁』对齐契约登记于 `e2e-tests/CLAUDE.md`(CP-032);上游放开硬钉后逐条去 overrides 化」;走升级分支时主 package.json 的 `@wdio/tauri-service` 版本号同步改。
6. **验证**:`grep -n "对齐契约" e2e-tests/CLAUDE.md` 命中且表格 7 行全列;`npm ls webdriverio @wdio/globals 2>&1` dedupe 单实例(退出码 0);未走升级分支时 `git diff --stat package.json package-lock.json` 为空;走升级分支时 `npm run e2e` 全绿。

---

## CP-001 · 双 TS 并存——触发条件登记硬化为可机检形式(原章一)

1. **位置**:`package.json:71`(`"@typescript/native": "npm:typescript@^7.0.2"`)、`:87`(`"typescript": "npm:@typescript/typescript6@^6.0.2"`);`.claude/adr.md:238` ADR-0010「TE-07 执行结果」段;`docs/compromises.md:19-20`。
2. **现状**:ADR-0010:238 触发条件为散文形式(「issue #10940 闭环 + TS7.1 稳定发布」),无可执行判定。2026-09-06 核查:issue #10940 仍 open、`typescript` dist-tags.latest = 7.0.x——双条件均未达成,本条**不动依赖**,只硬化登记。
3. **修复步骤**:
   1. 新建 `scripts/check-ts7-trigger.mjs`(零依赖,仅 node 内置 https;导出纯判定 `evaluateTrigger(issueState, latestVersion)`——issue closed 且 latest 匹配 `^7\.[1-9]\d*\.\d+$` 无预发布后缀才 triggered;`getJson` 极简 GET;main 双查询后按退出码 0=达成/1=未达成/2=查询失败;`process.argv[1]` 门控仅直跑执行——**完整全文照抄 review-01 CP-001 步骤 3.1**,注意 mjs 内写 `e.message` 而非 `(e as Error).message`)。
   2. `.claude/adr.md` TE-07 段(adr.md:238)末句替换为机检口径(「`node scripts/check-ts7-trigger.mjs` 退出码 0 即双条件达成…」完整文本照抄 review-01)。
   3. `docs/compromises.md` CP-001 条目「**修改方向**」句替换为机检脚本口径(照抄 review-01)。
4. **测试同步**:新建 `src/__tests__/deps-ts7-trigger.test.ts`,import `evaluateTrigger` 五用例:已闭环+7.1.0 → 达成;未闭环+7.1.0 → 不达成;已闭环+7.0.2 → 不达成;已闭环+7.1.0-beta.1 → 预发布不视为稳定;undefined/null 入参 → 不达成。既有用例零适配。
5. **文档同步**:见步骤 2/3;根 CLAUDE.md 静态门禁节不动。
6. **验证**:
   - `node scripts/check-ts7-trigger.mjs; echo $?` → 退出码 1 输出含 `未达成`(若实跑时上游已变,以退出码语义断言为准,不得为此改脚本)。
   - `npx vitest run src/__tests__/deps-ts7-trigger.test.ts` 全绿。
   - `grep -n "check-ts7-trigger" .claude/adr.md docs/compromises.md` 均命中。
   - `npx knip --production` 退出码 0(若报 unused file,在 knip.json `ignore` 数组追加 `"scripts/**"` 后重跑)。

---

# S02 L1 测试基建(1 项,豁免单一项)

## CP-040 · 默认 lib test target 0xc0000139——经 embed-manifest 注入 comctl32 v6 manifest,拆 [lib] test=false 重组与 TQ-COV-06 红线(原章五)

1. **位置**:
   - `src-tauri/Cargo.toml:10-25`——`[lib] test = false`(:16)+ F12 注释块(:17-21)+ `[[test]] lib_tests path = "src/lib.rs"`(:23-25)
   - `src-tauri/build.rs:2-16`——`cargo:rustc-link-arg-tests=/MANIFEST:EMBED`(:12)+ `/MANIFESTINPUT:tests-comctl6.manifest`(:13-16)+ `rerun-if-changed=icons`(:11)
   - `src-tauri/tests-comctl6.manifest`——comctl32 v6 依赖清单(现经 link-arg-tests 嵌入)
   - `.claude/test-exemptions.md:29`——~~cargo test 门禁~~ 已修复(2026-08-31 翻案)整行
   - 根 `CLAUDE.md:77`——「L1 定向测试红线(TQ-COV-06)」整条
   - `.cargo/config.toml`——**不存在**(Glob 零命中;rustflags 通道评估见步骤 2)
   - 波及引用:`src-tauri/src/lib.rs:157`(`mod lib_tests` 内嵌测试模块,与已删 [[test]] 目标同名,仅名字偶合,不动)、`src-tauri/src/git/CLAUDE.md:74`(命令壳测试 TQ-COV-06 标签)、`src-tauri/tests/*.rs` 头部 TQ-COV-06 注释标签
2. **现状**:根 CLAUDE.md:77 原文:「`cargo test` 带 filter 或 `--lib` 会绕过 `[lib] test=false` 重建默认 lib 单测 target——该 target 收不到 build.rs `rustc-link-arg-tests` 的 comctl32 v6 manifest(SxS),静态导入的 `TaskDialogIndirect` 解析到 v5 → 启动即 0xc0000139…定向测试一律 `cargo test --test lib_tests <filter> -- --test-threads=1`」。2026-08-31 全量实测 827 例(711 lib + 116 集成)全绿——link-arg-tests 通道对显式 [[test]] 与集成目标生效,唯独默认 lib test target 收不到(cargo 行为,与代码/环境无关)。
3. **修复步骤**(通道二选一已评估定案,**写死 embed-manifest 方案**):
   1. **方案评估结论(理由写死)**:
      - `.cargo/config.toml` `[build] rustflags = [...]`:**拒绝**——rustflags 作用于全部构建产物(bin/cdylib/staticlib 与全部 test target),主 exe 的 tauri 生成 manifest 会被链接参数覆盖(clobber DPI/兼容性声明),且无法区分 test/非 test 目标;`.cargo/config.toml` 当前不存在,为它引入全局副作用不值。
      - **embed-manifest crate:采用**——`embed_manifest::embed_manifest_file!` 经 `#[link_section = ".rsrc"]` 直接把 RT_MANIFEST 资源编进调用处所在产物:放在 `src/lib.rs` 且以 `#[cfg(all(windows, test))]` 门控后,仅 lib 单测 target 编译该代码 → 只有 lib 单测 exe 获得 comctl6 manifest;bin/cdylib/集成测试不编译 cfg(test) 代码,零副作用;不依赖 `rustc-link-arg-tests` 的 cargo target 分类行为,默认 lib test target 天然覆盖。
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
4. **测试同步**:零新增/零删除测试用例(本条为构建通道修复);lib 内嵌单测(含 `src/lib.rs:157 mod lib_tests`)与 `src-tauri/tests/*.rs` 六件原样。**计数等价断言**:改造前 `cargo test -- --test-threads=1` 全量计数 N1(应 = 827 或实测当前值),改造后 N2 必须 **N2 == N1 且全绿**。既有用例适配:无;`git_command_shell_tests.rs` 等头部 TQ-COV-06 注释标签保留(历史出处标签)。
5. **文档同步**:根 `CLAUDE.md:77` 删红线 bullet(「测试策略」表 L1 行命令不变);`.claude/test-exemptions.md` 删 :29 行 + 补销记;`src-tauri/Cargo.toml`/`src-tauri/build.rs`/`src-tauri/tests-comctl6.manifest` 头注按步骤 3.2/3.5 改口径(manifest 文件头注补「lib 单测经 embed-manifest 引用本文件;集成 test target 经 build.rs link-arg 引用」);`src-tauri/src/git/CLAUDE.md:74`「命令壳测试(TQ-COV-06)」标签行保留标签文字,补半句「(target 形态已复原,S02)」。**跨章一致性**:本清单 S01/S02 之前条目内 `cargo test --test lib_tests <filter>` 形态随本条失效,后续 Stage 一律按等价形态 `cargo test <filter> -- --test-threads=1` 适配——由 Stage 编排统一定调,不回改已起草清单。
6. **验证**(机械断言):
   - `cargo test --lib -- --test-threads=1` exit 0(默认 lib target 带 filter/--lib 形态,原 0xc0000139 通道)。
   - `cargo test validate_spawn_request -- --test-threads=1` exit 0(带 filter 定向形态)。
   - `cargo test -- --test-threads=1` 全量 exit 0 且计数 = N1。
   - `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` exit 0;`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` exit 0。
   - `grep -n "test = false\|\[\[test\]\]" src-tauri/Cargo.toml` 零命中;`grep -n "lib_tests" .claude/CLAUDE.md` 零命中;`grep -c "lib_tests" .claude/test-exemptions.md` = 0。
   - 主 exe manifest 无冲突:`npx tauri build --debug --no-bundle` 成功且产物启动正常(人工一次,窗口正常出现)。

---

# S03 e2e 基建族(7 项,定序:① 合并体 → ② CP-029;CP-030/028/041 与 ② 并行但注意 mockcli.e2e.ts 共碰)

**Stage 内定序与冲突约定**:
- **① 先行**:CP-003+CP-045+CP-046(run-wdio.cjs 单 agent)——Node 26 工具链落地后,② CP-029 在新工具链上二分定责(防环境变量污染定责结论)。
- **CP-028 与 CP-041 同改 mockcli.e2e.ts**——须同 agent 串行或明确前后序(建议 CP-028 先,CP-041 复用其单次确定性展开辅助形态)。
- **CP-041 亦改 run-wdio.cjs**(mockcli fixture 块追加于 claude 块后)——紧随 ① 合并体之后提交,避免二次冲突。
- **CP-029 若定责为环境出口**,改 run-wdio.cjs 注入方式,并入 run-wdio.cjs 变更序列尾部。
- **人工验证点**:CP-003 需全量 e2e 在 Node 26 实跑(时间盒);CP-029 定责结论需记录。

## CP-003+CP-045+CP-046 · run-wdio.cjs 合并体:删便携 Node 22 自动下载 + 假屋文档销项 + 真实屋校验键级断言(原章一 CP-003 + 章五 CP-045/046,单 agent)

**CP-003 · 删便携 Node 22 自动下载,Node 26 直跑 + engines 纳入主 toolchain**:

1. **位置**:`e2e-tests/run-wdio.cjs:246-288`(自动下载分支,含 :286-288 `else { fallback(); }`)、`:1-4`(文件头注释)、`:27`(`const https = require('https');`);`package.json:68`(`"@types/node": "^26.2.0"`)、无 `engines` 字段;`e2e-tests/CLAUDE.md:23-25`(「Node 版本兼容启动器」节);`e2e-tests/wdio.conf.ts:5`(文件头注释)。
2. **现状**(run-wdio.cjs:246-288 摘录):
   ```js
   if (major >= 26) {
     const nodeDir = path.resolve(__dirname, '..', '.temp', 'node22');
     const node22 = path.join(nodeDir, 'node.exe');
     // E2E-13①:便携 Node 22 预置 .temp/node22 或 CI 固定 Node 22 时跳过外网下载。
     if (fs.existsSync(node22)) { ... runWdio(node22); process.exit(0); ... }
     // 自动下载便携 Node 22
     console.log('[wdio-launcher] 下载便携 Node 22 (约 30MB)...');
     ... https.get(url, (res) => { ... 下载 v22.21.1 ... });
   } else { fallback(); }
   ```
   webdriverio#15265(Node 26 undici 8 不兼容)已于 9.30.0 修复,项目解析 9.30.1(package.json:99 overrides / lockfile 实证)——自动下载分支成为「无人触发的死兜底」。
3. **修复步骤**:
   1. **门禁实证(先行,失败即走分支 B)**:确保 `.temp/node22` 不存在(存在则临时改名 `node22.bak`,验证后删除),确认 `node --version` 主版本 ≥ 26;完成步骤 2-6 后 `npm run e2e` 全量跑通。全绿 → 继续;出现 tauri-service/undici 类 Node 26 证据 → **回滚本步骤全部改动**,把新证据登记进 `e2e-tests/CLAUDE.md`(改成「Node 26 直跑因 <新证据> 仍不可行,自动下载分支保留」)与 compromises.md CP-003 条目,本条转休眠。
   2. `run-wdio.cjs:246-288` 整块替换为(显式预置约定保留,自动下载删除):
      ```js
      if (major >= 26) {
        const nodeDir = path.resolve(__dirname, '..', '.temp', 'node22');
        const node22 = path.join(nodeDir, 'node.exe');

        // 显式预置约定(E2E-13①):.temp/node22 存在且 > 1MB 时强制切便携 Node 22
        // (判活只看大小,防中断残留的损坏文件被误用);不自动下载——
        // webdriverio 9.30.0 已修复 Node 26 undici 8 兼容(webdriverio#15265,CP-003)。
        if (fs.existsSync(node22)) {
          let size = 0;
          try { size = fs.statSync(node22).size; } catch { size = 0; }
          if (size > 1024 * 1024) {
            console.log(`[wdio-launcher] Node ${process.version} → 使用便携 Node 22`);
            runWdio(node22);
            process.exit(0);
          }
          console.warn('[wdio-launcher] 便携 Node 22 文件不完整(<1MB),改用当前 Node');
        }
      }
      fallback();
      ```
   3. `run-wdio.cjs:27` 删 `const https = require('https');`(全文件仅下载分支两处消费)。
   4. `run-wdio.cjs:1-4` 文件头注释替换为:「WDIO 启动器(CP-003):webdriverio 9.30.0 已修复 Node 26 undici 8 兼容(webdriverio#15265),Node >= 22 直跑;.temp/node22 显式预置便携 Node 22 时优先。CI 固定 Node 22(见 ci.yml)。」(以下数据隔离段原文保留不动)
   5. `package.json` 在 `"scripts"` 块之后、`"dependencies"` 之前插入:`"engines": { "node": ">=22" },`
   6. `e2e-tests/CLAUDE.md:23-25` 节整体替换为「### Node 版本(CP-003)」节(文本照抄 review-01 CP-003 步骤 3.6:Node >= 22 直跑 + 显式预置不自动下载 + engines 纳入主 toolchain)。
   7. `e2e-tests/wdio.conf.ts:5` 注释替换为:`// Node >= 22 直跑(run-wdio.cjs,webdriverio 9.30.0+ 修复 Node 26 兼容),CI 固定 Node 22。`
   8. 若步骤 1 曾改名 `.temp/node22`,验证通过后删除 `.temp/node22.bak` 与 `.temp/node22`(存在时)。
4. **测试同步**:run-wdio.cjs 为进程编排壳,无可单元化点;**全量 e2e 在 Node 26 下跑通即防复发验证**,按纪律 11 在 `.claude/test-exemptions.md` 登记豁免:「run-wdio.cjs 启动器分支——进程编排无单测锚点,由 L4 全量 e2e(Node 26 直跑)兜底,豁免原因:spawn 外部进程行为不可 jsdom 化」。既有用例零适配。
5. **文档同步**:`e2e-tests/CLAUDE.md` :23-25 按步骤 6;`:42` glyph-repro 节内「Node 26→便携 22 切换兜底,裸 `npx wdio` 会踩版本坑」改为「Node >= 22 直跑,裸 `npx wdio` 缺启动器的数据/假屋隔离链(SLTERM_DATA_DIR/USERPROFILE),禁止绕过启动器」(理由从版本坑换为隔离链);`wdio.conf.ts:5` 按步骤 7;`run-wdio.cjs:1-4` 按步骤 4;`.claude/test-exemptions.md` 按步骤 4 追加豁免行。
6. **验证**:
   - `node --version` → v26.x;`.temp/node22` 不存在时 `npm run e2e` 全绿(含 terminal.e2e.ts 末位杀 app 用例),输出无 `下载便携 Node 22` 字样。
   - `grep -n "nodejs.org\|https\.get\|require('https')" e2e-tests/run-wdio.cjs` → 零命中。
   - `npm pkg get engines` → `{"node":">=22"}`;`npm ls @types/node` → `@26.x`。
   - `npx tsc --noEmit` / `npx eslint src/` 退出码 0。

**CP-045 · e2e 假屋清理残留——文档同步销项(代码已是 per-pid 唯一命名)**:

1. **位置**:`e2e-tests/run-wdio.cjs:44`(`slterm-e2e-home-${process.pid}` per-pid 唯一命名,评审实证)、`:39-43`(唯一名免清空语义注释块);**漂移点** `run-wdio.cjs:13` 头注仍写旧固定名;`e2e-tests/CLAUDE.md:48` 仍写固定名;`.claude/test-exemptions.md:69` 已写 `<pid>`(正确,不动)。
2. **现状**:run-wdio.cjs:40-43 注释载明「假屋目录每次运行唯一(pid 后缀):IME/遥测组件…长驻句柄——固定名假屋的启动清空必撞 EPERM。唯一名免清空;exit 清理 best-effort,残留目录留在 tmp 由 OS 回收」——「按运行实例唯一命名」已落地;唯一残留 = 两处文档固定名表述。
3. **修复步骤**(纯文档/注释):
   1. `run-wdio.cjs:13` 头注改写为:「启动时建临时假屋(`os.tmpdir()/slterm-e2e-home-<pid>`,per-pid 唯一——IME/遥测句柄占用根因见 :39-43 注释)并把 USERPROFILE 指向它」。
   2. `e2e-tests/CLAUDE.md:48` 句中「`<os.tmpdir()>/slterm-e2e-home`」改写为「`<os.tmpdir()>/slterm-e2e-home-<pid>`(per-pid 唯一名,免启动清空——句柄占用残留无害,OS 回收)」。
   3. 销项:`docs/compromises.md` CP-045 勾选 `[x]` 并按 CP-015 先例补修复销项注记(「run-wdio.cjs:44 已是 per-pid 唯一命名(评审实证);残留 = 头注 + e2e-tests/CLAUDE.md:48 固定名表述,已同步」)。
   4. 不改动任何清理逻辑(44/168-171 行原样)。
4. **测试同步**:无(纯文档);L4 全量回归一轮即可。
5. **文档同步**:即步骤 1-3 本身;`e2e-tests/CLAUDE.md` 其它节(防复发校验 :50)不涉及假屋命名,不动。
6. **验证**:
   - `grep -rn "slterm-e2e-home" e2e-tests/CLAUDE.md e2e-tests/run-wdio.cjs .claude/test-exemptions.md` 所有命中均带 pid/唯一名语义。
   - `node -e "const s=require('fs').readFileSync('e2e-tests/run-wdio.cjs','utf8'); if(/slterm-e2e-home(?!-)/.test(s.replace(/\$\{process\.pid\}/,''))) process.exit(1)"` exit 0。
   - `npm run e2e` 一轮全绿(注释改动不影响运行,顺带验证)。

**CP-046 · e2e 真实屋校验并发误报——settings.json 键级断言改写**:

1. **位置**:`e2e-tests/run-wdio.cjs:66-75`(`snapFile` 整文件 sha256)、`:78-98`(`snapDir`)、`:100-108`(`snapshotUserHome`)、`:110-139`(`verifyRealHomeUnchanged`);`e2e-tests/CLAUDE.md:50`(防复发校验节)、`:54`(已知并发误报面节);`run-wdio.cjs:17-21` 头注。
2. **现状**:verifyRealHomeUnchanged:118-127 对 `~/.claude/settings.json` 与 `~/.slterminal/statusline-backup.json` 整文件 sha 前后相等判定;:128-132 hooks/ 目录整树相等;:134-137 hooks-events 存在性校验。误报面(CLAUDE.md:54):开发者并发跑真实 claude/slterminal 合法改写 settings.json → exit 校验报红;报红被习惯性忽略后真回归也会被放过。
3. **修复步骤**(run-wdio.cjs:66-139 改写):
   1. **哨兵键集合与键级快照**(新增,替换 settings.json 整文件 snap):
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
   2. **snapshotUserHome 改写**(:101-108):`claudeSettings: snapFile(...)` 改为 `claudeSettings: snapSettingsSentinels(path.join(realHome, '.claude', 'settings.json'))`;`statuslineBackup`/`hooksDir`/`hooksEventsExisted` 三字段保留原 snap 形态(非用户高频并发改写面,维持文件级/目录级)。
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
4. **测试同步**:L4 层无新用例(校验属 runner 退出路径);**人工负向验证一次**:临时在 exit 校验前向真实屋 settings.json 写入 `hooks` 键(测试后立即撤销)跑一次单 spec → 退出码 1 且 stderr 含「哨兵键 "hooks"」;再删除该键复跑 → 通过。验证完撤销临时代码。
5. **文档同步**:`e2e-tests/CLAUDE.md:50` 改写为哨兵键级口径(键 = hooks/statusLine;statusline-backup/hooks 目录维持文件级/整树 sha256;hooks-events 存在性校验保留;外部并发改写其它键不再报红);`:54` 改写为「已收窄(CP-046)…并发 hooks 注入/卸载仍会命中哨兵键报红——属真实泄漏信号,排查方向照旧」;`run-wdio.cjs:17-21` 头注同步一句键级口径。
6. **验证**:
   - 机检:`grep -n "SETTINGS_SENTINEL_KEYS\|snapSettingsSentinels" e2e-tests/run-wdio.cjs | wc -l` ≥ 4;`grep -n "expectFile(" e2e-tests/run-wdio.cjs` 命中 = 1(仅 statuslineBackup)。
   - 正向:`node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts`(任一短 spec)exit 0,日志含「真实用户目录校验通过(零接触)」。
   - 负向:人工注入验证记录 exit 1 + stderr 哨兵键文案。
   - 文档:`grep "哨兵键" e2e-tests/CLAUDE.md` ≥ 2 命中(:50/:54 两节)。

---

## CP-029 · editor.e2e.ts auto-reload 用例多轮确定性失败——二分定责后修环境或修产品(原章五,定序②:CP-003 合并体落地后)

1. **位置**:
   - `e2e-tests/editor.e2e.ts:283-386`——用例 "should persist modified content to disk after external change triggers reload then Ctrl+S save"(步骤 6 外部写盘 :324,步骤 7 等 auto-reload :327-337,**15s 超时确定性失败**)
   - 前端 reload 链:`src/panels/editor/useCodeMirror.ts:426-504`(`onFsEvent` → 路径规范化比较 :441-446 → kind==="Modify" 过滤 :449 → 无 dirty 自动重载 :484-503;`justSavedRef` 自写抑制 :431-437)
   - watcher 注册链:`src/workspace/Workspace.tsx:238-269`(SEC-01 effect,FE-38 `setProjectRoot` 成功后才 `startWatch`,`:262` `void startWatch(targetRoot)`)
   - 后端链:`src-tauri/src/notify/mod.rs:355-418`(notify_watch 三阶段)、`:491+`(L1 notify_tests,L1 同机通过是既定事实);`src-tauri/src/notify/pool.rs`(LruWatcherPool)
   - E2E 环境注入:`e2e-tests/run-wdio.cjs:34-37`(SLTERM_DATA_DIR)、`:44-47`(假 home USERPROFILE per-pid)
   - 归因冲突登记:`.claude/test-exemptions.md:24`(定性「Windows notify 环境级故障,非代码缺陷」)↔ `docs/compromises.md:105-106` CP-029(定性「待专项排查」,以本条为准)
2. **现状**:用例保持启用,每轮确定性失败并消耗 mocha retry(run-wdio 默认 retries=1)——失败信号被重试稀释。豁免表 :24 归因「同机 L1 notify 测试通过,页面内写入不产生 fs-event」,承诺「修复环境后复跑验收」未兑现。Workspace.tsx:235 注释「E2E editor auto-reload 失败根因修复」指 SEC-01 上提已完成,但失败仍在——上提不是根因或根因未除尽。
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
   3. **探针 B(watcher 注册态)**:同 spec 内追加——`await browser.execute((p) => window.__TAURI_INTERNALS__.core.invoke("notify_watch", { path: p }), dir)` 手动注册后再次写文件:手动注册后**收到** → SEC-01 自动注册链在 E2E 环境未生效 → 按产品缺陷修 Workspace.tsx:238-269(startWatch 失败静默 `void`——改挂 `.catch(err => console.error + toast)` 后再二分);手动注册仍**收不到** → 后端 watcher 在 E2E 进程环境失效 → 探针 C。
   4. **探针 C(环境消融)**:同机**不经 run-wdio** 手动起 `target/debug/slterminal.exe`(普通构建,不设 SLTERM_DATA_DIR/假 home)重复「建项目于临时目录 → 外部改文件 → 编辑器 auto-reload」人工验证:手动通过 → E2E 环境注入致失效;逐次消融:仅设 SLTERM_DATA_DIR → 加假 USERPROFILE → 加 SLTERM_CLAUDE_PROJECTS_DIR,定位令投递失效的注入项;手动亦失败 → 产品缺陷,回步骤 3 产品链修。
   5. **定责收尾(写死两条出口)**:
      - **环境缺陷出口**:修 run-wdio.cjs 注入方式(按消融定位的项);用例保持启用;**修订 test-exemptions.md:24** 归因行为实际根因(删「Windows notify 环境级故障」笼统措辞,写实测根因 + 修复方式 + 「复跑验收:YYYY-MM-DD WDIO_RETRIES=0 连续 3 轮通过」);销 CP-029 登记。
      - **产品缺陷出口**:修 useCodeMirror.ts:426-504 或 Workspace.tsx:238-269 实际缺陷;**删除 test-exemptions.md:24 豁免行**;按 bugfix 纪律补 L2 回归用例(use-code-mirror-reload 系,对照改动前老代码);销 CP-029 登记。
   6. 修后用例原文不改(editor.e2e.ts:283-386),复跑 `WDIO_RETRIES=0` 连续 3 轮全绿;再跑全量 `npm run e2e` 绿。
4. **测试同步**:editor.e2e.ts:283 用例**保持启用零改动**;探针 spec 取证完删除,不入库、不进 wdio.conf 终态;产品缺陷出口新增 L2 回归用例落 `use-code-mirror-reload-error.test.ts` 或 `editor-confirm.test.ts`(按实际缺陷点二选一);环境出口无新用例;notify L1 用例零改动——不得为迁就 E2E 改后端 notify 语义。
5. **文档同步**:`.claude/test-exemptions.md:24` 按两出口之一改写(环境出口:留行但归因改写;产品出口:删行);`e2e-tests/CLAUDE.md`「失败排查提示」节若根因属环境注入补一条注记;`docs/compromises.md` CP-029 销项勾选 + 一行修复注记(定责结论 + 修复面)。
6. **验证**:
   - `WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec editor.e2e.ts` 连续 3 轮 exit 0。
   - `npm run e2e` 全绿。
   - 环境出口:`grep "Windows notify 环境级故障" .claude/test-exemptions.md` 零命中;产品出口:`grep "editor.e2e.ts" .claude/test-exemptions.md` 零命中。
   - 探针清理:`ls e2e-tests/probe-fsevent.e2e.ts` 不存在;`grep probe e2e-tests/wdio.conf.ts` 零命中。

---

## CP-030 · tauri-service 焦点检查 +5-15s——beforeSuite 聚焦探针 fast-fail + cli-aliases 真实手势断言回归(原章五)

1. **位置**:
   - `e2e-tests/wdio.conf.ts:69-81`——beforeSuite(现有双 reset,TQ-E-08)
   - tauri-service 本地源码实证:`node_modules/@wdio/tauri-service/dist/cjs/index.js:3037`(`focusCommands = ['getTitle','findElement','findElements','$','$$','elementClick']`)、`:3133`(`core.invoke not available after 5s timeout`)、`:2987-2990`(getWindowStates 经 `plugin:wdio|get_window_states`)
   - 回退后的合成驱动:`e2e-tests/cli-aliases.e2e.ts:77-88`(setInputValue execute 合成)、`:184-256`(用例主流程,添加/删除全走 `browser.execute` 内 `.click()`)
   - 交互时序语义源:`src/panels/settings/pages/CliAliasesPage.tsx:174-179`(blur 不清空不提交竞态)、`:58-72`(成功添加后清空输入)
   - 登记点:`e2e-tests/CLAUDE.md` 外部坑节(focusCommands 条目)、「合成 JS click 无焦点语义」节
2. **现状**:e2e CLAUDE.md 实测登记:cli-aliases 真实手势版步骤 2 四个 focus 命令吃 40-60s,长链用例被拖出 mocha 60s 上限多轮失败——上游 1.3.0(2026-08-03)即最新,focusCommands 纯 WARN 无失败语义。现 spec 全部合成驱动:fC blur→click 竞态(:174-179 修复的目标)在 L4 侧零覆盖。
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
      - 输入(:202-204)合成 `setInputValue` → 元素命令:`await $('[data-e2e="cli-aliases-input-claude"]').setValue("cc");`
      - 添加钮(:205-207)→ `await $('[data-e2e="cli-aliases-add-claude"]').click();`
      - chip 出现等待(:208-214)保持 execute 轮询(executeScript 豁免 focusCommands,不动)
      - 删除钮(:260-262)→ `await $('[data-e2e="cli-aliases-remove-claude-cc"]').click();`
      - **恢复交互时序断言(紧跟添加钮点击之后、落盘等待之前)**:
        ```ts
        // 交互时序断言(CP-030 回归):真实指针序列(mousedown→input blur→mouseup→click)
        // 驱动——blur 不清空(CliAliasesPage blur 语义)+ 成功提交清空输入两语义同时落位。
        const inputAfterAdd = await $('[data-e2e="cli-aliases-input-claude"]');
        expect(await inputAfterAdd.getValue()).toBe("");
        ```
   3. **不 fork tauri-service、不改 node_modules**(锁定决策);其余 spec 不动(execute 内 helper 优先策略不变)。
4. **测试同步**:所有 spec 的 beforeSuite 自动获得探针(wdio.conf 单点);探针自身无独立测试(行为 = 失焦时 beforeSuite 抛错,人工验证一次:故意 Alt-Tab 离窗跑单 spec,应首条用例前报错退出);cli-aliases.e2e.ts 用例名不变,内部驱动换真实手势;L2 `settings-cli-aliases.test.tsx` 首条用例(FC-01 blur 编排先例)零改动——L4 版与其构成同源双锁;`mockcli.e2e.ts`/hooks 系 spec 的 execute 内 .click() 不强制升级。
5. **文档同步**:`e2e-tests/CLAUDE.md` 外部坑节 focusCommands 条目补「运行前提 = 窗口前台聚焦,wdio.conf beforeSuite TQ-E-10 探针 fast-fail 保证;前提满足后 $ 族命令正常速度,cli-aliases 已回归真实手势」;「合成 JS click 无焦点语义」节补「alias 添加链例外——blur→click 竞态经真实 elementClick 覆盖(CP-030),其余焦点类竞态仍归 L2」。
6. **验证**:
   - `npm run e2e` 全绿(cli-aliases spec 无 5s 级命令延迟;日志无 `core.invoke not available after 5s timeout` WARN)。
   - 探针负向验证(一次):跑 e2e 时 Alt-Tab 使 slTerminal 失焦 → beforeSuite 抛错、进程非零退出。
   - `WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec cli-aliases.e2e.ts` 1 轮 exit 0。
   - `npx tsc --noEmit` exit 0。

---

## CP-028 · e2e 导航树展开循环奇偶风险——NavTree 暴露 aria-expanded 探针 + 两处 6 轮循环收敛为单次确定性操作(原章五)

1. **位置**:
   - `e2e-tests/agent.e2e.ts:57-110`——`ensureTreeExpanded`,6 轮循环(62-109),children 计数判定
   - `e2e-tests/mockcli.e2e.ts:206-255`——CS-3 用例①内联 6 轮循环(209-255),注释自认「奇数次翻转必然到达展开稳态」
   - `e2e-tests/history.e2e.ts:79-118`——`ensureProjectPagesExpanded` **已改单次点击版(先例)**
   - 行组件:`src/features/navTree/NavProjectRow.tsx:47-49`(行根 div `data-e2e="nav-row-project"`)、`src/features/navTree/NavPageRow.tsx:69-71`(`nav-row-page`)、`src/features/navTree/NavHistoryNode.tsx:38-40`(`nav-history-node`)
   - 展开态真值:`src/features/navTree/useNavTree.ts:103-104`(`expanded`/`expandedHist` 两 Set)、`:175-191`(`toggleExpand`/`toggleHist`);`NavTree.tsx:430/494/528` 传 `expanded` prop
2. **现状**:agent.e2e.ts:89-94 注释:「页面行无会话时展开不渲染子级容器(DOM 无变化),故以项目展开为统一收敛点;…每轮点击各自提交后奇数次翻转必然到达展开稳态」——**无会话页面行的展开态 DOM 不可判**,靠翻转次数奇偶假设;用例结构变动即可能偶数翻转终态收起。行组件当前无任何展开态属性可探。
3. **修复步骤**:
   1. **NavTree 暴露 aria-expanded 探针**(三处行根):`NavProjectRow.tsx` 行根 div(:47-49)加 `aria-expanded={expanded}`(props 已有,直接引用);`NavPageRow.tsx` 行根(:69-71)同;`NavHistoryNode.tsx` 节点根(:38-40)同。语义 = 与 chevron 方向同源(同一 `expanded` prop),不新增状态;搜索态下属性随渲染态如实反映。
   2. **agent.e2e.ts `ensureTreeExpanded` 整体替换为单次确定性版**(照 history.e2e.ts:79-118 先例;单 execute 内只点 `aria-expanded !== "true"` 的项目行(含「当前」pill 容器)+ 其容器内全部未展开页面行,每行至多一次点击;随后 `browser.waitUntil`(5s/100ms)断言项目行与全部页面行 aria-expanded === "true"——**完整代码块照抄 review-05 CP-028 步骤 3.2**);原函数体(57-110)及顶部「展开态判定(DOM 结构)」长注释一并删除。
   3. **mockcli.e2e.ts 用例①内联循环(206-255)替换为同构单次版**(操作域 = 含「当前」pill 的项目容器);原 6 轮循环与「奇数次翻转」注释删除。
   4. 不改 `useNavTree` 状态语义、不改 NavTree 渲染结构;`waitForSessionRow`(agent.e2e.ts:117-156)内调用点不变。
4. **测试同步**:L2 `nav-tree.test.tsx`/`nav-tree-history.test.tsx` 各补一条 `aria-expanded` 初始 `"false"` 与点击后 `"true"` 断言(**建议新增**,非阻塞);L4 适配用例(全绿即过):agent.e2e.ts「nav 视图可通过活动栏按钮打开」「纯 shell 终端无活跃会话行」「动态四态」「R2 变体」「R3 变体」「R4 变体」;mockcli.e2e.ts CS-3 用例①;history.e2e.ts 回归(应原样绿——红了即探针语义错误)。
5. **文档同步**:`src/features/navTree/CLAUDE.md`「数据属性契约(写死)」节补:「行根/历史节点根挂 `aria-expanded`(展开态探针,与 chevron 同源,E2E 契约 CP-028)——值 ∈ "true"/"false",禁移除」。
6. **验证**:
   - `grep -c "aria-expanded" src/features/navTree/NavProjectRow.tsx src/features/navTree/NavPageRow.tsx src/features/navTree/NavHistoryNode.tsx` 各 ≥ 1。
   - `grep -c "for (let i = 0; i < 6" e2e-tests/agent.e2e.ts e2e-tests/mockcli.e2e.ts` 各 = 0。
   - `npm run e2e` 全绿;重点 `WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec agent.e2e.ts` 与 `--spec mockcli.e2e.ts` 各 1 轮 exit 0。
   - `npx tsc --noEmit` exit 0。
   - **附注**:history.e2e.ts:123-148 `ensureAllProjectsExpanded` 第三处 6 轮循环不在 CP-028 登记范围(项目行展开恒渲染子容器,DOM 可判、奇偶风险面不同);建议同 agent 顺手同改(单次 aria-expanded 版),不改不算本条未完成。

---

## CP-041 · mockcli 无后端 history provider——补 L4 专用测试 provider(env 门控注册),豁免两行销项(原章五;紧随 ① 合并体之后提交,与 CP-028 同改 mockcli.e2e.ts 须串行)

1. **位置**:
   - `.claude/test-exemptions.md:21-22`——两条豁免:「mockcli 历史条目展示(L4)」「mockcli 双击恢复注入(L4)」,兜底 L2 AC-4③/⑤
   - 后端注册表:`src-tauri/src/agent_history/provider.rs:46-62`(`static REGISTRY`、`resolve_provider` 未知 cliId → Validation)
   - 聚合层:`src-tauri/src/agent_history/mod.rs:20-26`(mod 声明)、`:114-133`(run_scan/is_claude_provider 遍历 REGISTRY)
   - claude 扫描根 env 先例:`SLTERM_CLAUDE_PROJECTS_DIR`(claude/scan.rs `resolve_projects_root` 内部自管,MC-305)
   - 前端扫描面:`src/features/backgroundTasks/sessionRefreshTask.ts:27` 遍历全部 history 能力 profile 逐个 scan——mockcli profile(helpers.ts:580-612)声明 history 能力,E2E 中每 tick 对 mockcli 发 scan
   - 前端恢复桩:`e2e-tests/helpers.ts:604-610`(buildResumeCommand/buildRestoreInput,输出 `mockcli --resume <id>`);L4 恢复断言先例 `e2e-tests/history.e2e.ts:535-596`
   - run-wdio fixture 通道:`e2e-tests/run-wdio.cjs:181-228`(claude-projects 副本 + 占位符替换 + 缺失即 exit(1))
2. **现状**:E2E 中 `agent_history_scan("mockcli", …)` 恒 Validation「未知 cliId: mockcli」→ sessionRefreshTask 聚合/mockcli 历史行/双击恢复链全断,两条豁免以此为唯一出口。mockcli.e2e.ts 用例② 依赖「未知 cliId: mockcli」错误透传(hooks 配置写命令)——该错误出自 **hooks 模块独立注册表**(`hooks/provider.rs:88`),与 history 注册表互不相通,本条不影响该用例。
3. **修复步骤**:
   1. **新建 `src-tauri/src/agent_history/mock.rs`**(L4 专用测试 provider;`MockCliHistoryProvider` 无状态单元结构体;`scan_root()` = env `SLTERM_MOCKCLI_PROJECTS_DIR` → Option<PathBuf>;impl `CliHistoryProvider` 四方法:scan 照 claude/scan.rs 同构(遍历扫描根一级编码目录,UUID 主干 .jsonl,解析 cwd/summary 首行——复用 claude::jsonl 解析助手,可见性不足则 pub(crate) 化,**不得复刻第二份解析**;条目打标 cli_id = "mockcli";env 缺失/目录不存在 → 空 Vec)、delete 照 claude ops、validate_session_id 用 `is_uuid_filename` 同口径、read_title 回退链 summary > firstPrompt、文件缺失 → Ok(None)——**骨架代码照抄 review-05 CP-041 步骤 3.1**,todo!() 两处由执行 agent 填充,`ScanRootGuard` 误引行删除)。
   2. **注册表 env 门控(provider.rs:46-62 改写)**:`static BASE_REGISTRY` = 现状逐字(仅 claude);`static E2E_REGISTRY` = claude + mockcli 两条目;`pub(crate) fn registry()` 按 env 存在性二选一返回;`resolve_provider` 与 mod.rs 的 `is_claude_provider`(:125-133)内 `REGISTRY.iter()` 改 `registry().iter()`;mod.rs:26 `use provider::{…, REGISTRY}` 同步改。**完整代码块照抄 review-05 CP-041 步骤 3.2**。
   3. **mod.rs 声明**:`pub mod claude; pub mod provider;`(:20-21)后加 `mod mock;`(crate 内私有)。
   4. **run-wdio.cjs 注入**(claude fixture 块(:211-228)之后追加):`fixtures/mockcli-projects` 缺失即 exit(1)(红线条款扩列 mockcli);`copyFixtureTree` 副本到 `.tmp-mockcli-projects`(占位符 `__E2E_PROJECT_DIR__` 替换同 claude 通道);`process.env.SLTERM_MOCKCLI_PROJECTS_DIR = tmpMockProjectsDir`——**完整代码块照抄 review-05 CP-041 步骤 3.4**;文件头注防复发校验段后补一行 mockcli env 说明。
   5. **fixture 新建 `e2e-tests/fixtures/mockcli-projects/`**:形态照 `fixtures/claude-projects/`——一级编码目录 + UUID 主干 .jsonl(summary 首行 + cwd 占位符,归属 E2E 项目目录),README 一行维护说明;fixture 会话 ≥1 条。
   6. **L4 用例(mockcli.e2e.ts 新增第三 describe「mockcli 历史链路(CP-041 L4:展示 + 双击恢复注入)」)**:用例一「历史条目展示」——setupTerminal + createProject + openNavView + 展开辅助(照 history.e2e.ts 模式/CP-028 单次确定性形态)→ 断言 nav-history-node 内行文本含 fixture 标题 + 行内 img src 含 "/cli-icons/mockcli.png";用例二「双击恢复」——dblclick fixture 行 → 照 history.e2e.ts:557-596 四步断言:activePage rootPath = e2eProjectDir → 终端容器就绪 → `__e2e_getTerminalText` 含 `mockcli --resume <fixture UUID>`。
4. **测试同步**:
   - L1 新增(provider.rs 领域测试模块):`registry_env_absent_returns_base_only`、`registry_present_e2e_env_includes_mockcli`(env 修改 set/remove 成对,`--test-threads=1` 保证串行);mock.rs `mod mock_provider_tests`:scan 空 env → 空 Vec、validate UUID 两态、read_title 文件缺失 → Ok(None)、fixture tempdir 单条目 scan 打标 cli_id == "mockcli"、delete 落盘真删(mk tempdir 隔离)。
   - L4 新增:上列两用例;既有 mockcli 用例①②零改动(用例②走 hooks 注册表,不受影响)。
   - 既有适配:mod.rs `command_scan_unknown_cli_id_returns_validation` 用例 cliId "nope" 仍未知,零改动;sessionRefreshTask L2 用例零改动(mock scan 不经后端注册表)。
   - 豁免销项:删 test-exemptions.md:21-22 两行;其 L2 兜底(AC-4③/⑤)保留原位。
5. **文档同步**:`.claude/test-exemptions.md` 删两行;`src-tauri/src/agent_history/CLAUDE.md` 注册表节补「env 门控扩展(CP-041):`registry()` 为当前生效表,`SLTERM_MOCKCLI_PROJECTS_DIR` 存在时追加 mockcli 条目——L4 专用测试 provider,env 命名/解析自管(MC-305 先例)」;`e2e-tests/CLAUDE.md`「用户目录隔离」节 fixture 通道段补 mockcli-projects 副本 + env 注入,「fixture 缺失必须终止」红线清单扩列 mockcli-projects;`e2e-tests/mockcli.e2e.ts` 文件头注补第三 describe 的数据隔离语义(扫描根 = .tmp-mockcli-projects 副本,不触真实 ~/.claude)。
6. **验证**:
   - L1:`cargo test mock -- --test-threads=1` exit 0(S02 后形态);全量 `cargo test -- --test-threads=1` 计数 = 基线 + 新增数,全绿。
   - 生产形态守卫:不设 env 时 `registry_env_absent_returns_base_only` 过;`resolve_provider("mockcli")` 仍 Validation 由用例锁死。
   - L4:`WDIO_RETRIES=0 node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts` 三轮 exit 0(含新增两用例);全量 `npm run e2e` 绿。
   - 机检:`grep -c "mockcli" .claude/test-exemptions.md` = 0;`grep -n "SLTERM_MOCKCLI_PROJECTS_DIR" e2e-tests/run-wdio.cjs src-tauri/src/agent_history/*.rs` 各 ≥ 1。

---

# S04 后端基础原语(7 项,子串行 4a → 4b)

**Stage 结构**:
- **4a 先行(全仓单 agent)**:CP-005 parking_lot 换装——触及 src-tauri 全仓,4b 全部项以其完成后 git 状态为基线。
- **4b 并行(≤5)**:CP-008(git 域,零重叠)、CP-010(conpty_api + 注册面 + App.tsx,零重叠)、CP-011+CP-034(pty 域,同 agent)、CP-014(shell.rs,零重叠)、CP-043(hooks 域 + 前端,零重叠)。
- **冲突预警**:CP-011 的 `join_with_timeout` 上提 reader.rs 与 CP-034 的 reader_loop 收敛同文件——同 agent 内先 034 删字段再 011 移函数,或反之;CP-005 与 CP-034 同改 reader.rs:23 import——4a 未完成的基线上 034 按 std 形态先行、4a 收尾统一换装是不推荐的双写路径,**编排上 4a 必须先完成**。

## CP-005 · std::sync::Mutex 中毒保持现状——全仓换装 parking_lot(原章二,S04-4a 全仓单 agent 先行)

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
2. **现状**:state.rs:4 `use std::sync::{Arc, Mutex, RwLock};`;src-tauri/src/CLAUDE.md:53 登记:「`state.rs` 等处的 `Arc<Mutex>` 保持标准库 `std::sync::Mutex`。持锁临界区均为短小无 panic 路径,中毒实际不可达,换 `parking_lot` 是零收益依赖变更」——本条即翻此登记。两种降级形态并存:settings.rs:101-103 map_err 形态;hooks/mod.rs:80-86 match 中毒分支;plan_balance/mod.rs:146/156/174 与 scan.rs:95 裸 `.lock().unwrap()`。2026-09-06 核查:8 处登记低估,生产站点实含 notify/mod.rs;`Cargo.toml` 无 parking_lot 依赖。
3. **修复步骤**(4a 先行,4b 各项以本项完成后为基线;全部照抄):
   1. `src-tauri/Cargo.toml:56`(ureq 行后)加:
      ```toml
      # CP-005: 消除 std Mutex 中毒攻击面——锁内 panic 不再连锁 panic 等待方;
      # parking_lot lock() 无 Result,全仓 map_err/unwrap/match 降级站点一并清除
      parking_lot = "0.12"
      ```
   2. 全仓类型替换:`std::sync::Mutex` → `parking_lot::Mutex`、`std::sync::RwLock` → `parking_lot::RwLock`(use 语句与全路径书写站点——git/mod.rs:88 签名、state.rs:259,260,286 参数签名——一并替换)。
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
   - 适配(逐一点名):`state.rs` `mod state_tests`(ring_buffer 五用例 :372-476 与 :341-369 构造用例的 `.lock().unwrap()/.read().unwrap()` → `.lock()/.read()`)、`mod project_root_tests`(:871 等 `read().unwrap()` → `.read()`);`settings.rs` 内嵌测试(并发用例 :520 起,锁调用形态若涉 unwrap 同步);`plan_balance/mod.rs` `reset_snapshot_for_test`(:69 `.lock().unwrap().take()` → `.lock().take()`);`notify/mod.rs` 测试(:882、:942);`pty/spawn.rs` 测试(:42 import 换 parking_lot);`agent_history/claude/scan.rs` scan_tests(若涉锁直接调用);`src-tauri/tests/` 集成测试侧经 `make_app_state` 透传,零改动(grep 确认无直接 lock 调用)。
   - 新增防复发用例:`hooks/mod.rs` `mod watcher_tests` 增 `start_signal_watcher_locks_without_poison_path`(连续启动两次,第二次命中「已启动跳过」分支——锁获取不再走 Result 形态由编译器保证,行为用例锁死幂等语义不回归);防复发主体 = 编译期(锁无 Result)+ 验证节 grep 守卫。
   - PTY 相关用例全部 `--test-threads=1` 串行(纪律不变)。
5. **文档同步**:
   - src-tauri/src/CLAUDE.md「std Mutex 中毒保持现状(DOC-10)」节(:53)整节重写为:「**parking_lot 换装(CP-005)**:`state.rs` 等全部持锁站点用 `parking_lot::Mutex/RwLock`,中毒攻击面消除(锁内 panic 不再连锁 panic 等待方);新建持锁临界区一律 parking_lot,禁止再引入 std::sync::Mutex/RwLock(grep 守卫)」;同文件「既定豁免」表「Mutex 中毒分支」行删除;
   - adr.md ADR-0009 表 09#14 行(:207)改写:「后端 Mutex **已换装 parking_lot**(CP-005,2026-09):中毒攻击面结构性消除,原『保持现状』登记作废」;
   - pty/CLAUDE.md「既定豁免」表「Mutex 中毒分支」行删除;git/CLAUDE.md「既定豁免」表「仓库缓存 Mutex 中毒分支」行删除;plan_balance/CLAUDE.md:17「照 hooks/mod.rs WATCHER 先例」句保留(先例本身换装,语义不冲突);
   - agent_history/CLAUDE.md、notify 相关若涉「锁中毒」措辞,grep 一并清理。
6. **验证**:
   - `rg "std::sync::(Mutex|RwLock)" src-tauri/` 零命中(退出码 1);
   - `rg "锁中毒|poison" src-tauri/src` 零命中(测试名/注释残留即红);
   - `cargo test -- --test-threads=1` 全绿(S02 后形态;若 S04 先于 S02 执行——不允许,编排上 S02 在 S04 前);
   - `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 与 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 退出码 0。

---

## CP-008 · git_status is_ignored() 死代码——语义对齐验证 + 删死分支(原章二,S04-4b)

1. **位置**:
   - `src-tauri/src/git/mod.rs:50-76`——`status_to_str`(:68-69 `is_ignored()` 死分支);`:26` `GitStatusEntry.status` 注释含 `| ignored`
   - `src-tauri/src/git/CLAUDE.md:21`——「`git_status` 不再扫描被忽略文件」节(末句「`status_to_str` 的 `is_ignored()` 分支保留为无害死代码」);`:65` 红线「不要恢复 `include_ignored(true)`」
   - `src-tauri/tests/git_status_tests.rs:30-56`——B1 纯函数映射用例(:48 `(git2::Status::IGNORED, Some("ignored"))`)
   - 前端容错消费面:`src/types/git.ts:7` status 注释值集、`src/features/explorer/FileIcon.tsx:28-35`、`src/features/commit/CommitFileList.tsx:277-279`、theme `GIT_FILE_COLORS.ignored` token、`src/__tests__/commit-context-menu.test.ts:115-119`、`commit-open-file.test.ts:295`、`file-icon.test.tsx:108`、`explorer-git-status.test.tsx:348,411`
2. **现状**:git/mod.rs:68-69 `} else if status.is_ignored() { Some("ignored") }`;唯一调用路径 `git_status_impl`(:155-161)的 `StatusOptions` 不含 `include_ignored` → ignored 标志永不置位 → 分支不可达;前端消费面全部为**渲染容错**(颜色映射/菜单空分支/打开拒绝),非功能消费——生产永不收到 `"ignored"` 条目,容错路径是死防御但**保留无害**。
3. **修复步骤**(语义对齐验证已完成,结论写死:无功能消费,可删):
   1. git/mod.rs:68-69 删除 `} else if status.is_ignored() { Some("ignored") }` 两分支行;`:26` 注释删 `| ignored`(改后:`modified | added | deleted | renamed | untracked | conflict`);
   2. `status_to_str` doc 注释「返回 None 表示无变更(Current)」保留;IGNORED 标志落入末支 `None`——与 Current 同语义(跳过),注释补一句:「`IGNORED` 落入 None:include_ignored 恒关,永不置位(CP-008 删死分支)」;
   3. 前端零改动(渲染容错保留);`src/types/git.ts:7` 注释值集删 `| ignored` 双边同步(硬约束 #4 注释对齐)。
4. **测试同步**:改 `git_status_tests.rs:48` → `(git2::Status::IGNORED, None)`(用例名 `test_status_to_str_all_flags` 不变);:174-190 include_ignored(false) 行为用例保留;新增防复发 `status_to_str_ignored_returns_none`(显式锁死 IGNORED → None)+ `git_status_ignored_file_never_emitted`(仓库含 .gitignore 忽略文件 → `git_status_impl` 结果无 ignored 条目——命令层锁死);L2 零改动。
5. **文档同步**:git/CLAUDE.md:21 节末句改写:「`status_to_str` 无 `is_ignored()` 分支(CP-008 已删——include_ignored 恒关,ignored 永不置位);未来若确需 ignored 感知,走独立轻量通道(.gitignore 判定),**禁止**恢复全量扫描」;git/CLAUDE.md:65 红线保留并追加同句引用。
6. **验证**:`rg "is_ignored" src-tauri/src` 零命中;`cargo test git_status -- --test-threads=1` 全绿;`npx tsc --noEmit` 退出码 0;clippy/fmt 通过。

---

## CP-010 · Win10 conpty 静默回退——一次性状态查询命令暴露(原章二,S04-4b 跨端)

1. **位置**:
   - `src-tauri/src/pty/conpty_api.rs:195-207`——`build_conpty_api`(回退仅 `tracing::warn!` :203)
   - `src-tauri/src/pty/conpty_api.rs:38-65,128-131`——`ConptyApi`/`Backend`/`is_bundled()`/`resolve_conpty_api`(OnceLock 单例)
   - `src-tauri/src/lib.rs:105-113`——`generate_handler!` 注册点
   - 三处注册红线:src-tauri/src/CLAUDE.md「新增命令必须三处注册」(`lib.rs` `generate_handler!`、`build.rs` `AppManifest::new().commands(...)`、`capabilities/default.json` `allow-<cmd>`,SEC-07)
   - 前端启动序列挂点:`src/App.tsx`(「启动对账 reconcile」先例,commit e99524f 同形态)
2. **现状**:conpty_api.rs:200-205 回退分支仅 `tracing::warn!("Win10 捆绑 ConPTY 加载失败,回退系统 conhost(滚轮不可用): {e:#}")` 后 `ConptyApi::system()`——捆绑/回退状态无任何前端暴露通道;用户无感知落入 0x3 老 conhost(无鼠标滚轮转发)。
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
      `build_conpty_api` 改形态:成功 → `STATUS.set(ConptyStatus{attempted:true,bundled:true,fallback_reason:None})`;回退 → warn 文案与 `fallback_reason` **同一变量**(`{e:#}` 存入 `fallback_reason: Some(format!("{e:#}"))`,日志与命令暴露同源零漂移);Win11/未尝试 → `ConptyStatus{attempted:false,bundled:false,fallback_reason:None}`。查询入口 `pub fn conpty_status() -> &'static ConptyStatus`(resolve_conpty_api 先行前提下 STATUS 必已初始化;防御分支走 should_bundle 推导)。
   2. 新命令(lib.rs :113 后注册,三处同步):
      ```rust
      /// 查询 ConPTY 后端状态(CP-010:Win10 回退可观测)
      #[tauri::command]
      pub async fn pty_conpty_status() -> Result<ConptyStatus, AppError> {
          Ok(conpty_api::conpty_status().clone())
      }
      ```
      三处注册:`lib.rs` `generate_handler!` 增 `pty::conpty_api::pty_conpty_status`;`build.rs` `AppManifest::new().commands(...)` 增 `"pty_conpty_status"`;`capabilities/default.json` 增 `"allow-pty_conpty_status"`。
   3. 前端:`src/types/pty.ts` 增 `ConptyStatus` 接口(`attempted: boolean; bundled: boolean; fallbackReason: string | null`,双边);`src/ipc/pty.ts` 增 `getConptyStatus()` wrapper;App.tsx 启动序列(紧随「启动对账 reconcile」)调一次:`attempted && !bundled` → `toast.show("warning", "终端已回退到系统控制台,鼠标滚轮转发不可用")`(单次,不重复弹);`bundled` → debug 日志静默。
4. **测试同步**:
   - L1(conpty_api.rs `mod conpty_api_tests`,现有 5 条保留):增 `conpty_status_win11_not_attempted`、`conpty_status_bundled_on_win10`、`conpty_status_fallback_reason_matches_warn`(失败注入 → fallback_reason=Some 且与 warn 同文案;注入点照 `ensure_extracted` 幂等用例先例);STATUS 单例跨用例污染 → 各用例独立进程或由防御推导分支保证。
   - 命令壳测试:tauri `mock_builder` 先例;L2 `src/__tests__/ipc-pty-contract.test.ts` 增命令名 + 返回键集合 camelCase 精确断言。
   - 既有用例适配:无(build_conpty_api 仅增 STATUS 记录,签名不变)。
   - **人工验证点**:Win10 实机删除 `%LOCALAPPDATA%\slterminal\conpty\` 触发回退 → 启动 toast 出现(ADR-0005 实机红线同批次执行)。
5. **文档同步**:pty/CLAUDE.md「Win10 捆绑 conhost(ADR-0005)」节补「回退状态经 `pty_conpty_status` 一次性查询暴露,启动 toast 提示降级后果(CP-010);warn 日志与 fallback_reason 同源」;pty/CLAUDE.md 豁免表「conpty_api vendor 提取/加载回退」行「当前兜底层级」列补「回退状态可观测(pty_conpty_status + 启动 toast)」;src/types/CLAUDE.md 契约对照 pty 条补 `ConptyStatus`;src/ipc/CLAUDE.md pty 通道描述同步;adr.md ADR-0005 不动。
6. **验证**:`rg "pty_conpty_status" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json` 三处全命中(缺一则 invoke reject,SEC-07);`cargo test conpty -- --test-threads=1` 全绿;`npx tsc --noEmit` + `npm test` 绿;人工:Win10 实机删提取目录 → 回退 + 启动 toast;Win11 恒静默。

---

## CP-011+CP-034 · pty_kill 超时显式清理(监督线程) + ring buffer 删除,reader 收敛单路径(原章二,S04-4b 同 agent;基线 = CP-005 完成后)

**CP-034 · ring buffer 半死机制——删替换层 + ring,reader 收敛单路径(先执行)**:

1. **位置**:
   - `src-tauri/src/pty/CLAUDE.md:58`——「Channel 可替换 + ring buffer 回放(E1)」登记行;`:115` 豁免表「`reader_loop` 残余 I/O 编排」行
   - `src-tauri/src/pty/reader.rs:1-3`(E1 头注释)、`:58-69`(reader_loop 文档)、`:70-78`(签名)、`:108-120`(EOF 分支 channel.read)、`:156-174`(数据分支 channel.read + ring append)、`:182-193`(Err 分支);`:392-438`(M11 分析块)
   - `src-tauri/src/state.rs:23-28`(`channel`/`output_ring` 字段)、`:311-335`(`RING_BUFFER_CAPACITY` + `ring_buffer_append`)、`:38-44`(Drop,与 CP-011 同改)
   - `src-tauri/src/pty/spawn.rs:1238-1244`(构造)、`:1256-1261`(reader 传参克隆)、`:1269-1279`(reader 线程 spawn)、`:1281-1292`(PtySession 构造)、`:2091`(测试直接构造 PtySession)
   - `.claude/test-exemptions.md:13`——DOC-01 豁免行含「channel 锁/…/ring buffer 批量写入」
2. **现状**:reader.rs:63-64 文档:「channel: 可替换的 Channel 引用,pty_reattach 通过写锁替换;ring: ring buffer,总是缓存最近输出供 reattach 回放」——对外重连命令已随 SEC-03 删除,替换与回放均无入口;断开时 ring 写入永不回放。pty/CLAUDE.md:58:「该机制保留于内部,对外重连命令已随 SEC-03 删除」——机制活着但无人受益。
3. **修复步骤**(全部照抄;确认无未来 reattach 规划——SEC-03 删除即终态):
   1. `PtySession` 删两字段(state.rs:23-26):`channel: Arc<RwLock<Option<Channel<PtyEvent>>>>` 与 `output_ring: Arc<Mutex<VecDeque<u8>>>` 全删(channel 全仓消费方 grep 确认仅 reader 一线);
   2. state.rs 删 `RING_BUFFER_CAPACITY`(:312)与 `ring_buffer_append`(:316-335);`use` 行清 VecDeque 与涉 ring 的锁引用(4a 基线上按实际残留清);
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
      - 数据分支:删 `channel.read()` 段与 `ring_buffer_append` 调用,改为:
        ```rust
        if let Err(e) = channel.send(PtyEvent::Output { bytes: batch }) {
            // CP-034: Channel 断开(前端已卸载)——单路径语义:退出,不缓冲
            tracing::debug!("Channel send 失败(前端已断开),reader 退出: {e}");
            break;
        }
        ```
      - EOF 分支:删 `channel.read()`(:108-114),Exit 事件 send 失败仅 debug 记日志后 break(:115-120 语义不变,去锁);
      - Err 分支:同 EOF 处理(:182-193);
      - 头注释(E1 句)、函数文档(:58-69)、M11 分析块(:392-438)同步删 RwLock/ring/reattach 全部措辞,改写为「Channel 直写 + 断开退出」;
   4. spawn.rs 适配::1239-1241 删 channel/output_ring 构造(channel 直接用 `on_output` 形参);:1256-1257 删 reader_channel/reader_ring 克隆;:1270-1278 reader 调用删 ring 实参;:1281-1292 PtySession 构造删两字段;VecDeque 与 RwLock import 清理;
   5. pty/CLAUDE.md:58 登记行整行删除;:115 豁免表行措辞改「依赖 Channel/管道系统调用」;
   6. test-exemptions.md:13 DOC-01 豁免行删「channel 锁/」「ring buffer 批量写入/」措辞。
4. **测试同步**:删 state.rs `mod state_tests` ring 六用例(:372/:382/:395/:413/:430/:452);改 spawn.rs :2091 测试 PtySession 构造删两字段;reader.rs `mod reader_tests` 零直接 ring 用例;防复发 = 验证节 grep 零命中 + 既有 PTY 集成用例(`pty_integration_tests` 7 条,SPAWN_LOCK 串行)全绿;「断开退出」分支不可 L1 构造(Channel send 失败需真实 IPC 对端),由 L4 PTY 通信用例 + L3 headless 覆盖;前端零改动(PtyEvent 载荷形态不变)。
5. **文档同步**:pty/CLAUDE.md:58 删行、:115 豁免表措辞改;test-exemptions.md:13 措辞同步;state.rs `PtySession` 字段 doc 删 E1 两行注释;reader.rs 头注释与 M11 块改写;无 ADR 登记点(E1 未入 ADR-0009 表,grep 确认)。
6. **验证**:`rg "output_ring|ring_buffer_append|RING_BUFFER_CAPACITY|RwLock<Option<Channel|pty_reattach|reattach" src-tauri/src` 零命中;`rg "ring" src-tauri/src/pty/` 零命中(注释残留即红);`cargo test -- --test-threads=1` 全绿(pty 集成串行);clippy/fmt 通过;L4 `npm run e2e` terminal spec 末位杀 app 用例绿。

**CP-011 · pty_kill 3s 超时仅 warn——超时后显式清理(监督线程)(后执行)**:

1. **位置**:
   - `src-tauri/src/state.rs:38-44`——`impl Drop for PtySession`(无超时 `handle.join()`,注释失真点)
   - `src-tauri/src/pty/spawn.rs:1418-1475`——`pty_kill`(超时分支 :1463-1468)
   - `src-tauri/src/pty/spawn.rs:1477-1538`——`pty_kill_all`(:1519-1526 超时分支)+ `KILL_JOIN_TIMEOUT`(:1538)/`KILL_JOIN_POLL_INTERVAL`(:1541)/`join_with_timeout`(:1543-)
   - `src-tauri/src/pty/spawn.rs:440-448`——`impl Drop for ConPtyInner`(先 drop writer,再 `ClosePseudoConsole`——真实无保护阻塞点)
   - `src-tauri/src/pty/CLAUDE.md:52-54`——「pty_kill 异步销毁」节(「超时放弃 join 记 warn,线程随 PtySession Drop 兜底」——失真登记)
2. **现状**:state.rs:38-44 超时路径下 `reader_handle` 已被 take → Drop join 不可达;可达场景(进程退出清空 sessions)是无超时 join——无界阻塞风险同款。spawn.rs:1466-1468 超时仅 warn「放弃 join(随 Drop 兜底)」;随后 session drop → master drop → `ConPtyInner::drop` → `ClosePseudoConsole` 无任何超时保护;上游已证实阻塞等待自 Win11 24H2 移除、Win10 永不修复(microsoft/terminal Discussion #17716)——Win10 永久形态,须应用侧规避。
3. **修复步骤**(采用「master drop 移入带超时监督路径」支;「先关输出管道句柄」支不采用——读端句柄由 reader 线程经 `clone_reader_with_pending_check` 独占 move,kill 路径无句柄可达;全部照抄):
   0. `join_with_timeout` 与两超时常量上提至 `src-tauri/src/pty/reader.rs`(纯函数无状态;state.rs 经 `crate::pty::reader::join_with_timeout` 引用);spawn.rs 改 import。
   1. spawn.rs 增常量(:1538 旁):`const CLOSE_PSEUDO_CONSOLE_TIMEOUT: Duration = Duration::from_secs(3);`(注释:CP-011,上游 Discussion #17716:pre-24H2 可永久阻塞,Win10 永不修复)。
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
   4. 注释失真修正:spawn.rs:1424-1425、1464-1465、1520-1524 三处「超时放弃 join 记 warn,线程随 PtySession Drop 兜底」改为「超时 detach reader,master drop 移交监督线程(CP-011)」;pty/CLAUDE.md:52-54 节同步;`// session drop → master drop → ClosePseudoConsole` 注释行(:1470、:1528)保留并补「正常路径;超时路径见上监督线程」。
   5. 决策抽纯函数(可测性,照 `eof_exit_code` 先例):reader.rs 增:
      ```rust
      /// CP-011: reader 未退出后的清理决策(纯函数,L1 锁死两分支)
      pub(crate) enum CleanupPlan { NormalDrop, DetachReaderSupervisedDrop }
      pub(crate) fn plan_cleanup_after_join_timeout(reader_finished: bool) -> CleanupPlan {
          if reader_finished { CleanupPlan::NormalDrop } else { CleanupPlan::DetachReaderSupervisedDrop }
      }
      ```
      pty_kill/pty_kill_all 按 plan 分支执行。
4. **测试同步**:reader.rs `mod reader_tests` 增 `cleanup_plan_finished_reader_normal_drop` / `cleanup_plan_timeout_reader_supervised_drop` + `join_with_timeout_timeout_returns_false`(mock 永不结束线程 + 10ms 级短超时,防 flaky);PTY 集成(spawn.rs,SPAWN_LOCK 串行)既有 kill 用例全量回归;spawn.rs 涉 KILL_JOIN_TIMEOUT/join_with_timeout 用例改 import(reader.rs);state.rs Drop 用例无直接构造(由集成路径覆盖);防复发 grep:裸 `JoinHandle::join` 零命中;不可自动化登记:.claude/test-exemptions.md 增行「`pty_kill` 超时→监督线程真实阻塞路径——Win32 阻塞不可注入——兜底 = `plan_cleanup_after_join_timeout` 决策用例 + pty 集成 kill 用例 + Win10 实机人工验证点(杀会话后应用无挂起)」。
5. **文档同步**:pty/CLAUDE.md「pty_kill 异步销毁」节(:52-54)重写(「`ClosePseudoConsole` 在 pre-Win11 24H2 上可能永久阻塞(上游 Discussion #17716,Win10 永不修复)。`pty_kill` 先提取 session 释放写锁,再在 `spawn_blocking` 中执行 `kill → join reader(3s)`:正常路径随闭包尾 drop;超时路径 reader detach、session 移入监督线程执行 drop(关 writer + ClosePseudoConsole),监督 3s 超时则清理线程 detach,进程退出时 OS 回收句柄(Job Object 保证子进程先死)」);pty/CLAUDE.md 豁免表增行;state.rs:21 字段注释补「超时/监督语义见 spawn.rs CP-011」。
6. **验证**:`rg "随 (PtySession )?Drop 兜底|随 Drop 兜底" src-tauri/src` 零命中;`rg "handle\.join\(\)|\.join\(\)\s*;" src-tauri/src` 零命中(仅余 join_with_timeout 内部与 spawn_blocking 的 `.await` join error map_err 形态);`cargo test -- --test-threads=1`(pty 系重点)全绿;clippy/fmt 通过;人工:Win10 实机高负载会话 kill,应用不挂起、3s 内 IPC 返回。

---

## CP-014 · shell 路径比对双侧 canonicalize 失败回退归一字符串(SEC-15 残余风险)——改 Win32 句柄级文件身份(原章三,S04-4b)

1. **位置**:
   - `src-tauri/src/pty/shell.rs:109-138`(`paths_match` 三分支;分支 2 字符串回退在 :125-133)
   - `src-tauri/src/pty/shell.rs:141-149`(`normalize_for_compare`)
   - `src-tauri/src/pty/shell.rs:514-600`(paths_match 纯函数测试)、`:601-618`(alias 兼容集成测试)、`:620-653`(真实应用执行别名条件测试)
   - `src-tauri/Cargo.toml:73-80`(windows crate features,缺 `Win32_Storage_FileSystem`)
   - `src-tauri/src/pty/CLAUDE.md:70-75`(Shell 白名单节 SEC-15 登记)
   - `.claude/adr.md:229`(D15 决策行)
2. **现状**:shell.rs:125-133 分支 2:`(Err(_), Err(_)) => { let a = normalize_for_compare(program); let b = normalize_for_compare(resolved); if cfg!(windows) { a.eq_ignore_ascii_case(&b) } else { a == b } }`——两侧均失败时纯字符串放行。shell.rs:100-108 doc 注释自登记:「理论上可构造同名字符串绕过——alias 兼容与风险的权衡,D15 决策,SEC-15」。windows crate 现状 features:JobObjects / Threading / Console / Foundation / Security / Pipes。
3. **修复步骤**(代码全量照抄 review-03 CP-014 步骤 3):
   1. `src-tauri/Cargo.toml:73-80` windows features 数组追加 `"Win32_Storage_FileSystem",`。
   2. `shell.rs` 新增句柄级身份函数(放在 `paths_match` 之前):`#[cfg(windows)] fn file_identity(path: &str) -> Option<(u32, u64)>`——CreateFileW(不带 FILE_FLAG_OPEN_REPARSE_POINT,alias reparse point 解析到真实目标)+ GetFileInformationByHandle,身份 = (volume serial, file index high<<32|low);任一侧打开/查询失败 → None(**完整代码照抄 review-03**)。
   3. `paths_match` 整函数替换:1) canonicalize 双成功 → 精确比较(Windows eq_ignore_ascii_case);2) 双侧均失败 → `fallback_identity_match`(Windows = file_identity 双侧 Some 且相等;任一侧打不开即拒绝,不降级字符串);3) 单侧失败即拒绝(SEC-15 收窄保留为纵深一层)。`normalize_for_compare` 收编为 `#[cfg(not(windows))]` 编译兜底(Windows 分支随字符串回退删除);shell.rs:100-108 原 doc 注释由新注释替代。
4. **测试同步**(L1,`shell.rs` 内嵌 `mod shell_tests`):
   - 既有适配:`paths_match_fallback_case_insensitive`(:549)、`paths_match_fallback_separator_normalization`(:560)、`paths_match_fallback_unequal`(:569)三则合并改写为 `fallback_both_unopenable_rejected`:`assert!(!paths_match(r"C:\no-such-dir-x\cmd.exe", r"C:\no-such-dir-x\cmd.exe"))`(同名同串也拒绝——字符串证据不再构成放行依据);canonical 三则(:575-599)不动;`allowlist_accepts_real_alias_when_present`(:632-653)保留,注释改「→ 句柄级身份比对放行」。
   - 新增(Windows 条件编译):`file_identity_same_file_via_hardlink_equal`(tempdir 实文件 + hard_link,身份相等且 fallback_identity_match true)、`file_identity_distinct_files_unequal`、`file_identity_missing_file_none`。
   - 防回归:上述三则 + `fallback_both_unopenable_rejected` 共同锁死「绕过需同 volume serial + file index」。
5. **文档同步**:pty/CLAUDE.md:75 改为「双侧 `canonicalize` 均失败时回退 Win32 句柄级文件身份比对(volume serial + file index,CreateFile 解析 alias reparse point);任一侧打不开即拒绝,不降级字符串;单侧失败即拒绝」;`.claude/adr.md:229` D15 行尾追加「D15 残余风险已销(2026-09):字符串回退改 Win32 句柄级文件身份比对,SEC-15 单侧拒绝保留为纵深」。
6. **验证**:`cargo test paths_match -- --test-threads=1` 与 `cargo test file_identity -- --test-threads=1` 全绿(S02 后形态);clippy 零警告(重点:normalize_for_compare cfg 后无 dead_code);`grep -n "eq_ignore_ascii_case(&b)" src-tauri/src/pty/shell.rs` → 零命中。

---

## CP-043 · SEC-12 statusline 原命令审查仅 warn 不阻断——命中即暂停 + 用户确认二次注入(原章三,S04-4b 跨端)

1. **位置**:
   - `src-tauri/src/hooks/claude/inject.rs:114-166`(SEC-12 注释段;`SUSPICIOUS_PATTERNS` :123-131;`warn_if_suspicious_statusline` :160-166)——**注意路径**:现状实为 `hooks/claude/inject.rs`(MC-213 下沉后),非 compromises 登记的 `hooks/inject.rs`
   - 调用点:`inject.rs:453-456`(`inject_impl` 注入路径)、`:615-618`(`reinject_statusline_impl` 启动重注入路径)
   - DTO:`src-tauri/src/hooks/mod.rs:31-40`(`AgentInjectionStatus` 三态)、`:45-52`(`AgentHookInjectionStatus`)
   - 前端:`src/types/agent.ts:37-43`、`src/ipc/agentHooks.ts:19-21`、`src/features/cliProfiles/profiles/claude/configEditor/ClaudeHooksConfigEditor.tsx:266-278`(`handleInject`)
   - 命令注册三处:`src-tauri/src/lib.rs:132`、`src-tauri/build.rs:48-53`、`src-tauri/capabilities/default.json:44-49`
   - 登记:`src-tauri/src/hooks/CLAUDE.md:78-80`(SEC-12 节)
2. **现状**:inject.rs:160-166 命中可疑模式仅普通 target warn,无审计通道;注入路径(:453-456)与重注入路径(:615-618)命中后照常写桥接配置 + 备份 + 原子写回 settings.json。链路背景:hooks 注入 = 后端直写 `~/.claude/settings.json`,「用户确认」无现成 IPC 契约可挂。
3. **修复步骤**(跨端,代码全量照抄 review-03 CP-043 步骤 3):
   1. `warn_if_suspicious_statusline` 整函数替换为 `audit_suspicious_statusline(command: &str) -> Option<&'static str>`——命中进审计通道(`tracing::warn!(target: "audit", …)`)并返回命中模式;旧「仅记录不阻断」语义作废。
   2. `inject_impl` 签名加第三参 `skip_suspicious_review: bool`;statusLine 桥接分支前插审查闸:`!skip_suspicious_review && !original_command.is_empty()` 且命中 → 返回 `Ok(AgentHookInjectionStatus { status: PendingConfirmation, version: None, suspicious_command: Some(original_command) })`——**settings.json 零写盘**(matcher 与桥接均不落盘;脚本已落盘无害,无 matcher 引用即惰性)。
   3. `reinject_statusline_impl` 启动路径:命中即跳过重注入 + 审计,不改写 settings(无用户交互可用;用户须进设置页手动注入走确认流)。
   4. DTO 改造(hooks/mod.rs):`AgentInjectionStatus` 增 `PendingConfirmation` 第四态;`AgentHookInjectionStatus` 增 `suspicious_command: Option<String>`(`#[serde(default, skip_serializing_if = "Option::is_none")]` 保住既有契约测试两键集合断言)。
   5. provider trait 加带默认实现方法 `confirm_inject()`(默认 Err Validation「该 CLI 不支持确认注入」;照 `ensure_hooks_scripts` 先例);claude provider override = `inject_impl(&settings_path, &script_dir, true)`;原 `inject` override 改传 `false`。
   6. 新命令 `agent_hooks_confirm_inject`(照 `run_agent_hooks_inject` 形态,spawn_blocking);三处注册:lib.rs generate_handler!、build.rs AppManifest(清单计数注释 37→38)、capabilities/default.json `allow-agent-hooks-confirm-inject`。
   7. 前端双边:`src/types/agent.ts` `AgentInjectionStatus` 加 `"pendingConfirmation"`、`AgentHookInjectionStatus` 加 `suspiciousCommand?: string`;`src/ipc/agentHooks.ts` 加 `confirmInject(cliId)` wrapper;`ClaudeHooksConfigEditor.tsx` `handleInject` 改确认流(状态机加 `pendingConfirm`:pendingConfirmation → 内联确认条展示 suspiciousCommand 原文(等宽字体完整展示不截断,`data-e2e` = `hooks-confirm-inject`/`hooks-cancel-confirm`)+ [确认注入]/[取消];确认 → `confirmInject` 二次调用;取消 → 清空待确认态)。
4. **测试同步**:
   - L1(inject.rs `mod inject_tests`):`inject_impl_suspicious_statusline_warns_but_injects`(:1841-1862)改写为 `inject_impl_suspicious_statusline_pends_confirmation`(断言 PendingConfirmation + suspicious_command 原文 + **settings.json 逐字节零写盘** + statusLine 未改写;随后 `inject_impl(..., true)` 完成注入断言桥接建立);`reinject_impl_suspicious_statusline_warns_but_reinjects`(:1865-1891)改写为 `reinject_impl_suspicious_statusline_skips_and_preserves`;新增 `inject_impl_clean_statusline_unaffected`(不命中 → 一步完成,suspicious_command 序列化缺键);新增审计断言(tracing-test `#[traced_test]` + `logs_contain("statusline 原命令命中可疑模式")`,TQ-COV-05 先例)。
   - L1(hooks/mod.rs):serde 用例新增 `injection_status_roundtrip_pending_confirmation`(三键集合 `["status", "suspiciousCommand", "version"]`、status "pendingConfirmation"、往返一致);既有三则 roundtrip 两键断言保留(skip_serializing_if 契约不破)。
   - L1 既有调用点适配:`make_inject_env` 驱动的全部 `inject_impl(&settings_path, &script_dir)` 调用(:1036、:1064、:1369、:1393、:1406-1407、:1517、:2056、:2035-2038 等)补第三参 `false`;命令层透传用例新增 `agent_hooks_confirm_inject_cli_id_passthrough`。
   - L2:`ipc-agent-hooks-contract.test.ts` 新增 confirmInject 契约用例(命令名/payload `{cliId}`/返回透传/异常传播四维);`settings-hooks-page.test.tsx` 新增「pendingConfirmation → 展示命令原文 → 确认二次调用 → Injected」全链路用例;全局 mock 补 `confirmInject`:setup.ts:108、:98、:77 及 terminal-strictmode/lifecycle、agent-status-hook、mock-cli-profile 各 mock 点逐一点名。
5. **文档同步**:hooks/CLAUDE.md:78-80 SEC-12 节改写为确认流口径(命中即暂停 + pendingConfirmation + 零写盘 + audit 通道 + confirm 二次调用;启动重注入命中 → 跳过 + 审计);inject.rs:114-121 段注释同步(「S19 文档同步」历史编号一并清理);src/ipc/CLAUDE.md「agent hooks 泛化命令(MC-211)」节 6 命令表改 7 命令;src/types/CLAUDE.md agent.ts 对照行同步四态值集;hooks/CLAUDE.md:91 trait 七方法红线补先例(confirm_inject 为第二个带默认实现的方法)。
6. **验证**:`cargo test suspicious -- --test-threads=1` 与 `cargo test injection_status_roundtrip -- --test-threads=1` 全绿(S02 后形态);`npx vitest run ipc-agent-hooks-contract settings-hooks-page` 全绿;`grep -n "agent_hooks_confirm_inject" src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json` 三处各 1 命中;`grep -rn "warn_if_suspicious_statusline" src-tauri/src/` 零命中。

---

# S05 DTO ts-rs 单源化(1 项,豁免单一项;硬前置 = S02 完成)

**预算警示**:58 文件 74 处 import 消费面;契约测试六件零改动策略;任务卡预算按独立大项计。

## CP-024 · IPC DTO 契约漂移——ts-rs 单源化(Rust derive 生成,删手写 src/types 对应面)(原章五)

1. **位置**:
   - `src/types/`——10 文件 317 行手写 DTO;导出面 `src/types/index.ts:1-20`;域文件 pty.ts/fs.ts/git.ts/notify.ts/agent.ts/agentHistory.ts/hooksConfig.ts(101 行,含 GUI 模型)/backgroundTasks.ts/planBalance.ts
   - 消费面:全仓 58 文件 74 处 `from "../types(/域)"` 形态 import(e2e-tests 0 处;深导入如 `src/ipc/agentHistory.ts:4`、`src/features/navTree/NavTree.tsx`)
   - Rust DTO 真身清单:`pty/spawn.rs:969`(PtyEvent)、`pty/spawn.rs:1020`(SpawnRequest)、`fs/mod.rs:19`(DirEntry)、`git/mod.rs:19/36`(GitStatusEntry/DiffHunk)、`notify/mod.rs:54`(FsEventPayload)、`hooks/signal.rs:28`(AgentEventPayload)、`hooks/mod.rs:33/47`(AgentInjectionStatus/AgentHookInjectionStatus)、`agent_history/mod.rs:35/62`(AgentHistorySession/AgentHistoryTitle)、`hooks/claude/config.rs:62/72/86`(HooksSubtree/MatcherGroup/HookHandler)、`background_tasks/mod.rs:24`(BackgroundTaskInfo)、`plan_balance/mod.rs:23-50`(PlanBalanceInfo/AmountInfo/WindowsInfo/WindowInfo)
   - 契约测试:`src/__tests__/helpers/ipc-contract.ts:29,83-85`(expectExactKeys)、`src/__tests__/ipc-*-contract.test.ts` 六件
   - 登记点:`src/types/CLAUDE.md:11-19`、`src/ipc/CLAUDE.md:94-95`(mockIPC 盲区红线)、根 CLAUDE.md 硬约束 #4
2. **现状**:
   - src/types/agent.ts:18-34 `AgentEventPayload` 手写十字段,可选字段 `usageSourcePath?: string | null`(`?` 源于旧信号缺键 serde default)——TS 的 `?` 与 Rust default 缺省语义当前靠人肉维持。
   - hooksConfig.ts:26-51 `HookHandlerJson` 为 C13-3 全字段矩阵,而 Rust `hooks/claude/config.rs:86-93` `HookHandler` **只声明 type+command 两字段**——手写面宽于 Rust 面,单源化必须先扩 Rust DTO。
   - hooksConfig.ts:57-100 `HooksConfigGui` 家族(4 类型)是前端 GUI 模型,**Rust 无对应**,不属于迁移面。
   - backgroundTasks.ts:16-22 `BACKGROUND_TASK_IDS` 等常量前端专有;agentHistory.ts:6 `TitleSource = string` 开放字符串(Rust 是枚举,语义有意放宽为开放串,不生成)。
   - PtyEvent 双侧 serde 形态一致:`#[serde(tag = "type", content = "data", rename_all = "camelCase")]` ↔ TS 判别联合。
3. **修复步骤**(阶段化,执行 agent 只抄写适配):
   1. **阶段 0(S02 前置确认)**:CP-040 已落地、默认 lib test target 恢复。ts-rs 的 `#[ts(export)]` 会在 derive 侧自动生成 `export_bindings_<类型名>` 内嵌单测(落位 lib 单测 target),**无需专用 [[test]] 结构**。
   2. **阶段 1(依赖接线)**:`src-tauri/Cargo.toml` [dependencies] 段(`serde` 行后)加:
      ```toml
      # CP-024:DTO 单源化——Rust derive 生成 TS 类型(serde-compat 默认特性解析
      # rename_all/tag/content/serde(default),与现有 serde 属性零冲突)
      ts-rs = "10"
      ```
      `cargo add ts-rs` 取解析 major 并锁 Cargo.lock;`cargo check` 通过。
   3. **阶段 2(Rust derive 接线)**:对上列 DTO 逐个加 `TS` derive + `#[ts(export, export_to = "../src/types/<域>.ts")]`(export_to 相对 src-tauri/CARGO_MANIFEST_DIR;同域多类型写同一路径,ts-rs 合并导出)。代表形态:
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
      **先行扩 DTO(写死)**:`hooks/claude/config.rs:86` `HookHandler` 扩至 C13-3 全字段矩阵(command: Option<String>、args: Option<Vec<String>>、async/async_rewake: Option<bool>、shell: Option<String>、url: Option<String>、headers: Option<BTreeMap<String,String>>、allowed_env_vars: Option<Vec<String>>、server: Option<String>、tool: Option<String>、input: Option<serde_json::Value>、prompt: Option<String>、model: Option<String>、continue_on_block: Option<bool>、if: Option<String>、timeout: Option<u64>、status_message: Option<String>,全部 `#[serde(default, skip_serializing_if = "Option::is_none")]`),校验层(SEC-05 type/command 审查)语义不变;`MatcherGroup.matcher` 补 `#[ts(optional)]`。
      首次接线后跑一次 `cargo test export_bindings -- --test-threads=1` 生成 9 个文件到 `src/types/`,逐文件与手写版 diff:**只允许注释与等价写法差异**;任何字段名/可选性/联合判别差异一律在 Rust 侧修(`#[ts(optional)]`/`#[ts(rename = "...")]`),不回改生成物。
   4. **阶段 3(前端残面收口)**:生成物物理覆盖同名文件(pty.ts/fs.ts/git.ts/notify.ts/agent.ts/agentHistory.ts/hooksConfig.ts/backgroundTasks.ts/planBalance.ts 九个);新建 `src/types/local.ts` 收容手写残面(TitleSource 开放串别名、BACKGROUND_TASK_IDS/BackgroundTaskId/PLAN_BALANCE_TASK_ID/SESSION_REFRESH_TASK_ID 常量族、ContextUsageSignal 前端窄视图——Rust 对应字段已生成,此接口保留为别名);`hooksConfigGui` 家族迁出为 `src/types/hooksConfigGui.ts`(含原文件 :57-100 四类型,头部注释保留「前端 GUI 模型,非后端 DTO」);`src/types/index.ts` 重写为:
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
   5. **阶段 4(契约测试同步)**:`ipc-*-contract.test.ts` 与 `helpers/ipc-contract.ts` **零改动**(expectExactKeys 守 JS 侧 payload 形状,与类型来源无关);L2 全量原样绿即形状等价证明。新增生成物漂移守卫(操作指令登记):提交前 `cargo test export_bindings -- --test-threads=1 && git diff --exit-code -- src/types`——漂移即红。
4. **测试同步**:
   - 新增(derive 自动生成):每个 `#[ts(export)]` 类型一条 `export_bindings_<类型名>` 单测;`cargo test export_bindings -- --test-threads=1` 全绿。
   - Rust 侧 HookHandler 扩字段后:补 L1 serde 用例 `hook_handler_full_matrix_roundtrip`(全 16 字段序列化/反序列化逐字段断言,锁 C13-3 矩阵)于 hooks/claude/config.rs 领域测试模块;BE-18 形态校验既有用例原样保留。
   - 既有用例适配逐一点名:`ipc-*-contract.test.ts` 六件零改动;`background-tasks-session-refresh.test.ts`/`agent-history-hook.test.tsx` 等 mock scan 用例零改动;`nav-tree*.test.tsx` 对 DTO 字面量 toEqual 断言不受字段序影响——保持零改动,红了才查形状不等价(属缺陷)。
5. **文档同步**:
   - `src/types/CLAUDE.md`:「双边对应契约」节改写为「单源化(ts-rs)」——Rust derive 为唯一真源;9 域文件为生成物(禁手改,生成/刷新指令 = `cargo test export_bindings -- --test-threads=1`);残面清单(local.ts / hooksConfigGui.ts)与各自理由;「修改注意事项」段改口:改 DTO = 改 Rust derive → 跑导出测试 → `git diff --exit-code -- src/types` 守卫。
   - 根 CLAUDE.md 硬约束 #4:改写为「DTO 单源:Rust `#[derive(TS)]` 经 ts-rs 生成 `src/types/` 对应文件,禁止手写第二份;字段语义值集同步登记与双侧字面量测试契约不变」。
   - `src/ipc/CLAUDE.md:94-95` mockIPC 盲区红线:补「DTO 形状真值源 = Rust ts-rs 生成,JS 侧不得另造结构」。
   - `src/types/CLAUDE.md:13-19` 域对照表:改为生成关系对照(类型 → export_to 文件),不再写「改一边必须改另一边」。
6. **验证**(可机检迁移完成判据,全部满足才销项):
   1. `npx tsc --noEmit` exit 0 且 `npx eslint src/` exit 0。
   2. `grep -rliE "ts-rs|generated" src/types/*.ts | wc -l` = 9;`ls src/types` = 9 域文件 + index.ts + local.ts + hooksConfigGui.ts + CLAUDE.md。
   3. `grep -rnE 'from "(\.\./)+types(/[a-zA-Z]+)?"' src --include="*.ts" --include="*.tsx" | wc -l` = 74;`grep -rn "from .*types/hooksConfig\"" src | grep -i gui` 零命中。
   4. `grep -rn "export interface AgentHistorySession\|export interface SpawnRequest\|export type PtyEvent" src/types` 的命中文件全部含生成头注;手写残面仅存于 local.ts/hooksConfigGui.ts。
   5. `cargo test export_bindings -- --test-threads=1` exit 0;随后 `git status --porcelain -- src/types` 输出为空。
   6. `npm test` exit 0;`npm run test:l3` exit 0。
   7. L4 抽验:`node e2e-tests/run-wdio.cjs --spec mockcli.e2e.ts` 全绿。
   8. `grep -rn "ts-rs" src-tauri/Cargo.toml` 命中且 `cargo tree -p ts-rs` 成功。

---

# S06 后端契约重设计(2 项,豁免 2 项:契约对偶 Stage;前置 = S05 完成)

**豁免理由(契约对偶)**:CP-006/CP-007 均改 IPC 契约面(fs_read_dir 签名 / agent_history_scan force 消亡),必须在 S05 ts-rs 单源化之后的契约稳态上执行,故单列 Stage 豁免并行限制。

## CP-006 · fs_read_dir 不分页——契约按理想终态重设计(游标分页)(原章二)

1. **位置**:
   - `src-tauri/src/fs/mod.rs:15-29`——`DirEntry` DTO
   - `src-tauri/src/fs/mod.rs:334-405`——`fs_read_dir` 命令 + `fs_read_dir_impl`(:348-405,整表返回)
   - `src-tauri/src/fs/CLAUDE.md`——「`fs_read_dir` 不分页(BE-21)」节 + 红线「**禁止给 `fs_read_dir` 加分页**:除非同步改前端 FileTree 虚拟化与 IPC 契约」
   - `src/types/fs.ts:4-15`——前端 DirEntry DTO;`src/ipc/fs.ts:77-78`——`readDir(path)` 唯一通道
   - 前端消费:`src/features/explorer/useFileTree.ts:37,432`(FileTree 虚拟化 FE-30 承接渲染)
2. **现状**:fs/mod.rs:348-405 `fs_read_dir_impl` 一次性 `Vec<DirEntry>` 全量返回,排序(:396-400 文件夹→文件、同类型小写名称序)与过滤(`.git`,:362-364)在返回前完成;无 cursor/limit 参数。本项即同步执行红线的前置条件。
3. **修复步骤**(前置:S05 ts-rs 已入仓;新契约 DTO 用 ts-rs 定义):
   1. 后端契约重定义(ts-rs DTO,双边导出):
      ```rust
      // src-tauri/src/fs/mod.rs(:15-29 后追加)
      /// 目录分页读取结果(CP-006:游标契约)
      #[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
      #[serde(rename_all = "camelCase")]
      #[ts(export, export_to = "../src/types/fs.ts")]
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
      内核实现写死:过滤 + 排序**全量完成后**再按游标切片(排序契约跨页稳定——游标 = 排序后序号的 base64 编码 `start..end` 形态,opaque 不透明);`limit` 越界钳制到 `[1, READ_DIR_PAGE_MAX]`,缺省 `READ_DIR_PAGE_DEFAULT`;`.git` 过滤与文件夹→文件排序语义零变更;**不采用 Channel 增量推送**——拉取式分页已削峰,推送式增加前端状态机复杂度,若执行期实测首帧仍不达标,另行立项。
   2. 前端双边:`src/types/fs.ts` 由 ts-rs 生成 `FsReadDirPage`;`src/ipc/fs.ts:77-78` 改:
      ```ts
      export async function readDirPage(path: string, cursor?: string, limit?: number): Promise<FsReadDirPage> {
        return invoke<FsReadDirPage>("fs_read_dir", { path, cursor, limit });
      }
      ```
   3. FileTree 迁移:`useFileTree.ts` `loadRoot` 改首帧拉首页(limit=默认)+ `nextCursor` 非空则后台续页拉取拼接(gen 计数丢弃旧代际响应的竞态语义不变;rootPath 变化清空语义保留);FileTree 虚拟化层(FE-30)对窗口数据天然透明,无改动;`refresh`/`refreshExpanded` 路径同步走分页聚合。
   4. 红线改写(见文档同步)。
4. **测试同步**:
   - L1:新增 `read_dir_first_page_has_cursor_when_overflow`、`read_dir_last_page_null_cursor`、`read_dir_cursor_resume_mid_list`(第二页接续无重复无遗漏)、`read_dir_page_limit_clamped_to_max`(limit=5000 → 钳 1000)、`read_dir_sort_order_stable_across_pages`(跨页拼接后整体序 = 文件夹→文件 + 小写名称序,锁死排序契约)、`read_dir_git_filter_still_applied`;
   - 既有适配:fs 测试中断言整表返回的用例改「分页遍历聚合后断言」(fs_read_dir 相关 L1 用例与 `src-tauri/tests/` 下涉 fs_read_dir 集成用例逐一点名);
   - L2:`use-file-tree.test.ts` 首帧 + 续页拼接用例、`explorer-virtualization.test.tsx` 数据供给适配、`ipc-fs-contract.test.ts` 参数结构改 `{ path, cursor, limit }` 键集合精确断言。
5. **文档同步**:fs/CLAUDE.md「`fs_read_dir` 不分页(BE-21)」节重写为「游标分页(CP-006):cursor/limit,默认 500/上限 1000,排序过滤后切片,游标 opaque;增量拉取由前端续页拼接」;红线改写为「**禁止无游标全量返回**:新增目录读取通道必须走游标分页契约」;adr.md ADR-0009 BE-21 行(:205)改写「~~不分页~~ → CP-006 已改游标分页(2026-09)」;src/types/CLAUDE.md 契约对照补 `FsReadDirPage`;src/ipc/CLAUDE.md fs 通道描述同步。
6. **验证**:`cargo test read_dir -- --test-threads=1` 全绿;`rg "禁止给 .fs_read_dir. 加分页" src-tauri/src/fs/CLAUDE.md` 零命中;ts-rs 导出产物含 `FsReadDirPage` 且与契约测试键集合一致;`npx tsc --noEmit` + `npm test` 全绿;万级单目录实测:首帧条目 = 500,交互无阻塞(人工)。

---

## CP-007 · session 扫描缓存死机制——先实测,达标删缓存层(原章二;含实测决策门槛,人工验证点)

1. **位置**:
   - `src-tauri/src/agent_history/claude/scan.rs:53-126`——BE-19 缓存(`SCAN_CACHE` :78、`ScanCacheKey` :60-65、`cached_scan` :84-110、`cache_key_of` :113-126)
   - `src-tauri/src/agent_history/mod.rs:88-122`——`agent_history_scan(cli_id, force)` 命令 + `run_scan`(force 分发 :114-122)
   - `src-tauri/src/agent_history/CLAUDE.md`——「扫描缓存 + force 通道(BE-19)」节 + 红线「**缓存键语义勿改**」
   - `src/features/backgroundTasks/sessionRefreshTask.ts:26-27`——唯一生产调用点恒 `force=true`(头注释 :4-5 登记理由)
   - `src/features/backgroundTasks/CLAUDE.md:41`——「扫描执行体 force 恒 true」节
   - `src/ipc/agentHistory.ts:16-20`、`src/ipc/CLAUDE.md:64`、`src/features/agentHistory/CLAUDE.md:32`——force 契约三面登记
2. **现状**:scan.rs:57-58 注释:「缓存键 = (目录 mtime, 文件数)……目录内会话文件的增删改不影响根键——由前端显式刷新(force=true)兜底」;前端唯一生产调用点恒 `scanAgentHistory(p.id, true)`——缓存永不命中,BE-19 沦为死机制。现状成本双付:死缓存维护(键计算 + 单槽 Mutex + clone 回填)+ 每次全量读盘。
3. **修复步骤**(先实测,门槛写死;两分支均写死,执行 agent 按实测结果照抄对应分支,不得自作主张):
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
      **人工验证点**:实测门槛判定须记录实际中位/max 数字进 commit message。
   1. **分支判定(机械)**:跑上例——绿 → 走删除分支(步骤 2);红 → 走指纹分支(步骤 3)。
   2. **删除分支(达标,主起草)**:
      1. scan.rs 删 `SCAN_CACHE`、`ScanCacheEntry`、`ScanCacheKey`、`cache_key_of`、`cached_scan`;`scan_sessions()` 与 `scan_sessions_with_force()` 合并为单一 `pub(crate) fn scan_sessions() -> Vec<AgentHistorySession>` = 原 uncached 直扫;
      2. agent_history/mod.rs:`agent_history_scan` 签名删 `force: Option<bool>` 参数;`run_scan` 删 `force` 参数与 `is_claude_provider` 身份比对分支 → `pub(crate) fn run_scan(provider: &dyn CliHistoryProvider) -> Vec<AgentHistorySession> { provider.scan() }`;
      3. `src/ipc/agentHistory.ts:16-20` `scanAgentHistory(cliId, force?)` 删 force 参数;`src/types/agentHistory.ts` 契约注释同步;
      4. sessionRefreshTask.ts:27 `scanAgentHistory(p.id, true)` → `scanAgentHistory(p.id)`;文件头注释 :4-5 整段改写为「扫描无缓存全量直扫(CP-007),无需 force」;
      5. 文件头 doc 注释(scan.rs:7-8)删「BE-19 缓存」句。
   3. **指纹分支(不达标,备选)**:
      1. `ScanCacheKey` 改目录内容指纹:`root` 一级目录逐项 `(file_name, mtime_ms, len)` 收集 → 按 file_name 排序 → FNV-1a 64 位哈希 + 一级条目数;`cache_key_of` 重写为指纹计算;
      2. force 通道、命令签名、sessionRefreshTask 恒 true 全保留;
      3. agent_history/CLAUDE.md BE-19 节改写为指纹口径,红线改写为「指纹算法勿改——失效精度承重」。
4. **测试同步**(按分支):
   - 删除分支:scan.rs `mod scan_tests` 删五用例(`scan_cache_hit_returns_stale_without_reread` :571、`scan_cache_invalidated_when_file_count_changes` :591、`scan_force_true_bypasses_cache` :614、`scan_cache_key_tracks_dir_mtime_and_file_count` :631、`scan_cache_isolated_per_root` :659);agent_history/mod.rs 删 `command_scan_force_true_bypasses_cache`(:577)与 `run_scan_force_true_on_non_claude_falls_back_to_trait_scan`(:598);新增防回归 `scan_reflects_deletion_immediately`(删会话文件后立即重扫结果为空);agent_history/mod.rs 增 `command_scan_without_force_param`;L2:`ipc-agent-history-contract.test.ts` 删 `force` 键(:69-114 全组适配)、`background-tasks-session-refresh.test.ts`「force=true 各调一次」断言改无第二实参、`agent-history-hook.test.tsx` 同步。
   - 指纹分支:缓存用例保留改指纹口径(`scan_cache_key_tracks_dir_mtime_and_file_count` 改写为 `scan_cache_key_tracks_dir_content_fingerprint`);新增 `scan_cache_invalidated_when_session_file_modified`。
   - 两分支共有:基准用例常驻。
5. **文档同步**:agent_history/CLAUDE.md BE-19 节按分支重写(删除分支:「扫描无缓存全量直扫——CP-007 实测 1000 会话中位 <50ms(基准用例常驻守卫);旧 BE-19 缓存层已删,force 参数已移出契约」);红线「缓存键语义勿改」删除分支中整行删除;src/ipc/CLAUDE.md:64、src/types/CLAUDE.md agentHistory 条、backgroundTasks/CLAUDE.md:41、features/agentHistory/CLAUDE.md:32 同步;BE-19 不在 ADR-0009 表(grep 确认),无 ADR 改写。
6. **验证**:
   - 删除分支:`rg "SCAN_CACHE|ScanCacheKey|scan_sessions_with_force|force" src-tauri/src/agent_history` 零命中;`rg "scanAgentHistory\([^)]*, ?true\)" src/` 零命中。
   - 指纹分支:`rg "dir_mtime_ms" src-tauri/src/agent_history` 零命中。
   - 共有:`cargo test scan -- --test-threads=1` 全绿(含基准用例);`npm test` 全绿。
   - 基准用例 flaky 处置:CI 机 IO 抖动时放宽顺序为「先 max、后中位」,任何放宽须在用例注释内留数字依据,禁止静默删除。

---

# S07 面板状态解耦(6 项,内顺序 CP-017 → CP-036 → CP-042 → CP-016 → CP-037 → CP-019;并行 ≤5)

**Stage 内重叠结论(实读)**:真重叠仅两处——① `tabClose.ts`(CP-017 第 4 步 closeTabGuarded 补 clearSettingsDirty + CP-036 新增 closeTabsGuarded,同文件同 import 行 → 同 agent,CP-017 先);② `pageApis.ts`(CP-042 整体重写 openSettingsPanel 已内嵌 CP-017 的 `renderer: "always"` 行 → CP-017 条目不重复改,以 CP-042 代码块为最终形态)。CP-016(sideViews/explorer)、CP-037(panels/markdown)、CP-019(panels/terminal)三者文件完全不相交。

## CP-017 · settings dirty 真值源脱离壳生命周期 + settings 纳入 always-render(原章四;与 CP-036 同 agent,先行)

1. **位置**:
   - `src/panels/settings/SettingsPanel.tsx:331-338`(壳挂载注册 false / 卸载 clear——真值源生命周期绑死壳)
   - `src/workspace/pageApis.ts:150-155`(`openSettingsPanel` 的 addPanel 无 `renderer` 参数)
   - `src/panelRegistry.ts:104-117`(`isAlwaysRenderPanel` 白名单不含 settings,SC-FE-06 决策写死处)
   - `src/features/settingsCenter/dirtyRegistry.ts:9-25`(真值源本体,注释明说「dirty 只在壳实例存活期间有意义」)
2. **现状**:SettingsPanel.tsx:333-337 `useEffect(() => { … setSettingsDirty(panelId, false); return () => clearSettingsDirty(panelId); }, [params?.panelId]);`;pageApis.ts:150-155 addPanel 无 renderer;panelRegistry.ts:111-117 白名单 = terminal + htmlviewer + markdownviewer;:109 注释「editor / gitshow / diff 故意排除」。
3. **修复步骤**:
   1. `panelRegistry.ts`——白名单追加 settings(SC-FE-06 口径翻案):
      ```ts
      /**
       * 检查面板是否应使用 renderer="always" 模式。
       * 显式白名单:terminal(保持 PTY 存活)+ htmlviewer/markdownviewer(避免 iframe
       * browsing context 销毁重建导致白屏 + CM 编辑实例切走切回不重建——草稿与缩放
       * 状态保活,决策 #17)+ settings(CP-017:dirty 真值源脱离壳生命周期——壳不随
       * 页签切换卸载,dirtyMap/dirtyRegistry 条目跨切签存活)。
       * editor / gitshow / diff 故意排除——CM6 重建无视觉闪屏,且大文件编辑器若始终挂载会显著增加内存开销。
       */
      export function isAlwaysRenderPanel(type: string): boolean {
        return (
          type === PANEL_TERMINAL ||
          type === PANEL_HTML_VIEWER ||
          type === PANEL_MARKDOWN_VIEWER ||
          type === "settings"
        );
      }
      ```
   2. `pageApis.ts`——`openSettingsPanel` 的 addPanel 补 `renderer: "always"`(**由 CP-042 代码块内嵌落地,本条目不重复改**):
      ```ts
      renderer: "always", // CP-017:settings 纳入 always-render(isAlwaysRenderPanel 白名单同步)
      ```
   3. `SettingsPanel.tsx`——删除壳生命周期绑定 effect(:331-338 整段删除),替换为说明注释:
      ```ts
      // CP-017:dirty 真值源(dirtyRegistry)脱离壳生命周期——不再随壳挂载注册/卸载清除。
      // 条目生命周期收口到「确认丢弃关闭」动作点(tabClose.ts closeTabGuarded /
      // closeTabsGuarded 与壳内 SC-FE-08 项目切换守卫),壳只负责读写,不拥有条目。
      ```
   4. `tabClose.ts`——`closeTabGuarded` 确认分支补清除(单面板关闭入口,×/Ctrl+W/中键/右键四路共用):
      ```ts
      export async function closeTabGuarded(
        api: { close(): void },
        panelId: string | undefined,
      ): Promise<void> {
        if (panelId?.startsWith("settings-") && isSettingsDirty(panelId)) {
          const ok = await confirmDialog({
            title: "未保存的修改",
            message: "当前配置页有未保存的修改,关闭将丢弃这些修改。",
            kind: "warning",
          });
          if (!ok) return;
          // CP-017:确认丢弃 = 真值源条目唯一清除点之一(壳卸载钩子已移除)
          clearSettingsDirty(panelId);
        }
        api.close();
      }
      ```
      (`import { isSettingsDirty, clearSettingsDirty } from "../features/settingsCenter/dirtyRegistry";`)
   5. `SettingsPanel.tsx`——SC-FE-08 项目切换守卫的确认分支(:383-400 一带,`ok` resolve 之后、`api.close()` 之前)同样补 `clearSettingsDirty(panelId);`;「初始评估静默关」分支不补(该分支只在刚挂载时触发,条目必不存在)。
   6. `dirtyRegistry.ts`——头注释改为:「壳(SettingsPanel)与关闭守卫共享同一 dirty 真值源。CP-017 后条目生命周期脱离壳:不写挂载注册、不做卸载 clear;条目在「确认丢弃关闭」动作点清除(tabClose.ts / SC-FE-08 守卫)。无条目 = 非 dirty。」
4. **测试同步**:改 `settings-panel-dirty.test.tsx`(删「壳卸载清除条目」断言;新增壳 unmount 后 `isSettingsDirty` 仍 true);改 `tab-close.test.ts`(新增「确认关闭 dirty settings 面板后 isSettingsDirty 为 false」「取消关闭后条目仍在」);改 `workspace-file-panel-types.test.ts`(`isAlwaysRenderPanel("settings")` 期望 true);改 `open-settings-panel.test.ts`(addPanel 参数断言补 `renderer: "always"`);`settings-panel-autoclose.test.tsx` SC-FE-08 确认分支现有断言保持。
5. **文档同步**:`src/workspace/CLAUDE.md` isAlwaysRenderPanel「不含 settings(SC-FE-06)」段改写为「settings 已纳入 renderer="always"(CP-017)」;`src/features/settingsCenter/CLAUDE.md`「dirtyRegistry 真值源(SC-FE-07)」节改「条目生命周期收口到确认丢弃关闭动作点;壳不拥有条目」;`src/panels/CLAUDE.md` settings 节 SC-FE-06 句删除并替换为 CP-017 口径。
6. **验证**:`grep -n "renderer" src/workspace/pageApis.ts` 命中 addPanel 一处 `"always"`;`grep -n "clearSettingsDirty" src/workspace/tabClose.ts` ≥ 1;`grep -c "setSettingsDirty(panelId, false)" src/panels/settings/SettingsPanel.tsx` = 0;`npx vitest run settings-panel-dirty tab-close open-settings-panel workspace-file-panel-types` 全绿。

---

## CP-036 · 批量关闭族(关闭其他/关闭全部)接入 dirty 守卫(原章四;与 CP-017 同 agent,后行)

1. **位置**:
   - `src/workspace/PageDockviewHost.tsx:287-304`(「关闭其他」:287-296、「关闭全部」:297-304 直关)
   - `src/workspace/tabClose.ts:19-32`(`closeTabGuarded` 单面板入口)
   - `src/features/settingsCenter/dirtyRegistry.ts:18`(`isSettingsDirty`)
2. **现状**:PageDockviewHost.tsx:289-295 `group.panels.filter((p) => p !== panel).forEach((p) => p.api.close());`——批量直关绕守卫;:278-279 注释自认「批量关闭族维持直关(批量确认交互未定义,遗留)」;workspace/CLAUDE.md「共享关闭守卫(FE-49)」节同登记。
3. **修复步骤**:
   1. `tabClose.ts`——`closeTabGuarded` 之后追加批量入口(一次性定义批量确认交互:dirty 面板列表 + 单次确认):
      ```ts
      /**
       * 守卫批量关闭(CP-036——关闭其他/关闭全部接入 dirty 守卫):
       * 收集待关列表中 dirty 的 settings 面板 → 单次 confirmDialog 列明确认 →
       * 确认才全部 close(并清除被丢弃面板的 dirtyRegistry 条目,CP-017 契约);
       * 无 dirty 面板零交互直关(对非 settings 面板行为零回归)。
       * @param tabs 待关闭页签(close 原语 + params.panelId 判据 + 页签标题——确认列表展示用)
       */
      export async function closeTabsGuarded(
        tabs: Array<{
          api: { close(): void };
          panelId: string | undefined;
          title: string;
        }>,
      ): Promise<void> {
        const dirtyTabs = tabs.filter(
          (t) => t.panelId?.startsWith("settings-") && isSettingsDirty(t.panelId),
        );
        if (dirtyTabs.length > 0) {
          const ok = await confirmDialog({
            title: "未保存的修改",
            message:
              `以下设置面板有未保存的修改,关闭将丢弃这些修改:\n` +
              dirtyTabs.map((t) => `· ${t.title}`).join("\n"),
            kind: "warning",
          });
          if (!ok) return;
          for (const t of dirtyTabs) clearSettingsDirty(t.panelId!);
        }
        for (const t of tabs) t.api.close();
      }
      ```
      (import 面加 `clearSettingsDirty`——与 CP-017 第 4 步同一 import 行。)
   2. `PageDockviewHost.tsx`——import 扩为 `{ closeTabGuarded, closeTabsGuarded }`;「关闭其他」action(:289-295)改:
      ```ts
      item("关闭其他", {
        danger: true,
        // CP-036:批量路径接入 closeTabsGuarded——dirty 面板列表 + 单次确认统一入口
        action: () => {
          const group = panel.api.group;
          if (!group) return;
          void closeTabsGuarded(
            group.panels
              .filter((p) => p !== panel)
              .map((p) => ({
                api: p.api,
                panelId: (p.params as TabParams | undefined)?.panelId,
                title: p.title,
              })),
          );
        },
      }),
      ```
      「关闭全部」action(:299-303)同形态(不过滤自身,`[...group.panels].map(...)`);并删除 :278-279 遗留注释。
   3. **页删除路径复核点**:Workspace 删页 → 整页 Dockview 实例销毁,settings 面板随之卸载——**不在本条决策范围**;S07 收尾时复核:若页删除不经任何守卫,残余破口须在产品决策(守卫 or 显式不守卫登记)后收口。
4. **测试同步**:改 `tab-close.test.ts` 新增「CP-036 closeTabsGuarded」用例组——① 无 dirty 零确认全关;② 含 1 dirty + 确认 → 全关且 isSettingsDirty 清除;③ 含多 dirty → confirmDialog 消息列全部标题;④ 取消 → 一个都不关、条目保留;⑤ 非 settings 面板直关;改 `workspace-page-dockview.test.tsx`(createTabMenuItems 宿主):「关闭其他/关闭全部」既有用例适配 mock `tabClose` 模块断言改调 `closeTabsGuarded`(参数形态:数组含 api/panelId/title);新增「批量路径不再直关」防复发用例;`workspace-callback-cache.test.tsx`/`workspace-header-actions.test.tsx` 若触及同名适配。
5. **文档同步**:`src/workspace/CLAUDE.md`「共享关闭守卫(FE-49)」节删遗留句,替换「批量路径经 `closeTabsGuarded` 统一入口(CP-036):dirty 面板列表 + 单次确认;确认后清除 dirtyRegistry 条目再全部 close」;`src/features/settingsCenter/CLAUDE.md` dirtyRegistry 节补批量清除点。
6. **验证**:`grep -n "forEach((p) => p.api.close())" src/workspace/PageDockviewHost.tsx` = 0;`grep -n "closeTabsGuarded" src/workspace/PageDockviewHost.tsx src/workspace/tabClose.ts` 各 ≥ 1;`npx vitest run tab-close workspace-page-dockview` 全绿。

---

## CP-042 · openSettingsPanel 改事件驱动 + 超时 toast 可观测化(原章四)

1. **位置**:
   - `src/workspace/pageApis.ts:135-164`(`openSettingsPanel`——:140-159 `for (let i = 0; i < 50; i++) { … await setTimeout 100 }` 轮询、:160-162 超时仅 `console.warn`)
   - `src/workspace/pageApis.ts:27-29`(`registerPageApi`——就绪信号唯一源头)
   - `src/workspace/Workspace.tsx:155-157`(`handlePageApiReady` → `registerPageApi(pageId, api)`)
   - 登记点:`src/workspace/CLAUDE.md:62`、`src/features/settingsCenter/CLAUDE.md`(openSettingsPanel 节)
2. **现状**:pageApis.ts:131 注释自述「100ms×50 轮询 getPageApi 就绪……超时 console.warn 降级(不抛异常)」;pageApis.ts:17 已 import `toast`(BE-23 警告用)——复用,无新增依赖。
3. **修复步骤**:
   1. `pageApis.ts`——`registerPageApi` 派发就绪事件:
      ```ts
      /** 页面 DockviewApi 就绪事件名(CP-042:openSettingsPanel 事件驱动等待;detail = pageId) */
      export const PAGE_API_READY_EVENT = "slterm:page-api-ready";

      /** 注册页面 DockviewApi(就绪时派发 window CustomEvent——事件驱动替代轮询) */
      export function registerPageApi(pageId: string, api: DockviewApi): void {
        pageApiMap.set(pageId, api);
        window.dispatchEvent(
          new CustomEvent(PAGE_API_READY_EVENT, { detail: pageId }),
        );
      }
      ```
   2. `pageApis.ts`——`openSettingsPanel` 整体重写(事件驱动等待 + 超时 toast;**addPanel 补 `renderer: "always"` 为 CP-017 接线点,本代码块即最终形态**):
      ```ts
      /**
       * 打开设置中心面板(同页单例)——调用方须先切到目标页
       * (本函数不切页,见 features/settingsCenter/openSettings.ts 编排)。
       *
       * 面板 id = `settings-{pageId}`;getPanel 命中 → focus 返回 true(同页单例),
       * 未命中 → addPanel(component "settings",renderer "always"——CP-017;
       * settingsPageId 深链时注入 params.selectedPage)。
       * 页面 api 就绪改事件驱动等待(CP-042):registerPageApi 派发
       * `slterm:page-api-ready`,5s 超时仅作防御底线——超时经 toast 可观测化
       * (原仅 console.warn 静默降级),返回 false 不抛异常。
       * @param settingsPageId 可选深链目标配置页 id(壳据此选中该配置页)
       * @returns 面板打开成功与否(超时返回 false)
       */
      export async function openSettingsPanel(
        pageId: string,
        settingsPageId?: string,
      ): Promise<boolean> {
        const panelId = `settings-${pageId}`;
        const api = await waitPageApi(pageId, 5000);
        if (!api) {
          console.warn(
            `[slTerminal] 页面 ${pageId} 的 DockviewApi 在 5s 内未就绪,无法打开设置中心`,
          );
          toast.show("warning", "设置中心打开失败:操作页面尚未就绪,请重试");
          return false;
        }
        const existing = api.getPanel(panelId);
        if (existing) {
          existing.focus?.();
          return true;
        }
        api.addPanel({
          id: panelId,
          component: "settings",
          title: "设置",
          renderer: "always",
          params: { panelId, ...(settingsPageId ? { selectedPage: settingsPageId } : {}) },
        });
        return true;
      }

      /** 等待页面 DockviewApi 注册——事件驱动(PAGE_API_READY_EVENT)+ 超时防御底线 */
      function waitPageApi(
        pageId: string,
        timeoutMs: number,
      ): Promise<DockviewApi | undefined> {
        const existing = getPageApi(pageId);
        if (existing) return Promise.resolve(existing);
        return new Promise((resolve) => {
          const onReady = (e: Event) => {
            if ((e as CustomEvent<string>).detail !== pageId) return;
            cleanup();
            resolve(getPageApi(pageId));
          };
          const timer = setTimeout(() => {
            cleanup();
            resolve(undefined);
          }, timeoutMs);
          const cleanup = () => {
            clearTimeout(timer);
            window.removeEventListener(PAGE_API_READY_EVENT, onReady);
          };
          window.addEventListener(PAGE_API_READY_EVENT, onReady);
        });
      }
      ```
      :123-134 原 doc 注释由上方新版替换。
4. **测试同步**:改写 `open-settings-panel.test.ts`——轮询类用例全部改写(未注册时挂起 → `registerPageApi(pageId, apiMock)` → 立即 resolve 且 addPanel 参数精确含 `renderer: "always"`);「5s 超时降级」用例断言返回值 false + `console.warn` + `toast.show("warning", …)` 各一次;「单例 focus 不新建」不变;新增「事件 detail 非目标 pageId 不唤醒」;`workspace-page-apis.test.ts` 补 registerPageApi 事件派发断言(注册后 window 收到 `slterm:page-api-ready` 且 detail === pageId)。
5. **文档同步**:`src/workspace/CLAUDE.md:62` 节改写为「事件驱动等待 `slterm:page-api-ready`(CP-042),5s 超时防御底线——超时 toast 可观测化后返回 false」;`src/features/settingsCenter/CLAUDE.md` openSettingsPanel 节同步;「openSettings 编排」节末句「console.warn 降级」改「超时经 toast 提示可观测化」。
6. **验证**:`grep -n "for (let i = 0; i < 50; i++)" src/workspace/pageApis.ts` 命中数 = 1(仅剩 `switchToPageAndFocus` 一处);`grep -n "PAGE_API_READY_EVENT" src/workspace/pageApis.ts` ≥ 3;`grep -n "toast.show" src/workspace/pageApis.ts` ≥ 2;`npx vitest run open-settings-panel workspace-page-apis` 全绿。

---

## CP-016 · 侧栏视图换区重建丢状态——视图状态上移注册表状态槽(原章四)

1. **位置**:
   - `src/features/sideViews/SideBarArea.tsx:97-141`(上下 pane 条件渲染视图,:111-114/:134-137 两处 `<def.component>`)
   - `src/features/sideViews/sideViewRegistry.ts:22-31`(`SideViewDef`/`SideViewComponentProps`)、`:34-56`(注册表类)
   - `src/features/explorer/useFileTree.ts:36`(`rootNodes` 组件内 state,展开态随卸载丢失)
   - `src/features/explorer/ExplorerPanel.tsx:43,63`(`React.FC` 无 props,:63 调 `useFileTree({ rootPath })`)
2. **现状**:SideBarArea.tsx:5-6 头注释:「切换即卸载旧视图组件——状态丢失语义 ADR-0001 已接受……」;sideViewRegistry.ts:34-56 注册表仅 defs 一张 Map;useFileTree.ts:36 `const [rootNodes, setRootNodes] = useState<TreeNode[]>([])`;:109-158 `toggleExpand` 展开态只写进 rootNodes;SideViewComponentProps 只有 `switchToPage`/`onDeletePage` 两字段。
3. **修复步骤**:
   1. `sideViewRegistry.ts`——`SideViewComponentProps` 追加两可选槽位:
      ```ts
      /** 侧栏视图组件的 props——与 SidebarTree props 精确匹配 */
      export interface SideViewComponentProps {
        /** 切换到指定操作页面(async——切换完成后再开面板) */
        switchToPage: (projectId: string, pageId: string) => Promise<void>;
        /** 删除指定操作页面 */
        onDeletePage: (projectId: string, pageId: string) => void;
        /** 视图恢复状态(CP-016:槽位切换/换区重建后由注册表状态槽回填;无历史状态则 undefined) */
        viewState?: unknown;
        /** 视图状态上呼(组件内部状态变化时持久化;模块级存活,跨挂载不丢) */
        onViewStateChange?: (state: unknown) => void;
      }
      ```
   2. `sideViewRegistry.ts`——`SideViewRegistry` 类新增状态槽:`private viewStates: Map<string, unknown>` + `getViewState<T>(id)` + `setViewState(id, state)`;`_reset()` 同步 `this.viewStates.clear()`。
   3. `SideBarArea.tsx`——两处 `<def.component …>` 统一改为受控消费(补 `viewState={sideViewRegistry.getViewState(def.id)}` 与 `onViewStateChange={(state) => sideViewRegistry.setViewState(def.id, state)}`);文件头注释 :4-6 删「状态丢失语义 ADR-0001 已接受」两句,替换为 CP-016 状态槽口径(文本照抄 review-04 CP-016 步骤 3.3)。
   4. `useFileTree.ts`——展开态真值源外移:`UseFileTreeOptions` 增 `viewState?: unknown` / `onViewStateChange?: (state: unknown) => void`;新增 `FileTreeViewState` 导出接口(`rootPath: string | null` + `expandedPaths: string[]`);hook 内新增 `viewStateRef`(ref 快照,不入 deps)、`onViewStateChangeRef`、`restoringRef`;新增 `commitViewState`(遍历 rootNodesRef 派生展开集上呼,restoring 期间短路)、`hasPath`(存在性守卫)、`restoreExpanded`(快照 rootPath 一致才恢复,浅→深逐层 toggleExpand,全程 restoringRef 抑制提交,完成后一次性 commit)——**完整代码块照抄 review-04 CP-016 步骤 3.4**;rootPath 变更 effect 的 `loadRoot(gen)` 之后追加 `.then(() => { if (gen !== genRef.current) return; void restoreExpanded(); })`;`toggleExpand` 末尾与 `reloadPreservingExpanded` 的 `setRootNodes(next);` 之后各追加 `commitViewState();`。
   5. `ExplorerPanel.tsx`——接 props 并透传(:43 改 `React.FC<SideViewComponentProps>` 析取 `viewState, onViewStateChange`;:63 调用补两字段;`switchToPage`/`onDeletePage` 保持不析取)。
4. **测试同步**:改 `sideViewRegistry.test.ts` 新增「CP-016 视图状态槽」组(setViewState/getViewState 同 id 覆盖、无条目 undefined、_reset 清空);改 `use-file-tree.test.ts` 新增「CP-016 恢复」组(① 匹配 rootPath 的 expandedPaths 恢复展开且子节点已加载;② rootPath 不符不恢复;③ toggleExpand 后 onViewStateChange 上呼含新展开路径、折叠后移除;④ 磁盘删除已展开目录经 triggerFsEvent 刷新后提交集收缩);既有 renderHook 调用点补传 `viewState: undefined, onViewStateChange: undefined`;新增 `sidebar-area-viewstate.test.tsx`(两 prop 透传断言 + 上呼后注册表可读回)。
5. **文档同步**:`src/features/sideViews/CLAUDE.md`「关闭语义(FE-21)」节删「ADR-0001 已确认接受」丢状态口径,改「状态经 sideViewRegistry 状态槽(getViewState/setViewState,`_reset` 同清)以视图 id 为键持久」;「外部坑/红线」中「换区重建丢失状态」「FE-21 隐藏视图卸载」两条同步修订;`src/features/explorer/CLAUDE.md`「宿主变更(ADR-0001)」节「已知行为:换区重建丢失展开状态」整段删除,替换为展开态槽位契约(FileTreeViewState 结构、rootPath 域键、提交/恢复时机);「测试模式」补 CP-016 恢复用例组说明。
6. **验证**:`npx eslint src/` 与 `npx tsc --noEmit` 退出码 0;`npx vitest run sideViewRegistry use-file-tree sidebar-area` 全绿;`grep -n "getViewState" src/features/sideViews/SideBarArea.tsx` = 2 处;`grep -c "commitViewState" src/features/explorer/useFileTree.ts` ≥ 4。

---

## CP-037 · markdown preview-only 改 CM 隐藏保活(display:none)(原章四)

1. **位置**:`src/panels/markdown/MarkdownPanel.tsx:13-16`(头注释登记「preview-only 卸载」已知行为)、`:275-287`(useCodeMirror 调用,:276 container 三元)、`:364-368`(`{mode !== "preview" && (<Allotment.Pane>…)}` 条件渲染)。
2. **现状**:276:`container: mode !== "preview" ? cmContainerRef.current : null,`;364-368 条件渲染;对照先例:同文件 :15「edit↔split CM pane 不卸载(React 位置保活),undo/光标保留」;panels/CLAUDE.md 登记「preview-only 卸载 CM(快照回填,光标/undo 重置登记已知行为)」。
3. **修复步骤**:
   1. `MarkdownPanel.tsx:364-368` 条件渲染改恒挂载 + `visible` 控制:
      ```tsx
            {/* CM pane 恒挂载(CP-037:preview-only 改 display:none 保活——undo/光标跨形态保留,
                照 edit↔split 先例);visible=false 时 allotment 收拢不占空间 */}
            <Allotment.Pane minSize={160} visible={mode !== "preview"}>
              <div ref={cmContainerRef} style={cmAreaStyle} />
            </Allotment.Pane>
      ```
      (删除外层 `{mode !== "preview" && (` 条件包裹。)
   2. `useCodeMirror` 调用(:275-287)container 表达式去 mode 三元:`container: cmContainerRef.current,`——mode 切换不再使 container 在元素/null 间跳变,EditorView 实例跨 edit/split/preview 全形态存活,光标/undo 栈保留(preview 态 onDocContent 仍实时写回 doc;preview 不监听外部修改语义不变)。
   3. 头注释 :13-16 改为 CP-037 口径(「CM 恒挂载……preview 态 allotment visible=false 隐藏保活——undo/光标跨形态保留,照 edit↔split 先例」)。
4. **测试同步**:改 `markdown-panel.test.tsx`——既有「preview 态 CM 卸载/container 传 null」断言全部翻转为「container 恒传 cmContainerRef.current」;新增防复发组「CP-037 preview 隐藏保活」:① edit 输入后切 preview 再切回 edit → EditorView 构造 spy 全过程仅调一次;② preview 态 CM pane 仍在 DOM(visible 属性/容器存在性断言);③ 切形态后 onDocContent 驱动链不断。
5. **文档同步**:`src/panels/CLAUDE.md` docViewer 家族节「preview-only 卸载 CM」句删除,替换「preview-only 改 CM 隐藏保活(CP-037,display:none 照 edit↔split 先例)——代价 preview 常驻一个 CM 实例内存,已接受」;`src/panels/markdown/CLAUDE.md`「CM 仅 edit/split 挂载」条目同步改写,外部坑/红线相关已知行为句删除。
6. **验证**:`grep -n "mode !== \"preview\" ? cmContainerRef" src/panels/markdown/MarkdownPanel.tsx` = 0;`grep -n "visible={mode !== \"preview\"}" src/panels/markdown/MarkdownPanel.tsx` ≥ 1;`npx vitest run markdown-panel` 全绿;L4 markdown.e2e.ts edit↔preview 往返后编辑器内容/光标不丢。
7. **S10 复核注记**:预览迁独立 webview(S10)后,workspace 层 CSS 显隐保活对预览 webview 是否仍然适用须在 S10-② 任务卡复核,结论登记 ADR-0019(本条目不因此阻塞)。

---

## CP-019 · PTY spawn 改事件驱动(ResizeObserver 首帧信号)(原章四)

1. **位置**:`src/panels/terminal/useXterm.ts:307-384`(PTY spawn 等待块);清理点 :517-543(:521-523 取消 rAF)。
2. **现状**::309-314 `const MAX_FRAMES = 30; const FIT_TIMEOUT = 500;`;:355-384 `pollFitAndSpawn` rAF 自轮询——`container.offsetWidth > 0 && offsetHeight > 0` 才 fit+spawn;30 帧或 500ms 超时回退 80×24。测试锁死:use-xterm-lifecycle.test.ts T1(:408)/T2(:424)/T3(:464)/T4(:485)。
3. **修复步骤**:
   1. `useXterm.ts` 删除 :310-314(声明)与 :355-384(pollFitAndSpawn 定义 + 启动 rAF),`doSpawn`(:316-350)与 `doSpawnRef.current = doSpawn;`(:353)原样保留。
   2. 原 :384 处替换为事件驱动 spawn:
      ```ts
      // ── PTY spawn(CP-019:事件驱动)──
      // ResizeObserver 首帧回调确认容器尺寸就绪 → fit → proposeDimensions →
      // pty.spawn(真实尺寸);500ms 超时仅作防御底线(回退 80×24,原 FIT_TIMEOUT 语义)
      let spawned = false;
      const spawnWithFit = () => {
        if (spawned) return;
        spawned = true;
        spawnObserver.disconnect();
        window.clearTimeout(spawnTimeoutId);
        if (canFit(term, fitAddon, container, isDisposedRef)) {
          try {
            fitAddon.fit();
            const dims = fitAddon.proposeDimensions();
            if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
              doSpawn(dims.cols, dims.rows);
              return;
            }
          } catch {
            // fit 失败 → 回退
          }
        }
        doSpawn(DEFAULT_COLS, DEFAULT_ROWS);
      };
      const spawnObserver = new ResizeObserver(() => {
        if (container.offsetWidth > 0 && container.offsetHeight > 0) spawnWithFit();
      });
      spawnObserver.observe(container);
      const spawnTimeoutId = window.setTimeout(spawnWithFit, 500);
      ```
   3. 清理段(:517-543)rAF 取消替换为 `spawnObserver.disconnect(); window.clearTimeout(spawnTimeoutId);`(与清理闭包同 effect 作用域)。
   4. 头注释「PTY spawn 等待布局就绪」相关行同步改为 ResizeObserver 事件驱动口径。
4. **测试同步**:改写 `use-xterm-lifecycle.test.ts` T1-T4 为 ResizeObserver 驱动(补 `globalThis.ResizeObserver` mock——记录回调、observe 触发时手动调回调;T1:offsetWidth=0 触发回调 → 不 spawn;T2:非零尺寸触发 → 以 proposeDimensions 真实尺寸 spawn;T3/T4 合并为「500ms 超时兜底」——尺寸恒 0、推进 fake timers 500ms → 回退 80×24);文件头注释(:3「rAF 轮询」)同步改。
5. **文档同步**:`src/panels/CLAUDE.md`「PTY spawn 等待布局就绪」节整节改写:「`useXterm` 挂载后不立即 spawn PTY:ResizeObserver 首帧回调确认容器尺寸就绪(CP-019 事件驱动)→ fit + proposeDimensions 取真实字符尺寸 → pty.spawn(真实 cols×rows)。500ms 超时仅作防御底线(回退 80×24)——正常路径不再有时序猜测轮询。」
6. **验证**:`grep -n "requestAnimationFrame" src/panels/terminal/useXterm.ts` = 0;`grep -n "ResizeObserver" src/panels/terminal/useXterm.ts` ≥ 2;`npx vitest run use-xterm-lifecycle` 全绿。

---

# S08 终端与主题体验(5 项,并行 ≤4)

**Stage 内约定**:CP-039 先于 CP-002(同 agent 共碰 JsonMode.tsx;CP-002 以 CP-039 后形态为基线);CP-020/CP-018 文件不相交可并行;CP-009 触 settings.rs 白名单,与 S04-4b 的 settings 测试同文件——编排上 S04 先行已消解。

## CP-020 · Ctrl+C 本地中断事件源——working 显式置 attention(原章四)

> 起草裁定:**不新增 interrupted 态**——F3 四态(working 绿/attention 黄/done 灰/error 红)是 `StatusDot` 单点渲染契约(`src/lib/agentStatus.ts:17` + `src/lib/CLAUDE.md` IC-03),新增第五态波及页签/导航树/历史行三处消费方;「attention」语义恰好覆盖中断后等待用户输入的场景,且 60s `idle_prompt` 本就会转 attention——本地中断只是把该转换提前显式化。60s 兜底语义保留不变。

1. **位置**:
   - `src/features/shortcuts/commandCatalog.ts:26-99`(COMMAND_CATALOG,无 terminal.interrupt)
   - `src/panels/terminal/keyboard.ts:18-51`(`createTerminalShortcuts`,:7/:49 明示「Ctrl+C 不注册命令」)
   - `src/panels/terminal/activeTerminal.ts:10-14`(`TerminalActions` 三字段)
   - `src/panels/terminal/useXterm.ts:220-227`(`terminalActions` 构造)、`:161-172`(`useXterm` props 面)
   - `src/panels/terminal/TerminalPanel.tsx:84-102`(`handleTabStateChange`,tabStatus 写点先例 :89)
   - `src/features/shortcuts/reserved.ts:14-20`(TERMINAL_RESERVED 含 `Ctrl+KeyC`——只拦用户覆盖,:8 注释)
2. **现状**:keyboard.ts:49 注释:「Ctrl+C 不注册命令 → 自然透传,xterm.js 发送 \x03 到 PTY」;TerminalPanel.tsx:89 `api.updateParameters({ ...latestParamsRef.current, tabStatus: state.status });`;agentStatus.ts:12-14 登记已知行为:「Ctrl+C 用户主动中断不发射任何 hook 事件,working 无中断出边为预期行为,依赖下一事件覆盖或空闲提示(~60s)衰减转 attention」;command-catalog.test.ts:44-48 守卫:每条 defaultKey 对自身 context 非保留。
3. **修复步骤**:
   1. `commandCatalog.ts` 在 `terminal.newline` 条目后追加:
      ```ts
      {
        // CP-020:Ctrl+C 本地中断事件源——派发本地 interrupt 后置 attention,再透传 \x03。
        // 保留键语义不变:isReserved 仍拦用户覆盖(用户无法改绑/解绑此键);
        // 代码默认键绑保留键为 CP-020 显式豁免(command-catalog.test 同步)。
        id: "terminal.interrupt",
        title: "中断(本地状态提示)",
        category: "terminal",
        context: "terminal",
        defaultKey: key("KeyC", { ctrl: true }),
        priority: 100,
      },
      ```
   2. `activeTerminal.ts`——`TerminalActions` 追加 `interrupt?: () => void;`(JSDoc:CP-020,实现须幂等)。
   3. `TerminalPanel.tsx`——新增 `handleInterrupt`:
      ```ts
      // CP-020:本地中断事件源——claude 上游中断不发 hook 事件,状态机无中断出边,
      // 前端自建:仅当页签当前 working 时置 attention(幂等;window capture 与 xterm
      // attachCustomKeyEventHandler 委托双路径各调一次,第二次为 no-op)
      const handleInterrupt = useCallback(() => {
        if (latestParamsRef.current.tabStatus !== "working") return;
        api.updateParameters({ ...latestParamsRef.current, tabStatus: "attention" });
      }, [api]);
      ```
      `useXterm({...})` 调用(:161-172)追加 `onInterrupt: handleInterrupt,`。
   4. `useXterm.ts`——props 接口追加 `onInterrupt?: () => void;`;`terminalActions`(:220-227)改为:
      ```ts
      const onInterruptRef = useRef(onInterrupt);
      onInterruptRef.current = onInterrupt;
      const terminalActions = useMemo<TerminalActions>(
        () => ({
          getSelection: () => terminalRef.current?.getSelection(),
          paste: (text: string) => terminalRef.current?.paste(text),
          writeToPty,
          interrupt: () => onInterruptRef.current?.(),
        }),
        [writeToPty],
      );
      ```
   5. `keyboard.ts`——在 `terminal.newline` 命令后追加:
      ```ts
      // CP-020:Ctrl+C 本地中断提示——派发 interrupt 后置 attention,再返回 false 透传,
      // xterm.js 仍发送 \x03 到 PTY(SIGINT 语义不变)
      commandFromMeta("terminal.interrupt", () => {
        const t = getActiveTerminal();
        if (!t?.interrupt) return false; // 无聚焦终端/旧实例 → 透传
        t.interrupt();
        return false; // 关键:透传——中断字节仍由 xterm 自然发送
      }),
      ```
      文件头 :7 注释改为:「Ctrl+C 注册为 terminal.interrupt 命令(CP-020):handler 派发本地中断提示后返回 false 透传,xterm.js 仍自然发送 \x03 到 PTY(SIGINT 语义不变)。」
   6. `src/lib/agentStatus.ts:12-14` 已知行为注释改为:「Ctrl+C 用户主动中断不发 hook 事件——CP-020 起由前端本地中断命令(terminal.interrupt)显式将 working 置 attention;60s idle_prompt 兜底语义保留」。
4. **测试同步**:改 `command-catalog.test.ts`(EXPECTED_IDS 追加 `"terminal.interrupt"`;:44-48 保留键守卫改为显式豁免形态——terminal.interrupt 断言 `isReserved(m.defaultKey!, m.context)).toBe(true)`,其余仍 false);改 `terminal-shortcuts.test.ts` 新增「CP-020 terminal.interrupt」组(① 有 active 且 interrupt 存在 → handler 返回 false 且 interrupt 被调一次;② 无 active → false;③ active 无 interrupt 字段 → false 不抛);新增 `terminal-interrupt-status.test.tsx`(tabStatus="working" → handleInterrupt → updateParameters 以 tabStatus:"attention" 调一次;"done"/null → 不调用——防复发对照修复前「中断滞留 working」);`use-xterm-lifecycle.test.ts` 补 `interrupt` 存在性断言。
5. **文档同步**:`src/features/shortcuts/CLAUDE.md`「Ctrl+C 保留为中断」整条改写为 CP-020 口径(handler 必须返回 false 透传;任何新增 terminal context 命令不得拦截透传语义);`src/panels/CLAUDE.md`「Ctrl+C 保留为中断」节与「中断场景已知行为」节同步改写;`src/features/agentStatus/CLAUDE.md` 若登记相关中断语义无需动。
6. **验证**:`grep -n "terminal.interrupt" src/features/shortcuts/commandCatalog.ts src/panels/terminal/keyboard.ts` 各 ≥ 1;`grep -n "commandFromMeta(\"terminal.interrupt\"" src/panels/terminal/keyboard.ts` 命中且紧邻 `return false;`;`npx vitest run command-catalog terminal-shortcuts terminal-interrupt` 全绿;L4 可观测(人工):claude 运行中按 Ctrl+C → 页签绿转黄,且 PTY 确实收到 \x03(claude 取消行为不回归)。

---

## CP-018 · WebGL 检测区分 SwiftShader 并一次性 toast 降级提示(原章四)

1. **位置**:`src/panels/terminal/webgl.ts:34-44`(`detectWebgl`)、`:81-111`(`tryLoad` 成功路径);`src/panels/terminal/useXterm.ts` 经 `setupWebglWithRetry` 消费(webgl.ts:64-142)。
2. **现状**:webgl.ts:34-44 `detectWebgl()` 单参 `canvas.getContext("webgl2")`,模块级 `webglCache`;:28-33 注释登记 FE-26 理由(blocklist 场景拒软件渲染 → DOM 掉帧;SwiftShader 远快于 DOM);全文件无任何降级信号/通知;检测契约被 `detect-webgl.test.ts:49` 与 `webgl-setup.test.ts` 锁死。
3. **修复步骤**:
   1. `webgl.ts` 顶部补 `import { toast } from "../../lib";`。
   2. `detectWebgl()` 保持原样不动(FE-26 注释保留)。
   3. `webgl.ts` 在 `detectWebgl` 之后新增:
      ```ts
      /** SwiftShader(软件渲染)判定缓存——模块级,一次检测全生命周期复用 */
      let swiftShaderCache: boolean | null = null;
      /** 软件渲染 toast 是否已提示(CP-018:全生命周期一次性) */
      let swiftShaderNotified = false;

      /**
       * 判定当前 WebGL2 渲染器是否软件渲染(SwiftShader)。
       * 经 WEBGL_debug_renderer_info 扩展读 UNMASKED_RENDERER_WEBGL;扩展缺失或
       * 读取出错时保守返回 false(不提示——避免误报)。不改 detectWebgl 检测契约。
       */
      export function isSwiftShaderRenderer(): boolean {
        if (swiftShaderCache !== null) return swiftShaderCache;
        try {
          const canvas = document.createElement("canvas");
          const gl = canvas.getContext("webgl2");
          if (!gl) {
            swiftShaderCache = false;
          } else {
            const ext = gl.getExtension("WEBGL_debug_renderer_info");
            const renderer = ext
              ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL))
              : "";
            swiftShaderCache = /swiftshader/i.test(renderer);
          }
        } catch {
          swiftShaderCache = false;
        }
        return swiftShaderCache;
      }

      /** 重置 SwiftShader 判定缓存与通知旗标(仅测试使用) */
      export function resetSwiftShaderCache(): void {
        swiftShaderCache = null;
        swiftShaderNotified = false;
      }
      ```
   4. `setupWebglWithRetry` 的 `tryLoad` 成功路径(:88 `onSuccess(webglAddon);` 之后)追加:
      ```ts
        // CP-018:软件渲染一次性降级提示——GPU blocklist 机器落入 SwiftShader 时
        // 用户无任何感知(FE-26 接受软件渲染的前提是「远快于 DOM」,但仍慢于硬件 GPU)
        if (isSwiftShaderRenderer() && !swiftShaderNotified) {
          swiftShaderNotified = true;
          toast.show("info", "当前终端使用软件渲染(SwiftShader),滚动性能可能下降");
        }
      ```
4. **测试同步**:改 `detect-webgl.test.ts` 新增 `isSwiftShaderRenderer` 用例组(① SwiftShader renderer 串 → true;② Intel renderer 串 → false;③ getExtension 返回 null → false;④ 每例前 `resetSwiftShaderCache()`);改 `webgl-setup.test.ts` 新增「context loss 重试路径上 SwiftShader 仅提示一次」(连续两次成功回调,`toast.show` 仅一次)。
5. **文档同步**:`src/panels/CLAUDE.md`「WebGL 优先 + DOM 兜底」节 FE-26 条目末尾追加 CP-018 口径(isSwiftShaderRenderer 判定 + 一次性 toast;检测契约不变)。
6. **验证**:`grep -n "isSwiftShaderRenderer" src/panels/terminal/webgl.ts` ≥ 2;`grep -n "failIfMajorPerformanceCaveat" src/panels/terminal/webgl.ts` 仍仅注释命中;`npx vitest run detect-webgl webgl-setup` 全绿。

---

## CP-039+CP-002 · editorTheme 订阅化(Compartment 热切换) + 摘除 codemirror-json-schema 自绘 lint/hover(原章四 CP-039 + 章一 CP-002,同 agent;CP-039 先行)

**CP-039 · editorTheme 订阅化——方案注册表响应式取色 + Compartment 热切换(先执行)**:

1. **位置**:
   - `src/theme/overrides.ts:40`(`export const editorTheme: Extension = schemeRegistry.getActive().editor.theme;`——模块级常量)
   - `src/theme/schemeRegistry.ts:44-52`(`setActive` 无变更通知机制)
   - 4 处消费:`src/panels/editor/useCodeMirror.ts:337-340`、`src/panels/diff/DiffPanel.tsx:547-550` 与 `:597-600`、`src/panels/gitshow/GitShowPanel.tsx:192-195`、`src/features/cliProfiles/profiles/claude/configEditor/JsonMode.tsx:164-167`
   - 登记点:`src/theme/CLAUDE.md:33`、`:65`;`src/theme/index.ts:61`(re-export)
2. **现状**:overrides.ts:16-17 头注释:「函数形导出每次调用取当前 active 方案(支持 D2 热切换);editorTheme 为模块级常量(求值时机由 main.tsx 启动序列保证)」;useCodeMirror.ts:337-340 扩展数组序 `editorSyntaxHighlight(), editorTheme, editorColorOverrides()`(ACC-05 顺序契约——syntax 必须先于 theme);theme-overrides.test.ts:97-101 锁常量契约;:219-231 锁消费点数组顺序;`main.tsx:67` 为唯一生产 setActive 调用点(运行期切换路径尚不存在——本条先消除 CM 掉队,为运行期切换铺路)。
3. **修复步骤**:
   1. `schemeRegistry.ts`——`SchemeRegistry` 类增订阅机制(`private listeners = new Set<() => void>()` + `onDidChange(listener): () => void` + `private notifyChange()`);`setActive` 两分支在 `this.activeId = …` 之后、`return` 之前各加 `this.notifyChange();`;`_reset()` 末尾追加 `this.notifyChange();`(listeners 集合本身不被 _reset 清)。
   2. `overrides.ts:40` 常量改函数:`export function getEditorTheme(): Extension { return schemeRegistry.getActive().editor.theme; }`;头注释同步修订。
   3. `src/theme/index.ts:58-64` re-export 删 `editorTheme` 改 `getEditorTheme`(breaking,4 消费点同步改)。
   4. 新建 `src/theme/editorThemeSlot.ts`:
      ```ts
      // editorThemeSlot.ts — CM 编辑器主题热切换槽(CP-039)
      //
      // 一个 EditorView 一个槽(Compartment 不可跨 view 共享——editor/CLAUDE.md 红线)。
      // extension 入扩展数组,替换原 [editorSyntaxHighlight(), editorTheme, editorColorOverrides()]
      // 三项(槽内数组顺序固化不变:syntax 必须先于 theme——ACC-05 mountStyles reverse 层叠)。
      // view 创建后 bind(view):订阅 schemeRegistry.onDidChange,方案一切换即 Compartment
      // 重配置——文档/光标/undo 全保留,编辑器不重建。

      import { Compartment } from "@codemirror/state";
      import type { Extension } from "@codemirror/state";
      import type { EditorView } from "@codemirror/view";
      import {
        getEditorTheme,
        editorColorOverrides,
        editorSyntaxHighlight,
      } from "./overrides";
      import { schemeRegistry } from "./schemeRegistry";

      /** 主题包:顺序固化 [syntax, theme, overrides](ACC-05——syntax 只能靠数组顺序决胜) */
      export function editorThemeBundle(): Extension[] {
        return [editorSyntaxHighlight(), getEditorTheme(), editorColorOverrides()];
      }

      export interface EditorThemeSlot {
        /** 入扩展数组的单项(Compartment 包装) */
        extension: Extension;
        /** view 创建后调用:订阅方案变更,返回取消函数(组件卸载时调用) */
        bind(view: EditorView): () => void;
      }

      /** 每 EditorView 创建一个槽 */
      export function createEditorThemeSlot(): EditorThemeSlot {
        const compartment = new Compartment();
        return {
          extension: compartment.of(editorThemeBundle()),
          bind(view) {
            return schemeRegistry.onDidChange(() => {
              view.dispatch({
                effects: compartment.reconfigure(editorThemeBundle()),
              });
            });
          },
        };
      }
      ```
   5. 消费点改造(4 处,模式一致):
      - `useCodeMirror.ts`:删 `editorTheme` import(改 `createEditorThemeSlot, type EditorThemeSlot`);hook 内 fontCompartment 附近加 `const themeSlotRef = useRef<EditorThemeSlot | null>(null); if (themeSlotRef.current === null) themeSlotRef.current = createEditorThemeSlot();`;扩展数组 :337-340 三行替换为 `themeSlotRef.current.extension,`;`viewRef.current = view;`(:370)之后加 `const unbindTheme = themeSlotRef.current.bind(view);`;cleanup(:397-406)`cleanup();` 之前加 `unbindTheme();`。
      - `JsonMode.tsx`(:150-182 挂载 effect):同模式——组件顶层 themeSlotRef;扩展数组 :164-167 三行替换;`viewRef.current = view;`(:175)后 bind;cleanup(:176-179)`view.destroy();` 前 unbindTheme。
      - `GitShowPanel.tsx`(:187-217):同 JsonMode 模式。
      - `DiffPanel.tsx`(左右两栏各一个 view):`leftThemeSlot`/`rightThemeSlot` 各栏独立槽(Compartment 不可跨 view 共享红线);两处扩展数组各替换;两 view 各 bind、cleanup 各先 unbindTheme。
4. **测试同步**:改 `theme-overrides.test.ts`(「editorTheme」describe 改写为 `getEditorTheme()` 响应式取色 + 切换后跟随;:239-266 用例改写为「getEditorTheme 切换后跟随 + 既有槽位 reconfigure 语义」;:219-231 数组顺序守卫正则对象改 `themeSlotRef.current.extension,`,新增 bundle 数组内 syntax 索引 < theme 索引断言);改 `theme-scheme-registry.test.ts` 新增 `onDidChange` 组(注册监听 → setActive 触发;未知 id 回退分支同样触发;取消后不再触发;_reset 触发;多监听全触发);消费点四测试补「setActive 后 view.dispatch 以 Compartment 效果被调」用例 + import 改 `getEditorTheme`;防复发:「方案切换后 EditorView 不重建(构造 spy 调用次数不变)」锁进 useCodeMirror 用例组。
5. **文档同步**:`src/theme/CLAUDE.md:33` 改写为 getEditorTheme 响应式口径;:65 外部坑「editorTheme 常量:需重载窗口」整条删除,替换为 onDidChange 订阅契约(含「监听方必须在 view 销毁前调取消函数」红线);:28 五导出节同步;panels/editor/CLAUDE.md「CM6 主题扩展与层叠(ACC-05)」节消费点写法改单槽,ACC-05 顺序契约落点改述为「槽内 editorThemeBundle() 数组序」;`.claude/adr.md` ADR-0002 追加后果条目(运行期即时切换整体仍否决,但 editorTheme 常量化这一系统性后果已消除,CP-039)。
6. **验证**:`grep -rn "import.*editorTheme[^S]" src/ --include=*.ts --include=*.tsx | grep -v getEditorTheme | grep -v editorThemeSlot` 零命中;`grep -n "onDidChange" src/theme/schemeRegistry.ts` ≥ 1;`npx vitest run theme-overrides theme-scheme-registry use-code-mirror diff-panel gitshow-panel` 全绿;`npx eslint src/` 与 `npx tsc --noEmit` 退出码 0。

**CP-002 · 摘除 codemirror-json-schema,自绘 lint/hover 层直消费 json-schema-library 11.x(后执行,以 CP-039 后 JsonMode.tsx 形态为基线)**:

1. **位置**:
   - `src/features/cliProfiles/profiles/claude/configEditor/JsonMode.tsx:19-25`(import 块)、`:156-163`(extensions 数组)
   - `package.json:47`(`"codemirror-json-schema": "0.8.1"`)、`:50`(`"json-schema": "0.4.0"`——仅 JsonMode.tsx:25 类型 import 消费)
   - `src/features/cliProfiles/profiles/claude/configEditor/schema/index.ts:22-24`(TE-15 去重评估结论注释)
   - `vitest.config.ts:9-16`、`vitest.l3.config.ts:8-15`(server.deps.inline 登记,专为 codemirror-json-schema 无扩展名 ESM 导入而设)
   - `src/__tests__/hooks-config-jsonmode.test.tsx:15-47, 97-103, 160-189`(mock 与扩展注册断言)
   - lockfile 双实例:`package-lock.json:5905-5907`(嵌套 9.3.5)与 `:9027-9029`(顶层 11.6.2)
2. **现状**:JsonMode.tsx import `jsonSchemaHover, jsonSchemaLinter, stateExtensions, handleRefresh` from "codemirror-json-schema" + `type JSONSchema7` from "json-schema";extensions 数组含 `linter(jsonParseLinter(), { delay: 300 })` / `linter(jsonSchemaLinter(), { needsRefresh: handleRefresh })` / `hoverTooltip(jsonSchemaHover())` / `stateExtensions(hooksSubSchema as unknown as JSONSchema7)`。schema/index.ts:45 已有 11.x 编译单例 `compileSchema(hooksSubSchema, { draft: "draft-07" })`,`validateHooksJson(text)` 返回 `{ isValid, diagnostics: { message, pointer } }`——自绘层直接复用,不重复编译。
3. **修复步骤**:
   1. 新建 `src/features/cliProfiles/profiles/claude/configEditor/jsonSchemaCm.ts`——自绘层:**lint** = `hooksSchemaLinter()`(linter 包装 `lintHooksSchemaDoc`,delay 300ms 与原对齐;`lintHooksSchemaDoc` 纯函数直调 `validateHooksJson`,诊断经 `pointerToRange` 定位,失败退回整文档);**hover** = `hooksSchemaHover()`(hoverTooltip 包装 `resolveHoverDescription`——`pathAt` 回溯键路径(最多 6 层/4096 字符窗口)→ `resolveSchemaPath` 逐段下钻 properties/items + 本地 `#/$defs/` $ref 解析(8 层防环)→ description);**pointer 定位策略**:JSON Pointer 逐段顺序文本搜索(数字段按第 n 个 `[` 近似),定位失败退回整文档下划——hooks 子树为小文档,近似定位足够。**完整全文照抄 review-01 CP-002 步骤 3.1**(约 170 行,含 `pointerToRange`/`toCmDiagnostic`/`lintHooksSchemaDoc`/`hooksSchemaLinter`/`resolveRef`/`resolveSchemaPath`/`pathAt`/`resolveHoverDescription`/`hooksSchemaHover` 全量)。
   2. `JsonMode.tsx` 编辑(CP-039 后形态上):import 块替换为 `import { hooksSchemaHover, hooksSchemaLinter } from "./jsonSchemaCm";`;`./schema` import 收窄为 `validateHooksJson, type JsonDiagnostic`(hooksSubSchema 不再直接消费);`@codemirror/view` import 收窄为 `EditorView`(hoverTooltip 不再直接消费);extensions 数组四行替换为:
      ```tsx
              linter(jsonParseLinter(), { delay: 300 }),
              hooksSchemaLinter(),
              hooksSchemaHover(),
      ```
      (`stateExtensions(...)` 行删除——其唯一职责是把 schema 注入 codemirror-json-schema 内部状态,自绘层经 validateHooksJson 闭包直取);文件头注释 CM6 扩展说明改为自绘层口径。
   3. `package.json` 删两行(`codemirror-json-schema`、`json-schema`);`npm install` 刷新 lockfile(嵌套 9.3.5 随之消失)。
   4. `vitest.config.ts:9-16` 与 `vitest.l3.config.ts:8-15` 的 `server.deps.inline` 块整删。
   5. `schema/index.ts:22-24` TE-15 注释替换为:「CP-002 已摘除 codemirror-json-schema:自绘 lint/hover 层(jsonSchemaCm.ts)直接消费本模块 11.x 编译单例,json-schema-library 全仓单实例(TE-15 消解)。」
4. **测试同步**:
   - 新建 `src/__tests__/hooks-json-schema-cm.test.ts`(直测纯函数,无 jsdom):`pointerToRange_顶层键_定位到值区间`、`pointerToRange_嵌套数组索引_收敛到目标行`、`pointerToRange_未知键_返回null`、`pointerToRange_空pointer_返回null`、`pathAt_嵌套位置_返回键序列`、`pathAt_文档头_返回空数组`、`resolveSchemaPath_事件键_命中hooks子schema`、`resolveSchemaPath_hookCommand$ref_解析出description`、`resolveSchemaPath_未知键_返回null`、`lintHooksSchemaDoc_合法配置_零诊断`、`lintHooksSchemaDoc_未知事件_单条error级诊断且message含Additional property`、`lintHooksSchemaDoc_语法错误_退回整文档区间`、`resolveHoverDescription_事件键_返回description`、`resolveHoverDescription_无description键_返回null`。
   - 适配 `hooks-config-jsonmode.test.tsx`:删 hoisted 四项 mock 与 :97-103 `vi.mock("codemirror-json-schema", …)` 块;新增 `vi.mock("…/jsonSchemaCm", …)`(两 mock 各返回 sentinel `[{ __schemaLinter: true }]`/`[{ __schemaHover: true }]`);「schema 扩展注册」用例(:160-189)重写为:断言 captured extensions 含两个 sentinel、mockLinter 仅被调一次(jsonParseLinter,`{ delay: 300 }`)、mockHoverTooltip 不再被组件调用;beforeEach mockClear 同步。
   - 既有 `hooks-config-schema.test.ts` 零改动(validateHooksJson 语义不动)。
5. **文档同步**:`src/features/cliProfiles/CLAUDE.md` TE-15 债务段整段替换为「TE-15 已消解(CP-002)」口径;`.claude/adr.md` TE-15 段(:240)末尾追加消解记录;vitest 双配置删除处注释同步消失。
6. **验证**:`grep -rn "codemirror-json-schema" src/ package.json vitest.config.ts vitest.l3.config.ts` 零命中(仅 docs/.claude 历史登记可命中);`grep -rn 'from "json-schema"' src/` 零命中;`npm ls json-schema-library` 仅 11.6.2 单实例;`npx tsc --noEmit` / `npx eslint src/` / `npx knip --production` 全 0;`npm test` 与 `npm run test:l3` 全绿。

---

## CP-009 · PASSTHROUGH_MODE(0x8)永久禁用——模式能力矩阵可配置化(原章二;人工验证点)

1. **位置**:
   - `src-tauri/src/pty/spawn.rs:52-55`——flag 常量(0x1 INHERIT_CURSOR / 0x2 RESIZE_QUIRK / 0x4 WIN32_INPUT_MODE)
   - `src-tauri/src/pty/spawn.rs:59-87`——`compute_conpty_flags(build_number, bundled)` 三态 + 0x8 禁用注释块(:71-79 实测记录)
   - `src-tauri/src/pty/spawn.rs:1153`——`create_conpty_pair(cols, rows, build)` flags 消费点
   - `src-tauri/src/settings.rs:22-29`——`SETTINGS_ALLOWED_KEYS` 六键白名单(SEC-11)
   - `src-tauri/src/pty/CLAUDE.md`——「PASSTHROUGH_MODE (0x8) 永久禁用」节 + 红线「**永不启用 0x8**」+「改 flags 必须实测真实 claude 滚轮」
   - `.claude/adr.md:167-177`——ADR-0007 审批门禁(:171 第 3 条「真实 claude 实机滚轮测试」)
2. **现状**:spawn.rs:80-87 三态输出恒不含 0x8;0x8 致 claude 全屏 TUI 滚轮失效(:71-74 实测,Win11 build 26200 双向实测),且「最小复现实验失败……阻断条件仅真实 claude 场景复现」——自动化不可守卫,重开必须人工门禁。
3. **修复步骤**(重开 0x8 永远走人工门禁,本步骤只落可配置化 + 默认矩阵守卫):
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
      (注:若 S05 ts-rs 尚未落地——编排上 S05 在 S08 前,正常可达;`#[ts(export)]` 的 export_to 路径按 S05 落地形态定。)
   2. `compute_conpty_flags` 改签名 `pub fn compute_conpty_flags(build_number: u32, bundled: bool, modes: &ConptyInputModes) -> u32`(三态决策逐位显式化:0x1/0x2 直取矩阵位;0x4 维持原门控 `modes.win32_input_mode && (bundled || build_number >= CONPTY_WIN11_MIN_BUILD)`;补 `const FLAG_PASSTHROUGH_MODE: u32 = 0x8;` 末行 `if modes.passthrough_mode { flags |= FLAG_PASSTHROUGH_MODE; }`)——默认矩阵输出与原三态恒等。
   3. spawn 路径接线(:1153 调用点):`pty_spawn` 在 spawn_blocking 内经 `crate::settings::read_existing_settings` 读 `conptyInputModes` 段(缺失/解析失败 → `ConptyInputModes::default()` 并 debug 记日志,不阻塞 spawn),矩阵随 `create_conpty_pair` 传入。
   4. 白名单:settings.rs:22-29 `SETTINGS_ALLOWED_KEYS` 六键改七键,追加 `crate::pty::spawn::SETTINGS_KEY`。
   5. 前端:`src/types` 增 `ConptyInputModes`(ts-rs 导出双边);settingsCenter 新增「终端输入模式」设置页(四开关;passthrough 开关旁常驻警示「启用将导致 claude 等全屏 TUI 鼠标滚轮失效,变更须实测验证」);store 段 `conptyInputModes` 走既有 settings 保存通道(段形态,照 fontSize 先例)。
   6. **人工验证点(不可自动化)**:任何 `passthrough_mode: true` 默认值变更或矩阵位默认翻转,合并前必须完成 ADR-0007 门禁第 3 条(真实 claude 实机滚轮:全屏 TUI + 滚轮滚动)+ Win10 21376 阈值核对;门禁记录在 PR 描述留证。
4. **测试同步**:spawn.rs 现有 compute_conpty_flags 用例(ADR-0005 登记 L1 7 条)全部注入 `ConptyInputModes::default()` 适配新签名,期望输出不变(0x7/0x7/0x3);新增 `conpty_flags_default_matrix_matches_legacy_tristate`(三输入 × 默认矩阵 → 0x7/0x7/0x3,防复发主用例)、`conpty_flags_passthrough_mode_adds_0x8`、`conpty_flags_win32_input_still_gated_by_build`(win32_input_mode=true + Win10 回退 → 0x4 不置位);settings.rs 白名单用例增第七键(长度 6→7 断言同步);L2 settingsCenter 新页用例(四开关渲染 + passthrough 警示文案断言)、`ipc-settings` 契约段键白名单;人工门禁登记:test-exemptions.md 增行「非默认 flags 矩阵(尤其 0x8)真实滚轮行为——自动化不可守卫(假阴性,spawn.rs:76-79 实证)——兜底 = ADR-0007 门禁第 3 条人工实测」。
5. **文档同步**:pty/CLAUDE.md「PASSTHROUGH_MODE (0x8) 永久禁用」节改写为「0x8 默认禁用 + 能力矩阵可配置化(CP-009)」;红线「永不启用 0x8」改写为「默认矩阵不含 0x8;任何 0x8 启用/默认翻转须过 ADR-0007 门禁第 3 条人工实测,无实测记录禁合入」;adr.md ADR-0007「后果」节追加 CP-009 口径(0x8 启用变更还须 `conpty_flags_default_matrix_matches_legacy_tristate` 守卫用例绿);settings.rs:14-21 白名单注释补 conptyInputModes 先例。
6. **验证**:`cargo test conpty -- --test-threads=1` 全绿(含三态等价用例);`rg "永不启用 0x8" src-tauri/src/pty/CLAUDE.md` 零命中;`npx tsc --noEmit` + `npm test` 绿;人工:门禁记录出现在合并说明——无 0x8 默认值变更时本项豁免执行。

---

# S09 编辑器大文件(1 项,豁免单一项)

## CP-022 · 大文件只读分片浏览路径(10MB 可编辑上限不变)(原章四)

1. **位置**:
   - `src/panels/editor/useCodeMirror.ts:46,48`(`MAX_FILE_SIZE_BYTES = 10_000_000` / `LARGE_FILE_WARN_BYTES = 1_000_000`)、`:305-309`(超限拒绝分支)
   - `src/panels/gitshow/GitShowPanel.tsx:177-185`(超限拒绝 + 大文件警告 header)
   - `src/panels/diff/DiffPanel.tsx:245-258`(head/workdir 双路超限检查)
   - 消费测试:`use-code-mirror.test.ts:1029,1047`、`gitshow-panel.test.tsx:345,362`、`diff-panel.test.tsx:729,741`
   - 既有通道先例:后端 `fs_read_file` 256KB Channel 分块(BE-03,editor/CLAUDE.md:28-29)
2. **现状**:useCodeMirror.ts:305-308 `sizeHint > MAX_FILE_SIZE_BYTES` → doc 替换为拒绝文案 + `filePathRef.current = undefined`;GitShowPanel.tsx:178 拒绝文案「文件过大……已拒绝打开以保护内存」;diff 双栏直接拒绝。CM6 不支持部分文档模型(FE-31 登记属实),但生态可只读分片——本条补只读路径,不改 10MB 可编辑上限语义。
3. **修复步骤(契约/模块级设计,代码给接口签名骨架)**:
   1. **后端 range 读块 IPC(新命令)**:`src-tauri/src/fs/` 新增 `fs_read_file_range(path, offset_bytes, length_bytes) -> Result<String, AppError>`——经 `spawn_blocking` 读指定字节区间(UTF-8 边界安全:读取后裁到首个完整字符边界);按硬约束 #3 三处注册(lib.rs generate_handler!、build.rs AppManifest、capabilities/default.json);DTO 双边(#4);`src/ipc/fs.ts` 增 wrapper:
      ```ts
      /** 分块读文件指定字节区间(CP-022 大文件只读浏览)——返回区间内完整 UTF-8 文本 */
      export function readFileRange(filePath: string, offsetBytes: number, lengthBytes: number): Promise<string> {
        return invoke("fs_read_file_range", { filePath, offsetBytes, lengthBytes });
      }
      ```
   2. **前端新模块 `src/panels/editor/largeFileViewer/`**(editor 面板子组件,不经 panelRegistry 新类型——同面板内形态切换):
      ```
      largeFileViewer/
        LargeFileViewer.tsx      — 宿主组件(接管渲染替代 CM)
        useLineIndex.ts          — 行起始偏移索引 hook
        blockCache.ts            — LRU 读块缓存
      ```
      关键接口签名骨架:
      ```ts
      // blockCache.ts
      /** 读块大小(字节)——对齐后端 256KB 分块先例(BE-03) */
      export const READ_BLOCK_BYTES = 256 * 1024;
      /** 缓存块数上限(LRU) */
      export const BLOCK_CACHE_LIMIT = 32;
      /** 按需读块(命中 LRU 直接返回;未命中经 ipc/fs.readFileRange 拉取) */
      export async function readBlock(filePath: string, blockIndex: number): Promise<string>;

      // useLineIndex.ts
      /** 行起始偏移索引——首块扫 \n 建初始索引,滚动至未索引区时按需向后扩展 */
      export function useLineIndex(filePath: string, fileSizeBytes: number): {
        /** 总行数(索引未覆盖到 EOF 时为下界估计值) */
        lineCount: number;
        /** 取行文本(虚拟化窗口调用;行所在块未载入则同步触发读块后重渲染) */
        getLine(lineIndex: number): string | undefined;
        /** 索引是否已覆盖到 EOF(行数从估计转精确) */
        fullyIndexed: boolean;
      };

      // LargeFileViewer.tsx
      interface LargeFileViewerProps {
        filePath: string;
        fileSizeBytes: number;
        /** 来源面板展示用(editor/gitshow/diff) */
        sourceLabel: string;
      }
      /** 只读大文件浏览:固定行高虚拟化行窗口(窗口 = 可见行数 + 上下 overscan 各 20 行),
       *  行内经 useLineIndex 按需读块;顶部信息条提示「只读浏览(文件大小),可编辑上限 10MB」 */
      export const LargeFileViewer: React.FC<LargeFileViewerProps>;
      ```
      虚拟化窗口实现要点(写死):行高 `LARGE_FILE_LINE_HEIGHT = 20`(复用编辑器同款字号,默认 14px,行高 1.4);容器 `overflow: auto` 自身为滚动容器;滚动事件换算 `startLine = floor(scrollTop / LINE_HEIGHT)`,仅渲染窗口内行(参照 FileTree 手实现虚拟化先例)。
   3. **超限拒绝语义改引导**:
      - `useCodeMirror.ts:305-309` 拒绝分支不再置错误 doc,改为向上报告:hook 返回值增 `largeFile: { filePath: string; sizeBytes: number } | null`;EditorPanel 检测 `largeFile` → 渲染 `<LargeFileViewer>` 替代 CM 编辑区;`filePathRef.current = undefined` 保留(防误保存覆盖原文件)。
      - `GitShowPanel.tsx:177-185`:`sizeHint > MAX_FILE_SIZE_BYTES` 分支改渲染 LargeFileViewer(`sourceLabel="git show"`);1MB-10MB 警告 header 语义不变。
      - `DiffPanel.tsx:245-258`:任一侧超限 → 该侧以 LargeFileViewer 展示(对齐/滚动同步对只读浏览侧降级为单文档浏览,占位对齐装饰跳过);两侧均超限时分栏各自 LargeFileViewer。
   4. 10MB 可编辑上限与 1MB 警告阈值常量不动(仍单点导出)。
4. **测试同步**:
   - L1:新 `fs_read_file_range_tests.rs`(临时文件全分支:空区间/越界 clamp/多字节字符边界不截断/不存在路径报 AppError)。
   - L2 新增 `large-file-viewer.test.tsx`:mock `ipc/fs.readFileRange`——行索引扩展、窗口渲染行数与滚动位置、LRU 驱逐;`useLineIndex` 单测:首块索引行数、跨块行拼接、多字节字符边界。
   - 改 `use-code-mirror.test.ts:1029`:「超过 MAX_FILE_SIZE_BYTES → 拒绝」改写为「→ 返回 largeFile 信号且 view 不创建」;:1047 警告用例不变。
   - 改 `gitshow-panel.test.tsx:345`:拒绝文案断言改 LargeFileViewer 渲染断言(data-e2e 锚);:362 警告 header 用例不变。
   - 改 `diff-panel.test.tsx:729,741`:超限用例改 LargeFileViewer 分栏断言。
   - 防复发:修复前「超限 = 静态拒绝文案」行为锁进上述改写用例的 before 形态断言(拒绝文案不再出现)。
5. **文档同步**:`src/panels/editor/CLAUDE.md`「大文件不虚拟化(FE-31 登记,D3 关闭)」节改写:CM6 部分文档模型不支持仍属实——可编辑域维持 10MB 上限 + 1MB 警告 + BE-03 分块三层防线;新增第四层「>10MB 只读分片浏览(CP-022:LargeFileViewer 虚拟化行窗口 + fs_read_file_range 按需读块)」,editor/gitshow/diff 超限拒绝语义改引导;`src/panels/CLAUDE.md` docViewer 家族/gitshow/diff 节补超限引导口径;`src/ipc/CLAUDE.md` 登记新 wrapper。
6. **验证**:`grep -n "fs_read_file_range" src-tauri/src/lib.rs src/ipc/fs.ts` 各 ≥ 1;`npx vitest run large-file-viewer use-code-mirror gitshow-panel diff-panel` 全绿;`cargo test fs_read_file_range -- --test-threads=1` 全绿;人工/L4:>10MB 文本经 editor 打开 → 只读浏览可滚动至 EOF 且行内容正确(抽样断言)。

---

# S10 预览通道根治(6 项,串行为主)

**Stage 定序(硬约束)**:① 多 webview WDIO 可达性 spike(隐性硬前置)→ ② webview 迁移(CP-012/013/031/044 同卡)→ ③ CP-033 新上下文重实证 → ④ CP-035 主窗口 CSP 终态回收。**② 未完成不得执行 ③④**;spike no-go 则整体回退「同态维持」备选(不做,仅登记)。
**任务卡注明**:复核 CP-037(display:none 保活)在新架构下的形态——预览迁 webview 后 workspace 层 CSS 显隐保活对预览 webview 是否仍然适用,结论登记 ADR-0019。

## S10-① 多 webview WDIO 可达性 spike(隐性硬前置,首任务;非 CP 项)

1. **位置**:技术选型现实——Tauri 2.11.5 多 webview 支持已实证可行;WDIO 侧 = `@wdio/tauri-service` embedded driver(webview2-com COM 直连)。
2. **现状**:预览通道当前 = 主窗口内 sandbox iframe srcDoc(继承主窗口 CSP);迁移后 WDIO 能否枚举/驱动独立预览 webview 未实证——这是整个 S10 的 go/no-go。
3. **修复步骤**(spike,产物 = 结论登记):
   1. 写最小 spike 工程(或本仓临时分支):主窗口 + 一个独立预览 webview(加载专用宿主页),WDIO 脚本验证:① webview 可被枚举(list/getWindowStates 或等价物);② `browser.execute` 在预览 webview 上下文可达;③ 焦点语义(预览聚焦时主窗口命令可达性);④ 销毁语义(关预览 webview 后 driver 不挂)。
   2. 同步实测预览 webview 加载方式两候选:**asset 协议宿主页**(首选,CSP 可控)vs **data: 注入**——结论随 spike 产物记录。
   3. 产物:go/no-go 结论 + 加载方式结论 + WDIO selector 可达策略,写入 `e2e-tests/CLAUDE.md` 与 ADR-0019 草稿。
   4. **no-go 出口**:S10 整体转「同态维持」——不迁移,仅登记保留现状的理由;CP-012/013/035/044 转休眠(compromises.md 各补 spike 结论注记),CP-031/033 同转。
4. **测试同步**:spike 代码不入库(临时分支/临时目录);结论驱动的正式 e2e 策略归 ②。
5. **文档同步**:spike 结论写 ADR-0019(新立)与 e2e-tests/CLAUDE.md。
6. **验证**:spike 四问各有实测答案(yes/no + 证据);结论落文档。

---

## S10-② · webview 迁移:CP-012(script-src 回收)+ CP-013(上行命令面收窄为零)+ CP-031(escapeScriptClose 消亡判定)+ CP-044(targetOrigin 收敛)——单任务卡

**波及面四区(任务卡必须显式覆盖)**:shortcuts 重放通道退役 / docViewer 层六件 + 六测试改写 / panelRegistry 白名单复核 + layoutSerde 存量迁移 / e2e 可达性适配(html.e2e.ts + markdown.e2e.ts)。

**CP-012 · CSP `script-src 'unsafe-inline'` + `dangerousDisableAssetCspModification` 放宽(原章三)**:

1. **位置**:`src-tauri/tauri.conf.json:24-27`(security 块);`src/__tests__/csp-config.test.ts:39-54`(两守卫);`src/panels/CLAUDE.md:164`(CSP 全局放宽红线);`src/panels/docViewer/CLAUDE.md` PreviewFrame 节;`.claude/adr.md:203`(ADR-0009 SEC-09)。
2. **现状**:tauri.conf.json:25 CSP 含 `script-src 'self' 'unsafe-inline'`;:26 `dangerousDisableAssetCspModification: ["script-src"]`;PreviewFrame.tsx:284 `srcDoc={injectScript(html, buildInjectedScript(nonce, segments), INJECTED_MARKER)}`——srcdoc 继承主窗口 CSP,注入脚本必须内联 → 主窗口被迫放行 unsafe-inline。
3. **修复步骤**:
   1. **webview 迁移契约**(执行 agent 照抄契约,实现适配):每个 docViewer 面板(htmlviewer/markdownviewer)的预览内容不再经主 window 内 iframe srcDoc,改在独立 Tauri webview 中渲染;预览 webview 加载专用宿主页,宿主页 CSP 单独放行内联脚本(注入机制 injectScript + buildInjectedScript + nonce 原样迁入,仍只在预览 CSP 域内宽松)。
   2. **消息桥契约**:替代 window.postMessage(跨 window 关系可能不成立,以 spike 结论为准);首选 Tauri event 通道或 webview 原生消息接口。消息集契约:上行仅渲染态(zoom/scroll/nav),下行 reset/zoom_set/scroll_set;不含任何命令重放。
   3. **主窗口 CSP 终态**:
      ```json
      "security": {
        "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: https://asset.localhost; font-src 'self' data:"
      }
      ```
      即删除整个 `"dangerousDisableAssetCspModification"` 键,CSP 字符串中 `script-src 'self' 'unsafe-inline'` 改为 `script-src 'self'`。(img-src/font-src 的 data: 回收归 ④ CP-035,本轮不动。)
4. **测试同步**:`csp-config.test.ts` 用例「script-src 放行同源 + 内联脚本/事件」(:39-47)改写为锁终态(`directives["script-src"]` 恰好等于 `["'self'"]`,CSP 字符串 `not.toMatch(/script-src[^;]*'unsafe-inline'/)`);「关闭 script-src 的 nonce 注入」(:49-54)删除,新增「dangerousDisableAssetCspModification 不存在」;img-src(:81-95)/font-src(:97-103)用例本轮不动(避免一次锁两个终态)。L2 四测试(html-panel/markdown-panel/doc-viewer-injection/doc-viewer-zoom-runtime)随容器改写同步重写(适配点名见 CP-013/044)。
5. **文档同步**:`src/panels/CLAUDE.md:164` 红线改为:「预览渲染于独立 webview(自有 CSP,内联脚本仅该域放行);主窗口 CSP 禁 script-src 'unsafe-inline'、禁 dangerousDisableAssetCspModification;srcdoc iframe 通道已退役。收紧主窗口 CSP 不再影响预览。」;`src/panels/docViewer/CLAUDE.md` PreviewFrame 节整体重写为 webview 架构红线(渲染容器/消息桥/nonce 语义/保活语义);`.claude/adr.md` 新增 **ADR-0019「预览渲染迁独立 webview」**(动机 = SEC-09 回收 + CP-012/013/035/044 同根、spike 结论、消息桥契约、波及面四区处置、CP-037 复核结论);ADR-0009 SEC-09 行尾追加「已被 ADR-0019 取代」。
6. **验证**:`grep -c "unsafe-inline" src-tauri/tauri.conf.json` = 1(仅剩 style-src);`grep -c "dangerousDisableAssetCspModification" src-tauri/tauri.conf.json` = 0;`npx vitest run src/__tests__/csp-config.test.ts` 全绿;L4:`html.e2e.ts` 缩放链路用例在新 webview 下通过;:87 不再 skip(见 CP-031)。

**CP-013 · HTML 预览 nonce 可被注入脚本内部伪造——上行命令面收窄为零(原章三)**:

1. **位置**:`buildInjectedScript.ts:58-63`(D16 威胁模型注释)、`:69-75`(基础段 keydown 转发,上行 slterm_key);`PreviewFrame.tsx:216-241`(slterm_key 通道:nonce 校验后查 global 命令集,合成 KeyboardEvent 重放,TRUSTED_MARKER 标记);`src/__tests__/command-catalog.test.ts:60-66`(global 命令集守卫,锁死 `[global.closeTab]`)。
2. **现状**:buildInjectedScript.ts:58-63 自登记:「nonce 明文内联于 srcDoc——iframe 内任意脚本可读取注入脚本提取 nonce 并伪造 slterm_key/slterm_zoom……键盘转发由 global 命令集最小化兜底(当前仅 global.closeTab)」;PreviewFrame.tsx:224-241 `registry.exportContextBindings("global")` 命中后合成 KeyboardEvent + TRUSTED_MARKER + dispatchEvent。**漂移留痕**:TRUSTED_MARKER 注释称「供 ShortcutRegistry 识别来源」,实测 ShortcutRegistry 全仓零消费——退役时注释与实现一并清理,不留「识别」伪契约。
3. **修复步骤**:
   1. `buildInjectedScript.ts` 删除基础段 keydown 转发(现 :69-75):注入脚本不再上行 slterm_key;`buildInjectedScript(nonce, extra)` 签名保留——nonce 仍拼入 zoom/scroll/nav 消息,父侧校验语义不变。
   2. `PreviewFrame.tsx`(或新宿主组件)删除 slterm_key 整分支(:216-241):删 ShortcutRegistry 查询、合成 KeyboardEvent、TRUSTED_MARKER、dispatchEvent 重放;上行处理只保留 ZOOM/SCROLL/NAV 三个渲染态分支。
   3. `buildInjectedScript.ts:29` 删 `TRUSTED_MARKER` 常量导出。
   4. 预览内键盘语义:预览 webview 聚焦时 Ctrl+W 等全局快捷键由主窗口 ShortcutRegistry window capture 层处理;若 spike 证实 webview 焦点吞键,补 host 级 keydown 透传(仅转发按键事件,不转发命令)。
   5. 上行消息终态集合(写死):`{slterm_zoom, slterm_scroll, slterm_nav}`;任何新增上行类型必须过面板守卫 + L2 白名单守卫(见 CP-044)。
4. **测试同步**:`command-catalog.test.ts` 用例「global context 命令集恒为 [global.closeTab]」改写为「预览消息通道不含命令重放」(断言 previewMessages.ts 上行类型集合不含 key 类型);`html-panel.test.tsx` keydown 重放用例(含 :659-660 `__slterm_postMessage` 断言)删除,新增负面用例「slterm_key 消息被静默忽略」(dispatch 带合法 nonce 的 slterm_key → 无 dispatchEvent、无 closeTab);`markdown-panel.test.tsx` 同步删 key 转发断言;`e2e-tests/html.e2e.ts:12-80`「iframe 内 Ctrl+W postMessage → 转发关闭」用例改写:新架构下经主窗口快捷键路径验证关闭链路,或按 spike 结论登记豁免。
5. **文档同步**:`buildInjectedScript.ts:58-63` 威胁模型注释删除,替换为:「上行仅渲染态(zoom/scroll/nav),无命令重放通道——SEC-04 内部伪造威胁面随 CP-012 webview 迁移消除(ADR-0019)」;`docViewer/CLAUDE.md` 上行通道列表删 slterm_key 条目;`previewMessages.ts` 头注释同步;`.claude/adr.md:230` D16 行尾追加「威胁模型已消除,ADR-0019」;adr.md:405 ADR-0017「global 命令集不因 md 扩充」行尾追加「global 重放通道已退役,ADR-0019」。
6. **验证**:`grep -rn "slterm_key" src/` = 0;`grep -rn "TRUSTED_MARKER" src/` = 0;`npx vitest run command-catalog html-panel` 全绿;L2 负面用例「slterm_key 被忽略」通过且为新增用例。

**CP-031 · escapeScriptClose 宿主 script 破坏——消亡判定(原章四;零独立代码)**:

1. **位置**:`src/lib/injectScript.ts:12-14`(escapeScriptClose 无差别转义)、`:40`(调用点);`e2e-tests/html.e2e.ts:87`(`it.skip("内联 <script> 与内联事件属性在预览中执行")`)。
2. **现状**:injectScript.ts:12-14 `return html.replace(/<\/script>/gi, "<\\/script>");`——宿主 HTML 内所有 `</script>` 被转义 → 宿主 script 吞到 EOF 永不执行。**漂移留痕(实读)**:html.e2e.ts:87 skip 用例是**空壳**(函数体仅一行注释),skip 注释登记的根因是 **CSP 'unsafe-inline'**(修复需动 tauri.conf.json),与 escapeScriptClose 是**两个独立缺陷通道**;escapeScriptClose 的 e2e 触发通道是 `<body onload>` 内联事件属性 fixture(html.e2e.ts:97-99 注释实证)。
3. **修复步骤(消亡判定,S10 收尾逐项断言,全中才销项)**:
   - 判定一:`grep -rn "escapeScriptClose" src/` 零命中(注入机制重写后该函数不存在,或整文件 injectScript.ts 已删除);
   - 判定二:新预览注入机制中宿主 HTML 的 `<script>` 段**不经任何字符串级转义**进入渲染文档(以新架构源码为准核对);
   - 判定三:预览 HTML 自带 `<script>` 在新架构下真实可执行——由改写后的 e2e 用例在真实 WebView2 验证。
   **html.e2e.ts:87 skip 用例改写方向**:用例恢复为真实断言(fixture HTML 内嵌宿主 `<script>` 设置文档标记或 postMessage 上行 → 新架构渲染后断言标记出现 → 取消 skip);若新架构宿主 script 仍受 CSP 约束,则改写为**事件属性通道**(`<body onload>`,同缩放用例先例)并在用例注释登记原因,skip 解除与否以 S10 架构实际能力为准;原空壳注释「CSP 修复后取消 skip 即可恢复」删除。
   **翻案出口**:若 S10 架构评审结论为「仍同态」,CP-031 翻案回独立修复(转义收窄为「仅注入点之前宿主部分」),届时重新起草。
4. **测试同步**:归 S10 条目;本条仅要求收尾时执行三条机械断言。
5. **文档同步**:`src/panels/CLAUDE.md`「宿主内联 `<script>` 不执行」节 S10 销项时整段删除,替换为新架构注入机制说明;`src/panels/markdown/CLAUDE.md` 外部坑同节同步;panels/CLAUDE.md:37 登记点(injectScript 相关)同步销项。
6. **验证**:`grep -rn "escapeScriptClose" src/` 退出码 1;`grep -n "it.skip" e2e-tests/html.e2e.ts` 相对 S10 前基线 -1;改写用例在 `npm run e2e`(html spec)真实通过。

**CP-044 · postMessage targetOrigin 恒 `"*"`(原章三)**:

1. **位置**:`buildInjectedScript.ts:16-17`(拼接纪律 4)、`:75`(基础段上行 `"*"`);`PreviewFrame.tsx:140-143`(resetZoom 下行)、`:267/:272/:274`(handleLoad 下行);`zoomRuntime.ts:23-26`(注释)、`:67`(上行);`docViewer/CLAUDE.md:19` 红线;上行校验 `PreviewFrame.tsx:165` `if (e.origin !== "null") return;`。
2. **现状**:全通道 targetOrigin 恒 `"*"`(SEC-03 实证:opaque origin 下传具体 origin 会被 Chromium 静默丢弃,`"*"` 是当前唯一可行形态);防护全押 nonce + global 命令集最小化。zoomRuntime.ts:23-26 注释原文:「targetOrigin 必须匹配【接收方】窗口 origin——iframe 为 opaque origin 只影响消息到达父后的 e.origin 序列化为 "null",与发送 targetOrigin 无关」。
3. **修复步骤**(迁移期加锁 + 落地后收敛,同卡两段):
   1. **迁移期(② 完成前)维持 `"*"`**,把防护不变量锁进 L2(见测试同步);本条不做独立代码改动。
   2. **独立 webview 落地后:origin 语义重估收敛**——预览 webview 加载固定宿主页后具备确定性 origin,`"*"` 收敛为单点常量:
      ```ts
      // previewMessages.ts 内新增(CP-044:独立 webview 后 targetOrigin 收敛单点;
      // 终值以 S10-① spike 实测 origin 为准,全仓唯一登记点)
      /** 预览 webview 固定 origin——上行校验与下行 targetOrigin 统一引用 */
      export const PREVIEW_ORIGIN = "https://slterm-preview.localhost";
      ```
      下行:新宿主所有 `postMessage(msg, "*")` 实参改 `PREVIEW_ORIGIN`;zoomRuntime 源码生成内上行的 `"*"` 改 `JSON.stringify(PREVIEW_ORIGIN)` 插值;上行:宿主侧校验 `e.origin !== "null"` 改 `e.origin !== PREVIEW_ORIGIN`;预览侧下行监听 `e.source !== win.parent` 校验保留。
   3. **备选结论登记**:若 spike 证实 webview 间不构成 window postMessage 关系(消息桥改 Tauri event),targetOrigin 问题随消息桥消亡——spike 产物须明确记录该结论,本条以「通道退役」销项,不再引入 PREVIEW_ORIGIN。
   4. 迁移期不变量(锁死):nonce 校验(现有)+ 命令集白名单(CP-013 后上行仅渲染态)+ 消息类型集合守卫。
4. **测试同步**:`doc-viewer-preview-messages.test.ts` 新增两守卫:「上行消息类型白名单恰好为渲染态集合」`[ZOOM_MSG_TYPE, SCROLL_MSG_TYPE, NAV_MSG_TYPE].sort()`;「下行消息类型白名单恰好为控制集合」`[RESET_MSG_TYPE, ZOOM_SET_MSG_TYPE, SCROLL_SET_MSG_TYPE]`;既有 origin 负面用例收敛后改写为 PREVIEW_ORIGIN 语义(收敛前不动);`doc-viewer-injection.test.ts` 注入源码断言收敛后 `"*"` 字面量断言改 PREVIEW_ORIGIN 序列化断言(防回归重新引入)。
5. **文档同步**:`docViewer/CLAUDE.md:19` 红线改为「独立 webview 前(迁移期)targetOrigin "*" + nonce/白名单不变量;独立 webview 后统一 `PREVIEW_ORIGIN` 单点(previewMessages.ts),上行校验 e.origin === PREVIEW_ORIGIN」;`buildInjectedScript.ts:16-17` 拼接纪律 4、`zoomRuntime.ts:23-26` 注释同步;ADR-0019 内登记收敛结论(含备选「通道退役」分支判定)。
6. **验证**:迁移期 `npx vitest run doc-viewer-preview-messages` 全绿;收敛后 `grep -rn '"\*"' src/panels/docViewer/ | grep -v PREVIEW_ORIGIN` = 0;`grep -rn "PREVIEW_ORIGIN" src/panels/docViewer/` ≥ 4;L4 html.e2e.ts 缩放/滚动往返用例在收敛后真实 WebView2 下通过。

---

## S10-③ · CP-033 KaTeX 字体内联产物:维持期 CI diff 守卫 + 新 webview 上下文重实证(原章一;硬前置 = ② 完成)

1. **位置**:`scripts/gen-katex-inline.mjs`(全 57 行)、`src/panels/markdown/generated/katexInlineCss.ts`(产物 369,263 字节 ≈ 361KB);`src/panels/markdown/mdPipeline.ts:36`(import)、`:237-243`(buildPreviewDocument head `<style>` 拼接);`src-tauri/tauri.conf.json:25-26`;`src/panels/markdown/CLAUDE.md:23`;`.claude/adr.md:430/435`(ADR-0018);`.github/workflows/ci.yml`;既有 L4 断言 `e2e-tests/markdown.e2e.ts:136-138`。
2. **现状**:gen 脚本头注释自述「~0.9MB 文本」为初版遗留,实际产物 361KB;防漏跑仅靠产物头「勿手改」注释与口头约定,无 CI 守卫。ADR-0018:435 被否决备选(asset 协议 + convertFileSrc)的否决前提是「opaque origin iframe 内行为未实证」——预览迁独立 webview 后此前提变化,正是重实证窗口。
3. **修复步骤**:
   - **A. 维持期 CI diff 守卫(两分支都要做)**:
     1. `.github/workflows/ci.yml` 在「Dead code check (knip)」步骤(:55-57)之后插入:
        ```yaml
            # KaTeX 内联产物 diff 守卫(CP-033):katex 升级漏跑 gen 脚本即红
            - name: Guard — KaTeX 内联产物与生成脚本一致
              run: |
                node scripts/gen-katex-inline.mjs
                git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts
        ```
     2. `scripts/gen-katex-inline.mjs:8` 注释「~0.9MB 文本」改为「产物约 361KB(2026-09-06 实测),勿手改」。
   - **B. 新上下文重实证(② 完成后执行)**:
     1. **实证步骤(固定)**:真实 WebView2 环境渲染含数学公式(`$x^2$` + 块级)的 md,断言:公式 DOM `getComputedStyle` font-family 命中 KaTeX 字体族(非 serif 回退)、DevTools 网络面板无字体请求失败/CORS 拒绝。实证通道:新增临时 L4 spec 或在 markdown.e2e.ts 加守卫用例。
     2. **分支 B1(asset 通道可行)→ 删生成物**:删 gen 脚本与产物;`mdPipeline.ts:36` 删 import,:240 head 改经 asset 协议引用 katex css/字体(URL 形态按 S10 webview 架构定,约束:运行时取字体、渲染产物不含 data: 字体串);`tauri.conf.json:25` CSP `font-src` 增 `asset: https://asset.localhost`(与 img-src 同形态);改 L4 断言通道;`markdown/CLAUDE.md:23` 节改写为运行时取字体口径;ADR-0018 追加「逆转记录」节。
     3. **分支 B2(仍不可行)→ 局部 CSP 兜底**:保留生成物;在 S10 新 webview 的局部 CSP 中显式放行 `font-src data:`(主窗口 CSP 不动);ADR-0018 追加实证结论(不可行证据 + 保留决策),CI 守卫(A)成为长期形态。
4. **测试同步**:A 步 CI 守卫自身即测试;B1 分支 `markdown.e2e.ts:136-138` 的「KaTeX 内联字体已装配」断言改为运行时字体通道断言(语义 = 公式字形命中 KaTeX 字体族),新增用例名建议「数学公式_块级与行内_字体族命中 KaTeX(非回退 serif)」;L2 `markdown-render-pipeline.test.ts` 若断言 head 含 KATEX_INLINE_CSS 特征串,同步改接新通道;B2 分支 L4 无新增,维持 A 守卫。
5. **文档同步**:A 步 `markdown/CLAUDE.md:23` 句尾追加「一致性由 CI diff 守卫(.github/workflows/ci.yml,CP-033)」;B1 分支整节改写 + ADR-0018「逆转触发点」节后追加「逆转记录(CP-033)」;B2 分支 ADR-0018 追加「维持记录(CP-033)」。
6. **验证**:A 步 `grep -n "KaTeX 内联产物 diff 守卫" .github/workflows/ci.yml` 命中;**红测**:本地临时改 katexInlineCss.ts 一个字符后 `node scripts/gen-katex-inline.mjs && git diff --exit-code src/panels/markdown/generated/katexInlineCss.ts` 退出码非 0,还原后退出码 0;B1:`test -f src/panels/markdown/generated/katexInlineCss.ts` 为假、`grep -rn "gen-katex-inline" scripts/ src/` 零命中、`npm run e2e` markdown spec 全绿且字体断言命中;B2:`grep -n "font-src" src-tauri/tauri.conf.json` 维持 `'self' data:`,新 webview CSP 处可见局部放行。

---

## S10-④ · CP-035 主窗口 CSP img-src/font-src `data:` 回收(原章三;硬前置 = ② 完成 + ③ 实证通过)

1. **位置**:`src-tauri/tauri.conf.json:25`(img-src 含 `data:`、font-src 含 `data:`);`src/__tests__/csp-config.test.ts:89-103`(img-src 守卫 :89-95、font-src 守卫 :97-103);资源通道:`src/panels/markdown/mdRenderAsync.ts`(资源 data: URL 替换)+ `src/panels/markdown/assets.ts`(MIME 白名单)。
2. **现状**:tauri.conf.json:25 `img-src 'self' data: asset: https://asset.localhost; font-src 'self' data:'`;csp-config.test.ts:89-95 锁 img-src 含 `data:` 且不含 `blob:`(注释自证「svg 经 `<img>` 惰性上下文加载(内嵌 script 不执行),data: 不承载脚本」——W3C 行为单点兜底);:97-103 锁 font-src 含 `data:`(KaTeX 字体内联,CP-033 同源项)。
3. **修复步骤**(硬前置 = ② 迁移完成 + ③ 新上下文重实证;两步未完成不得执行):
   1. 主窗口 CSP 终态(tauri.conf.json:25 整串替换,`dangerousDisableAssetCspModification` 已由 CP-012 删除):
      ```json
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: https://asset.localhost; font-src 'self'"
      ```
      即主窗口 img-src/font-src 双双回收 `data:`;`style-src 'unsafe-inline'` 保留(React inline style / CM6 注入样式,csp-config.test.ts:73-79 守卫,与本族无关)。
   2. 预览 webview CSP 独立放行 `data:`(img/font)——docViewer 资源内联通道只在预览 CSP 域宽松,不进主窗口。
   3. **③ CP-033 实证为 font-src 回收的硬前置**:实证通过才允许 font-src 回收;实证失败则 font-src 回收拆为独立后续项登记 `docs/compromises.md`,img-src 回收照常(图片通道与字体无关)。
   4. **svg data: 处置(默认执行)**:markdown `assets.ts` MIME 白名单剔除 `image/svg+xml`(显式禁用 svg 内联);html 侧资源通道同口径剔除。若执行期判定「`<img>` 惰性上下文」证据充分要保留 svg,必须在 ADR-0019 登记理由 + L2 锁白名单断言——默认按禁用执行。
4. **测试同步**:`csp-config.test.ts`——「img-src 放行 data:」(:89-95)改写为断言恰好等于 `["'self'", "asset:", "https://asset.localhost"]` 且不含 data:(blob: 否定保留);「font-src 放行同源 + data:」(:97-103)改写为恰好等于 `["'self'"]`;新增「data: 不在主窗口任何指令」:`for (const [name, sources] of Object.entries(directives)) { expect(sources).not.toContain("data:"); }`;markdown 资源 L2 新增「svg MIME 不在资源内联白名单」断言(既有放行用例改写为拒绝)。
5. **文档同步**:`src/panels/CLAUDE.md:32` ADR-0018 句改「……data: URL + 预览 webview CSP data: 放行(主窗口已回收)」;`src/panels/markdown/CLAUDE.md`「本地资源(决策 #9,ADR-0018)」节登记 svg 禁用;`.claude/adr.md` ADR-0018「逆转触发点」节标注:「已回收(ADR-0019 终步):主窗口 img-src/font-src data: 移除;KaTeX 字体经新预览上下文实证;svg data: 显式禁用」。
6. **验证**:`grep -o "data:" src-tauri/tauri.conf.json` = 0 行;`npx vitest run src/__tests__/csp-config.test.ts` 全绿;L4 markdown.e2e.ts 图片加载用例在预览 webview 下通过;font-src 实证记录(③ 产物)归档 ADR-0019。

---

# S11 Dockview 重设计(1 项,豁免单一项;前置 = S07 + S10 完成)

## CP-004 · 多 Dockview 实例 + MAX_PAGES=20——共享宿主 + 页组分组模型(原章二)

1. **位置**:
   - `src/stores/projects.ts:14-15`——`export const MAX_PAGES = 20;`(FE-01/D1 契约)
   - `src/stores/projects.ts:61-62`——`addPage` 超限拒绝返回 false 契约
   - `src/workspace/CLAUDE.md:11-17`——「多 Dockview 实例(H6)」节,**:15 笔误**「豁免登记见 ADR-0001 配套豁免表」(实为 ADR-0009,adr.md:202 FE-01 行)
   - `src/workspace/Workspace.tsx:72,99-113,146,152,229-230`——`initializedPages` Set + `ensurePageInitialized` 惰性初始化
   - `src/workspace/PageDockviewHost.tsx:721-735`——`display: visible ? "block" : "none"` 实例级 CSS 显隐 + `<DockviewReact>`
   - `src/workspace/layoutSerde.ts:10-12,77-105`——saveLayout/loadLayout(toJSON/fromJSON 单点,硬约束 #7)
   - `src/panelRegistry.ts:54-76`——panelRegistry 面板组件注册表
   - e2e:`e2e-tests/helpers.ts:260-301`(`__slterm_e2e_resetProjects`/`__slterm_e2e_addPage`/`__slterm_e2e_switchToPage`)、`e2e-tests/wdio.conf.ts:72-79`(beforeSuite 双 reset)、`e2e-tests/CLAUDE.md:58`(resetProjects 防 MAX_PAGES 触发登记)
2. **现状**:projects.ts:14 注释:「页面总数上限(FE-01/D1 契约)——多 Dockview 实例架构每页一实例,上限防内存/DOM 线性增长」;stores/CLAUDE.md FE-36 节:上限为跨项目全局计数;workspace/CLAUDE.md:13:「每个操作页面拥有独立 `<DockviewReact>` 实例。页面切换通过 CSS `display:none/block`,终端不销毁。根因:xterm.js 不支持二次 `open()`(Issue #4978)」;:15「ADR-0001」实指 ADR-0009(笔误实存)。
3. **修复步骤**(最大架构项,给到模块/契约级设计;执行 agent 不得另起方向):
   0. **笔误先行**(独立小步):workspace/CLAUDE.md:15 `ADR-0001 配套豁免表` → `ADR-0009 配套豁免表`。
   1. **共享宿主结构**:Workspace 渲染**单一** `<DockviewReact>` 共享宿主;删除 `Workspace.tsx:72` 的 `initializedPages` state 与 `ensurePageInitialized`(:99-113)——多实例 map 消亡,惰性初始化语义随实例消亡;`PageDockviewHost.tsx` 从「每页一实例组件」改造为「页组渲染组件」或删除,`window.__dockviewApi` 重指不变量收敛为「宿主唯一,指向即宿主」。
   2. **页组分组模型**:每操作页面 = 宿主内一个顶级 branch group,`groupId = page-${pageId}` 由纯函数产出:
      ```ts
      // src/workspace/pageGroups.ts(新)
      export const pageGroupId = (pageId: string): string => `page-${pageId}`;
      export const panelIdInPage = (pageId: string, localId: string): string => `${pageId}:${localId}`;
      export const pageOfPanelId = (panelId: string): string | null => panelId.includes(":") ? panelId.slice(0, panelId.indexOf(":")) : null;
      export function panelsOfPage(api: DockviewApi, pageId: string): IDockviewPanel[];
      ```
      **panelId 页前缀协议**(全仓承重契约,逐点核改):panelId 从「nextPanelId() 裸值」改为「页前缀 + localId」;冲击面 = titleManager 终端编号(`terminal-N` 每页从 0 起计数契约不变——编号仍 local)、tabClose 守卫 settings- 前缀判据(判据不变,dirtyRegistry 键同 params.panelId)、TerminalRegistry 键、SEC-08 `PtySession.panel_id` 归属校验(存全量前缀 id,校验逻辑零改)、DefaultTab params、文件型面板 findExistingEditor 查重键。跨页组拖拽禁止:宿主 `onDidAddPanel` 校验「panel 页前缀 == 目标 group 页前缀」,越界即回迁原页组(纯函数 `panelBelongsToGroup(panelId, groupId)`,L2 锁死)。
   3. **面板生命周期契约**(写死):新增面板 addPanel 时 options.group 显式指定目标页组;页面切换显隐从「实例级 display」改为「页组容器 display」(每页组 DOM 外包容器 div,切页只切页容器 display:none/block);**终端面板不随切页卸载/重建,xterm 实例只 open 一次(#4978 约束不变)**;`renderer="always"` 白名单语义不变——恒挂载面板在隐藏页组内保持挂载,fit/resize 仅在页组可见时执行;页面删除:先 kill 页组内全部终端面板,再移除空页组;`onDidLayoutChange` 恢复守卫(restoreGuardRef)语义不变,单宿主下全页组变更统一写回。
   4. **layoutSerde 契约演进**(硬约束 #7 单点不破):`saveLayout(api)` 不变(单宿主全量 toJSON),存储形态从「每页一份 SerializedDockview」改为「单宿主 JSON 一份 + 页组 id 索引」;projects store `OperationPage.layout` 字段语义改为「该页在宿主内的页组子树切片」;`loadLayout` 增页组提取 `loadPageGroup(api, pageId, saved)`(`reuseExistingPanels: true` 语义不变,白名单过滤与 patchLegacyLayout 照旧);`patchLegacyLayout` 增迁移:识别旧多实例格式(每页独立 layout.grid 无 page- 前缀组)→ 包成页组子树合入宿主;无法归组的面板丢弃 + console.error(不阻断启动,Watermark 接管空页语义不变)。
   5. **MAX_PAGES 消亡**:删 projects.ts:14-15 常量与 `addPage` 上限判定(含 toast「页面数已达上限」调用);上限消亡理由写进 stores/CLAUDE.md(实例数不再随页线性增长,容器/渲染管线共享)。
   6. **e2e 适配**:wdio.conf.ts:72-79 beforeSuite 双 reset 保留(隔离语义不变);e2e-tests/CLAUDE.md:58「防止跨 spec 累积触发 `MAX_PAGES=20` 上限」句改为「跨 spec 状态隔离」;helpers.ts 三个 helper 若经 pageId→api map 取数,改为 getPageApi(pageId) 页组查询。
4. **测试同步**:改 L2 workspace 系全量——「多实例各自存活」「CSS 显隐」「initializedPages 惰性初始化」用例改页组语义;stores/projects 上限用例(FE-36 跨项目计数、超限 false + toast)删除并替换为「addPage 无上限」用例;layoutSerde 旧格式用例保留 + 新增迁移用例(归组成功/脏面板丢弃两分支);新增 pageGroups.ts 纯函数用例(panelIdInPage/pageOfPanelId 往返、panelBelongsToGroup 越界判定)、跨页拖拽回迁用例;既有用例适配逐一点名:workspace-*.test.tsx、projects-store 测试、layoutSerde 测试、settings 面板跨页单例用例(F11 语义不变);L4:terminal.e2e.ts H6 跨页存活用例、settings.e2e.ts `__slterm_e2e_getSettingsPanelCount` 跨页用例(helpers.ts 适配后全量回归)。
5. **文档同步**:workspace/CLAUDE.md「多 Dockview 实例(H6)」节(:11-17)整节重写为共享宿主页组模型(含 #4978 约束不变、跨页拖拽禁止、面板生命周期契约),:15 笔误随步骤 0 修正;stores/CLAUDE.md「FE-01/FE-36 页面总数上限」节改写为「上限随多实例架构消亡(S11 共享宿主)」;adr.md ADR-0009 FE-01 行(adr.md:202)改写「多实例保持决策被本项取代(CP-004 修复:转共享宿主 + 页组模型,上限消亡)」;panels/CLAUDE.md renderer=always 白名单语义若措辞关联同步核修;e2e-tests/CLAUDE.md:58 句改。
6. **验证**:`rg "MAX_PAGES|initializedPages|ensurePageInitialized" src/` 零命中;`npx tsc --noEmit`、`npx eslint src/` 退出码 0;`npm test` 全绿;`npm run e2e` 全绿(H6 跨页存活、settings 跨页计数用例必须绿);**人工验证点**:双页各开终端 + 编辑器,切页 20 次往返,终端输出不丢、编辑器无重建闪屏。

---

# S12 收尾(1 CP 项 + 销项总扫 + 全量回归;并行 ≤3)

## CP-023 · Rust 行覆盖 88.20% 收尾——生产代码口径重测 + pty 纯逻辑抽取收敛(原章五)

1. **位置**:
   - `.claude/test-exemptions.md:25`——TQ-COV 收尾登记行(88.20% 含测试代码口径,目标 90% 差 1.8pp)
   - `src-tauri/src/main.rs:1-7`——可覆盖行 = 4/5/6 共 3 行(fn main 壳 + install_panic_hook + run 调用),结构性零覆盖
   - pty 模块既有纯逻辑抽取先例:`src-tauri/src/pty/spawn.rs:95`(`build_cmdline`)、`:121`(`build_env_block`)
   - 测试模块分布(全部 `#[cfg(test)] mod <领域>_tests`,待加 `#[coverage(off)]`,约 40 处):home.rs / app_dir.rs / state.rs / settings.rs / fs/mod.rs(6 处)/ error.rs / hooks/{watcher,signal,provider,mod×2}.rs / hooks/claude/{config,mod×2,inject}.rs / agent_history/{provider,mod}.rs / agent_history/claude/{mod×4,scan,ops,jsonl}.rs / projects.rs / lib.rs(:157)/ pty/{spawn×2,shell,conpty_api,reader}.rs / background_tasks/{registry,mod×2}.rs / plan_balance/{deepseek,mod×2,kimi,query,source}.rs / notify/{mod,pool}.rs
2. **现状**:test-exemptions.md:25 原文:「Rust 行覆盖 88.20%(llvm-cov 含测试代码口径)| 目标 90% 差 1.8pp;残余缺口集中 PTY Win32 分支 + main.rs 结构性零覆盖 + 编译器生成物计数缺失」;main.rs 全文 7 行,L1 无法启动 tauri 运行时,与豁免表 `lib.rs run()` 行(:15)同构;本机已装 cargo-llvm-cov 0.9.0,`--help` 仅有文件级 `--ignore-filename-regex`,无 cfg(test) 代码级排除开关——口径变更须走 Rust 侧 `#[coverage(off)]` 属性(rustc 1.96 已稳定支持,模块级生效)。
3. **修复步骤**:
   1. **生产代码口径改造(机械批处理)**:对上述测试模块清单中每个**文件级** `#[cfg(test)] mod <名> {` 紧跟一行加 `#[coverage(off)]`(属性写在 mod 上,模块内全部测试代码退出计数;模块内 `#[cfg(test)]` 子项与内联 guard 不动)。示例:
      ```rust
      #[cfg(test)]
      #[coverage(off)]
      mod spawn_tests {
      ```
      完成后 `grep -rn "#\[cfg(test)\]" src-tauri/src | wc -l` 与改造前相同。
   2. **基线重测**(S02 已恢复默认 lib test target,直接全量):`cargo llvm-cov -- --test-threads=1`,记录摘要行覆盖率 P_new(生产代码口径)。全量测试必须全绿(exit 0)——覆盖率数字只在全绿时有效。
   3. **目标重定(写死分支)**:
      - 若 P_new ≥ 90%:删 test-exemptions.md:25 整行,在豁免表「原豁免表脚注」区(:43 一带)补一行销记「TQ-COV 收尾已达成(生产口径 P_new,YYYY-MM-DD 实测)」;本条完成。
      - 若 P_new < 90%:把 :25 行改写为「Rust 行覆盖 P_new(生产代码口径,llvm-cov + #[coverage(off)] 于测试模块)| 目标 90% 差 X.pp;残余缺口 = 本表 pty/ 各行逐条登记项之和」,X 写实测值;继续步骤 4。
   4. **pty 缺口系统性收敛**:`cargo llvm-cov --html -- --test-threads=1` 出报告,逐文件扫 `pty/{spawn,reader,conpty_api,shell,win_build}.rs` 未覆盖区(不含豁免表已登记行);对每个未覆盖**判定/计算分支**,照 `build_cmdline`/`build_env_block` 先例抽为不依赖 Win32 句柄/运行时状态的纯函数(同文件就近,`fn` 私有),补领域测试模块用例;确属 Win32 API 组合无法纯化的,逐条进 pty/CLAUDE.md 既定豁免表(项目/原因/兜底层级三列)+ 同步 test-exemptions.md 汇总行,**禁止再新开「收尾」型笼统登记**。
   5. **main.rs 3 行处理(写死)**:test-exemptions.md 豁免表新增一行:「`main.rs` fn main 3 行胶水 | 结构性零覆盖:L1 无法启动 tauri 运行时安装 panic hook + run() | L4 `terminal.e2e.ts` 启动链真实执行 + `lib.rs run()` 行同构豁免先例」;不给 fn main 加 `#[coverage(off)]`(生产代码语义,登记豁免更诚实)。
4. **测试同步**:步骤 4 每抽一个纯函数,新增用例名 = 「对象_行为_场景」snake_case 裸名,落在所属文件的领域测试模块;既有用例零适配(`#[coverage(off)]` 不改变测试行为);防复发维度:本次为覆盖补齐而非 bugfix,无回归面;抽取过程不得改变任何生产行为(纯机械移动,签名/语义逐字保持)。
5. **文档同步**:test-exemptions.md:25 按步骤 3 写死口径改写或销记;步骤 5 新增 main.rs 行(位置在 lib.rs run() 行之后);`src-tauri/src/pty/CLAUDE.md`「既定豁免」表步骤 4 逐条新增项同步,表头注记补「生产代码口径 = 测试模块 #[coverage(off)](TQ-COV 收尾,YYYY-MM)」;根 CLAUDE.md「测试策略」表不改。
6. **验证**:`cargo test -- --test-threads=1` exit 0,用例数与改造前全量相等;`cargo llvm-cov -- --test-threads=1` exit 0 且摘要百分比 = 登记新值(±0.1pp);`grep -rn "#\[coverage(off)\]" src-tauri/src | wc -l` ≥ 40,且与文件级测试模块清单逐一对应(抽查 pty 5 文件 + lib.rs 必须命中);`grep -c "main.rs" .claude/test-exemptions.md` ≥ 1。

---

## S12 销项总扫 + 全量回归(非 CP 任务)

1. **compromises.md 销项总扫**:44 项逐条核对销项勾选 + 修复注记(格式照 CP-015 先例);各 Stage 遗留的「转休眠/翻案」项(CP-001 机检未触发、CP-003 分支 B、CP-007 指纹分支、CP-029 环境出口、CP-031 同态维持、CP-033 分支 B2 等)确认登记口径一致;文档与代码一致性抽查(抽 ≥5 个已销项的登记点原文核对)。
2. **全量回归验收**:`cargo test -- --test-threads=1`、`npm test`、`npm run test:l3`、`npm run e2e` 四级全绿;`npx tsc --noEmit`、`npx eslint src/`、clippy、rustfmt、`npx knip --production` 静态门禁全绿;`npx tauri build --debug --no-bundle` 构建成功。
3. **人工验证点总收**:CP-009(claude 滚轮)、CP-010(Win10 回退 toast)、S10(预览回归)、S11(布局全场景)的人工验收记录汇总归档。

---

# 附:各章起草附注汇总(漂移点 + 编排线索)

## 章一(依赖与技术选型,review-01)

1. CP-003 行号精确;替换注意 :286-288 的 `else { fallback(); }` 属同一块,新块末尾 `fallback();` 已合并其语义,勿残留旧 else。
2. CP-002 触点扩面:package.json:50(`json-schema` 仅类型 import 消费)、vitest 双配置 inline 登记、hooks-config-jsonmode.test.tsx 大面积 mock——执行 agent 不得只改 JsonMode.tsx。
3. CP-038 成因定论:pin 是 78cb1b8 首次引入即精确形态;「查成因」的答案就是「无成因」,按无成因路径落步骤,异常分支(pin 成因浮现)有落点。
4. CP-032 成因已实挖(a027b17/1233336),执行 agent 只剩「上游是否放开硬钉」一项评估。
5. knip.json 旧路径遗留(非本批范围):ignoreIssues 仍列 `src/panels/hooksConfig/*`(:156-184,F11 迁址后旧目录已不存在);CP-002 删 `json-schema` 依赖时 knip 会实际校验 unusedDependencies——若漏删必红。旧路径清理建议归 S08 同 agent 顺手或另立条目。
6. CP-003 同步面扩面:wdio.conf.ts:5 与 run-wdio.cjs:1-4、CLAUDE.md:42 同述「Node 22 便携启动器」;S01(CP-032)插入新节锚定「### E2E helper 命名与挂载位置」节避免行号漂移。
7. CP-033 产物体积登记失真:「~0.9MB」实 361KB——A 步含注释修正。
8. e2e-tests/CLAUDE.md:42 禁用裸 npx wdio 的理由换锚:版本坑消失后理由换为隔离链。

## 章二(后端架构与平台,review-02)

1. CP-004 e2e 触点比登记广:wdio.conf.ts:72-79 双 reset、e2e-tests/CLAUDE.md:58、helpers.ts 三 helper 全部已写入步骤;workspace/CLAUDE.md:15 笔误实证(ADR-0001 → ADR-0009)。
2. CP-005 站点数漂移:登记「8 处」低估——生产站点实含 notify/mod.rs;测试局部与 #[cfg(test)] 槽位须一并换装才能收敛 grep 至零。执行 agent 不得只改登记点名的文件。
3. CP-005 与 CP-034 的 reader.rs 冲突:4a 先行,034 以 parking_lot 换装后为基线。
4. CP-007 触点扩面:force 契约三面文本登记(src/ipc/CLAUDE.md:64、backgroundTasks/CLAUDE.md:41、agentHistory/CLAUDE.md:32)删除分支须同步;BE-19 未登记于 ADR-0009 表,无 ADR 改写。
5. CP-008 测试断言实证:tests/git_status_tests.rs:48 有 `(IGNORED, Some("ignored"))` 直接映射断言——删分支必改;前端消费 = 纯渲染容错,零改动保留。
6. CP-009 白名单连锁:加键即动 SEC-11 白名单与对应测试;ADR-0007 门禁第 3 条实读 = adr.md:171;flags 消费点实读 spawn.rs:1153。
7. CP-010 三处注册实证;「warn 保留并接上抛」落法 = warn 文案与 fallback_reason 同一 `format!("{e:#}")` 变量。
8. CP-011 失真注释四处实位(spawn.rs:1424-1425、1464-1467、1520-1524、pty/CLAUDE.md:52-54);真实阻塞点在 ConPtyInner::drop(spawn.rs:440-448);「先关输出管道句柄」支不采用理由已录。
9. CP-034 测试构造点实扩:spawn.rs:2091 测试直接构造 PtySession 含两字段;PtySession.channel 全仓消费方 grep 确认仅 reader 一线。
10. Stage 编排:S04-4a 先行单 agent;CP-011/034 同 agent;CP-008/010 与 011/034 零重叠可并行;S06 两项零重叠可并行;S08 CP-009 与 S04 的 settings 测试同文件——S04 先行;S11 与 S04/S06/S08 全零重叠。
11. ts-rs 前置实证:2026-09-06 实读 Cargo.toml 与 src-tauri/src 均无 ts-rs——CP-006 执行前必须确认 S05 已落仓。
12. CP-007 基准门槛机器可检性:CI IO 抖动可能 flaky——尾部门槛 max < 200ms 即为此设;放宽顺序「先 max、后中位」,放宽须留数字依据。

## 章三(安全放宽,review-03)

1. CP-043 路径漂移:hooks/inject.rs 实为 `hooks/claude/inject.rs`(MC-213 下沉);inject.rs:119「S19 文档同步」历史编号随注释改写清理。
2. CP-013 行号微漂移:PreviewFrame.tsx 重放段实为 :227-240;威胁模型注释 :58-63 补登。
3. TRUSTED_MARKER 消费方失真:ShortcutRegistry 全仓零消费——退役时注释与实现一并清理。
4. command-catalog 守卫位置::63-66 用例名「global context 命令集恒为 [global.closeTab]」;按 CP-013 步骤改写而非删除守卫意图。
5. csp-config.test.ts 行号属实(:39-54 script-src 两守卫、:89-95 img-src、:97-103 font-src)。
6. shell.rs 行号属实(:109-138);Cargo.toml windows features 现状无 Win32_Storage_FileSystem,须新增。
7. docViewer L2 测试面宽于输入清单:四测试之外尚有 `doc-viewer-preview-messages.test.ts` 与 `doc-viewer-floating-area.test.tsx`——S10-② 波及面须一并处理。
8. hooks 命令注册为「三处」(lib.rs + build.rs + capabilities)。

## 章四(前端架构,review-04)

1. CP-031 与 html.e2e.ts:87:skip 空壳根因是 CSP 'unsafe-inline',与 escapeScriptClose 是两个独立缺陷通道——已按「两个独立缺陷、S10 一并消亡」起草。
2. CP-017 破口面:页删除路径(Workspace 删页 → Dockview 实例销毁)不在 CP-036 决策范围——S07 收尾复核点已留在 CP-036 第 3 步。
3. CP-016 决策措辞落地:「受控组件」= props 方向受控(viewState/onViewStateChange),状态本体存 sideViewRegistry 模块级 plain Map(隐藏视图不渲染,无需响应式订阅);zones/open/splitRatio 不动。
4. CP-020 状态机裁定:不新增 interrupted 态(理由见条目首段)。
5. CP-039 运行期切换路径:setActive 生产调用点仅 main.tsx:67(启动期)——本条先消除 CM 掉队,运行期切换 UI 不在本条。
6. CP-042 行号:轮询体实在 pageApis.ts:140-159。
7. S07 内重叠结论与编排建议(017→036→042→016→037→019)已并入 S07 节头。
8. 跨 Stage 线索:CP-039+CP-002 共碰 JsonMode.tsx(039 先);CP-021+CP-026 契约/实现合并(S01);CP-018 与 CP-019 文件不相交但运行路径相邻;CP-022 独立;CP-031 零代码。

## 章五(测试覆盖缺口,review-05)

1. CP-045「唯一名免清空语义已在头注」部分失真:头注 :13 仍写旧固定名——销项步骤须连头注一起改(已写入)。
2. CP-028 登记外第三处同构循环:history.e2e.ts:123-148 `ensureAllProjectsExpanded`——不在本条范围,建议 S03 同 agent 顺手同改。
3. CP-024「58 文件」实测 = 58 文件 74 处 import;存在深导入 `types/<域>` 形态 → 生成物必须同名同路径覆盖。
4. CP-024 Rust HookHandler 面窄于 TS:单源化前置「扩 Rust DTO」必须发生(BE-18/SEC-05 校验语义不变)。
5. CP-024 TitleSource 双边语义有意不一致:Rust 枚举 vs TS 开放字符串——保留 TS 别名于 local.ts,不生成。
6. **CP-040 波及既有清单**:review-02/03/04 验证节多处 `cargo test --test lib_tests <filter>` 形态,S02 拆除后失效——后续 Stage 执行统一按 `cargo test <filter> -- --test-threads=1` 等价适配(本清单头部「验证命令形态注意」已统调)。
7. CP-040 附加事实:`.cargo/config.toml` 不存在;src/lib.rs:157 存在内嵌 `mod lib_tests`(与 [[test]] 目标同名偶合,执行 agent 勿混淆——拆的是 Cargo.toml 目标,不是该模块);tauri-service 本地 dist 与 review 登记逐点吻合。
8. CP-023 工具事实:cargo-llvm-cov 0.9.0 无 cfg(test) 代码级排除开关——生产口径只能走 `#[coverage(off)]`(rustc 1.96 稳定);llvm-cov 全量必须在 S02 之后跑。
9. CP-029/041 事实补充:Workspace.tsx:235 注释指 SEC-01 上提(已完成但失败仍在);hooks 配置写命令的「未知 cliId」与 history 注册表相互独立(hooks/provider.rs:88),CP-041 不破坏 mockcli.e2e.ts 用例②。
10. S03 编排线索已并入 S03 节头(CP-028/041 共改 mockcli.e2e.ts;CP-041/029 环境出口均触 run-wdio.cjs,并入合并体序列)。

## 章六(遗留清理,review-06)

1. CP-026 ticker 行号::90(now state)+ :92-97(effect)——条目按全文照抄方式消除行号依赖。
2. CP-025 误导注释范围:仅注释滞后,mock 本体早已指向 navTree——改注释即闭合。
3. CP-027 陈旧行号:main.tsx 超时页色值实为 :36-43(:28-30 为 SEC-10 注释块)——闭合时交叉引用整块删除顺带消除。
4. 「既定例外」字面仅 main.tsx:30 一处;其余为交叉引用/手动同步登记,条目逐一列全。
5. CP-026 死面佐证:knip.out:169 标 AgentStatusState 未使用、knip.json:22-24 types 豁免压制——条目含 knip.json 销项步骤。
6. agentStatus/CLAUDE.md 载「useAgentStatus 留存不改(NAV-01)」与 CP-026 收窄决策相悖——以锁定决策为准,文档随迁改写。
7. e2e 注释零漂移:e2e-tests/ 7 处注释按 hook 名引用 useAgentStatus——hook 名不变,注释不动。
8. S01 内五项分工文件重叠表与「CP-021/026 拆 agent 会造成编译断裂窗口」预警已并入 S01 节头。






