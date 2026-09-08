// Workspace — 共享 Dockview 宿主编排层（CP-004/S11 改造）
//
// 多实例架构消亡：本组件只保留编排（Allotment 三栏 + 页面切换/删除编排），
// Dockview 渲染收敛单一 WorkspaceDockHost（共享宿主）——每个操作页面 = 宿主内
// 一个顶级页组（pageGroups.ts 协议），页面切换 = 页组容器显隐（宿主可见性
// 单点，xterm 只 open 一次，#4978 约束不变——见 WorkspaceDockHost.tsx 头注）。
//
// 页面总数上限随多实例消亡（旧上限常量已删除——实例数不再随页线性增长，
// 容器/渲染管线共享，见 stores/CLAUDE.md）。
//
// F2/onReady 回调稳定化、FE-33 惰性回调 map、页面惰性初始化均随
// 多实例架构消亡——宿主 onReady 只发生一次（WorkspaceDockHost 内部）。

import React, { useCallback, useEffect, useRef } from "react";
import { Allotment } from "allotment";
import "allotment/dist/style.css";
import WorkspaceDockHost from "./WorkspaceDockHost";
import { titleManager } from "./titleManager";
import { switchToPageShared, getHostApi } from "./pageApis";
import { pageGroupId } from "./pageGroups";
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
import { E2E_ENABLED, toast } from "../lib";
import { setProjectRoot } from "../ipc/fs";
import { startWatch, stopWatch } from "../ipc/notify";
import { markWorkspaceReady } from "../../e2e-tests/helpers";

/** Allotment 主区最小宽度（px）。活动栏与侧栏区尺寸常量来自 ../features/sideViews */
const MAIN_MIN_SIZE = 200;

// ---- Workspace 主组件 ----

const Workspace: React.FC = () => {
  // E2E 测试就绪信号：Workspace 挂载后立即可见（渲染阶段同步设置，非 useEffect）
  if (E2E_ENABLED) {
    markWorkspaceReady();
  }

  const activePageId = useLayout((s) => s.activePageId);
  const sideOpen = useSideBar((s) => s.open);
  const sideWidth = useSideBar((s) => s.width);
  const setSideWidth = useSideBar((s) => s.setWidth);
  const anyOpen = deriveLayout(sideOpen) !== "hidden";

  /**
   * 操作页面切换（仅更新 activePageId——页组容器显隐由宿主订阅刷新；
   * projectId 参数保留兼容侧栏视图接口——NAV-05 三槽）。
   * setProjectRoot → setActivePage 委托 switchToPageShared（store 层编排单点）。
   */
  const switchToPage = useCallback(async (_projectId: string, pageId: string) => {
    await switchToPageShared(pageId);
  }, []);

  /** 删除操作页面：先 kill 页组内全部终端（组移除 → 面板卸载链 → PTY kill），
   *  再移除页组，最后清 store 与标题管理状态（页面生命周期契约——页面删除
   *  即放弃页上一切，不做面板级 dirty 确认，CP-036 收尾复核结论不变） */
  const onDeletePage = useCallback((projectId: string, pageId: string) => {
    const layoutStore = useLayout.getState();
    const isActive = layoutStore.activePageId === pageId;

    // 先移除宿主页组（dockview removeGroup → onDidRemovePanel → 面板卸载 →
    // TerminalPanel cleanup → PTY kill；P2-49: dockview api 内部自动清理监听器）
    const api = getHostApi();
    const group = api?.getGroup(pageGroupId(pageId));
    if (api && group) {
      try {
        api.removeGroup(group);
      } catch (err) {
        console.error(`[slTerminal] 删除页面 ${pageId} 页组失败:`, err);
      }
    }

    // 清理标题管理器状态（registry + counters）
    titleManager.onDeletePage(pageId);

    // 从 store 移除（removePage 内部处理 project.activePageId 转移）
    useProjects.getState().removePage(projectId, pageId);

    // 活跃页转移（layout store——宿主订阅刷新可见性）
    if (isActive) {
      layoutStore.setActivePage(null);
      const nextPageId = useProjects.getState().projects[projectId]?.activePageId;
      if (nextPageId) layoutStore.setActivePage(nextPageId);
    }
  }, []);

  // SEC-01: 活动项目变化时同步项目根路径到后端（路径沙箱边界）
  // 文件监听（fs-event）跟随项目激活——宿主从 ExplorerPanel 上提到本项目激活层：
  // 编辑器外部修改 reload / commit 面板刷新等消费方依赖 fs-event，
  // 不依赖 explorer 视图是否打开（E2E editor auto-reload 失败根因修复）。
  // ExplorerPanel 不再管理 watcher（防双管理互停），统一由本 effect 单点负责。
  const prevRootRef = useRef<string | null>(null);
  useEffect(() => {
    // BE-10：activePageId 置 null（删除末页/移除活跃项目）→ 停掉旧项目 watcher，
    // 防 OS 句柄残留至 LRU 淘汰（两条置 null 链：onDeletePage 删末页、NavTree removeProject）
    if (!activePageId) {
      if (prevRootRef.current) {
        void stopWatch(prevRootRef.current);
        prevRootRef.current = null;
      }
      return;
    }
    // 从当前快照推导活跃项目的 rootPath（避免以 projects 为 deps 导致频繁触发）
    const { projects: currentProjects } = useProjects.getState();
    for (const [, proj] of Object.entries(currentProjects)) {
      if (proj.pages.some((p) => p.pageId === activePageId)) {
        if (proj.rootPath && proj.rootPath !== prevRootRef.current) {
          const prev = prevRootRef.current;
          prevRootRef.current = proj.rootPath;
          if (prev) void stopWatch(prev);
          // FE-38：setProjectRoot 成功后才 startWatch（失败不启动 watcher）；
          // 过期守卫：then 回调时 prevRootRef 已指向其他项目（快速连切）→ 丢弃
          const targetRoot = proj.rootPath;
          setProjectRoot(targetRoot)
            .then(() => {
              if (prevRootRef.current !== targetRoot) return;
              // CP-029(S03): startWatch 失败不再静默（曾无 .catch → unhandled
              // rejection，E2E 假项目根等场景 watcher 缺失无任何可见信号，
              // 干扰 auto-reload 类问题定责）；错误可视化，不阻断切换
              void startWatch(targetRoot).catch((err: unknown) => {
                console.error("[slTerminal] 文件监听启动失败:", String(err));
              });
            })
            .catch((err) => {
              console.error("[slTerminal] 设置项目根路径失败:", err);
              // FE-04（D7）：SEC-01 兜底失败时 toast 告警，不阻断切换
              toast.show("warning", "项目根路径设置失败，文件操作可能被拒绝");
            });
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
        {/* pane3: 主区 — 共享 Dockview 宿主（全部操作页页组） */}
        <Allotment.Pane minSize={MAIN_MIN_SIZE}>
          <div style={{ width: "100%", height: "100%", position: "relative" }}>
            <WorkspaceDockHost />
          </div>
        </Allotment.Pane>
      </Allotment>
    </div>
  );
};

export default Workspace;

// 向后兼容：测试从 Workspace.tsx 导入 createRightHeader / createTabMenuItems / applyRename
export { createRightHeader, createTabMenuItems, applyRename } from "./PageDockviewHost";
