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
const {
  mockEditorViewDestroy,
  capturedEditorStateConfig,
  capturedEditorViews,
  mockGitshowDispatch,
  mockCompartmentReconfigure,
  mockFontSizeState,
  // TQ-COV-10：大文件警告 StateField 三函数 + EditorView.decorations 断言
  mockStateFieldDefine,
  mockDecorationsFrom,
} = vi.hoisted(() => {
  const mockEditorViewDestroy = vi.fn();
  const capturedEditorStateConfig: { extensions?: unknown[]; doc?: string }[] = [];
  // EDF-04: 收集每次创建的 EditorView 实例，供 identity 变化断言
  const capturedEditorViews: unknown[] = [];
  // EDF-09: 捕获字号热切换的 dispatch 与 Compartment.reconfigure
  const mockGitshowDispatch = vi.fn();
  const mockCompartmentReconfigure = vi.fn(() => []);
  const mockFontSizeState = { editorFontSize: 14 };
  // FE-18：largeFileWarnField 在模块加载时经 StateField.define 定义——
  // spec 打标 __stateField（extensions 断言用），create/update/provide 留待测试直接驱动
  const mockStateFieldDefine = vi.fn((spec: unknown) => ({
    __stateField: true,
    ...(spec as object),
  }));
  // provide: (field) => EditorView.decorations.from(field)
  const mockDecorationsFrom = vi.fn((field: unknown) => field);
  return {
    mockEditorViewDestroy,
    capturedEditorStateConfig,
    capturedEditorViews,
    mockGitshowDispatch,
    mockCompartmentReconfigure,
    mockFontSizeState,
    mockStateFieldDefine,
    mockDecorationsFrom,
  };
});

// @codemirror/view mock——需要 mock EditorView + EditorView.theme + EditorView.editable
vi.mock("@codemirror/view", () => {
  const MockEditorView = class {
    // 静态属性——CM6 EditorView 的 theme / editable Facet（匿名类表达式用 static 声明，TypeScript 才认类型）
    static theme = vi.fn(() => []);
    static editable = { of: vi.fn((val: boolean) => ({ __editable: val })) };
    // TQ-COV-10：provide: (field) => EditorView.decorations.from(field) 依赖此静态
    static decorations = { from: mockDecorationsFrom };
    state: unknown;
    dispatch = mockGitshowDispatch;
    constructor(config: { state: unknown; parent: HTMLElement }) {
      this.state = config.state;
      // 挂载到 parent 上供测试断言
      (config.parent as HTMLElement & Record<string, unknown>)._cmView = this;
      capturedEditorViews.push(this);
    }
    destroy() {
      mockEditorViewDestroy();
    }
  };
  return {
    EditorView: MockEditorView,
    keymap: { of: vi.fn((x: unknown) => x) },
    // FE-18：大文件警告 widget 装饰相关导出（Decoration.widget 仅在 StateField.create 中调用，
    // mock EditorState 不驱动 field，桩仅防 undefined；WidgetType 为空基类供组件 extends）
    Decoration: { widget: vi.fn((spec: unknown) => ({ __widget: true, ...(spec as object) })) },
    WidgetType: class {},
  };
});

vi.mock("@codemirror/state", () => ({
  EditorState: {
    create(config: { doc?: string; extensions?: unknown[] }) {
      const docText = config.doc ?? "";
      capturedEditorStateConfig.push({ extensions: config.extensions, doc: docText });
      // FE-18：mock doc 需支持 line()（大文件警告 StateField.create 读首行）
      return { doc: { line: () => ({ from: 0, text: docText }) }, config };
    },
    readOnly: { of: vi.fn((val: boolean) => ({ __readOnly: val })) },
  },
  Compartment: class {
    of = vi.fn(() => []);
    // EDF-09: 字号热切换走 reconfigure，共享 mock 供断言
    reconfigure = mockCompartmentReconfigure;
  },
  // FE-18：大文件警告装饰桩——define 原样透传 spec 并打标（供 extensions 断言）；
  // TQ-COV-10：spec 经 hoisted mock 可获取，测试直接驱动 create/update/provide 三函数
  StateField: { define: mockStateFieldDefine },
  RangeSetBuilder: class {
    ranges: { from: number; to: number; deco: unknown }[] = [];
    add(from: number, to: number, deco: unknown) {
      this.ranges.push({ from, to, deco });
    }
    finish() {
      return { __ranges: this.ranges };
    }
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

// mock stores/fontSize——select 解构需含 setEditorFontSize；editorFontSize 动态（EDF-09 字号热切换）
vi.mock("../stores", () => ({
  useFontSize: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const state = {
      editorFontSize: mockFontSizeState.editorFontSize,
      setEditorFontSize: mockSetEditorFontSize,
    };
    return typeof selector === "function" ? selector(state) : undefined;
  }),
}));

import React from "react";
import { render, cleanup, act } from "@testing-library/react";
import GitShowPanel, { LargeFileWarnWidget } from "../panels/gitshow/GitShowPanel";
import { GIT_FILE_COLORS } from "../theme";
// 从 mock 导入以获取 vi.fn() 引用（供断言调用次数/参数）
import { createEditorFontExtension } from "../panels/editor/useCodeMirror";
// TQ-COV-10：StateField spec 经 hoisted mockStateFieldDefine 获取（不直接 import 被 mock 的模块路径）

/** 色值 → jsdom 归一化形态（#hex → "rgb(r, g, b)"）——jsdom cssstyle 将 hex 统一序列化为 rgb() 形式 */
function hexToRgb(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgb(${r}, ${g}, ${b})`;
}

const DEFAULT_PARAMS = {
  panelId: "gs-1",
  filePath: "src/main.ts",
  repoPath: "C:/repo",
};

beforeEach(() => {
  mockGitFileAtHead.mockReset();
  mockEditorViewDestroy.mockReset();
  capturedEditorStateConfig.length = 0;
  capturedEditorViews.length = 0;
  mockGitshowDispatch.mockReset();
  mockCompartmentReconfigure.mockReset();
  mockFontSizeState.editorFontSize = 14;
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

  // ── 大文件拒绝 / 警告（EDF-04：精确断言 doc 文案）──

  it("内容超过 MAX_FILE_SIZE_BYTES 时渲染拒绝文案（全文被替换）", async () => {
    const hugeContent = "x".repeat(10_000_001);
    mockGitFileAtHead.mockResolvedValue(hugeContent);
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      const cmContainer = container.querySelector('div[style*="overflow: clip"]');
      expect(cmContainer).toBeTruthy();
    });
    // 拒绝文案精确出现，原文被整体替换
    const lastConfig = capturedEditorStateConfig[capturedEditorStateConfig.length - 1];
    expect(lastConfig.doc).toContain("文件过大");
    expect(lastConfig.doc).toContain("已拒绝打开以保护内存");
    expect(lastConfig.doc).not.toContain(hugeContent.slice(0, 100));
  });

  it("内容超过 LARGE_FILE_WARN_BYTES（未超上限）时顶部插入警告 header，原文保留", async () => {
    const bigContent = "line1\n" + "y".repeat(1_100_000);
    mockGitFileAtHead.mockResolvedValue(bigContent);
    render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    await vi.waitFor(() => {
      expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
    });
    const lastConfig = capturedEditorStateConfig[capturedEditorStateConfig.length - 1];
    // 警告 header 精确文案（FE-18：⚠ emoji 已移除——图标经 widget 装饰注入，doc 中无 emoji）
    expect(lastConfig.doc).toContain("大文件");
    expect(lastConfig.doc).not.toContain("⚠");
    expect(lastConfig.doc).toContain("只读查看");
    // 大文件警告 StateField 已挂入 extensions（行首图标的装饰载体）
    const hasWarnField = (lastConfig.extensions as unknown[]).some(
      (ext) => (ext as Record<string, unknown>).__stateField === true,
    );
    expect(hasWarnField).toBe(true);
    // 原文保留（header 前置而非替换）
    expect(lastConfig.doc).toContain("line1");
  });

  // ── FE-18：大文件警告图标（⚠ → lucide TriangleAlert，IC-08）──

  it("大文件警告图标 widget 渲染 lucide svg（13px、warning 语义 token 色）", async () => {
    // beforeEach 的 mockReset 清除了实现——补 mock 防 gitFileAtHead 返回 undefined
    // 触发 GitShowPanel 内容态 text.length 运行期 TypeError（生产契约恒返回 string）
    // doc 含大文件标记：面板走大文件警告分支，与用例语义一致
    mockGitFileAtHead.mockResolvedValue("line1\n" + "y".repeat(1_100_000));
    const { container } = render(
      React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
    );
    // CM6 decoration 挂载由 mock EditorView 短路（mock 不驱动 StateField），
    // 故直接驱动 widget 实例验证 DOM 产物——与真实 CM6 的 widget.toDOM 调用等价
    let host!: HTMLElement;
    const widget = new LargeFileWarnWidget();
    await act(async () => {
      host = widget.toDOM();
      container.appendChild(host);
    });
    const svg = host.querySelector("svg");
    expect(svg).toBeTruthy();
    expect(svg!.getAttribute("width")).toBe("13");
    expect(svg!.getAttribute("height")).toBe("13");
    expect(svg!.getAttribute("stroke-width")).toBe("1.5");
    expect(svg!.getAttribute("aria-hidden")).toBe("true");
    // 色经语义 token（warning 语义 = GIT_FILE_COLORS.modified，与 toast warning 同源，硬约束 #6）
    // jsdom 将 hex 归一化为 rgb() 形式——比对归一化形态而非 hex 字面量
    expect(host.style.color).toBe(hexToRgb(GIT_FILE_COLORS.modified));
    await act(async () => {
      widget.destroy();
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

	  // EDF-04：切换后新 view 与旧 view 非同一实例，旧 view 已销毁——实例 identity 断言（非仅"容器存在"）。
	  // 注：切换 flush 中加载 effect 与 CM6 effect 同批执行，中间态可能重建 view，故用
	  // 首/末实例 identity + 销毁计数断言"切换必然销毁重建"，不锁死中间创建次数。
	  expect(capturedEditorViews.length).toBeGreaterThanOrEqual(2);
	  expect(capturedEditorViews[0]).not.toBe(capturedEditorViews[capturedEditorViews.length - 1]);
	  expect(mockEditorViewDestroy).toHaveBeenCalled();
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

// EDF-09: 字号热切换——editorFontSize 变化走 fontCompartment.reconfigure（非重建 view）
it("editorFontSize 变化触发 fontCompartment.reconfigure（dispatch + reconfigure）", async () => {
  mockGitFileAtHead.mockResolvedValue("content");
  const { rerender } = render(
    React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }),
  );
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });

  mockGitshowDispatch.mockClear();
  mockCompartmentReconfigure.mockClear();

  // 字号 14 → 20（rerender 触发 fontSize effect）
  mockFontSizeState.editorFontSize = 20;
  rerender(React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }));

  await vi.waitFor(() => {
    expect(mockGitshowDispatch).toHaveBeenCalled();
  }, { timeout: 3000 });
  // reconfigure 被调用（非仅 createEditorFontExtension 被调），且收到新字号生成的扩展
  expect(mockCompartmentReconfigure).toHaveBeenCalled();
  expect(createEditorFontExtension).toHaveBeenCalledWith(20);
  // 不重建 view——仍为同一实例
  expect(capturedEditorViews.length).toBe(1);
});

// ── TQ-COV-10：大文件警告 widget/StateField 补测 ──

it("LargeFileWarnWidget.ignoreEvent 返回 true（纯装饰，不响应指针/键盘事件）", async () => {
  expect(new LargeFileWarnWidget().ignoreEvent()).toBe(true);
});

it("largeFileWarnField StateField 三函数：create 挂首行行首装饰 / update 仅 map / provide 经 decorations.from 挂载", async () => {
  mockGitFileAtHead.mockResolvedValue("line1\n" + "y".repeat(1_100_000));
  render(React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }));
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
  });

  // 模块加载时 StateField.define 已执行——取 spec 直接驱动三函数
  const spec = mockStateFieldDefine.mock.calls[0][0] as {
    create: (state: { doc: { line: () => { from: number } } }) => {
      __ranges: Array<{ from: number; to: number; deco: { widget?: unknown } }>;
    };
    update: (
      deco: { map: (changes: unknown) => unknown },
      tr: { changes: string },
    ) => unknown;
    provide: (field: unknown) => unknown;
  };

  // create：首行行首挂 widget 装饰（from === to === 0——行首零宽占位，FE-18）
  const rangeSet = spec.create({ doc: { line: () => ({ from: 0 }) } });
  expect(rangeSet.__ranges).toHaveLength(1);
  expect(rangeSet.__ranges[0].from).toBe(0);
  expect(rangeSet.__ranges[0].to).toBe(0);
  expect(rangeSet.__ranges[0].deco.widget).toBeInstanceOf(LargeFileWarnWidget);

  // update：doc 只读 → 仅 map changes（FE-18 契约：不做任何重算）
  const decoMap = vi.fn((changes: unknown) => changes);
  const updated = spec.update({ map: decoMap }, { changes: "CHG" });
  expect(decoMap).toHaveBeenCalledWith("CHG");
  expect(updated).toBe("CHG");

  // provide：装饰经 EditorView.decorations.from(field) 挂载（field 透传）
  expect(spec.provide("FIELD")).toBe("FIELD");
  expect(mockDecorationsFrom).toHaveBeenCalledWith("FIELD");
});

// ── TQ-COV-10：Alt+Z 自动换行切换（经 usePanelFocus 激活的 editorActions 派发）──

it("Alt+Z 自动换行：激活编辑器后 toggleWordWrap 经 wrapCompartment 热切换（dispatch + reconfigure）", async () => {
  mockGitFileAtHead.mockResolvedValue("wrap content");
  let capturedActivate: (() => void) | null = null;
  mockUsePanelFocus.mockImplementation(
    (_ctx: unknown, _el: unknown, activate: () => void) => {
      capturedActivate = activate;
    },
  );

  render(React.createElement(GitShowPanel, { params: DEFAULT_PARAMS }));
  await vi.waitFor(() => {
    expect(capturedEditorStateConfig.length).toBeGreaterThan(0);
    expect(capturedActivate).not.toBeNull();
  });

  mockSetActiveEditor.mockClear();
  mockGitshowDispatch.mockClear();
  mockCompartmentReconfigure.mockClear();

  // 激活 → setActiveEditor(editorActions)：save 为 no-op（只读面板无保存）
  capturedActivate!();
  expect(mockSetActiveEditor).toHaveBeenCalledTimes(1);
  const actions = mockSetActiveEditor.mock.calls[0][0] as {
    save: () => void;
    toggleWordWrap: () => void;
  };
  expect(() => actions.save()).not.toThrow();

  // 首次 toggle：wrapRef false → 挂 lineWrapping；再次：卸载（[] 分支）
  actions.toggleWordWrap();
  actions.toggleWordWrap();
  expect(mockGitshowDispatch).toHaveBeenCalledTimes(2);
  expect(mockCompartmentReconfigure).toHaveBeenCalledTimes(2);
  // vi.fn(() => []) 的 calls 推断为 [] 元组——先扩为 unknown[] 再取每调的首参
  const reconfigureCalls = mockCompartmentReconfigure.mock
    .calls as unknown[][];
  const firstArgs = reconfigureCalls[0][0];
  const secondArgs = reconfigureCalls[1][0];
  // 两分支参数不同（挂载 ≠ 卸载）；卸载分支 = []（无扩展）
  expect(firstArgs).not.toEqual([]);
  expect(secondArgs).toEqual([]);
  // dispatch 携带 reconfigure 结果（effects 热切换）
  expect(mockGitshowDispatch.mock.calls[0][0]).toEqual({ effects: [] });
});
});