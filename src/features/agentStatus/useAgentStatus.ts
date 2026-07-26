// useAgentStatus.ts — Agent 状态 hook
//
// 从 useLayout + useProjects 推导当前项目，扫描 TerminalRegistry 获取 panelId 列表，
// 订阅 onHookEvent 事件驱动更新行状态、过滤非当前项目会话、按 lastEventAt 倒序排列。
//
// Stop → 状态置 done（保留在列表）；SessionEnd / exit → 移除。
// 事件含 transcriptPath 时异步调用 contextUsage 更新用量条。

import { useState, useEffect, useRef, useCallback } from "react";
import { useLayout } from "../../stores/layout";
import { useProjects } from "../../stores/projects";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import { onHookEvent, contextUsage } from "../../ipc/hooks";
import { eventToStatus } from "../../lib/claudeStatus";
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

/** 从 panelId 解析 pageId。格式：terminal-{pageId}-{seq} */
function parsePageId(panelId: string): string {
  // panelId 格式: terminal-<pageId>-<数字序号>
  // 如 "terminal-page1-0" → "page1"，"terminal-my-page-2" → "my-page"
  const withoutPrefix = panelId.startsWith("terminal-")
    ? panelId.slice("terminal-".length)
    : panelId;
  const lastDash = withoutPrefix.lastIndexOf("-");
  if (lastDash <= 0) return withoutPrefix;
  // 确认最后一段为纯数字序号才剥离
  const suffix = withoutPrefix.slice(lastDash + 1);
  if (/^\d+$/.test(suffix)) {
    return withoutPrefix.slice(0, lastDash);
  }
  return withoutPrefix;
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

  // 计算当前项目的 pageId 集合
  const projectPageIds = new Set(
    activeProject?.pages.map((pg) => pg.pageId) ?? [],
  );

  // 项目无 root——no-root 态
  const projectRoot = activeProject?.rootPath ?? null;

  // 事件处理回调（稳定引用）
  const handleHookEvent = useCallback(
    (payload: HookEventPayload) => {
      if (!projectRoot) return;

      const pageId = parsePageId(payload.panelId);

      // 过滤：仅保留属于当前项目的 panelId
      if (!projectPageIds.has(pageId)) return;

      const newStatus = eventToStatus(
        payload.event,
        payload.notificationType,
      );

      setRows((prev) => {
        const existingIdx = prev.findIndex(
          (r) => r.panelId === payload.panelId,
        );
        const now = payload.timestamp || Date.now();

        // SessionEnd → 移除
        if (
          payload.event === "SessionEnd" ||
          payload.event === "Exit"
        ) {
          if (existingIdx === -1) return prev;
          const next = [...prev];
          next.splice(existingIdx, 1);
          return next;
        }

        // Stop → 状态置 done，保留
        const resolvedStatus =
          payload.event === "Stop" ? "done" : newStatus;

        // 简单标题回退：若 titleManager 无注册则用 pageId
        const pageTitle = `终端 ${pageId}`;

        const row: AgentSessionRow = {
          panelId: payload.panelId,
          pageId,
          projectId: activeProject?.projectId ?? "",
          title: pageTitle,
          status: resolvedStatus,
          lastEventAt: now,
          transcriptPath: payload.transcriptPath || undefined,
          usage: existingIdx >= 0 ? prev[existingIdx].usage : undefined,
        };

        if (existingIdx >= 0) {
          // 更新已有行
          const next = [...prev];
          next[existingIdx] = {
            ...next[existingIdx],
            status: row.status,
            lastEventAt: row.lastEventAt,
            transcriptPath:
              row.transcriptPath ?? next[existingIdx].transcriptPath,
          };
          // 按 lastEventAt 倒序排列
          next.sort((a, b) => b.lastEventAt - a.lastEventAt);
          return next;
        }

        // 新行
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
            // 解析失败 → usage = null（已默认）
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
      const pageId = parsePageId(panelId);
      if (!projectPageIds.has(pageId)) continue;

      initialRows.push({
        panelId,
        pageId,
        projectId: activeProject.projectId,
        title: `终端 ${pageId}`,
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
