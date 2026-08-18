// explorer-error-placeholder.test.tsx — 文件树加载错误占位测试（FE-07）
//
// 覆盖：readDir 失败 → ExplorerPanel 渲染错误占位（错误消息 + 重试按钮）、
// 重试成功恢复文件树、rootPath 为空不渲染占位、useFileTree rootError 状态流转

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";
import {
  render,
  fireEvent,
  waitFor,
  cleanup,
  renderHook,
  act,
} from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockReadDir = vi.fn();
  const mockGitStatus = vi.fn();
  const mockStartWatch = vi.fn();

  return {
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    resetAll() {
      mockReadDir.mockReset();
      mockGitStatus.mockReset();
      mockStartWatch.mockReset();
      mockReadDir.mockResolvedValue([]);
      mockGitStatus.mockResolvedValue([]);
      mockStartWatch.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../lib/ConfirmDialog", () => ({
  confirmDialog: vi.fn(),
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

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
  stopWatch: vi.fn().mockResolvedValue(undefined),
  onFsEvent: () => () => {},
}));

// ─── 真实模块导入（mock 之后）───
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { ExplorerPanel } from "../features/explorer";
import { useFileTree } from "../features/explorer/useFileTree";

/** 种子项目 store（照 explorer-crud-success 模式） */
function seedProject(rootPath: string = "C:/test-project") {
  useProjects.setState({
    projects: {
      "proj-1": {
        projectId: "proj-1",
        name: "测试项目",
        rootPath,
        pages: [
          { pageId: "page-1", name: "页面 1", layout: {}, cwd: rootPath, createdAt: 1, lastAccessedAt: 1 },
        ],
        activePageId: "page-1",
        version: 1,
      },
    },
    expandedNodes: { "proj-1": true },
    deletionLock: { pendingDelete: null, acquiredAt: null },
  });
  useLayout.setState({ activePageId: "page-1" });
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.resetAll();
  cleanup();
  useProjects.setState({ projects: {}, expandedNodes: {} });
  useLayout.setState({ activePageId: null });
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__dockviewApi = {
    addPanel: vi.fn(),
    getPanel: vi.fn(),
  };
});

// =====================================================================
// ExplorerPanel 错误占位渲染
// =====================================================================

describe("ExplorerPanel 加载错误占位（FE-07）", () => {
  it("E1: readDir 失败 → 渲染错误占位（错误消息 + 重试按钮），不伪装空目录", async () => {
    // 首次 readDir 失败（路径沙箱拒绝等场景）
    mocks.mockReadDir.mockRejectedValueOnce(new Error("path not allowed"));

    seedProject();
    const { findByTestId, findByText } = render(
      React.createElement(ExplorerPanel),
    );

    // 错误占位出现：标题 + 错误消息 + 重试按钮
    const placeholder = await findByTestId("explorer-load-error", undefined, { timeout: 3000 });
    expect(placeholder).toBeTruthy();
    await findByText("文件树加载失败");
    // S08 契约：错误消息统一经 getErrorMessage（Error → String(err) 含 "Error: " 前缀），
    // 断言用包含匹配而非精确相等
    await findByText(/path not allowed/);
    await findByTestId("explorer-load-retry");
  });

  it("E2: 点击重试 → 重新 readDir；成功后占位消失并渲染文件树", async () => {
    mocks.mockReadDir.mockRejectedValueOnce(new Error("temporary failure"));

    seedProject();
    const { findByTestId, queryByTestId, getAllByText } = render(
      React.createElement(ExplorerPanel),
    );

    await findByTestId("explorer-load-error", undefined, { timeout: 3000 });

    // 第二次 readDir 成功（返回文件条目）
    const fileEntry = { name: "main.ts", path: "C:/test-project/main.ts", isDir: false, size: 64, modified: 1 };
    mocks.mockReadDir.mockResolvedValueOnce([fileEntry]);

    fireEvent.click(await findByTestId("explorer-load-retry"));

    // 重试触发重新 readDir（首次失败 + 重试成功 = 2 次）
    await waitFor(() => {
      expect(mocks.mockReadDir).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });

    // 占位消失、文件树恢复渲染
    await waitFor(() => {
      expect(queryByTestId("explorer-load-error")).toBeNull();
    }, { timeout: 3000 });
    expect(getAllByText("main.ts").length).toBeGreaterThan(0);
  });

  it("E3: rootPath 为空 → 渲染空态，不渲染错误占位", () => {
    const { queryByTestId, getByText } = render(
      React.createElement(ExplorerPanel),
    );
    expect(queryByTestId("explorer-load-error")).toBeNull();
    expect(getByText("选择一个项目以浏览文件")).toBeTruthy();
  });
});

// =====================================================================
// useFileTree rootError 状态流转
// =====================================================================

describe("useFileTree rootError（FE-07）", () => {
  it("R1: readDir 失败 → rootError 非空；refresh 成功后 rootError 清空", async () => {
    mocks.mockReadDir.mockRejectedValueOnce(new Error("disk error"));

    const { result } = renderHook(() => useFileTree({ rootPath: "C:/proj" }));

    // 失败 → rootError 记录错误消息（S08 契约：getErrorMessage 兜底 String(err) 含 "Error: " 前缀，用包含匹配）
    await waitFor(() => {
      expect(result.current.rootError).toContain("disk error");
    }, { timeout: 3000 });
    expect(result.current.rootNodes).toEqual([]);

    // 磁盘恢复 → refresh 成功后错误清除
    mocks.mockReadDir.mockResolvedValueOnce([
      { name: "a.ts", path: "C:/proj/a.ts", isDir: false, size: 10, modified: 1 },
    ]);
    await act(async () => {
      await result.current.refresh();
    });
    expect(result.current.rootError).toBeNull();
    expect(result.current.rootNodes.length).toBe(1);
  });

  it("R2: 子目录展开失败 → 按路径记录错误，但 rootError 不受影响（容错不冒泡）", async () => {
    // 根目录成功；src 子目录 readDir 失败
    mocks.mockReadDir.mockImplementation((dirPath: string) => {
      if (dirPath === "C:/proj") {
        return Promise.resolve([
          { name: "src", path: "C:/proj/src", isDir: true, size: null, modified: 1 },
        ]);
      }
      if (dirPath === "C:/proj/src") {
        return Promise.reject(new Error("subdir error"));
      }
      return Promise.resolve([]);
    });

    const { result } = renderHook(() => useFileTree({ rootPath: "C:/proj" }));

    await waitFor(() => {
      expect(result.current.rootNodes.length).toBe(1);
    }, { timeout: 3000 });

    // 展开 src → 读取失败返回 []，rootError 保持 null（子目录错误不上报根）
    await act(async () => {
      await result.current.toggleExpand("C:/proj/src");
    });
    expect(result.current.rootError).toBeNull();
    const srcNode = result.current.rootNodes[0];
    expect(srcNode.expanded).toBe(true);
    expect(srcNode.children).toEqual([]);
  });
});
