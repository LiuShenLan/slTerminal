// NavTree.tsx —— 统一导航树主组件（NAV-01/02/03/04/09）
//
// 层级（UI-303，决策 5）：项目 → 页面 → 会话；活跃会话挂页面下（panelId→pageId），
// 历史会话折叠节点挂项目下（cwd 归属）。
// 顶部（NAV-04/FT-07）：分组标题「导航」（11px 全大写 0.08em fg-3）+ 刷新钮（IconRefresh）
// + 搜索框（#1a1a1e 底 INPUT_BG、圆角 5、12px、占位「搜索项目 / 页面 / 会话…」fg-4、
// focus 描边 FOCUS_BORDER）。
// 行结构（UI-501/502/503）：chevron 12px fg-3 + 图标 + 名称 + 右侧 11px fg-4 元数据；
//   行高 28（会话行 30）、圆角 5、hover #222227；选中行 accent-dim 底（hover 0.22）+ fg-1；
//   每级左缩 15px + 1px 发丝引导线（SIDEBAR_COLORS.treeGuide）。
// 项目行（NAV-09/UI-505）：500 字重 fg-1 + 彩色文件夹图标 + 「当前」pill + 页面计数 pill。
// 历史节点（NAV-03/UI-303）：时钟图标 + 「历史session」+ 计数 pill；展开 = 单行历史行
//   （StatusDot + logo + 标题 + 相对时间，prompt 预览 → 原生 title tooltip）；
//   双击恢复三分支（运行中 → SessionActionDialog / 孤儿/无 cwd → 无操作 / 普通 → 恢复）
//   + 右键菜单（复制恢复命令/分支恢复/删除）沿用 historyContextMenu 策略（原 HistorySessionList（已删）右键菜单）。
// 搜索（NAV-04）：子串不区分大小写过滤项目/页面/会话名；父节点因子命中而显示；
//   查询非空时命中链自动展开（searching 覆盖手动展开态）。
//
// CRUD 迁移自 SidebarTree（NAV-06 承接约定，行为不变）：添加项目 / 新建页面 /
// 删除项目（confirmDialog 确认，OV-02）/ 删除页面（onDeletePage 委托）/ 内联重命名；
// 右键菜单删除「打开 Hooks 配置」项（决策 4 入口唯一化——配置钮移至活动栏底部）。
// 新建页面空布局 = makeEmptyLayout（迁移自 SidebarTree——空布局由 Watermark 接管）。
//
// 测试数据属性契约（写死）：容器 data-e2e="nav-tree"；行 data-e2e=
// nav-row-project / nav-row-page / nav-row-session / nav-history-node。
//
// 展开/折叠默认收起（NAV-10 契约：测试辅助按点击展开驱动；行点击 = 切换展开 +
// 行为委托：页面行点击同时切换页面——照 SidebarTree 切换语义）。
// 历史节点常驻项目下（不随项目展开态隐藏）——计数 pill 与历史行入口恒可见（NAV-10 契约）。

import React, { useCallback, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import { useProjects, createProjectId, createPageId } from "../../stores/projects";
import type { OperationPage, Project } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import { useNavTree } from "./useNavTree";
import type { NavProjectModel, NavSessionModel } from "./useNavTree";
import { NavProjectRow } from "./NavProjectRow";
import { NavPageRow } from "./NavPageRow";
import { NavSessionRow } from "./NavSessionRow";
import { NavHistoryNode } from "./NavHistoryNode";
import { NavHistoryRow } from "./NavHistoryRow";
import { NavContextMenu } from "./NavContextMenu";
import type { NavMenuState } from "./NavContextMenu";
import { open } from "../../ipc/dialog";
import { writeText } from "../../ipc/clipboard";
import { deleteHistorySession } from "../../ipc/agentHistory";
import { sendToastNotification } from "../../ipc/notification";
import {
  buildResumeCommand,
  getHistoryContextMenuItems,
  keyOf,
  restoreHistorySession,
  SessionActionDialog,
} from "../agentHistory";
import { confirmDialog } from "../../lib";
import {
  findPageIdForPanelId,
  findPanelForSession,
  switchToPageAndFocus,
} from "../../workspace/pageApis";
import {
  IconEmptyBox,
  IconHistory,
  IconPlus,
  IconRefresh,
  IconSearch,
} from "../../lib/icons";
import { childrenStyle } from "./navStyles";
import {
  DIM_FG,
  FOCUS_BORDER,
  INPUT_BG,
  PANEL_BG,
  PLACEHOLDER_FG,
  SEPARATOR_BG,
  SIDEBAR_COLORS,
  SIDEBAR_FG,
} from "../../theme";
import type { AgentHistorySession } from "../../types/agentHistory";

/**
 * 生成新操作页面的空白布局（不含任何默认面板）——迁移自 SidebarTree（NAV-06 承接约定）。
 * 新页面显示 Watermark 组件（"打开终端或编辑器开始工作"）。
 */
export function makeEmptyLayout(): Record<string, unknown> {
  return {};
}

/**
 * 导航树 props——switchToPage/onDeletePage 可选（NAV-10 契约：独立渲染
 * `<NavTree />` 无 props；宿主侧（SideBarArea，NAV-05）透传时行为与 SidebarTree 一致）。
 */
export interface NavTreeProps {
  /** 切换到指定操作页面（async——切换完成后再开面板）；缺省回退 switchToPageShared */
  switchToPage?: (projectId: string, pageId: string) => Promise<void>;
  /** 删除指定操作页面；缺省回退 store 级 removePage */
  onDeletePage?: (projectId: string, pageId: string) => void;
}

// 反查函数 findPanelForSession / findPageIdForPanelId 已上提 workspace/pageApis（FE-09）

// ---- 顶部/搜索/空态/按钮样式 ----

/** 分组标题「导航」（FT-07：11px 全大写 0.08em fg-3）+ 刷新钮行 */
const headerStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: "8px 12px 8px", // GL-04：间距收敛 10/6 → 8
  fontSize: 11,
  letterSpacing: "0.08em",
  textTransform: "uppercase",
  color: DIM_FG, // fg-3
  userSelect: "none",
  flexShrink: 0,
};

/** 刷新钮（13px 线性图标，fg-3 hover fg-1） */
const refreshBtnStyle: CSSProperties = {
  background: "none",
  border: "none",
  padding: 0,
  display: "flex",
  alignItems: "center",
  cursor: "pointer",
  color: DIM_FG,
};

/** 搜索框外层（NAV-04：位于分组标题下） */
const searchWrapStyle: CSSProperties = {
  padding: "2px 12px 8px", // GL-04：间距收敛 10 → 12
  flexShrink: 0,
};

/** 搜索框（#1a1a1e 底 INPUT_BG、圆角 5、12px；focus 描边 FOCUS_BORDER） */
const searchBoxStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  background: INPUT_BG,
  border: "1px solid transparent",
  borderRadius: 5,
  padding: "5px 8px",
  color: DIM_FG,
  fontSize: 12,
};

const searchInputStyle: CSSProperties = {
  background: "none",
  border: "none",
  // UI-808：input 键盘可达，去 outline:none 让全局 :focus-visible 环生效
  font: "inherit",
  width: "100%",
  minWidth: 0,
};

/** 空态（UI-806：15px 线性图标 fg-4 + 说明 fg-3，居中） */
const emptyStateStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 8,
  padding: "20px 12px",
  fontSize: 12,
  color: DIM_FG, // 说明 fg-3
  textAlign: "center",
  userSelect: "none",
};

/** 底部「添加项目」钮（CRUD 迁移自 SidebarTree 工具栏按钮） */
const addButtonStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 6,
  width: "100%",
  padding: "5px 0",
  background: INPUT_BG,
  border: "none",
  borderRadius: 5,
  color: SIDEBAR_COLORS.fg, // fg-2
  fontSize: 12,
  cursor: "pointer",
  userSelect: "none",
};

// ---- 主组件 ----

export const NavTree: React.FC<NavTreeProps> = ({ switchToPage, onDeletePage }) => {
  const nav = useNavTree();
  const renamePage = useProjects((s) => s.renamePage);

  /** 当前正在内联重命名的页面 ID（null = 无） */
  const [renamingPageId, setRenamingPageId] = useState<string | null>(null);
  const [menu, setMenu] = useState<NavMenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });
  /** 双击运行中会话的动作弹窗目标（null = 关闭）——原 HistorySessionList（已删）语义 */
  const [dialogSession, setDialogSession] = useState<AgentHistorySession | null>(
    null,
  );
  const [searchFocused, setSearchFocused] = useState(false);

  const closeMenu = useCallback(() => {
    setMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  // 页面切换委托：宿主注入 switchToPage（SideBarArea，NAV-05）优先（含
  // setProjectRoot 前置 + 页面初始化编排）；独立渲染兜底 = setActivePage
  // （仅切活跃页面，路径沙箱/页面初始化由宿主链路保证——生产恒走宿主 prop）
  const switchPage = useCallback(
    (projectId: string, pageId: string) => {
      if (switchToPage) return switchToPage(projectId, pageId);
      void useLayout.getState().setActivePage(pageId);
    },
    [switchToPage],
  );

  // 删除页面委托：宿主注入 onDeletePage（Workspace 编排）优先；
  // 独立渲染兜底 = store 级 removePage（不清理 Dockview 布局）
  const deletePage = useCallback(
    (projectId: string, pageId: string) => {
      if (onDeletePage) return onDeletePage(projectId, pageId);
      useProjects.getState().removePage(projectId, pageId);
    },
    [onDeletePage],
  );

  // 添加项目（迁移自 SidebarTree.handleAddProject——行为不变：选文件夹 → 建项目 + 默认空布局页面）
  const handleAddProject = useCallback(async () => {
    try {
      const result = await open({
        directory: true,
        multiple: false,
        title: "选择项目文件夹",
      });
      if (!result) return;
      const dirPath = Array.isArray(result) ? result[0] : result;
      if (!dirPath) return;

      const name = dirPath.split(/[/\\]/).pop() || dirPath;
      const projectId = createProjectId();
      const pageId = createPageId();
      const page: OperationPage = {
        pageId,
        name,
        layout: makeEmptyLayout(),
        cwd: dirPath,
        createdAt: Date.now(),
        lastAccessedAt: Date.now(),
      };
      const project: Project = {
        projectId,
        name,
        rootPath: dirPath,
        pages: [page],
        activePageId: pageId,
        version: 1,
      };
      useProjects.getState().addProject(project);
    } catch (err) {
      console.error("[slTerminal] 添加项目失败:", err);
    }
  }, []);

  // 新建操作页面（迁移自 SidebarTree.handleNewPage——行为不变，不自动切换）
  const handleNewPage = useCallback((projectId: string, cwd: string) => {
    const pageId = createPageId();
    const page: OperationPage = {
      pageId,
      name: `页面-${Date.now() % 10000}`,
      layout: makeEmptyLayout(),
      cwd,
      createdAt: Date.now(),
      lastAccessedAt: Date.now(),
    };
    useProjects.getState().addPage(projectId, page);
  }, []);

  // 点击会话行 → 聚焦对应终端页签（照 AgentStatusView 现跳转逻辑 + B14 防御分层）
  const handleSessionFocus = useCallback(async (panelId: string) => {
    const pageId = findPageIdForPanelId(panelId);
    if (!pageId) return;
    await switchToPageAndFocus(pageId, panelId);
  }, []);

  // 切换到该会话所在操作页面并聚焦终端页签（SessionActionDialog 动作，原 HistorySessionList（已删）语义）
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
        // B14: 解析不出属主页面时明确提示而非静默返回
        sendToastNotification("未找到运行中的会话", {
          body: "该会话已结束或无法定位其终端页签",
        });
        return;
      }
      await switchToPageAndFocus(pageId, panelId);
    },
    [],
  );

  // 历史行双击分派三分支（原 HistorySessionList（已删）语义：运行中 → 动作弹窗 / 孤儿/无 cwd → 无操作 / 普通 → 恢复）
  const handleHistoryDoubleClick = useCallback(
    (session: AgentHistorySession) => {
      const status = nav.activeStatuses.get(
        keyOf(session.cliId, session.sessionId),
      );
      const orphan = session.cwd !== null && !session.cwdExists;
      const noCwd = session.cwd === null;
      if (status != null) {
        setDialogSession(session);
      } else if (orphan || noCwd) {
        // 孤儿（起始目录已删除）/ 无 cwd 行：恢复失败概率高，禁用优于报错——无操作
      } else {
        // 普通恢复（fork: false 显式传——NAV-10 契约断言恢复编排入口双参形态）
        void restoreHistorySession(session, { fork: false });
      }
    },
    [nav.activeStatuses],
  );

  // 历史行右键菜单——委托 historyContextMenu 策略（原 HistorySessionList（已删）调用形态）
  const handleHistoryContextMenu = useCallback(
    (session: AgentHistorySession, pos: { x: number; y: number }) => {
      const status = nav.activeStatuses.get(
        keyOf(session.cliId, session.sessionId),
      );
      const title = session.title ?? session.sessionId.slice(0, 8);
      const items = getHistoryContextMenuItems(session, {
        active: status != null, // 运行中（删除禁用判定）
        orphan: session.cwd !== null && !session.cwdExists,
        noCwd: session.cwd === null,
        // 复制恢复命令（buildResumeCommand 委托 profile.history.buildResumeCommand）
        onCopy: () => {
          void writeText(buildResumeCommand(session));
        },
        // 分支恢复：fork 编排（--fork-session 复制历史到新 sessionId，原会话不动）
        onFork: () => {
          void restoreHistorySession(session, { fork: true });
        },
        // 删除：confirmDialog 确认 → 删除 IPC → removeLocal 即时局部刷新（不重扫）
        onDelete: () => {
          void confirmDialog({
            title: "确认删除",
            message: `确定删除会话"${title}"？此操作不可撤销。`,
            danger: true,
          }).then(async (ok) => {
            if (!ok) return;
            try {
              await deleteHistorySession(session.cliId, session.sessionId);
              nav.removeLocal(session.sessionId);
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
      // 策略项转菜单项：label 以「删除」开头 = 危险项（UI-802 ERROR_FG）
      setMenu({
        visible: true,
        x: pos.x,
        y: pos.y,
        items: items.map((item) => ({
          ...item,
          danger: item.label.startsWith("删除"),
        })),
      });
    },
    [nav.activeStatuses, nav.removeLocal],
  );

  // ---- 树渲染（查询过滤 + 展开判定） ----

  const renderSession = (sessionModel: NavSessionModel) => (
    <NavSessionRow
      key={sessionModel.row.panelId}
      row={sessionModel.row}
      active={sessionModel.active}
      onFocus={handleSessionFocus}
    />
  );

  const renderPage = (projModel: NavProjectModel, pageModel: NavProjectModel["pages"][number]) => {
    if (!pageModel.match) return null; // 搜索未命中页面整行隐藏（父节点因子命中而显示）
    const proj = projModel.project;
    const page = pageModel.page;
    // 展开判定：搜索中命中链自动展开，否则跟随手动展开态（NAV-01 组件内维护，默认收起）
    const expanded = nav.searching
      ? pageModel.sessions.length > 0
      : nav.expanded.has(page.pageId);
    // 页面行右侧元数据（11px fg-4）：页面收起且含活跃会话时显示最近会话标题（mockup 契约）
    const meta =
      !expanded && pageModel.sessions.length > 0
        ? pageModel.sessions[0].row.title
        : undefined;
    return (
      <div key={page.pageId}>
        <NavPageRow
          page={page}
          selected={page.pageId === nav.activePageId}
          expanded={expanded}
          meta={meta}
          isRenaming={renamingPageId === page.pageId}
          onRename={(newName) => {
            renamePage(proj.projectId, page.pageId, newName);
            setRenamingPageId(null);
          }}
          onCancelRename={() => setRenamingPageId(null)}
          onClick={() => {
            // 行点击 = 切换会话展开 + 切换页面（照 SidebarTree 切换语义，NAV-10 契约）
            nav.toggleExpand(page.pageId);
            void switchPage(proj.projectId, page.pageId);
          }}
          onToggle={() => nav.toggleExpand(page.pageId)}
          onContextMenu={(e) => {
            e.preventDefault();
            setMenu({
              visible: true,
              x: e.clientX,
              y: e.clientY,
              items: [
                {
                  label: "重命名操作页面",
                  action: () => setRenamingPageId(page.pageId),
                },
                {
                  label: "删除操作页面",
                  danger: true,
                  action: () => deletePage(proj.projectId, page.pageId),
                },
              ],
            });
          }}
        />
        {expanded && pageModel.sessions.length > 0 && (
          <div style={childrenStyle}>
            {pageModel.sessions.map((s) => renderSession(s))}
          </div>
        )}
      </div>
    );
  };

  const renderHistory = (model: NavProjectModel) => {
    const projId = model.project.projectId;
    // 搜索中仅当有会话行命中时显示历史节点；非搜索态常驻（NAV-10 契约）
    if (nav.searching && !model.history.match) return null;
    // 展开判定：搜索中命中链自动展开，否则跟随手动展开态（默认收起）
    const expanded = nav.searching
      ? model.history.sessions.length > 0
      : nav.expandedHist.has(projId);
    return (
      <NavHistoryNode
        key={`history-${projId}`}
        total={model.history.total}
        expanded={expanded}
        onToggle={() => {
          const willExpand = !expanded;
          nav.toggleHist(projId);
          // 展开触发重扫（照 agentHistory 历史区展开刷新语义；useAgentHistory generation 防竞）
          if (willExpand) void nav.refresh();
        }}
      >
        {model.history.sessions.length === 0 ? (
          // 空历史空态（NAV-03：15px 线性图标 fg-4 + 说明 fg-3）
          <EmptyState icon={<IconHistory size={15} />} text="暂无历史会话" />
        ) : (
          model.history.sessions.map((session) => (
            <NavHistoryRow
              key={keyOf(session.cliId, session.sessionId)}
              session={session}
              status={nav.activeStatuses.get(keyOf(session.cliId, session.sessionId))}
              onDoubleClick={handleHistoryDoubleClick}
              onContextMenu={handleHistoryContextMenu}
            />
          ))
        )}
      </NavHistoryNode>
    );
  };

  const renderProject = (model: NavProjectModel) => {
    const proj = model.project;
    const projId = proj.projectId;
    const hasChildren = model.pages.some((p) => p.match) || model.history.match;
    // 展开判定：搜索中命中链自动展开，否则跟随手动展开态（默认收起）
    const expanded = nav.searching ? hasChildren : nav.expanded.has(projId);
    return (
      <div key={projId}>
        <NavProjectRow
          project={proj}
          expanded={expanded}
          isCurrent={model.isCurrent}
          pageCount={proj.pages.length}
          onToggle={() => nav.toggleExpand(projId)}
          onContextMenu={(e) => {
            e.preventDefault();
            setMenu({
              visible: true,
              x: e.clientX,
              y: e.clientY,
              items: [
                {
                  label: "新建操作页面",
                  action: () => handleNewPage(projId, proj.rootPath),
                },
                {
                  label: "删除项目",
                  danger: true,
                  action: async () => {
                    // FE-03：window.confirm → 应用内 confirmDialog（OV-02 统一确认通道）
                    const ok = await confirmDialog({
                      title: "确认删除",
                      message: `确定删除项目 "${proj.name}"？`,
                      danger: true,
                    });
                    if (ok) useProjects.getState().removeProject(projId);
                  },
                },
              ],
            });
          }}
        />
        {expanded && (
          <div style={childrenStyle}>
            {model.pages.map((pageModel) => renderPage(model, pageModel))}
          </div>
        )}
        {/* 历史节点常驻项目下（不随项目展开态隐藏——NAV-10 契约）；外包
            childrenStyle 容器与操作页面同级缩进（15px + 发丝引导线），
            位于 pages 容器之后恒置最下方（人工验证问题 2 修订） */}
        <div style={childrenStyle}>{renderHistory(model)}</div>
      </div>
    );
  };

  const visibleProjects = nav.tree.filter((p) => p.match);

  return (
    <div
      data-e2e="nav-tree"
      style={{
        width: "100%",
        height: "100%",
        background: PANEL_BG,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      {/* 分组标题「导航」+ 刷新钮（刷新 = 重扫历史会话） */}
      <div style={headerStyle}>
        <span>导航</span>
        <button
          type="button"
          title="刷新"
          aria-label="刷新"
          onClick={() => void nav.refresh()}
          style={refreshBtnStyle}
        >
          <IconRefresh size={13} />
        </button>
      </div>

      {/* 搜索框（NAV-04：位于分组标题下） */}
      <div style={searchWrapStyle}>
        <div
          style={{
            ...searchBoxStyle,
            borderColor: searchFocused ? FOCUS_BORDER : "transparent",
          }}
          onFocus={() => setSearchFocused(true)}
          onBlur={() => setSearchFocused(false)}
        >
          <span style={{ display: "flex", flexShrink: 0 }}>
            <IconSearch size={12} />
          </span>
          <input
            value={nav.query}
            onChange={(e) => nav.setQuery(e.target.value)}
            placeholder="搜索项目 / 页面 / 会话…"
            // 占位符颜色 fg-4（NAV-04 契约）：::placeholder 无法用内联样式控制——
            // 输入为空时输入框文字即占位符文本（fg-4），有内容时切换 fg-1
            style={{ ...searchInputStyle, color: nav.query ? SIDEBAR_FG : PLACEHOLDER_FG }}
          />
        </div>
      </div>

      {/* 树区 */}
      <div
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          overflowX: "hidden",
          padding: "0 8px 12px", // GL-04：间距收敛 6 → 8
        }}
      >
        {visibleProjects.length === 0 ? (
          nav.tree.length === 0 ? (
            <EmptyState
              icon={<IconEmptyBox size={15} />}
              text="暂无项目，点击下方「添加项目」开始"
            />
          ) : (
            <EmptyState
              icon={<IconSearch size={15} />}
              text="没有找到匹配的项目 / 页面 / 会话"
            />
          )
        ) : (
          visibleProjects.map((model) => renderProject(model))
        )}
      </div>

      {/* 底部「添加项目」钮（CRUD 迁移自 SidebarTree 工具栏按钮） */}
      <div
        style={{
          padding: "6px 8px",
          borderTop: `1px solid ${SEPARATOR_BG}`,
          flexShrink: 0,
        }}
      >
        <button
          type="button"
          onClick={handleAddProject}
          title="添加项目"
          style={addButtonStyle}
        >
          <IconPlus size={12} />
          <span>添加项目</span>
        </button>
      </div>

      {/* 右键菜单（项目/页面/历史行共用） */}
      <NavContextMenu state={menu} onClose={closeMenu} />

      {/* 双击运行中历史会话的动作弹窗（原 HistorySessionList（已删）语义） */}
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

/** 空态（UI-806：15px 线性图标 fg-4 + 说明 fg-3，居中） */
const EmptyState: React.FC<{ icon: ReactNode; text: string }> = ({
  icon,
  text,
}) => (
  <div style={emptyStateStyle}>
    <span style={{ color: PLACEHOLDER_FG, display: "flex" }}>{icon}</span>
    <span>{text}</span>
  </div>
);
