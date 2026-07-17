// PageDockviewHost — 单个操作页面的 Dockview 实例
//
// 包含 PageDockview 组件及其依赖：DefaultTab、Watermark、RightHeader、ContextMenu 工厂、
// 标题应用辅助函数。从 Workspace.tsx 提取，Workspace.tsx 只保留编排层。
//
// F1: PageDockview 用 React.memo 包裹，配合稳定化 props 减少不必要的重渲染。
// F2: savedLayout 通过 useRef 读取，不进入 handleReady 的 useCallback deps。

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
import { panelRegistry, PANEL_TERMINAL } from "../panelRegistry";
import { saveLayout, loadLayout } from "./layoutSerde";
import { titleManager } from "./titleManager";
import type { TitleUpdate } from "./titleManager";
import {
  INPUT_BORDER,
  SECONDARY_BG,
  BUTTON_FG,
  PLACEHOLDER_FG,
  SEPARATOR_BG,
} from "../theme";

const WATERMARK_TEXT = "打开终端或编辑器开始工作";

// ---- 类型 ----

/** 扩展的 params 类型（终端面板通过 updateParameters 设置 tabIcon） */
export interface TabParams {
  panelId?: string;
  filePath?: string;
  cwd?: string;
  tabIcon?: string | null;
}

export interface PageDockviewProps {
  pageId: string;
  cwd: string | undefined;
  rootPath: string | undefined;
  savedLayout: Record<string, unknown> | undefined;
  visible: boolean;
  onReady: (api: DockviewApi) => void;
  onLayoutChange: (layout: Record<string, unknown>) => void;
}

// ---- 工厂函数 ----

/** 创建 Watermark 组件（捕获 pageId + cwd 闭包） */
function createWatermark(
  nextPanelId: () => string,
  pageId: string,
  cwd: string | undefined,
): React.FC<IWatermarkPanelProps> {
  const Watermark: React.FC<IWatermarkPanelProps> = ({ containerApi }) => (
    <div
      style={{
        display: "flex", flexDirection: "column", alignItems: "center",
        justifyContent: "center", height: "100%", color: INPUT_BORDER, fontSize: 14,
        userSelect: "none", gap: 12,
      }}
    >
      <span>{WATERMARK_TEXT}</span>
      <div style={{ display: "flex", gap: 8 }}>
        <button
          onClick={() => {
            const id = nextPanelId();
            const title = titleManager.getTerminalTitle(pageId);
            containerApi.addPanel({
              id, component: PANEL_TERMINAL, title,
              params: { panelId: id, cwd }, renderer: "always",
            });
          }}
          style={{
            background: SECONDARY_BG, border: `1px solid ${SEPARATOR_BG}`, color: BUTTON_FG,
            cursor: "pointer", fontSize: 13, padding: "4px 12px", borderRadius: 4,
          }}
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
          containerApi.addPanel({
            id, component: PANEL_TERMINAL, title,
            params: { panelId: id, cwd }, renderer: "always",
            position: { referenceGroup: group },
          });
        }}
        style={{
          background: "none", border: `1px solid ${SEPARATOR_BG}`, color: BUTTON_FG,
          cursor: "pointer", fontSize: 16, width: 24, height: 24, borderRadius: 4,
          display: "flex", alignItems: "center", justifyContent: "center", lineHeight: 1,
        }}
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

// ---- 辅助函数 ----

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
    // FE-22: 从 params 判断面板类型（替代 panel.view?.contentComponent 非公共 API）
    // 文件型面板（editor/htmlviewer）的 params 携带 filePath
    const filePath = params.filePath;
    if (filePath !== undefined) {
      // 先注销旧条目（避免 fromJSON 重复注册）
      titleManager.unregisterEditor(pageId, params.panelId);
      titleManager.registerEditor(pageId, params.panelId, filePath);
    }
  }

  const updates = titleManager.recomputeTitles(pageId, rootPath);
  applyTitleUpdates(api, updates);
}

// ---- DefaultTab ----

const DefaultTab: React.FC<IDockviewPanelProps> = (props) => {
  const { api, params } = props;
  const tabParams = params as TabParams;
  const [title, setTitle] = useState(api.title || api.component || "");
  const [tabIcon, setTabIcon] = useState<string | null>(
    tabParams?.tabIcon ?? null,
  );
  useEffect(() => {
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
    <div style={{
      display: "flex", alignItems: "center", height: "100%",
      padding: "0 8px", gap: 6, userSelect: "none",
    }}>
      {tabIcon && (
        <img src={tabIcon} width={16} height={16}
          style={{ flexShrink: 0, display: "block" }} alt="" />
      )}
      <span style={{ fontSize: 13 }}>{title}</span>
      <button
        onClick={(e) => { e.stopPropagation(); api.close(); }}
        style={{
          background: "none", border: "none", color: PLACEHOLDER_FG,
          cursor: "pointer", padding: "0 2px", fontSize: 14, lineHeight: 1,
        }}
        title="关闭"
      >×</button>
    </div>
  );
};

// ---- PageDockview（React.memo 包裹）----

/**
 * 单个操作页面的 Dockview 实例。
 * F1: React.memo 包裹 + F2: savedLayout 经 useRef 去稳 handleReady。
 */
const PageDockview: React.FC<PageDockviewProps> = React.memo(({
  pageId, cwd, rootPath, savedLayout, visible, onReady: onApiReady, onLayoutChange,
}) => {
  const apiRef = useRef<DockviewApi | null>(null);
  const panelSeqRef = useRef(0);
  const restoreGuardRef = useRef(false);
  /** 收集 handleReady 内注册的三个 disposable（onDidLayoutFromJSON/onDidLayoutChange/onDidRemovePanel） */
  const disposablesRef = useRef<Array<{ dispose(): void }>>([]);

  // F2: savedLayout 通过 useRef 读取，不进入 handleReady 的 useCallback deps
  const savedLayoutRef = useRef(savedLayout);
  savedLayoutRef.current = savedLayout;

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

  // F2: savedLayout 已从 deps 移除——通过 savedLayoutRef.current 读取
  const handleReady = useCallback((event: { api: DockviewApi }) => {
    const { api } = event;
    apiRef.current = api;
    onApiReady(api);

    // FE-04: 先清理旧监听器（handleReady 重触发或页面重建时防泄漏）
    disposablesRef.current.forEach((d) => d.dispose());
    disposablesRef.current = [];

    // 恢复保存的布局（无布局时留空，由 Watermark 组件接管显示）
    const layout = savedLayoutRef.current;
    let restored = false;
    if (layout && Object.keys(layout).length > 0) {
      restored = loadLayout(api, layout);
    }
    // 不创建默认终端——空白页面由 watermarkComponent 渲染
    // "打开终端或编辑器开始工作"，用户可点击"新建终端"按钮

    // 从保存布局恢复后，重建编辑器注册表并重算标题（忽略持久化的 title）
    if (restored) {
      rebuildAndRecomputeTitles(api, pageId, rootPath);
    }

    // fromJSON 恢复守卫 — 程序化恢复不触发布局保存
    disposablesRef.current.push(
      api.onDidLayoutFromJSON(() => {
        restoreGuardRef.current = true;
        setTimeout(() => { restoreGuardRef.current = false; }, 0);
      }),
    );

    // 布局变更 → 保存到 store（硬约束 #7）
    disposablesRef.current.push(
      api.onDidLayoutChange(() => {
        if (restoreGuardRef.current) return;
        const layout = saveLayout(api);
        onLayoutChange(layout as Record<string, unknown>);
      }),
    );

    // 面板关闭 → 注销编辑器 + 重算剩余面板标题
    disposablesRef.current.push(
      api.onDidRemovePanel((panel) => {
        const params = panel.params as { panelId?: string } | undefined;
        if (params?.panelId) {
          titleManager.unregisterEditor(pageId, params.panelId);
        }
        if (rootPath) {
          const updates = titleManager.recomputeTitles(pageId, rootPath);
          applyTitleUpdates(api, updates);
        }
      }),
    );
  }, [onApiReady, cwd, pageId, rootPath, nextPanelId, onLayoutChange]);
  // 注意：savedLayout 已从 deps 移除——通过 savedLayoutRef 读取最新值

  // FE-04: 组件卸载时清理所有 disposable（onDidLayoutFromJSON/onDidLayoutChange/onDidRemovePanel）
  useEffect(() => {
    return () => {
      disposablesRef.current.forEach((d) => d.dispose());
    };
  }, []);

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
    <div style={{
      display: visible ? "block" : "none",
      width: "100%", height: "100%",
    }}>
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
});

export default PageDockview;
export { createRightHeader, createGetContextMenu };
