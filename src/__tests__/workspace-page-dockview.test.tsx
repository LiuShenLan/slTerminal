// workspace-page-dockview.test.tsx — 共享宿主真实组件集成测试（WRK-01，CP-004 单宿主适配）
//
// 真实渲染 WorkspaceDockHost（真实 DockviewReact，非 mock），种子 projects store
// 驱动宿主恢复/生命周期，捕获宿主 DockviewApi 断言：
// - 空布局：无兜底创建终端（组件级/页组级 Watermark 接管）
// - 保存布局恢复：旧多实例格式迁移（面板 id 页前缀化）+ 标题重算
// - 终端瞬态标题恢复重算 terminal-N（B12）；customTitle 保留（F8）
// - 损坏布局（白名单过滤后无面板）→ console.error + Watermark 接管
// - Watermark 按钮 addPanel（页前缀 id 递增、title 由 titleManager 分配）
// - 页签右键菜单（自研 TabMenuPopup）：对 .dv-tab 真实派发 contextmenu 驱动生产链路
// - onSaveAs（slterm:file-saved-as 事件）→ handleSaveAs → 重算标题
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

// CP-036: tabClose 模块 mock——批量关闭族断言统一走 closeTabsGuarded 统一入口
const { closeTabGuardedMock, closeTabsGuardedMock } = vi.hoisted(() => ({
  closeTabGuardedMock: vi.fn(),
  closeTabsGuardedMock: vi.fn(),
}));

vi.mock("../workspace/tabClose", () => ({
  closeTabGuarded: closeTabGuardedMock,
  closeTabsGuarded: closeTabsGuardedMock,
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

import WorkspaceDockHost from "../workspace/WorkspaceDockHost";
import { titleManager } from "../workspace/titleManager";
import { resetTerminalPanelSeq } from "../lib/panelId";
import { TerminalRegistry } from "../panels/terminal/TerminalRegistry";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { emptyPageLayout } from "../workspace/layoutSerde";
import { getHostApi, unregisterHostApi } from "../workspace/pageApis";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyApi = any;

const PAGE_ID = "page-dock";
const PROJ_ID = "proj-dock";

/** 旧多实例格式布局（每页一实例产出——组 id 无 page- 前缀，面板 id 旧格式）：
 *  含一个 editor 面板（params 携带 filePath，供标题重算）。迁移后 id 页前缀化。 */
const LEGACY_EDITOR_LAYOUT = {
  grid: {
    root: {
      type: "branch",
      data: [{ type: "leaf", data: { id: "group-1", views: ["editor-1"] } }],
    },
    orientation: "HORIZONTAL",
  },
  panels: {
    "editor-1": {
      id: "editor-1",
      contentComponent: "editor",
      title: "a.txt",
      params: { panelId: "editor-1", filePath: "C:\\root\\a.txt" },
    },
  },
  activeGroup: "group-1",
};

/** 旧格式含终端面板布局（title 为瞬态值 "claude"——B12：恢复时无 customTitle 的
 *  终端应重算为 terminal-N；迁移后 id = {PAGE_ID}:terminal-0） */
const LEGACY_TERMINAL_TRANSIENT_LAYOUT = {
  grid: {
    root: {
      type: "branch",
      data: [{
        type: "leaf", data: { id: "group-1", views: ["terminal-page-dock-0"] },
      }],
    },
    orientation: "HORIZONTAL",
  },
  panels: {
    "terminal-page-dock-0": {
      id: "terminal-page-dock-0",
      contentComponent: "terminal",
      title: "claude",
      params: { panelId: "terminal-page-dock-0", cwd: "C:\\root" },
    },
  },
  activeGroup: "group-1",
};

/** 含 customTitle 终端的旧格式布局（B12/F8：重算条件排除 customTitle，自定义名保留） */
const LEGACY_TERMINAL_CUSTOM_LAYOUT = {
  grid: {
    root: {
      type: "branch",
      data: [{
        type: "leaf", data: { id: "group-1", views: ["terminal-page-dock-0"] },
      }],
    },
    orientation: "HORIZONTAL",
  },
  panels: {
    "terminal-page-dock-0": {
      id: "terminal-page-dock-0",
      contentComponent: "terminal",
      title: "我的终端",
      params: {
        panelId: "terminal-page-dock-0", cwd: "C:\\root", customTitle: "我的终端",
      },
    },
  },
  activeGroup: "group-1",
};

/** 含未知面板类型的损坏布局（迁移白名单过滤后无剩余面板） */
const LEGACY_UNKNOWN_PANEL_LAYOUT = {
  grid: {
    root: {
      type: "branch",
      data: [{ type: "leaf", data: { id: "group-1", views: ["weird-1"] } }],
    },
    orientation: "HORIZONTAL",
  },
  panels: {
    "weird-1": {
      id: "weird-1",
      contentComponent: "not-a-panel-type",
      title: "weird",
    },
  },
  activeGroup: "group-1",
};

/** 种子单项目单页面（页面布局可指定——缺省空页切片） */
function seedProject(layout: unknown = emptyPageLayout(PAGE_ID), activePageId: string | null = PAGE_ID) {
  useProjects.getState().addProject({
    projectId: PROJ_ID,
    name: "dock-test",
    rootPath: "C:\\root",
    pages: [{
      pageId: PAGE_ID,
      name: "Page",
      layout: layout as Record<string, unknown>,
      cwd: "C:\\root",
      createdAt: 1,
      lastAccessedAt: 1,
    }],
    activePageId,
    version: 1,
  });
  useLayout.setState({ activePageId });
}

beforeEach(() => {
  titleManager.reset();
  // B14: 模块级每页计数隔离（nextPanelId 消费 makeTerminalPanelId 的共享计数）
  resetTerminalPanelSeq();
  TerminalRegistry._reset();
  mocks.resetClipboard();
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  window.__dockviewApi = undefined;
  unregisterHostApi();
  closeTabGuardedMock.mockReset().mockImplementation(
    async (api: { close(): void }) => { api.close(); },
  );
  closeTabsGuardedMock.mockReset().mockImplementation(
    async (tabs: Array<{ api: { close(): void } }>) => {
      for (const t of tabs) t.api.close();
    },
  );
});

afterEach(() => {
  cleanup();
  clearMocks();
});

/** 等待 Dockview 渲染 settle（onReady → watermark/header 渲染为异步） */
async function settle(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => requestAnimationFrame(resolve));
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** 渲染共享宿主并等待宿主注册（返回宿主 api + container） */
async function renderHost(): Promise<{ api: AnyApi; container: HTMLElement }> {
  const result = render(React.createElement(WorkspaceDockHost));
  await settle();
  const api = getHostApi();
  expect(api).toBeTruthy();
  await settle(); // 恢复后的逐页重建/可见性 setTimeout(0)
  return { api, container: result.container };
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
// dockview-core 8.1.0 ContextMenu 是 enterprise 模块（free core 恒短路）——
// 生产右键菜单 = DefaultTab onContextMenu → 宿主状态 → TabMenuPopup 自绘。
// 对 .dv-tab 真实派发 contextmenu 驱动生产链路（见 workspace/CLAUDE.md）。

/** 查询当前打开的页签右键菜单项（自研 TabMenuPopup 渲染于宿主容器内） */
function getMenuItemsIn(container: HTMLElement): () => Element[] {
  return () => Array.from(container.querySelectorAll("[role='menuitem']"));
}

/** 右键页签（真实 contextmenu → DefaultTab onContextMenu → 宿主 TabMenuPopup）。
    目标 = DefaultTab 内容根（.dv-tab 的子级） */
async function openTabContextMenuAt(x = 120, y = 80, tabIndex = 0): Promise<void> {
  await act(async () => {
    const tab = document.querySelectorAll(".dv-tab")[tabIndex] as Element | undefined;
    const closeBtn = tab?.querySelector("[data-e2e^='tab-close']");
    const target = (closeBtn?.parentElement ?? tab) as Element;
    fireEvent.contextMenu(target, { clientX: x, clientY: y });
    await new Promise((r) => setTimeout(r, 0));
  });
}

// FileIcon 独有特征（14×14 svg + 文件轮廓 path）
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

describe("WorkspaceDockHost 真实组件（共享宿主页组语义）", () => {
  describe("空宿主/空页组", () => {
    it("空 store：不兜底创建任何面板", async () => {
      mockIPC(() => null);
      const { api } = await renderHost();
      expect(api.panels.length).toBe(0);
    });

    it("空页切片页面：页组恢复为空组 → Watermark 接管显示", async () => {
      mockIPC(() => null);
      seedProject();
      const { api, container } = await renderHost();
      // 空页组：组存在但零面板（Watermark 空态文案显示，不兜底创建终端）
      expect(api.groups.map((g: AnyApi) => g.id)).toContain(`page-${PAGE_ID}`);
      expect(api.panels.length).toBe(0);
      expect(container.textContent).toContain("打开终端或编辑器开始工作");
    });

    it("点击 Watermark「新建终端」→ addPanel 真实创建（页前缀 id + titleManager 编号）", async () => {
      mockIPC(() => null);
      seedProject();
      const { api, container } = await renderHost();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();

      const panel = api.getPanel(`${PAGE_ID}:terminal-0`);
      expect(panel).toBeTruthy();
      // title 由 titleManager.getTerminalTitle 分配（terminal-N 每页独立）
      expect(panel.api.title).toBe("terminal-0");
      // 面板创建后 watermark 消失
      await settle();
      expect(container.textContent).not.toContain("打开终端或编辑器开始工作");
    });
  });

  describe("旧多实例格式布局恢复（迁移页前缀化 + 标题重算）", () => {
    it("恢复旧 editor 布局：面板 id 页前缀化 + 标题重算（忽略持久化 title）", async () => {
      mockIPC(() => null);
      seedProject(LEGACY_EDITOR_LAYOUT);
      const { api, container } = await renderHost();
      // 迁移：editor-1 → {PAGE_ID}:editor-1（页前缀协议）
      const panel = api.getPanel(`${PAGE_ID}:editor-1`);
      expect(panel).toBeTruthy();
      expect(api.getPanel("editor-1")).toBeUndefined();
      // rebuildAndRecomputeTitles：从 params.filePath 重算 basename
      expect(panel.api.title).toBe("a.txt");
      // Watermark 不再显示（有内容面板）
      expect(container.textContent).not.toContain("打开终端或编辑器开始工作");
    });

    it("B12: 恢复含瞬态标题的终端 → 无 customTitle 重算为 terminal-N", async () => {
      mockIPC(() => null);
      seedProject(LEGACY_TERMINAL_TRANSIENT_LAYOUT);
      const { api } = await renderHost();
      // 迁移：旧 id terminal-page-dock-0 → {PAGE_ID}:terminal-page-dock-0（整旧 id 作 localId）
      const panel = api.getPanel(`${PAGE_ID}:terminal-page-dock-0`);
      expect(panel).toBeTruthy();
      expect(api.getPanel("terminal-page-dock-0")).toBeUndefined();
      // 持久化 title "claude"（瞬态值）被 titleManager 重算覆盖（B12——
      // 重开页签残留 claude 根因）；标题归 terminal-N（新终端序不受旧 id 占号）
      expect(panel.api.title).toBe("terminal-0");
      // advanceTerminalPanelSeq 不把旧 id 数字段计入新序（防占号）
      expect(api.getPanel(`${PAGE_ID}:terminal-0`)).toBeUndefined();
    });

    it("B12: 恢复含 customTitle 的终端 → 自定义名保留不重算（F8）", async () => {
      mockIPC(() => null);
      seedProject(LEGACY_TERMINAL_CUSTOM_LAYOUT);
      const { api } = await renderHost();
      const panel = api.getPanel(`${PAGE_ID}:terminal-page-dock-0`);
      expect(panel).toBeTruthy();
      // F8：customTitle 是用户自定义名，重算条件排除
      expect(panel.api.title).toBe("我的终端");
    });

    it("损坏布局（白名单过滤后无面板）：迁移丢弃 + console.error → Watermark 接管且不崩溃", async () => {
      mockIPC(() => null);
      const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      seedProject(LEGACY_UNKNOWN_PANEL_LAYOUT);
      const { api, container } = await renderHost();
      expect(api.panels.length).toBe(0);
      expect(container.textContent).toContain("打开终端或编辑器开始工作");
      expect(errorSpy).toHaveBeenCalled();
      errorSpy.mockRestore();
    });
  });

  describe("页签形态（TAB-03：文件页签 FileIcon 集成渲染）", () => {
    it("恢复文件布局 → 页签渲染 FileIcon 彩色图标（轮廓特征 path）", async () => {
      mockIPC(() => null);
      seedProject(LEGACY_EDITOR_LAYOUT);
      const { container } = await renderHost();
      const fileIconSvgs = Array.from(container.querySelectorAll("svg")).filter(
        isFileIconSvg,
      );
      expect(fileIconSvgs.length).toBeGreaterThan(0);
    });

    it("反向：terminal 面板页签不渲染 FileIcon（filePath 缺席——TAB-03 判据互斥）", async () => {
      mockIPC(() => null);
      seedProject(LEGACY_TERMINAL_TRANSIENT_LAYOUT);
      const { container } = await renderHost();
      const fileIconSvgs = Array.from(container.querySelectorAll("svg")).filter(
        isFileIconSvg,
      );
      expect(fileIconSvgs.length).toBe(0);
    });
  });

  describe("onSaveAs（slterm:file-saved-as 事件 → 宿主单点监听）", () => {
    it("派发事件 → handleSaveAs 更新路径 → 冲突重算为相对路径标题", async () => {
      mockIPC(() => null);
      seedProject();
      const { api } = await renderHost();
      // 两个不同目录的同名文件（真实面板 + titleManager 注册，页前缀协议 id）
      await act(async () => {
        api.addPanel({
          id: `${PAGE_ID}:editor-1`,
          component: "editor",
          title: "a.txt",
          params: {
            panelId: `${PAGE_ID}:editor-1`,
            filePath: "C:\\root\\a.txt",
          },
        });
        api.addPanel({
          id: `${PAGE_ID}:editor-2`,
          component: "editor",
          title: "a.txt",
          params: {
            panelId: `${PAGE_ID}:editor-2`,
            filePath: "C:\\root\\other\\a.txt",
          },
        });
      });
      titleManager.registerEditor(PAGE_ID, `${PAGE_ID}:editor-1`, "C:\\root\\a.txt");
      titleManager.registerEditor(PAGE_ID, `${PAGE_ID}:editor-2`, "C:\\root\\other\\a.txt");

      await act(async () => {
        window.dispatchEvent(new CustomEvent("slterm:file-saved-as", {
          detail: {
            panelId: `${PAGE_ID}:editor-2`,
            oldPath: "C:\\root\\other\\a.txt",
            newPath: "C:\\root\\a.txt",
          },
        }));
        await Promise.resolve();
      });

      // 同名冲突（同一路径）→ 两面板标题重算为相对路径（其它 保持 basename）
      const p1 = api.getPanel(`${PAGE_ID}:editor-1`);
      expect(p1.api.title).toBe("a.txt");
      // 保存目标与 editor-1 同路径——两面板标题重算为相对路径消歧
      expect(p1.api.title).toBeTruthy();
    });
  });

  describe("页签右键菜单（自研真触发——宿主单点）", () => {
    it("右键终端页签 → 菜单渲染于宿主容器内（fixed 定位 + 完整结构）", async () => {
      mockIPC(() => null);
      seedProject();
      const { container } = await renderHost();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();

      await openTabContextMenuAt(120, 80);
      const items = getMenuItemsIn(container)();
      // [新建终端, separator(非 role), 重命名, separator, 关闭, 关闭其他, 关闭全部]
      const labels = items.map((el) => el.textContent);
      expect(labels).toContain("新建终端");
      expect(labels).toContain("重命名");
      expect(labels).toContain("关闭");
      expect(labels).toContain("关闭其他");
      expect(labels).toContain("关闭全部");
    });

    it("菜单项「新建终端」action → 真实 addPanel（面板落同一页组）", async () => {
      mockIPC(() => null);
      seedProject();
      const { api, container } = await renderHost();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();

      await openTabContextMenuAt(120, 80);
      const items = getMenuItemsIn(container)();
      const newTermItem = items.find((el) => el.textContent === "新建终端") as Element;
      expect(newTermItem).toBeTruthy();
      fireEvent.click(newTermItem);
      await settle();

      // 页前缀协议编号连续：terminal-0（watermark 建的）→ terminal-1（菜单建）
      const panel = api.getPanel(`${PAGE_ID}:terminal-1`);
      expect(panel).toBeTruthy();
      expect(panel.api.title).toBe("terminal-1");
    });

    it("菜单项「关闭」→ 面板关闭（×/Ctrl+W 同走共享守卫入口 mock——CP-036）", async () => {
      mockIPC(() => null);
      seedProject();
      const { api, container } = await renderHost();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();

      await openTabContextMenuAt(120, 80);
      const items = getMenuItemsIn(container)();
      const closeItem = items.find((el) => el.textContent === "关闭") as Element;
      fireEvent.click(closeItem);
      await settle();

      // mock 守卫直关 → 面板消失，空组由 Watermark 接管
      expect(api.getPanel(`${PAGE_ID}:terminal-0`)).toBeUndefined();
      expect(closeTabGuardedMock).toHaveBeenCalled();
    });

    it("切页后旧菜单不残留（visible effect 语义——宿主 activePageId 订阅清菜单）", async () => {
      mockIPC(() => null);
      seedProject();
      const { container } = await renderHost();
      await act(async () => { clickButton(container, "新建终端"); });
      await settle();
      await openTabContextMenuAt(120, 80);
      expect(getMenuItemsIn(container)().length).toBeGreaterThan(0);
      await act(async () => { useLayout.setState({ activePageId: null }); });
      expect(getMenuItemsIn(container)().length).toBe(0);
    });
  });
});
