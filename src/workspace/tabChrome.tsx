// tabChrome — 页签 chrome 共享件（DefaultTab/Watermark/RightHeader/页签菜单/applyRename）。
// S11 共享宿主改造后原「每页一实例宿主组件」消亡（CP-004），仅存页签共享件，故名随实（FE-10）。
//
// 页组语义：操作页面 = 宿主内顶级页组（pageGroups.ts 协议），面板 id 全量
// 页前缀（{pageId}:localId）；本文件各工厂不再闭包页面实例，目标页在 action
// 时点解析（panel 属主页 / 活跃页），跨实例假设清零。

import React, { useMemo, useState, useEffect } from "react";
import {
  type DockviewApi,
  type DockviewGroupPanel,
  type IDockviewPanelProps,
  type IDockviewHeaderActionsProps,
  type IWatermarkPanelProps,
} from "dockview-react";
import { PANEL_TERMINAL } from "../panelRegistry";
import { FileIcon } from "../features/explorer/FileIcon";
import { closeTabGuarded, closeTabsGuarded } from "./tabClose";
import { StatusDot } from "../lib/StatusDot";
import type { AgentStatus } from "../lib/agentStatus";
import { titleManager } from "./titleManager";
import type { TitleUpdate } from "./titleManager";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { TerminalRenameDialog } from "./TerminalRenameDialog";
import { copyRelativePath } from "../lib/copyRelativePath";
import { TabMenuPopup } from "./TabMenuPopup";
import type { TabMenuItem } from "./TabMenuPopup";
import { IconEmptyBox } from "../lib/icons";
import { pageOfPanelId, pageGroupId, pageIdOfGroupId, makeTerminalIdInPage } from "./pageGroups";
import { useLayout } from "../stores/layout";
import { useProjects } from "../stores/projects";
import { saveLayout } from "./layoutSerde";
import {
  SECONDARY_BG,
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

/** 页签右键菜单/重命名目标面板结构——dockview IDockviewPanel 结构兼容子集
    （自研菜单经 containerApi.getPanel(panelId) 取真实对象，结构赋值自动成立；
     api.group 供「关闭族/新建终端 referenceGroup」定位右键瞬间所属组） */
export interface TabMenuPanel {
  id: string;
  title: string | undefined;
  params: Record<string, unknown> | undefined;
  view: { contentComponent: string | undefined };
  api: {
    close(): void;
    setTitle(title: string): void;
    updateParameters(parameters: Record<string, unknown>): void;
    group?: DockviewGroupPanel;
  };
}

/**
 * DefaultTab → 宿主的右键上报事件（window CustomEvent 协议）。
 * dockview-react 渲染 DefaultTab 的 framework part 不在 React 子树内（context
 * 不传播），故走事件广播（slterm:file-saved-as 先例）：宿主（唯一）监听，
 * 经 getPanel(panelId) 解析——panelId 页前缀全局唯一，命中即弹菜单。
 */
export const TAB_CONTEXT_MENU_EVENT = "slterm:tab-context-menu";

/** 事件 detail（panelId 页前缀全局唯一；x/y 为右键视口坐标，fixed 定位） */
export interface TabContextMenuDetail {
  panelId: string;
  x: number;
  y: number;
}

/** 目标页组解析（action 时点）：面板属主页组优先，兜底活跃页组 */
function resolvePageId(panelId: string | undefined, fallbackPageId: string | null): string | null {
  const owner = panelId !== undefined ? pageOfPanelId(panelId) : null;
  return owner ?? fallbackPageId;
}

/**
 * 新建终端面板（宿主内共享工厂——Watermark/RightHeader/右键菜单三入口合一）：
 * addPanel 显式 position.referenceGroup = 目标页组（生命周期契约——新增面板
 * options.group 显式指定，不随切页卸载；页组 id 字符串形态，组未挂载时
 * dockview 抛错前先经 getGroup 守卫返回 null）。
 */
export function addTerminalPanel(
  api: DockviewApi,
  pageId: string,
  cwd: string | undefined,
): string | null {
  const gid = pageGroupId(pageId);
  if (!api.getGroup(gid)) return null;
  const id = makeTerminalIdInPage(pageId);
  api.addPanel({
    id,
    component: PANEL_TERMINAL,
    title: titleManager.getTerminalTitle(pageId),
    params: { panelId: id, cwd },
    renderer: "always",
    position: { referenceGroup: gid },
  });
  return id;
}

// ---- 工厂函数 ----

/**
 * 创建 Watermark 组件（空页组接管——dockview 对空组渲染 watermarkComponent）。
 * 目标页 = 点击时点活跃页（空页组即活跃页组；containerApi.addPanel 无
 * position 落活跃组）。捕获 cwd 由活跃页上下文现取，不闭包页面实例。
 */
export function createWatermark(
  getApi: () => DockviewApi | null,
): React.FC<IWatermarkPanelProps> {
  const Watermark: React.FC<IWatermarkPanelProps> = () => {
    const pageId = useLayout((s) => s.activePageId);
    const cwd = useMemo(() => {
      if (!pageId) return undefined;
      const { projects } = useProjects.getState();
      for (const [, proj] of Object.entries(projects)) {
        const p = proj.pages.find((pg) => pg.pageId === pageId);
        if (p) return p.cwd ?? proj.rootPath;
      }
      return undefined;
    }, [pageId]);
    return (
      <div
        style={{
          display: "flex", flexDirection: "column", alignItems: "center",
          justifyContent: "center", height: "100%", // UI-204：正文 13px
          userSelect: "none", gap: 12,
        }}
      >
        {/* GL-05：空态统一——15px 线性图标 fg-4 + 说明文字 fg-3 */}
        <span style={{ color: PLACEHOLDER_FG, display: "flex" }}>
          <IconEmptyBox size={15} />
        </span>
        <span style={{ color: DIM_FG, fontSize: 13 }}>{WATERMARK_TEXT}</span>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            onClick={() => {
              // 空页组 = 当前活跃页（可见性单点保证），落活跃页组
              const api = getApi();
              if (!api) return;
              const target = useLayout.getState().activePageId;
              if (target) void addTerminalPanel(api, target, cwd);
            }}
            style={{
              background: SECONDARY_BG, border: `1px solid ${SEPARATOR_BG}`, color: SIDEBAR_FG,
              // FE-16: 圆角 6（UI-306 按钮档）
              cursor: "pointer", fontSize: 13, padding: "4px 12px", borderRadius: 6,
            }}
          >新建终端</button>
        </div>
      </div>
    );
  };
  return Watermark;
}

/** 创建 RightHeaderActions 组件（组头 + 钮——新建终端落本组） */
function createRightHeader(
  getApi: () => DockviewApi | null,
): React.FC<IDockviewHeaderActionsProps> {
  const Header: React.FC<IDockviewHeaderActionsProps> = ({ group }) => {
    // TAB-04: + 钮 hover 状态（同 DefaultTab ×——inline style 无法表达 :hover，
    // 执行期定为 React 状态）
    const [hovered, setHovered] = useState(false);
    const pageId = useLayout((s) => s.activePageId);
    return (
      <div style={{ display: "flex", alignItems: "center", height: "100%", paddingRight: 4 }}>
        <button
          onClick={() => {
            // 目标页 = 组属主页（页组协议解析）兜底活跃页——页组可见性单点保证
            // 非活跃页组不可见不可点，两值同页
            const api = getApi();
            if (!api || !pageId) return;
            const ownerPage = pageIdOfGroupId(group.id) ?? pageId;
            void addTerminalPanel(api, ownerPage, undefined);
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
  panel: TabMenuPanel,
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

// ---- 页签右键菜单（自研——dockview 8.1 free core 无 contextMenuService，
//      getTabContextMenuItems 路径恒短路，菜单机制自绘于 TabMenuPopup，见 workspace/CLAUDE.md）----

/**
 * 创建页签右键菜单项构建器（纯函数导出供 L2 直测）。
 * 单宿主下右键面板属主页 = 菜单目标页（页前缀协议解析，兜底入参 pageId）；
 * action 内面板/组引用取右键瞬间快照，行为与 dockview 6.6.1 原生菜单零漂移。
 * @param getApi 返回宿主 dockview api（宿主 onReady 后恒非空）
 * @param pageId 菜单构建时已知页（panelId 无页前缀的测试/防御形态兜底）
 */
export function createTabMenuItems(
  getApi: () => DockviewApi | null,
  pageId: string | null,
  onRenameRequest: (panel: TabMenuPanel) => void,
  /** 页面所属项目根查询——「复制相对路径」基准（区别于浏览 cwd，每页恒定）。
   * 传回调而非静态值：单宿主下右键目标页在工厂构建期未知，须在 action 内按解析页现取
   * （S11 共享宿主改造曾漏传致恒输出绝对路径——回归修复）。 */
  getProjectRootPath?: (pageId: string | null) => string | undefined,
): (panel: TabMenuPanel) => TabMenuItem[] {
  return (panel: TabMenuPanel) => {
    // 单宿主右键目标必在可见（活跃）页组；面板属主页解析（防御跨页组残留）
    const menuPageId = resolvePageId(panel.id, pageId);
    // 仅终端面板显示「重命名」：判据为 view.contentComponent（panel.component 不存在）
    const isTerminal = panel.view.contentComponent === PANEL_TERMINAL;
    // claude 运行中（agentSession 存在即运行中，二态模型）→ 禁用重命名；
    // 菜单每次右键重新构建，判断实时
    const claudeRunning = TerminalRegistry.get(panel.id)?.agentSession != null;
    // 文件型页签判据 = params.filePath 存在（与 DefaultTab 文件图标判据同源，TAB-03）
    // ——仅文件型页签显示「复制相对路径」
    const filePath = (panel.params as TabParams | undefined)?.filePath;
    const item = (
      label: string,
      opts: { danger?: boolean; disabled?: boolean; action: () => void },
    ): TabMenuItem => ({ label, ...opts });

    const items: TabMenuItem[] = [
      item("新建终端", {
        action: () => {
          // FE-04: 点击时才分配编号（延迟到 action 执行，而非菜单构建时）
          const api = getApi();
          if (!api) return; // 理论不可达——右键必在宿主就绪后
          // 目标页 = 面板属主页（页前缀解析），无前缀面板（e2e 裸 id/防御形态）
          // 兜底活跃页——右键必发生在可见（活跃）页组
          const target = menuPageId ?? useLayout.getState().activePageId;
          if (!target) return;
          // 新面板落 target 页组（addTerminalPanel 显式 position referenceGroup）
          void addTerminalPanel(api, target, undefined);
        },
      }),
      "separator",
    ];
    if (typeof filePath === "string" && filePath.length > 0) {
      items.unshift(
        item("复制相对路径", {
          action: () =>
            copyRelativePath(filePath, getProjectRootPath?.(menuPageId)),
        }),
        "separator",
      );
    }
    if (isTerminal) {
      items.push(
        item("重命名", {
          disabled: claudeRunning,
          action: () => onRenameRequest(panel),
        }),
        "separator",
      );
    }
    // 关闭类 = 危险项（UI-802 ERROR_FG）；组级操作经右键瞬间的 panel.api.group
    items.push(
      item("关闭", {
        danger: true,
        // FE-49: 单面板「关闭」与 ×/Ctrl+W/中键同走共享守卫 closeTabGuarded——
        // panelId 取 params（判据同 DefaultTab 的 settings 面板形态，同源无漂移）
        action: () => {
          void closeTabGuarded(
            panel.api,
            (panel.params as TabParams | undefined)?.panelId,
          );
        },
      }),
      item("关闭其他", {
        danger: true,
        // CP-036：批量路径接入 closeTabsGuarded——dirty 面板列表 + 单次确认统一入口
        action: () => {
          const group = panel.api.group;
          if (!group) return;
          void closeTabsGuarded(
            group.panels
              .filter((p) => p !== panel)
              .map((p) => ({
                api: p.api,
                panelId: (p.params as TabParams | undefined)?.panelId,
                title: p.title ?? "",
              })),
          );
        },
      }),
      item("关闭全部", {
        danger: true,
        // CP-036：同「关闭其他」——不过滤自身，组内全部面板经统一入口
        action: () => {
          const group = panel.api.group;
          if (!group) return;
          void closeTabsGuarded(
            [...group.panels].map((p) => ({
              api: p.api,
              panelId: (p.params as TabParams | undefined)?.panelId,
              title: p.title ?? "",
            })),
          );
        },
      }),
    );
    return items;
  };
}

// ---- 辅助函数（宿主生命周期消费——WorkspaceDockHost） ----

/** 将 TitleUpdate[] 应用到 DockviewApi（批量 setTitle） */
export function applyTitleUpdates(
  api: DockviewApi,
  updates: TitleUpdate[],
): void {
  for (const { panelId, title } of updates) {
    const panel = api.getPanel(panelId);
    if (panel) panel.api.setTitle(title);
  }
}

/**
 * 页布局恢复后重建标题注册表并重算（宿主 restore/页组并入后逐页调用）：
 * - 终端 pass：恢复的终端面板（无 customTitle）用 titleManager 重算编号——
 *   持久化 title 可能是瞬态值（如 claude 运行中退出保存的 "claude"），恢复后
 *   必须回 terminal-N；F8 自定义名（customTitle）保留。
 * - 编辑器注册：文件型面板（params.filePath）按页登记后重算冲突标题。
 * @param panelIds 该页全部面板 id（panelsOfPage 输出）
 */
export function rebuildAndRecomputeTitles(
  api: DockviewApi,
  pageId: string,
  rootPath: string | undefined,
  panelIds: string[],
): void {
  for (const panelId of panelIds) {
    const panel = api.getPanel(panelId);
    if (!panel) continue;
    const params = panel.params as TabParams | undefined;
    if (!params?.panelId) continue;
    if (panel.view?.contentComponent === PANEL_TERMINAL) {
      if (params.customTitle === undefined) {
        panel.api.setTitle(titleManager.getTerminalTitle(pageId));
      }
      continue;
    }
    // 文件型面板（editor/htmlviewer 等）的 params 携带 filePath
    const filePath = params.filePath;
    if (filePath !== undefined) {
      titleManager.unregisterEditor(pageId, panelId);
      titleManager.registerEditor(pageId, panelId, filePath);
    }
  }
  if (!rootPath) return;
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
  // ——editor/htmlviewer/gitshow/diff——的面板携带 filePath；terminal/settings
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
      onContextMenu={(e) => {
        // 自研页签右键菜单（dockview 8.1 free core 无 contextMenuService，库内路径恒短路）：
        // preventDefault/stopPropagation 拦 WebView 原生菜单与库内死监听，经 CustomEvent
        // 上报宿主——宿主 getPanel(panelId) 解析命中后弹 TabMenuPopup。
        // 无 panelId（裸面板/直渲染测试）→ 不拦截不弹
        const panelId = tabParams?.panelId;
        if (!panelId) return;
        e.preventDefault();
        e.stopPropagation();
        window.dispatchEvent(new CustomEvent<TabContextMenuDetail>(
          TAB_CONTEXT_MENU_EVENT,
          { detail: { panelId, x: e.clientX, y: e.clientY } },
        ));
      }}
      onAuxClick={(e) => {
        // FE-49 鼠标中键关闭页签：auxclick = 完整点击语义（auxclick 仅在同元素
        // 完成按下+弹起时触发——按下后拖离再弹起即天然取消）；目标 = 本页签自身
        // api，无需聚焦/激活（对比 Ctrl+W 的 activePanel 语义）。中键不触发 × 的
        // onClick（click 仅主键），× 上的中键经冒泡同样走本路径关闭（浏览器惯例）。
        // autoscroll 预防已由宿主容器 capture mousedown 单点拦截；
        // auxclick 无库内默认动作，preventDefault 仅防御（dockview 对 auxclick
        // 零消费，无需 stopPropagation）
        if (e.button !== 1) return;
        e.preventDefault();
        void closeTabGuarded(api, tabParams?.panelId);
      }}
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
        // FE-04: E2E 关闭按钮选择器——panelId 存在时 `tab-close-{panelId}`，
        // 否则 "tab-close"（模板字符串，供 wdio 精确定位）
        data-e2e={tabParams?.panelId ? `tab-close-${tabParams?.panelId}` : "tab-close"}
        onClick={(e) => {
          e.stopPropagation();
          // SC-FE-07 × 关闭守卫：判据与确认逻辑已统一迁至 tabClose.ts 的
          // closeTabGuarded（FE-49）——× / Ctrl+W / 鼠标中键 / 右键菜单「关闭」
          // 四路共用同一入口，防多路守卫漂移（语义沿革见 tabClose.ts 头注释）
          void closeTabGuarded(api, tabParams?.panelId);
        }}
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

/**
 * 宿主级 dockview 主题/容器样式（单宿主唯一实例挂载点共享——变量注入 +
 * display:none 态：无活跃页（删除末页/项目移除）时整宿主隐藏，与旧
 * 「各页实例级 display」的空白主区语义一致）。
 */
export function hostContainerStyle(visible: boolean): Record<string, string> {
  return {
    ...dockviewVarStyle(),
    display: visible ? "block" : "none",
    width: "100%",
    height: "100%",
  };
}

// 页签右键菜单组件（自研 fixed 弹层——宿主单点持有菜单态，渲染随宿主）
export { TabMenuPopup, TerminalRenameDialog };
export { createRightHeader };
