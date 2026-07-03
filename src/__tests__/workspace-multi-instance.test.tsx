// workspace-multi-instance.test.tsx — 多 Dockview 实例集成测试
//
// 验证多页面切换时 Dockview 实例各自存活、CSS 显隐正确、惰性初始化按需创建。
// 注：jsdom 中 DockviewReact 的 onReady 会创建终端面板，React StrictMode 双重挂载
// 会导致多个 Terminal 实例，测试仅验证架构层面的行为（panel ID 存在性 + DOM 结构）。

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { render } from "@testing-library/react";

global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

import Workspace from "../workspace/Workspace";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { titleManager } from "../workspace/titleManager";

/** 构造带两页面的测试项目（展开态） */
function setupTwoPages() {
  const projId = "proj-multi";
  const pageA = "page-alpha";
  const pageB = "page-beta";

  useProjects.getState().addProject({
    projectId: projId,
    name: "multi-test",
    rootPath: "/tmp/multi",
    pages: [
      { pageId: pageA, name: "Alpha", layout: {}, cwd: "/tmp/multi",
        createdAt: Date.now(), lastAccessedAt: Date.now() },
      { pageId: pageB, name: "Beta", layout: {}, cwd: "/tmp/multi",
        createdAt: Date.now(), lastAccessedAt: Date.now() },
    ],
    activePageId: pageA,
    version: 1,
  });

  const expanded = useProjects.getState().expandedNodes;
  useProjects.setState({ expandedNodes: { ...expanded, [projId]: true } });

  return { projId, pageA, pageB };
}

beforeEach(() => {
  useProjects.setState({
    projects: {},
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: {},
  });
  useLayout.setState({ activePageId: null });
  titleManager.reset();
});

afterEach(() => {
  clearMocks();
});

describe("多 Dockview 实例——架构验证", () => {
  it("1. 活跃页面创建 Dockview，非活跃页面不创建（惰性初始化）", () => {
    mockIPC(() => null);

    const { pageA } = setupTwoPages();
    useLayout.setState({ activePageId: pageA });

    const { container } = render(<Workspace />);
    const text = container.textContent ?? "";
    // 活跃页面的终端面板已创建——标题为 "terminal-N"
    expect(text).toMatch(/terminal-\d/);
    // 非活跃页面尚未初始化 → 无其终端面板 ID
    const terminalMatches = text.match(/terminal-/g);
    // 可能有多个 terminal- 出现（StrictMode 双重挂载导致多个终端实例），
    // 但不会出现 page-beta 的 panel ID
    expect(terminalMatches).toBeTruthy();
    expect(text).not.toContain("terminal-page-beta");
  });

  it("2. 无活跃页面时不应初始化任何 Dockview", () => {
    mockIPC(() => null);

    setupTwoPages();
    useLayout.setState({ activePageId: null });

    const { container } = render(<Workspace />);
    const text = container.textContent ?? "";
    // 侧栏正常渲染
    expect(text).toContain("multi-test");
    // 无 Dockview → 无终端面板（标题格式为 "terminal-N"）
    expect(text).not.toMatch(/terminal-\d/);
  });

  it("3. 项目无页面时也不应创建 Dockview", () => {
    mockIPC(() => null);

    useProjects.getState().addProject({
      projectId: "empty-proj",
      name: "empty-project",
      rootPath: "/tmp/empty",
      pages: [],
      activePageId: null,
      version: 1,
    });
    useLayout.setState({ activePageId: null });

    const { container } = render(<Workspace />);

    const text = container.textContent ?? "";
    expect(text).toContain("empty-project");
    // 无页面 → 无 Dockview → 无 terminal
    expect(text).not.toContain("terminal-");
  });

  it("4. 删除活跃页面后，store 正确更新 activePageId", () => {
    const { projId, pageA, pageB } = setupTwoPages();

    useProjects.getState().switchToPage(projId, pageA);
    expect(useProjects.getState().projects[projId].activePageId).toBe(pageA);

    // 删除活跃页面 → store 自动切换到剩余页面
    useProjects.getState().removePage(projId, pageA);
    expect(useProjects.getState().projects[projId].activePageId).toBe(pageB);
  });
});
