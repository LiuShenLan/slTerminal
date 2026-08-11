// useAgentStatus.ts — Agent 状态 hook
//
// 行 = 运行中的编码 CLI 会话（非全部终端）。
// 建行双通道：sessionChange（session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在）——两通道独立幂等。
// 删行三通道：sessionChange（session 为 null）∨ SessionEnd/Exit hook 事件 ∨ remove 事件。
// 初始扫描只建 agentSession 非 null 的行；携 transcriptPath 时主动拉 contextUsage（修复问题 2b）。
// #5 竞态双保险：① registry/agent-event 双 listener 经 ref 读最新状态，effect deps [] 订阅永不重建；
// ② 初始扫描按注册表现值对账（agentSession 非 null 才建行），兜底任何事件丢失。
//
// 行 cliId（MC-410）：hook 事件通道建行按 MC-205 三级解析
// （payload.cliId → TerminalRegistry.get(panelId)?.agentSession?.cliId → CLAUDE_CLI_ID）写入；
// OSC 133 通道建行经 setAgentSession 的 sessionChange 自然驱动（cliId 取 agentSession.cliId，缺省兜底）。

import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { useLayout } from "../../stores/layout";
import { useProjects } from "../../stores/projects";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
// ZQ-2: 来源 CLI 标识三级解析单点（契约 4）——空串/空白 cliId 同等回退
import { resolvePayloadCliId } from "../../panels/terminal/resolvePayloadCliId";
import { onAgentEvent, contextUsage } from "../../ipc/agentHooks";
import { parseTerminalPageId } from "../../lib/panelId";
import { getPageApi } from "../../workspace/pageApis";
import { cliProfileRegistry } from "../cliProfiles";
// AC-5: 事件名字面量只允许出现在 profiles/claude/（claude 合法领地）——
// SessionEnd/Exit 判定一律引用本常量，不写字面量
import { CLAUDE_CLI_ID, SESSION_END_EVENT, EXIT_EVENT } from "../cliProfiles/profiles/claude";
import type { AgentStatus } from "../../lib/agentStatus";
import type { AgentEventPayload } from "../../types/agent";
import type { ContextUsage } from "../../types/agent";

// ---- 类型定义 ----

/** 单行 Agent 会话数据 */
export interface AgentSessionRow {
  panelId: string;
  pageId: string;
  projectId: string;
  /** 会话所属 CLI 标识（hook 事件通道 = 三级解析结果；OSC 133 通道 = agentSession.cliId）——供行 logo 查 profile */
  cliId: string;
  /** 会话 UUID（hook 事件 payload.sessionId；matchedCommand-only 会话缺省）——供视图层标题覆盖匹配 */
  sessionId?: string;
  title: string;
  status: AgentStatus;
  lastEventAt: number;
  transcriptPath?: string;
  usage?: ContextUsage | null;
}

/** 视图状态机 */
export type AgentStatusState =
  | { kind: "no-root" }
  | { kind: "empty" }
  | { kind: "ready" };

/** hook 返回值 */
export interface AgentStatusResult {
  state: AgentStatusState;
  rows: AgentSessionRow[];
  currentProjectName: string | null;
  /** 相对时间基准（60s ticker 驱动重算——idle 会话无 hook 事件时时间文本冻结，问题 1b 修复） */
  now: number;
}

// ---- 辅助函数 ----

/** 根据 panelId 查找页签标题，无 dockviewApi 或面板时回退 */
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

export function useAgentStatus(): AgentStatusResult {
  const activePageId = useLayout((s) => s.activePageId);
  const projects = useProjects((s) => s.projects);
  const [rows, setRows] = useState<AgentSessionRow[]>([]);
  const [now, setNow] = useState(() => Date.now());

  // 相对时间定时刷新（问题 1b 修复）：formatRelativeTime 渲染时计算，
  // 无 hook 事件时组件不重渲染 → 时间文本永久冻结；60s ticker 强制重算
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 60_000);
    return () => clearInterval(timer);
  }, []);

  // 跟踪事件回调引用（避免 onAgentEvent 重建订阅）
  const rowsRef = useRef(rows);
  rowsRef.current = rows;

  // 当前活跃项目
  const projectList = Object.values(projects);
  const activeProject = projectList.find((p) =>
    p.pages.some((pg) => pg.pageId === activePageId),
  );
  const currentProjectName = activeProject?.name ?? null;

  const projectPageIds = useMemo(
    () => new Set(activeProject?.pages.map((pg) => pg.pageId) ?? []),
    [activeProject],
  );

  const projectRoot = activeProject?.rootPath ?? null;

  // ref 副本供稳定订阅（dept []）回调读取最新值，防 R4 竞态（remove 事件丢失）
  const projectRootRef = useRef(projectRoot);
  projectRootRef.current = projectRoot;
  const projectPageIdsRef = useRef(projectPageIds);
  projectPageIdsRef.current = projectPageIds;
  const activeProjectRef = useRef(activeProject);
  activeProjectRef.current = activeProject;

  // ---- hook 事件处理（deps []——所有数据经 ref 读取，回调永不重建） ----
  const handleHookEvent = useCallback(
    (payload: AgentEventPayload) => {
      const projRoot = projectRootRef.current;
      if (!projRoot) return;

      const pageId = parseTerminalPageId(payload.panelId);
      if (!pageId) return;

      const pageIds = projectPageIdsRef.current;
      if (!pageIds.has(pageId)) return;

      const proj = activeProjectRef.current;
      if (!proj) return;

      // MC-205 三级解析单点（ZQ-2，契约 4）：payload.cliId（trim 后非空）→ 注册表
      // agentSession.cliId（反查）→ CLAUDE_CLI_ID（缺省兼容旧信号）；
      // 空串/仅空白与 null/undefined 同等回退（原 ?? 链遇空串短路失效）
      const cliId = resolvePayloadCliId(payload);
      const profile = cliProfileRegistry.get(cliId);
      // MC-206：未知 cliId（未注册）或无 hooks 能力 → console.warn + 跳过（不建行/不置图标/不通知），不抛异常
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

      // SessionEnd / Exit → 删行
      if (payload.event === SESSION_END_EVENT || payload.event === EXIT_EVENT) {
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
            transcriptPath:
              payload.transcriptPath ?? next[existingIdx].transcriptPath,
            sessionId: payload.sessionId ?? next[existingIdx].sessionId,
          };
          next.sort((a, b) => b.lastEventAt - a.lastEventAt);
          return next;
        }

        // 建新行：hook 事件通道（非 SessionEnd/Exit 且行不存在——与 sessionChange 通道独立幂等）
        // ZQ-3 决策 2：null 映射事件建行但 status null（无图标）——感知存活
        // （SessionStart 丢失场景：事件到达即会话存在，行必须出现）且不误标
        // attention（null 状态表示「无状态」，与 deriveActiveSessionStatuses
        // 「status 为 null 不产出键」语义一致）
        const row: AgentSessionRow = {
          panelId: payload.panelId,
          pageId,
          projectId: proj.projectId,
          cliId,
          title: pageTitle,
          status: newStatus,
          lastEventAt: payload.timestamp || Date.now(),
          transcriptPath: payload.transcriptPath || undefined,
          sessionId: payload.sessionId || undefined,
          usage: undefined,
        };

        const next = [row, ...prev];
        next.sort((a, b) => b.lastEventAt - a.lastEventAt);
        return next;
      });

      // 事件含 transcriptPath 时异步拉取用量（cliId 传行 cliId——事件通道建行的行 cliId = 三级解析结果）
      if (payload.transcriptPath) {
        contextUsage(cliId, payload.transcriptPath)
          .then((usage) => {
            setRows((prev) =>
              prev.map((r) =>
                r.panelId === payload.panelId ? { ...r, usage } : r,
              ),
            );
          })
          .catch((err) => {
            console.error("contextUsage 拉取失败:", err);
          });
      }
    },
    [], // deps []——所有动态数据经 ref 读取，回调永不重建
  );

  // 订阅 onAgentEvent（deps [handleHookEvent]，handleHookEvent deps [] 故永不重建）
  useEffect(() => {
    const unlisten = onAgentEvent(handleHookEvent);
    return () => {
      unlisten();
    };
  }, [handleHookEvent]);

  // ---- TerminalRegistry 订阅：sessionChange 建/删行 + remove 删行 ----
  // deps []——订阅永不重建，remove 事件永不丢失（根除 R4 根因：同 commit passive destroy
  // 顺序 SideBarArea 先于主区，旧 deps 重订阅窗口内 remove 丢失）
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
          // OSC 133 通道建行的行 cliId：agentSession.cliId（MC-107 命中时写入），缺省兜底防旧数据/mock
          const rowCliId = entry.agentSession.cliId ?? CLAUDE_CLI_ID;

          // session 非 null → 建行（幂等：行已存在则跳过）
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
              transcriptPath: entry.agentSession!.transcriptPath,
              sessionId: entry.agentSession!.sessionId,
              usage: undefined,
            };
            return [...prev, row].sort((a, b) => b.lastEventAt - a.lastEventAt);
          });

          // 携 transcriptPath 时主动拉取用量（cliId 传行 cliId）
          if (entry.agentSession.transcriptPath) {
            contextUsage(rowCliId, entry.agentSession.transcriptPath)
              .then((usage) => {
                setRows((prev) =>
                  prev.map((r) =>
                    r.panelId === event.panelId ? { ...r, usage } : r,
                  ),
                );
              })
              .catch((err) => {
                console.error("contextUsage 拉取失败:", err);
              });
          }
        } else {
          // session 为 null → 删行
          setRows((prev) => {
            const idx = prev.findIndex((r) => r.panelId === event.panelId);
            if (idx === -1) return prev;
            const next = [...prev];
            next.splice(idx, 1);
            return next;
          });
        }
      } else if (event.type === "remove") {
        // remove → 删行
        setRows((prev) => {
          const idx = prev.findIndex((r) => r.panelId === event.panelId);
          if (idx === -1) return prev;
          const next = [...prev];
          next.splice(idx, 1);
          return next;
        });
      }
      // register 事件不建行——建行由 sessionChange（非 null）和 hook 事件双通道负责
    });

    return unsub;
  }, []); // deps []——订阅永不重建

  // ---- 初始扫描 + 项目切换（只建 agentSession 非 null 的行；携 transcriptPath 主动拉 usage） ----
  useEffect(() => {
    if (!projectRoot || !activeProject) {
      setRows([]);
      return;
    }

    // 遍历 TerminalRegistry，只建 agentSession 非 null 的行
    const allTerminals = TerminalRegistry.getAll();
    const initialRows: AgentSessionRow[] = [];

    for (const [panelId, entry] of allTerminals) {
      if (!entry.agentSession) continue; // 纯 shell 终端不建行

      const pageId = parseTerminalPageId(panelId);
      if (!pageId) continue;
      if (!projectPageIds.has(pageId)) continue;

      // 初始扫描建行的行 cliId：agentSession.cliId，缺省兜底防旧数据/mock
      const rowCliId = entry.agentSession.cliId ?? CLAUDE_CLI_ID;

      initialRows.push({
        panelId,
        pageId,
        projectId: activeProject.projectId,
        cliId: rowCliId,
        title: resolveTitle(panelId, pageId),
        status: "attention",
        lastEventAt: entry.agentSession.lastEventAt,
        transcriptPath: entry.agentSession.transcriptPath,
        sessionId: entry.agentSession.sessionId,
        usage: undefined,
      });

      // 携 transcriptPath 时主动拉取一次（修复问题 2b：切项目后 idle 会话用量永远 --；cliId 传行 cliId）
      if (entry.agentSession.transcriptPath) {
        contextUsage(rowCliId, entry.agentSession.transcriptPath)
          .then((usage) => {
            setRows((prev) =>
              prev.map((r) =>
                r.panelId === panelId ? { ...r, usage } : r,
              ),
            );
          })
          .catch((err) => {
            console.error("contextUsage 拉取失败:", err);
          });
      }
    }

    initialRows.sort((a, b) => b.lastEventAt - a.lastEventAt);
    setRows(initialRows);
  }, [projectRoot, activeProject?.projectId]);

  // 派生视图状态
  let state: AgentStatusState;
  if (!projectRoot) {
    state = { kind: "no-root" };
  } else if (rows.length === 0) {
    state = { kind: "empty" };
  } else {
    state = { kind: "ready" };
  }

  return { state, rows, currentProjectName, now };
}
