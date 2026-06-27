// explorer-notify.test.tsx — ExplorerPanel 文件监听集成测试
//
// 验证：项目根路径变化时 ExplorerPanel 调用 startWatch 启动文件监听。
// 使用 mock ipc/notify + 真实 stores，遵循 sidebar-actions 模式。

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import { render, cleanup } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockStartWatch = vi.fn().mockResolvedValue(undefined);
  const mockReadDir = vi.fn().mockResolvedValue([]);
  const mockGitStatus = vi.fn().mockResolvedValue([]);

  return {
    mockStartWatch,
    mockReadDir,
    mockGitStatus,
    resetAll() {
      mockStartWatch.mockClear();
      mockReadDir.mockClear();
      mockGitStatus.mockClear();
    },
  };
});

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
}));

vi.mock("../ipc/fs", () => ({
  readDir: mocks.mockReadDir,
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  writeFile: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitStatus: mocks.mockGitStatus,
}));

// 导入真实 stores + ExplorerPanel（在 IPC mock 之后）
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { ExplorerPanel } from "../features/explorer";

// ─── 辅助函数 ───

function populateStore(rootPath = "C:\\test-project") {
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
            cwd: `${rootPath}\\sub`,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
  });
  useLayout.setState({ activePageId: "page-1" });
}

function resetStore() {
  useProjects.setState({ projects: {} });
  useLayout.setState({ activePageId: null });
}

function renderExplorer() {
  return render(React.createElement(ExplorerPanel));
}

// ─── Tests ───

describe("ExplorerPanel 文件监听集成", () => {
  beforeEach(() => {
    cleanup();
    mocks.resetAll();
    resetStore();
  });

  describe("startWatch 调用时机", () => {
    it("1. 活跃项目存在时 → 调用 startWatch(projectRootPath)", () => {
      populateStore("C:\\my-project");
      renderExplorer();

      expect(mocks.mockStartWatch).toHaveBeenCalledWith("C:\\my-project");
      expect(mocks.mockStartWatch).toHaveBeenCalledTimes(1);
    });

    it("2. 无活跃项目时 → 不调用 startWatch", () => {
      renderExplorer();

      expect(mocks.mockStartWatch).not.toHaveBeenCalled();
    });

    it("3. 切换项目 → 为新项目根路径重新调用 startWatch", () => {
      populateStore("C:\\project-a");
      const { rerender } = renderExplorer();
      expect(mocks.mockStartWatch).toHaveBeenCalledWith("C:\\project-a");

      // 切换到 project-b
      mocks.resetAll();
      useProjects.setState({
        projects: {
          "proj-2": {
            projectId: "proj-2",
            name: "项目 B",
            rootPath: "C:\\project-b",
            pages: [
              {
                pageId: "page-2",
                name: "页面 2",
                layout: {},
                cwd: "C:\\project-b",
                createdAt: 2,
                lastAccessedAt: 2,
              },
            ],
            activePageId: "page-2",
            version: 1,
          },
        },
      });
      useLayout.setState({ activePageId: "page-2" });
      rerender(React.createElement(ExplorerPanel));

      expect(mocks.mockStartWatch).toHaveBeenCalledWith("C:\\project-b");
    });

    it("4. startWatch 失败时 → 不抛错，静默 console.error", () => {
      const consoleSpy = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      mocks.resetAll();
      mocks.mockStartWatch.mockRejectedValueOnce(
        new Error("路径不存在"),
      );

      populateStore("C:\\invalid");
      expect(() => renderExplorer()).not.toThrow();

      // 异步错误在 microtask 中捕获
      expect(mocks.mockStartWatch).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });

  describe("projectRootPath 计算", () => {
    it("5. cwd 和 rootPath 不同时 → 对 rootPath 启动监听（项目根）", () => {
      useProjects.setState({
        projects: {
          "proj-3": {
            projectId: "proj-3",
            name: "深层项目",
            rootPath: "D:\\repo-root",
            pages: [
              {
                pageId: "page-3",
                name: "子目录页面",
                layout: {},
                cwd: "D:\\repo-root\\src\\components",
                createdAt: 3,
                lastAccessedAt: 3,
              },
            ],
            activePageId: "page-3",
            version: 1,
          },
        },
      });
      useLayout.setState({ activePageId: "page-3" });
      renderExplorer();

      // 应传 rootPath 而非 cwd
      expect(mocks.mockStartWatch).toHaveBeenCalledWith("D:\\repo-root");
      expect(mocks.mockStartWatch).not.toHaveBeenCalledWith(
        "D:\\repo-root\\src\\components",
      );
    });

    it("6. 仅 rootPath、无 cwd → 使用 rootPath", () => {
      useProjects.setState({
        projects: {
          "proj-4": {
            projectId: "proj-4",
            name: "无 cwd 项目",
            rootPath: "E:\\bare-repo",
            pages: [
              {
                pageId: "page-4",
                name: "默认页面",
                layout: {},
                // cwd 为 undefined
                createdAt: 4,
                lastAccessedAt: 4,
              },
            ],
            activePageId: "page-4",
            version: 1,
          },
        },
      });
      useLayout.setState({ activePageId: "page-4" });
      renderExplorer();

      expect(mocks.mockStartWatch).toHaveBeenCalledWith("E:\\bare-repo");
    });
  });
});
