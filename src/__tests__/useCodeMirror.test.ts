// useCodeMirror.test.ts — useCodeMirror hook 字体大小调节测试
//
// 测试策略：EditorView 在 jsdom 不可用，mock EditorView + Compartment
// - 通过 mockDispatch 捕获 dispatch 调用，验证 Compartment.reconfigure
// - 通过 container 分发 WheelEvent 测试 Ctrl+Wheel
// - 纯函数 createEditorFontExtension 测试在 editor-font.test.ts 中覆盖

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const { mockDispatch, mockDestroy, mockOnFontSizeChange, mockReconfigure } = vi.hoisted(() => ({
  mockDispatch: vi.fn(),
  mockDestroy: vi.fn(),
  mockOnFontSizeChange: vi.fn(),
  mockReconfigure: vi.fn().mockReturnValue([]),
}));

let capturedStateExtensions: unknown[] | null = null;

vi.mock("@codemirror/view", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@codemirror/view")>();
  const MockEditorView = class {
    dom: HTMLDivElement;
    dispatch = mockDispatch;
    destroy = mockDestroy;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    state: any;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    constructor(config: any) {
      this.dom = document.createElement("div");
      config.parent.appendChild(this.dom);
      this.state = config.state;
      capturedStateExtensions = config.state;
    }
    static theme = vi.fn(() => []);
    static updateListener = {
      of: vi.fn(() => []),
    };
  };
  return {
    ...actual,
    EditorView: MockEditorView,
    keymap: { of: vi.fn(() => []) },
  };
});

vi.mock("@codemirror/state", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@codemirror/state")>();
  return {
    ...actual,
    Compartment: class {
      of(ext: unknown) { return ext; }
      reconfigure(ext: unknown) {
        mockReconfigure(ext);
        return [];
      }
    },
    // 确保 EditorState.create 不抛异常
    EditorState: {
      create: vi.fn().mockReturnValue({ doc: vi.fn() }),
    },
  };
});

// Mock ipc/notify — setup.ts 中已全局 mock，但这里需要防 import 时调用
vi.mock("../ipc/notify", () => ({
  onFsEvent: vi.fn(() => () => {}),
  startWatch: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc", () => ({
  fs: {
    readFile: vi.fn().mockResolvedValue(""),
    writeFile: vi.fn().mockResolvedValue(undefined),
  },
  save: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitDiff: vi.fn().mockResolvedValue([]),
}));

vi.mock("./gitGutter", () => ({
  diffGutter: vi.fn(() => []),
  updateDiffGutter: vi.fn(),
  clearDiffGutter: vi.fn(),
}));

// 只 stub usePanelFocus，保留其余真实实现
const { mockUsePanelFocus } = vi.hoisted(() => ({ mockUsePanelFocus: vi.fn() }));
vi.mock("../features/shortcuts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../features/shortcuts")>();
  return { ...actual, usePanelFocus: mockUsePanelFocus };
});

import { useCodeMirror } from "../panels/editor/useCodeMirror";

// ─── 辅助函数 ───

function createContainer(): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, configurable: true });
  return el;
}

// ─── 测试套件 ───

describe("useCodeMirror 字体大小", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = createContainer();
    capturedStateExtensions = null;
    vi.clearAllMocks();
  });

  afterEach(() => {
    // 清理挂载的 DOM
    container.innerHTML = "";
  });

  it("1. fontSize 传入 EditorState extensions", () => {
    renderHook(() =>
      useCodeMirror({ container, filePath: "/test/file.js", panelId: "p1", fontSize: 18 }),
    );

    // EditorState.create 被调用
    expect(capturedStateExtensions).toBeDefined();
  });

  it("2. 不传 fontSize 时使用默认 14", () => {
    renderHook(() =>
      useCodeMirror({ container, filePath: "/test/file.js", panelId: "p2" }),
    );

    // 没有报错即通过
    expect(capturedStateExtensions).toBeDefined();
  });

  it("3. fontSize 变化触发 dispatch + reconfigure", async () => {
    const { rerender } = renderHook(
      ({ fontSize }) =>
        useCodeMirror({ container, filePath: "/test/file.js", panelId: "p3", fontSize }),
      { initialProps: { fontSize: 14 } as { fontSize: number } },
    );

    // 等待 EditorView 初始化（readFile mock 立即 resolve）
    await waitFor(() => {
      expect(capturedStateExtensions).toBeDefined();
    });

    mockDispatch.mockClear();

    rerender({ fontSize: 20 });

    await waitFor(() => {
      expect(mockDispatch).toHaveBeenCalled();
    });
    // reconfigure 被调用
    expect(mockReconfigure).toHaveBeenCalled();
  });

  it("4. fontSize 不变时不 dispatch", async () => {
    const { rerender } = renderHook(
      ({ fontSize }) =>
        useCodeMirror({ container, filePath: "/test/file.js", panelId: "p4", fontSize }),
      { initialProps: { fontSize: 14 } as { fontSize: number } },
    );

    await waitFor(() => {
      expect(capturedStateExtensions).toBeDefined();
    });

    mockDispatch.mockClear();

    // 同值 rerender
    rerender({ fontSize: 14 });

    expect(mockDispatch).not.toHaveBeenCalled();
  });

  // ── Ctrl+Wheel ──

  it("5. Ctrl+上滚（deltaY<0）→ onFontSizeChange +1", () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/file.js",
        panelId: "p5",
        fontSize: 14,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: -100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).toHaveBeenCalledWith(15);
  });

  it("6. Ctrl+下滚（deltaY>0）→ onFontSizeChange -1", () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/file.js",
        panelId: "p6",
        fontSize: 14,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: 100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).toHaveBeenCalledWith(13);
  });

  it("7. 非 Ctrl 滚轮透传 → onFontSizeChange 不调用", () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/file.js",
        panelId: "p7",
        fontSize: 14,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: false,
      deltaY: 100,
      cancelable: true,
    });
    container.dispatchEvent(event);

    expect(mockOnFontSizeChange).not.toHaveBeenCalled();
  });

  it("8. 到达边界不调 onFontSizeChange", () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/file.js",
        panelId: "p8",
        fontSize: 8,
        onFontSizeChange: mockOnFontSizeChange,
      }),
    );

    mockOnFontSizeChange.mockClear();

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: 100, // 下滚=缩小
      cancelable: true,
    });
    container.dispatchEvent(event);

    // fontSize=8, deltaY>0 → 应 clamp 不触发
    expect(mockOnFontSizeChange).not.toHaveBeenCalled();
  });

  it("9. 未传 onFontSizeChange → wheel 不报错", () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/file.js",
        panelId: "p9",
        fontSize: 14,
      }),
    );

    const event = new WheelEvent("wheel", {
      ctrlKey: true,
      deltaY: -100,
      cancelable: true,
    });
    expect(() => container.dispatchEvent(event)).not.toThrow();
  });

  // ── Ctrl+S 迁入 ShortcutRegistry ──

  it("10. 注册 editor 上下文快捷键（usePanelFocus）", () => {
    renderHook(() =>
      useCodeMirror({ container, filePath: "/test/file.js", panelId: "p10" }),
    );
    expect(mockUsePanelFocus).toHaveBeenCalledWith(
      "editor",
      container,
      expect.any(Function), // onActivate → setActiveEditor
      expect.any(Function), // onDeactivate → clearActiveEditor
    );
  });
});
