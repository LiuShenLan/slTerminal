// Workspace — 多 Dockview 实例架构
//
// 每个操作页面拥有独立 <DockviewReact> 实例。
// 页面切换通过 CSS display:none/block 实现，终端不销毁。
// xterm.js 不支持二次 open()（Issue #4978），此架构从根本上解决 H6（终端跨页面存活）。
//
// 约束：#7 布局单点 — 每个 PageDockview 的 onDidLayoutChange 直接写 store
//       #8 会话单点 — 终端会话只在面板内管理，不跨页面

import React, { useCallback, useRef, useState, useMemo, useEffect } from "react";
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
import { panelRegistry, PANEL_TERMINAL, FILE_PANEL_TYPES } from "./panelRegistry";
import { saveLayout, loadLayout } from "./layoutSerde";
import { titleManager } from "./titleManager";
import type { TitleUpdate } from "./titleManager";
import { SidebarTree } from "../features/sidebar";
import { ExplorerPanel } from "../features/explorer";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { ErrorBoundary } from "../lib";
import { INPUT_BORDER, SECONDARY_BG, BUTTON_FG, PLACEHOLDER_FG, SEPARATOR_BG } from "../theme";
import { markWorkspaceReady } from "../../e2e-tests/helpers";

declare global {
  interface Window {
    __dockviewApi?: DockviewApi;
  }
}

/** Allotment 三栏布局尺寸约束（px） */
const SIDEBAR_PREFERRED_SIZE = 250;
const SIDEBAR_MIN_SIZE = 160;
const SIDEBAR_MAX_SIZE = 400;
const EXPLORER_PREFERRED_SIZE = 250;
const EXPLORER_MIN_SIZE = 180;
const EXPLORER_MAX_SIZE = 500;
const MAIN_MIN_SIZE = 200;

const WATERMARK_TEXT = "打开终端或编辑器开始工作";

// ---- 工厂函数：创建 per-page 的 Dockview 子组件 ----

/** 创建 Watermark 组件（捕获 pageId + cwd 闭包） */
function createWatermark(
  nextPanelId: () => string,
  pageId: string,
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
            const title = titleManager.getTerminalTitle(pageId);
            containerApi.addPanel({ id, component: PANEL_TERMINAL, title,
              params: { panelId: id, cwd }, renderer: "always" });
          }}
          style={{ background: SECONDARY_BG, border: `1px solid ${SEPARATOR_BG}`, color: BUTTON_FG,
            cursor: "pointer", fontSize: 13, padding: "4px 12px", borderRadius: 4 }}
        >新建终端</button>
      </div>
    </div>
  );
  return Watermark;
}

/** 创建 RightHeaderActions 组件（捕获 pageId + cwd 闭包） */
function createRightHeader(
  nextPanelId: () => string,
  pageId: string,
  cwd: string | undefined,
): React.FC<IDockviewHeaderActionsProps> {
  const Header: React.FC<IDockviewHeaderActionsProps> = ({ containerApi, group }) => (
    <div style={{ display: "flex", alignItems: "center", height: "100%", paddingRight: 4 }}>
      <button
        onClick={() => {
          const id = nextPanelId();
          const title = titleManager.getTerminalTitle(pageId);
          containerApi.addPanel({ id, component: PANEL_TERMINAL, title,
            params: { panelId: id, cwd }, renderer: "always",
            position: { referenceGroup: group } });
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
  pageId: string,
): (params: GetTabContextMenuItemsParams) => (BuiltInContextMenuItem | ReactContextMenuItemConfig)[] {
  return (params: GetTabContextMenuItemsParams) => {
    const newTerminalId = nextPanelId();
    return [
      { label: "新建终端", action: () => { params.api.addPanel(
          { id: newTerminalId, component: PANEL_TERMINAL, title: titleManager.getTerminalTitle(pageId),
            params: { panelId: newTerminalId }, renderer: "always",
            position: { referenceGroup: params.group } }); } },
      "separator",
      "close", "closeOthers", "closeAll",
    ];
  };
}

// ---- DefaultTab ----

/** 扩展的 params 类型（终端面板通过 updateParameters 设置 tabIcon） */
interface TabParams {
  panelId?: string;
  filePath?: string;
  cwd?: string;
  tabIcon?: string | null;
}

const DefaultTab: React.FC<IDockviewPanelProps> = (props) => {
  const { api, params } = props;
  const tabParams = params as TabParams;
  const [title, setTitle] = React.useState(api.title || api.component || "");
  const [tabIcon, setTabIcon] = React.useState<string | null>(
    tabParams?.tabIcon ?? null,
  );
  React.useEffect(() => {
    const d1 = api.onDidTitleChange((event) => {
      setTitle(event.title);
    });
    const d2 = api.onDidParametersChange((event) => {
      // event 就是 Parameters 对象本身（Dockview PanelApi.onDidParametersChange
      // 类型签名为 Event<Parameters>，回调直接接收 Parameters 对象）
      const p = event as TabParams;
      setTabIcon(p?.tabIcon ?? null);
    });
    return () => {
      d1.dispose();
      d2.dispose();
    };
  }, [api]);
  return (
    <div style={{ display: "flex", alignItems: "center", height: "100%",
      padding: "0 8px", gap: 6, userSelect: "none" }}>
      {tabIcon && (
        <img src={tabIcon} width={16} height={16}
          style={{ flexShrink: 0, display: "block" }} alt="" />
      )}
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
  rootPath: string | undefined;
  savedLayout: Record<string, unknown> | undefined;
  visible: boolean;
  onReady: (api: DockviewApi) => void;
  onLayoutChange: (layout: Record<string, unknown>) => void;
}

/** 将 TitleUpdate[] 应用到 DockviewApi（批量 setTitle） */
function applyTitleUpdates(
  api: DockviewApi,
  updates: TitleUpdate[],
): void {
  for (const { panelId, title } of updates) {
    const panel = api.getPanel(panelId);
    if (panel) panel.api.setTitle(title);
  }
}

/** 遍历 DockviewApi 中所有编辑器面板，重建 titleManager 注册表并重算标题 */
function rebuildAndRecomputeTitles(
  api: DockviewApi,
  pageId: string,
  rootPath: string | undefined,
): void {
  if (!rootPath) return;

  // 遍历所有面板，重建注册表
  for (const panel of api.panels) {
    const params = panel.params as { panelId?: string; filePath?: string } | undefined;
    if (!params?.panelId) continue;
    const component = panel.view?.contentComponent;
    if (component && FILE_PANEL_TYPES.has(component)) {
      const filePath = params.filePath;
      // 先注销旧条目（避免 fromJSON 重复注册）
      titleManager.unregisterEditor(pageId, params.panelId);
      titleManager.registerEditor(pageId, params.panelId, filePath);
    }
  }

  const updates = titleManager.recomputeTitles(pageId, rootPath);
  applyTitleUpdates(api, updates);
}

const PageDockview: React.FC<PageDockviewProps> = ({
  pageId, cwd, rootPath, savedLayout, visible, onReady: onApiReady, onLayoutChange,
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
  const Watermark = useMemo(
    () => createWatermark(nextPanelId, pageId, cwd),
    [nextPanelId, pageId, cwd],
  );
  const RightHeader = useMemo(
    () => createRightHeader(nextPanelId, pageId, cwd),
    [nextPanelId, pageId, cwd],
  );
  const getTabContextMenuItems = useMemo(
    () => createGetContextMenu(nextPanelId, pageId),
    [nextPanelId, pageId],
  );

  const handleReady = useCallback((event: { api: DockviewApi }) => {
    const { api } = event;
    apiRef.current = api;
    onApiReady(api);

    // 恢复保存的布局（无布局时留空，由 Watermark 组件接管显示）
    let restored = false;
    if (savedLayout && Object.keys(savedLayout).length > 0) {
      restored = loadLayout(api, savedLayout);
    }
    // 不创建默认终端——空白页面由 watermarkComponent 渲染
    // "打开终端或编辑器开始工作"，用户可点击"新建终端"按钮

    // 从保存布局恢复后，重建编辑器注册表并重算标题（忽略持久化的 title）
    if (restored) {
      rebuildAndRecomputeTitles(api, pageId, rootPath);
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

    // 面板关闭 → 注销编辑器 + 重算剩余面板标题
    api.onDidRemovePanel((panel) => {
      const params = panel.params as { panelId?: string } | undefined;
      if (params?.panelId) {
        titleManager.unregisterEditor(pageId, params.panelId);
      }
      if (rootPath) {
        const updates = titleManager.recomputeTitles(pageId, rootPath);
        applyTitleUpdates(api, updates);
      }
    });
  }, [onApiReady, savedLayout, cwd, pageId, rootPath, nextPanelId, onLayoutChange]);

  // 监听 slterm:file-saved-as 事件（Ctrl+S 另存为 / 首次保存后更新标题）
  useEffect(() => {
    const onSaveAs = (e: Event) => {
      const detail = (e as CustomEvent).detail as {
        panelId: string;
        oldPath: string | null;
        newPath: string;
      };
      if (!rootPath) return;
      const updates = titleManager.handleSaveAs(
        pageId, detail.panelId, detail.newPath, rootPath,
      );
      const api = apiRef.current;
      if (api) applyTitleUpdates(api, updates);
    };

    window.addEventListener("slterm:file-saved-as", onSaveAs);
    return () => {
      window.removeEventListener("slterm:file-saved-as", onSaveAs);
    };
  }, [pageId, rootPath]);

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
  if (import.meta.env.DEV) {
    markWorkspaceReady();
  }

  const pageApiMapRef = useRef<Map<string, DockviewApi>>(new Map());
  const [initializedPages, setInitializedPages] = useState<Set<string>>(new Set());

  const activePageId = useLayout((s) => s.activePageId);
  const projects = useProjects((s) => s.projects);

  /** 收集所有操作页面（扁平化列表） */
  const allPages = useMemo(() => {
    const pages: {
      projectId: string; pageId: string; rootPath: string;
      cwd?: string; layout?: Record<string, unknown>;
    }[] = [];
    for (const [projId, proj] of Object.entries(projects)) {
      for (const page of proj.pages) {
        pages.push({
          projectId: projId, pageId: page.pageId, rootPath: proj.rootPath,
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
        <Allotment.Pane preferredSize={SIDEBAR_PREFERRED_SIZE} minSize={SIDEBAR_MIN_SIZE} maxSize={SIDEBAR_MAX_SIZE}>
          <SidebarTree switchToPage={switchToPage} onDeletePage={onDeletePage} />
        </Allotment.Pane>
        <Allotment.Pane preferredSize={EXPLORER_PREFERRED_SIZE} minSize={EXPLORER_MIN_SIZE} maxSize={EXPLORER_MAX_SIZE}>
          <ExplorerPanel />
        </Allotment.Pane>
        <Allotment.Pane minSize={MAIN_MIN_SIZE}>
          <div style={{ width: "100%", height: "100%", position: "relative" }}>
            {allPages.map((page) =>
              initializedPages.has(page.pageId) ? (
                <ErrorBoundary key={page.pageId} variant="inline">
                  <PageDockview
                    pageId={page.pageId}
                    cwd={page.cwd}
                    rootPath={page.rootPath}
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

// 供测试验证 RightHeader/ContextMenu 的 addPanel 行为
export { createRightHeader, createGetContextMenu };
