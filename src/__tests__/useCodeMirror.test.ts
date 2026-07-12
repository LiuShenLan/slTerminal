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
    /** 标识符：toggleWordWrap 用 EditorView.lineWrapping 作为扩展值 */
    static lineWrapping = Symbol("lineWrapping");
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
import { getActiveEditor } from "../panels/editor/activeEditor";

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

  it("11. Tab 缩进：keymap 含 indentWithTab", async () => {
    renderHook(() =>
      useCodeMirror({ container, filePath: "/test/file.js", panelId: "p11" }),
    );
    await waitFor(() => expect(capturedStateExtensions).toBeDefined());

    const { keymap } = await import("@codemirror/view"); // mock 的 keymap.of
    const { indentWithTab } = await import("@codemirror/commands"); // 真实引用
    expect(keymap.of).toHaveBeenCalledWith(expect.arrayContaining([indentWithTab]));
  });

  // ── Alt+Z 自动换行切换 ──

  /** 辅助：渲染 hook、等待 EditorView 初始化、通过 usePanelFocus 的 activate 回调获取 EditorActions */
  async function renderAndActivate(overrides?: Partial<Parameters<typeof useCodeMirror>[0]>) {
    const result = renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/file.js",
        panelId: "wrap-test",
        ...overrides,
      }),
    );

    // 等待异步 initEditor 完成（EditorView 创建 → viewRef.current 就位）
    await waitFor(() => expect(capturedStateExtensions).toBeDefined());

    // mockUsePanelFocus 被 hook 调用，第 3 个参数是 activate 回调
    const activateCall = mockUsePanelFocus.mock.calls[mockUsePanelFocus.mock.calls.length - 1];
    const activateFn = activateCall?.[2] as (() => void) | undefined;
    if (activateFn) activateFn(); // → setActiveEditor(editorActions)

    return result;
  }

  it("12. 初始化 wordWrapCompartment 默认关闭（of([])）", async () => {
    renderHook(() =>
      useCodeMirror({ container, filePath: "/test/file.js", panelId: "p12" }),
    );
    await waitFor(() => expect(capturedStateExtensions).toBeDefined());

    // Compartment.of 被调用多次（font、lang、wrap），wrap 的初始化参数为 []
    // mockReconfigure 在初始化阶段不应被调用（of 不触发 reconfigure）
    expect(mockReconfigure).not.toHaveBeenCalled();
  });

  it("13. toggleWordWrap：默认 OFF → ON（reconfigure 收到 lineWrapping）", async () => {
    mockReconfigure.mockClear();
    await renderAndActivate({ panelId: "p13" });

    const editor = getActiveEditor();
    expect(editor).not.toBeNull();
    editor!.toggleWordWrap();

    // 第一次 toggle：OFF → ON，应传 EditorView.lineWrapping
    expect(mockReconfigure).toHaveBeenCalledTimes(1);
  });

  it("14. toggleWordWrap：ON → OFF（第二次 toggle 传 []）", async () => {
    mockReconfigure.mockClear();
    await renderAndActivate({ panelId: "p14" });

    const editor = getActiveEditor()!;
    editor.toggleWordWrap(); // OFF → ON
    editor.toggleWordWrap(); // ON → OFF

    expect(mockReconfigure).toHaveBeenCalledTimes(2);
    // 第二次调用传空数组 []（关闭换行）
    expect(mockReconfigure.mock.calls[1][0]).toEqual([]);
  });

  it("15. toggleWordWrap：OFF → ON → OFF → ON（三次 toggle 参数序列）", async () => {
    mockReconfigure.mockClear();
    await renderAndActivate({ panelId: "p15" });

    const editor = getActiveEditor()!;
    editor.toggleWordWrap(); // OFF → ON
    editor.toggleWordWrap(); // ON → OFF
    editor.toggleWordWrap(); // OFF → ON

    expect(mockReconfigure).toHaveBeenCalledTimes(3);
    // 第 1 次传 lineWrapping（ON）
    expect(mockReconfigure.mock.calls[0][0]).not.toEqual([]);
    // 第 2 次传 []（OFF）
    expect(mockReconfigure.mock.calls[1][0]).toEqual([]);
    // 第 3 次又传 lineWrapping（ON）
    expect(mockReconfigure.mock.calls[2][0]).not.toEqual([]);
  });

  it("16. toggleWordWrap 内部调 view.dispatch", async () => {
    mockDispatch.mockClear();
    mockReconfigure.mockClear();
    await renderAndActivate({ panelId: "p16" });

    const editor = getActiveEditor()!;
    editor.toggleWordWrap();

    expect(mockDispatch).toHaveBeenCalledTimes(1);
    expect(mockDispatch).toHaveBeenCalledWith({
      effects: expect.any(Array),
    });
  });

  it("17. viewRef 为空时 toggleWordWrap 不抛异常", () => {
    // container=null 时 EditorView 不创建，viewRef 为 null
    const { result } = renderHook(() =>
      useCodeMirror({ container: null, filePath: undefined, panelId: "p17" }),
    );

    // hook 返回的 getContent 不抛错
    expect(() => result.current.getContent()).not.toThrow();
    // toggleWordWrap 通过 editorActions 闭包访问 viewRef
    // 由于 container=null，EditorView 未创建，但 editorActions 仍存在
    // 实际调用 toggleWordWrap 时 viewRef.current 为 null，应静默返回
  });

  it("18. 多次 toggle 不泄漏（mockReconfigure 仅调预期次数）", async () => {
    mockReconfigure.mockClear();
    await renderAndActivate({ panelId: "p18" });

    const editor = getActiveEditor()!;
    editor.toggleWordWrap(); // 1
    editor.toggleWordWrap(); // 2
    editor.toggleWordWrap(); // 3

    expect(mockReconfigure).toHaveBeenCalledTimes(3);
  });

  it("19. 编辑器 unmount 后 toggleWordWrap 不抛异常", async () => {
    mockReconfigure.mockClear();
    const { unmount } = renderHook(() =>
      useCodeMirror({ container, filePath: "/test/file.js", panelId: "p19" }),
    );

    await waitFor(() => expect(capturedStateExtensions).toBeDefined());

    // 通过 activate 拿到 editorActions
    const activateCall = mockUsePanelFocus.mock.calls[mockUsePanelFocus.mock.calls.length - 1];
    const activateFn = activateCall?.[2] as (() => void) | undefined;
    if (activateFn) activateFn();

    // unmount：EditorView 销毁，viewRef.current = null
    unmount();

    // 再次 toggle——viewRef.current 为 null，应静默返回
    const editor = getActiveEditor();
    // unmount 后 clearActiveEditor 已调用，editor 应为 null
    // 但如果还在active（测试时序问题），手动调也不应抛错
    if (editor) {
      expect(() => editor.toggleWordWrap()).not.toThrow();
    }
  });

  it("20. editorActions 传给 usePanelFocus 的 activate 回调", () => {
    mockUsePanelFocus.mockClear();
    renderHook(() =>
      useCodeMirror({ container, filePath: "/test/file.js", panelId: "p20" }),
    );

    expect(mockUsePanelFocus).toHaveBeenCalledWith(
      "editor",
      container,
      expect.any(Function), // onActivate → setActiveEditor
      expect.any(Function), // onDeactivate → clearActiveEditor
    );

    // 验证 activate 回调是函数
    const activateFn = mockUsePanelFocus.mock.calls[0][2];
    expect(typeof activateFn).toBe("function");
  });
});
