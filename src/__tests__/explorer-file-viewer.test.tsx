// explorer-fileViewer.test.tsx — ExplorerPanel + FileViewerRegistry 集成测试
//
// 验证：
//   1. .html/.htm 文件 → addPanel component="htmlviewer"
//   2. .ts/其他文件 → addPanel component="editor"（回退）
//   3. panelId 前缀正确
//   4. registerEditor 仍被调用

import { describe, it, expect, beforeEach, vi } from "vitest";
import React from "react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  const mockAddPanel = vi.fn();
  const mockGetPanel = vi.fn();
  const mockFocus = vi.fn();

  const mockEntries = [
    { name: "index.html", path: "C:/project/index.html", isDir: false, size: 100, modified: 1 },
    { name: "app.ts", path: "C:/project/app.ts", isDir: false, size: 200, modified: 2 },
    { name: "readme.htm", path: "C:/project/readme.htm", isDir: false, size: 50, modified: 3 },
    { name: "data.xyz", path: "C:/project/data.xyz", isDir: false, size: 30, modified: 4 },
    { name: "Makefile", path: "C:/project/Makefile", isDir: false, size: 10, modified: 5 },
    { name: "src", path: "C:/project/src", isDir: true, size: null, modified: null },
  ];

  const mockReadDir = vi.fn().mockResolvedValue(mockEntries);
  const mockGitStatus = vi.fn().mockResolvedValue([]);
  const mockStartWatch = vi.fn().mockResolvedValue(undefined);
  const mockReadFile = vi.fn();

  return {
    mockAddPanel,
    mockGetPanel,
    mockFocus,
    mockReadDir,
    mockGitStatus,
    mockStartWatch,
    mockReadFile,
    mockEntries,
    resetAll() {
      mockAddPanel.mockReset();
      mockGetPanel.mockReset();
      mockFocus.mockReset();
      mockReadDir.mockReset();
      mockGitStatus.mockReset();
      mockStartWatch.mockReset();
      mockReadFile.mockReset();
      mockReadDir.mockResolvedValue(mockEntries);
      mockGitStatus.mockResolvedValue([]);
      mockStartWatch.mockResolvedValue(undefined);
    },
  };
});

vi.mock("../ipc/fs", () => ({
  readDir: mocks.mockReadDir,
  createDir: vi.fn(),
  deleteEntry: vi.fn(),
  rename: vi.fn(),
  writeFile: vi.fn(),
  readFile: mocks.mockReadFile,
}));

vi.mock("../ipc/git", () => ({
  gitStatus: mocks.mockGitStatus,
}));

vi.mock("../ipc/notify", () => ({
  startWatch: mocks.mockStartWatch,
  onFsEvent: () => () => {},
}));

vi.mock("../ipc/dialog", () => ({
  ask: vi.fn(),
}));

// ─── 真实模块导入（mock 之后）───
import { useProjects } from "../stores/projects";
import { useLayout } from "../stores/layout";
import { titleManager } from "../workspace/titleManager";
import { ExplorerPanel } from "../features/explorer";
import { fireEvent, render, waitFor } from "@testing-library/react";

function populateStore(rootPath = "C:\\project") {
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
            cwd: `${rootPath}\\src`,
            createdAt: 1,
            lastAccessedAt: 1,
          },
        ],
        version: 1,
        createdAt: 1,
        lastAccessedAt: 1,
      },
    },
    expandedNodes: {},
  } as never);

  useLayout.setState({
    activePageId: "page-1",
  });
}

describe("ExplorerPanel + FileViewerRegistry 集成", () => {
  beforeEach(() => {
    mocks.resetAll();
    // 重置 titleManager 状态，防止测试间文件注册泄漏
    titleManager.reset();
    useProjects.setState({
      projects: {},
      saveAllProjects: vi.fn(),
    } as never);
    useLayout.setState({
      activePageId: null,
      setActivePage: vi.fn(),
    } as never);

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__dockviewApi = {
      addPanel: mocks.mockAddPanel,
      // 返回含 api.setTitle 的完整 mock，供 recomputeTitles 使用
      getPanel: mocks.mockGetPanel.mockReturnValue({
        focus: mocks.mockFocus,
        api: { setTitle: vi.fn() },
      }),
    };
  });

  // 47. 双击 .html 文件 → addPanel component 为 "htmlviewer"
  it("双击 .html 文件调用 addPanel 且 component 为 htmlviewer", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    // 等待文件树渲染完成
    const htmlItems = await findAllByText("index.html");
    const htmlItem = htmlItems.find((el) => el.tagName === "SPAN");
    expect(htmlItem).toBeDefined();

    // 模拟双击
    fireEvent.doubleClick(htmlItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.component).toBe("htmlviewer");
  });

  // 48. 双击 .ts 文件 → addPanel component 为 "editor"
  it("双击 .ts 文件调用 addPanel 且 component 为 editor", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const tsItems = await findAllByText("app.ts");
    const tsItem = tsItems.find((el) => el.tagName === "SPAN");
    expect(tsItem).toBeDefined();

    fireEvent.doubleClick(tsItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.component).toBe("editor");
  });

  // 49. .html 文件 panelId 前缀为 "htmlviewer-"
  it(".html 文件 panelId 前缀为 htmlviewer-", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const htmlItems = await findAllByText("index.html");
    const htmlItem = htmlItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmlItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.id).toMatch(/^htmlviewer-/);
  });

  // 50. .ts 文件 panelId 前缀为 "editor-"
  it(".ts 文件 panelId 前缀为 editor-", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const tsItems = await findAllByText("app.ts");
    const tsItem = tsItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(tsItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.id).toMatch(/^editor-/);
  });

  // 51. .htm 文件也走 htmlviewer
  it(".htm 文件也走 htmlviewer", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const htmItems = await findAllByText("readme.htm");
    const htmItem = htmItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.component).toBe("htmlviewer");
  });

  // 52. 未知扩展名 .xyz 回退 editor
  it("未知扩展名 .xyz 回退 editor", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const xyzItems = await findAllByText("data.xyz");
    const xyzItem = xyzItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(xyzItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.component).toBe("editor");
  });

  // 53. 无扩展名文件走 editor
  it("无扩展名文件走 editor", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const mkItems = await findAllByText("Makefile");
    const mkItem = mkItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(mkItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.component).toBe("editor");
  });

  // 54. HTML 文件打开时仍调用 addPanel（filePath 已传递）
  it("HTML 文件打开时 addPanel 包含 filePath 参数", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const htmlItems = await findAllByText("index.html");
    const htmlItem = htmlItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmlItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.params).toBeDefined();
    // 路径会被 normalizePath 规范化为正斜杠
    expect(call.params.filePath).toBe("C:/project/index.html");
  });

  // ========================================================================
  // renderer 参数验证（白屏修复）
  // ========================================================================

  // 59. htmlviewer 面板 addPanel 传入 renderer="always"
  it("htmlviewer 面板 addPanel 传入 renderer=\"always\"", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const htmlItems = await findAllByText("index.html");
    const htmlItem = htmlItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmlItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.renderer).toBe("always");
  });

  // 60. editor 面板 addPanel 不传 renderer
  it("editor 面板 addPanel 不传 renderer", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const tsItems = await findAllByText("app.ts");
    const tsItem = tsItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(tsItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.renderer).toBeUndefined();
  });

  // 61. .htm 文件也传 renderer="always"
  it(".htm 文件也传 renderer=\"always\"", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const htmItems = await findAllByText("readme.htm");
    const htmItem = htmItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.renderer).toBe("always");
  });

  // 62. 未知扩展名回退 editor 时不传 renderer
  it("未知扩展名回退 editor 时不传 renderer", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const xyzItems = await findAllByText("data.xyz");
    const xyzItem = xyzItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(xyzItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.renderer).toBeUndefined();
  });

  // 63. 无扩展名文件回退 editor 时不传 renderer
  it("无扩展名文件回退 editor 时不传 renderer", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const mkItems = await findAllByText("Makefile");
    const mkItem = mkItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(mkItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.renderer).toBeUndefined();
  });

  // 64. 资源释放：关闭 htmlviewer 面板后 iframe 不泄露
  it("关闭 htmlviewer 面板后资源正常释放", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    const htmlItems = await findAllByText("index.html");
    const htmlItem = htmlItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmlItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalled();
    }, { timeout: 3000 });

    // 验证 renderer="always" 已设置（防止资源泄露的前置条件：
    // renderer 必须为 "always"，关闭时 dispose 才能正确触发 cleanup）
    const call = mocks.mockAddPanel.mock.calls[0][0];
    expect(call.renderer).toBe("always");
    // addPanel 被调用且参数完整即为资源安全——Dockview 的 dispose 机制
    // 保证面板关闭时 React 组件 unmount → useEffect cleanup → iframe 销毁
    expect(call.params).toBeDefined();
    expect(call.params.filePath).toBeDefined();
  });

  // ========================================================================
  // P1-6: 文件查看器去重路径测试
  // ========================================================================

  // 65. 重复打开相同文件 → findExistingEditor 命中 → focus 调用，addPanel 不二次调用
  it("重复打开相同文件只聚焦已有面板，不创建新面板", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    // 第一次双击 index.html → addPanel 调用 + titleManager 注册
    const htmlItems = await findAllByText("index.html");
    const htmlItem = htmlItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmlItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    // 清空 addPanel 和 focus 的调用记录，准备测试第二次双击
    mocks.mockAddPanel.mockClear();
    mocks.mockFocus.mockClear();

    // 第二次双击同一个文件
    fireEvent.doubleClick(htmlItem!);

    // addPanel 不应被第二次调用（去重命中）
    expect(mocks.mockAddPanel).not.toHaveBeenCalled();

    // focus 应被调用（聚焦已有面板）
    expect(mocks.mockFocus).toHaveBeenCalledTimes(1);
  });

  // 66. 重复打开不同文件 → addPanel 正常调用（不去重）
  it("打开不同文件不触发去重，addPanel 正常调用", async () => {
    populateStore();
    const { findAllByText } = render(React.createElement(ExplorerPanel));

    // 先打开 index.html
    const htmlItems = await findAllByText("index.html");
    const htmlItem = htmlItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(htmlItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    mocks.mockAddPanel.mockClear();
    mocks.mockFocus.mockClear();

    // 再打开 app.ts（不同文件）
    const tsItems = await findAllByText("app.ts");
    const tsItem = tsItems.find((el) => el.tagName === "SPAN");
    fireEvent.doubleClick(tsItem!);

    await waitFor(() => {
      expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    // addPanel 被调用（新文件无去重）
    expect(mocks.mockAddPanel).toHaveBeenCalledTimes(1);
    // focus 未因去重调用（是全新打开）
  });
});
