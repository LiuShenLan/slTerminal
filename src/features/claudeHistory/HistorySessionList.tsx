// HistorySessionList.tsx — 历史会话列表（FE-07 集成部分）
//
// 两种展示模式（props.mode）：
//   current —— 当前项目区：平铺 HistorySessionRow（isCurrentProject 按 rootPath 过滤，mtime 降序）
//   all     —— 全部项目区：groupByCwd 二级折叠（组标题 = basename + title 悬停完整路径，空组不显示）
// 搜索过滤经 matchesSearch（Stage 04）作用于两区（README 4.3.4）。
//
// 交互分派：
//   双击（行 onDoubleClick 消费方）三分支（README 4.3.2/4.3.3）：
//     普通行 → restoreHistorySession(session)；孤儿/无 cwd 行 → 无操作；
//     ⚡ 运行中行 → dialog.ask「该会话已在运行中」→ 确认走 fork 恢复
//   右键菜单 → getHistoryContextMenuItems 策略查询；删除/重命名完成回调
//   （removeLocal/updateLocalTitle 来自 useClaudeHistory）经 props 注入。
// 重命名经 InputDialog（调用方打开，提交后 renameHistorySession → updateLocalTitle）。
//
// 配色全部 theme/colors.ts token（硬约束 #6），零硬编码色值。

import React, { useCallback, useEffect, useRef, useState } from "react";
import { ask } from "../../ipc/dialog";
import { writeText } from "../../ipc/clipboard";
import {
  deleteHistorySession,
  renameHistorySession,
} from "../../ipc/claudeHistory";
import { restoreHistorySession } from "./restoreSession";
import { HistorySessionRow } from "./HistorySessionRow";
import { InputDialog } from "./InputDialog";
import {
  buildResumeCommand,
  getHistoryContextMenuItems,
} from "./historyContextMenu";
import type { HistoryMenuItem } from "./historyContextMenu";
import { basename } from "../../lib/path";
import { groupByCwd, isCurrentProject, matchesSearch } from "./historyModel";
import type { HistorySession } from "../../types/claudeHistory";
import {
  EXPLORER_COLORS,
  INPUT_BORDER,
  PLACEHOLDER_FG,
  SIDEBAR_BG,
  SIDEBAR_FG,
  ACTIVE_SELECTION_BG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
} from "../../theme";

/** 折叠箭头样式 */
const arrowStyle: React.CSSProperties = {
  display: "inline-block",
  width: 16,
  fontSize: 10,
  color: EXPLORER_COLORS.arrowClosed,
  textAlign: "center",
  lineHeight: "22px",
  userSelect: "none",
  flexShrink: 0,
};

/** 组标题栏样式（全部项目区二级折叠头） */
const groupHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "2px 8px",
  height: 22,
  cursor: "pointer",
  userSelect: "none",
  fontSize: 12,
  color: EXPLORER_COLORS.fg,
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
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
        borderRadius: 4,
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
            style={{
              padding: "4px 12px",
              color: PLACEHOLDER_FG,
              fontSize: 13,
              userSelect: "none",
            }}
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
              padding: "4px 12px",
              cursor: "pointer",
              color: SIDEBAR_FG,
              fontSize: 13,
              userSelect: "none",
            }}
            onMouseEnter={(e) => {
              (e.target as HTMLDivElement).style.background =
                ACTIVE_SELECTION_BG;
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

// ── 历史会话列表 ──

export interface HistorySessionListProps {
  /** current = 当前项目区平铺；all = 全部项目区二级折叠分组 */
  mode: "current" | "all";
  /** 全部历史会话（未过滤未分组，本组件按 mode 派生） */
  sessions: HistorySession[];
  /** 当前项目 rootPath（null 时 current 区由 ClaudeHistorySections 显示「无活跃项目」，本组件不渲染） */
  rootPath: string | null;
  /** 搜索词（matchesSearch 过滤，作用于两区） */
  search: string;
  /** ⚡ 运行中会话 id 集合 */
  activeIds: Set<string>;
  /** 选中会话 id（受控，ClaudeHistorySections 持有） */
  selectedId: string | null;
  /** 单击选中回调 */
  onSelect(id: string): void;
  /** 删除成功后的即时局部刷新（useClaudeHistory.removeLocal，不重扫） */
  removeLocal(id: string): void;
  /** 重命名成功后的即时局部刷新（useClaudeHistory.updateLocalTitle，不重扫） */
  updateLocalTitle(id: string, title: string): void;
}

export const HistorySessionList: React.FC<HistorySessionListProps> = ({
  mode,
  sessions,
  rootPath,
  search,
  activeIds,
  selectedId,
  onSelect,
  removeLocal,
  updateLocalTitle,
}) => {
  const [menu, setMenu] = useState<MenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  const closeMenu = useCallback(() => {
    setMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  /** 重命名弹窗目标会话（null = 关闭） */
  const [renameTarget, setRenameTarget] = useState<HistorySession | null>(null);

  /** 全部项目区折叠的组集合（键 = 规范化 cwd；cwd 为 null 的未知目录组键 = ""） */
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    () => new Set(),
  );

  const toggleGroup = (key: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  /** 行状态标记派生（⚡ / ✗ / 无 cwd——Row 的 active/orphan/noCwd 三 props） */
  const rowFlags = useCallback(
    (session: HistorySession) => ({
      active: activeIds.has(session.sessionId),
      orphan: session.cwd !== null && !session.cwdExists,
      noCwd: session.cwd === null,
    }),
    [activeIds],
  );

  /** 双击分派三分支（README 4.3.2/4.3.3）：普通 → 恢复；孤儿/无 cwd → 无操作；⚡ → ask 引导分支恢复 */
  const handleDoubleClick = useCallback(
    (session: HistorySession) => {
      const { active, orphan, noCwd } = rowFlags(session);
      if (active) {
        // ⚡ 运行中：同一会话两终端不 fork 同时恢复会交错写入 transcript（破坏性行为），
        // 弹窗提示引导分支恢复（README 4.3.3）
        void ask("该会话已在运行中", {
          title: "会话运行中",
          kind: "warning",
          okLabel: "分支恢复",
        }).then((ok) => {
          if (ok) void restoreHistorySession(session, { fork: true });
        });
      } else if (orphan || noCwd) {
        // 孤儿（起始目录已删除）/ 无 cwd 行：恢复失败概率高，禁用优于报错（README 4.2）——无操作
      } else {
        void restoreHistorySession(session);
      }
    },
    [rowFlags],
  );

  /** 右键菜单——委托策略注册表；操作回调在本层实现（剪贴板/fork/删除/重命名） */
  const handleContextMenu = useCallback(
    (session: HistorySession, pos: { x: number; y: number }) => {
      const flags = rowFlags(session);
      const title = session.title ?? session.sessionId.slice(0, 8);
      const items = getHistoryContextMenuItems(session, {
        ...flags,
        // 复制恢复命令：命令构造见 buildResumeCommand（README 4.4，带单引号路径）
        onCopy: () => {
          void writeText(buildResumeCommand(session));
        },
        // 分支恢复：fork 编排（--fork-session 复制历史到新 sessionId，原会话不动）
        onFork: () => {
          void restoreHistorySession(session, { fork: true });
        },
        // 删除：ask 确认 → 删除 IPC → 成功后 removeLocal 即时局部刷新（不重扫）
        onDelete: () => {
          void ask(`确定删除会话"${title}"？此操作不可撤销。`, {
            title: "确认删除",
            kind: "warning",
          }).then(async (ok) => {
            if (!ok) return;
            try {
              await deleteHistorySession(session.sessionId);
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
        // 重命名：由本组件打开 InputDialog；提交后重命名 IPC → updateLocalTitle 即时刷新
        onRename: () => {
          setRenameTarget(session);
        },
      });
      setMenu({ visible: true, x: pos.x, y: pos.y, items });
    },
    [rowFlags, removeLocal],
  );

  /** 重命名提交：IPC 成功后 updateLocalTitle 即时刷新，关闭弹窗 */
  const handleRenameSubmit = useCallback(
    (session: HistorySession, value: string) => {
      void renameHistorySession(session.sessionId, value)
        .then(() => {
          updateLocalTitle(session.sessionId, value);
          setRenameTarget(null);
        })
        .catch((err) => {
          console.error(
            "[slTerminal] 重命名历史会话失败:",
            session.sessionId,
            err,
          );
        });
    },
    [updateLocalTitle],
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

  const renderRows = (list: HistorySession[]) =>
    list.map((s) => (
      <HistorySessionRow
        key={s.sessionId}
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
          const collapsed = collapsedGroups.has(key);
          return (
            <div key={key}>
              {/* 组标题 = basename；title 悬停完整路径；未知目录组无 title */}
              <div
                data-e2e="agent-history-group"
                style={groupHeaderStyle}
                onClick={() => toggleGroup(key)}
                title={group.cwd ?? undefined}
              >
                <span style={arrowStyle}>{collapsed ? "▶" : "▼"}</span>
                <span>{group.cwd ? basename(group.cwd) : "(未知目录)"}</span>
              </div>
              {!collapsed && renderRows(group.sessions)}
            </div>
          );
        })
      )}

      <ContextMenu state={menu} onClose={closeMenu} />

      {renameTarget && (
        <InputDialog
          title="重命名会话"
          initialValue={renameTarget.title ?? renameTarget.sessionId.slice(0, 8)}
          onSubmit={(value) => handleRenameSubmit(renameTarget, value)}
          onCancel={() => setRenameTarget(null)}
        />
      )}
    </div>
  );
};
