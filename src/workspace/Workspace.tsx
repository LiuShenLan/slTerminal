// Workspace — 多 Dockview 实例架构编排层
//
// 每个操作页面拥有独立 Dockview 实例，页面切换通过 CSS display:none/block 实现。
// xterm.js 不支持二次 open()（Issue #4978），此架构从根本上解决 H6（终端跨页面存活）。
//
// PageDockview 逻辑已提取到 PageDockviewHost.tsx（J4），本文件只保留编排层：
// Allotment 三栏布局 + 页面切换 + 生命周期管理。
//
// F2: onReady/onLayoutChange 稳定化——通过 ref 持有的回调 map，同一 pageId 始终返回
//     同一函数引用，配合 PageDockview 的 React.memo 避免不必要的重渲染。
//
// 约束：#7 布局单点 — 每个 PageDockview 的 onDidLayoutChange 直接写 store
//       #8 会话单点 — 终端会话只在面板内管理，不跨页面

import React, { useCallback, useRef, useState, useMemo, useEffect } from "react";
import { type DockviewApi } from "dockview-react";
import { Allotment } from "allotment";
import "allotment/dist/style.css";
import PageDockview from "./PageDockviewHost";
import { titleManager } from "./titleManager";
import { SidebarTree } from "../features/sidebar";
import { ExplorerPanel } from "../features/explorer";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { ErrorBoundary } from "../lib";
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
    const api = pageApiMapRef.current.get(pageId);
    if (api) {
      api.clear();
      api.dispose();
      pageApiMapRef.current.delete(pageId);
    }

    // 清理标题管理器状态（registry + counters）
    titleManager.onDeletePage(pageId);

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

  /** PageDockview onReady: 注册 API（稳定引用，deps=[]） */
  const handlePageApiReady = useCallback((pageId: string, api: DockviewApi) => {
    pageApiMapRef.current.set(pageId, api);
    if (pageId === useLayout.getState().activePageId) {
      window.__dockviewApi = api;
    }
  }, []);

  /** PageDockview 布局变更: 写入 store（稳定引用，deps=[]） */
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

  // F2: 通过 ref 持有最新的 handler，使 PageDockview 的 onReady/onLayoutChange 回调引用稳定
  const handlePageApiReadyRef = useRef(handlePageApiReady);
  handlePageApiReadyRef.current = handlePageApiReady;
  const handlePageLayoutChangeRef = useRef(handlePageLayoutChange);
  handlePageLayoutChangeRef.current = handlePageLayoutChange;

  // F2: 稳定回调 map——同一 pageId 始终返回同一函数引用。
  // 惰性创建，pageId 生命周期内引用不变；页面删除时清理对应条目。
  const pageCallbacksRef = useRef<Map<string, {
    onReady: (api: DockviewApi) => void;
    onLayoutChange: (layout: Record<string, unknown>) => void;
  }>>(new Map());

  // 清理已删除页面的回调 + 确保当前页面回调存在
  const activePageIds = new Set(allPages.map((p) => p.pageId));
  for (const key of pageCallbacksRef.current.keys()) {
    if (!activePageIds.has(key)) {
      pageCallbacksRef.current.delete(key);
    }
  }
  for (const page of allPages) {
    if (!pageCallbacksRef.current.has(page.pageId)) {
      pageCallbacksRef.current.set(page.pageId, {
        onReady: (api: DockviewApi) => handlePageApiReadyRef.current(page.pageId, api),
        onLayoutChange: (layout: Record<string, unknown>) =>
          handlePageLayoutChangeRef.current(page.pageId, layout),
      });
    }
  }

  // E2E 兼容：activePageId 变化时自动初始化对应页面（Workspace 挂载后生效）
  useEffect(() => {
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
            {allPages.map((page) => {
              const callbacks = pageCallbacksRef.current.get(page.pageId);
              return initializedPages.has(page.pageId) && callbacks ? (
                <ErrorBoundary key={page.pageId} variant="inline">
                  <PageDockview
                    pageId={page.pageId}
                    cwd={page.cwd}
                    rootPath={page.rootPath}
                    savedLayout={page.layout}
                    visible={page.pageId === activePageId}
                    onReady={callbacks.onReady}
                    onLayoutChange={callbacks.onLayoutChange}
                  />
                </ErrorBoundary>
              ) : null;
            })}
          </div>
        </Allotment.Pane>
      </Allotment>
    </div>
  );
};

export default Workspace;

// 向后兼容：测试从 Workspace.tsx 导入 createRightHeader / createGetContextMenu
export { createRightHeader, createGetContextMenu } from "./PageDockviewHost";
