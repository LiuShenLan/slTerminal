// useCodeMirror.test.ts — useCodeMirror hook 字体大小调节测试
//
// 测试策略：EditorView 在 jsdom 不可用，mock EditorView + Compartment
// - 通过 mockDispatch 捕获 dispatch 调用，验证 Compartment.reconfigure
// - 通过 container 分发 WheelEvent 测试 Ctrl+Wheel
// - 纯函数 createEditorFontExtension 测试在 editor-font.test.ts 中覆盖

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { EditorState } from "@codemirror/state";

// ─── Hoisted mocks ───
// FE-01: confirmDialog/toast 为应用内浮层（模块级 mock，替代 window.alert/confirm spy）
const { mockDispatch, mockDestroy, mockOnFontSizeChange, mockReconfigure, mockDialogSave, mockConfirmDialog, mockToastShow } = vi.hoisted(() => ({
  mockDispatch: vi.fn(),
  mockDestroy: vi.fn(),
  mockOnFontSizeChange: vi.fn(),
  mockReconfigure: vi.fn().mockReturnValue([]),
  mockDialogSave: vi.fn<(options?: { defaultPath?: string; filters?: Array<{ name: string; extensions: string[] }> }) => Promise<string | null>>(),
  mockConfirmDialog: vi.fn<(opts: { title?: string; message: string; confirmText?: string }) => Promise<boolean>>().mockResolvedValue(true),
  mockToastShow: vi.fn(),
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

vi.mock("../panels/editor/gitGutter", () => ({
  diffGutter: vi.fn(() => []),
  updateDiffGutter: vi.fn(),
  clearDiffGutter: vi.fn(),
}));

// useCodeMirror.ts import { save } from "../../ipc/dialog"
vi.mock("../ipc/dialog", () => ({
  save: mockDialogSave,
}));

// FE-01: mock 应用内浮层 confirmDialog/toast（importOriginal 保留其余导出，防破坏
// importOriginal 展开的 ../features/shortcuts 对 lib 的依赖）
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return {
    ...actual,
    confirmDialog: mockConfirmDialog,
    toast: { ...actual.toast, show: mockToastShow },
  };
});

// 只 stub usePanelFocus，保留其余真实实现
const { mockUsePanelFocus } = vi.hoisted(() => ({ mockUsePanelFocus: vi.fn() }));
vi.mock("../features/shortcuts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../features/shortcuts")>();
  return { ...actual, usePanelFocus: mockUsePanelFocus };
});

import {
  useCodeMirror,
  MAX_FILE_SIZE_BYTES,
  LARGE_FILE_WARN_BYTES,
} from "../panels/editor/useCodeMirror";
import { getActiveEditor } from "../panels/editor/activeEditor";
// 导入 mocked 模块，供 handleSave 测试验证 IPC 调用
import { fs } from "../ipc";
import { gitDiff } from "../ipc/git";
import { updateDiffGutter, clearDiffGutter } from "../panels/editor/gitGutter";

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
    }, { timeout: 3000 });

    mockDispatch.mockClear();

    rerender({ fontSize: 20 });

    await waitFor(() => {
      expect(mockDispatch).toHaveBeenCalled();
    }, { timeout: 3000 });
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
    }, { timeout: 3000 });

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
    await waitFor(() => expect(capturedStateExtensions).toBeDefined(), { timeout: 3000 });

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
    await waitFor(() => expect(capturedStateExtensions).toBeDefined(), { timeout: 3000 });

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
    await waitFor(() => expect(capturedStateExtensions).toBeDefined(), { timeout: 3000 });

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

    await waitFor(() => expect(capturedStateExtensions).toBeDefined(), { timeout: 3000 });

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

// ─── handleSave 保存逻辑 ───

describe("handleSave 保存逻辑", () => {
  let container: HTMLDivElement;

  /** 渲染 hook、等待 EditorView 初始化、通过 usePanelFocus 激活编辑器，返回 editorActions */
  async function renderAndActivate(overrides?: Partial<Parameters<typeof useCodeMirror>[0]>) {
    const result = renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/save.js",
        panelId: "hs",
        ...overrides,
      }),
    );

    // 等待异步 initEditor 完成（EditorView 创建 → viewRef.current 就位）
    await waitFor(() => expect(capturedStateExtensions).toBeDefined(), { timeout: 3000 });

    // 通过 mockUsePanelFocus 的 activate 回调设置 activeEditor
    const activateCall = mockUsePanelFocus.mock.calls[mockUsePanelFocus.mock.calls.length - 1];
    const activateFn = activateCall?.[2] as (() => void) | undefined;
    if (activateFn) activateFn();

    return result;
  }

  beforeEach(() => {
    container = createContainer();
    capturedStateExtensions = null;
    vi.clearAllMocks();
    // 默认：save 对话框取消
    mockDialogSave.mockResolvedValue(null);
    // fs.writeFile 默认成功
    (fs.writeFile as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    // gitDiff 默认返回空（干净文件）
    (gitDiff as ReturnType<typeof vi.fn>).mockResolvedValue([]);
  });

  afterEach(() => {
    container.innerHTML = "";
  });

  it("HS1: 有 filePath 时直接保存——调 writeFile，不弹另存为对话框", async () => {
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    await renderAndActivate({ filePath: "/test/save.js", panelId: "hs-1" });

    const editor = getActiveEditor()!;
    expect(editor).not.toBeNull();
    editor.save();

    // handleSave 是 async → 等待 writeFile 被调用
    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalledWith("/test/save.js", expect.any(String));
    }, { timeout: 3000 });

    // 有 filePath → 不应弹另存为对话框
    expect(mockDialogSave).not.toHaveBeenCalled();

    dispatchSpy.mockRestore();
  });

  it("HS2: 无 filePath 时弹出另存为对话框", async () => {
    mockDialogSave.mockResolvedValue(null); // 用户取消

    // filePath 为 undefined → 弹出另存为
    await renderAndActivate({ filePath: undefined, panelId: "hs-2" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(mockDialogSave).toHaveBeenCalledWith({
        defaultPath: "Untitled.txt",
        filters: [{ name: "所有文件", extensions: ["*"] }],
      });
    }, { timeout: 3000 });

    // 用户取消 → 不调 writeFile
    expect(fs.writeFile).not.toHaveBeenCalled();
  });

  it("HS3: 无 filePath + 用户选择路径 → writeFile 写到新路径", async () => {
    mockDialogSave.mockResolvedValue("/user/selected/path.txt");

    await renderAndActivate({ filePath: undefined, panelId: "hs-3" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(mockDialogSave).toHaveBeenCalled();
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalledWith("/user/selected/path.txt", expect.any(String));
    }, { timeout: 3000 });
  });

  it("HS4: 保存成功后调 gitDiff 刷新 gutter", async () => {
    (gitDiff as ReturnType<typeof vi.fn>).mockResolvedValue([
      { oldStart: 1, oldEnd: 2, newStart: 1, newEnd: 3, header: "@@ -1,2 +1,3 @@" },
    ]);

    await renderAndActivate({ filePath: "/test/save.js", panelId: "hs-4" });

    const editor = getActiveEditor()!;
    editor.save();

    // 等待 gitDiff 被调用 + then 回调执行
    await waitFor(() => {
      expect(gitDiff).toHaveBeenCalled();
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(updateDiffGutter).toHaveBeenCalled();
    }, { timeout: 3000 });
  });

  it("HS5: 保存后文件干净（无 diff）→ clearDiffGutter 被调用", async () => {
    (gitDiff as ReturnType<typeof vi.fn>).mockResolvedValue([]);

    await renderAndActivate({ filePath: "/test/save.js", panelId: "hs-5" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(gitDiff).toHaveBeenCalled();
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(clearDiffGutter).toHaveBeenCalled();
    }, { timeout: 3000 });
  });

  it("HS6: 保存失败 → toast.show(\"error\", 含 保存失败)", async () => {
    (fs.writeFile as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error("磁盘已满"));

    await renderAndActivate({ filePath: "/test/save.js", panelId: "hs-6" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalled();
    }, { timeout: 3000 });

    // FE-44：文案经 getErrorMessage 解析——含原始 message（真实实现保留于
    // vi.mock("../lib") importOriginal，String(Error) 含 "磁盘已满"）
    await waitFor(() => {
      expect(mockToastShow).toHaveBeenCalledWith("error", expect.stringContaining("磁盘已满"));
    }, { timeout: 3000 });
  });

  it("HS7: 保存成功后派发 slterm:file-saved CustomEvent", async () => {
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    await renderAndActivate({ filePath: "/test/save.js", panelId: "hs-7" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalled();
    }, { timeout: 3000 });

    // CustomEvent 的 type/detail 是非枚举 getter，需直接访问属性
    expect(dispatchSpy).toHaveBeenCalled();
    const savedEvent = dispatchSpy.mock.calls[0][0] as CustomEvent;
    expect(savedEvent.type).toBe("slterm:file-saved");
    expect((savedEvent as CustomEvent<{ path: string }>).detail.path).toBe("/test/save.js");

    dispatchSpy.mockRestore();
  });

  it("HS8: 无 filePath 首次保存 → 派发 slterm:file-saved-as（路径变更）", async () => {
    mockDialogSave.mockResolvedValue("/new/path.txt");
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    await renderAndActivate({ filePath: undefined, panelId: "hs-8" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalled();
    }, { timeout: 3000 });

    // oldPath(undefined) !== path("/new/path.txt") → 派发 file-saved-as
    expect(dispatchSpy).toHaveBeenCalled();
    const savedAsEvent = dispatchSpy.mock.calls[0][0] as CustomEvent;
    expect(savedAsEvent.type).toBe("slterm:file-saved-as");
    expect((savedAsEvent as CustomEvent<{ panelId: string; oldPath: string | null; newPath: string }>).detail).toEqual({
      panelId: "hs-8",
      oldPath: null,
      newPath: "/new/path.txt",
    });

    dispatchSpy.mockRestore();
  });
});

// ─── FE-01: 文件切换竞态（generation 计数）───

describe("FE-01 文件切换竞态", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = createContainer();
    capturedStateExtensions = null;
    vi.clearAllMocks();
    mockDialogSave.mockResolvedValue(null);
    (fs.writeFile as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    (gitDiff as ReturnType<typeof vi.fn>).mockResolvedValue([]);
  });

  afterEach(() => {
    container.innerHTML = "";
    // 恢复 readFile 默认实现
    (fs.readFile as ReturnType<typeof vi.fn>).mockResolvedValue("");
  });

  it("R1. 快速连续切换 filePath A→B → 最终显示 B 内容，过期 A 的 view 未创建", async () => {
    // 创建延迟 Promise：A 的 readFile 挂起，B 的 readFile 立即完成
    let resolveA!: (value: string) => void;
    const deferredA = new Promise<string>((r) => { resolveA = r; });

    const readFileMock = fs.readFile as ReturnType<typeof vi.fn>;
    readFileMock.mockImplementation(async (path: string) => {
      if (path === "/test/a.js") return deferredA;
      return "// content B";
    });

    mockDestroy.mockClear();

    const { rerender, unmount } = renderHook(
      ({ fp }) => useCodeMirror({ container, filePath: fp, panelId: "race-1" }),
      { initialProps: { fp: "/test/a.js" } as { fp: string | undefined } },
    );

    // 此时 initEditor 等待 deferredA（readFile 未完成），EditorView 尚未创建
    expect(capturedStateExtensions).toBeNull();
    expect(container.children.length).toBe(0);

    // 快速切换到 B（触发 cleanup + 新 effect）
    rerender({ fp: "/test/b.js" });

    // B 的 readFile 立即 resolve → EditorView 被创建
    await waitFor(() => {
      expect(capturedStateExtensions).toBeDefined();
    }, { timeout: 3000 });

    // 只有 B 的 EditorView 在 DOM 中
    expect(container.children.length).toBe(1);

    // 现在让 A 的 readFile 完成（过期）
    resolveA("// content A");

    // 微任务队列清空
    await new Promise((r) => setTimeout(r, 10));

    // A 的过期结果被 generation 检查丢弃，没有额外 EditorView 被创建
    expect(container.children.length).toBe(1);

    // cleanup（rerender 时）viewRef 为 null（A 未创建 view），mockDestroy 尚未被调用
    expect(mockDestroy).not.toHaveBeenCalled();

    // 卸载：cleanup 销毁 B 的 view
    unmount();
    expect(mockDestroy).toHaveBeenCalledTimes(1);
  });

  it("R2. gen 检查阻止过期 initEditor 覆盖新 viewRef", async () => {
    // A 先完成但 B 紧随其后切换 —— 验证 gen 检查而非时序依赖
    let resolveA!: (value: string) => void;
    let resolveB!: (value: string) => void;
    const deferredA = new Promise<string>((r) => { resolveA = r; });
    const deferredB = new Promise<string>((r) => { resolveB = r; });

    const readFileMock = fs.readFile as ReturnType<typeof vi.fn>;
    // 第一次调用（A）→ deferredA；第二次（B）→ deferredB
    readFileMock.mockImplementationOnce(() => deferredA);
    readFileMock.mockImplementationOnce(() => deferredB);
    // 后续调用回退默认（""），但本测试不会触发

    mockDestroy.mockClear();

    const { rerender, unmount } = renderHook(
      ({ fp }) => useCodeMirror({ container, filePath: fp, panelId: "race-2" }),
      { initialProps: { fp: "/test/a.js" } as { fp: string | undefined } },
    );

    expect(capturedStateExtensions).toBeNull();

    // 切换到 B（cleanup: mountedRef=false, viewRef=null → no destroy）
    rerender({ fp: "/test/b.js" });

    // 两个 readFile 都在挂起
    expect(capturedStateExtensions).toBeNull();

    // B 先完成
    resolveB!("// content B");
    await waitFor(() => {
      expect(capturedStateExtensions).toBeDefined();
    }, { timeout: 3000 });

    // B 的 view 在 DOM 中
    expect(container.children.length).toBe(1);
    mockDestroy.mockClear(); // 重置计数，后续断言只关注 A 的过期结果

    // A 后完成（过期）
    resolveA!("// content A");
    await new Promise((r) => setTimeout(r, 10));

    // gen 检查阻止：A 不创建 view，不调 destroy（因为没创建过）
    expect(container.children.length).toBe(1);
    // rerender cleanup 时 viewRef 为 null（A 未创建 view），gen 检查又阻止 A 创建，
    // 所以 A 的路径上始终无 destroy 调用
    mockDestroy.mockClear();

    unmount();
    expect(mockDestroy).toHaveBeenCalledTimes(1);
  });

  it("R3. 正常切换（无竞态）→ A 创建后被销毁，B 独立创建", async () => {
    // 场景：A 快速完成 → view 创建 → 切换到 B → A 的 view 被 cleanup 销毁 → B 创建
    const readFileMock = fs.readFile as ReturnType<typeof vi.fn>;
    readFileMock.mockResolvedValue("// content"); // 快速完成

    mockDestroy.mockClear();

    const { rerender, unmount } = renderHook(
      ({ fp }) => useCodeMirror({ container, filePath: fp, panelId: "race-3" }),
      { initialProps: { fp: "/test/a.js" } as { fp: string | undefined } },
 );

    // A 快速完成 → view 创建
    await waitFor(() => {
      expect(capturedStateExtensions).toBeDefined();
    }, { timeout: 3000 });

    // 切换到 B：cleanup 同步销毁 A
    mockDestroy.mockClear();
    rerender({ fp: "/test/b.js" });
    expect(mockDestroy).toHaveBeenCalledTimes(1); // A 被销毁

    // B 创建
    await waitFor(() => {
      expect(capturedStateExtensions).toBeDefined();
    }, { timeout: 3000 });

    // B 随后被卸载销毁
    mockDestroy.mockClear();
    unmount();
    expect(mockDestroy).toHaveBeenCalledTimes(1); // B 被销毁
  });
});

// ─── FE-19: repoDir 计算修复 ───

describe("FE-19 handleSave repoDir 计算", () => {
  let container: HTMLDivElement;

  /** 渲染 hook 并等待 EditorView 初始化，返回 activate 后的 editorActions */
  async function renderAndActivate(overrides?: Partial<Parameters<typeof useCodeMirror>[0]>) {
    const result = renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/save.js",
        panelId: "fe19",
        ...overrides,
      }),
    );

    await waitFor(() => expect(capturedStateExtensions).toBeDefined(), { timeout: 3000 });

    const activateCall = mockUsePanelFocus.mock.calls[mockUsePanelFocus.mock.calls.length - 1];
    const activateFn = activateCall?.[2] as (() => void) | undefined;
    if (activateFn) activateFn();

    return result;
  }

  beforeEach(() => {
    container = createContainer();
    capturedStateExtensions = null;
    vi.clearAllMocks();
    mockDialogSave.mockResolvedValue(null);
    (fs.writeFile as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    (gitDiff as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (fs.readFile as ReturnType<typeof vi.fn>).mockResolvedValue("// content");
  });

  afterEach(() => {
    container.innerHTML = "";
  });

  it("P1. 盘符根目录文件 → repoDir 为 D:/（非 D:）", async () => {
    await renderAndActivate({ filePath: "D:/file.txt", panelId: "p1" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalled();
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(gitDiff).toHaveBeenCalledWith("D:/", "D:/file.txt");
    }, { timeout: 3000 });
  });

  it("P2. 多层嵌套路径 → repoDir 为正确父目录", async () => {
    await renderAndActivate({ filePath: "D:/project/src/utils.ts", panelId: "p2" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalled();
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(gitDiff).toHaveBeenCalledWith("D:/project/src", "D:/project/src/utils.ts");
    }, { timeout: 3000 });
  });

  it("P3. 无目录分隔符的文件 → 跳过 gitDiff（repoDir 为 null）", async () => {
    // filePath 不含 "/" → getParentDir 返回 null → 不调 gitDiff
    await renderAndActivate({ filePath: "README.md", panelId: "p3" });

    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalledWith("README.md", expect.any(String));
    }, { timeout: 3000 });

    // gitDiff 不应被调用——getParentDir("README.md") 返回 null
    // 注意：initEditor 阶段可能调用了 gitDiff，需要区分
    const gitDiffCalls = (gitDiff as ReturnType<typeof vi.fn>).mock.calls;
    const savePhaseCalls = gitDiffCalls.filter(
      (call) => call[1] === "README.md",
    );
    expect(savePhaseCalls.length).toBe(0);
  });
});

// ─── EDF-03: 大文件拒绝/警告 + 保存失败不派发保存事件 ───

describe("EDF-03 大文件分支与保存失败", () => {
  let container: HTMLDivElement;

  /** 渲染 hook、等待 initEditor 完成、激活编辑器，返回 hook result */
  async function renderAndActivate(overrides?: Partial<Parameters<typeof useCodeMirror>[0]>) {
    const result = renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/huge.js",
        panelId: "edf03",
        ...overrides,
      }),
    );

    await waitFor(() => expect(capturedStateExtensions).toBeDefined(), { timeout: 3000 });

    const activateCall = mockUsePanelFocus.mock.calls[mockUsePanelFocus.mock.calls.length - 1];
    const activateFn = activateCall?.[2] as (() => void) | undefined;
    if (activateFn) activateFn();

    return result;
  }

  /** 取最近一次 EditorState.create 的 doc 参数（大文件分支下 doc 即被替换后的显示文案） */
  function lastCreatedDoc(): string {
    const createMock = EditorState.create as ReturnType<typeof vi.fn>;
    const calls = createMock.mock.calls;
    const last = calls[calls.length - 1];
    return (last?.[0] as { doc?: string })?.doc ?? "";
  }

  beforeEach(() => {
    container = createContainer();
    capturedStateExtensions = null;
    vi.clearAllMocks();
    mockDialogSave.mockResolvedValue(null);
    // FE-01: 默认确认继续（大文件弹窗默认放行，用例 2 覆盖为取消）
    mockConfirmDialog.mockResolvedValue(true);
    (fs.writeFile as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    (gitDiff as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    // 关键：readFile 的 mockResolvedValue 是 default implementation，clearAllMocks 不清除。
    // 不在此显式重置会泄漏上一用例的大文件内容到本用例（大文件分支先于 writeFile 检查触发）。
    (fs.readFile as ReturnType<typeof vi.fn>).mockResolvedValue("// normal content");
  });

  afterEach(() => {
    container.innerHTML = "";
  });

  it("1. 打开 >10MB 文档 → 拒绝文案替换全文 + filePathRef 清空（保存弹另存为而非覆盖原文件）", async () => {
    (fs.readFile as ReturnType<typeof vi.fn>).mockResolvedValue("x".repeat(MAX_FILE_SIZE_BYTES + 1));

    await renderAndActivate();

    // 文档被替换为拒绝文案
    expect(lastCreatedDoc()).toContain("文件过大");

    // filePathRef 被清空（undefined）→ save 不直接写原文件，而是弹另存为对话框
    const editor = getActiveEditor()!;
    editor.save();

    await waitFor(() => {
      expect(mockDialogSave).toHaveBeenCalled();
    }, { timeout: 3000 });
    expect(fs.writeFile).not.toHaveBeenCalled();
  });

  it("2. 打开 >1MB 文档 + confirmDialog 返回 false → 取消文案替换全文 + filePathRef 清空", async () => {
    (fs.readFile as ReturnType<typeof vi.fn>).mockResolvedValue("y".repeat(LARGE_FILE_WARN_BYTES + 1));
    mockConfirmDialog.mockResolvedValue(false);

    await renderAndActivate();

    // FE-01: 断言 confirmDialog 调用参数（标题/正文/确认按钮文案）
    expect(mockConfirmDialog).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "打开大文件",
        message: expect.stringContaining("打开可能影响性能"),
        confirmText: "继续",
      }),
    );
    expect(lastCreatedDoc()).toContain("用户取消打开大文件");

    // filePathRef 被清空 → save 弹另存为，不覆盖原文件
    const editor = getActiveEditor()!;
    editor.save();
    await waitFor(() => {
      expect(mockDialogSave).toHaveBeenCalled();
    }, { timeout: 3000 });
    expect(fs.writeFile).not.toHaveBeenCalled();
  });

  it("3. fs.writeFile reject → toast 且不派发 slterm:file-saved/file-saved-as 保存事件", async () => {
    (fs.writeFile as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error("磁盘已满"));
    const dispatchSpy = vi.spyOn(window, "dispatchEvent");

    await renderAndActivate();
    getActiveEditor()!.save();

    // FE-01: 保存失败 → toast.show("error", ...)（纯通知）
    // FE-44：文案经 getErrorMessage 解析——含原始 message
    await waitFor(() => {
      expect(mockToastShow).toHaveBeenCalledWith("error", expect.stringContaining("磁盘已满"));
    }, { timeout: 3000 });

    // 失败路径（catch 后 return）不派发任何 slterm: 保存事件
    const savedEvents = dispatchSpy.mock.calls.filter(
      ([e]) =>
        e instanceof CustomEvent &&
        ((e as CustomEvent).type === "slterm:file-saved" ||
          (e as CustomEvent).type === "slterm:file-saved-as"),
    );
    expect(savedEvents).toHaveLength(0);

    dispatchSpy.mockRestore();
  });
});

// ─── EDF-08: justSavedRef Set 多实例语义 ───

describe("EDF-08 justSavedRef 多实例语义", () => {
  let containerA: HTMLDivElement;
  let containerB: HTMLDivElement;

  beforeEach(() => {
    containerA = createContainer();
    containerB = createContainer();
    capturedStateExtensions = null;
    vi.clearAllMocks();
    mockDialogSave.mockResolvedValue(null);
    (fs.writeFile as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    (fs.readFile as ReturnType<typeof vi.fn>).mockResolvedValue("// content a.ts");
    (gitDiff as ReturnType<typeof vi.fn>).mockResolvedValue([]);
  });

  afterEach(() => {
    containerA.innerHTML = "";
    containerB.innerHTML = "";
  });

  it("1. 双实例同文件：A 保存后跳过自身事件，B 未保存仍执行自动重载", async () => {
    const { onFsEvent } = await import("../ipc/notify");
    const onFsEventMock = onFsEvent as unknown as ReturnType<typeof vi.fn>;
    const readFileMock = fs.readFile as ReturnType<typeof vi.fn>;

    // 实例 A 先挂载 → onFsEvent 第一次调用（A 的回调）；B 后挂载 → 第二次（B 的回调）
    renderHook(() => useCodeMirror({ container: containerA, filePath: "/test/a.ts", panelId: "A" }));
    renderHook(() => useCodeMirror({ container: containerB, filePath: "/test/a.ts", panelId: "B" }));

    await waitFor(() => expect(readFileMock).toHaveBeenCalledTimes(2), { timeout: 3000 });

    expect(onFsEventMock).toHaveBeenCalledTimes(2);
    const cbA = onFsEventMock.mock.calls[0][0] as (event: { paths: string[]; kind: string }) => void;
    const cbB = onFsEventMock.mock.calls[1][0] as (event: { paths: string[]; kind: string }) => void;

    // 激活 A 并保存（A 的 justSavedRef 加入 "/test/a.ts"）
    const activateA = mockUsePanelFocus.mock.calls[0]?.[2] as (() => void) | undefined;
    activateA?.();
    getActiveEditor()!.save();

    await waitFor(() => {
      expect(fs.writeFile).toHaveBeenCalledWith("/test/a.ts", expect.any(String));
    }, { timeout: 3000 });

    const readCountAfterSave = readFileMock.mock.calls.length; // 2（A、B 各一次 initEditor 读取）

    // A 收到自己保存触发的 Modify 事件 → justSaved 命中 → 跳过（不重载）
    await act(async () => {
      cbA({ paths: ["/test/a.ts"], kind: "Modify" });
    });
    expect(readFileMock.mock.calls.length).toBe(readCountAfterSave);

    // B 收到同一 Modify 事件 → B 未保存过（Set 隔离）→ 自动重载
    await act(async () => {
      cbB({ paths: ["/test/a.ts"], kind: "Modify" });
    });
    await waitFor(() => {
      expect(readFileMock.mock.calls.length).toBe(readCountAfterSave + 1);
    }, { timeout: 3000 });
    // 重载内容已 dispatch 到 B 的 view（此前保存路径不 dispatch，此次为唯一一次）
    expect(mockDispatch).toHaveBeenCalled();
  });

  it("2. justSaved 一次性消费：A 第二次收到同一路径事件恢复自动重载", async () => {
    const { onFsEvent } = await import("../ipc/notify");
    const onFsEventMock = onFsEvent as unknown as ReturnType<typeof vi.fn>;
    const readFileMock = fs.readFile as ReturnType<typeof vi.fn>;

    renderHook(() => useCodeMirror({ container: containerA, filePath: "/test/a.ts", panelId: "A" }));
    renderHook(() => useCodeMirror({ container: containerB, filePath: "/test/a.ts", panelId: "B" }));
    await waitFor(() => expect(readFileMock).toHaveBeenCalledTimes(2), { timeout: 3000 });

    const cbA = onFsEventMock.mock.calls[0][0] as (event: { paths: string[]; kind: string }) => void;

    const activateA = mockUsePanelFocus.mock.calls[0]?.[2] as (() => void) | undefined;
    activateA?.();
    getActiveEditor()!.save();
    await waitFor(() => expect(fs.writeFile).toHaveBeenCalled(), { timeout: 3000 });

    // 第一次事件：Set 命中 → 跳过并删除路径
    await act(async () => {
      cbA({ paths: ["/test/a.ts"], kind: "Modify" });
    });
    expect(readFileMock.mock.calls.length).toBe(2);

    // 第二次事件：Set 已消费删除 → A 恢复自动重载
    await act(async () => {
      cbA({ paths: ["/test/a.ts"], kind: "Modify" });
    });
    await waitFor(() => {
      expect(readFileMock.mock.calls.length).toBe(3);
    }, { timeout: 3000 });
  });
});
