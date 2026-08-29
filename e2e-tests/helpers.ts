/**
 * E2E 测试辅助——所有 E2E helper 在 DEV 模式下统一挂载。
 * 生产构建中此文件完全不被打包（main.tsx 通过动态 import 加载，
 * Vite 在 production 模式下 tree-shake 掉 import.meta.env.DEV 分支）。
 *
 * 命名约定：
 * - __slterm_e2e_* — window 全局 helper（测试脚本直接调用）
 * - __e2e_*       — 终端容器 DOM 元素 helper（随面板挂载/卸载）
 */

import React from "react";
import { EditorView } from "@codemirror/view";
import { agentHooks } from "../src/ipc";
import type { AgentHookInjectionStatus } from "../src/ipc/agentHooks";
import { writeText } from "../src/ipc/clipboard";
import { setProjectRoot } from "../src/ipc/fs";
import { writeHooksConfig } from "../src/ipc/hooksConfig";
import { getShortcutRegistry } from "../src/features/shortcuts";
import { useProjects, createProjectId, createPageId } from "../src/stores/projects";
import type { OperationPage, Project } from "../src/stores/projects";
import { useLayout } from "../src/stores/layout";
import { makeEmptyLayout } from "../src/features/navTree/NavTree";
import { titleManager } from "../src/workspace/titleManager";
import { switchToPageShared } from "../src/workspace/pageApis";
import { openSettings } from "../src/features/settingsCenter/openSettings";
import { useFontSize, FONT_SIZE_DEFAULT } from "../src/stores/fontSize";
import { useKeybindings } from "../src/stores/keybindings";
import { useSideBar } from "../src/stores/sideBar";
import { cliProfileRegistry } from "../src/features/cliProfiles";
import {
  DEFAULT_ZONES,
  DEFAULT_OPEN,
  WIDTH_DEFAULT,
  SPLIT_DEFAULT,
} from "../src/features/sideViews/sideBarState";
import type { CodingCliProfile, HooksConfigEditorProps } from "../src/features/cliProfiles/types";

// ── Window 全局类型扩展 ──

declare global {
  interface Window {
    // 工作区（Workspace 设置）
    __slterm_e2e_workspaceReady?: boolean;
    // 剪贴板
    __slterm_e2e_writeClipboard?: (text: string) => Promise<void>;
    // 诊断
    __slterm_e2e_shortcutDebug?: () => { stack: string[]; commands: string[] };
    // 项目管理
    __slterm_e2e_createProject?: (dirPath: string) => Promise<string>;
    /** 用例开始前清空项目 store（wdio beforeTest 调用；FE-36 全局页数上限兼容） */
    __slterm_e2e_resetProjects?: () => void;
    /** spec 间隔离前端配置类 store（wdio beforeSuite 调用；TQ-E-08） */
    __slterm_e2e_resetSettings?: () => void;
    __slterm_e2e_getProjectIdForPage?: (pageId: string) => string | null;
    /** 新增操作页面——被 store 拒绝（超页数上限等）返回 null */
    __slterm_e2e_addPage?: (projectId: string, name: string, rootPath: string) => string | null;
    __slterm_e2e_switchToPage?: (pageId: string) => Promise<void>;
    // 页签标题
    __slterm_e2e_registerAndRecompute?: (
      pageId: string, rootPath: string, panelId: string, filePath?: string
    ) => void;
    __slterm_e2e_getActivePageInfo?: () => { pageId: string; rootPath: string } | null;
    // 初始化竞态协调
    __slterm_e2e_projectPending?: boolean;
    // 侧栏视图（SB-25）
    __slterm_e2e_getSideBarState?: () => SideBarSnapshot | null;
    __slterm_e2e_toggleSideView?: (id: string) => void;
    __slterm_e2e_moveSideViewButton?: (id: string, zone: string, index: number) => void;
    // hooks 注入/卸载/状态（P1-TE-04；底层泛化命令 agent_hooks_*，cliId 固定 "claude"）
    __slterm_e2e_injectHooks?: () => Promise<AgentHookInjectionStatus>;
    __slterm_e2e_uninstallHooks?: () => Promise<void>;
    __slterm_e2e_getHookInjectionStatus?: () => Promise<AgentHookInjectionStatus>;
    // hooks 配置面板 JSON 模式（P3-TE-18）
    __slterm_e2e_setHooksConfigJson?: (text: string) => boolean;
    __slterm_e2e_getHooksConfigJson?: () => string | null;
    // mockcli 测试 profile 注册（Stage 07 AC-4：L4 经 E2E helper 注册）
    __slterm_e2e_registerMockCliProfile?: () => void;
    // 设置中心面板（F11，SC-E2E-01）
    __slterm_e2e_openSettings?: () => Promise<void>;
    __slterm_e2e_getSettingsPanelState?: () => { selectedPage: string | null } | null;
    __slterm_e2e_getSettingsPanelCount?: () => number;
    __slterm_e2e_switchSettingsPage?: (id: string) => boolean;
  }
}

/** useSideBar.getState() 的纯数据快照（去函数键，供 browser.execute 序列化） */
interface SideBarSnapshot {
  zones: { top: string[]; bottom: string[] };
  open: { top: string | null; bottom: string | null };
  width: number;
  splitRatio: number;
  loaded: boolean;
}

// ── installAllE2eHelpers —— 主入口 ──

/** 在 window 上安装全部 E2E 辅助函数（不含终端容器级 helper） */
export function installAllE2eHelpers(): void {
  installClipboard();
  installShortcutDebug();
  installProjectHelpers();
  installSettingsHelpers();
  installSettingsPanelHelpers();
  installTitleHelpers();
  installSideBarHelpers();
  installHookHelpers();
  installHooksConfigHelpers();
  installMockCliProfile();

  // 标记 Workspace 就绪（Workspace 组件渲染时同步设置）
  window.__slterm_e2e_workspaceReady = false;
}

/** Workspace 就绪标记——由 Workspace 组件渲染阶段调用 */
export function markWorkspaceReady(): void {
  window.__slterm_e2e_workspaceReady = true;
}

// ── 终端容器级 helper ──

/** useTerminalInstance 调用的终端 E2E 上下文 */
export interface TerminalE2eContext {
  writeToTerminal: (text: string) => void;
  getTerminalText: () => string;
}

/**
 * 在终端容器 DOM 上安装 __e2e_writeToTerminal / __e2e_getTerminalText。
 * 由 useTerminalInstance 在 DEV 模式下调用。
 */
export function initTerminalE2e(container: HTMLElement, ctx: TerminalE2eContext): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const h = container as any;
  h.__e2e_writeToTerminal = ctx.writeToTerminal;
  h.__e2e_getTerminalText = ctx.getTerminalText;
}

/**
 * 在终端容器上安装 __e2e_writeToPty。
 * 由 useXterm 在 DEV 模式下调用。
 */
export function installTerminalWriteToPty(
  container: HTMLElement,
  writeFn: (data: string) => void,
): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (container as any).__e2e_writeToPty = writeFn;
}

/**
 * 设置终端 session 就绪标记 __e2e_sessionReady 和 __e2e_error。
 * 由 useXterm 在 DEV 模式下调用。
 */
export function setTerminalSessionReady(container: HTMLElement, ready: boolean): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (container as any).__e2e_sessionReady = ready;
}

/** 设置终端 spawn 错误信息 */
export function setTerminalSessionError(container: HTMLElement, error: string): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (container as any).__e2e_error = error;
}

// ── 私有实现 ──

/** __slterm_e2e_writeClipboard */
function installClipboard(): void {
  window.__slterm_e2e_writeClipboard = writeText;
}

/** __slterm_e2e_shortcutDebug */
function installShortcutDebug(): void {
  window.__slterm_e2e_shortcutDebug = () => {
    const r = getShortcutRegistry();
    return { stack: r._contextStack(), commands: r.listCommands().map((c) => c.id) };
  };
}

/** __slterm_e2e_createProject / __slterm_e2e_addPage / __slterm_e2e_switchToPage / __slterm_e2e_getProjectIdForPage */
function installProjectHelpers(): void {
  // __slterm_e2e_createProject —— 程序化创建测试项目（绕过原生对话框）
  window.__slterm_e2e_createProject = async (dirPath: string) => {
    window.__slterm_e2e_projectPending = true; // 阻止 localStorage 恢复覆盖
    const name = dirPath.split(/[/\\]/).pop() || dirPath;
    const projectId = createProjectId();
    const pageId = createPageId();

    const page: OperationPage = {
      pageId,
      name,
      layout: makeEmptyLayout(),
      cwd: dirPath,
      createdAt: Date.now(),
      lastAccessedAt: Date.now(),
    };

    const project: Project = {
      projectId,
      name,
      rootPath: dirPath,
      pages: [page],
      activePageId: pageId,
      version: 1,
    };

    useProjects.getState().addProject(project);
    // DBG-8: setProjectRoot 必须在 setActivePage 之前（路径沙箱前置条件）
    try {
      await setProjectRoot(dirPath);
    } catch (err) {
      console.error("[slTerminal e2e] 设置项目根路径失败:", err);
    }
    useLayout.getState().setActivePage(pageId);
    return pageId;
  };

  // __slterm_e2e_resetProjects —— 用例开始前清空项目 store（wdio beforeTest 调用）。
  // 单 session 共享 app 实例（wdio.conf 文件头注释）：前序 spec/用例的项目在
  // store 累积（一轮可 20+ 项目/30+ 页），S06 FE-36 全局页数上限（MAX_PAGES=20）
  // 会拒绝后续 addPage（H6/E2E-04 回归根因）。清空粒度 = 用例级——用例内
  // 多项目累积不受影响（agent R2 切项目往返等）。
  window.__slterm_e2e_resetProjects = () => {
    useProjects.setState({ projects: {}, expandedNodes: {} });
    // 同步清 activePageId——残留指向被清项目的 pageId 会使
    // __slterm_e2e_getActivePageInfo 断链（「无法获取活跃页面信息」）
    useLayout.setState({ activePageId: null });
  };

  // __slterm_e2e_getProjectIdForPage
  window.__slterm_e2e_getProjectIdForPage = (pageId: string) => {
    const { projects } = useProjects.getState();
    for (const [projId, proj] of Object.entries(projects)) {
      if (proj.pages.some((p) => p.pageId === pageId)) {
        return projId;
      }
    }
    return null;
  };

  // __slterm_e2e_addPage —— 在已有项目中新增操作页面（H6 跨页面存活测试）
  // 返回 null 表示被 store 拒绝（项目不存在/页面总数超 MAX_PAGES 上限）——
  // spec 侧 addPage 返回值断言可提前失败，避免切换幽灵页面的隐性超时
  window.__slterm_e2e_addPage = (projectId: string, name: string, rootPath: string) => {
    const pageId = createPageId();
    const page: OperationPage = {
      pageId,
      name,
      layout: makeEmptyLayout(),
      cwd: rootPath,
      createdAt: Date.now(),
      lastAccessedAt: Date.now(),
    };
    const ok = useProjects.getState().addPage(projectId, page);
    if (!ok) return null;
    return pageId;
  };

  // __slterm_e2e_switchToPage —— 切换活跃页面（H6 跨页面切换验证）
  // 委托 switchToPageShared 统一处理 setProjectRoot → setActivePage → __dockviewApi 重指向
  window.__slterm_e2e_switchToPage = async (pageId: string) => {
    await switchToPageShared(pageId);
  };
}

/**
 * __slterm_e2e_resetSettings —— spec 间隔离前端配置类 store（TQ-E-08）。
 * 后端 settings.json 由 run-wdio.cjs 备份/还原做进程级隔离；此处管
 * 同一 run 内跨 spec 的 Zustand 内存态（keybindings/sideBar/fontSize）。
 * 不清 hooks 注入状态——hooks.e2e.ts 依赖 ensureHooksInjected 的幂等。
 */
function installSettingsHelpers(): void {
  window.__slterm_e2e_resetSettings = () => {
    // keybindings：清空用户覆盖 → 全部回退默认键
    useKeybindings.setState({ overrides: {}, loaded: true });
    // sideBar：回到生产默认态（DEFAULT_ZONES/DEFAULT_OPEN/WIDTH_DEFAULT/SPLIT_DEFAULT）
    useSideBar.setState({
      zones: { top: [...DEFAULT_ZONES.top], bottom: [...DEFAULT_ZONES.bottom] },
      open: { ...DEFAULT_OPEN },
      width: WIDTH_DEFAULT,
      splitRatio: SPLIT_DEFAULT,
      loaded: true,
    });
    // fontSize：terminal/editor 双字号回默认（FONT_SIZE_DEFAULT=14）
    useFontSize.setState({
      terminalFontSize: FONT_SIZE_DEFAULT,
      editorFontSize: FONT_SIZE_DEFAULT,
    });
  };
}

/**
 * __slterm_e2e_openSettings / __slterm_e2e_getSettingsPanelState /
 * __slterm_e2e_getSettingsPanelCount / __slterm_e2e_switchSettingsPage
 *
 * 设置中心面板 E2E 后门（F11，SC-E2E-01）：
 * - openSettings 直接复用生产编排（无项目 toast / 目标页面选择 / 切页 →
 *   openSettingsPanel 同页单例），等价活动栏配置钮点击
 * - 面板状态/计数经 window.__dockviewApi（活跃页面实例）查 settings- 前缀面板
 *   （panelId 契约 SC-FE-02：settings-{pageId}）读 params.selectedPage——
 *   选中配置页随布局持久化的真值源
 * - 切配置页走 DOM 点击 data-e2e="settings-nav-{id}"（与真实用户同路径，
 *   触发 handlePageSelect 全链：dirty 守卫 → persistParams）
 */
function installSettingsPanelHelpers(): void {
  // __slterm_e2e_openSettings —— 打开设置中心面板（Promise<void>，失败不抛，
  // 内部 openSettingsPanel 超时 console.warn 降级）
  window.__slterm_e2e_openSettings = () => openSettings();

  // __slterm_e2e_getSettingsPanelState —— 活跃页面 api 中 settings- 面板的
  // 选中配置页；无 api / 无面板返回 null
  window.__slterm_e2e_getSettingsPanelState = () => {
    const api = window.__dockviewApi;
    if (!api) return null;
    for (const panel of api.panels) {
      if (panel.id.startsWith("settings-")) {
        const params = panel.params as { selectedPage?: string } | undefined;
        return { selectedPage: params?.selectedPage ?? null };
      }
    }
    return null;
  };

  // __slterm_e2e_getSettingsPanelCount —— 活跃页面 api 中 settings- 面板总数
  // （同页单例断言用：正常 ≤1；无 api 返回 0）
  window.__slterm_e2e_getSettingsPanelCount = () => {
    const api = window.__dockviewApi;
    if (!api) return 0;
    let count = 0;
    for (const panel of api.panels) {
      if (panel.id.startsWith("settings-")) count += 1;
    }
    return count;
  };

  // __slterm_e2e_switchSettingsPage —— 经 DOM 点击左导航配置页项
  // （data-e2e="settings-nav-{id}"）。返回是否点击成功（导航项不存在返回 false，
  // 供 spec 侧提前失败而非隐性超时）
  window.__slterm_e2e_switchSettingsPage = (id: string): boolean => {
    const el = document.querySelector(`[data-e2e="settings-nav-${id}"]`);
    if (!(el instanceof HTMLElement)) return false;
    el.click();
    return true;
  };
}

/** __slterm_e2e_registerAndRecompute / __slterm_e2e_getActivePageInfo */
function installTitleHelpers(): void {
  // __slterm_e2e_registerAndRecompute —— 注册编辑器并重算标题
  window.__slterm_e2e_registerAndRecompute = (
    pageId: string,
    rootPath: string,
    panelId: string,
    filePath?: string,
  ) => {
    titleManager.registerEditor(pageId, panelId, filePath);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const api = window.__dockviewApi as any;
    if (api && rootPath) {
      const updates = titleManager.recomputeTitles(pageId, rootPath);
      for (const { panelId: pid, title } of updates) {
        const p = api.getPanel(pid);
        if (p) p.api.setTitle(title);
      }
    }
  };

  // __slterm_e2e_getActivePageInfo —— 获取活跃页面信息
  window.__slterm_e2e_getActivePageInfo = () => {
    const state = useProjects.getState();
    const activeId = useLayout.getState().activePageId;
    if (!activeId) return null;
    for (const [, proj] of Object.entries(state.projects)) {
      for (const page of proj.pages) {
        if (page.pageId === activeId) {
          return {
            pageId: page.pageId,
            rootPath: proj.rootPath,
          };
        }
      }
    }
    return null;
  };
}

/** __slterm_e2e_getSideBarState / __slterm_e2e_toggleSideView / __slterm_e2e_moveSideViewButton */
function installSideBarHelpers(): void {
  // __slterm_e2e_getSideBarState —— 返回 useSideBar 纯数据快照（去函数键，可安全经 browser.execute 序列化）
  window.__slterm_e2e_getSideBarState = () => {
    const state = useSideBar.getState();
    return {
      zones: { top: [...state.zones.top], bottom: [...state.zones.bottom] },
      open: { top: state.open.top, bottom: state.open.bottom },
      width: state.width,
      splitRatio: state.splitRatio,
      loaded: state.loaded,
    };
  };

  // __slterm_e2e_toggleSideView —— 等价点击活动栏按钮，走 store.toggleView（委托 toggleViewPure）
  window.__slterm_e2e_toggleSideView = (id: string) => {
    useSideBar.getState().toggleView(id);
  };

  // __slterm_e2e_moveSideViewButton —— 等价拖拽落点，走 store.moveButton（委托 moveButtonPure）
  // zone 类型为 "top" | "bottom"，调用方保证传入合法值
  window.__slterm_e2e_moveSideViewButton = (id: string, zone: string, index: number) => {
    useSideBar.getState().moveButton(id, zone as "top" | "bottom", index);
  };
}

/**
 * __slterm_e2e_injectHooks / __slterm_e2e_uninstallHooks / __slterm_e2e_getHookInjectionStatus
 *
 * 泛化命令（agent_hooks_* 六命令全表）的 E2E 后门——cliId 实参固定 "claude"：
 * E2E 辅助代码属测试基建，字面量合法；随第二 CLI 接入如需覆盖再扩展参数。
 */
function installHookHelpers(): void {
  window.__slterm_e2e_injectHooks = async () => agentHooks.inject("claude");
  window.__slterm_e2e_uninstallHooks = async () => agentHooks.uninstall("claude");
  window.__slterm_e2e_getHookInjectionStatus = async () => agentHooks.getInjectionStatus("claude");
}

/**
 * __slterm_e2e_setHooksConfigJson / __slterm_e2e_getHooksConfigJson（P3-TE-18）
 * hooks 配置面板 JSON 模式 CM6 编辑器读写。
 *
 * 定位方式：`EditorView.findFromDOM`（CM6 公共静态 API）从
 * `data-e2e="hooks-json-editor"` 容器（JsonMode 挂载点）反查 EditorView 实例，
 * 不经组件私有 ref。写入走 `view.dispatch` 全文档替换——触发真实
 * updateListener → onChange → dirty + 校验上报，与用户输入同一路径。
 */
function installHooksConfigHelpers(): void {
  /** 整文替换 JSON 模式 CM6 文档。返回是否成功注入（面板未就绪返回 false） */
  window.__slterm_e2e_setHooksConfigJson = (text: string): boolean => {
    const container = document.querySelector('[data-e2e="hooks-json-editor"]');
    if (!container) return false;
    const view = EditorView.findFromDOM(container as HTMLElement);
    if (!view) return false;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    return true;
  };

  /** 读取 JSON 模式 CM6 当前文档（切层/加载后等待内容就绪、断言编辑器内容用）。无编辑器返回 null */
  window.__slterm_e2e_getHooksConfigJson = (): string | null => {
    const container = document.querySelector('[data-e2e="hooks-json-editor"]');
    if (!container) return null;
    const view = EditorView.findFromDOM(container as HTMLElement);
    return view ? view.state.doc.toString() : null;
  };
}

/**
 * mockcli 桩编辑器（CS-3）：mockcli 定义 configEditor 字段挂载点。
 *
 * helpers.ts 为 .ts 无 JSX——用 React.createElement 构造；渲染
 * data-e2e="mockcli-config-editor" 标记（与 L2 桩 mockCliProfile.ts 同标记口径，KZ-7）。
 * 保存按钮触发真实 writeHooksConfig("mockcli", ...)——mockcli 无后端 provider，
 * invoke 必被后端拒绝（Validation「未知 cliId: mockcli」），错误经 setState 展示于
 * data-e2e="mockcli-config-error"（mockcli.e2e.ts 用例 ② 的 cliId 全链携带证据）。
 */
function MockCliConfigEditor(
  _props: HooksConfigEditorProps,
): React.ReactElement | null {
  // 桩不消费 props（无 dirty/守卫语义），显式引用规避 no-unused-vars
  void _props;
  const [error, setError] = React.useState<string | null>(null);
  const [saving, setSaving] = React.useState(false);
  const handleSave = async () => {
    setSaving(true);
    try {
      await writeHooksConfig("mockcli", "user", { mock: true });
      setError(null);
    } catch (err) {
      // Tauri 2 命令错误经 serde 外部标记序列化（AppError enum → { "<camelCase 变体>": "文案" }，
      // 如 { validation: "未知 cliId: mockcli" }）——非 Error 实例且无 message 键，String(err)
      // 只得 "[object Object]"；递归取首个字符串字段值作文案（IoKind 形态
      // { ioKind: { kind, message } } 内部优先 message 键）
      const extractErrorText = (v: unknown): string | null => {
        if (typeof v === "string") return v;
        if (typeof v !== "object" || v === null) return null;
        const o = v as Record<string, unknown>;
        if (typeof o.message === "string") return o.message;
        for (const val of Object.values(o)) {
          const text = extractErrorText(val);
          if (text !== null) return text;
        }
        return null;
      };
      setError(err instanceof Error ? err.message : extractErrorText(err) ?? String(err));
    } finally {
      setSaving(false);
    }
  };
  return React.createElement(
    "div",
    { "data-e2e": "mockcli-config-editor" },
    React.createElement(
      "button",
      {
        type: "button",
        "data-e2e": "mockcli-config-save",
        disabled: saving,
        onClick: handleSave,
      },
      "保存（mockcli 桩）",
    ),
    error === null
      ? null
      : React.createElement("div", { "data-e2e": "mockcli-config-error" }, error),
  );
}

/**
 * __slterm_e2e_registerMockCliProfile（Stage 07 AC-4：mock profile 的 L4 注册入口）
 *
 * mockcli 是测试夹具而非真实 CLI（spec 06 §7 约定）：id/displayName/commands/
 * tabTitle 均 "mockcli"、iconSrc "/cli-icons/mockcli.png"（Stage 01 已放资源）、
 * hooks/history 全能力（恒等映射/桩策略）。仅经本 helper 注册——本文件整体在
 * E2E_ENABLED 门控内加载（main.tsx 内联 import.meta.env 字面量分支），生产构建
 * 整块 tree-shake，生产二进制无此 profile。register 幂等（同 id 覆盖），重复
 * 调用安全。
 */
function installMockCliProfile(): void {
  const mockCliProfile: CodingCliProfile = {
    id: "mockcli",
    displayName: "mockcli",
    commands: ["mockcli"],
    iconSrc: "/cli-icons/mockcli.png",
    tabTitle: "mockcli",
    capabilities: {
      hooks: {
        // 桩实现（恒等映射简化形态）：非 Stop 事件一律 working，Stop → done——
        // 供 AC-4 ②「eventToStatus 被真实调用」类用例断言
        eventToStatus: (event) => (event === "Stop" ? "done" : "working"),
        // 桩实现：一律不触发通知
        classifyNotification: () => null,
        // 百分比桩：任意 usage 恒 42（与 L2 mockCliProfile 桩同口径）
        computeUsagePercent: () => 42,
        restartHint: "mockcli 桩提示：hooks 改动需重启 mockcli 会话生效",
        hasConfigEditor: true,
        // CS-3：桩编辑器（React.createElement 构造——helpers.ts 为 .ts 无 JSX；
        // data-e2e="mockcli-config-editor" 标记与 L2 桩同口径；保存动作经真实
        // writeHooksConfig 携带 mockcli cliId——用例 ② 的 cliId 透传断言）
        configEditor: MockCliConfigEditor,
        // CS-3：桩声明单层（值集由 profile 自声明——mockcli 仅 user 层可写）
        configLayers: [{ id: "user", label: "User", hint: "mockcli 桩单层（user）" }],
      },
      history: {
        supportsFork: true,
        // 桩输出带可识别前缀 "mockcli --resume"（AC-4 ⑤ 恢复注入断言用）
        buildResumeCommand: (session) => `mockcli --resume ${session.sessionId}`,
        buildRestoreInput: (session, opts) =>
          `mockcli --resume ${session.sessionId}${opts.fork ? " --fork" : ""}\r`,
      },
    },
  };
  window.__slterm_e2e_registerMockCliProfile = () => {
    cliProfileRegistry.register(mockCliProfile);
  };
}
