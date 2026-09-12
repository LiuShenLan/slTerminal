// WorkspaceDockHost — 共享 Dockview 宿主（CP-004/S11 核心：单一 DockviewReact）
//
// 多实例架构消亡后的替代形态：
// - 宿主渲染唯一 `<DockviewReact>`；每个操作页面 = 宿主内一组网格叶组
//   （主组 id = page-{pageId}；ADR-0020 起支持页内分屏——单页多组，组页归属
//   经 pageIdOfGroup 派生，不由组 id 断言）。
// - 叶组显隐（页面切换）：dockview 叶级可见性——仅活跃页所属各组可见
//   （setActivePageVisibility 单点：隐藏叶 DOM 保留、面板不卸载、xterm 只
//   open 一次，#4978 约束不变；fit/resize 仅在组可见时执行——隐藏组
//   display:none 尺寸归零，ResizeObserver 不触发，切回后 dockview 重排 →
//   观测器自然恢复）。仅网格分屏——disableFloatingGroups 禁 floating，
//   popout 不调用（D7）。
// - 生命周期契约：新增面板经 resolvePageGroupForAdd 显式落组；组归属审计
//   （auditGroupMembership：onDidMovePanel/onDidAddPanel 事件源 + 恢复后全量，
//   stray 空壳组清理 + 跨页混组少数派回迁——dockview `_moving` 门控吞移动期
//   onDidAddPanel，守卫主事件源必须是 onDidMovePanel）；页面删除先经组移除
//   （面板卸载链 kill PTY）；restoreGuardRef 恢复守卫语义不变（程序化恢复期间
//   的布局事件不写回、审计跳过）。
// - 布局单点：#7 —— 变更经 saveLayout/slice 写回 store；恢复经 composeHostLayout
//   /loadPageGroup（layoutSerde.ts，页子树切片——单页多组随切片持久化）。

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  DockviewReact,
  type DockviewApi,
  type DockviewGroupPanel,
  type IDockviewPanel,
} from "dockview-react";
import { panelRegistry } from "../panelRegistry";
import {
  createWatermark,
  createRightHeader,
  DefaultTab,
  TabMenuPopup,
  TerminalRenameDialog,
  applyRename,
  createTabMenuItems,
  rebuildAndRecomputeTitles,
  TAB_CONTEXT_MENU_EVENT,
  hostContainerStyle,
  type TabContextMenuDetail,
  type TabMenuPanel,
} from "./tabChrome";
import type { TabMenuItem } from "./TabMenuPopup";
import { composeHostLayout, loadLayout, loadPageGroup } from "./layoutSerde";
import {
  registerHostApi,
  unregisterHostApi,
  markPageGroupMounted,
  unregisterPageGroup,
  syncHostLayoutToStore,
  projectRootOfPage,
} from "./pageApis";
import { pageIdOfGroupId, pageOfPanelId, pageIdOfGroup, groupsOfPage, panelsOfPage, resolvePageGroupForAdd } from "./pageGroups";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { titleManager } from "./titleManager";
import { advanceTerminalPanelSeq } from "../lib/panelId";

/** 宿主容器 E2E 锚点类名（dockview 根 DOM 定位用） */
export const DOCK_HOST_CLASS = "slterm-dock-host";

// 无 props——可见性由宿主自订阅：活跃页缺失（删除末页/项目移除）时整宿主
// display:none（旧「页面实例全隐藏」的空白主区语义——terminal 不卸载不销毁）；
// hostReady 前保持可见（dockview 初始化需已布局容器）
export type WorkspaceDockHostProps = Record<string, never>;

/**
 * 叶组可见性单点（ADR-0020；页面切换/恢复/增删页后调用）：遍历宿主全部
 * 网格叶组，按派生归属 setVisible(属主 === 活跃页)——活跃页多组（页内分屏）
 * 同显、其余页各组同隐（隐藏叶 DOM 保留面板不卸载）。非网格组跳过（D7 禁用
 * floating/popout，防御分支）。无活跃页 → 全隐（宿主由外层一并隐藏）。
 */
export function setActivePageVisibility(
  api: DockviewApi,
  activePageId: string | null,
): void {
  for (const group of api.groups) {
    if (group.api.location.type !== "grid") continue;
    const owner = pageIdOfGroup(group);
    const visible = activePageId !== null && owner === activePageId;
    // 等值跳过（红线）：dockview setVisible 无条件 fire onDidLayoutChange——
    // 不跳过则 sync()→setVisible→layoutChange→store 写回→sync() 死循环
    if (group.api.isVisible === visible) continue;
    try {
      group.api.setVisible(visible);
    } catch (err) {
      console.error(`[slTerminal] 组可见性设置失败(${group.id}):`, err);
    }
  }
}

/** 审计重入旗标——audit 内 moveTo 同步触发 onDidMovePanel 重入，幂等空扫直接退出 */
let auditingMembership = false;

/**
 * 组归属审计（ADR-0020；导出供 L2 直测——onDidMovePanel/onDidAddPanel 事件源
 * + 恢复后全量一次）：
 * - 主组（page- 前缀 id）：组内他页面板 moveTo 回各自页组（空主组 = Watermark
 *   载体，不删）；
 * - 自生组（分屏产物）：空壳 → removeGroup；混组以首面板页为属主，少数派
 *   moveTo 回各自页组（moveTo 后源组走空由 dockview 自清 + 下轮审计兜底）。
 * restoreGuard 期间由调用方跳过（程序化恢复产物已经剪枝合法）。
 */
export function auditGroupMembership(api: DockviewApi): void {
  if (auditingMembership) return;
  auditingMembership = true;
  try {
    for (const group of [...api.groups]) {
      const groupPageId = pageIdOfGroupId(group.id);
      if (groupPageId !== null) {
        // 主组：他页面板回迁（主组 id 即属主，面板页前缀不一致即越界）
        for (const panel of [...group.panels]) {
          if (pageOfPanelId(panel.id) !== groupPageId) {
            movePanelToPageGroup(api, panel, pageOfPanelId(panel.id));
          }
        }
        continue;
      }
      // 自生组：空壳清理
      if (group.panels.length === 0) {
        try {
          api.removeGroup(group);
        } catch (err) {
          console.error(`[slTerminal] 空壳组 ${group.id} 清理失败:`, err);
        }
        continue;
      }
      // 自生组：首面板页为属主，少数派回迁
      const owner = pageOfPanelId(group.panels[0].id);
      for (const panel of [...group.panels]) {
        if (pageOfPanelId(panel.id) !== owner) {
          movePanelToPageGroup(api, panel, pageOfPanelId(panel.id));
        }
      }
    }
  } finally {
    auditingMembership = false;
  }
}

/** 面板回迁所属页（resolvePageGroupForAdd 目标组；页已无组 → 滞留现组由删页路径收口） */
function movePanelToPageGroup(
  api: DockviewApi,
  panel: IDockviewPanel,
  pageId: string | null,
): void {
  if (!pageId) return; // 无页归属面板（理论不可达——旧布局已迁移）保留现组
  const home = resolvePageGroupForAdd(api, pageId);
  if (!home || panel.group === home) return;
  console.warn(`[slTerminal] 跨页组面板回迁:${panel.id} → 页面 ${pageId} 组`);
  try {
    // moveTo 回迁（dockview 面板级移动原语——含焦点/事件一致处理）
    panel.api.moveTo({ group: home as unknown as DockviewGroupPanel });
  } catch (err) {
    console.error(`[slTerminal] 面板回迁失败(${panel.id}):`, err);
  }
}

const WorkspaceDockHost: React.FC<WorkspaceDockHostProps> = () => {
  const apiRef = useRef<DockviewApi | null>(null);
  /** FE-09: handleReady 内构建的 disposables 暂存——组件卸载时统一消费（此前无消费路径） */
  const disposablesRef = useRef<Array<{ dispose(): void }>>([]);
  /** 恢复守卫——程序化 fromJSON 期间 onDidLayoutChange 不写回 store（语义沿革
   *  同旧 PageDockview.restoreGuardRef） */
  const restoreGuardRef = useRef(false);
  /** 宿主已注册（onReady 完成初始恢复一次） */
  const [hostReady, setHostReady] = useState(false);
  /** 页签右键菜单态（单宿主单点持有——随宿主渲染） */
  const [tabMenu, setTabMenu] = useState<{ x: number; y: number; items: TabMenuItem[] } | null>(null);
  /** 重命名弹窗目标（右键菜单「重命名」→ setRenameTarget） */
  const [renameTarget, setRenameTarget] = useState<{ panel: TabMenuPanel; initialTitle: string } | null>(null);
  const renameTargetRef = useRef(renameTarget);
  renameTargetRef.current = renameTarget;

  const activePageId = useLayout((s) => s.activePageId);
  // 菜单随页切换清空（旧多实例 visible effect 语义）
  const prevActiveRef = useRef(activePageId);
  useEffect(() => {
    if (prevActiveRef.current !== activePageId) {
      prevActiveRef.current = activePageId;
      setTabMenu(null);
    }
  }, [activePageId]);

  // FE-49: 页签条防 autoscroll 捕获监听（旧每页容器 capture 单点 → 宿主容器单点）
  const containerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onMiddleDown = (e: MouseEvent) => {
      if (e.button !== 1) return;
      if ((e.target as Element | null)?.closest?.(".dv-tabs-and-actions-container")) {
        e.preventDefault();
      }
    };
    el.addEventListener("mousedown", onMiddleDown, { capture: true });
    return () => el.removeEventListener("mousedown", onMiddleDown, { capture: true });
  }, []);

  /** 宿主 API 引用注入（菜单/水印/组头 action 现取） */
  const getApi = useCallback(() => apiRef.current, []);

  // 水印/组头（单宿主构造一次，组件引用稳定——dockview 不重建）
  const Watermark = useMemo(() => createWatermark(getApi), [getApi]);
  const RightHeader = useMemo(() => createRightHeader(getApi), [getApi]);

  const openRenameDialog = useCallback((panel: TabMenuPanel) => {
    const p = panel.params as { customTitle?: string } | undefined;
    setRenameTarget({ panel, initialTitle: p?.customTitle ?? panel.title ?? "" });
  }, []);

  const handleRenameConfirm = useCallback((newTitle: string) => {
    const target = renameTargetRef.current;
    const api = apiRef.current;
    if (target && api) {
      applyRename(api, target.panel, newTitle, () => syncHostLayoutToStore(api));
      setRenameTarget(null);
    }
  }, []);

  const closeTabMenu = useCallback(() => setTabMenu(null), []);

  /** 页布局恢复后重建（advance seq + 编辑器注册 + 标题重算）——逐页调用 */
  const rebuildPageAfterRestore = useCallback((api: DockviewApi, pageId: string) => {
    const ids = panelsOfPage(api, pageId).map((p) => p.id);
    advanceTerminalPanelSeq(pageId, ids);
    rebuildAndRecomputeTitles(api, pageId, projectRootOfPage(pageId) ?? undefined, ids);
  }, []);

  /**
   * 宿主 onReady——单一注册点：
   * 1. 注册宿主 API + __dockviewApi（宿主唯一，收敛重指不变量）；
   * 2. 订阅生命周期事件（恢复守卫/布局写回/面板移除清理/页组增删/越界守卫）；
   * 3. 初始恢复：composeHostLayout（全部页切片）→ fromJSON → 逐页重建
   *    → 活跃页组可见性。
   */
  const handleReady = useCallback((event: { api: DockviewApi }) => {
    const api = event.api;
    apiRef.current = api;
    registerHostApi(api);
    // E2E 兼容：__dockviewApi 恒指向宿主（旧「活跃页实例重指」收敛——宿主唯一）
    window.__dockviewApi = api;

    // fromJSON 恢复守卫——程序化恢复不触发布局保存（onDidLayoutFromJSON 事件置位）
    const disposables: Array<{ dispose(): void }> = [];
    disposables.push(
      api.onDidLayoutFromJSON(() => {
        restoreGuardRef.current = true;
        setTimeout(() => { restoreGuardRef.current = false; }, 0);
      }),
    );

    // 布局变更 → 切片写回 store（硬约束 #7：saveLayout 单点 + 每页切片）
    disposables.push(
      api.onDidLayoutChange(() => {
        if (restoreGuardRef.current) return;
        syncHostLayoutToStore(api);
      }),
    );

    // 面板关闭 → 注销编辑器 + 重算同页剩余标题（旧实例级语义平移宿主）
    disposables.push(
      api.onDidRemovePanel((panel) => {
        const pageId = pageOfPanelId(panel.id);
        if (!pageId) return;
        const params = panel.params as { panelId?: string } | undefined;
        if (params?.panelId) titleManager.unregisterEditor(pageId, params.panelId);
        const rootPath = projectRootOfPage(pageId);
        if (rootPath) {
          const updates = titleManager.recomputeTitles(pageId, rootPath);
          for (const { panelId, title } of updates) {
            const p = api.getPanel(panelId);
            if (p) p.api.setTitle(title);
          }
        }
      }),
    );

    // 组归属审计（ADR-0020）：
    // - onDidMovePanel = 拖拽守卫主事件源（dockview `_moving` 门控吞移动期
    //   onDidAddPanel/onDidRemovePanel，onDidMovePanel 在 movingLock 外触发）；
    //   移动后补一次可见性收敛（新分屏组 dockview 默认可见，本调用幂等兜底）。
    // - onDidAddPanel = addPanel 落组兜底（E2E 裸 addPanel 无 position 落活跃组
    //   等路径防御）。
    // 红线（真实环境实证，2026-09-12）：两事件回调内**禁止同步** audit/可见性
    // 读写——onDidAddPanel 在 addPanel 流程中途 fire、onDidMovePanel 在 moveTo
    // 链路（含空组自动删除）落定前 fire，此时 audit 的 moveTo/setVisible 撞
    // dockview 已 dispose 中间态资源（"resource is already disposed"）。一律
    // 延迟到宏任务后执行；restoreGuard 在回调执行时点复查（恢复期间入队的
    // 回调执行时守卫仍在——复位 setTimeout 排后）。
    const scheduleMembershipAudit = () => {
      setTimeout(() => {
        if (restoreGuardRef.current) return;
        auditGroupMembership(api);
        setActivePageVisibility(api, useLayout.getState().activePageId);
      }, 0);
    };
    disposables.push(
      api.onDidMovePanel(scheduleMembershipAudit),
      api.onDidAddPanel(scheduleMembershipAudit),
    );

    // 页组增删 → 挂载标记同步（pageGroups 协议 id 过滤——dockview 自生组不标记）
    disposables.push(
      api.onDidAddGroup((group) => {
        const pageId = pageIdOfGroupId(group.id);
        if (pageId !== null) markPageGroupMounted(pageId);
      }),
      api.onDidRemoveGroup((group) => {
        const pageId = pageIdOfGroupId(group.id);
        if (pageId !== null) unregisterPageGroup(pageId);
      }),
    );

    // 宿主卸载清理（页面目录订阅停摆、E2E/全局 API 置空）
    disposables.push({
      dispose: () => {
        apiRef.current = null;
        window.__dockviewApi = undefined;
        unregisterHostApi();
      },
    });

    // FE-09: 暂存本批 disposables——组件级卸载 effect 统一消费
    disposablesRef.current = disposables;

    // 初始恢复：全部页切片汇编 → fromJSON → 逐页重建
    const { projects } = useProjects.getState();
    const pageSlices: Array<{ pageId: string; layout: unknown }> = [];
    for (const [, proj] of Object.entries(projects)) {
      for (const page of proj.pages) {
        pageSlices.push({ pageId: page.pageId, layout: page.layout });
      }
    }
    const hostLayout = composeHostLayout(pageSlices, useLayout.getState().activePageId);
    const restored = loadLayout(api, hostLayout as object);
    if (restored) {
      for (const { pageId } of pageSlices) {
        markPageGroupMounted(pageId);
        rebuildPageAfterRestore(api, pageId);
      }
    } else {
      // 恢复失败 → Watermark 接管空宿主语义；仍标记已挂载（空态可交互）
      for (const { pageId } of pageSlices) markPageGroupMounted(pageId);
      console.error("[slTerminal] 宿主布局恢复失败——空宿主由 Watermark 接管");
    }
    // 恢复守卫窗口结束后统一同步一次切片（fromJSON 规范化可能微调布局——
    // 守卫跳过 onDidLayoutChange 期间写回，此处显式补一次，内容幂等）
    setTimeout(() => {
      // 恢复后全量审计一次（恢复产物经剪枝合法，本调用为防御兜底）
      auditGroupMembership(api);
      syncHostLayoutToStore(api);
      // 活跃页可见性（初始状态同步——fromJSON 后网格平铺态收敛为活跃页各组可见）
      setActivePageVisibility(api, useLayout.getState().activePageId);
    }, 0);

    setHostReady(true);
  }, [rebuildPageAfterRestore]);

  // FE-09: 宿主卸载消费 disposables——事件订阅随 dockview api dispose 自动释放，
  // 本通道的生效点 = 自定义清理（apiRef/__dockviewApi/unregisterHostApi 置空）
  useEffect(
    () => () => {
      for (const d of disposablesRef.current) d.dispose();
      disposablesRef.current = [];
    },
    [],
  );

  // 页面目录（增/删）→ 宿主页组同步——store 订阅（zustand 同步回调——
  // addPage 返回前页组已并入，restoreSession 等随后切页/加面板时序安全）
  useEffect(() => {
    if (!hostReady) return;
    const api = apiRef.current;
    if (!api) return;

    const sync = () => {
      const { projects } = useProjects.getState();
      const desired = new Set<string>();
      for (const [, proj] of Object.entries(projects)) {
        for (const page of proj.pages) desired.add(page.pageId);
      }
      // 宿主现有页集合（派生归属——组事件与订阅解耦，直接查宿主；
      // 自生组经首面板前缀派生，空主组经 id 快车道）
      const present = new Set<string>();
      for (const group of api.groups) {
        const pid = pageIdOfGroup(group);
        if (pid !== null) present.add(pid);
      }
      // 新增页 → 页组并入（loadPageGroup——reuseExistingPanels 保留既有面板实例，
      // 终端不二次 open；fromJSON 期间的布局事件由 onDidLayoutFromJSON 守卫吞掉）
      for (const pageId of desired) {
        if (present.has(pageId)) continue;
        const { projects: projs } = useProjects.getState();
        let layout: unknown = {};
        for (const [, proj] of Object.entries(projs)) {
          const p = proj.pages.find((pg) => pg.pageId === pageId);
          if (p) { layout = p.layout; break; }
        }
        const ok = loadPageGroup(api, pageId, layout);
        if (ok) {
          markPageGroupMounted(pageId);
          rebuildPageAfterRestore(api, pageId);
          // 并入后保证可见性不变量（活跃页各组经 fromJSON 重排后需重新收敛）
          setActivePageVisibility(api, useLayout.getState().activePageId);
        } else {
          console.error(`[slTerminal] 页组 ${pageId} 并入宿主失败`);
        }
      }
      // 删除页（store 移除后组仍残留——如 removeProject 路径）→ 页内全部组
      // 逐组移除兜底（页内分屏多组一并清除）
      for (const pageId of present) {
        if (desired.has(pageId)) continue;
        for (const group of groupsOfPage(api, pageId)) {
          try {
            api.removeGroup(group); // 组移除 → 面板卸载链 → 终端 kill
          } catch (err) {
            console.error(`[slTerminal] 页组 ${pageId} 组 ${group.id} 移除失败:`, err);
          }
        }
        unregisterPageGroup(pageId);
        titleManager.onDeletePage(pageId);
      }
      // 兜底后可见性不变量（删除活跃页组等场景）
      setActivePageVisibility(api, useLayout.getState().activePageId);
    };
    sync();
    return useProjects.subscribe(sync);
  }, [hostReady, rebuildPageAfterRestore]);

  // 活跃页 → 叶组可见性（切页单点；hostReady 后订阅）
  useEffect(() => {
    if (!hostReady) return;
    const api = apiRef.current;
    if (!api) return;
    setActivePageVisibility(api, activePageId);
  }, [hostReady, activePageId]);

  // 页签右键事件监听（单宿主——DefaultTab 广播，宿主解析命中弹菜单）
  const buildTabMenuItems = useMemo(
    () =>
      createTabMenuItems(getApi, null, openRenameDialog, (pid) =>
        pid ? (projectRootOfPage(pid) ?? undefined) : undefined,
      ),
    [getApi, openRenameDialog],
  );
  useEffect(() => {
    const onTabContextMenu = (e: Event) => {
      const detail = (e as CustomEvent<TabContextMenuDetail>).detail;
      const api = apiRef.current;
      if (!api || !detail?.panelId) return;
      const panel = api.getPanel(detail.panelId);
      if (!panel) return;
      setTabMenu({ x: detail.x, y: detail.y, items: buildTabMenuItems(panel) });
    };
    window.addEventListener(TAB_CONTEXT_MENU_EVENT, onTabContextMenu);
    return () => window.removeEventListener(TAB_CONTEXT_MENU_EVENT, onTabContextMenu);
  }, [buildTabMenuItems]);

  // slterm:file-saved-as（Ctrl+S 另存为/首次保存后标题更新）——宿主单点监听
  useEffect(() => {
    const onSaveAs = (e: Event) => {
      const detail = (e as CustomEvent).detail as {
        panelId: string;
        newPath: string;
      };
      const pageId = pageOfPanelId(detail.panelId);
      if (!pageId) return;
      const rootPath = projectRootOfPage(pageId);
      if (!rootPath) return;
      const api = apiRef.current;
      if (!api) return;
      const updates = titleManager.handleSaveAs(pageId, detail.panelId, detail.newPath, rootPath);
      for (const { panelId, title } of updates) {
        const p = api.getPanel(panelId);
        if (p) p.api.setTitle(title);
      }
    };
    window.addEventListener("slterm:file-saved-as", onSaveAs);
    return () => window.removeEventListener("slterm:file-saved-as", onSaveAs);
  }, []);

  // 无活跃页时隐藏宿主（旧「页面全隐藏」空白主区语义）——dockview 不卸载
  // （终端存活）；hostReady 前恒可见（dockview 初始化需已布局容器）
  const containerVisible = !hostReady || activePageId !== null;

  return (
    <div
      ref={containerRef}
      className={DOCK_HOST_CLASS}
      style={hostContainerStyle(containerVisible)}
    >
      <DockviewReact
        className="dockview-theme-dark"
        components={panelRegistry}
        onReady={handleReady}
        watermarkComponent={Watermark}
        defaultTabComponent={DefaultTab}
        rightHeaderActionsComponent={RightHeader}
        disableFloatingGroups={true}
      />
      {/* 页签右键菜单（自研 fixed 弹层，UI-802 规格在 TabMenuPopup）——宿主单点 */}
      <TabMenuPopup menu={tabMenu} onClose={closeTabMenu} />
      {/* 重命名弹窗：仅活跃页可触发右键，可见性有保证 */}
      {renameTarget && (
        <TerminalRenameDialog
          initialTitle={renameTarget.initialTitle}
          onConfirm={handleRenameConfirm}
          onCancel={() => setRenameTarget(null)}
        />
      )}
    </div>
  );
};

export default WorkspaceDockHost;
