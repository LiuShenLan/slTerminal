// title-bar.test.tsx —— 自绘标题栏 TitleBar 组件 L2 测试（TB-06，9 用例）
//
// 组件契约（stage-04 工作流写死 + TB-04 / 问题 6 修订）：
//   - 路径 src/features/titleBar/TitleBar.tsx，无 props（自订阅 stores）
//   - 三段结构：左 app 标识 / 中「项目名 / 页面名」/ 右三窗口钮
//   - 三钮点击调用 ipc/window 的 minimizeWindow/toggleMaximizeWindow/closeWindow
//   - 左/中段容器带 data-tauri-drag-region="deep"（子树拖拽——裸属性只命中直接点击
//     元素本身，文字 span/svg logo 子元素会拦截拖拽）
//   - 左/中段拖拽区 height "100%" 撑满 34px 全高（问题 6：无项目时中段空 div 高度 0，
//     点击落点在无 drag 属性的父容器拖不动——仅断言 deep 属性无法防此回归）
//   - 中段无 React 双击 handler——双击最大化由 Tauri 原生拖拽区脚本承担（drag.js
//     detail===2 → internal_toggle_maximize；React onDoubleClick 会与之双重 toggle）
// 测试模式：vi.mock ../ipc/window（三 wrapper 桩）+ 真实 projects store（beforeEach setState 种子）

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ─── mock ipc/window 三 wrapper（组件点击目标） ───
vi.mock("../ipc/window", () => ({
  minimizeWindow: vi.fn().mockResolvedValue(undefined),
  toggleMaximizeWindow: vi.fn().mockResolvedValue(undefined),
  closeWindow: vi.fn().mockResolvedValue(undefined),
}));

import { render, screen, fireEvent, cleanup, act } from "@testing-library/react";
import { Profiler } from "react";
import { TitleBar } from "../features/titleBar/TitleBar";
import {
  minimizeWindow,
  toggleMaximizeWindow,
  closeWindow,
} from "../ipc/window";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";

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

  // ═══ FE-21 窄订阅：无关项目变更不触发重渲染，标题切换响应保持 ═══

  it("FE-21-1 无关项目变更不触发重渲染（标题不变）", () => {
    // Profiler 计数 = TitleBar 子树提交次数：初始挂载 1 次；无关 store 变更
    // 若仍触发 TitleBar 重渲染则计数增长（Zustand 浅比较失效即红）
    let commits = 0;
    render(
      <Profiler id="titlebar-fn21" onRender={() => { commits++; }}>
        <TitleBar />
      </Profiler>,
    );
    expect(commits).toBe(1);
    // 无关变更：新增另一项目（标题推导只涉及首个/活跃项目，proj-2 数据不参与）
    act(() => {
      useProjects.setState((s) => ({
        projects: {
          ...s.projects,
          "proj-2": {
            projectId: "proj-2",
            name: "无关项目",
            rootPath: "D:/other",
            pages: [
              { pageId: "page-9", name: "页面X", layout: {}, createdAt: 9, lastAccessedAt: 9 },
            ],
            activePageId: "page-9",
            version: 1,
          },
        },
      }));
    });
    // 无新提交（未重渲染）+ 标题不变
    expect(commits).toBe(1);
    expect(screen.getByText(/测试项目/)).not.toBeNull();
    expect(screen.getByText(/页面A/)).not.toBeNull();
    expect(screen.queryByText(/无关项目/)).toBeNull();
  });

  it("FE-21-2 切换布局活跃页（layout store）后标题响应更新", () => {
    render(<TitleBar />);
    // 种子默认 layout.activePageId 为空 → 回退首个项目 activePageId → 页面A
    expect(screen.getByText(/页面A/)).not.toBeNull();
    // 经 layout store 切换活跃页 → 中段标题随 activePageId 更新（窄订阅不破坏切换响应）
    act(() => {
      useLayout.setState({ activePageId: "page-2" });
    });
    expect(screen.getByText(/页面B/)).not.toBeNull();
    expect(screen.queryByText(/页面A/)).toBeNull();
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

  // ═══ 中段双击（TB-04 修订：React onDoubleClick 已删除——与 Tauri 原生拖拽区
  //      双击最大化双重 toggle，净无效果；原生脚本在 mousedown 侧处理） ═══

  it("TB-06-6 中段无 React 双击 handler——双击不调 toggleMaximizeWindow（原生承担）", () => {
    render(<TitleBar />);
    // 双击中段标题：不应再经 wrapper 调 toggleMaximizeWindow（原生 drag.js 处理）
    fireEvent.doubleClick(screen.getByText(/测试项目/));
    expect(toggleMaximizeWindow).not.toHaveBeenCalled();
    expect(minimizeWindow).not.toHaveBeenCalled();
    expect(closeWindow).not.toHaveBeenCalled();
  });

  // ═══ 拖拽区域 ═══

  it("TB-06-7 左/中段容器带 data-tauri-drag-region=\"deep\"（子树拖拽）", () => {
    const { container } = render(<TitleBar />);
    const regions = Array.from(
      container.querySelectorAll("[data-tauri-drag-region]"),
    );
    // 恰两处：左段 app 标识 + 中段标题区（右段三钮不在拖拽区内）
    expect(regions).toHaveLength(2);
    for (const r of regions) {
      expect(r.getAttribute("data-tauri-drag-region")).toBe("deep");
    }
  });

  it("TB-06-8 无项目时中段空 div 撑满全高可拖（问题 6 防回归）", () => {
    // 清空项目：中段无「项目名 / 页面名」内容——空 div 除 deep 属性外还须 height
    // 100% 撑满 34px：无显式高度时空 div 高度 0，点击落点在无 drag 属性的父容器
    // 拖不动（问题 6 根因；仅断言 deep 属性无法防此回归）
    useProjects.setState({
      projects: {},
      deletionLock: { pendingDelete: null, acquiredAt: null },
      expandedNodes: {},
    });
    const { container } = render(<TitleBar />);
    const regions = Array.from(
      container.querySelectorAll("[data-tauri-drag-region]"),
    );
    expect(regions).toHaveLength(2);
    expect(regions[0].textContent).toContain("slTerminal");
    expect(regions[1].textContent).toBe("");
    expect(regions[1].getAttribute("data-tauri-drag-region")).toBe("deep");
    expect((regions[1] as HTMLElement).style.height).toBe("100%");
  });

  it("TB-06-9 左/中段拖拽区均撑满全高（34px 栏任意处可拖，问题 6）", () => {
    const { container } = render(<TitleBar />);
    const regions = Array.from(
      container.querySelectorAll("[data-tauri-drag-region]"),
    ) as HTMLElement[];
    expect(regions).toHaveLength(2);
    for (const r of regions) {
      expect(r.style.height).toBe("100%");
    }
  });
});
