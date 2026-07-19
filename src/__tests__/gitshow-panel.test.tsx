// gitshow-panel.test.tsx — GitShowPanel L2 测试
//
// 覆盖：loading 态 / 内容渲染 / 错误占位文案 /
// oldPath 优先于 filePath / readOnly 配置。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// mock IPC git 模块——gitFileAtHead 返回模拟内容
const { mockGitFileAtHead } = vi.hoisted(() => ({
  mockGitFileAtHead: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitFileAtHead: mockGitFileAtHead,
}));

// mock CM6——jsdom 无布局引擎，EditorView 无法真实工作
const { mockEditorViewDestroy, capturedEditorStateConfig } = vi.hoisted(() => {
  const mockEditorViewDestroy = vi.fn();
  const capturedEditorStateConfig: { extensions?: unknown[]; doc?: string }[] = [];
  return { mockEditorViewDestroy, capturedEditorStateConfig };
});

// @codemirror/view mock——需要 mock EditorView + EditorView.theme + EditorView.editable
vi.mock("@codemirror/view", () => {
  const MockEditorView = class {
    // 静态属性——CM6 EditorView 的 theme / editable Facet（匿名类表达式用 static 声明，TypeScript 才认类型）
    static theme = vi.fn(() => []);
    static editable = { of: vi.fn((val: boolean) => ({ __editable: val })) };
    state: unknown;
    constructor(config: { state: unknown; parent: HTMLElement }) {
      this.state = config.state;
      // 挂载到 parent 上供测试断言
      (config.parent as HTMLElement & Record<string, unknown>)._cmView = this;
    }
    destroy() {
      mockEditorViewDestroy();
    }
  };
  return {
    EditorView: MockEditorView,
    keymap: { of: vi.fn(() => []) },
  };
});

vi.mock("@codemirror/state", () => ({
  EditorState: {
    create(config: { doc?: string; extensions?: unknown[] }) {
      capturedEditorStateConfig.push({ extensions: config.extensions, doc: config.doc });
      return { doc: config.doc ?? "", config };
    },
    readOnly: { of: vi.fn((val: boolean) => ({ __readOnly: val })) },
  },
  Compartment: vi.fn(() => ({ of: vi.fn(() => []) })),
}));

vi.mock("@codemirror/theme-one-dark", () => ({
  oneDark: [],
}));

vi.mock("codemirror", () => ({
  basicSetup: [],
}));

// mock useCodeMirror 导出——供 GitShowPanel import 复用
vi.mock("../panels/editor/useCodeMirror", () => ({
  getLanguageExtension: vi.fn(() => []),
  MAX_FILE_SIZE_BYTES: 10_000_000,
  LARGE_FILE_WARN_BYTES: 1_000_000,
  createEditorFontExtension: vi.fn(() => []),
}));

// mock stores/fontSize
vi.mock("../stores", () => ({
  useFontSize: vi.fn((selector: (s: { editorFontSize: number }) => number) =>
    selector({ editorFontSize: 14 }),
  ),
}));

import React from "react";
import { render, cleanup } from "@testing-library/react";
import GitShowPanel from "../panels/gitshow/GitShowPanel";

const DEFAULT_PARAMS = {
  panelId: "gs-1",
  filePath: "src/main.ts",
  repoPath: "C:/repo",
};

beforeEach(() => {
  mockGitFileAtHead.mockReset();
  mockEditorViewDestroy.mockReset();
  capturedEditorStateConfig.length = 0;
});

afterEach(() => {
  cleanup();
});

describe("GitShowPanel", () => {
  // ── 三态：loading ──

  it("初始渲染 loading 态（加载中...）", () => {
    // gitFileAtHead 保持 pending（不 resolve），验证 loading 文案
    mockGitFileAtHead.mockReturnValue(new Promise(() => {}));
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    const span = container.querySelector("span");
    expect(span).toBeTruthy();
    expect(span!.textContent).toBe("加载中...");
  });

  // ── 三态：content ──

  it("加载成功后渲染 CM6 容器", async () => {
    mockGitFileAtHead.mockResolvedValue("console.log('hello');");
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      const div = container.querySelector('div[style*="overflow"]');
      expect(div).toBeTruthy();
    });
  });

  it("content 容器使用 overflow: clip 样式", async () => {
    mockGitFileAtHead.mockResolvedValue("some content");
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      const cmDiv = container.querySelector('div[style*="overflow: clip"]');
      expect(cmDiv).toBeTruthy();
      expect(cmDiv!.getAttribute("style")).toContain("background");
    });
  });

  // ── 三态：error ──

  it("gitFileAtHead reject 时显示错误占位文案", async () => {
    mockGitFileAtHead.mockRejectedValue(new Error("HEAD 中不存在"));
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      const span = container.querySelector("span");
      expect(span).toBeTruthy();
      expect(span!.textContent).toBe("该文件在 HEAD 中不存在");
    });
  });

  it("错误态 span 有 color 样式", async () => {
    mockGitFileAtHead.mockRejectedValue(new Error("任意错误"));
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      const span = container.querySelector("span");
      expect(span).toBeTruthy();
      expect(span!.getAttribute("style")).toContain("color");
    });
  });

  it("错误态背景容器存在", async () => {
    mockGitFileAtHead.mockRejectedValue(new Error("fail"));
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      const centerDiv = container.querySelector("div");
      expect(centerDiv).toBeTruthy();
      expect(centerDiv!.getAttribute("style")).toContain("background");
    });
  });

  // ── oldPath 优先于 filePath ──

  it("oldPath 和 filePath 同时存在时，gitFileAtHead 传入 oldPath", async () => {
    mockGitFileAtHead.mockResolvedValue("HEAD content");
    render(
      React.createElement(GitShowPanel, {
        params: {
          ...DEFAULT_PARAMS,
          filePath: "src/new.ts",
          oldPath: "src/old.ts",
        },
      }),
    );
    await vi.waitFor(() => {
      expect(mockGitFileAtHead).toHaveBeenCalledWith("C:/repo", "src/old.ts");
    });
  });

  it("oldPath 缺失时，gitFileAtHead 传入 filePath", async () => {
    mockGitFileAtHead.mockResolvedValue("HEAD content");
    render(
      React.createElement(GitShowPanel, {
        params: { ...DEFAULT_PARAMS, filePath: "src/main.ts" },
      }),
    );
    await vi.waitFor(() => {
      expect(mockGitFileAtHead).toHaveBeenCalledWith("C:/repo", "src/main.ts");
    });
  });

  // ── 大文件拒绝 ──

  it("内容超过 MAX_FILE_SIZE_BYTES 时渲染容器", async () => {
    const hugeContent = "x".repeat(10_000_001);
    mockGitFileAtHead.mockResolvedValue(hugeContent);
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      const cmContainer = container.querySelector('div[style*="overflow: clip"]');
      expect(cmContainer).toBeTruthy();
    });
  });

  // ── readOnly 配置 ──

  it("CM6 状态含 readOnly 配置", async () => {
    mockGitFileAtHead.mockResolvedValue("read-only content");
    render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
      const lastConfig = capturedEditorStateConfig[capturedEditorStateConfig.length - 1];
      expect(lastConfig.extensions).toBeDefined();
      // 扩展数组中应包含 { __readOnly: true }（来自 EditorState.readOnly.of(true)）
      const hasReadOnly = (lastConfig.extensions as unknown[]).some(
        (ext) => (ext as Record<string, unknown>).__readOnly === true,
      );
      expect(hasReadOnly).toBe(true);
    });
  });

  // ── 组件卸载清理 ──

  it("卸载时 destroy CM6 EditorView", async () => {
    mockGitFileAtHead.mockResolvedValue("some content");
    const { unmount } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    // 等待 CM6 视图创建完成（EditorState.create 被调用后才算视图就绪）
    await vi.waitFor(() => {
      expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
    });
    // 此时视图已创建但未销毁
    expect(mockEditorViewDestroy).not.toHaveBeenCalled();
    unmount();
    expect(mockEditorViewDestroy).toHaveBeenCalled();
  });

  // ── 竞态：切换 params 时旧请求结果被忽略 ──

  it("快速切换 repoPath 时旧请求结果不覆盖新内容", async () => {
    let resolveFirst: (v: string) => void;
    const firstPromise = new Promise<string>((r) => { resolveFirst = r; });
    const secondContent = "new repo content";

    mockGitFileAtHead
      .mockReturnValueOnce(firstPromise)
      .mockResolvedValueOnce(secondContent);

    const { rerender, container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );

    expect(mockGitFileAtHead).toHaveBeenCalledWith("C:/repo", "src/main.ts");

    rerender(
      React.createElement(GitShowPanel, {
        params: { ...DEFAULT_PARAMS, repoPath: "D:/other-repo" },
      }),
    );

    expect(mockGitFileAtHead).toHaveBeenCalledWith("D:/other-repo", "src/main.ts");

    resolveFirst!("stale content from old repo");
    // 旧结果不应覆盖新内容——容器中应显示第二次加载的内容
    await vi.waitFor(() => {
      const cmContainer = container.querySelector('div[style*="overflow: clip"]');
      expect(cmContainer).toBeTruthy();
    });
  });
	// ── params 变化时进入 loading 态 ──

	it("params 变化后先显示 loading 态再显示新内容", async () => {
	  let resolveSecond: (v: string) => void;
	  const secondPromise = new Promise<string>((r) => { resolveSecond = r; });

	  mockGitFileAtHead
	    .mockResolvedValueOnce("first content")
	    .mockReturnValueOnce(secondPromise);

	  const { rerender, container } = render(
	    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
	  );

	  // 等待首次内容加载完成
	  await vi.waitFor(() => {
	    const cmContainer = container.querySelector('div[style*="overflow: clip"]');
	    expect(cmContainer).toBeTruthy();
	  });

	  // 切换 filePath，第二个请求保持 pending
	  rerender(
	    React.createElement(GitShowPanel, {
	      params: { ...DEFAULT_PARAMS, filePath: "src/other.ts" },
	    }),
	  );

	  // 应立即显示 loading 态（而非残留旧 CM6 容器）
	  const loadingSpan = container.querySelector("span");
	  expect(loadingSpan).toBeTruthy();
	  expect(loadingSpan!.textContent).toBe("加载中...");

	  // 第二个请求完成后显示新内容
	  resolveSecond!("second content");
	  await vi.waitFor(() => {
	    const cmContainer = container.querySelector('div[style*="overflow: clip"]');
	    expect(cmContainer).toBeTruthy();
	  });
	});
});