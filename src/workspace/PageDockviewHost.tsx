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
import { FileIcon } from "../features/explorer/FileIcon";
import { saveLayout, loadLayout } from "./layoutSerde";
import { makeTerminalPanelId, advanceTerminalPanelSeq } from "../lib/panelId";
import { StatusDot } from "../lib/StatusDot";
import type { AgentStatus } from "../lib/agentStatus";
import { titleManager } from "./titleManager";
import type { TitleUpdate } from "./titleManager";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { TerminalRenameDialog } from "./TerminalRenameDialog";
import {
  INPUT_BORDER,
  SECONDARY_BG,
  BUTTON_FG,
  PLACEHOLDER_FG,
  SEPARATOR_BG,
  SIDEBAR_FG,
  DIM_FG,
  FOCUS_BORDER,
  dockviewVarStyle,
} from "../theme";

const WATERMARK_TEXT = "打开终端或编辑器开始工作";

// ---- 类型 ----

/** 扩展的 params 类型（终端面板通过 updateParameters 设置 tabStatus / tabLogo / customTitle） */
export interface TabParams {
  panelId?: string;
  filePath?: string;
  cwd?: string;
  /** 终端会话状态（IC-03：状态圆点渲染数据源，由 TerminalPanel 写入；null=无状态） */
  tabStatus?: AgentStatus | null;
  /** CLI 品牌 logo 根绝对路径（F9 修订：跟随页签名显示，不依赖 tabStatus；由 TerminalPanel 会话绑定写入） */
  tabLogo?: string | null;
  /** 用户自定义页签标题（右键菜单重命名，随布局 JSON 持久化） */
  customTitle?: string;
}

/** 右键菜单面板类型（GetTabContextMenuItemsParams["panel"] 派生，免额外 import） */
export type ContextMenuPanel = GetTabContextMenuItemsParams["panel"];

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
  const Header: React.FC<IDockviewHeaderActionsProps> = ({ containerApi, group }) => {
    // TAB-04: + 钮 hover 状态（同 DefaultTab ×——inline style 无法表达 :hover，
    // 执行期定为 React 状态）
    const [hovered, setHovered] = useState(false);
    return (
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
          onMouseEnter={() => setHovered(true)}
          onMouseLeave={() => setHovered(false)}
          style={{
            // TAB-04: 扁平图标钮——去边框；22px 圆角 4；fg-3（DIM_FG，design.md 明度
            // 阶梯 fg-3=#8a857d，与 IC-06 活动栏图标同映射）；hover 底 ui.secondaryBg
            background: hovered ? SECONDARY_BG : "none",
            border: "none", color: DIM_FG,
            cursor: "pointer", fontSize: 16, width: 22, height: 22, borderRadius: 4,
            display: "flex", alignItems: "center", justifyContent: "center", lineHeight: 1,
          }}
          title="新建终端"
        >+</button>
      </div>
    );
  };
  return Header;
}

/**
 * 应用页签重命名（导出纯函数供 L2 直测）：
 * 1. updateParameters 写入 customTitle（随布局 JSON 持久化的单一真值源）
 * 2. setTitle 更新显示
 * 3. 显式 onLayoutChange(saveLayout(api)) 触发持久化——setTitle/updateParameters
 *    均不触发 onDidLayoutChange（dockviewPanel.js:84-95 只更新 _title + fire title change）
 */
export function applyRename(
  api: DockviewApi,
  panel: ContextMenuPanel,
  newTitle: string,
  onLayoutChange: (layout: Record<string, unknown>) => void,
): void {
  panel.api.updateParameters({
    ...(panel.params ?? {}),
    customTitle: newTitle,
  });
  panel.api.setTitle(newTitle);
  onLayoutChange(saveLayout(api) as Record<string, unknown>);
}

/** 创建 getTabContextMenuItems 回调（捕获 pageId 闭包） */
function createGetContextMenu(
  nextPanelId: () => string,
  pageId: string,
  onRenameRequest: (panel: ContextMenuPanel) => void,
): (params: GetTabContextMenuItemsParams) => (BuiltInContextMenuItem | ReactContextMenuItemConfig)[] {
  return (params: GetTabContextMenuItemsParams) => {
    const newTerminalId = nextPanelId();
    // 仅终端面板显示「重命名」：判据为 view.contentComponent（panel.component 不存在）
    const isTerminal = params.panel.view.contentComponent === PANEL_TERMINAL;
    // claude 运行中（agentSession 存在即运行中，二态模型）→ 禁用重命名；
    // 菜单每次右键重新构建，判断实时
    const claudeRunning = TerminalRegistry.get(params.panel.id)?.agentSession != null;
    const items: (BuiltInContextMenuItem | ReactContextMenuItemConfig)[] = [
      { label: "新建终端", action: () => { params.api.addPanel(
          { id: newTerminalId, component: PANEL_TERMINAL, title: titleManager.getTerminalTitle(pageId),
            params: { panelId: newTerminalId }, renderer: "always",
            position: { referenceGroup: params.group } }); } },
      "separator",
    ];
    if (isTerminal) {
      items.push(
        { label: "重命名", disabled: claudeRunning, action: () => onRenameRequest(params.panel) },
        "separator",
      );
    }
    items.push("close", "closeOthers", "closeAll");
    return items;
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

/** 遍历 DockviewApi 中所有面板，重建 titleManager 注册表并重算标题 */
function rebuildAndRecomputeTitles(
  api: DockviewApi,
  pageId: string,
  rootPath: string | undefined,
): void {
  // B12: 终端 pass——布局恢复的终端面板（无 customTitle）用 titleManager 重算编号。
  // 持久化 title 可能是瞬态值（如 claude 运行中退出保存的 "claude"），恢复后
  // 必须回 terminal-N；F8 自定义名（customTitle）保留。终端编号不依赖项目根，
  // 此 pass 置于 rootPath 检查之前。
  for (const panel of api.panels) {
    const params = panel.params as { panelId?: string; customTitle?: string } | undefined;
    if (!params?.panelId) continue;
    if (panel.view?.contentComponent !== PANEL_TERMINAL) continue;
    if (params.customTitle !== undefined) continue;
    panel.api.setTitle(titleManager.getTerminalTitle(pageId));
  }

  if (!rootPath) return;

  // 遍历所有面板，重建编辑器注册表
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

/** 默认页签组件（导出供 L2 测试直接渲染——D2 最小可测性重构，零行为变更） */
export const DefaultTab: React.FC<IDockviewPanelProps> = (props) => {
  const { api, params } = props;
  const tabParams = params as TabParams;
  const [title, setTitle] = useState(api.title || api.component || "");
  const [tabStatus, setTabStatus] = useState<AgentStatus>(
    tabParams?.tabStatus ?? null,
  );
  const [tabLogo, setTabLogo] = useState<string | null>(
    tabParams?.tabLogo ?? null,
  );
  // TAB-03: 文件路径状态化——params prop 在 updateParameters 后不变，
  // 文件页签图标须经 onDidParametersChange 事件驱动（照 tabStatus/tabLogo 先例）
  const [filePath, setFilePath] = useState<string | null>(
    tabParams?.filePath ?? null,
  );
  // TAB-01: 激活态 = 面板是本组可见面板（isActive）且本组为聚焦组（isGroupActive）
  // ——底部指示条只给聚焦组的激活页签（非聚焦组可见页签已有实底 #0a0a0b，不加条）
  // isActive ?? true：真实 dockview 恒为 boolean；测试 fake api 未提供该字段，
  // 兜底视为本组可见面板（测试只驱动 isGroupActive 维度）
  const [isActive, setIsActive] = useState(api.isActive ?? true);
  const [isGroupActive, setIsGroupActive] = useState(api.isGroupActive);
  // TAB-02: hover 状态——执行期定为 React 状态条件渲染（inline style 无法表达
  // 跨 .dv-tab 父级的 :hover；dockview 内置 .dv-default-tab-action 显隐规则只作用于
  // 其内置默认页签，不作用于自定义 defaultTabComponent，故不可复用其 CSS）
  const [tabHovered, setTabHovered] = useState(false);
  const [closeHovered, setCloseHovered] = useState(false);
  useEffect(() => {
    const d1 = api.onDidTitleChange((event) => {
      setTitle(event.title);
    });
    const d2 = api.onDidParametersChange((event) => {
      // event 就是 Parameters 对象本身（Dockview PanelApi.onDidParametersChange
      // 类型签名为 Event<Parameters>，回调直接接收 Parameters 对象）
      const p = event as TabParams;
      setTabStatus(p?.tabStatus ?? null);
      setTabLogo(p?.tabLogo ?? null);
      setFilePath(p?.filePath ?? null);
    });
    // TAB-01: 激活态订阅（? 可选调用——测试 fake api 未提供 onDidActiveChange，
    // 真实 dockview 恒有；if (e) 守卫：事件为 undefined 时不崩溃且保持当前态）
    const d3 = api.onDidActiveChange?.((e) => { if (e) setIsActive(e.isActive); });
    const d4 = api.onDidActiveGroupChange?.((e) => { if (e) setIsGroupActive(e.isActive); });
    return () => {
      d1.dispose();
      d2.dispose();
      d3?.dispose();
      d4?.dispose();
    };
  }, [api]);
  // TAB-03: 文件型页签判据 = params.filePath 存在（只有 FILE_PANEL_TYPES
  // ——editor/htmlviewer/gitshow/diff——的面板携带 filePath；terminal/hooksConfig
  // 恒不设置，见 panelRegistry.ts），命中即渲染 FileIcon 彩色图标（按扩展名取色；
  // gitshow/diff 标题含 suffix 不影响图标）
  // 文件名 = 路径 basename（兼容 \ 与 / 分隔）
  const fileName = filePath != null
    ? filePath.split(/[\\/]/).pop() || filePath
    : null;
  return (
    <div
      onMouseEnter={() => setTabHovered(true)}
      onMouseLeave={() => setTabHovered(false)}
      style={{
        display: "flex", alignItems: "center", height: "100%",
        padding: "0 8px", gap: 6, userSelect: "none",
        // 本 div 保持静态定位（不设 position）——TAB-01 指示条 absolute 锚定
        // 库内建 position:relative 的 .dv-tab，bottom:0 即页签真实底边
        // （与 dockview 的 4px 内边距解耦，padding 变化不影响指示条贴底）
      }}
    >
      {/* 终端状态圆点（IC-03：tabIcon emoji/img 分支随 STATUS_EMOJI 删除，
          改 StatusDot 按状态渲染——working 绿/attention 黄/done 灰/error 红） */}
      {tabStatus != null && <StatusDot status={tabStatus} />}
      {/* CLI 品牌 logo：跟随页签名显示（F9 行为修订）——tabLogo 有值即渲染，
          不依赖 tabStatus；状态圆点缺席时 logo 顶到标题前（位置语义不变） */}
      {tabLogo && (
        <img src={tabLogo} width={16} height={16}
          style={{ flexShrink: 0, display: "block" }} alt="CLI 图标" />
      )}
      {/* 文件型页签：FileIcon 彩色图标（TAB-03）——与终端分支（圆点/logo）互斥 */}
      {fileName && <FileIcon name={fileName} isDir={false} />}
      {/* 标题：hover 时未激活页签文字变 fg-1（激活页签 dockview 变量已置 fg-1），
          底不变——TAB-01 hover 仅文字变色 */}
      <span style={{
        fontSize: 13,
        color: tabHovered && !isActive ? SIDEBAR_FG : undefined,
      }}>{title}</span>
      <button
        onClick={(e) => { e.stopPropagation(); api.close(); }}
        onMouseEnter={() => setCloseHovered(true)}
        onMouseLeave={() => setCloseHovered(false)}
        style={{
          // TAB-02: × 默认不可见（opacity 0 + pointerEvents none 防误点），hover 页签
          // 时显现——用 opacity 保布局稳定（条件渲染会致页签宽度随 hover 跳动）；
          // 激活页签同样不常驻。自身 hover 底 #2b2b31 = --dv-icon-hover-background-color
          // （linear 库变量已注入，复用单点，不新造色值）
          background: closeHovered ? "var(--dv-icon-hover-background-color)" : "none",
          border: "none", color: PLACEHOLDER_FG,
          cursor: "pointer", padding: "1px 4px", fontSize: 14, lineHeight: 1,
          borderRadius: 4, opacity: tabHovered ? 1 : 0,
          pointerEvents: tabHovered ? "auto" : "none",
        }}
        title="关闭"
      >×</button>
      {/* TAB-01: 激活页签底部 2px 指示条——absolute 定位锚定 .dv-tab（最近定位祖先），
          色 FOCUS_BORDER（#6e9ff2）；pointerEvents none 不拦截页签点击 */}
      {isActive && isGroupActive && (
        <div style={{
          position: "absolute", left: 0, right: 0, bottom: 0, height: 2,
          background: FOCUS_BORDER, pointerEvents: "none",
        }} />
      )}
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
  const restoreGuardRef = useRef(false);
  /** 收集 handleReady 内注册的三个 disposable（onDidLayoutFromJSON/onDidLayoutChange/onDidRemovePanel） */
  const disposablesRef = useRef<Array<{ dispose(): void }>>([]);

  // F2: savedLayout 通过 useRef 读取，不进入 handleReady 的 useCallback deps
  const savedLayoutRef = useRef(savedLayout);
  savedLayoutRef.current = savedLayout;

  /** per-page 稳定 panel ID 生成器（B14：生成单点 makeTerminalPanelId，与
   *  restoreSession 共享模块级每页计数——同页手动终端与恢复终端 id 互斥） */
  const nextPanelId = useCallback((): string => {
    return makeTerminalPanelId(pageId);
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
  // 重命名弹窗目标面板（右键菜单「重命名」→ setRenameTarget → 渲染 TerminalRenameDialog）
  const [renameTarget, setRenameTarget] = useState<{ panel: ContextMenuPanel; initialTitle: string } | null>(null);
  // ref 模式读取当前目标（handleRenameConfirm 保持稳定引用，照 ExplorerPanel actions 模式）
  const renameTargetRef = useRef(renameTarget);
  renameTargetRef.current = renameTarget;

  /** 打开重命名弹窗（预填 customTitle 优先，避免预填运行中命令的瞬态标题） */
  const openRenameDialog = useCallback((panel: ContextMenuPanel) => {
    const p = panel.params as TabParams | undefined;
    setRenameTarget({ panel, initialTitle: p?.customTitle ?? panel.title ?? "" });
  }, []);

  /** 重命名确认：applyRename（写 customTitle + setTitle + 显式保存）后关闭弹窗 */
  const handleRenameConfirm = useCallback((newTitle: string) => {
    const target = renameTargetRef.current;
    const api = apiRef.current;
    if (target && api) {
      applyRename(api, target.panel, newTitle, onLayoutChange);
      setRenameTarget(null);
    }
  }, [onLayoutChange]);

  const getTabContextMenuItems = useMemo(
    () => createGetContextMenu(nextPanelId, pageId, openRenameDialog),
    [nextPanelId, pageId, openRenameDialog],
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
      // B14: 先把本页终端序号计数推进到现有面板 max+1——布局恢复的持久化
      // 面板不占用计数，不推进则后续新建/恢复终端可能与已存在面板 id 重号
      advanceTerminalPanelSeq(
        pageId,
        api.panels.map((p) => p.id),
      );
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
      // dockview CSS 变量（20 条，active 方案 libraries.dockview）内联注入，
      // 替代主题类暗色常量；className="dockview-theme-dark" 保留供布局样式
      ...dockviewVarStyle(),
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
      {/* 重命名弹窗：仅活跃页可触发右键，页面可见性有保证；切页后随 display:none 隐藏 */}
      {renameTarget && (
        <TerminalRenameDialog
          initialTitle={renameTarget.initialTitle}
          onConfirm={handleRenameConfirm}
          onCancel={() => setRenameTarget(null)}
        />
      )}
    </div>
  );
});

export default PageDockview;
export { createRightHeader, createGetContextMenu };
