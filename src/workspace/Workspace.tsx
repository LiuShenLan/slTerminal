// Workspace — 多 Dockview 实例架构编排层
//
// 每个操作页面拥有独立 Dockview 实例，页面切换通过 CSS display:none/block 实现。
// xterm.js 不支持二次 open()（Issue #4978），此架构从根本上解决 H6（终端跨页面存活）。
//
// FE-01（D1 决策）：保持多 Dockview 实例（架构豁免登记，见 docs/review-fix/stages.md S19）——
// 页面总数上限 MAX_PAGES = 20 在 stores/projects.ts 的 addPage 拒绝超限新增（防内存/DOM
// 线性增长）；本组件不主动限制，超限由 store 层 toast 告警。
//
// PageDockview 逻辑已提取到 PageDockviewHost.tsx（J4），本文件只保留编排层：
// Allotment 三栏布局 + 页面切换 + 生命周期管理。
//
// F2: onReady/onLayoutChange 稳定化——通过 ref 持有的回调 map，同一 pageId 始终返回
//     同一函数引用，配合 PageDockview 的 React.memo 避免不必要的重渲染。
//
// FE-33: 回调 map 按 pageId 惰性创建 + 缓存（getOrCreate 模式），effect 依赖收窄为
//     页面 ID 集合键——页面重命名/布局变更等不触发重建（详见 pageCallbacksRef 处注释）。
//
// 约束：#7 布局单点 — 每个 PageDockview 的 onDidLayoutChange 直接写 store
//       #8 会话单点 — 终端会话只在面板内管理，不跨页面

import React, { useCallback, useRef, useState, useMemo, useEffect } from "react";
import { type DockviewApi } from "dockview-react";
import { Allotment } from "allotment";
import "allotment/dist/style.css";
import PageDockview from "./PageDockviewHost";
import { titleManager } from "./titleManager";
import {
  registerPageApi,
  unregisterPageApi,
  getPageApi,
  switchToPageShared,
} from "./pageApis";
// 侧栏视图 + CLI profile：side-effect 注册（静态 import 链保证 App init 的 loadFromDisk 运行时注册已完成）
import "../features/sideViews/sideViewDefs";
// CLI profile 注册触发点（D-07）：side-effect import 使 claude profile 在任何消费方使用前完成注册（照 sideViewDefs/schemes 先例）
import "../features/cliProfiles/profiles";
import {
  ActivityBar,
  SideBarArea,
  ACTIVITY_BAR_SIZE,
  WIDTH_MIN,
  WIDTH_MAX,
  deriveLayout,
} from "../features/sideViews";
import { useSideBar } from "../stores/sideBar";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { allotmentVarStyle } from "../theme";
import { ErrorBoundary, E2E_ENABLED, toast } from "../lib";
import { setProjectRoot } from "../ipc/fs";
import { startWatch, stopWatch } from "../ipc/notify";
import { markWorkspaceReady } from "../../e2e-tests/helpers";

declare global {
  interface Window {
    __dockviewApi?: DockviewApi;
  }
}

/** Allotment 主区最小宽度（px）。活动栏与侧栏区尺寸常量来自 ../features/sideViews */
const MAIN_MIN_SIZE = 200;

// ---- Workspace 主组件 ----

const Workspace: React.FC = () => {
  // E2E 测试就绪信号：Workspace 挂载后立即可见（渲染阶段同步设置，非 useEffect）
  if (E2E_ENABLED) {
    markWorkspaceReady();
  }

  const [initializedPages, setInitializedPages] = useState<Set<string>>(new Set());

  const activePageId = useLayout((s) => s.activePageId);
  const projects = useProjects((s) => s.projects);
  const sideOpen = useSideBar((s) => s.open);
  const sideWidth = useSideBar((s) => s.width);
  const setSideWidth = useSideBar((s) => s.setWidth);
  const anyOpen = deriveLayout(sideOpen) !== "hidden";

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

  /** 操作页面切换（仅更新 activePageId + CSS 显隐；projectId 参数保留兼容侧栏视图接口——NAV-05 三槽）
   *
   * ensurePageInitialized 依赖组件 setState，不下放 pageApis。
   * 其余逻辑（setProjectRoot → setActivePage → __dockviewApi 重指向）委托 switchToPageShared。 */
  const switchToPage = useCallback(async (_projectId: string, pageId: string) => {
    ensurePageInitialized(pageId);
    await switchToPageShared(pageId);
  }, [ensurePageInitialized]);

  /** 删除操作页面 */
  const onDeletePage = useCallback((projectId: string, pageId: string) => {
    const layoutStore = useLayout.getState();
    const isActive = layoutStore.activePageId === pageId;

    // 销毁该页面的 Dockview（触发面板卸载 → useXterm cleanup → PTY kill）
    // P2-49: dockview-react api.dispose() 内部自动清理所有事件监听器
    const api = getPageApi(pageId);
    if (api) {
      api.clear();
      api.dispose();
      unregisterPageApi(pageId);
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
        const nextApi = getPageApi(nextPageId);
        if (nextApi) window.__dockviewApi = nextApi;
      }
    }
  }, [ensurePageInitialized]);

  /** PageDockview onReady: 注册 API（稳定引用，deps=[]） */
  const handlePageApiReady = useCallback((pageId: string, api: DockviewApi) => {
    registerPageApi(pageId, api);
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

  // FE-33: 回调按 pageId 惰性创建 + 缓存（getOrCreate 模式）——已存在则返回缓存引用，
  // 仅缺失时创建，页面生命周期内引用不变（与 F2 稳定引用目标一致）。
  const getOrCreatePageCallbacks = useCallback((pageId: string) => {
    let callbacks = pageCallbacksRef.current.get(pageId);
    if (!callbacks) {
      callbacks = {
        onReady: (api: DockviewApi) => handlePageApiReadyRef.current(pageId, api),
        onLayoutChange: (layout: Record<string, unknown>) =>
          handlePageLayoutChangeRef.current(pageId, layout),
      };
      pageCallbacksRef.current.set(pageId, callbacks);
    }
    return callbacks;
  }, []);

  // FE-33: effect 依赖收窄——从 allPages（对象引用，任何页面字段变更即变）收窄为
  // 页面 ID 集合键（primitive string）：页面重命名/布局变更等不改 ID 集合时不触发
  // effect，回调 map 不再随每次 allPages 变化重建。渲染期 ref 仍只读（FE-03 不变量）——
  // 创建与删除均收敛于此 effect，经 getOrCreate 惰性创建 + 缓存。
  const pageIdSetKey = useMemo(
    () => allPages.map((p) => p.pageId).sort().join("|"),
    [allPages],
  );

  useEffect(() => {
    const activePageIds = new Set(
      pageIdSetKey === "" ? [] : pageIdSetKey.split("|"),
    );
    // 清理已删除页面的回调（getOrCreate 只增不删，删除全量收敛于此）
    for (const key of pageCallbacksRef.current.keys()) {
      if (!activePageIds.has(key)) {
        pageCallbacksRef.current.delete(key);
      }
    }
    // 确保当前页面回调存在（getOrCreate 幂等：已存在返回缓存引用，缺失才创建）
    for (const pageId of activePageIds) {
      getOrCreatePageCallbacks(pageId);
    }
  }, [pageIdSetKey, getOrCreatePageCallbacks]);

  // E2E 兼容：activePageId 变化时自动初始化对应页面（Workspace 挂载后生效）
  useEffect(() => {
    if (activePageId) ensurePageInitialized(activePageId);
  }, [activePageId, ensurePageInitialized]);

  // SEC-01: 活动项目变化时同步项目根路径到后端（路径沙箱边界）
  // 文件监听（fs-event）跟随项目激活——宿主从 ExplorerPanel 上提到本项目激活层：
  // 编辑器外部修改 reload / commit 面板刷新等消费方依赖 fs-event，
  // 不依赖 explorer 视图是否打开（E2E editor auto-reload 失败根因修复）。
  // ExplorerPanel 不再管理 watcher（防双管理互停），统一由本 effect 单点负责。
  const prevRootRef = useRef<string | null>(null);
  useEffect(() => {
    if (!activePageId) return;
    // 从当前快照推导活跃项目的 rootPath（避免以 projects 为 deps 导致频繁触发）
    const { projects: currentProjects } = useProjects.getState();
    for (const [, proj] of Object.entries(currentProjects)) {
      if (proj.pages.some((p) => p.pageId === activePageId)) {
        if (proj.rootPath && proj.rootPath !== prevRootRef.current) {
          const prev = prevRootRef.current;
          prevRootRef.current = proj.rootPath;
          if (prev) void stopWatch(prev);
          setProjectRoot(proj.rootPath).catch((err) => {
            console.error("[slTerminal] 设置项目根路径失败:", err);
            // FE-04（D7）：SEC-01 兜底失败时 toast 告警，不阻断切换
            toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝");
          });
          void startWatch(proj.rootPath);
        }
        break;
      }
    }
  }, [activePageId]);

  return (
    <div style={{
      // allotment CSS 变量（--separator-border / --focus-border，active 方案
      // libraries.allotment）——根容器注入，CSS 变量继承天然覆盖外层与本文件
      // 内层 SideBarArea 两处 Allotment（SideBarArea 不改）
      ...allotmentVarStyle(),
      width: "100%", height: "100%",
    }}>
      <Allotment onChange={(sizes) => {
        // 侧栏区可见时同步宽度到 store（setWidth 内部 clamp，无需重复校验）
        if (anyOpen && sizes.length >= 2) {
          setSideWidth(sizes[1]);
        }
      }}>
        {/* pane1: 活动栏 — 40px 固定 */}
        <Allotment.Pane preferredSize={ACTIVITY_BAR_SIZE} minSize={ACTIVITY_BAR_SIZE} maxSize={ACTIVITY_BAR_SIZE}>
          <ActivityBar />
        </Allotment.Pane>
        {/* pane2: 侧栏区 — 可显隐，宽度持久化 */}
        <Allotment.Pane preferredSize={sideWidth} minSize={WIDTH_MIN} maxSize={WIDTH_MAX} visible={anyOpen}>
          <SideBarArea switchToPage={switchToPage} onDeletePage={onDeletePage} />
        </Allotment.Pane>
        {/* pane3: 主区 — 不变 */}
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
export { createRightHeader, createGetContextMenu, applyRename } from "./PageDockviewHost";
