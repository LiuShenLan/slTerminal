// HistorySessionList.tsx — 历史会话列表（FE-07 集成部分）
//
// 两种展示模式（props.mode）：
//   current —— 当前项目区：平铺 HistorySessionRow（isCurrentProject 按 rootPath 过滤，mtime 降序）
//   all     —— 全部项目区：groupByCwd 二级折叠（组标题 = basename + (N) 计数 + 悬停完整路径，
//              组默认收起——问题 3 修复：expandedGroups 白名单模型；组内容再缩进 + 二级引导线）
// 搜索过滤经 matchesSearch（Stage 04）作用于两区（README 4.3.4）。
//
// 交互分派：
//   双击（行 onDoubleClick 消费方）三分支（README 4.3.2/4.3.3，问题 5 修复）：
//     普通行 → restoreHistorySession(session)；孤儿/无 cwd 行 → 无操作；
//     运行中行（status 非 null）→ SessionActionDialog 弹窗（「切换到该会话操作页面」/取消，
//     分支恢复仅保留在右键菜单）——切换 = 反查 TerminalRegistry 定位 panelId → 切页 + 聚焦
//   右键菜单 → getHistoryContextMenuItems 策略查询（复制/分支恢复/删除）；删除完成回调
//   （removeLocal 来自 useAgentHistory）经 props 注入。
// 状态标记：行 status 四态来自 useAgentHistory.activeStatuses（与活跃区同源，问题 2 修复；
//   复合键 cliId|sessionId 查询——MC-313）。
//
// 配色全部 theme/colors.ts token（硬约束 #6），零硬编码色值。

import React, { useCallback, useEffect, useRef, useState } from "react";
import { writeText } from "../../ipc/clipboard";
import { deleteHistorySession } from "../../ipc/agentHistory";
import { sendToastNotification } from "../../ipc/notification";
import { restoreHistorySession } from "./restoreSession";
import { HistorySessionRow } from "./HistorySessionRow";
import { SessionActionDialog } from "./SessionActionDialog";
import {
  buildResumeCommand,
  getHistoryContextMenuItems,
} from "./historyContextMenu";
import type { HistoryMenuItem } from "./historyContextMenu";
import { confirmDialog } from "../../lib";
import { basename } from "../../lib/path";
import { groupByCwd, isCurrentProject, keyOf, matchesSearch } from "./historyModel";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import { parseTerminalPageId } from "../../lib/panelId";
import { switchToPageAndFocus } from "../../workspace/pageApis";
import { useProjects } from "../../stores/projects";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { AgentStatus } from "../../lib/agentStatus";
import {
  EXPLORER_COLORS,
  INPUT_BORDER,
  PLACEHOLDER_FG,
  SIDEBAR_BG,
  SIDEBAR_FG,
  ERROR_FG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
} from "../../theme";
import { IconChevronRight, IconChevronDown } from "../../lib/icons";

/** 折叠箭头样式（chevron 12px，色经 EXPLORER_COLORS arrow 槽位 token——IC-05） */
const arrowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  width: 16,
  color: EXPLORER_COLORS.arrowClosed,
  userSelect: "none",
  flexShrink: 0,
};

/** 组标题栏样式（全部项目区二级折叠头，12px 粗体——问题 4 三级字号层级） */
const groupHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "2px 8px",
  height: 22,
  cursor: "pointer",
  userSelect: "none",
  fontSize: 12,
  fontWeight: "bold",
  color: EXPLORER_COLORS.fg,
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
};

/** 组内会话数（灰色小字） */
const groupCountStyle: React.CSSProperties = {
  color: INPUT_BORDER,
  fontSize: 11,
  fontWeight: "normal",
  marginLeft: 4,
};

/** 空态/提示文案样式 */
const emptyHintStyle: React.CSSProperties = {
  padding: "4px 8px",
  color: INPUT_BORDER,
  fontSize: 11,
  fontStyle: "italic",
  userSelect: "none",
};

// ── 右键菜单 ──

interface MenuState {
  visible: boolean;
  x: number;
  y: number;
  items: HistoryMenuItem[];
}

/** 菜单项行样式（项 28px 高、圆角 5——UI-802，照 NavContextMenu 规范） */
const menuItemStyle: React.CSSProperties = {
  height: 28,
  margin: "0 4px",
  borderRadius: 5,
  display: "flex",
  alignItems: "center",
  padding: "0 12px",
  fontSize: 12,
  userSelect: "none",
};

/** 右键菜单浮层（纯渲染，照 CommitFileList.tsx 私有 ContextMenu 模式） */
const ContextMenu: React.FC<{
  state: MenuState;
  onClose: () => void;
}> = ({ state, onClose }) => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!state.visible) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [state.visible, onClose]);

  if (!state.visible) return null;

  return (
    <div
      ref={menuRef}
      data-e2e="agent-history-menu"
      style={{
        position: "fixed",
        left: state.x,
        top: state.y,
        background: SIDEBAR_BG,
        border: `1px solid ${CONTEXT_MENU_BORDER}`,
        borderRadius: 5,
        padding: "4px 0",
        minWidth: 160,
        zIndex: 1000,
        boxShadow: SIDEBAR_COLORS.contextMenuShadow,
      }}
    >
      {state.items.map((item, i) =>
        item.disabled ? (
          // 禁用项：灰显（PLACEHOLDER_FG）、无点击回调
          <div
            key={i}
            style={{ ...menuItemStyle, color: PLACEHOLDER_FG, cursor: "default" }}
          >
            {item.label}
          </div>
        ) : (
          <div
            key={i}
            onClick={() => {
              item.action();
              onClose();
            }}
            style={{
              ...menuItemStyle,
              cursor: "pointer",
              // 危险项（删除类）ERROR_FG 着色（UI-802）
              color: item.danger ? ERROR_FG : SIDEBAR_FG,
            }}
            onMouseEnter={(e) => {
              (e.target as HTMLDivElement).style.background =
                SIDEBAR_COLORS.hover;
            }}
            onMouseLeave={(e) => {
              (e.target as HTMLDivElement).style.background = "transparent";
            }}
          >
            {item.label}
          </div>
        ),
      )}
    </div>
  );
};

// ── 反查 TerminalRegistry：sessionId → panelId（双击弹窗「切换到该会话操作页面」用） ──

/**
 * 反查运行中会话所在终端面板：复合键 `cliId|sessionId` 精确匹配（MC-313——
 * 与 deriveActiveSessionStatuses 同键形态，防跨 CLI sessionId 理论冲突），
 * 两侧键构造均经 keyOf 单点（cliId 缺省回退 CLAUDE_CLI_ID + 转义，ZQ-1）；
 * 未命中 → undefined。
 */
function findPanelForSession(cliId: string, sessionId: string): string | undefined {
  const key = keyOf(cliId, sessionId);
  for (const [panelId, entry] of TerminalRegistry.getAll()) {
    const cs = entry.agentSession;
    if (!cs) continue;
    let id = cs.sessionId;
    if (!id && cs.usageSourcePath) {
      const base = basename(cs.usageSourcePath);
      id = base.endsWith(".jsonl") ? base.slice(0, -".jsonl".length) : base;
    }
    if (!id) continue;
    if (keyOf(cs.cliId, id) === key) return panelId;
  }
  return undefined;
}

/**
 * panelId → 属主 pageId（B14）：先按已知页面集合做前缀匹配——旧恢复格式
 * （terminal-{pageId}-{Date.now}-{seq}）的 pageId 含数字段，语法切分会把
 * Date.now 段误并入 pageId 得到幽灵页面（导航后主区空白根因）；前缀匹配
 * 对旧格式可靠。兜底 parseTerminalPageId（新格式）；均未命中 → null。
 */
function findPageIdForPanelId(panelId: string): string | null {
  const { projects } = useProjects.getState();
  for (const project of Object.values(projects)) {
    for (const page of project.pages) {
      if (panelId.startsWith(`terminal-${page.pageId}-`)) {
        return page.pageId;
      }
    }
  }
  return parseTerminalPageId(panelId);
}

// ── 历史会话列表 ──

export interface HistorySessionListProps {
  /** current = 当前项目区平铺；all = 全部项目区二级折叠分组 */
  mode: "current" | "all";
  /** 全部历史会话（未过滤未分组，本组件按 mode 派生） */
  sessions: AgentHistorySession[];
  /** 当前项目 rootPath（null 时 current 区显示「无活跃项目」，本组件不渲染） */
  rootPath: string | null;
  /** 搜索词（matchesSearch 过滤，作用于两区） */
  search: string;
  /** 运行中会话四态映射（Map<cliId|sessionId, status>，与活跃区同源——问题 2 修复，复合键 MC-313） */
  activeStatuses: Map<string, AgentStatus>;
  /** 选中会话 id（受控，调用方持有） */
  selectedId: string | null;
  /** 单击选中回调 */
  onSelect(id: string): void;
  /** 删除成功后的即时局部刷新（useAgentHistory.removeLocal，不重扫） */
  removeLocal(id: string): void;
}

export const HistorySessionList: React.FC<HistorySessionListProps> = ({
  mode,
  sessions,
  rootPath,
  search,
  activeStatuses,
  selectedId,
  onSelect,
  removeLocal,
}) => {
  const [menu, setMenu] = useState<MenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  /** 双击运行中会话的动作弹窗目标（null = 关闭） */
  const [dialogSession, setDialogSession] = useState<AgentHistorySession | null>(
    null,
  );

  const closeMenu = useCallback(() => {
    setMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  /** 全部项目区展开的组集合（白名单模型——初始空 = 默认收起，问题 3 修复） */
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(
    () => new Set(),
  );

  const toggleGroup = (key: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  /** 行状态标记派生（四态 status / 孤儿标记 / 无 cwd——Row 的 status/orphan/noCwd 三 props） */
  const rowFlags = useCallback(
    (session: AgentHistorySession) => ({
      status: activeStatuses.get(keyOf(session.cliId, session.sessionId)),
      orphan: session.cwd !== null && !session.cwdExists,
      noCwd: session.cwd === null,
    }),
    [activeStatuses],
  );

  /** 切换到该会话所在操作页面并聚焦终端页签（问题 5 修复） */
  const handleSwitchToSession = useCallback(
    async (session: AgentHistorySession) => {
      const panelId = findPanelForSession(session.cliId, session.sessionId);
      if (!panelId) {
        sendToastNotification("未找到运行中的会话", {
          body: "该会话已结束或无法定位其终端页签",
        });
        return;
      }
      const pageId = findPageIdForPanelId(panelId);
      if (!pageId) {
        // B14: 解析不出属主页面时明确提示而非静默返回——旧格式防误导航兜底
        sendToastNotification("未找到运行中的会话", {
          body: "该会话已结束或无法定位其终端页签",
        });
        return;
      }
      // switchToPageAndFocus 内部：activePageId 相同则直接聚焦，不同则先切页（setProjectRoot 前置）
      await switchToPageAndFocus(pageId, panelId);
    },
    [],
  );

  /** 双击分派三分支（README 4.3.2/4.3.3，问题 5 修复）：
   *  普通行 → 恢复；孤儿/无 cwd → 无操作；运行中（status 非 null）→ 动作弹窗
   * （分支恢复仅保留在右键菜单——双击弹窗不再提供） */
  const handleDoubleClick = useCallback(
    (session: AgentHistorySession) => {
      const { status, orphan, noCwd } = rowFlags(session);
      if (status != null) {
        // 运行中：动作弹窗（切换到该会话操作页面 / 取消）
        setDialogSession(session);
      } else if (orphan || noCwd) {
        // 孤儿（起始目录已删除）/ 无 cwd 行：恢复失败概率高，禁用优于报错（README 4.2）——无操作
      } else {
        void restoreHistorySession(session);
      }
    },
    [rowFlags],
  );

  /** 右键菜单——委托策略注册表；操作回调在本层实现（剪贴板/fork/删除） */
  const handleContextMenu = useCallback(
    (session: AgentHistorySession, pos: { x: number; y: number }) => {
      const { status, orphan, noCwd } = rowFlags(session);
      const title = session.title ?? session.sessionId.slice(0, 8);
      const items = getHistoryContextMenuItems(session, {
        active: status != null, // 运行中（删除禁用判定）
        orphan,
        noCwd,
        // 复制恢复命令：命令构造见 buildResumeCommand（README 4.4，带单引号路径）
        onCopy: () => {
          void writeText(buildResumeCommand(session));
        },
        // 分支恢复：fork 编排（--fork-session 复制历史到新 sessionId，原会话不动）
        onFork: () => {
          void restoreHistorySession(session, { fork: true });
        },
        // 删除：confirmDialog 确认 → 删除 IPC → 成功后 removeLocal 即时局部刷新（不重扫）
        onDelete: () => {
          void confirmDialog({
            title: "确认删除",
            message: `确定删除会话"${title}"？此操作不可撤销。`,
            danger: true,
          }).then(async (ok) => {
            if (!ok) return;
            try {
              await deleteHistorySession(session.cliId, session.sessionId);
              removeLocal(session.sessionId);
            } catch (err) {
              console.error(
                "[slTerminal] 删除历史会话失败:",
                session.sessionId,
                err,
              );
            }
          });
        },
      });
      setMenu({ visible: true, x: pos.x, y: pos.y, items });
    },
    [rowFlags, removeLocal],
  );

  // 搜索过滤（matchesSearch 作用于两区；空词恒匹配）
  const filtered = sessions.filter((s) => matchesSearch(s, search));

  // 当前项目区：isCurrentProject 过滤 + mtimeMs 降序（README 4.2 排序）
  const currentSessions =
    mode === "current"
      ? filtered
          .filter((s) => isCurrentProject(s.cwd, rootPath))
          .sort((a, b) => b.mtimeMs - a.mtimeMs)
      : [];

  // 全部项目区：groupByCwd 二级折叠（组内/组间均按最近活动降序，空组不产生）
  const groups = mode === "all" ? groupByCwd(filtered) : [];

  const renderRows = (list: AgentHistorySession[]) =>
    list.map((s) => (
      <HistorySessionRow
        key={keyOf(s.cliId, s.sessionId)}
        session={s}
        selected={s.sessionId === selectedId}
        onSelect={onSelect}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
        {...rowFlags(s)}
      />
    ));

  return (
    <div>
      {mode === "current" ? (
        currentSessions.length === 0 ? (
          <div style={emptyHintStyle}>
            {search.trim() ? "无匹配的会话" : "该项目暂无历史会话"}
          </div>
        ) : (
          renderRows(currentSessions)
        )
      ) : groups.length === 0 ? (
        <div style={emptyHintStyle}>
          {search.trim() ? "无匹配的会话" : "暂无历史会话"}
        </div>
      ) : (
        groups.map((group) => {
          // 组键：规范化 cwd；无 cwd 的未知目录组键 = ""（组标题「(未知目录)」）
          const key = group.cwd ?? "";
          const expanded = expandedGroups.has(key);
          return (
            <div key={key}>
              {/* 组标题 = basename + (N) 计数；title 悬停完整路径；未知目录组无 title */}
              <div
                data-e2e="agent-history-group"
                style={groupHeaderStyle}
                onClick={() => toggleGroup(key)}
                title={group.cwd ?? undefined}
              >
                <span style={arrowStyle}>
                  {expanded ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
                </span>
                <span>{group.cwd ? basename(group.cwd) : "(未知目录)"}</span>
                <span style={groupCountStyle}>({group.sessions.length})</span>
              </div>
              {expanded && (
                // 组内容：再缩进 12px + 二级引导线（问题 4 树形层级）
                <div
                  style={{
                    paddingLeft: 12,
                    borderLeft: `1px solid ${SIDEBAR_COLORS.treeGuide}`,
                    marginLeft: 7, // 对齐组标题箭头右侧内容
                  }}
                >
                  {renderRows(group.sessions)}
                </div>
              )}
            </div>
          );
        })
      )}

      <ContextMenu state={menu} onClose={closeMenu} />

      {dialogSession && (
        <SessionActionDialog
          title="会话运行中"
          message="该会话已在运行中，恢复会与现有会话冲突。"
          actions={[
            {
              label: "切换到该会话操作页面",
              action: () => {
                const target = dialogSession;
                setDialogSession(null);
                void handleSwitchToSession(target);
              },
            },
          ]}
          onCancel={() => setDialogSession(null)}
        />
      )}
    </div>
  );
};
