// Workspace — 统一分屏页签区
//
// 外层 flexbox：侧栏 (250px) + Dockview。
// 启动时自动创建终端面板（onReady）。
// Watermark 提供"新建终端""新建编辑器"按钮，新面板自动携带当前页面 cwd。
// 右键菜单可新建终端/编辑器，addPanel 传 renderer: 'always' 保持 PTY 存活。
// 暴露 window.__dockviewApi 供 E2E 测试使用。
// 操作页面切换：saveLayout → store → api.clear() → fromJSON(目标, reuseExistingPanels: true)

import React, { useCallback, useRef } from "react";
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
import { panelRegistry } from "./panelRegistry";
import { saveLayout, loadLayout } from "./layoutSerde";
import { SidebarTree } from "../features/sidebar";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// E2E 测试用全局 API 类型声明
declare global {
  interface Window {
    __dockviewApi?: DockviewApi;
  }
}

const WATERMARK_TEXT = "打开终端或编辑器开始工作";

/** 获取当前活跃页面的 cwd */
function getActivePageCwd(): string | undefined {
  const activePageId = useLayout.getState().activePageId;
  if (!activePageId) return undefined;
  const { projects } = useProjects.getState();
  for (const [, proj] of Object.entries(projects)) {
    const page = proj.pages.find((p) => p.pageId === activePageId);
    if (page?.cwd) return page.cwd;
  }
  return undefined;
}

/** Watermark 空态组件 */
const Watermark: React.FC<IWatermarkPanelProps> = ({ containerApi }) => (
  <div
    style={{
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      justifyContent: "center",
      height: "100%",
      color: "#6C6C6C",
      fontSize: 14,
      userSelect: "none",
      gap: 12,
    }}
  >
    <span>{WATERMARK_TEXT}</span>
    <div style={{ display: "flex", gap: 8 }}>
      <button
        onClick={() => {
          const id = `terminal-${Date.now()}`;
          const cwd = getActivePageCwd();
          containerApi.addPanel({
            id,
            component: "terminal",
            params: { panelId: id, cwd },
            renderer: "always",
          });
        }}
        style={{
          background: "#2D2D2D",
          border: "1px solid #444",
          color: "#ccc",
          cursor: "pointer",
          fontSize: 13,
          padding: "4px 12px",
          borderRadius: 4,
        }}
      >
        新建终端
      </button>
      <button
        onClick={() => {
          const id = `editor-${Date.now()}`;
          const cwd = getActivePageCwd();
          containerApi.addPanel({
            id,
            component: "editor",
            params: { panelId: id, cwd },
          });
        }}
        style={{
          background: "#2D2D2D",
          border: "1px solid #444",
          color: "#ccc",
          cursor: "pointer",
          fontSize: 13,
          padding: "4px 12px",
          borderRadius: 4,
        }}
      >
        新建编辑器
      </button>
    </div>
  </div>
);

/** 顶栏右侧 "+" 按钮 */
const RightHeaderActions: React.FC<IDockviewHeaderActionsProps> = ({
  containerApi,
}) => {
  const handleNewTerminal = () => {
    const id = `terminal-${Date.now()}`;
    const cwd = getActivePageCwd();
    containerApi.addPanel({
      id,
      component: "terminal",
      params: { panelId: id, cwd },
      renderer: "always",
    });
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        height: "100%",
        paddingRight: 4,
      }}
    >
      <button
        onClick={handleNewTerminal}
        style={{
          background: "none",
          border: "1px solid #444",
          color: "#ccc",
          cursor: "pointer",
          fontSize: 16,
          width: 24,
          height: 24,
          borderRadius: 4,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          lineHeight: 1,
        }}
        title="新建终端"
      >
        +
      </button>
    </div>
  );
};

const Workspace: React.FC = () => {
  const isReadyRef = useRef(false);
  const apiRef = useRef<DockviewApi | null>(null);
  /** 脏布局跟踪：页面切换前冲刷到 store */
  const dirtyPageIdRef = useRef<string | null>(null);
  /** fromJSON 恢复守卫：程序化恢复时不触发布局保存 */
  const restoreGuardRef = useRef(false);

  /** 冲刷脏布局到 store */
  const flushDirtyLayout = useCallback(() => {
    const dirtyId = dirtyPageIdRef.current;
    const api = apiRef.current;
    if (!dirtyId || !api) return;
    const layout = saveLayout(api);
    const { projects } = useProjects.getState();
    for (const [, proj] of Object.entries(projects)) {
      if (proj.pages.some((p) => p.pageId === dirtyId)) {
        useProjects.getState().updatePageLayout(
          proj.projectId,
          dirtyId,
          layout as Record<string, unknown>,
        );
        break;
      }
    }
    dirtyPageIdRef.current = null;
  }, []);

  /** 操作页面切换 */
  const switchToPage = useCallback((projectId: string, pageId: string) => {
    const api = apiRef.current;
    if (!api) return;

    const layoutStore = useLayout.getState();
    if (layoutStore.isLayoutSwitching) return;
    // H6: 自切换守卫 — 点击已激活页面无操作
    if (layoutStore.activePageId === pageId) return;

    layoutStore.setLayoutSwitching(true);

    try {
      // 冲刷当前页脏布局
      flushDirtyLayout();

      const projectsStore = useProjects.getState();
      const project = projectsStore.projects[projectId];
      if (!project) return;

      const targetPage = project.pages.find((p) => p.pageId === pageId);
      const targetLayout = targetPage?.layout;

      // B2 修复：fromJSON 内部已调用 clear()，前置 clear 破坏 reuseExistingPanels
      if (targetLayout && Object.keys(targetLayout).length > 0) {
        loadLayout(api, targetLayout);
      } else {
        api.clear(); // 无保存布局时才主动清空
      }

      projectsStore.switchToPage(projectId, pageId);
      layoutStore.setActivePage(pageId);
    } finally {
      layoutStore.setLayoutSwitching(false);
    }
  }, [flushDirtyLayout]);

  const onReady = useCallback(
    (event: { api: DockviewApi }) => {
      if (isReadyRef.current) return;
      isReadyRef.current = true;

      const { api } = event;
      apiRef.current = api;

      window.__dockviewApi = api;

      // H7: 尝试恢复上次活跃页面的布局
      const { activePageId } = useLayout.getState();
      const { projects } = useProjects.getState();
      let restored = false;

      if (activePageId) {
        for (const [, proj] of Object.entries(projects)) {
          const page = proj.pages.find((p) => p.pageId === activePageId);
          if (page?.layout && Object.keys(page.layout).length > 0) {
            loadLayout(api, page.layout);
            restored = true;
            break;
          }
        }
      }

      if (!restored) {
        // 有项目但无保存布局 → 创建默认终端
        if (Object.keys(projects).length > 0) {
          const initPanelId = `terminal-init-${Date.now()}`;
          api.addPanel({
            id: initPanelId,
            component: "terminal",
            params: { panelId: initPanelId },
            renderer: "always",
          });
        }
        // 无项目 → Watermark 自然显示
      }

      // H7: fromJSON 恢复守卫 — 程序化恢复不触发保存
      api.onDidLayoutFromJSON(() => {
        restoreGuardRef.current = true;
        setTimeout(() => {
          restoreGuardRef.current = false;
        }, 0);
      });

      // H7: 布局变更 → 标记脏（延迟写入 store，切换页面前冲刷）
      api.onDidLayoutChange(() => {
        if (restoreGuardRef.current) return;
        const activeId = useLayout.getState().activePageId;
        if (!activeId) return;
        dirtyPageIdRef.current = activeId;
      });
    },
    [],
  );

  const getTabContextMenuItems = useCallback(
    (
      params: GetTabContextMenuItemsParams,
    ): (BuiltInContextMenuItem | ReactContextMenuItemConfig)[] => {
      const { api } = params;

      const newTerminalId = `terminal-${Date.now()}`;
      const newEditorId = `editor-${Date.now()}`;

      return [
        {
          label: "新建终端",
          action: () => {
            api.addPanel({
              id: newTerminalId,
              component: "terminal",
              params: { panelId: newTerminalId },
              renderer: "always",
            });
          },
        },
        {
          label: "新建编辑器",
          action: () => {
            api.addPanel({
              id: newEditorId,
              component: "editor",
              params: { panelId: newEditorId },
            });
          },
        },
        "separator",
        "close",
        "closeOthers",
        "closeAll",
      ];
    },
    [],
  );

  return (
    <div style={{ display: "flex", width: "100%", height: "100%" }}>
      <SidebarTree switchToPage={switchToPage} />
      <div style={{ flex: 1, minWidth: 0, height: "100%" }}>
        <DockviewReact
          className="dockview-theme-dark"
          components={panelRegistry}
          onReady={onReady}
          watermarkComponent={Watermark}
          defaultTabComponent={DefaultTab}
          rightHeaderActionsComponent={RightHeaderActions}
          getTabContextMenuItems={getTabContextMenuItems}
        />
      </div>
    </div>
  );
};

const DefaultTab: React.FC<IDockviewPanelProps> = (props) => {
  const { api } = props;
  const title = api.title || api.component || "";

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation();
    api.close();
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        height: "100%",
        padding: "0 8px",
        gap: 6,
        userSelect: "none",
      }}
    >
      <span style={{ fontSize: 13 }}>{title}</span>
      <button
        onClick={handleClose}
        style={{
          background: "none",
          border: "none",
          color: "#808080",
          cursor: "pointer",
          padding: "0 2px",
          fontSize: 14,
          lineHeight: 1,
        }}
        title="关闭"
      >
        ×
      </button>
    </div>
  );
};

export default Workspace;
