// title-bar.test.tsx —— 自绘标题栏 TitleBar 组件 L2 测试（TB-06，7 用例）
//
// 组件契约（stage-04 工作流写死）：
//   - 路径 src/features/titleBar/TitleBar.tsx，无 props（自订阅 stores）
//   - 三段结构：左 app 标识 / 中「项目名 / 页面名」/ 右三窗口钮
//   - 三钮点击调用 ipc/window 的 minimizeWindow/toggleMaximizeWindow/closeWindow
//   - 中段双击调用 toggleMaximizeWindow；左/中段容器带 data-tauri-drag-region
// 测试模式：vi.mock ../ipc/window（三 wrapper 桩）+ 真实 projects store（beforeEach setState 种子）

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ─── mock ipc/window 三 wrapper（组件点击目标） ───
vi.mock("../ipc/window", () => ({
  minimizeWindow: vi.fn().mockResolvedValue(undefined),
  toggleMaximizeWindow: vi.fn().mockResolvedValue(undefined),
  closeWindow: vi.fn().mockResolvedValue(undefined),
}));

import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { TitleBar } from "../features/titleBar/TitleBar";
import {
  minimizeWindow,
  toggleMaximizeWindow,
  closeWindow,
} from "../ipc/window";
import { useProjects } from "../stores/projects";

// ── 测试辅助：projects store 种子 ──

/** 种子项目：两个页面，活跃页 = 页面A（activePageId 指向 page-1） */
function seedProjects(): void {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath: "D:/test",
        pages: [
          { pageId: "page-1", name: "页面A", layout: {}, createdAt: 1, lastAccessedAt: 1 },
          { pageId: "page-2", name: "页面B", layout: {}, createdAt: 2, lastAccessedAt: 2 },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
}

/** 按规格顺序（最小化/最大化/关闭）取右段三钮——规格 TB-02 顺序契约 */
function getWindowButtons(): HTMLElement[] {
  const buttons = screen.getAllByRole("button");
  expect(buttons).toHaveLength(3);
  return buttons as HTMLElement[];
}

describe("TitleBar", () => {
  beforeEach(() => {
    seedProjects();
    vi.clearAllMocks();
  });

  // 项目惯例：render 测试显式 cleanup（无 globals，RTL auto-cleanup 不生效）
  afterEach(cleanup);

  // ═══ 三段结构 ═══

  it("TB-06-1 渲染三段结构：左段标识 + 右段三窗口钮", () => {
    render(<TitleBar />);
    // 左段：app 标识文本
    expect(screen.getByText("slTerminal")).not.toBeNull();
    // 右段：最小化/最大化/关闭三钮
    expect(getWindowButtons()).toHaveLength(3);
  });

  // ═══ 项目/页面名显示 ═══

  it("TB-06-2 中段按 store 种子显示活跃项目名与活跃页面名", () => {
    render(<TitleBar />);
    // 活跃项目名（种子「测试项目」）+ 活跃页面名（activePageId → 「页面A」）
    expect(screen.getByText(/测试项目/)).not.toBeNull();
    expect(screen.getByText(/页面A/)).not.toBeNull();
    // 非活跃页「页面B」不应出现
    expect(screen.queryByText(/页面B/)).toBeNull();
  });

  // ═══ 三钮点击 → 对应 wrapper ═══

  it("TB-06-3 点击最小化钮调用 minimizeWindow 一次", () => {
    render(<TitleBar />);
    fireEvent.click(getWindowButtons()[0]);
    expect(minimizeWindow).toHaveBeenCalledTimes(1);
    expect(toggleMaximizeWindow).not.toHaveBeenCalled();
    expect(closeWindow).not.toHaveBeenCalled();
  });

  it("TB-06-4 点击最大化钮调用 toggleMaximizeWindow 一次", () => {
    render(<TitleBar />);
    fireEvent.click(getWindowButtons()[1]);
    expect(toggleMaximizeWindow).toHaveBeenCalledTimes(1);
    expect(minimizeWindow).not.toHaveBeenCalled();
    expect(closeWindow).not.toHaveBeenCalled();
  });

  it("TB-06-5 点击关闭钮调用 closeWindow 一次", () => {
    render(<TitleBar />);
    fireEvent.click(getWindowButtons()[2]);
    expect(closeWindow).toHaveBeenCalledTimes(1);
    expect(minimizeWindow).not.toHaveBeenCalled();
    expect(toggleMaximizeWindow).not.toHaveBeenCalled();
  });

  // ═══ 中段双击 ═══

  it("TB-06-6 中段双击调用 toggleMaximizeWindow 一次", () => {
    render(<TitleBar />);
    // 双击中段标题（事件冒泡至中段容器 onDoubleClick）
    fireEvent.doubleClick(screen.getByText(/测试项目/));
    expect(toggleMaximizeWindow).toHaveBeenCalledTimes(1);
    expect(minimizeWindow).not.toHaveBeenCalled();
    expect(closeWindow).not.toHaveBeenCalled();
  });

  // ═══ 拖拽区域 ═══

  it("TB-06-7 容器含 data-tauri-drag-region 属性", () => {
    const { container } = render(<TitleBar />);
    expect(container.querySelector("[data-tauri-drag-region]")).not.toBeNull();
  });
});
