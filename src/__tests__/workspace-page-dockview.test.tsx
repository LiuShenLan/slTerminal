// workspace-page-dockview.test.tsx — PageDockviewHost 真实组件集成测试（WRK-01）
//
// 真实渲染 PageDockview（真实 DockviewReact，非 mock），通过 onReady 捕获真实
// DockviewApi 驱动断言：
// - handleReady：空布局不兜底创建终端（H6/Watermark 接管）、保存布局恢复 + 标题重算、
//   损坏布局（白名单过滤后无面板）回退 Watermark
// - Watermark 按钮 addPanel（终端 id 递增、title 由 titleManager 分配）
// - RightHeader "+" 按钮 addPanel（同 group）
// - onSaveAs（slterm:file-saved-as 事件）→ handleSaveAs → 重算标题 → 真实 setTitle
// - 页签右键菜单（自研 TabMenuPopup）：对 .dv-tab 真实派发 contextmenu 驱动生产链路
//   （dockview 8.1 free core 无 contextMenuService，不依赖探针）——菜单项/坐标/关闭
//   手势/复制相对路径等断言
//
// 注：jsdom 中真实 Dockview 的 addPanel/setTitle/fromJSON 均可用（集成验证）。

import { describe, it, expect, beforeEach, afterEach, afterAll, vi } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import React from "react";
import { render, act, fireEvent, cleanup } from "@testing-library/react";

// Mock @xterm/xterm — xterm.js 6.1+ 渲染器初始化在 jsdom 中抛异常（同 workspace 测试）
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn(function (this: Record<string, unknown>) {
    this.open = vi.fn();
    this.dispose = vi.fn();
    this.loadAddon = vi.fn();
    this.write = vi.fn();
    this.writeln = vi.fn();
    this.onData = vi.fn();
    this.focus = vi.fn();
    this.attachCustomKeyEventHandler = vi.fn();
    this.element = document.createElement("div");
    this.options = {} as Record<string, unknown>;
    this.parser = { registerOscHandler: vi.fn(() => ({ dispose: vi.fn() })) };
    return this;
  }),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn(function (this: Record<string, unknown>) {
    this.fit = vi.fn();
    this.proposeDimensions = vi.fn(() => ({ cols: 80, rows: 24 }));
    this.dispose = vi.fn();
    return this;
  }),
}));

// ─── Hoisted clipboard mock（「复制相对路径」用例断言写剪贴板）───
const mocks = vi.hoisted(() => {
  const mockWriteText = vi.fn();
  return {
    mockWriteText,
    resetClipboard() {
      mockWriteText.mockReset();
      mockWriteText.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../ipc/clipboard", () => ({
  writeText: mocks.mockWriteText,
  readText: vi.fn().mockResolvedValue(""),
}));

// 模块级 stub 须 afterAll 恢复——防同 worker 后续文件被污染（TQ-A-02）
const originalResizeObserver = global.ResizeObserver;
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};
afterAll(() => {
  global.ResizeObserver = originalResizeObserver;
});

import PageDockview from "../workspace/PageDockviewHost";
import { titleManager } from "../workspace/titleManager";
import { resetTerminalPanelSeq } from "../lib/panelId";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyApi = any;

const PAGE_ID = "page-dock";

/** 含一个 editor 面板的合法保存布局（面板 params 携带 filePath，供标题重算） */
const EDITOR_LAYOUT = {
  grid: {
    root: { type: "branch", data: [{ type: "leaf", data: { views: ["editor-1"] } }] },
    orientation: "HORIZONTAL",
  },
  panels: {
    "editor-1": {
      id: "editor-1",
      contentComponent: "editor",
      title: "a.txt",
      position: { group: "group-1" },
      params: { panelId: "editor-1", filePath: "C:\\root\\a.txt" },
    },
  },
  activeGroup: "group-1",
};

/** 含一个终端面板的合法保存布局（title 为瞬态值 "claude"——claude 运行中退出保存形态，
 *  B12：恢复时无 customTitle 的终端应重算为 terminal-N） */
const TERMINAL_TRANSIENT_LAYOUT = {
  grid: {
    root: { type: "branch", data: [{ type: "leaf", data: { views: ["terminal-page-dock-0"] } }] },
    orientation: "HORIZONTAL",
  },
  panels: {
    "terminal-page-dock-0": {
      id: "terminal-page-dock-0",
      contentComponent: "terminal",
      title: "claude",
      position: { group: "group-1" },
      params: { panelId: "terminal-page-dock-0", cwd: "C:\\root" },
    },
  },
  activeGroup: "group-1",
};

/** 含 customTitle 终端面板的合法保存布局（B12/F8：重算条件排除 customTitle，自定义名保留） */
const TERMINAL_CUSTOM_LAYOUT = {
  grid: {
    root: { type: "branch", data: [{ type: "leaf", data: { views: ["terminal-page-dock-0"] } }] },
    orientation: "HORIZONTAL",
  },
  panels: {
    "terminal-page-dock-0": {
      id: "terminal-page-dock-0",
      contentComponent: "terminal",
      title: "我的终端",
      position: { group: "group-1" },
      params: { panelId: "terminal-page-dock-0", cwd: "C:\\root", customTitle: "我的终端" },
    },
  },
  activeGroup: "group-1",
};

/** 含未知面板类型的损坏布局（loadLayout 白名单过滤后无剩余面板） */
const UNKNOWN_PANEL_LAYOUT = {
  grid: {
    root: { type: "branch", data: [{ type: "leaf", data: { views: ["weird-1"] } }] },
    orientation: "HORIZONTAL",
  },
  panels: {
    "weird-1": {
      id: "weird-1",
      contentComponent: "not-a-panel-type",
      title: "weird",
      position: { group: "group-1" },
    },
  },
  activeGroup: "group-1",
};

beforeEach(() => {
  titleManager.reset();
  // B14: 模块级每页计数隔离（nextPanelId 消费 makeTerminalPanelId 的共享计数）
  resetTerminalPanelSeq();
  // TQ-COV-08: 右键菜单「重命名」disabled 判据经 TerminalRegistry 会话状态
  TerminalRegistry._reset();
  mocks.resetClipboard();
});

afterEach(() => {
  // RTL 无 auto-cleanup（vitest globals 未开启）——Dockview 实例跨测试残留会
  // 污染按钮查询与全局事件监听，必须显式清理
  cleanup();
  clearMocks();
  // 注：页签菜单自研渲染于容器内（无 body 挂载残留），cleanup() 已随组件卸载
  // 清除其 document/window 关闭监听
});

/** 等待 Dockview 渲染 settle（onReady → watermark/header 渲染为异步） */
async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => requestAnimationFrame(resolve));
    await Promise.resolve();
  });
}

/** 渲染 PageDockview 并等待 onReady（返回捕获的 api + container） */
async function renderDock(overrides: Record<string, unknown> = {}) {
  let capturedApi: AnyApi = null;
  const onLayoutChange = vi.fn();
  const result = render(
    React.createElement(PageDockview, {
      pageId: PAGE_ID,
      cwd: "C:\\root",
      rootPath: "C:\\root",
      savedLayout: undefined,
      visible: true,
      onReady: (api: AnyApi) => { capturedApi = api; },
      onLayoutChange,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ...(overrides as any),
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any),
  );
  await settle();
  expect(capturedApi).toBeTruthy();
  // 空布局语义：onReady 后不兜底创建任何面板（H6/Watermark 接管）。
  // 传 savedLayout 的用例跳过此检查（恢复的面板是预期的）。
  if (overrides.savedLayout === undefined && capturedApi.panels.length > 0) {
    expect.fail(
      `onReady 后应有 0 个面板，实际 ${capturedApi.panels.length} 个: ` +
      capturedApi.panels.map((p: AnyApi) => p.id).join(","),
    );
  }
  return { api: capturedApi, container: result.container, onLayoutChange };
}

/** 点击容器中文本匹配的按钮（StrictMode 双渲染取最后一个） */
function clickButton(container: HTMLElement, text: string): void {
  const btns = Array.from(container.querySelectorAll("button")).filter(
    (b) => b.textContent === text,
  );
  if (btns.length === 0) {
    expect.fail(`找不到按钮「${text}」。HTML: ${container.innerHTML.slice(0, 5000)}`);
  }
  fireEvent.click(btns[btns.length - 1]);
}

// ---- 页签右键菜单（自研真触发）----
//
// dockview-core 8.1.0 的 ContextMenu 是 enterprise 模块（free core 不注册
// contextMenuService，「getTabContextMenuItems」路径恒短路）——生产右键菜单由
// DefaultTab onContextMenu → PageDockview 状态 → TabMenuPopup 自绘（见
// workspace/CLAUDE.md）。本测试对 .dv-tab 真实派发 contextmenu 驱动生产链路，
// 无需 fake service 探针：菜单渲染于 PageDockview 容器内，[role="menuitem"] 查询。

/** 查询当前打开的页签右键菜单项（自研 TabMenuPopup 渲染于 dockview 容器内） */
function getMenuItemsIn(container: HTMLElement): () => Element[] {
  return () => Array.from(container.querySelectorAll("[role='menuitem']"));
}

/** 右键第一个页签（真实 contextmenu 事件 → DefaultTab onContextMenu → TabMenuPopup）。
    坐标可传入，供菜单 fixed 定位断言。
    注：右键派发目标必须是 DefaultTab 内容根——.dv-tab 是 DefaultTab div 的父级，
    对 .dv-tab 自身派发冒泡向上不经过 DefaultTab div（生产用户点击命中的是页签内容区，
    即 data-e2e=tab-close 按钮的父级） */
async function openTabContextMenuAt(x = 120, y = 80, tabIndex = 0): Promise<void> {
  await act(async () => {
    const tab = document.querySelectorAll(".dv-tab")[tabIndex] as Element | undefined;
    const closeBtn = tab?.querySelector("[data-e2e^='tab-close']");
    const target = (closeBtn?.parentElement ?? tab) as Element;
    fireEvent.contextMenu(target, {
      clientX: x,
      clientY: y,
    });
    await new Promise((r) => setTimeout(r, 0));
  });
}

/** 右键第一个页签（默认坐标） */
async function openTabContextMenu(): Promise<void> {
  await openTabContextMenuAt();
}

// FileIcon 独有特征（features/explorer/FileIcon.tsx 自绘坐标——区分于
// dockview 自身图标/StatusDot 等其它 svg）：
// - 14×14 svg + viewBox="0 0 14 14"
// - 文件轮廓 path（右上折角 + 弧线收角，含 a 弧线命令）
const FILE_ICON_OUTLINE =
  "M3.5 1.5H8L11 4.5v6.5a1.5 1.5 0 0 1-1.5 1.5h-6A1.5 1.5 0 0 1 2 11V3a1.5 1.5 0 0 1 1.5-1.5Z";
function isFileIconSvg(svg: SVGSVGElement): boolean {
  if (
    svg.getAttribute("width") !== "14" ||
    svg.getAttribute("viewBox") !== "0 0 14 14"
  ) {
    return false;
  }
  return Array.from(svg.querySelectorAll("path")).some(
    (p) => p.getAttribute("d") === FILE_ICON_OUTLINE,
  );
}

describe("PageDockview 真实组件", () => {
  describe("handleReady", () => {
    it("空布局：不兜底创建终端（api.panels 为空 + Watermark 显示）", async () => {
      mockIPC(() => null);
      const { api, container } = await renderDock();
      expect(api.panels.length).toBe(0);
      expect(container.textContent).toContain("打开终端或编辑器开始工作");
      expect(container.textContent).not.toMatch(/terminal-/);
    });

    it("恢复保存布局：面板恢复 + 标题经 titleManager 重算（忽略持久化 title）", async () => {
      mockIPC(() => null);
      const { api, container } = await renderDock({ savedLayout: EDITOR_LAYOUT });
      const panel = api.getPanel("editor-1");
      expect(panel).toBeTruthy();
      // rebuildAndRecomputeTitles：从 params.filePath 重算 basename
      expect(panel.api.title).toBe("a.txt");
      // Watermark 不再显示（有内容面板）
      expect(container.textContent).not.toContain("打开终端或编辑器开始工作");
    });

    it("B12: 恢复含瞬态标题的终端 → 无 customTitle 重算为 terminal-N", async () => {
      mockIPC(() => null);
      const { api } = await renderDock({ savedLayout: TERMINAL_TRANSIENT_LAYOUT });
      const panel = api.getPanel("terminal-page-dock-0");
      expect(panel).toBeTruthy();
      // 持久化 title "claude"（瞬态值）被 titleManager 重算覆盖（B12——
      // 重开页签残留 claude 根因）
      expect(panel.api.title).toBe("terminal-0");
    });

    it("B12: 恢复含 customTitle 的终端 → 自定义名保留不重算（F8）", async () => {
      mockIPC(() => null);
      const { api } = await renderDock({ savedLayout: TERMINAL_CUSTOM_LAYOUT });
      const panel = api.getPanel("terminal-page-dock-0");
      expect(panel).toBeTruthy();
      // F8：customTitle 是用户自定义名，重算条件排除
      expect(panel.api.title).toBe("我的终端");
    });

    it("损坏布局（白名单过滤后无面板）：loadLayout 返回 false → Watermark 接管且不崩溃", async () => {
      mockIPC(() => null);
      const { api, container } = await renderDock({ savedLayout: UNKNOWN_PANEL_LAYOUT });
      expect(api.panels.length).toBe(0);
      expect(container.textContent).toContain("打开终端或编辑器开始工作");
    });
  });

  describe("Watermark 按钮", () => {
    it("点击「新建终端」→ addPanel 真实创建终端面板（titleManager 编号分配）", async () => {
      mockIPC(() => null);
      const { api, container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });

      const panel = api.getPanel(`terminal-${PAGE_ID}-0`);
      expect(panel).toBeTruthy();
      // title 由 titleManager.getTerminalTitle 分配（terminal-N 每页独立）
      expect(panel.api.title).toBe("terminal-0");
      // 面板创建后 watermark 消失（不重复显示——正确行为）
      await settle();
      expect(container.textContent).not.toContain("打开终端或编辑器开始工作");
    });
  });

  describe("RightHeader「+」按钮", () => {
    it("点击「+」→ addPanel 到当前 group（id 递增 + referenceGroup）", async () => {
      mockIPC(() => null);
      const { api, container } = await renderDock();
      // 先经 Watermark 按钮创建第一个面板（递增 nextPanelId 编号器，
      // 使 "+" 创建的下一个面板 id 递增为 terminal-1）
      await act(async () => { clickButton(container, "新建终端"); });
      await settle(); // watermark 消失、group header（含 "+"）渲染完成
      await act(async () => { clickButton(container, "+"); });
      const p1 = api.getPanel(`terminal-${PAGE_ID}-1`);
      expect(p1).toBeTruthy();
      expect(p1.api.title).toBe("terminal-1");
      // 两个面板同属一个 group（未分屏）
      expect(api.panels.length).toBe(2);
    });
  });

  describe("页签形态（TAB-03：文件页签 FileIcon 集成渲染）", () => {
    it("恢复文件布局 → 页签渲染 FileIcon 彩色图标（轮廓特征 path）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock({ savedLayout: EDITOR_LAYOUT });
      // 双特征过滤（14×14 viewBox + 文件轮廓 path）后即 FileIcon 本体
      const fileIconSvgs = Array.from(container.querySelectorAll("svg")).filter(
        isFileIconSvg,
      );
      expect(fileIconSvgs.length).toBeGreaterThan(0);
    });

    it("反向：terminal 面板页签不渲染 FileIcon（filePath 缺席——TAB-03 判据互斥）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock({
        savedLayout: TERMINAL_TRANSIENT_LAYOUT,
      });
      // 终端页签无 filePath → 无 FileIcon：特征 svg 应为 0（其它 svg 不受影响）
      const fileIconSvgs = Array.from(container.querySelectorAll("svg")).filter(
        isFileIconSvg,
      );
      expect(fileIconSvgs.length).toBe(0);
    });
  });

  describe("onSaveAs（slterm:file-saved-as 事件）", () => {
    it("派发事件 → handleSaveAs 更新路径 → 冲突重算为相对路径标题", async () => {
      mockIPC(() => null);
      const { api } = await renderDock();
      // 两个不同目录的同名文件（真实面板 + titleManager 注册）
      await act(async () => {
        api.addPanel({
          id: "editor-1",
          component: "editor",
          title: "a.txt",
          params: { panelId: "editor-1", filePath: "C:\\root\\a.txt" },
        });
        api.addPanel({
          id: "editor-2",
          component: "editor",
          title: "a.txt",
          params: { panelId: "editor-2", filePath: "C:\\root\\other\\a.txt" },
        });
      });
      titleManager.registerEditor(PAGE_ID, "editor-1", "C:\\root\\a.txt");
      titleManager.registerEditor(PAGE_ID, "editor-2", "C:\\root\\other\\a.txt");

      // editor-1 另存为到 sub/a.txt → 与 editor-2 同名 → 双方标题重算为相对路径
      await act(async () => {
        window.dispatchEvent(new CustomEvent("slterm:file-saved-as", {
          detail: {
            panelId: "editor-1",
            oldPath: "C:\\root\\a.txt",
            newPath: "C:\\root\\sub\\a.txt",
          },
        }));
      });

      expect(api.getPanel("editor-1").api.title).toBe("sub/a.txt");
      expect(api.getPanel("editor-2").api.title).toBe("other/a.txt");
    });

    it("rootPath 为空 → 事件被忽略（不崩溃）", async () => {
      mockIPC(() => null);
      const { api } = await renderDock({ rootPath: undefined });
      titleManager.registerEditor(PAGE_ID, "editor-1", "C:\\root\\a.txt");
      await act(async () => {
        window.dispatchEvent(new CustomEvent("slterm:file-saved-as", {
          detail: { panelId: "editor-1", oldPath: "C:\\root\\a.txt", newPath: "C:\\root\\sub\\a.txt" },
        }));
      });
      expect(api).toBeTruthy();
    });
  });

  // ============================================================
  // TQ-COV-08 覆盖补测（2026-08）——页签右键菜单真实链路（fake contextMenuService
  // 驱动：dockview 8.1.0 free core 无 ContextMenu 模块，右键真实触发 getTabContextMenuItems
  // 后由探针渲染菜单项并派发点击；断言全部为用户可见行为）
  // ============================================================

  /** 创建两个终端面板（watermark 按钮 + "+" 按钮），返回 api + container */
  async function twoTerminals() {
    const rendered = await renderDock();
    await act(async () => { clickButton(rendered.container, "新建终端"); });
    await settle();
    await act(async () => { clickButton(rendered.container, "+"); });
    await settle();
    return rendered;
  }

  describe("页签右键菜单（TQ-COV-08）", () => {
    it("右键终端页签 → 真实菜单构建渲染 5 项（新建终端/重命名/关闭/关闭其他/关闭全部）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      // 用户可见：菜单项渲染于 DOM（7 项结构 minus 分隔线）
      expect(getItems().map((m) => m.textContent)).toEqual([
        "新建终端", "重命名", "关闭", "关闭其他", "关闭全部",
      ]);
    });

    it("点击「重命名」→ 弹窗预填当前标题 → 输入新名确定 → 标题/customTitle 更新 + 显式保存", async () => {
      mockIPC(() => null);
      const { api, container, onLayoutChange } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      const panelId = `terminal-${PAGE_ID}-0`;
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const rename = getItems().find((m) => m.textContent === "重命名") as Element;
      await act(async () => { fireEvent.click(rename); });
      await settle();
      // 用户可见：TerminalRenameDialog 弹窗出现（含输入框与确定按钮）
      const input = container.querySelector("input") as HTMLInputElement;
      expect(input).toBeTruthy();
      expect(input.value).toBe("terminal-0"); // 预填当前标题（customTitle 优先）
      const confirmBtn = Array.from(container.querySelectorAll("button")).find(
        (b) => b.textContent === "确定",
      ) as HTMLButtonElement;
      expect(confirmBtn).toBeTruthy();
      onLayoutChange.mockClear();
      await act(async () => {
        fireEvent.change(input, { target: { value: "我的终端" } });
        fireEvent.click(confirmBtn);
      });
      await settle();
      // 用户可见：页签标题更新为自定义名
      expect(api.getPanel(panelId).api.title).toBe("我的终端");
      // customTitle 写入 params（随布局 JSON 持久化的单一真值源）
      expect(api.getPanel(panelId).params.customTitle).toBe("我的终端");
      // applyRename 显式 onLayoutChange(saveLayout(api))——最近一次保存的布局
      // 必须已含新 customTitle（setTitle/updateParameters 不触发 onDidLayoutChange，
      // 此保存即持久化证据；不数精确调用次数——面板创建期的 debounce 事件可能迟到）
      const lastSaved = onLayoutChange.mock.calls[onLayoutChange.mock.calls.length - 1]?.[0] as
        | { panels?: Record<string, { params?: Record<string, unknown> }> }
        | undefined;
      expect(lastSaved?.panels?.[panelId]?.params?.customTitle).toBe("我的终端");
      // 弹窗关闭
      expect(container.querySelector("input")).toBeNull();
    });

    it("恢复 customTitle 终端 → 重命名弹窗预填 customTitle（F8 优先）", async () => {
      mockIPC(() => null);
      const { api, container, onLayoutChange } = await renderDock({
        savedLayout: TERMINAL_CUSTOM_LAYOUT,
      });
      const panelId = "terminal-page-dock-0";
      expect(api.getPanel(panelId).api.title).toBe("我的终端");
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const rename = getItems().find((m) => m.textContent === "重命名") as Element;
      await act(async () => { fireEvent.click(rename); });
      await settle();
      const input = container.querySelector("input") as HTMLInputElement;
      // 用户可见：预填 = 既有 customTitle（非瞬态标题）
      expect(input.value).toBe("我的终端");
      const confirmBtn = Array.from(container.querySelectorAll("button")).find(
        (b) => b.textContent === "确定",
      ) as HTMLButtonElement;
      onLayoutChange.mockClear();
      await act(async () => {
        fireEvent.change(input, { target: { value: "我的终端2" } });
        fireEvent.click(confirmBtn);
      });
      await settle();
      expect(api.getPanel(panelId).api.title).toBe("我的终端2");
      expect(api.getPanel(panelId).params.customTitle).toBe("我的终端2");
      // 显式保存的布局已含新 customTitle
      const lastSaved = onLayoutChange.mock.calls[onLayoutChange.mock.calls.length - 1]?.[0] as
        | { panels?: Record<string, { params?: Record<string, unknown> }> }
        | undefined;
      expect(lastSaved?.panels?.[panelId]?.params?.customTitle).toBe("我的终端2");
    });

    it("点击「关闭」→ 右键面板关闭（其余保留）", async () => {
      mockIPC(() => null);
      const { api, container } = await twoTerminals();
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const close = getItems().find((m) => m.textContent === "关闭") as Element;
      await act(async () => { fireEvent.click(close); });
      await settle();
      // 用户可见：右键的第一个终端被关闭，第二个保留
      expect(api.panels.map((p: AnyApi) => p.id)).toEqual([`terminal-${PAGE_ID}-1`]);
    });

    it("点击「关闭其他」→ 仅保留右键面板", async () => {
      mockIPC(() => null);
      const { api, container } = await twoTerminals();
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const closeOthers = getItems().find((m) => m.textContent === "关闭其他") as Element;
      await act(async () => { fireEvent.click(closeOthers); });
      await settle();
      expect(api.panels.map((p: AnyApi) => p.id)).toEqual([`terminal-${PAGE_ID}-0`]);
    });

    it("点击「关闭全部」→ 面板清空 + Watermark 空态回归", async () => {
      mockIPC(() => null);
      const { api, container } = await twoTerminals();
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const closeAll = getItems().find((m) => m.textContent === "关闭全部") as Element;
      await act(async () => { fireEvent.click(closeAll); });
      await settle();
      // 用户可见：面板全关 + 空白页由 Watermark 接管
      expect(api.panels.length).toBe(0);
      expect(container.textContent).toContain("打开终端或编辑器开始工作");
    });

    it("菜单项 hover → SECONDARY_BG 底；危险项（关闭类）ERROR_FG 字（UI-802）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const newTerminal = getItems().find((m) => m.textContent === "新建终端") as HTMLElement;
      // 用户可见：hover 菜单项 → ui.secondaryBg 底（#222227 → rgb(34, 34, 39)）
      await act(async () => { fireEvent.mouseEnter(newTerminal); });
      expect(newTerminal.style.background).toBe("rgb(34, 34, 39)");
      // 离开 → 还原透明
      await act(async () => { fireEvent.mouseLeave(newTerminal); });
      expect(newTerminal.style.background).toBe("transparent");
      // 危险项（关闭）→ ERROR_FG 文字（#d9706b → rgb(217, 112, 107)）
      const close = getItems().find((m) => m.textContent === "关闭") as HTMLElement;
      expect(close.style.color).toBe("rgb(217, 112, 107)");
    });

    it("重命名弹窗取消（确定旁「取消」按钮）→ 弹窗关闭且标题不变", async () => {
      mockIPC(() => null);
      const { api, container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      const panelId = `terminal-${PAGE_ID}-0`;
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const rename = getItems().find((m) => m.textContent === "重命名") as Element;
      await act(async () => { fireEvent.click(rename); });
      await settle();
      const input = container.querySelector("input") as HTMLInputElement;
      expect(input).toBeTruthy();
      await act(async () => {
        fireEvent.change(input, { target: { value: "改了名" } });
        const cancelBtn = Array.from(container.querySelectorAll("button")).find(
          (b) => b.textContent === "取消",
        ) as HTMLButtonElement;
        fireEvent.click(cancelBtn);
      });
      await settle();
      // 用户可见：弹窗关闭（输入框消失）
      expect(container.querySelector("input")).toBeNull();
      // 标题未被修改（取消不落盘）
      expect(api.getPanel(panelId).api.title).toBe("terminal-0");
    });

    it("claude 运行中（agentSession 存在）→ 重命名项 disabled：置灰 + 点击不弹窗", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      const panelId = `terminal-${PAGE_ID}-0`;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      TerminalRegistry.register(panelId, {} as any);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      TerminalRegistry.setAgentSession(panelId, { sessionId: "s1" } as any);
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const rename = getItems().find((m) => m.textContent === "重命名") as HTMLElement;
      // 用户可见：disabled 置灰样式（opacity 0.4 + pointerEvents none）
      expect(rename.style.opacity).toBe("0.4");
      expect(rename.style.pointerEvents).toBe("none");
      await act(async () => { fireEvent.click(rename); });
      await settle();
      // 点击无响应：弹窗不出现
      expect(container.querySelector("input")).toBeNull();
    });

    it("恢复编辑器布局 → 右键文件页签菜单含「复制相对路径」（无重命名），点击 → 剪贴板相对路径", async () => {
      mockIPC(() => null);
      const { container } = await renderDock({ savedLayout: EDITOR_LAYOUT });
      // renderDock 默认 rootPath "C:\\root"，filePath "C:\\root\\a.txt" → 相对 "a.txt"
      // （反斜杠经 relativePath 归一化）
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      expect(getItems().map((m) => m.textContent)).toEqual([
        "复制相对路径", "新建终端", "关闭", "关闭其他", "关闭全部",
      ]);
      const copy = getItems().find((m) => m.textContent === "复制相对路径") as Element;
      await act(async () => { fireEvent.click(copy); });
      expect(mocks.mockWriteText).toHaveBeenCalledTimes(1);
      expect(mocks.mockWriteText).toHaveBeenCalledWith("a.txt");
    });

    it("文件页签目标在项目根外 → 点击「复制相对路径」兜底写完整绝对路径", async () => {
      mockIPC(() => null);
      const outsideLayout: AnyApi = {
        ...EDITOR_LAYOUT,
        panels: {
          "editor-1": {
            ...EDITOR_LAYOUT.panels["editor-1"],
            params: { panelId: "editor-1", filePath: "C:/outside/b.txt" },
          },
        },
      };
      const { container } = await renderDock({ savedLayout: outsideLayout });
      const getItems = getMenuItemsIn(container);
      await openTabContextMenu();
      const copy = getItems().find((m) => m.textContent === "复制相对路径") as Element;
      await act(async () => { fireEvent.click(copy); });
      expect(mocks.mockWriteText).toHaveBeenCalledWith("C:/outside/b.txt");
    });

    it("菜单 fixed 定位在右键坐标处（容器 + left/top + zIndex）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      await openTabContextMenuAt(333, 222);
      const menu = container.querySelector("[data-e2e='tab-menu']") as HTMLElement;
      expect(menu).toBeTruthy();
      expect(menu.style.position).toBe("fixed");
      expect(menu.style.left).toBe("333px");
      expect(menu.style.top).toBe("222px");
      expect(menu.style.zIndex).toBe("1000");
    });

    it("点击菜单外区域 → 菜单关闭（document mousedown）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      await openTabContextMenu();
      expect(container.querySelector("[data-e2e='tab-menu']")).toBeTruthy();
      // 点在菜单容器外的 root（PageDockview 根是菜单容器祖先，contains 不成立）
      await act(async () => { fireEvent.mouseDown(container); });
      expect(container.querySelector("[data-e2e='tab-menu']")).toBeNull();
    });

    it("Escape → 菜单关闭", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      await openTabContextMenu();
      expect(container.querySelector("[data-e2e='tab-menu']")).toBeTruthy();
      await act(async () => {
        fireEvent.keyDown(window, { key: "Escape" });
      });
      expect(container.querySelector("[data-e2e='tab-menu']")).toBeNull();
    });

    it("不关闭旧菜单直接右键另一页签 → 菜单重开且无残留（单份渲染）", async () => {
      mockIPC(() => null);
      const { api, container } = await twoTerminals();
      expect(api.panels.length).toBe(2);
      const getItems = getMenuItemsIn(container);
      await openTabContextMenuAt(40, 40, 0);
      expect(getItems().length).toBe(5);
      // 直接右键第二个页签（不点关闭）——旧菜单被替换而非叠加
      await openTabContextMenuAt(220, 160, 1);
      expect(container.querySelectorAll("[data-e2e='tab-menu']").length).toBe(1);
      expect(getItems().length).toBe(5);
      const menu = container.querySelector("[data-e2e='tab-menu']") as HTMLElement;
      expect(menu.style.left).toBe("220px");
      expect(menu.style.top).toBe("160px");
    });
  });

  describe("页签/按钮 hover（TQ-COV-08）", () => {
    it("× 关闭钮 hover → 背景 var(--dv-icon-hover-background-color)（TAB-02）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      const tab = container.querySelector(".dv-tab") as HTMLElement;
      const closeBtn = Array.from(tab.querySelectorAll("button")).find(
        (b) => b.textContent === "×",
      ) as HTMLButtonElement;
      await act(async () => { fireEvent.mouseEnter(closeBtn); });
      // 用户可见：hover 时显示 hover 底色（默认 none）
      expect(closeBtn.style.background).toBe("var(--dv-icon-hover-background-color)");
      await act(async () => { fireEvent.mouseLeave(closeBtn); });
      expect(closeBtn.style.background).toBe("none");
    });

    it("「+」钮 hover → 背景 SECONDARY_BG，离开还原（TAB-04）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      const plusBtn = Array.from(container.querySelectorAll("button")).find(
        (b) => b.textContent === "+",
      ) as HTMLButtonElement;
      expect(plusBtn).toBeTruthy();
      await act(async () => { fireEvent.mouseEnter(plusBtn); });
      // 用户可见：hover 底 ui.secondaryBg（#222227 → rgb(34, 34, 39)）
      expect(plusBtn.style.background).toBe("rgb(34, 34, 39)");
      await act(async () => { fireEvent.mouseLeave(plusBtn); });
      expect(plusBtn.style.background).toBe("none");
    });

    it("hover 未激活页签 → 标题文字变 fg-1（TAB-01 仅文字变色）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      await act(async () => { clickButton(container, "+"); });
      await settle();
      const tabs = Array.from(container.querySelectorAll(".dv-tab"));
      // 两个同组页签：后创建的面板（terminal-1）为激活——第一个（terminal-0）非激活。
      // onMouseEnter 挂在 DefaultTab 根 div（.dv-tab > .dv-react-part 内层），
      // 以标题 span 的父元素为派发目标
      const inactiveTab = tabs[0] as HTMLElement;
      const titleSpan = inactiveTab.querySelector("span") as HTMLElement;
      const tabRoot = titleSpan.parentElement as HTMLElement;
      await act(async () => { fireEvent.mouseEnter(tabRoot); });
      // 用户可见：hover 未激活页签文字变 sidebarFg（#ece9e4 → rgb(236, 233, 228)），
      // 激活页签（第一个）文字不变
      expect(titleSpan.style.color).toBe("rgb(236, 233, 228)");
    });
  });

  describe("页面可见性（TQ-COV-08）", () => {
    it("visible=false → 根容器 display:none（多实例 CSS 显隐）", async () => {
      mockIPC(() => null);
      const { container } = await renderDock({ visible: false });
      // 用户可见：非活跃页面 DOM 隐藏（display:none）——H6 终端跨页面存活的基础
      const root = container.firstElementChild as HTMLElement;
      expect(root.style.display).toBe("none");
    });

    it("visible=true → 根容器 display:block", async () => {
      mockIPC(() => null);
      const { container } = await renderDock({ visible: true });
      const root = container.firstElementChild as HTMLElement;
      expect(root.style.display).toBe("block");
    });
  });

  describe("布局恢复边界（TQ-COV-08）", () => {
    /** 含一个无 panelId 编辑器面板的布局（触发 rebuildAndRecomputeTitles 双 pass 的
     *  `!params?.panelId continue` 守卫——面板自身恢复不受影响） */
    const BARE_PANEL_LAYOUT = {
      grid: {
        root: { type: "branch", data: [{ type: "leaf", data: { views: ["editor-1", "bare"] } }] },
        orientation: "HORIZONTAL",
      },
      panels: {
        "editor-1": {
          id: "editor-1",
          contentComponent: "editor",
          title: "a.txt",
          position: { group: "group-1" },
          params: { panelId: "editor-1", filePath: "C:\\root\\a.txt" },
        },
        "bare": {
          id: "bare",
          contentComponent: "editor",
          title: "bare",
          position: { group: "group-1" },
          params: {},
        },
      },
      activeGroup: "group-1",
    };

    it("恢复含无 panelId 面板的布局 → 面板齐全 + 正常面板标题重算（守卫 continue 不崩）", async () => {
      mockIPC(() => null);
      const { api } = await renderDock({ savedLayout: BARE_PANEL_LAYOUT });
      // 用户可见：两个面板都恢复（无 panelId 的面板不参与标题注册但保留）
      expect(api.panels.map((p: AnyApi) => p.id)).toEqual(["editor-1", "bare"]);
      expect(api.getPanel("editor-1").api.title).toBe("a.txt");
    });

    it("rootPath 为空 + 恢复终端布局 → 终端标题仍重算 terminal-N（B12 pass 先于 rootPath 检查）", async () => {
      mockIPC(() => null);
      const { api } = await renderDock({
        savedLayout: TERMINAL_TRANSIENT_LAYOUT,
        rootPath: undefined,
      });
      // 用户可见：瞬态标题 "claude" 被重算（不依赖 rootPath 的终端 pass 先执行）
      expect(api.getPanel("terminal-page-dock-0").api.title).toBe("terminal-0");
    });

    it("尾部反斜杠 filePath → 文件名回退整路径（FileIcon 渲染 + 标题回退 component）", async () => {
      mockIPC(() => null);
      const TRAILING_LAYOUT = {
        grid: {
          root: { type: "branch", data: [{ type: "leaf", data: { views: ["editor-x"] } }] },
          orientation: "HORIZONTAL",
        },
        panels: {
          "editor-x": {
            id: "editor-x",
            contentComponent: "editor",
            title: "sub",
            position: { group: "group-1" },
            params: { panelId: "editor-x", filePath: "C:\\root\\sub\\" },
          },
        },
        activeGroup: "group-1",
      };
      const { container } = await renderDock({ savedLayout: TRAILING_LAYOUT });
      // 用户可见：FileIcon 渲染（文件名 = 整路径回退，TAB-03 判据命中）
      const fileIconSvgs = Array.from(container.querySelectorAll("svg")).filter(
        isFileIconSvg,
      );
      expect(fileIconSvgs.length).toBeGreaterThan(0);
      // basename 为空 → 重算标题为空 → DefaultTab 回退 component 名
      const tab = container.querySelector(".dv-tab") as HTMLElement;
      expect(tab.textContent).toBe("editor×");
    });
  });

  describe("onDidRemovePanel / 布局恢复守卫（TQ-COV-08）", () => {
    /** 两个同目录层级不同但 basename 相同的编辑器（冲突 → 相对路径标题） */
    const CONFLICT_LAYOUT = {
      grid: {
        root: { type: "branch", data: [{ type: "leaf", data: { views: ["editor-1", "editor-2"] } }] },
        orientation: "HORIZONTAL",
      },
      panels: {
        "editor-1": {
          id: "editor-1",
          contentComponent: "editor",
          title: "a.txt",
          position: { group: "group-1" },
          params: { panelId: "editor-1", filePath: "C:\\root\\a.txt" },
        },
        "editor-2": {
          id: "editor-2",
          contentComponent: "editor",
          title: "a.txt",
          position: { group: "group-1" },
          params: { panelId: "editor-2", filePath: "C:\\root\\other\\a.txt" },
        },
      },
      activeGroup: "group-1",
    };

    it("removePanel 编辑器 → 面板消失 + 剩余面板标题重算（冲突解除回 basename）+ 布局保存", async () => {
      mockIPC(() => null);
      const { api, onLayoutChange } = await renderDock({ savedLayout: CONFLICT_LAYOUT });
      // 恢复期同名冲突 → 相对路径标题（用户可见）
      expect(api.getPanel("editor-2").api.title).toBe("other/a.txt");
      onLayoutChange.mockClear();
      await act(async () => { api.removePanel(api.getPanel("editor-1")); });
      await settle();
      // 用户可见：面板关闭 + 剩余同名编辑器冲突解除 → 标题回 basename
      expect(api.getPanel("editor-1")).toBeUndefined();
      expect(api.getPanel("editor-2").api.title).toBe("a.txt");
      // onDidRemovePanel 触发的布局变更写回 store
      expect(onLayoutChange).toHaveBeenCalled();
    });

    it("幽灵编辑器注册（无对应面板）→ 移除其它面板时重算目标不存在 → 跳过不崩溃", async () => {
      mockIPC(() => null);
      const { api } = await renderDock({ savedLayout: CONFLICT_LAYOUT });
      // 幽灵条目：titleManager 有注册但 dockview 无对应面板（getPanel 返回 null）
      titleManager.registerEditor(PAGE_ID, "phantom", "C:\\root\\phantom.txt");
      await act(async () => { api.removePanel(api.getPanel("editor-1")); });
      await settle();
      // 用户可见：真实面板仍正确重算（幽灵条目被 applyTitleUpdates 跳过）
      expect(api.getPanel("editor-2").api.title).toBe("a.txt");
    });

    it("rootPath 为空 + 移除无 panelId 面板 → 跳过注销与重算不崩溃", async () => {
      mockIPC(() => null);
      const { api } = await renderDock({ rootPath: undefined });
      await act(async () => {
        api.addPanel({ id: "bare-3", component: "editor", title: "bare", params: {} });
      });
      await settle();
      await act(async () => { api.removePanel(api.getPanel("bare-3")); });
      await settle();
      // 用户可见：无 panelId 面板可正常关闭
      expect(api.panels.length).toBe(0);
    });

    it("fromJSON 恢复守卫：程序化恢复后的布局变更不写回，守卫复位后恢复写回", async () => {
      mockIPC(() => null);
      const { api, onLayoutChange } = await renderDock();
      // 程序化恢复（非用户操作）→ onDidLayoutFromJSON 置守卫
      await act(async () => { api.fromJSON(api.toJSON()); });
      // 守卫期内 addPanel → 布局变更被抑制（不写回 store）
      await act(async () => {
        api.addPanel({
          id: "guard-1", component: "editor", title: "g1",
          params: { panelId: "guard-1", filePath: "C:\\root\\g1.txt" },
        });
      });
      expect(onLayoutChange).not.toHaveBeenCalled();
      // setTimeout(0) 复位后 → 布局变更正常写回
      await settle();
      await act(async () => {
        api.addPanel({
          id: "guard-2", component: "editor", title: "g2",
          params: { panelId: "guard-2", filePath: "C:\\root\\g2.txt" },
        });
      });
      expect(onLayoutChange).toHaveBeenCalled();
    });
  });

  describe("DefaultTab 标题回退（TQ-COV-08）", () => {
    it("addPanel 空标题 → 页签显示 component 名回退", async () => {
      mockIPC(() => null);
      const { api, container } = await renderDock();
      await act(async () => {
        api.addPanel({ id: "e0", component: "editor", title: "", params: { panelId: "e0" } });
      });
      await settle();
      // 用户可见：标题回退链 api.title || api.component || "" → "editor"
      const tab = container.querySelector(".dv-tab") as HTMLElement;
      expect(tab.textContent).toBe("editor×");
    });
  });
});
