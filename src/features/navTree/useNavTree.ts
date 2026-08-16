// useNavTree.ts —— 导航树数据 hook（NAV-01/02/03/04 数据接入）
//
// 数据源（全部只读引用既有数据层，零新增订阅）：
//   - 项目/页面树：useProjects（照 SidebarTree 订阅形态）
//   - 活跃会话：useAgentStatus（rows——panelId/pageId/projectId/cliId/title/status/usage）
//   - 历史会话：useAgentHistory（sessions + activeStatuses + scan/removeLocal）
//
// 归属规则（决策 5）：
//   - 活跃会话挂页面下：row.pageId（useAgentStatus 内部已按 parseTerminalPageId 解析）
//   - 历史会话挂项目下：session.cwd 前缀匹配项目 rootPath（规范化 + 忽略大小写后
//     cwd === rootPath 或为其子路径；无归属项目（孤儿目录）→ 导航树不展示）
//
// 展开/折叠状态组件内维护（NAV-01）：项目/页面/历史节点默认收起
// （NAV-10 契约：expandTo 测试辅助按点击展开驱动——expanded = 项目/页面展开集合，
// expandedHist = 历史节点展开集合，两个 Set 默认空 = 全部收起）。
//
// 搜索（NAV-04）：query 子串不区分大小写过滤项目/页面/会话名；
// 父节点因子命中而显示（match 链）；查询非空时命中链自动展开（searching 覆盖手动展开态）。

import { useCallback, useEffect, useMemo, useState } from "react";
import { useProjects } from "../../stores/projects";
import type { OperationPage, Project } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { useAgentStatus } from "../agentStatus/useAgentStatus";
import type { AgentSessionRow } from "../agentStatus/useAgentStatus";
import { useAgentHistory } from "../agentHistory";
import type { AgentHistoryState } from "../agentHistory";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { AgentStatus } from "../../lib/agentStatus";
import { normalizePath } from "../../lib/path";

// ---- 模型（查询过滤后的导航树） ----

export interface NavSessionModel {
  row: AgentSessionRow;
  /** 页面级「活跃会话」标记（活跃页面上的最近事件行）——accent-dim 选中底（设计 6.3） */
  active: boolean;
}

export interface NavPageModel {
  page: OperationPage;
  /** 查询命中（自身名或子会话命中）；查询为空恒 true */
  match: boolean;
  /** 查询过滤后的会话行（rows 已按 lastEventAt 降序） */
  sessions: NavSessionModel[];
}

export interface NavHistoryModel {
  /** 查询命中（有会话行命中）；查询为空恒 true */
  match: boolean;
  /** 查询过滤后的历史行（mtimeMs 降序） */
  sessions: AgentHistorySession[];
  /** 项目全部历史会话数（计数 pill） */
  total: number;
}

export interface NavProjectModel {
  project: Project;
  /** 查询命中（自身名或任一子节点命中）；查询为空恒 true */
  match: boolean;
  /** 当前活跃项目（含活跃页面）——「当前」pill（NAV-09） */
  isCurrent: boolean;
  pages: NavPageModel[];
  history: NavHistoryModel;
}

// ---- 归属辅助 ----

/**
 * 历史会话归属项目判定（决策 5：历史折叠节点挂项目下，cwd 归属）：
 * 两侧规范化（反斜杠→/）+ 忽略大小写 + 去尾部斜杠后，cwd === rootPath 或
 * cwd 为 rootPath 子路径（前缀 + "/" 防同前缀目录误归属）。cwd 为 null/空 → false。
 */
export function isCwdUnderProject(cwd: string | null, rootPath: string): boolean {
  if (!cwd) return false;
  const a = normalizePath(cwd).toLowerCase().replace(/\/+$/, "");
  const b = normalizePath(rootPath).toLowerCase().replace(/\/+$/, "");
  if (a === b) return true;
  return a.startsWith(`${b}/`);
}

export interface UseNavTreeResult {
  /** 查询过滤后的导航树（项目 → 页面 → 会话 + 历史节点） */
  tree: NavProjectModel[];
  query: string;
  setQuery(q: string): void;
  /** 查询非空 = 搜索中（命中链自动展开） */
  searching: boolean;
  /** 项目/页面展开集合（默认收起；组件内维护，NAV-01） */
  expanded: Set<string>;
  toggleExpand(id: string): void;
  /** 历史节点展开集合（默认收起） */
  expandedHist: Set<string>;
  toggleHist(id: string): void;
  /** 活跃页面 id（页面行选中态判定） */
  activePageId: string | null;
  /** 历史扫描状态（useAgentHistory 透传） */
  historyState: AgentHistoryState;
  /** 刷新（重扫历史会话——「导航」头刷新钮） */
  refresh(): void;
  /** 删除历史会话后的即时局部刷新（不重扫） */
  removeLocal(sessionId: string): void;
  /** 运行中会话四态（历史行圆点；复合键 cliId|sessionId，MC-313） */
  activeStatuses: Map<string, AgentStatus>;
}

export function useNavTree(): UseNavTreeResult {
  const projects = useProjects((s) => s.projects);
  const activePageId = useLayout((s) => s.activePageId);
  const { rows } = useAgentStatus();
  const history = useAgentHistory();

  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const [expandedHist, setExpandedHist] = useState<Set<string>>(() => new Set());

  // 当前活跃项目（含活跃页面的项目——「当前」pill 判定，照 useAgentStatus 推导）
  const currentProjectId = useMemo(() => {
    if (!activePageId) return null;
    for (const proj of Object.values(projects)) {
      if (proj.pages.some((p) => p.pageId === activePageId)) return proj.projectId;
    }
    return null;
  }, [projects, activePageId]);

  // 活跃会话按页面归组（useAgentStatus rows 已按 lastEventAt 降序）
  const sessionsByPage = useMemo(() => {
    const map = new Map<string, AgentSessionRow[]>();
    for (const row of rows) {
      const list = map.get(row.pageId);
      if (list) list.push(row);
      else map.set(row.pageId, [row]);
    }
    return map;
  }, [rows]);

  // 历史会话按项目归组（cwd 前缀匹配 rootPath 归属；组内 mtimeMs 降序——照 HistorySessionList current 区）
  const historyByProject = useMemo(() => {
    const map = new Map<string, AgentHistorySession[]>();
    const projList = Object.values(projects);
    for (const session of history.sessions) {
      const proj = projList.find((p) => isCwdUnderProject(session.cwd, p.rootPath));
      if (!proj) continue; // 无归属项目（孤儿目录）→ 导航树不展示
      const list = map.get(proj.projectId);
      if (list) list.push(session);
      else map.set(proj.projectId, [session]);
    }
    for (const list of map.values()) list.sort((a, b) => b.mtimeMs - a.mtimeMs);
    return map;
  }, [history.sessions, projects]);

  const toggleExpand = useCallback((id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const toggleHist = useCallback((id: string) => {
    setExpandedHist((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const searching = query.trim().length > 0;

  // 挂载即扫描历史（NAV-10 契约：计数 pill 与历史行在首屏可见——历史折叠节点
  // 常驻项目下，计数需要数据；useAgentHistory generation 防竞兜底重复扫描）
  useEffect(() => {
    void history.scan();
  }, [history.scan]);

  // 树模型派生：搜索过滤（NAV-04）+ 归属归组（决策 5）
  const tree = useMemo(() => {
    const q = query.trim().toLowerCase();
    // 搜索命中：子串不区分大小写；查询为空恒命中
    const hit = (text: string) => (q ? text.toLowerCase().includes(q) : true);

    const result: NavProjectModel[] = [];
    for (const proj of Object.values(projects)) {
      const pages: NavPageModel[] = proj.pages.map((page) => {
        const pageRows = sessionsByPage.get(page.pageId) ?? [];
        // 页面级「活跃会话」= 活跃页面上的最近事件行（rows 已降序，首行即最近）
        const activeSession =
          page.pageId === activePageId ? pageRows[0] : undefined;
        const sessions: NavSessionModel[] = [];
        for (const row of pageRows) {
          if (!hit(row.title)) continue;
          sessions.push({ row, active: row === activeSession });
        }
        return {
          page,
          match: q ? hit(page.name) || sessions.length > 0 : true,
          sessions,
        };
      });

      const projHistory = historyByProject.get(proj.projectId) ?? [];
      const visibleHistory = q
        ? projHistory.filter((s) => hit(s.title ?? s.sessionId.slice(0, 8)))
        : projHistory;
      const historyModel: NavHistoryModel = {
        match: q ? visibleHistory.length > 0 : true,
        sessions: visibleHistory,
        total: projHistory.length,
      };

      result.push({
        project: proj,
        match: q
          ? hit(proj.name) || pages.some((p) => p.match) || historyModel.match
          : true,
        isCurrent: proj.projectId === currentProjectId,
        pages,
        history: historyModel,
      });
    }
    return result;
  }, [projects, sessionsByPage, historyByProject, activePageId, currentProjectId, query]);

  return {
    tree,
    query,
    setQuery,
    searching,
    expanded,
    toggleExpand,
    expandedHist,
    toggleHist,
    activePageId,
    historyState: history.state,
    refresh: history.scan,
    removeLocal: history.removeLocal,
    activeStatuses: history.activeStatuses,
  };
}
