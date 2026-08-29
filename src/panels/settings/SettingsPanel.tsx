// SettingsPanel — 设置中心面板壳（F11，SC-FE-03）
//
// 职责：左导航（组序 global→project，固定 180px）+ 右配置页槽位（SettingsPageRegistry
// 分派渲染，key={selectedPage} 强制重挂载——ADR-0001 先例，dirty/页内状态随卸载丢弃）。
// - 选中页：params.selectedPage 命中注册表 → 用之；否则回退全局组第一页；注册表空 → 空态。
// - 壳是 params 持久化单点：persistParams（updateParameters + 显式 onLayoutChange(saveLayout)
//   + 按 panelId `settings-` 前缀解析 pageId → updatePageLayout 写 store）——选中切换与
//   onPageParamsChange（pageParams[selectedPage] 槽 merge patch）均经此通道。
// - onDidParametersChange 订阅外部 selectedPage 变化（扁平事件结构红线：回调直接是 Parameters）。
// - corrupted 警示条：挂载 loadSettings() → corrupted → 顶部警示条（× 可关，不阻塞）。
// - 配色全走 theme/colors.ts token（硬约束 #6）。
// 注册触发点：顶部 import "../../features/settingsCenter/pages"（side-effect import 即注册，
// 新增配置页 = pages.ts 追加注册 + 本 import 链保持引用，禁止隐式初始化）。

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
// side-effect import 注册触发点（SC-FE-04：import 即注册全部配置页）
import "../../features/settingsCenter/pages";
import { getSettingsPageRegistry } from "../../features/settingsCenter";
import type { SettingsPage, SettingsPageGroup } from "../../features/settingsCenter";
import {
  setSettingsDirty,
  clearSettingsDirty,
  isSettingsDirty,
} from "../../features/settingsCenter/dirtyRegistry";
import { confirmDialog } from "../../lib";
import { loadSettings } from "../../ipc/settings";
import { saveLayout } from "../../workspace/layoutSerde";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import {
  PANEL_BG,
  SECONDARY_BG,
  SIDEBAR_FG,
  DIM_FG,
  EXPLORER_SELECTION_BG,
  SEPARATOR_BG,
  PLACEHOLDER_FG,
  ERROR_BANNER_BG,
  ERROR_BANNER_BORDER,
  ERROR_BANNER_FG,
} from "../../theme";
import type { DockviewPanelApi, DockviewApi } from "dockview-react";

/** SettingsPanel 面板参数（params 持久化单点）：selectedPage = 选中配置页 id 随布局持久化；
    pageParams = 各页内参数（按 pageId 键槽，页组件经 SettingsPageProps 读写） */
interface SettingsPanelProps {
  /** Dockview 传入的面板 API */
  api: DockviewPanelApi;
  /** Dockview 传入的容器 API（saveLayout 序列化布局用） */
  containerApi: DockviewApi;
  /** Dockview 传入的面板参数 */
  params?: {
    panelId?: string;
    selectedPage?: string;
    pageParams?: Record<string, Record<string, unknown>>;
  };
}

/** 导航组序（规格 §4.3：组序「全局」在上、「项目」在下） */
const GROUP_ORDER: Array<{ group: SettingsPageGroup; title: string }> = [
  { group: "global", title: "全局" },
  { group: "project", title: "项目" },
];

/** confirmDialog 弹窗关闭后守卫窗口（ms）——期间内的回归触发的重读被抑制（防循环，
    照 HooksConfigPanel/hub ASK_GUARD_MS 同常量，SC-FE-07） */
const ASK_GUARD_MS = 500;

/** 面板根容器样式 */
const containerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  background: PANEL_BG,
  display: "flex",
  flexDirection: "column",
};

/** 警示条样式（ERROR_BANNER token 组） */
const bannerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 8,
  padding: "6px 12px",
  fontSize: 12,
  color: ERROR_BANNER_FG,
  background: ERROR_BANNER_BG,
  borderBottom: `1px solid ${ERROR_BANNER_BORDER}`,
  flexShrink: 0,
};

/** 警示条关闭按钮样式 */
const bannerCloseStyle: React.CSSProperties = {
  border: "none",
  background: "transparent",
  color: ERROR_BANNER_FG,
  fontSize: 14,
  lineHeight: 1,
  cursor: "pointer",
  padding: "2px 4px",
};

/** 主体（导航 + 槽位）横排样式 */
const bodyStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  minHeight: 0,
};

/** 左导航固定 180px（规格 §4.3：不可拖拽） */
const navStyle: React.CSSProperties = {
  width: 180,
  flexShrink: 0,
  background: SECONDARY_BG,
  borderRight: `1px solid ${SEPARATOR_BG}`,
  overflowY: "auto",
  padding: "8px 0",
};

/** 组标题样式 */
const groupTitleStyle: React.CSSProperties = {
  padding: "6px 12px 4px",
  fontSize: 11,
  color: DIM_FG,
  userSelect: "none",
};

/** 页项样式（active = 选中态背景高亮，硬约束 #6 token） */
function navItemStyle(active: boolean): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 6,
    width: "100%",
    padding: "5px 12px",
    fontSize: 13,
    textAlign: "left",
    cursor: "pointer",
    background: active ? EXPLORER_SELECTION_BG : "transparent",
    color: SIDEBAR_FG,
    border: "none",
  };
}

/** dirty 圆点样式（7px 中性色 token——不用 F3 四态色，防语义混淆，SC-FE-07 口径） */
const dirtyDotStyle: React.CSSProperties = {
  width: 7,
  height: 7,
  borderRadius: "50%",
  background: DIM_FG,
  flexShrink: 0,
};

/** 右侧配置页槽位样式（flex 撑满，minHeight 0 防内容撑开） */
const contentStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  minWidth: 0,
  minHeight: 0,
};

/** 空态（注册表无任何配置页）居中样式 */
const emptyStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

const SettingsPanel: React.FC<SettingsPanelProps> = ({
  api,
  containerApi,
  params,
}) => {
  /** 注册表页集合（模块级单例，挂载期稳定——注册全在 import 期完成） */
  const registry = getSettingsPageRegistry();

  // 选中页（SC-FE-03）：params.selectedPage 命中注册表 → 用之；否则全局组第一页；注册表空 → null（空态）
  const [selectedPage, setSelectedPage] = useState<string | null>(() => {
    const saved = params?.selectedPage;
    if (saved && registry.get(saved)) return saved;
    return registry.getAll("global")[0]?.id ?? null;
  });

  // ref 镜像：异步回调闭包内读取最新值
  const selectedPageRef = useRef(selectedPage);
  selectedPageRef.current = selectedPage;
  // params 最新快照：props 同步 + onDidParametersChange 同步（下方订阅）——persistParams
  // 合并基准必须随每次参数变化更新，否则连续 onPageParamsChange patch 会互相覆盖
  // （照 TerminalPanel latestParamsRef 先例）
  const paramsRef = useRef(params);
  paramsRef.current = params;

  // 页 dirty 汇聚（导航项 dirty 圆点数据源；SC-FE-07 增强 dirtyRegistry 同步与切页守卫）
  const [dirtyMap, setDirtyMap] = useState<Record<string, boolean>>({});
  // dirtyMap ref 镜像：切页守卫异步闭包内读取最新值（照 selectedPageRef 先例）
  const dirtyMapRef = useRef(dirtyMap);
  dirtyMapRef.current = dirtyMap;
  // 切页确认弹窗守卫：弹窗打开期间 + 关闭后短暂窗口内抑制回归触发的重读（防循环，
  // 照 hub askGuard 先例，SC-FE-07）
  const askGuardRef = useRef(false);
  // 切项目自动关闭守卫状态（SC-FE-08）：isFirstRunRef 区分「初始评估」与「变化触发」——
  // 挂载时布局恢复场景静默关（新挂载不可能 dirty，无守卫必要）；closedRef 防重复 close
  // （close 后组件卸载前 effect 重跑）；closeGuardRef 关闭确认弹窗期间防 effect 重入
  const isFirstRunRef = useRef(true);
  const closedRef = useRef(false);
  const closeGuardRef = useRef(false);

  // 切项目自动关闭数据源订阅（SC-FE-08）：activePageId = 当前活跃页面；
  // projects = 全部项目（ownProjectId / activeProjectId 反查基准）
  const activePageId = useLayout((s) => s.activePageId);
  const projects = useProjects((s) => s.projects);

  // corrupted 警示条（挂载 loadSettings；× 可关，不阻塞）
  const [corrupted, setCorrupted] = useState(false);
  const [bannerDismissed, setBannerDismissed] = useState(false);

  /** 显式布局保存终点（照 HooksConfigPanel handleLayoutPersist 先例改 settings- 前缀）：
      经 pageId 查 projId 写 store，等价于页面级 onLayoutChange 链 */
  const handleLayoutPersist = useCallback(
    (layout: Record<string, unknown>) => {
      const panelId = params?.panelId;
      if (!panelId?.startsWith("settings-")) return;
      const pageId = panelId.slice("settings-".length);
      const { projects } = useProjects.getState();
      for (const [projId, proj] of Object.entries(projects)) {
        if (proj.pages.some((p) => p.pageId === pageId)) {
          useProjects.getState().updatePageLayout(projId, pageId, layout);
          break;
        }
      }
    },
    [params?.panelId],
  );

  /** 壳是 params 持久化单点：updateParameters + 显式 onLayoutChange(saveLayout(containerApi))
      + updatePageLayout——updateParameters 不触发 onDidLayoutChange，必须显式保存（F8 先例） */
  const persistParams = useCallback(
    (patch: Record<string, unknown>) => {
      api.updateParameters({ ...(paramsRef.current ?? {}), ...patch });
      handleLayoutPersist(saveLayout(containerApi) as Record<string, unknown>);
    },
    [api, containerApi, handleLayoutPersist],
  );

  /** 导航切换（选中态经 persistParams 随布局 JSON 持久化；SC-FE-07 切页守卫：
      当前页 dirty → confirmDialog 确认丢弃；取消不切换，确认清 dirty 后切换） */
  const handlePageSelect = useCallback(
    (pageId: string) => {
      if (pageId === selectedPageRef.current) return;
      void (async () => {
        const currentPageId = selectedPageRef.current;
        const currentDirty =
          currentPageId !== null && dirtyMapRef.current[currentPageId] === true;
        if (currentDirty) {
          // 守卫前置：弹窗开/关伴随回归触发，无守卫将再弹窗（防循环，照 hub 先例）
          askGuardRef.current = true;
          let ok: boolean;
          try {
            ok = await confirmDialog({
              title: "未保存的修改",
              message: "当前配置页有未保存的修改，切换将丢弃这些修改。",
              kind: "warning",
            });
          } finally {
            setTimeout(() => {
              askGuardRef.current = false;
            }, ASK_GUARD_MS);
          }
          if (!ok) return;
          // 确认丢弃：清当前页 dirty（圆点 + dirtyRegistry 同步）再切换
          setDirtyMap((prev) => ({
            ...prev,
            [currentPageId as string]: false,
          }));
          if (paramsRef.current?.panelId) {
            setSettingsDirty(paramsRef.current.panelId, false);
          }
        }
        setSelectedPage(pageId);
        persistParams({ selectedPage: pageId });
      })();
    },
    [persistParams],
  );

  /** 页组件 dirty 上报（SC-FE-03 圆点槽；SC-FE-07 同步 dirtyRegistry 真值源——
      DefaultTab × 关闭守卫与壳共享同一 dirty，防两处状态漂移） */
  const handleDirtyChange = useCallback((dirty: boolean) => {
    setDirtyMap((prev) => {
      const pageId = selectedPageRef.current;
      if (pageId === null) return prev;
      return { ...prev, [pageId]: dirty };
    });
    if (paramsRef.current?.panelId) {
      setSettingsDirty(paramsRef.current.panelId, dirty);
    }
  }, []);

  /** 页内参数 patch 通道（壳是 params 持久化单点）：pageParams[selectedPage] 槽 merge patch */
  const handlePageParamsChange = useCallback(
    (patch: Record<string, unknown>) => {
      const pageId = selectedPageRef.current;
      if (pageId === null) return;
      const latest = paramsRef.current?.pageParams ?? {};
      persistParams({
        pageParams: {
          ...latest,
          [pageId]: { ...(latest[pageId] ?? {}), ...patch },
        },
      });
    },
    [persistParams],
  );

  /** 外部 selectedPage 变化订阅（扁平事件结构红线：回调直接是 Parameters 对象） */
  useEffect(() => {
    const d = api.onDidParametersChange((parameters) => {
      // 合并基准随每次参数变化更新（连续 patch 不互相覆盖）
      paramsRef.current = { ...paramsRef.current, ...(parameters as object) };
      const p = parameters as { selectedPage?: string };
      if (p.selectedPage && registry.get(p.selectedPage)) {
        setSelectedPage(p.selectedPage);
      }
    });
    return () => d.dispose();
  }, [api]);

  // dirtyRegistry 挂载注册/卸载 clear（SC-FE-07）：面板存活期间 × 关闭守卫可查；
  // 卸载即清除——面板关闭后不存在「未保存修改」（新挂载不可能 dirty）
  useEffect(() => {
    const panelId = params?.panelId;
    if (!panelId) return;
    setSettingsDirty(panelId, false);
    return () => clearSettingsDirty(panelId);
  }, [params?.panelId]);

  // 切项目自动关闭（SC-FE-08）：订阅 activePageId 所属项目，与面板自身所属项目不同 → 关闭。
  // - ownProjectId：panelId 去 `settings-` 前缀（SC-FE-02 panelId 契约）后反查 projects；
  //   activeProjectId：activePageId 反查。两者均非空且不同才动作——归属解析失败不误关。
  // - activePageId === null → 不动（删除末页/启动瞬态，防连锁误关）。
  // - 初始评估：挂载时 activeProjectId 已定且不一致（布局恢复场景）→ 直接 api.close() 静默
  //   ——新挂载不可能 dirty（壳挂载注册 false），无守卫必要。
  // - 变化触发：dirty 守卫——isSettingsDirty → confirmDialog 确认才关；取消则不关，
  //   面板暂留非活跃项目（尊重用户选择，文档注明）。非 dirty 直关。
  // - closedRef 防重复 close（close 后组件卸载前 effect 重跑兜底）；
  //   closeGuardRef 弹窗期间防重入（projects 异步变化会重跑 effect，弹窗不得堆叠）。
  useEffect(() => {
    const panelId = params?.panelId;
    if (!panelId?.startsWith("settings-")) return;
    const ownPageId = panelId.slice("settings-".length);
    const findProjectId = (pageId: string): string | null => {
      for (const [projId, proj] of Object.entries(projects)) {
        if (proj.pages.some((p) => p.pageId === pageId)) return projId;
      }
      return null;
    };
    const ownProjectId = findProjectId(ownPageId);
    const activeProjectId = activePageId === null ? null : findProjectId(activePageId);
    // 初始评估标记：无论本轮是否动作，首轮过后「初始评估」语义即失效（后续均为变化触发）
    const firstRun = isFirstRunRef.current;
    isFirstRunRef.current = false;
    if (closedRef.current || closeGuardRef.current) return;
    if (activeProjectId === null || ownProjectId === null) return;
    if (activeProjectId === ownProjectId) return;
    if (firstRun) {
      // 布局恢复场景：挂载即静默关（新挂载不可能 dirty，见上方注释）
      closedRef.current = true;
      api.close();
      return;
    }
    void (async () => {
      if (!isSettingsDirty(panelId)) {
        closedRef.current = true;
        api.close();
        return;
      }
      // 弹窗期间置位防重入（照 SC-FE-07 askGuard 先例，弹窗关闭后短暂窗口抑制回归重读）
      closeGuardRef.current = true;
      let ok: boolean;
      try {
        ok = await confirmDialog({
          title: "未保存的修改",
          message: "当前配置页有未保存的修改，关闭将丢弃这些修改。",
          kind: "warning",
        });
      } finally {
        setTimeout(() => {
          closeGuardRef.current = false;
        }, ASK_GUARD_MS);
      }
      if (!ok) return; // 取消则不关——面板暂留非活跃项目，尊重用户选择
      closedRef.current = true;
      api.close();
    })();
  }, [activePageId, projects, api, params?.panelId]);

  // corrupted 警示条检测（挂载一次；不阻塞面板渲染）
  useEffect(() => {
    let mounted = true;
    loadSettings()
      .then((s) => {
        if (mounted) setCorrupted(s.corrupted);
      })
      .catch((e) => console.error("加载设置失败", e));
    return () => {
      mounted = false;
    };
  }, []);

  // 导航数据（注册表挂载期稳定，组件生命周期内不重算）
  const globalPages = useMemo(() => registry.getAll("global"), [registry]);
  const projectPages = useMemo(() => registry.getAll("project"), [registry]);

  // 当前选中页注册项（右侧槽位渲染分派数据源）；null → 空态
  const selectedPageObj: SettingsPage | undefined = selectedPage
    ? registry.get(selectedPage)
    : undefined;

  /** 渲染一个导航组（组标题 + 页项；空组不渲染区块保持组序稳定） */
  const renderGroup = (
    group: SettingsPageGroup,
    title: string,
    pages: SettingsPage[],
  ) => {
    if (pages.length === 0) return null;
    return (
      <div key={group}>
        <div style={groupTitleStyle} data-e2e={`settings-nav-group-${group}`}>
          {title}
        </div>
        {pages.map((p) => (
          <button
            key={p.id}
            type="button"
            data-e2e={`settings-nav-${p.id}`}
            onClick={() => handlePageSelect(p.id)}
            style={navItemStyle(selectedPage === p.id)}
          >
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {p.title}
            </span>
            {dirtyMap[p.id] && (
              <span data-e2e={`settings-nav-dirty-${p.id}`} style={dirtyDotStyle} />
            )}
          </button>
        ))}
      </div>
    );
  };

  return (
    <div style={containerStyle} data-e2e="settings-panel">
      {/* corrupted 警示条（× 可关；不阻塞配置操作） */}
      {corrupted && !bannerDismissed && (
        <div style={bannerStyle} data-e2e="settings-corrupted-banner">
          <span>设置文件已损坏，已从备份/默认值恢复</span>
          <button
            type="button"
            aria-label="关闭提示"
            data-e2e="settings-corrupted-banner-close"
            onClick={() => setBannerDismissed(true)}
            style={bannerCloseStyle}
          >
            ×
          </button>
        </div>
      )}
      <div style={bodyStyle}>
        {/* 左导航（组序 global→project，固定 180px） */}
        <nav style={navStyle}>
          {GROUP_ORDER.map(({ group, title }) =>
            renderGroup(group, title, group === "global" ? globalPages : projectPages),
          )}
        </nav>
        {/* 右配置页槽位（key={selectedPage} 强制重挂载——ADR-0001 先例）；
            SettingsPageProps 透传：dirty 上报 + pageParams 槽 + patch 通道 */}
        <div style={contentStyle}>
          {selectedPageObj ? (
            <selectedPageObj.component
              key={selectedPage}
              onDirtyChange={handleDirtyChange}
              pageParams={params?.pageParams?.[selectedPage as string]}
              onPageParamsChange={handlePageParamsChange}
            />
          ) : (
            <div style={emptyStyle}>
              <span style={{ color: PLACEHOLDER_FG, fontSize: 13 }}>暂无配置页</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default SettingsPanel;
