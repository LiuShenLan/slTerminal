// useAgentStatus.ts — Agent 状态 hook
//
// 从 useLayout + useProjects 推导当前项目，扫描 TerminalRegistry 获取 panelId 列表，
// 订阅 onHookEvent 事件驱动更新行状态、过滤非当前项目会话、按 lastEventAt 倒序排列。
//
// SessionEnd / Exit → 移除行。
// 事件含 transcriptPath 时异步调用 contextUsage 更新用量条。

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

// ---- 类型定义 ----

/** 单行 Agent 会话数据 */
export interface AgentSessionRow {
  panelId: string;
  pageId: string;
  projectId: string;
  title: string;
  status: ClaudeStatus;
  lastEventAt: number;
  transcriptPath?: string;
  usage?: { inputTokens: number; outputTokens: number } | null;
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

  // FE-06：useMemo 稳定 projectPageIds，handleHookEvent deps 随之稳定，useEffect 不再每渲染重订阅
  const projectPageIds = useMemo(
    () => new Set(activeProject?.pages.map((pg) => pg.pageId) ?? []),
    [activeProject],
  );

  // 项目无 root——no-root 态
  const projectRoot = activeProject?.rootPath ?? null;

  // 事件处理回调（deps 现由 useMemo 稳定）
  const handleHookEvent = useCallback(
    (payload: HookEventPayload) => {
      if (!projectRoot) return;

      const pageId = parseTerminalPageId(payload.panelId);
      if (!pageId) return;

      // 过滤：仅保留属于当前项目的 panelId
      if (!projectPageIds.has(pageId)) return;

      // FE-05：eventToStatus 可能返回 null——null 时不覆盖已有行 status
      const newStatus = eventToStatus(
        payload.event,
        payload.notificationType,
      );

      setRows((prev) => {
        const existingIdx = prev.findIndex(
          (r) => r.panelId === payload.panelId,
        );
        const now = payload.timestamp || Date.now();

        // SessionEnd / Exit → 移除
        if (
          payload.event === "SessionEnd" ||
          payload.event === "Exit"
        ) {
          if (existingIdx === -1) return prev;
          const next = [...prev];
          next.splice(existingIdx, 1);
          return next;
        }

        // FE-04：标题查 dockviewApi，事件到达时刷新已有行标题
        const pageTitle = resolveTitle(payload.panelId, pageId);

        if (existingIdx >= 0) {
          // 更新已有行——null 状态不覆盖旧值
          const next = [...prev];
          next[existingIdx] = {
            ...next[existingIdx],
            title: pageTitle,
            ...(newStatus !== null ? { status: newStatus } : {}),
            lastEventAt: now,
            transcriptPath:
              payload.transcriptPath ?? next[existingIdx].transcriptPath,
          };
          next.sort((a, b) => b.lastEventAt - a.lastEventAt);
          return next;
        }

        // 新行：eventToStatus 为 null 时兜底 attention
        const row: AgentSessionRow = {
          panelId: payload.panelId,
          pageId,
          projectId: activeProject?.projectId ?? "",
          title: pageTitle,
          status: newStatus ?? "attention",
          lastEventAt: now,
          transcriptPath: payload.transcriptPath || undefined,
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
                r.panelId === payload.panelId
                  ? { ...r, usage }
                  : r,
              ),
            );
          })
          .catch(() => {
            // 解析失败 → usage 保持旧值（null 由 promise resolve 时置）
          });
      }
    },
    [projectRoot, projectPageIds, activeProject],
  );

  // 订阅 onHookEvent
  useEffect(() => {
    const unlisten = onHookEvent(handleHookEvent);
    return () => {
      unlisten();
    };
  }, [handleHookEvent]);

  // FE-03：TerminalRegistry 订阅——register 插入行，remove 移除行
  useEffect(() => {
    if (!projectRoot || !activeProject) return;

    const unsub = TerminalRegistry.subscribe((event) => {
      const pageId = parseTerminalPageId(event.panelId);
      if (!pageId) return;
      if (!projectPageIds.has(pageId)) return;

      if (event.type === "register") {
        // 插入 🟡 行（同初始扫描语义）
        setRows((prev) => {
          if (prev.some((r) => r.panelId === event.panelId)) return prev;
          const row: AgentSessionRow = {
            panelId: event.panelId,
            pageId,
            projectId: activeProject.projectId,
            title: resolveTitle(event.panelId, pageId),
            status: "attention",
            lastEventAt: Date.now(),
            usage: undefined,
          };
          return [...prev, row].sort((a, b) => b.lastEventAt - a.lastEventAt);
        });
      } else if (event.type === "remove") {
        setRows((prev) => {
          const idx = prev.findIndex((r) => r.panelId === event.panelId);
          if (idx === -1) return prev;
          const next = [...prev];
          next.splice(idx, 1);
          return next;
        });
      }
    });

    return unsub;
  }, [projectRoot, activeProject?.projectId, projectPageIds]);

  // 切换项目时清空旧行 + 从 TerminalRegistry 初始扫描
  useEffect(() => {
    if (!projectRoot || !activeProject) {
      setRows([]);
      return;
    }

    // 从 TerminalRegistry 获取当前项目下的所有 panelId
    const allTerminals = TerminalRegistry.getAll();
    const initialRows: AgentSessionRow[] = [];

    for (const [panelId] of allTerminals) {
      const pageId = parseTerminalPageId(panelId);
      if (!pageId) continue;
      if (!projectPageIds.has(pageId)) continue;

      initialRows.push({
        panelId,
        pageId,
        projectId: activeProject.projectId,
        title: resolveTitle(panelId, pageId),
        status: "attention", // 初始态：命令运行中
        lastEventAt: Date.now(),
        usage: undefined,
      });
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
