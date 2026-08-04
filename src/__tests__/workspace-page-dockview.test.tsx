// workspace-page-dockview.test.tsx — PageDockviewHost 真实组件集成测试（WRK-01）
//
// 真实渲染 PageDockview（真实 DockviewReact，非 mock），通过 onReady 捕获真实
// DockviewApi 驱动断言：
// - handleReady：空布局不兜底创建终端（H6/Watermark 接管）、保存布局恢复 + 标题重算、
//   损坏布局（白名单过滤后无面板）回退 Watermark
// - Watermark 按钮 addPanel（终端 id 递增、title 由 titleManager 分配）
// - RightHeader "+" 按钮 addPanel（同 group）
// - onSaveAs（slterm:file-saved-as 事件）→ handleSaveAs → 重算标题 → 真实 setTitle
//
// 注：jsdom 中真实 Dockview 的 addPanel/setTitle/fromJSON 均可用（探针验证）。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
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

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

import PageDockview from "../workspace/PageDockviewHost";
import { titleManager } from "../workspace/titleManager";

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
});

afterEach(() => {
  // RTL 无 auto-cleanup（vitest globals 未开启）——Dockview 实例跨测试残留会
  // 污染按钮查询与全局事件监听，必须显式清理
  cleanup();
  clearMocks();
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
});
