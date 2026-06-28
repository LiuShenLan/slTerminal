// Workspace — 多 Dockview 实例架构
//
// 每个操作页面拥有独立 <DockviewReact> 实例。
// 页面切换通过 CSS display:none/block 实现，终端不销毁。
// xterm.js 不支持二次 open()（Issue #4978），此架构从根本上解决 H6（终端跨页面存活）。
//
// 约束：#7 布局单点 — 每个 PageDockview 的 onDidLayoutChange 直接写 store
//       #8 会话单点 — 终端会话只在面板内管理，不跨页面

import React, { useCallback, useRef, useState, useMemo } from "react";
import {
  DockviewReact,
  type DockviewApi,
  type IDockviewPanelProps,
  type GetTabContextMenuItemsParams,
  type ReactContextMenuItemConfig,
  type BuiltInContextMenuItem,
  type IDockviewHeaderActionsProps,
  type IWatermarkPanelProps,
} from "dockview-react";
import { Allotment } from "allotment";
import "allotment/dist/style.css";
import { panelRegistry, PANEL_TERMINAL, PANEL_EDITOR } from "./panelRegistry";
import { saveLayout, loadLayout } from "./layoutSerde";
import { SidebarTree } from "../features/sidebar";
import { ExplorerPanel } from "../features/explorer";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { ErrorBoundary } from "../lib";
import { INPUT_BORDER, SECONDARY_BG, BUTTON_FG, PLACEHOLDER_FG, SEPARATOR_BG } from "../theme";

declare global {
  interface Window {
    __dockviewApi?: DockviewApi;
    __slterm_e2e_workspaceReady?: boolean;
  }
}

const WATERMARK_TEXT = "打开终端或编辑器开始工作";

// ---- 工厂函数：创建 per-page 的 Dockview 子组件 ----

/** 创建 Watermark 组件（捕获 pageId + cwd 闭包） */
function createWatermark(
  nextPanelId: () => string,
  cwd: string | undefined,
): React.FC<IWatermarkPanelProps> {
  const Watermark: React.FC<IWatermarkPanelProps> = ({ containerApi }) => (
    <div
      style={{ display: "flex", flexDirection: "column", alignItems: "center",
        justifyContent: "center", height: "100%", color: INPUT_BORDER, fontSize: 14,
        userSelect: "none", gap: 12 }}
    >
      <span>{WATERMARK_TEXT}</span>
      <div style={{ display: "flex", gap: 8 }}>
        <button
          onClick={() => {
            const id = nextPanelId();
            containerApi.addPanel({ id, component: PANEL_TERMINAL,
              params: { panelId: id, cwd }, renderer: "always" });
          }}
          style={{ background: SECONDARY_BG, border: `1px solid ${SEPARATOR_BG}`, color: BUTTON_FG,
            cursor: "pointer", fontSize: 13, padding: "4px 12px", borderRadius: 4 }}
        >新建终端</button>
        <button
          onClick={() => {
            const id = nextPanelId();
            containerApi.addPanel({ id, component: PANEL_EDITOR,
              params: { panelId: id, cwd } });
          }}
          style={{ background: SECONDARY_BG, border: `1px solid ${SEPARATOR_BG}`, color: BUTTON_FG,
            cursor: "pointer", fontSize: 13, padding: "4px 12px", borderRadius: 4 }}
        >新建编辑器</button>
      </div>
    </div>
  );
  return Watermark;
}

/** 创建 RightHeaderActions 组件（捕获 pageId + cwd 闭包） */
function createRightHeader(
  nextPanelId: () => string,
  cwd: string | undefined,
): React.FC<IDockviewHeaderActionsProps> {
  const Header: React.FC<IDockviewHeaderActionsProps> = ({ containerApi }) => (
    <div style={{ display: "flex", alignItems: "center", height: "100%", paddingRight: 4 }}>
      <button
        onClick={() => {
          const id = nextPanelId();
          containerApi.addPanel({ id, component: PANEL_TERMINAL,
            params: { panelId: id, cwd }, renderer: "always" });
        }}
        style={{ background: "none", border: `1px solid ${SEPARATOR_BG}`, color: BUTTON_FG,
          cursor: "pointer", fontSize: 16, width: 24, height: 24, borderRadius: 4,
          display: "flex", alignItems: "center", justifyContent: "center", lineHeight: 1 }}
        title="新建终端"
      >+</button>
    </div>
  );
  return Header;
}

/** 创建 getTabContextMenuItems 回调（捕获 pageId 闭包） */
function createGetContextMenu(
  nextPanelId: () => string,
): (params: GetTabContextMenuItemsParams) => (BuiltInContextMenuItem | ReactContextMenuItemConfig)[] {
  return (params: GetTabContextMenuItemsParams) => {
    const newTerminalId = nextPanelId();
    const newEditorId = nextPanelId();
    return [
      { label: "新建终端", action: () => { params.api.addPanel(
          { id: newTerminalId, component: PANEL_TERMINAL, params: { panelId: newTerminalId },
            renderer: "always" }); } },
      { label: "新建编辑器", action: () => { params.api.addPanel(
          { id: newEditorId, component: PANEL_EDITOR, params: { panelId: newEditorId } }); } },
      "separator",
      "close", "closeOthers", "closeAll",
    ];
  };
}

// ---- DefaultTab ----

const DefaultTab: React.FC<IDockviewPanelProps> = (props) => {
  const { api } = props;
  const title = api.title || api.component || "";
  return (
    <div style={{ display: "flex", alignItems: "center", height: "100%",
      padding: "0 8px", gap: 6, userSelect: "none" }}>
      <span style={{ fontSize: 13 }}>{title}</span>
      <button
        onClick={(e) => { e.stopPropagation(); api.close(); }}
        style={{ background: "none", border: "none", color: PLACEHOLDER_FG,
          cursor: "pointer", padding: "0 2px", fontSize: 14, lineHeight: 1 }}
        title="关闭"
      >×</button>
    </div>
  );
};

// ---- PageDockview — 单个操作页面的 Dockview 实例 ----

interface PageDockviewProps {
  pageId: string;
  cwd: string | undefined;
  savedLayout: Record<string, unknown> | undefined;
  visible: boolean;
  onReady: (api: DockviewApi) => void;
  onLayoutChange: (layout: Record<string, unknown>) => void;
}

const PageDockview: React.FC<PageDockviewProps> = ({
  pageId, cwd, savedLayout, visible, onReady: onApiReady, onLayoutChange,
}) => {
  const apiRef = useRef<DockviewApi | null>(null);
  const panelSeqRef = useRef(0);
  const restoreGuardRef = useRef(false);

  /** per-page 稳定 panel ID 生成器 */
  const nextPanelId = useCallback((): string => {
    const seq = panelSeqRef.current++;
    return `terminal-${pageId}-${seq}`;
  }, [pageId]);

  // per-page 子组件（useMemo 防止每次渲染重建组件引用）
  const Watermark = useMemo(() => createWatermark(nextPanelId, cwd), [nextPanelId, cwd]);
  const RightHeader = useMemo(() => createRightHeader(nextPanelId, cwd), [nextPanelId, cwd]);
  const getTabContextMenuItems = useMemo(
    () => createGetContextMenu(nextPanelId), [nextPanelId]);

  const handleReady = useCallback((event: { api: DockviewApi }) => {
    const { api } = event;
    apiRef.current = api;
    onApiReady(api);

    // 恢复保存的布局或创建默认终端
    let restored = false;
    if (savedLayout && Object.keys(savedLayout).length > 0) {
      restored = loadLayout(api, savedLayout);
    }
    if (!restored) {
      // 无保存布局 或 恢复失败 → 创建默认终端
      const id = nextPanelId();
      api.addPanel({ id, component: PANEL_TERMINAL,
        params: { panelId: id, cwd }, renderer: "always" });
    }

    // fromJSON 恢复守卫 — 程序化恢复不触发布局保存
    api.onDidLayoutFromJSON(() => {
      restoreGuardRef.current = true;
      setTimeout(() => { restoreGuardRef.current = false; }, 0);
    });

    // 布局变更 → 保存到 store（硬约束 #7）
    api.onDidLayoutChange(() => {
      if (restoreGuardRef.current) return;
      const layout = saveLayout(api);
      onLayoutChange(layout as Record<string, unknown>);
    });
  }, [onApiReady, savedLayout, cwd, nextPanelId, onLayoutChange]);

  return (
    <div style={{ display: visible ? "block" : "none",
      width: "100%", height: "100%" }}>
      <DockviewReact
        className="dockview-theme-dark"
        components={panelRegistry}
        onReady={handleReady}
        watermarkComponent={Watermark}
        defaultTabComponent={DefaultTab}
        rightHeaderActionsComponent={RightHeader}
        getTabContextMenuItems={getTabContextMenuItems}
      />
    </div>
  );
};

// ---- Workspace 主组件 ----

const Workspace: React.FC = () => {
  // E2E 测试就绪信号：Workspace 挂载后立即可见（渲染阶段同步设置，非 useEffect）
  window.__slterm_e2e_workspaceReady = true;

  const pageApiMapRef = useRef<Map<string, DockviewApi>>(new Map());
  const [initializedPages, setInitializedPages] = useState<Set<string>>(new Set());

  const activePageId = useLayout((s) => s.activePageId);
  const projects = useProjects((s) => s.projects);

  /** 收集所有操作页面（扁平化列表） */
  const allPages = useMemo(() => {
    const pages: {
      projectId: string; pageId: string;
      cwd?: string; layout?: Record<string, unknown>;
    }[] = [];
    for (const [projId, proj] of Object.entries(projects)) {
      for (const page of proj.pages) {
        pages.push({
          projectId: projId, pageId: page.pageId,
          cwd: page.cwd, layout: page.layout as Record<string, unknown> | undefined,
        });
      }
    }
    return pages;
  }, [projects]);

  /** 确保目标页面已初始化（惰性创建 Dockview 实例） */
  const ensurePageInitialized = useCallback((pageId: string) => {
    setInitializedPages((prev) => {
      if (prev.has(pageId)) return prev;
      return new Set([...prev, pageId]);
    });
  }, []);

  /** 操作页面切换（仅更新 activePageId + CSS 显隐，projectId 保留兼容 SidebarTree 接口） */
  const switchToPage = useCallback((_projectId: string, pageId: string) => {
    const layoutStore = useLayout.getState();
    if (layoutStore.activePageId === pageId) return;

    ensurePageInitialized(pageId);
    layoutStore.setActivePage(pageId);

    // 更新 E2E 全局 API 指向活跃页面
    const api = pageApiMapRef.current.get(pageId);
    if (api) window.__dockviewApi = api;
  }, [ensurePageInitialized]);

  /** 删除操作页面 */
  const onDeletePage = useCallback((projectId: string, pageId: string) => {
    const layoutStore = useLayout.getState();
    const isActive = layoutStore.activePageId === pageId;

    // 销毁该页面的 Dockview（触发面板卸载 → useXterm cleanup → PTY kill）
    // P2-49: dockview-react api.dispose() 内部自动清理所有事件监听器
    // （onDidLayoutChange、onDidLayoutFromJSON、onDidActiveGroupChange 等），
    // 也会触发所有面板的 dispose，无需额外手动解绑。
    const api = pageApiMapRef.current.get(pageId);
    if (api) {
      api.clear();
      api.dispose();
      pageApiMapRef.current.delete(pageId);
    }

    // 从 store 移除
    useProjects.getState().removePage(projectId, pageId);

    // 从初始化集合移除（React 将卸载该 PageDockview）
    setInitializedPages((prev) => {
      const next = new Set(prev);
      next.delete(pageId);
      return next;
    });

    if (isActive) {
      layoutStore.setActivePage(null);
      const nextPageId = useProjects.getState().projects[projectId]?.activePageId;
      if (nextPageId) {
        ensurePageInitialized(nextPageId);
        layoutStore.setActivePage(nextPageId);
        const nextApi = pageApiMapRef.current.get(nextPageId);
        if (nextApi) window.__dockviewApi = nextApi;
      }
    }
  }, [ensurePageInitialized]);

  /** PageDockview onReady: 注册 API */
  const handlePageApiReady = useCallback((pageId: string, api: DockviewApi) => {
    pageApiMapRef.current.set(pageId, api);
    if (pageId === useLayout.getState().activePageId) {
      window.__dockviewApi = api;
    }
  }, []);

  /** PageDockview 布局变更: 写入 store */
  const handlePageLayoutChange = useCallback(
    (pageId: string, layout: Record<string, unknown>) => {
      const { projects: projs } = useProjects.getState();
      for (const [projId, proj] of Object.entries(projs)) {
        if (proj.pages.some((p) => p.pageId === pageId)) {
          useProjects.getState().updatePageLayout(projId, pageId, layout);
          break;
        }
      }
    }, []);

  // E2E 兼容：activePageId 变化时自动初始化对应页面（Workspace 挂载后生效）
  React.useEffect(() => {
    if (activePageId) ensurePageInitialized(activePageId);
  }, [activePageId, ensurePageInitialized]);

  return (
    <div style={{ width: "100%", height: "100%" }}>
      <Allotment>
        <Allotment.Pane preferredSize={250} minSize={160} maxSize={400}>
          <SidebarTree switchToPage={switchToPage} onDeletePage={onDeletePage} />
        </Allotment.Pane>
        <Allotment.Pane preferredSize={250} minSize={180} maxSize={500}>
          <ExplorerPanel />
        </Allotment.Pane>
        <Allotment.Pane minSize={200}>
          <div style={{ width: "100%", height: "100%", position: "relative" }}>
            {allPages.map((page) =>
              initializedPages.has(page.pageId) ? (
                <ErrorBoundary key={page.pageId} variant="inline">
                  <PageDockview
                    pageId={page.pageId}
                    cwd={page.cwd}
                    savedLayout={page.layout}
                    visible={page.pageId === activePageId}
                    onReady={(api) => handlePageApiReady(page.pageId, api)}
                    onLayoutChange={(layout) => handlePageLayoutChange(page.pageId, layout)}
                  />
                </ErrorBoundary>
              ) : null
            )}
          </div>
        </Allotment.Pane>
      </Allotment>
    </div>
  );
};

export default Workspace;
