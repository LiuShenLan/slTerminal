// gitshow-panel.test.tsx — GitShowPanel L2 测试
//
// 覆盖：loading 态 / 内容渲染 / 错误占位文案 /
// oldPath 优先于 filePath / readOnly 配置。

import { describe, it, expect, afterEach, vi, beforeEach } from "vitest";

// mock IPC git 模块——gitFileAtHead 返回模拟内容
const { mockGitFileAtHead } = vi.hoisted(() => ({
  mockGitFileAtHead: vi.fn(),
}));

// 新增功能相关 mock
const {
  mockUseFontSizeWheel,
  mockUsePanelFocus,
  mockSetActiveEditor,
  mockClearActiveEditor,
  mockSetEditorFontSize,
  mockSearchFn,
  mockHighlightMatchesFn,
} = vi.hoisted(() => ({
  mockUseFontSizeWheel: vi.fn(),
  mockUsePanelFocus: vi.fn(),
  mockSetActiveEditor: vi.fn(),
  mockClearActiveEditor: vi.fn(),
  mockSetEditorFontSize: vi.fn(),
  mockSearchFn: vi.fn(() => [{ __searchPanel: true }]),
  mockHighlightMatchesFn: vi.fn(() => [{ __highlightMatches: true }]),
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
    keymap: { of: vi.fn((x: unknown) => x) },
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
  Compartment: class {
    of = vi.fn(() => []);
  },
}));

vi.mock("@codemirror/theme-one-dark", () => ({
  oneDark: [],
}));

vi.mock("codemirror", () => ({
  basicSetup: [],
}));

// 新增：@codemirror/search mock
vi.mock("@codemirror/search", () => ({
  search: mockSearchFn,
  searchKeymap: [{ __searchKeymap: true }],
  highlightSelectionMatches: mockHighlightMatchesFn,
}));

// 新增：useFontSizeWheel mock
vi.mock("../lib/useFontSizeWheel", () => ({
  useFontSizeWheel: mockUseFontSizeWheel,
}));

// 新增：usePanelFocus mock
vi.mock("../features/shortcuts", () => ({
  usePanelFocus: mockUsePanelFocus,
}));

// 新增：activeEditor mock
vi.mock("../panels/editor/activeEditor", () => ({
  setActiveEditor: mockSetActiveEditor,
  clearActiveEditor: mockClearActiveEditor,
}));

// 新增：fontSize 常量 mock
vi.mock("../stores/fontSize", () => ({
  FONT_SIZE_MIN: 8,
  FONT_SIZE_MAX: 32,
}));

// mock useCodeMirror 导出——供 GitShowPanel import 复用
vi.mock("../panels/editor/useCodeMirror", () => ({
  getLanguageExtension: vi.fn(() => []),
  MAX_FILE_SIZE_BYTES: 10_000_000,
  LARGE_FILE_WARN_BYTES: 1_000_000,
  createEditorFontExtension: vi.fn(() => []),
}));

// mock stores/fontSize——select 解构需含 setEditorFontSize
vi.mock("../stores", () => ({
  useFontSize: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const state = { editorFontSize: 14, setEditorFontSize: mockSetEditorFontSize };
    return typeof selector === "function" ? selector(state) : undefined;
  }),
}));

import React from "react";
import { render, cleanup } from "@testing-library/react";
import GitShowPanel from "../panels/gitshow/GitShowPanel";
// 从 mock 导入以获取 vi.fn() 引用（供断言调用次数/参数）
import { createEditorFontExtension } from "../panels/editor/useCodeMirror";

const DEFAULT_PARAMS = {
  panelId: "gs-1",
  filePath: "src/main.ts",
  repoPath: "C:/repo",
};

beforeEach(() => {
  mockGitFileAtHead.mockReset();
  mockEditorViewDestroy.mockReset();
  capturedEditorStateConfig.length = 0;
  mockUseFontSizeWheel.mockReset();
  mockUsePanelFocus.mockReset();
  mockSetActiveEditor.mockReset();
  mockClearActiveEditor.mockReset();
  mockSetEditorFontSize.mockReset();
  // 清除 createEditorFontExtension 调用记录（vi.fn 在 mock factory 中创建，通过 import 获取同一引用）
  (createEditorFontExtension as ReturnType<typeof vi.fn>).mockClear?.();
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

// ── 新增功能测试（14-18）──

// 14: 搜索扩展——验证 @codemirror/search 函数被调用
it("includes @codemirror/search extensions in CM6 config", async () => {
  mockGitFileAtHead.mockResolvedValue("searchable content");
  render(
    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
  );
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });
  // search({ top: true }) 被调用
  expect(mockSearchFn).toHaveBeenCalledWith({ top: true });
  // highlightSelectionMatches() 被调用
  expect(mockHighlightMatchesFn).toHaveBeenCalled();
});

// 15: useFontSizeWheel——取非 null container 的调用（content 渲染后）
it("calls useFontSizeWheel with correct params", async () => {
  mockGitFileAtHead.mockResolvedValue("some content");
  render(
    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
  );
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });
  // 取最后一次调用（content 渲染后 containerRef.current 为 DOM 元素）
  const calls = mockUseFontSizeWheel.mock.calls;
  const lastCall = calls[calls.length - 1];
  expect(lastCall[0]).toBeInstanceOf(HTMLElement);
  expect(lastCall[1]).toBe(8);  // FONT_SIZE_MIN
  expect(lastCall[2]).toBe(32); // FONT_SIZE_MAX
  expect(lastCall[4]).toBe(mockSetEditorFontSize);
});

// 16: usePanelFocus
it("registers editor focus via usePanelFocus", async () => {
  mockGitFileAtHead.mockResolvedValue("some content");
  render(
    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
  );
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });
  const calls = mockUsePanelFocus.mock.calls;
  const lastCall = calls[calls.length - 1];
  expect(lastCall[0]).toBe("editor");
  expect(lastCall[1]).toBeInstanceOf(HTMLElement);
});

// 17: createEditorFontExtension 在 Compartment.of 中被调用
it("createEditorFontExtension called with default fontSize 14", async () => {
  mockGitFileAtHead.mockResolvedValue("hello");
  render(
    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
  );
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });
  expect(createEditorFontExtension).toHaveBeenCalledWith(14);
});

// 18: CM6 创建 effect deps 不含 editorFontSize
it("CM6 creation effect does not recreate view on fontSize change", async () => {
  mockGitFileAtHead.mockResolvedValue("content");
  render(
    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
  );
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });
  const afterCreation = capturedEditorStateConfig.length;
  expect(afterCreation).toBeGreaterThan(0);
  expect(createEditorFontExtension).toHaveBeenCalledWith(14);
});

// 19: editable.of(false) 已移除——编辑器应保持可聚焦
it("does NOT disable editability via editable.of(false)", async () => {
  mockGitFileAtHead.mockResolvedValue("focusable content");
  render(
    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
  );
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });
  const lastConfig = capturedEditorStateConfig[capturedEditorStateConfig.length - 1];
  const exts = lastConfig.extensions as Record<string, unknown>[];
  const hasEditableFalse = exts.some(
    (e) => (e as Record<string, unknown>).__editable === false,
  );
  expect(hasEditableFalse).toBe(false);
});
});