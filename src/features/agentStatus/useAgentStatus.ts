// useAgentStatus.ts — Agent 状态 hook
//
// 行 = 运行中的 claude 会话（非全部终端）。
// 建行双通道：sessionChange（session 非 null）∨ hook 事件（非 SessionEnd/Exit 且行不存在）——两通道独立幂等。
// 删行三通道：sessionChange（session 为 null）∨ SessionEnd/Exit hook 事件 ∨ remove 事件。
// 初始扫描只建 claudeSession 非 null 的行；携 transcriptPath 时主动拉 contextUsage（修复问题 2b）。
// #5 竞态双保险：① registry/hook-event 双 listener 经 ref 读最新状态，effect deps [] 订阅永不重建；
// ② 初始扫描按注册表现值对账（claudeSession 非 null 才建行），兜底任何事件丢失。

import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { useLayout } from "../../stores/layout";
import { useProjects } from "../../stores/projects";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import { onHookEvent, contextUsage } from "../../ipc/hooks";
import { eventToStatus } from "../../lib/claudeStatus";
import { parseTerminalPageId } from "../../lib/panelId";
import { getPageApi } from "../../workspace/pageApis";
import type { ClaudeStatus } from "../../lib/claudeStatus";
import type { HookEventPayload } from "../../ipc/hooks";
import type { ContextUsage } from "../../types/hooks";

// ---- 类型定义 ----

/** 单行 Agent 会话数据 */
export interface AgentSessionRow {
  panelId: string;
  pageId: string;
  projectId: string;
  /** 会话 UUID（hook 事件 payload.sessionId；matchedCommand-only 会话缺省）——供视图层标题覆盖匹配 */
  sessionId?: string;
  title: string;
  status: ClaudeStatus;
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

  // 跟踪事件回调引用（避免 onHookEvent 重建订阅）
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
    (payload: HookEventPayload) => {
      const projRoot = projectRootRef.current;
      if (!projRoot) return;

      const pageId = parseTerminalPageId(payload.panelId);
      if (!pageId) return;

      const pageIds = projectPageIdsRef.current;
      if (!pageIds.has(pageId)) return;

      const proj = activeProjectRef.current;
      if (!proj) return;

      const newStatus = eventToStatus(payload.event, payload.notificationType);

      // SessionEnd / Exit → 删行
      if (payload.event === "SessionEnd" || payload.event === "Exit") {
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
        const row: AgentSessionRow = {
          panelId: payload.panelId,
          pageId,
          projectId: proj.projectId,
          title: pageTitle,
          status: newStatus ?? "attention",
          lastEventAt: payload.timestamp || Date.now(),
          transcriptPath: payload.transcriptPath || undefined,
          sessionId: payload.sessionId || undefined,
          usage: undefined,
        };

        const next = [row, ...prev];
        next.sort((a, b) => b.lastEventAt - a.lastEventAt);
        return next;
      });

      // 事件含 transcriptPath 时异步拉取用量
      if (payload.transcriptPath) {
        contextUsage(payload.transcriptPath)
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

  // 订阅 onHookEvent（deps [handleHookEvent]，handleHookEvent deps [] 故永不重建）
  useEffect(() => {
    const unlisten = onHookEvent(handleHookEvent);
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

        if (entry.claudeSession && entry.claudeSession !== null) {
          // session 非 null → 建行（幂等：行已存在则跳过）
          setRows((prev) => {
            if (prev.some((r) => r.panelId === event.panelId)) return prev;
            const row: AgentSessionRow = {
              panelId: event.panelId,
              pageId,
              projectId: proj.projectId,
              title: resolveTitle(event.panelId, pageId),
              status: "attention",
              lastEventAt: entry.claudeSession!.lastEventAt,
              transcriptPath: entry.claudeSession!.transcriptPath,
              sessionId: entry.claudeSession!.sessionId,
              usage: undefined,
            };
            return [...prev, row].sort((a, b) => b.lastEventAt - a.lastEventAt);
          });

          // 携 transcriptPath 时主动拉取用量
          if (entry.claudeSession.transcriptPath) {
            contextUsage(entry.claudeSession.transcriptPath)
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

  // ---- 初始扫描 + 项目切换（只建 claudeSession 非 null 的行；携 transcriptPath 主动拉 usage） ----
  useEffect(() => {
    if (!projectRoot || !activeProject) {
      setRows([]);
      return;
    }

    // 遍历 TerminalRegistry，只建 claudeSession 非 null 的行
    const allTerminals = TerminalRegistry.getAll();
    const initialRows: AgentSessionRow[] = [];

    for (const [panelId, entry] of allTerminals) {
      if (!entry.claudeSession) continue; // 纯 shell 终端不建行

      const pageId = parseTerminalPageId(panelId);
      if (!pageId) continue;
      if (!projectPageIds.has(pageId)) continue;

      initialRows.push({
        panelId,
        pageId,
        projectId: activeProject.projectId,
        title: resolveTitle(panelId, pageId),
        status: "attention",
        lastEventAt: entry.claudeSession.lastEventAt,
        transcriptPath: entry.claudeSession.transcriptPath,
        sessionId: entry.claudeSession.sessionId,
        usage: undefined,
      });

      // 携 transcriptPath 时主动拉取一次（修复问题 2b：切项目后 idle 会话用量永远 --）
      if (entry.claudeSession.transcriptPath) {
        contextUsage(entry.claudeSession.transcriptPath)
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

  return { state, rows, currentProjectName };
}
