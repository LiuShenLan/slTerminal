# 章六「遗留清理与同步点」修复清单(CP-025 / CP-026 / CP-027)

> 起草纪律:每个修复点均经现状代码实读核对(行号为 2026-09-07 工作区实态);已锁定决策只落成可照抄步骤,不另起方向。三项均归 **Stage 01**。章四 CP-021 条目锁契约(NavTree.tsx 落点、60s setInterval、`now` prop 替换 NavHistoryRow:38 内部 `Date.now()`),本条目的 CP-026 为其实现侧,同一 agent 合并落地。

---

## CP-025 · 退役 sidebar 目录物理删除 [Stage 01]

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

4. **测试同步**:
   - 无行为变化(mock 本体早已指向 navTree),不新增用例;`agent-history-restore.test.ts` 全部既有用例原样通过即回归锁。
   - 防复发断言(注释与目标漂移回归):验证节 grep 锁 `features/sidebar` 在 `src/` 零命中。

5. **文档同步**:
   - `src/features/sidebar/CLAUDE.md` 随目录删除即闭合,无迁移内容(navTree/CLAUDE.md「CRUD 迁移承接(NAV-06)」已承载其有效约定)。
   - 其余 CLAUDE.md 无涉。

6. **验证**:
   - `test ! -e src/features/sidebar`(退出码 0)。
   - `grep -rn "features/sidebar" src/ .claude/skills/` 退出码 1(零命中;compromises 文档与 knip.out 为历史产物不计)。
   - `npx vitest run agent-history-restore` 全绿。

---

## CP-026 · useAgentStatus 迁入 navTree + 返回面收窄 + 60s ticker 移交宿主 [Stage 01]

> 实现侧与章四 CP-021 同 agent 合并落地:本条目收窄返回面并把 ticker 显式移交 NavTree 宿主,消弭 S01→S08 之间相对时间冻结回归窗口。

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
   2. 返回面收窄——`src/features/navTree/useAgentStatus.ts` 全文照抄为(迁移 + 死面删除一步完成;import 深度不变,两目录同为 `src/features/<x>/`,`../cliProfiles` 等相对路径原样保留):
      ```ts
      // useAgentStatus.ts —— 活跃会话行数据 hook(CP-026 自 features/agentStatus 迁入 navTree)
      //
      // 行 = 运行中的编码 CLI 会话(非全部终端)。
      // 建行双通道:sessionChange(session 非 null)∨ hook 事件(非 SessionEnd/Exit 且行不存在)——两通道独立幂等。
      // 删行三通道:sessionChange(session 为 null)∨ SessionEnd/Exit hook 事件 ∨ remove 事件。
      // 初始扫描只建 agentSession 非 null 的行。usage 数据源 = ContextUsage 信号事件
      // (statusline 桥接通道,官方 used_percentage 口径——行存在才更新,不建行/删行/不动状态)。
      // #5 竞态双保险:① registry/agent-event 双 listener 经 ref 读最新状态,effect deps [] 订阅永不重建;
      // ② 初始扫描按注册表现值对账(agentSession 非 null 才建行),兜底任何事件丢失。
      //
      // 行 cliId(MC-410):hook 事件通道建行按 MC-205 三级解析
      // (payload.cliId → TerminalRegistry.get(panelId)?.agentSession?.cliId → CLAUDE_CLI_ID)写入;
      // OSC 133 通道建行经 setAgentSession 的 sessionChange 自然驱动(cliId 取 agentSession.cliId,缺省兜底)。
      //
      // CP-026:返回面收窄为 AgentSessionRow[]——state/currentProjectName/now 为已退役视图时代
      // 死面,删除;相对时间 60s ticker 移交 NavTree 宿主(CP-021),now 经 prop 注入 NavHistoryRow。

      import { useState, useEffect, useRef, useCallback, useMemo } from "react";
      import { useLayout } from "../../stores/layout";
      import { useProjects } from "../../stores/projects";
      import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
      // ZQ-2: 来源 CLI 标识三级解析单点(契约 4)——空串/空白 cliId 同等回退
      import { resolvePayloadCliId } from "../../panels/terminal/resolvePayloadCliId";
      import { onAgentEvent } from "../../ipc/agentHooks";
      import { parseTerminalPageId } from "../../lib/panelId";
      import { getPageApi } from "../../workspace/pageApis";
      import { cliProfileRegistry } from "../cliProfiles";
      // AC-5: 事件名字面量只允许出现在 profiles/claude/(claude 合法领地)——
      // SessionEnd/Exit/ContextUsage 判定一律引用本常量,不写字面量
      import {
        CLAUDE_CLI_ID,
        CONTEXT_USAGE_EVENT,
        SESSION_END_EVENT,
        EXIT_EVENT,
      } from "../cliProfiles/profiles/claude";
      import type { AgentStatus } from "../../lib/agentStatus";
      import type { AgentEventPayload, ContextUsageSignal } from "../../types/agent";

      // ---- 类型定义 ----

      /** 单行 Agent 会话数据 */
      export interface AgentSessionRow {
        panelId: string;
        pageId: string;
        projectId: string;
        /** 会话所属 CLI 标识(hook 事件通道 = 三级解析结果;OSC 133 通道 = agentSession.cliId)——供行 logo 查 profile */
        cliId: string;
        /** 会话 UUID(hook 事件 payload.sessionId;matchedCommand-only 会话缺省)——供视图层标题覆盖匹配 */
        sessionId?: string;
        title: string;
        status: AgentStatus;
        lastEventAt: number;
        usageSourcePath?: string;
        /** context 用量信号(ContextUsage 事件推送——官方 used_percentage 口径;未收到 → undefined) */
        usage?: ContextUsageSignal | null;
      }

      // ---- 辅助函数 ----

      /** 根据 panelId 查找页签标题,无 dockviewApi 或面板时回退 */
      function resolveTitle(panelId: string, pageId: string): string {
        try {
          const api = getPageApi(pageId);
          if (!api) return `终端 ${pageId}`;
          const panel = api.getPanel(panelId);
          return panel?.title ?? `终端 ${pageId}`;
        } catch {
          return `终端 ${pageId}`;
        }
      }

      // ---- Hook ----

      export function useAgentStatus(): AgentSessionRow[] {
        const activePageId = useLayout((s) => s.activePageId);
        const projects = useProjects((s) => s.projects);
        const [rows, setRows] = useState<AgentSessionRow[]>([]);

        // 跟踪事件回调引用(避免 onAgentEvent 重建订阅)
        const rowsRef = useRef(rows);
        rowsRef.current = rows;

        // 当前活跃项目
        const projectList = Object.values(projects);
        const activeProject = projectList.find((p) =>
          p.pages.some((pg) => pg.pageId === activePageId),
        );

        const projectPageIds = useMemo(
          () => new Set(activeProject?.pages.map((pg) => pg.pageId) ?? []),
          [activeProject],
        );

        const projectRoot = activeProject?.rootPath ?? null;

        // generation 计数器(照 useFileTree 先例):项目切换时递增,
        // 初始扫描 setRows 前检查——快速切项目时丢弃过期扫描结果
        const genRef = useRef(0);

        // ref 副本供稳定订阅(dept [])回调读取最新值,防 R4 竞态(remove 事件丢失)
        const projectRootRef = useRef(projectRoot);
        projectRootRef.current = projectRoot;
        const projectPageIdsRef = useRef(projectPageIds);
        projectPageIdsRef.current = projectPageIds;
        const activeProjectRef = useRef(activeProject);
        activeProjectRef.current = activeProject;

        // ---- 面板标题订阅(行 title 动态跟随页签,人工验证问题 3 一致化) ----
        // 行 title 是建行时刻的面板标题快照(resolveTitle)——页签异步标题覆盖
        // (useXterm refreshSessionTitle → api.setTitle)后行不更新(/resume 侧栏
        // 固定 claude 根因)。订阅面板 onDidTitleChange → 行 title 实时同步;
        // 订阅失败(页面 api 未就绪/面板已卸载)→ 静默跳过,行保持快照标题。
        const panelTitleSubsRef = useRef<Map<string, () => void>>(new Map());

        /** 取消单个面板标题订阅(幂等:不存在条目零操作;删行时调用) */
        const unsubscribePanelTitle = useCallback((panelId: string) => {
          panelTitleSubsRef.current.get(panelId)?.();
          panelTitleSubsRef.current.delete(panelId);
        }, []);

        /** 全量取消(项目切换/无项目/卸载——初始扫描 effect cleanup) */
        const unsubscribeAllPanelTitles = useCallback(() => {
          for (const dispose of panelTitleSubsRef.current.values()) dispose();
          panelTitleSubsRef.current.clear();
        }, []);

        /** 建行时订阅面板 onDidTitleChange → 行 title 实时更新(幂等先清旧订阅) */
        const subscribePanelTitle = useCallback(
          (pageId: string, panelId: string) => {
            unsubscribePanelTitle(panelId); // StrictMode 双渲染/重复建行安全
            try {
              // 必须用行所在页面的 api(跨页 panelId 不混用)
              const panel = getPageApi(pageId)?.getPanel(panelId);
              if (!panel?.api?.onDidTitleChange) return; // 未就绪 → 静默不订阅
              const disposable = panel.api.onDidTitleChange((e) => {
                // e.title 为 TitleEvent.title(dockviewPanelApi.d.ts,TerminalPanel:152 先例)
                setRows((prev) =>
                  prev.map((r) =>
                    r.panelId === panelId ? { ...r, title: e.title } : r,
                  ),
                );
              });
              panelTitleSubsRef.current.set(panelId, () => disposable.dispose());
            } catch {
              // 面板 api 获取异常 → 不订阅(行保持快照标题,resolveTitle 兜底不变)
            }
          },
          [unsubscribePanelTitle],
        );

        // ---- hook 事件处理(deps []——所有数据经 ref 读取,回调永不重建) ----
        const handleHookEvent = useCallback(
          (payload: AgentEventPayload) => {
            const projRoot = projectRootRef.current;
            if (!projRoot) return;

            const pageId = parseTerminalPageId(payload.panelId);
            if (!pageId) return;

            const pageIds = projectPageIdsRef.current;
            if (!pageIds.has(pageId)) return;

            // ContextUsage 信号:行存在才更新 usage(不建行/删行/不动状态——事件先于
            // 建行到达时忽略;桥接脚本 1s 节流保证行建立后很快有数据);字段缺失 → 忽略
            if (payload.event === CONTEXT_USAGE_EVENT) {
              if (typeof payload.usedPercentage === "number") {
                const pct = payload.usedPercentage;
                setRows((prev) =>
                  prev.map((r) =>
                    r.panelId === payload.panelId
                      ? { ...r, usage: { usedPercentage: pct } }
                      : r,
                  ),
                );
              }
              return;
            }

            const proj = activeProjectRef.current;
            if (!proj) return;

            // MC-205 三级解析单点(ZQ-2,契约 4):payload.cliId(trim 后非空)→ 注册表
            // agentSession.cliId(反查)→ CLAUDE_CLI_ID(缺省兼容旧信号);
            // 空串/仅空白与 null/undefined 同等回退(原 ?? 链遇空串短路失效)
            const cliId = resolvePayloadCliId(payload);
            const profile = cliProfileRegistry.get(cliId);
            // MC-206:未知 cliId(未注册)或无 hooks 能力 → console.warn + 跳过(不建行/不置图标/不通知),不抛异常
            if (!profile?.capabilities?.hooks) {
              console.warn(
                `未知 cliId ${cliId} 的 hook 事件已跳过——未注册或缺少 hooks 能力`,
              );
              return;
            }

            const newStatus = profile.capabilities.hooks.eventToStatus(
              payload.event,
              payload.notificationType,
            );

            // SessionEnd / Exit → 删行(同步取消面板标题订阅防泄漏)
            if (payload.event === SESSION_END_EVENT || payload.event === EXIT_EVENT) {
              unsubscribePanelTitle(payload.panelId);
              setRows((prev) => {
                const idx = prev.findIndex((r) => r.panelId === payload.panelId);
                if (idx === -1) return prev;
                const next = [...prev];
                next.splice(idx, 1);
                return next;
              });
              return;
            }

            const pageTitle = resolveTitle(payload.panelId, pageId);

            setRows((prev) => {
              const existingIdx = prev.findIndex(
                (r) => r.panelId === payload.panelId,
              );

              if (existingIdx >= 0) {
                // 更新已有行——null 状态不覆盖旧值
                const next = [...prev];
                next[existingIdx] = {
                  ...next[existingIdx],
                  title: pageTitle,
                  ...(newStatus !== null ? { status: newStatus } : {}),
                  lastEventAt: payload.timestamp || Date.now(),
                  usageSourcePath:
                    payload.usageSourcePath ?? next[existingIdx].usageSourcePath,
                  sessionId: payload.sessionId ?? next[existingIdx].sessionId,
                };
                next.sort((a, b) => b.lastEventAt - a.lastEventAt);
                return next;
              }

              // 建新行:hook 事件通道(非 SessionEnd/Exit 且行不存在——与 sessionChange 通道独立幂等)
              // ZQ-3 决策 2:null 映射事件建行但 status null(无图标)——感知存活
              // (SessionStart 丢失场景:事件到达即会话存在,行必须出现)且不误标
              // attention(null 状态表示「无状态」,与 deriveActiveSessionStatuses
              // 「status 为 null 不产出键」语义一致)
              const row: AgentSessionRow = {
                panelId: payload.panelId,
                pageId,
                projectId: proj.projectId,
                cliId,
                title: pageTitle,
                status: newStatus,
                lastEventAt: payload.timestamp || Date.now(),
                usageSourcePath: payload.usageSourcePath || undefined,
                sessionId: payload.sessionId || undefined,
                usage: undefined,
              };

              const next = [row, ...prev];
              next.sort((a, b) => b.lastEventAt - a.lastEventAt);
              return next;
            });
            // 建行后订阅面板标题——行 title 随页签标题演进(异步覆盖通道)
            subscribePanelTitle(pageId, payload.panelId);
          },
          [], // deps []——所有动态数据经 ref 读取,回调永不重建
        );

        // 订阅 onAgentEvent(deps [handleHookEvent],handleHookEvent deps [] 故永不重建)
        useEffect(() => {
          const unlisten = onAgentEvent(handleHookEvent);
          return () => {
            unlisten();
          };
        }, [handleHookEvent]);

        // ---- TerminalRegistry 订阅:sessionChange 建/删行 + remove 删行 ----
        // deps []——订阅永不重建,remove 事件永不丢失(根除 R4 根因:同 commit passive destroy
        // 顺序 SideBarArea 先于主区,旧 deps 重订阅窗口内 remove 丢失)
        useEffect(() => {
          const unsub = TerminalRegistry.subscribe((event) => {
            const pageIds = projectPageIdsRef.current;
            const proj = activeProjectRef.current;
            if (!proj) return;

            const pageId = parseTerminalPageId(event.panelId);
            if (!pageId) return;
            if (!pageIds.has(pageId)) return;

            if (event.type === "sessionChange") {
              const entry = TerminalRegistry.get(event.panelId);
              if (!entry) return;

              if (entry.agentSession && entry.agentSession !== null) {
                // OSC 133 通道建行的行 cliId:agentSession.cliId(MC-107 命中时写入),缺省兜底防旧数据/mock
                const rowCliId = entry.agentSession.cliId ?? CLAUDE_CLI_ID;

                // session 非 null → 建行(幂等:行已存在则跳过)
                setRows((prev) => {
                  if (prev.some((r) => r.panelId === event.panelId)) return prev;
                  const row: AgentSessionRow = {
                    panelId: event.panelId,
                    pageId,
                    projectId: proj.projectId,
                    cliId: rowCliId,
                    title: resolveTitle(event.panelId, pageId),
                    status: "attention",
                    lastEventAt: entry.agentSession!.lastEventAt,
                    usageSourcePath: entry.agentSession!.usageSourcePath,
                    sessionId: entry.agentSession!.sessionId,
                    usage: undefined,
                  };
                  return [...prev, row].sort((a, b) => b.lastEventAt - a.lastEventAt);
                });
                // 建行后订阅面板标题(幂等:重复建行先清旧订阅)
                subscribePanelTitle(pageId, event.panelId);
              } else {
                // session 为 null → 删行(同步取消面板标题订阅)
                unsubscribePanelTitle(event.panelId);
                setRows((prev) => {
                  const idx = prev.findIndex((r) => r.panelId === event.panelId);
                  if (idx === -1) return prev;
                  const next = [...prev];
                  next.splice(idx, 1);
                  return next;
                });
              }
            } else if (event.type === "remove") {
              // remove → 删行(同步取消面板标题订阅)
              unsubscribePanelTitle(event.panelId);
              setRows((prev) => {
                const idx = prev.findIndex((r) => r.panelId === event.panelId);
                if (idx === -1) return prev;
                const next = [...prev];
                next.splice(idx, 1);
                return next;
              });
            }
            // register 事件不建行——建行由 sessionChange(非 null)和 hook 事件双通道负责
          });

          return unsub;
        }, []); // deps []——订阅永不重建

        // ---- 初始扫描 + 项目切换(只建 agentSession 非 null 的行;携 usageSourcePath 主动拉 usage) ----
        useEffect(() => {
          // FE-23: generation 递增——本项目扫描结果的 setRows 前检查,
          // 快速切项目时旧扫描(理论上的慢 resolveTitle 等异步延伸)不覆盖新状态
          const gen = ++genRef.current;
          if (!projectRoot || !activeProject) {
            // 无项目 → 清行 + 全量取消面板标题订阅(防跨项目残留监听)
            unsubscribeAllPanelTitles();
            setRows([]);
            return;
          }

          // 遍历 TerminalRegistry,只建 agentSession 非 null 的行
          const allTerminals = TerminalRegistry.getAll();
          const initialRows: AgentSessionRow[] = [];

          for (const [panelId, entry] of allTerminals) {
            if (!entry.agentSession) continue; // 纯 shell 终端不建行

            const pageId = parseTerminalPageId(panelId);
            if (!pageId) continue;
            if (!projectPageIds.has(pageId)) continue;

            // 初始扫描建行的行 cliId:agentSession.cliId,缺省兜底防旧数据/mock
            const rowCliId = entry.agentSession.cliId ?? CLAUDE_CLI_ID;

            initialRows.push({
              panelId,
              pageId,
              projectId: activeProject.projectId,
              cliId: rowCliId,
              title: resolveTitle(panelId, pageId),
              status: "attention",
              lastEventAt: entry.agentSession.lastEventAt,
              usageSourcePath: entry.agentSession.usageSourcePath,
              sessionId: entry.agentSession.sessionId,
              usage: undefined,
            });
          }

          initialRows.sort((a, b) => b.lastEventAt - a.lastEventAt);
          // FE-23: generation 过期检查——项目已切换则丢弃本次扫描结果
          if (gen !== genRef.current) return;
          setRows(initialRows);
          // 扫描建行后逐行订阅面板标题(subscription 在 cleanup 全清,无残留)
          for (const row of initialRows) subscribePanelTitle(row.pageId, row.panelId);

          // 项目切换/卸载 → 全量取消面板标题订阅(行随下轮扫描重建)
          return () => unsubscribeAllPanelTitles();
        }, [projectRoot, activeProject?.projectId]);

        return rows;
      }
      ```
   3. 改接线 `src/features/navTree/useNavTree.ts`:
      - :24-25 两行改为:
        ```ts
        import { useAgentStatus } from "./useAgentStatus";
        import type { AgentSessionRow } from "./useAgentStatus";
        ```
      - :99 改为:
        ```ts
        const rows = useAgentStatus();
        ```
      - 文件头 :5 注释「活跃会话:useAgentStatus(rows——...)」不变(hook 名未变)。
   4. 改 `src/features/navTree/NavSessionRow.tsx:14`:
      ```ts
      import type { AgentSessionRow } from "./useAgentStatus";
      ```
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
      - 解构(:29-34)改为:
        ```ts
        export const NavHistoryRow: React.FC<NavHistoryRowProps> = ({
          session,
          status,
          now,
          onDoubleClick,
          onContextMenu,
        }) => {
        ```
      - :38 改为:
        ```ts
        const timeStr = formatRelativeTime(session.mtimeMs, now);
        ```
   7. 删除 `src/features/agentStatus/CLAUDE.md`(有效约束已随迁,见文档同步节),目录即空闭合。

4. **测试同步**:
   - `src/__tests__/agent-status-hook.test.ts`:
     - :144 import 改为 `import { useAgentStatus } from "../features/navTree/useAgentStatus";`。
     - 「状态机派生」组(:305-333)三用例改写为 rows-only 断言——`result.current` 现为数组,`result.current.state` 全部改为对 `result.current` 的直接断言(如 `expect(result.current).toEqual([])`),用例名相应去态语义(「无活跃项目时返回空行数组」等)。
     - 「now ticker」组(:335-387)三用例**删除**(ticker 已移交 NavTree 宿主,hook 内不再有 ticker 可测)。
     - 新增用例「返回面收窄回归(CP-026)」:
       ```ts
       it("返回面收窄:结果为数组,不含 state/currentProjectName/now 死面(契约)", () => {
         seedProject();
         const { result } = renderHook(() => useAgentStatus());
         expect(Array.isArray(result.current)).toBe(true);
         expect("state" in result.current).toBe(false);
         expect("now" in result.current).toBe(false);
         expect("currentProjectName" in result.current).toBe(false);
       });
       ```
       (防复发对照:修复前该 hook 返回对象四面。)
     - 其余用例中 `result.current.rows` 统一改为 `result.current`,`result.current.rows[0]` 改为 `result.current[0]`(:402-1441 全部逐处适配)。
   - `src/__tests__/nav-tree.test.tsx`:
     - :51 mock 路径改为 `"../features/navTree/useAgentStatus"`;:138 type import 改为 `"../features/navTree/useAgentStatus"`。
     - mock 形状收窄——删除全部 `state: { kind: "ready" },` / `currentProjectName: ...,` / `now: ...,` 三行,保留 `rows`(resetAll :261-266 及 :307-310、:357、:400、:430、:551、:742、:988、:1329、:1367 共 10 处 `mockReturnValue` 逐一适配)。
   - `src/__tests__/nav-tree-history.test.tsx`:
     - :35 mock 路径改为 `"../features/navTree/useAgentStatus"`;:210-215 resetAll 形状改为:
       ```ts
       mockUseAgentStatus.mockReturnValue({ rows: [] });
       ```
     - 新增宿主 ticker 用例组「CP-021 宿主 60s ticker(渲染层与数据层节奏解耦)」:
       ```tsx
       it("CP-021:历史行相对时间随宿主 ticker 跨 60s 自动刷新(fake timers)", async () => {
         vi.useFakeTimers();
         try {
           const base = Date.now();
           seedProject("C:/projA", "proj-A", "项目A", [{ pageId: "pageA", name: "页面 A" }]);
           seedActivePage("pageA");
           mockScanHistory.mockResolvedValue([
             makeHistorySession({ mtimeMs: base - 5 * 60_000 }), // 5 分钟前
           ]);
           const { container } = render(<NavTree />);
           const node = await expandHistoryNode(container);
           expect(node.textContent).toContain("5 分钟前");

           // 59s:未跨档,文本不变(防误刷新)
           act(() => {
             vi.advanceTimersByTime(59_000);
           });
           expect(node.textContent).toContain("5 分钟前");

           // 再 1s(累计 60s):宿主 ticker 触发,now 推进 → 6 分钟档
           act(() => {
             vi.advanceTimersByTime(1_000);
           });
           expect(node.textContent).toContain("6 分钟前");
         } finally {
           vi.useRealTimers();
         }
       });
       ```
       (makeHistorySession 默认 mtimeMs 为 `Date.now() - 5 * 60_000`,:168;用例显式传 mtimeMs 锁定基准。`act` 自 `@testing-library/react` 导入——该文件 :20 当前只导入了 `describe, it, expect, beforeEach, afterEach, vi`,需补 `act`。)
   - `src/__tests__/nav-history-row.test.tsx`:
     - `renderRow`(:78-96)改签名,`now` 显式传入(锁「行内不再有自主 Date.now」):
       ```tsx
       function renderRow(
         session: AgentHistorySession,
         props: { status?: AgentStatus | null; now?: number } = {},
       ) {
         const onDoubleClick = vi.fn();
         const onContextMenu = vi.fn();
         const now = props.now ?? Date.now();
         const utils = render(
           <NavHistoryRow
             session={session}
             status={props.status}
             now={now}
             onDoubleClick={onDoubleClick}
             onContextMenu={onContextMenu}
           />,
         );
         const row = utils.container.querySelector(
           '[data-e2e="nav-row-session"]',
         ) as HTMLElement;
         return { ...utils, row, onDoubleClick, onContextMenu, now };
       }
       ```
     - :107 与 :193 两处 `formatRelativeTime(session.mtimeMs, Date.now())` 改为 `formatRelativeTime(session.mtimeMs, now)`(用 renderRow 返回值)。
     - 新增用例「相对时间跟随 now prop 重算(CP-021,防行内 Date.now() 复发)」:
       ```tsx
       it("rerender 传 now+60s → 相对时间文本随 prop 重算(无内部 Date.now)", () => {
         const session = makeSession({ mtimeMs: Date.now() - 60_000 });
         const { row, now, rerender } = renderRow(session);
         expect(row.textContent).toContain(
           formatRelativeTime(session.mtimeMs, now),
         );
         const later = now + 60_000;
         rerender(
           <NavHistoryRow
             session={session}
             status={undefined}
             now={later}
             onDoubleClick={vi.fn()}
             onContextMenu={vi.fn()}
           />,
         );
         expect(row.textContent).toContain(
           formatRelativeTime(session.mtimeMs, later),
         );
       });
       ```

5. **文档同步**:
   - 删除 `src/features/agentStatus/CLAUDE.md`;其有效约束随迁 `src/features/navTree/CLAUDE.md`——在「层级与数据源」节后新增一节:
     ```
     ### 活跃会话行数据 hook(useAgentStatus,CP-026 自 agentStatus 迁入)

     - **行建模**:行 = 运行中的编码 CLI 会话(agentSession 为 null 的纯 shell 不建行)。建行双通道幂等(sessionChange 非 null ∨ hook 事件非 SessionEnd/Exit 且行不存在);删行三通道(sessionChange null ∨ SessionEnd/Exit ∨ remove)。初始扫描只建 agentSession 非 null 的行。
     - **行 cliId(MC-410/MC-205/ZQ-2)**:hook 事件通道经 resolvePayloadCliId 三级解析单点(payload.cliId trim 非空 → 注册表反查 → CLAUDE_CLI_ID 缺省;空串/空白同等回退);OSC 133 通道取 agentSession.cliId;未知 cliId/无 hooks 能力 → console.warn + 跳过(MC-206)。
     - **建行 status(ZQ-3 决策 2)**:hook 通道建行 status 原样写入(null 映射事件建行但 status null 无图标——感知存活且不误标 attention);更新已有行 null 不覆盖旧值。
     - **ContextUsage 信号分支(AC-5)**:行存在才更新 usage(usedPercentage 数字校验),不建行/删行/不动状态;事件名一律经 profiles/claude 导出常量,禁字面量。
     - **行 title 动态跟随页签**:建行三通道订阅面板 onDidTitleChange → 行 title 实时同步;订阅表 Map<panelId, dispose>,删行三通道/项目切换/卸载时取消。
     - **FE-23 generation 防竞**:照 useFileTree 先例——项目切换递增 genRef,初始扫描 setRows 前检查。
     - **项目域过滤**:hook 内部按活跃项目过滤(projectPageIds/projectRoot 经 ref 供稳定订阅读取)。
     - **返回面契约**:useAgentStatus 返回 AgentSessionRow[](CP-026 收窄——state/currentProjectName/now 死面已删,不得回加);相对时间 60s ticker 由 NavTree 宿主单点持有(CP-021),数据 hook 不自建 ticker。
     ```
   - `src/features/navTree/CLAUDE.md`「硬约束」节「数据 hook 不自建订阅」条补一句:「相对时间 60s ticker 由 NavTree 宿主单点持有(CP-021,now 经 prop 注入 NavHistoryRow),数据 hook 不自建 ticker」。
   - `src/features/agentHistory/CLAUDE.md:89` MC-318 整条改写为:「1. **历史区相对时间刷新(CP-021 已修)**:`formatRelativeTime` 渲染时计算,相对时间基准 `now` 由 navTree 宿主 60s ticker 驱动重算——与 sessionRefresh 数据层节奏解耦,禁用/慢档不冻结。」(「已知限制(MC-318)」标题可保留作历史锚,或改为「历史区相对时间刷新」。)
   - `knip.json`:删除 `ignoreIssues` 中 `"src/features/agentStatus/useAgentStatus.ts": ["types"]` 条目(:22-24,含尾逗号行)——迁移后文件路径变更且 AgentSessionRow 有非测试消费,豁免无存在理由。

6. **验证**:
   - `test -f src/features/navTree/useAgentStatus.ts && test ! -e src/features/agentStatus`(退出码 0)。
   - `grep -rn "features/agentStatus" src/ knip.json` 退出码 1(零命中)。
   - `grep -n "Date.now()" src/features/navTree/NavHistoryRow.tsx` 退出码 1(零命中)。
   - `grep -n "60_000" src/features/navTree/NavTree.tsx` ≥ 1 命中。
   - `grep -n "currentProjectName\|AgentStatusState\|AgentStatusResult" src/features/navTree/useAgentStatus.ts` 退出码 1(零命中)。
   - `npx tsc --noEmit` 绿;`npx eslint src/` 绿。
   - `npx vitest run agent-status-hook nav-tree nav-history-row` 全绿(nav-tree filter 覆盖 nav-tree.test.tsx 与 nav-tree-history.test.tsx)。
   - `npx knip` 无新增 issue(agentStatus 条目已删,AgentSessionRow 有生产消费)。

---

## CP-027 · 启动链 fail-safe 三处静态色构建期注入闭合 [Stage 01]

> 与 CP-038 同 agent(package.json 收敛单 agent):scripts 段接线归本项,devDependencies pin 归 CP-038,同一文件串行改。

1. **位置**:
   - `index.html:10`:`<body style="margin: 0; padding: 0; overflow: hidden; background: #0a0a0b;">`
   - `src-tauri/tauri.conf.json:21`:`"backgroundColor": "#0a0a0b"`
   - `src/main.tsx:28-47`(fail-safe 页;:30 注释含「既定例外」字样;:36 `background = "#0a0a0b"`、:37 `color = "#ece9e4"`、:43 `messageSpan.style.color = "#d9706b"`)
   - 色源:`src/theme/schemes/linear.ts:74`(`appBgPrimary: "#0a0a0b"`)、:77(`sidebarFg: "#ece9e4"`)、:78(`errorFg: "#d9706b"`)
   - 登记/交叉引用待删:`linear.ts:6-11` 文件头 fail-safe 交叉引用块、`src/theme/CLAUDE.md:54` 与 `:62`、`main.tsx:30`、`.claude/adr.md` ADR-0002 :58 被否决备选、ADR-0003 :80 对接段
   - 打包脚本:`.claude/package.ps1:23`(`npx tauri build`——经 tauri.conf.json `beforeBuildCommand: npm run build` 触发 prebuild 钩子)

2. **现状**:
   - 三处硬编码与 linear.ts 现值一致(2026-09-06 核查实证,本次复核仍一致);`linear.ts:6-11`:
     ```
     // 交叉引用(启动链 fail-safe):React 挂载前的静态硬编码色不在方案系统内,
     // 改本文件对应 ui 值时必须手动同步——
     //   ui.appBgPrimary (#0a0a0b) ↔ index.html:10 body background
     //   ui.appBgPrimary (#0a0a0b) ↔ src-tauri/tauri.conf.json:21 window backgroundColor
     //   ui.panelBg (#0a0a0b) / ui.errorFg (#d9706b) ↔ src/main.tsx:28 超时错误页(文字 #ece9e4)
     ```
     (注:其中 `main.tsx:28` 为陈旧行号,实际色值行 :36-43——闭合时整块删除即顺带消除。)
   - `main.tsx:30`:`// 视觉效果与原模板逐项一致(深色底 + 居中 + 错误红;色值属启动链 fail-safe 既定例外)。`

3. **修复步骤**:
   1. 新建单一色源消费模块 `src/theme/startupColors.ts`(构建期生成物,照 `src/panels/markdown/generated/katexInlineCss.ts` 生成物先例入库):
      ```ts
      // startupColors.ts — 启动链 fail-safe 静态色(构建期生成物,勿手改)
      // 色源单一 = schemes/linear.ts(appBgPrimary/sidebarFg/errorFg)——生成器 scripts/sync-startup-colors.mjs(CP-027),
      // 经 package.json predev/prebuild 接线;改 linear.ts 三槽位后跑 npm run sync:startup-colors 即同步三处消费点。
      export const STARTUP_FAIL_SAFE_BG = "#0a0a0b";
      export const STARTUP_FAIL_SAFE_FG = "#ece9e4";
      export const STARTUP_FAIL_SAFE_ERROR_FG = "#d9706b";
      ```
   2. 新建生成器 `scripts/sync-startup-colors.mjs`,全文照抄:
      ```js
      // sync-startup-colors.mjs — 启动链 fail-safe 静态色同步(CP-027)
      // 色源单一:src/theme/schemes/linear.ts 三个 ui 标量(appBgPrimary/sidebarFg/errorFg);
      // 提取后改写三处消费点:index.html body background、src-tauri/tauri.conf.json backgroundColor、
      // src/theme/startupColors.ts(生成物)。纯 Node ESM,零依赖;提取失败 → 非零退出即红。
      import { readFileSync, writeFileSync } from "node:fs";
      import { fileURLToPath, pathToFileURL } from "node:url";
      import { dirname, join, resolve } from "node:path";

      const root = join(dirname(fileURLToPath(import.meta.url)), "..");

      /** 从 linear.ts 文本提取三个 fail-safe 槽位(锚定标量键,值为小写 6 位 hex) */
      export function extractStartupColors(linearTs) {
        const pick = (key) => {
          const m = linearTs.match(new RegExp(`\\b${key}:\\s*"(#[0-9a-f]{6})"`));
          if (!m) {
            throw new Error(`linear.ts 未找到槽位 ${key}(CP-027 锚定提取失败,禁止自估色值)`);
          }
          return m[1];
        };
        return {
          bg: pick("appBgPrimary"),
          fg: pick("sidebarFg"),
          errorFg: pick("errorFg"),
        };
      }

      /** 生成 startupColors.ts 全文(照抄锁形态,测试同款断言) */
      export function renderStartupColorsModule({ bg, fg, errorFg }) {
        return [
          "// startupColors.ts — 启动链 fail-safe 静态色(构建期生成物,勿手改)",
          "// 色源单一 = schemes/linear.ts(appBgPrimary/sidebarFg/errorFg)——生成器 scripts/sync-startup-colors.mjs(CP-027),",
          "// 经 package.json predev/prebuild 接线;改 linear.ts 三槽位后跑 npm run sync:startup-colors 即同步三处消费点。",
          `export const STARTUP_FAIL_SAFE_BG = "${bg}";`,
          `export const STARTUP_FAIL_SAFE_FG = "${fg}";`,
          `export const STARTUP_FAIL_SAFE_ERROR_FG = "${errorFg}";`,
          "",
        ].join("\n");
      }

      function rewriteIfChanged(path, next) {
        const prev = readFileSync(path, "utf8");
        if (prev === next) return false;
        writeFileSync(path, next);
        return true;
      }

      function main() {
        const linearTs = readFileSync(join(root, "src/theme/schemes/linear.ts"), "utf8");
        const colors = extractStartupColors(linearTs);

        // index.html body 底色
        const indexHtmlPath = join(root, "index.html");
        const indexHtml = readFileSync(indexHtmlPath, "utf8");
        const nextIndexHtml = indexHtml.replace(
          /background:\s*#[0-9a-fA-F]{6};/,
          `background: ${colors.bg};`,
        );
        if (nextIndexHtml === indexHtml) throw new Error("index.html body background 改写失败");

        // tauri.conf.json 窗口底色
        const confPath = join(root, "src-tauri/tauri.conf.json");
        const conf = readFileSync(confPath, "utf8");
        const nextConf = conf.replace(
          /"backgroundColor":\s*"#[0-9a-fA-F]{6}"/,
          `"backgroundColor": "${colors.bg}"`,
        );
        if (nextConf === conf) throw new Error("tauri.conf.json backgroundColor 改写失败");

        const changed = [
          rewriteIfChanged(indexHtmlPath, nextIndexHtml) && "index.html",
          rewriteIfChanged(confPath, nextConf) && "src-tauri/tauri.conf.json",
          rewriteIfChanged(
            join(root, "src/theme/startupColors.ts"),
            renderStartupColorsModule(colors),
          ) && "src/theme/startupColors.ts",
        ].filter(Boolean);

        console.log(
          changed.length
            ? `[sync-startup-colors] 已同步: ${changed.join(", ")}`
            : "[sync-startup-colors] 三处消费点与 linear.ts 一致,无改动",
        );
      }

      // 仅直接执行时跑主流程(vitest import 提取/渲染函数无副作用)
      if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
        main();
      }
      ```
   3. `package.json` scripts 段(:6-18)接线——追加三行(与 CP-038 同 agent 串行改同一文件):
      ```json
      "sync:startup-colors": "node scripts/sync-startup-colors.mjs",
      "predev": "npm run sync:startup-colors",
      "prebuild": "npm run sync:startup-colors",
      ```
      (npm 生命周期:predev 挂到既有 `dev`、prebuild 挂到既有 `build`;`npx tauri build` 经 `beforeBuildCommand: npm run build` 触发 prebuild,`.claude/package.ps1` 的 `npx tauri build --no-bundle` 同链路覆盖,用户 `npx tauri build --debug --no-bundle` 习惯路径同样覆盖。)
   4. `src/main.tsx`:
      - 顶部静态 import(:1-4 区域)追加:
        ```ts
        // CP-027:fail-safe 静态色读构建期生成常量(色源 = linear.ts;本模块零依赖,
        // 不触发 theme facade 求值——启动链静态 import 面约束不破)
        import {
          STARTUP_FAIL_SAFE_BG,
          STARTUP_FAIL_SAFE_ERROR_FG,
          STARTUP_FAIL_SAFE_FG,
        } from "./theme/startupColors";
        ```
      - :30 注释行改为:
        ```ts
        // 视觉效果与原模板逐项一致(深色底 + 居中 + 错误红;色值经 startupColors 常量,
        // 构建期自 linear.ts 注入——CP-027)。
        ```
      - :36/:37/:43 三处字面量替换:
        ```ts
        container.style.background = STARTUP_FAIL_SAFE_BG;
        container.style.color = STARTUP_FAIL_SAFE_FG;
        ```
        ```ts
        messageSpan.style.color = STARTUP_FAIL_SAFE_ERROR_FG;
        ```
   5. 删交叉引用与例外登记:
      - `src/theme/schemes/linear.ts`:删除文件头 :6-11 交叉引用整块(保留 :1-5 值契约锚点与 :12-13 下文)。
      - `src/theme/CLAUDE.md:54` 改写为:
        ```
        **启动链 fail-safe 三处静态色**(先于方案加载,不随方案切换):色源 = `schemes/linear.ts`(`appBgPrimary`/`sidebarFg`/`errorFg`),构建期由 `scripts/sync-startup-colors.mjs` 提取改写 `index.html` body background、`tauri.conf.json` backgroundColor、`src/theme/startupColors.ts`(生成物,勿手改);`main.tsx` 超时错误页读 `startupColors.ts` 常量(零依赖模块,不触发 facade 求值)。接线 = package.json `predev`/`prebuild`——硬约束 #6 自此无例外。
        ```
      - `src/theme/CLAUDE.md:62` 红线条删除;「外部坑/红线」节如有需要可补「`startupColors.ts` 为生成物禁手改;改 linear 三槽位后须 `npm run sync:startup-colors`」一句(推荐补)。
      - `.claude/adr.md` ADR-0002 :58 被否决备选整条改写为:「启动链 fail-safe 收编(运行期通道,被否决):index.html/tauri.conf.json 为静态层无法用 TS token。**2026-09 CP-027 修订**:构建期通道成立——`scripts/sync-startup-colors.mjs` 从 linear.ts 提取改写三处消费点,运行期仍不经 facade。」
      - `.claude/adr.md` ADR-0002 :63 后果条「main.tsx 静态 import 图收敛为 react/react-dom/lib/e2eEnabled」改写为「……收敛为 react/react-dom/lib/e2eEnabled/theme/startupColors(零依赖常量模块,不触发 facade 求值)」。
      - `.claude/adr.md` ADR-0003 :80「启动链 fail-safe 三处静态色手动同步 `#0a0a0b`」改为「启动链 fail-safe 静态色经 sync 脚本从 linear.ts 构建期注入(CP-027)」。

4. **测试同步**:
   - 新增 `src/__tests__/startup-colors-sync.test.ts`(L2;领域对象-能力行为命名):
     ```ts
     // startup-colors-sync.test.ts — 启动链 fail-safe 静态色单源一致守卫(L2,CP-027)
     //
     // 防复发对照:修复前 index.html / tauri.conf.json / main.tsx 三处硬编码靠人工与
     // linear.ts 同步(ADR-0002 被否决备选)——本文件锁「三处消费点 == linear.ts 提取值」
     // 与「main.tsx 无 fail-safe 字面量」,漂移即红。

     import { describe, it, expect } from "vitest";
     import { readFileSync } from "node:fs";
     import { join } from "node:path";
     import {
       extractStartupColors,
       renderStartupColorsModule,
     } from "../../scripts/sync-startup-colors.mjs";

     const root = join(__dirname, "../..");
     const linearTs = readFileSync(join(root, "src/theme/schemes/linear.ts"), "utf8");
     const colors = extractStartupColors(linearTs);

     describe("extractStartupColors(CP-027 色源提取)", () => {
       it("linear.ts 现值提取:bg/fg/errorFg 三槽位", () => {
         expect(colors).toEqual({
           bg: "#0a0a0b",
           fg: "#ece9e4",
           errorFg: "#d9706b",
         });
       });

       it("槽位缺失 → 抛错(禁自估色值)", () => {
         expect(() => extractStartupColors("export const linear = {};")).toThrow(
           /appBgPrimary/,
         );
       });
     });

     describe("三处消费点与 linear.ts 一致(防漂移回归)", () => {
       it("index.html body background == appBgPrimary", () => {
         const html = readFileSync(join(root, "index.html"), "utf8");
         expect(html).toContain(`background: ${colors.bg};`);
       });

       it("tauri.conf.json backgroundColor == appBgPrimary", () => {
         const conf = readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8");
         expect(conf).toContain(`"backgroundColor": "${colors.bg}"`);
       });

       it("startupColors.ts == 渲染器输出(生成物未手改)", () => {
         const mod = readFileSync(join(root, "src/theme/startupColors.ts"), "utf8");
         expect(mod).toBe(renderStartupColorsModule(colors));
       });

       it("main.tsx 无 fail-safe 硬编码字面量(读 startupColors 常量)", () => {
         const mainTs = readFileSync(join(root, "src/main.tsx"), "utf8");
         expect(mainTs).not.toContain('"#0a0a0b"');
         expect(mainTs).not.toContain('"#ece9e4"');
         expect(mainTs).not.toContain('"#d9706b"');
         expect(mainTs).toContain("STARTUP_FAIL_SAFE_BG");
       });
     });
     ```
   - 既有 `src/__tests__/theme-scheme-registry.test.ts:160-161`(`linear.ui.appBgPrimary` 断言)与 `theme-colors.test.ts` 不涉及本改动,原样通过即回归。

5. **文档同步**:
   - `src/theme/CLAUDE.md`(见修复步骤 5,登记点改写 + 红线增补)。
   - `src/theme/schemes/linear.ts` 文件头(删交叉引用块)。
   - `.claude/adr.md` ADR-0002 :58/:63、ADR-0003 :80(见修复步骤 5)。
   - `src/main.tsx` 注释口径(见修复步骤 4)。
   - 硬约束 #6 本身(根 `.claude/CLAUDE.md:49`)文本不破——「既定例外清单」机制保留,例外清单归零,无需改根文件。

6. **验证**:
   - 接线生效:`npm run sync:startup-colors` 退出码 0 且输出「无改动」(幂等,首跑后三文件无 diff)。
   - 提取链路:临时将 `linear.ts` 的 `appBgPrimary` 值改为 `#111111` → `npm run sync:startup-colors` → `grep -n "#111111" index.html src-tauri/tauri.conf.json src/theme/startupColors.ts` 三处命中 → `git checkout -- .` 还原(机械演练后必须还原)。
   - `grep -n "#0a0a0b" index.html src-tauri/tauri.conf.json src/main.tsx` 仅前两文件各 1 命中、main.tsx 0 命中(main.tsx 经常量读)。
   - `grep -rn "既定例外" src/main.tsx src/theme/schemes/linear.ts` 退出码 1(零命中)。
   - `grep -n "fail-safe" src/theme/CLAUDE.md` 命中为「无例外」新口径(:54 改写段),不再含「手动同步」。
   - `npx vitest run startup-colors-sync` 全绿;`npx tsc --noEmit` 绿。
   - `npx tauri build --debug --no-bundle` 成功(prebuild 触发,构建期注入链路端到端)。

---

## 起草附注

### 漂移点(转述 vs 现状,均按现状修订)

1. **CP-026 ticker 行号**:compromises 转述「60s ticker(useAgentStatus.ts:90-97)」——现状精确为 :90(`now` state)+ :92-97(effect),条目按全文照抄方式消除行号依赖。
2. **CP-025 误导注释范围**:除 :4 注释外,同文件 :63-66 mock 本体早已指向 `../features/navTree/NavTree`,仅注释滞后——改注释即闭合,无行为变化。
3. **CP-027 陈旧行号**:linear.ts 头注释与 theme/CLAUDE.md 记 main.tsx 超时页为「:28」——现状色值实为 :36-43(:28-30 为 SEC-10 注释块),闭合时交叉引用整块删除顺带消除。
4. **「既定例外」字面**:compromises 称「三处『既定例外』登记」——「既定例外」字面仅 `main.tsx:30` 一处;其余为交叉引用/手动同步登记(linear.ts 头、theme/CLAUDE.md:54/:62、ADR-0002/0003),条目逐一列全。
5. **CP-026 死面佐证补充**:除「测试 mock 形状」外,`knip.out:169` 标 `AgentStatusState` 未使用、`knip.json:22-24` 以 `types` 豁免压制——条目含 knip.json 销项步骤(转述未提)。
6. **agentStatus/CLAUDE.md 与本次决策冲突**:该文件载「useAgentStatus 留存不改(NAV-01 数据接入契约)」——与 CP-026 收窄决策相悖,以锁定决策为准,文档随迁改写(条目已含迁移全文)。
7. **e2e 注释零漂移**:`e2e-tests/` 7 处注释按 hook 名引用 useAgentStatus(history.e2e.ts:73 等)——hook 名不变,注释无需动,不列入步骤。

### S01 内五项分工文件重叠线索

| 项 | 主触文件 | 与他人的重叠 |
|----|---------|-------------|
| CP-025 | `src/features/sidebar/`(删除)、`agent-history-restore.test.ts`(:4 注释)、`systematic-changes-plan/config.json`(:52) | 无重叠——独立 agent 可并行 |
| CP-026+CP-021(同 agent) | `src/features/navTree/`(useAgentStatus.ts 迁入全文、NavTree.tsx、NavHistoryRow.tsx、useNavTree.ts、NavSessionRow.tsx、CLAUDE.md)、`src/features/agentStatus/`(删除)、`agentHistory/CLAUDE.md:89`、`knip.json`、4 个测试文件 | 与 CP-025 无重叠;**章四 review-04 的 CP-021 占位条目即本 agent 的契约锚**,勿另派 |
| CP-027+CP-038(同 agent) | CP-027:`scripts/sync-startup-colors.mjs`(新)、`src/theme/startupColors.ts`(新)、`index.html`、`src-tauri/tauri.conf.json`、`main.tsx`、`linear.ts` 头、`theme/CLAUDE.md`、`.claude/adr.md`、`package.json` scripts 段 | **CP-038 改 `package.json` devDependencies pin——同一文件不同字段,须同 agent 串行**;与 CP-032 同文件三方错峰 |
| CP-032 | `package.json` overrides 段 + wdio 家族 devDependencies | 与 CP-027/CP-038 共享 `package.json`——若不同 agent,须按「scripts 段 / devDependencies+overrides 段」字段级隔离并串行提交 |
| CP-001 | package.json 双 TS(TS6 包装器 + `@typescript/native` 别名) | 同属 package.json 收敛面——涉及 scripts/devDependencies 字段时与 CP-027/038/032 四方错峰,建议全部 package.json 改动由 CP-027+CP-038 agent 统一收口或明确字段级分工 |

**关键冲突预警**:CP-026 的 `nav-tree-history.test.tsx` 新增 ticker 用例依赖 `NavHistoryRow` 的 `now` prop 与 NavTree 宿主 ticker——若 CP-021/026 被拆给不同 agent,S01 内会出现「契约改了实现没改」的编译断裂窗口;按锁定决策同 agent 执行则无此风险。
