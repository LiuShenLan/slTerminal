// commit-view.test.tsx — CommitView 状态机 L2 测试
//
// 覆盖：状态机 4 态（no-root / loading / error / ready）+ 各态提示文字颜色断言
// （人工验证问题 4：提示色统一 DIM_FG——原 INPUT_BORDER 边框 token 近乎不可见）。
// 拆分自原 commit-view.test.tsx（SVC-14）：
// - 列表渲染/折叠交互 → commit-view-list.test.tsx
// - useCommitStatus 计时/切换/refresh → commit-view-status.test.ts
// - openCommitFile 分派/去重/映射表 → commit-open-file.test.ts
// - CommitFileList 右键菜单 UI → commit-context-menu-ui.test.tsx

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const { mockGitStatus, mockOnFsEventCallback } = vi.hoisted(() => {
  let onFsEventCb: (() => void) | null = null;
  return {
    mockGitStatus: vi.fn(),
    // 保存 onFsEvent 注册的回调引用，供测试手动触发 fs-event
    mockOnFsEventCallback: {
      set cb(fn: (() => void) | null) { onFsEventCb = fn; },
      get cb() { return onFsEventCb; },
      trigger() { onFsEventCb?.(); },
    },
  };
});

// mock IPC git —— gitStatus 返回模拟数据
vi.mock("../ipc/git", () => ({
  gitStatus: mockGitStatus,
}));

// mock IPC notify —— onFsEvent 保存回调引用
vi.mock("../ipc/notify", () => ({
  onFsEvent: vi.fn((cb: () => void) => {
    mockOnFsEventCallback.cb = cb;
    return () => { mockOnFsEventCallback.cb = null; };
  }),
}));

import React from "react";
import { render, cleanup, waitFor } from "@testing-library/react";
import { CommitView } from "../features/commit/CommitView";
import { titleManager } from "../workspace/titleManager";
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { resetProjectStores } from "./helpers/workspace-setup";
import { DIM_FG, HTML_PANEL_LOADING_FG } from "../theme";

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"；rgba 去除内空白后比对） */
function hexToRgb(hex: string): string {
  if (hex.startsWith("#")) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgb(${r}, ${g}, ${b})`;
  }
  return hex;
}

/** 定位提示文字所在 div（root 下 textContent 精确匹配的元素） */
function findHint(container: HTMLElement, text: string): HTMLElement {
  const el = Array.from(container.querySelectorAll("div")).find(
    (d) => d.textContent === text,
  );
  expect(el).toBeTruthy();
  return el as HTMLElement;
}

// ── 辅助函数：种子 stores ──
function seedProject(rootPath: string) {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          {
            pageId: "page-1",
            name: "操作页面 1",
            layout: {},
            cwd: undefined,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    deletionLock: { pendingDelete: null, acquiredAt: null },
    expandedNodes: { "proj-1": true },
  });
  useLayout.setState({ activePageId: "page-1" });
}

function resetStores() {
  resetProjectStores(); // 共享重置：projects/layout/sideBar/keybindings 全量（TQ-B-10）
  titleManager.reset();
}

// ── 辅助：构造 GitStatusEntry ──
function makeEntry(
  path: string,
  status: string,
  oldPath: string | null = null,
) {
  return { path, status, oldPath };
}

beforeEach(() => {
  mockGitStatus.mockReset();
  mockOnFsEventCallback.cb = null;
  resetStores();
});

afterEach(() => {
  cleanup();
});

describe("CommitView 状态机", () => {
  it("无 rootPath 时显示「选择一个项目以查看变更」（提示色 DIM_FG——人工验证问题 4）", () => {
    // 不种子 stores → rootPath 为 null
    const { container } = render(React.createElement(CommitView));
    const hint = container.querySelector('[data-e2e="commit-view"]');
    expect(hint).toBeTruthy();
    expect(hint!.textContent).toContain("选择一个项目以查看变更");
    // 提示文字色 = fg-3（DIM_FG）——原 INPUT_BORDER（rgba 白 9%）近乎不可见
    const hintDiv = findHint(container, "选择一个项目以查看变更");
    expect(hintDiv.style.color).toBe(hexToRgb(DIM_FG));
  });

  it("loading 态显示「加载中…」（提示色 HTML_PANEL_LOADING_FG 保持）", () => {
    seedProject("C:/repo");
    // gitStatus 保持 pending
    mockGitStatus.mockReturnValue(new Promise(() => {}));
    const { container } = render(React.createElement(CommitView));
    const hint = container.querySelector('[data-e2e="commit-view"]');
    expect(hint).toBeTruthy();
    expect(hint!.textContent).toContain("加载中…");
    const hintDiv = findHint(container, "加载中…");
    expect(hintDiv.style.color).toBe(hexToRgb(HTML_PANEL_LOADING_FG));
  });

  it("gitStatus 失败显示「当前项目并非 git 项目」（提示色 DIM_FG——人工验证问题 4）", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockRejectedValue(new Error("not a git repo"));
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const hint = container.querySelector('[data-e2e="commit-view"]');
      expect(hint!.textContent).toContain("当前项目并非 git 项目");
    });
    const hintDiv = findHint(container, "当前项目并非 git 项目");
    expect(hintDiv.style.color).toBe(hexToRgb(DIM_FG));
  });

  it("gitStatus 成功显示两列表", async () => {
    seedProject("C:/repo");
    mockGitStatus.mockResolvedValue([
      makeEntry("C:/repo/a.ts", "modified"),
      makeEntry("C:/repo/b.ts", "untracked"),
    ]);
    const { container } = render(React.createElement(CommitView));
    await waitFor(() => {
      const changes = container.querySelector('[data-e2e="commit-changes"]');
      expect(changes).toBeTruthy();
      const unversioned = container.querySelector('[data-e2e="commit-unversioned"]');
      expect(unversioned).toBeTruthy();
    });
  });
});
