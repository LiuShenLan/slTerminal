// workspace-sideviews.test.tsx — Workspace 集成：侧栏视图三栏结构验证
//
// 验证 Workspace 三栏改造后 Allotment 的 pane 结构：
// - pane1 活动栏 40px 固定（preferredSize/minSize/maxSize）
// - pane2 侧栏区 visible=anyOpen + width 驱动
// - pane3 主区 minSize 保持
// - 外层 onChange → setWidth、switchToPage 透传 SideBarArea
//
// 约束：mock allotment 捕获 Pane props，真实 sideBar store 种子，stub ActivityBar/SideBarArea

/* eslint-disable @typescript-eslint/no-explicit-any */

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, cleanup, act } from "@testing-library/react";

// ─── 来自真实 sideBarState 的常量与纯函数（不移入 mock 以保持单真值源） ───
import {
  ACTIVITY_BAR_SIZE,
  DEFAULT_ZONES,
  DEFAULT_OPEN,
} from "../features/sideViews/sideBarState";

// ─── Hoisted: Allotment 捕获层 ───
const allotmentMocks = vi.hoisted(() => {
  /** 按顺序记录每个 Pane 的 props（浅拷贝） */
  const paneProps: Array<Record<string, unknown>> = [];
  /** 记录最近一次 Allotment 的 onChange 回调 */
  let capturedOnChange: ((sizes: number[]) => void) | null = null;

  const MockAllotment: any = ({ children, onChange }: any) => {
    capturedOnChange = onChange || null;
    // 清除上一轮 pane props（每次 render 重建）
    paneProps.length = 0;
    return React.createElement("div", { "data-testid": "allotment" }, children);
  };
  MockAllotment.Pane = (props: any) => {
    const { children, ...rest } = props;
    paneProps.push({ ...rest });
    return React.createElement("div", { "data-testid": "allotment-pane" }, children);
  };

  return { MockAllotment, paneProps, getOnChange: () => capturedOnChange, reset() { paneProps.length = 0; capturedOnChange = null; } };
});

vi.mock("allotment", () => ({
  Allotment: allotmentMocks.MockAllotment,
}));

// ─── Hoisted: SideBarArea stub props 捕获 ───
const sideBarAreaCapture = vi.hoisted(() => {
  let props: Record<string, unknown> | null = null;
  return {
    get current() { return props; },
    set current(v: Record<string, unknown> | null) { props = v; },
    reset() { props = null; },
  };
});

// ─── Mock ../features/sideViews: 保留真实常量/deriveLayout，stub 组件 ───
vi.mock("../features/sideViews", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../features/sideViews")>();
  return {
    ...actual,
    ActivityBar: () => React.createElement("div", { "data-testid": "activity-bar" }),
    SideBarArea: (props: Record<string, unknown>) => {
      sideBarAreaCapture.current = { ...props };
      return React.createElement("div", { "data-testid": "sidebar-area" });
    },
  };
});

// ─── Mock ../features/sideViews/sideViewDefs: side-effect import 静默（防注册链） ───
vi.mock("../features/sideViews/sideViewDefs", () => ({}));

// ─── Workspace 其余依赖 mock ───
vi.mock("dockview-react", () => ({
  DockviewReact: vi.fn(() => null),
}));

vi.mock("../panelRegistry", () => ({
  panelRegistry: {},
}));

vi.mock("../workspace/layoutSerde", () => ({
  saveLayout: vi.fn(() => ({})),
  loadLayout: vi.fn(() => false),
}));

vi.mock("../features/sidebar", () => ({
  SidebarTree: vi.fn(() => null),
  makeEmptyLayout: vi.fn(() => ({})),
}));

vi.mock("../features/explorer", () => ({
  ExplorerPanel: vi.fn(() => null),
}));

vi.mock("../ipc/fs", () => ({
  readDir: vi.fn(),
  readFile: vi.fn(),
  writeFile: vi.fn(),
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  setProjectRoot: vi.fn(() => Promise.resolve()),
}));

// ─── 真实 store ───
import Workspace from "../workspace/Workspace";
import { useSideBar } from "../stores/sideBar";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

// ─── 辅助 ───

function resetStores() {
  useProjects.setState({
    projects: {},
    expandedNodes: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: null });
  useSideBar.setState({
    zones: { ...DEFAULT_ZONES },
    open: { ...DEFAULT_OPEN },
    width: 250,
    splitRatio: 0.5,
    loaded: true,
  });
}

function seedProjectAndPage() {
  const projectId = "proj-sv";
  const pageId = "page-sv";
  const rootPath = "C:\\sv-test";
  useProjects.setState({
    projects: {
      [projectId]: {
        projectId,
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId,
            name: "测试页面",
            layout: {},
            cwd: rootPath,
            createdAt: Date.now(),
            lastAccessedAt: Date.now(),
          },
        ],
        activePageId: pageId,
        version: 1,
      },
    },
    expandedNodes: { [projectId]: true },
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: pageId });
}

/** 获取当前记录的 pane props（按渲染顺序：活动栏 / 侧栏区 / 主区） */
function getPane(index: number): Record<string, unknown> | undefined {
  return allotmentMocks.paneProps[index];
}

// ─── Tests ───

describe("Workspace 侧栏视图集成", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cleanup();
    resetStores();
    allotmentMocks.reset();
    sideBarAreaCapture.reset();
  });

  describe("SB-24.1: 活动栏 pane 40px 固定", () => {
    it("pane1 preferredSize === ACTIVITY_BAR_SIZE (40)", () => {
      seedProjectAndPage();
      render(React.createElement(Workspace));
      expect(getPane(0)?.preferredSize).toBe(ACTIVITY_BAR_SIZE);
    });

    it("pane1 minSize === ACTIVITY_BAR_SIZE", () => {
      seedProjectAndPage();
      render(React.createElement(Workspace));
      expect(getPane(0)?.minSize).toBe(ACTIVITY_BAR_SIZE);
    });

    it("pane1 maxSize === ACTIVITY_BAR_SIZE", () => {
      seedProjectAndPage();
      render(React.createElement(Workspace));
      expect(getPane(0)?.maxSize).toBe(ACTIVITY_BAR_SIZE);
    });
  });

  describe("SB-24.2: 侧栏区 pane visible=anyOpen + preferredSize=width", () => {
    it("默认 open={top:'projects'} → anyOpen=true → visible=true", () => {
      seedProjectAndPage();
      render(React.createElement(Workspace));
      expect(getPane(1)?.visible).toBe(true);
    });

    it("preferredSize 来自 store 的 width", () => {
      seedProjectAndPage();
      useSideBar.setState({ width: 320 });
      render(React.createElement(Workspace));
      expect(getPane(1)?.preferredSize).toBe(320);
    });

    it("open 双空 → anyOpen=false → visible=false", () => {
      seedProjectAndPage();
      useSideBar.setState({
        open: { top: null, bottom: null },
      });
      render(React.createElement(Workspace));
      expect(getPane(1)?.visible).toBe(false);
    });

    it("单开下区 → anyOpen=true（single-bottom）", () => {
      seedProjectAndPage();
      useSideBar.setState({
        zones: { top: ["projects", "explorer"], bottom: ["projects"] },
        open: { top: null, bottom: "projects" },
      });
      render(React.createElement(Workspace));
      expect(getPane(1)?.visible).toBe(true);
    });

    it("双开 → anyOpen=true（split）", () => {
      seedProjectAndPage();
      useSideBar.setState({
        zones: { top: ["projects"], bottom: ["explorer"] },
        open: { top: "projects", bottom: "explorer" },
      });
      render(React.createElement(Workspace));
      expect(getPane(1)?.visible).toBe(true);
    });
  });

  describe("SB-24.3: 主区 pane 保持", () => {
    it("pane3 minSize === 200（MAIN_MIN_SIZE）", () => {
      seedProjectAndPage();
      render(React.createElement(Workspace));
      expect(getPane(2)?.minSize).toBe(200);
    });
  });

  describe("SB-24.4: 外层 Allotment onChange → setWidth", () => {
    it("anyOpen=false 时不调用 setWidth（短路径）", () => {
      seedProjectAndPage();
      // 先设为隐藏
      useSideBar.setState({
        open: { top: null, bottom: null },
      });
      render(React.createElement(Workspace));

      const onChange = allotmentMocks.getOnChange();
      expect(onChange).toBeTruthy();

      const widthBefore = useSideBar.getState().width;
      // 模拟拖拽
      act(() => { onChange!([40, 300, 600]); });
      // anyOpen=false → 短路，width 不变
      expect(useSideBar.getState().width).toBe(widthBefore);
    });

    it("anyOpen=true 时 onChange 同步 sizes[1] 到 store", () => {
      seedProjectAndPage();
      // 默认 anyOpen=true（open.top='projects'）
      render(React.createElement(Workspace));

      const onChange = allotmentMocks.getOnChange();
      expect(onChange).toBeTruthy();

      act(() => { onChange!([40, 300, 600]); });
      // setWidth 内部 clamp，300 在 [160,500] 内 → 直接写入
      expect(useSideBar.getState().width).toBe(300);
    });
  });

  describe("SB-24.5: SideBarArea props 透传", () => {
    it("switchToPage 作为函数传入 SideBarArea", () => {
      seedProjectAndPage();
      render(React.createElement(Workspace));
      const props = sideBarAreaCapture.current;
      expect(props).toBeTruthy();
      expect(typeof props!.switchToPage).toBe("function");
    });

    it("onDeletePage 作为函数传入 SideBarArea", () => {
      seedProjectAndPage();
      render(React.createElement(Workspace));
      const props = sideBarAreaCapture.current;
      expect(props).toBeTruthy();
      expect(typeof props!.onDeletePage).toBe("function");
    });
  });
});
