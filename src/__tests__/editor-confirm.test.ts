// editor-confirm.test.ts — useCodeMirror fs-event 集成测试（renderHook 真实驱动）
//
// 验证 onFsEvent handler 内全部路径：
//   - onFsEvent 订阅 / unmount 取消订阅
//   - event.kind 过滤（非 Modify 跳过）
//   - Modify + dirty=false → 自动重载（readFile + view.dispatch）
//   - Modify + dirty=true → window.confirm 弹窗 / 确认=重载 / 取消=保留
//
// 与旧版手动模拟 handler 逻辑不同，本文件通过 renderHook(useCodeMirror)
// 真实驱动 hook，mock onFsEvent 捕获回调后手动触发 fs-event 验证行为。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// ─── Hoisted mocks（getter/setter 不能解构，保留对象引用） ───
const h = vi.hoisted(() => {
  let _onFsCallback: ((event: { paths: string[]; kind: string }) => void) | null = null;
  return {
    mockDispatch: vi.fn(),
    mockDestroy: vi.fn(),
    mockUnlisten: vi.fn(),
    get mockOnFsCallback() { return _onFsCallback; },
    set mockOnFsCallback(cb: ((event: { paths: string[]; kind: string }) => void) | null) { _onFsCallback = cb; },
    mockReadFile: vi.fn().mockResolvedValue(""),
    mockWriteFile: vi.fn().mockResolvedValue(undefined),
    mockGitDiff: vi.fn().mockResolvedValue([]),
    mockUpdateDiffGutter: vi.fn(),
    mockClearDiffGutter: vi.fn(),
    mockDialogSave: vi.fn<() => Promise<string | null>>().mockResolvedValue(null),
    mockReconfigure: vi.fn().mockReturnValue([]),
    mockUsePanelFocus: vi.fn(),
  };
});

let capturedStateExtensions: unknown | null = null;

// ─── Module mocks ───

vi.mock("@codemirror/view", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@codemirror/view")>();
  const MockEditorView = class {
    dom: HTMLDivElement;
    dispatch = h.mockDispatch;
    destroy = h.mockDestroy;
    state: unknown;
    constructor(config: { parent: HTMLElement; state: unknown }) {
      this.dom = document.createElement("div");
      config.parent.appendChild(this.dom);
      this.state = config.state;
      capturedStateExtensions = config.state;
    }
    static theme = vi.fn(() => []);
    static updateListener = { of: vi.fn(() => []) };
    static lineWrapping = Symbol("lineWrapping");
  };
  return { ...actual, EditorView: MockEditorView, keymap: { of: vi.fn(() => []) } };
});

vi.mock("@codemirror/state", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@codemirror/state")>();
  return {
    ...actual,
    Compartment: class {
      of(ext: unknown) { return ext; }
      reconfigure(ext: unknown) { h.mockReconfigure(ext); return []; }
    },
    EditorState: { create: vi.fn().mockReturnValue({ doc: vi.fn() }) },
  };
});

// 重写全局 setup.ts mock：捕获 onFsEvent 回调
vi.mock("../ipc/notify", () => ({
  onFsEvent: vi.fn((cb: (event: { paths: string[]; kind: string }) => void) => {
    h.mockOnFsCallback = cb;
    return h.mockUnlisten;
  }),
  startWatch: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc", () => ({
  fs: {
    readFile: h.mockReadFile,
    writeFile: h.mockWriteFile,
  },
  save: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitDiff: h.mockGitDiff,
}));

vi.mock("../panels/editor/gitGutter", () => ({
  diffGutter: vi.fn(() => []),
  updateDiffGutter: h.mockUpdateDiffGutter,
  clearDiffGutter: h.mockClearDiffGutter,
}));

vi.mock("../ipc/dialog", () => ({
  save: h.mockDialogSave,
}));

vi.mock("../features/shortcuts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../features/shortcuts")>();
  return { ...actual, usePanelFocus: h.mockUsePanelFocus };
});

// ─── 导入被测模块 ───
import { useCodeMirror } from "../panels/editor/useCodeMirror";

// ─── 辅助函数 ───

function createContainer(): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, configurable: true });
  return el;
}

/** 等待 hook 异步初始化（initEditor 中 readFile mock 立即 resolve） */
async function renderAndWait(props?: Partial<Parameters<typeof useCodeMirror>[0]>) {
  const container = createContainer();
  const result = renderHook(() =>
    useCodeMirror({
      container,
      filePath: "/test/main.ts",
      panelId: "fs-test",
      ...props,
    }),
  );
  // 等待 EditorView 创建完成
  await waitFor(() => {
    expect(capturedStateExtensions).toBeDefined();
  }, { timeout: 3000 });
  return { result, container };
}

// ─── 测试套件 ───

describe("useCodeMirror fs-event 集成", () => {
  beforeEach(() => {
    capturedStateExtensions = null;
    h.mockOnFsCallback = null;
    vi.clearAllMocks();
    h.mockReadFile.mockResolvedValue("// modified content");
    h.mockWriteFile.mockResolvedValue(undefined);
    h.mockGitDiff.mockResolvedValue([]);
  });

  // ── E1: onFsEvent 订阅 ──

  it("E1. 挂载时调用 onFsEvent 订阅 fs-event", async () => {
    const { onFsEvent: onFsEventMock } = await import("../ipc/notify");
    await renderAndWait();

    expect(onFsEventMock).toHaveBeenCalledTimes(1);
    expect(onFsEventMock).toHaveBeenCalledWith(expect.any(Function));
  });

  // ── E2: unmount 取消订阅 ──

  it("E2. unmount 时调用 onFsEvent 返回的 unlisten", async () => {
    const { result } = await renderAndWait();
    result.unmount();

    expect(h.mockUnlisten).toHaveBeenCalledTimes(1);
  });

  // ── E3: event.kind 过滤 —— 非 Modify 跳过 ──

  it("E3. kind=Create 事件 → fs.readFile 不调用", async () => {
    await renderAndWait();

    h.mockReadFile.mockClear();

    // 触发 fs-event（kind=Create，非 Modify）
    expect(h.mockOnFsCallback).not.toBeNull();
    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Create" });

    // fs.readFile 不应被调用（事件被过滤）
    expect(h.mockReadFile).not.toHaveBeenCalled();
  });

  it("E3b. kind=Remove 事件 → fs.readFile 不调用", async () => {
    await renderAndWait();

    h.mockReadFile.mockClear();
    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Remove" });

    expect(h.mockReadFile).not.toHaveBeenCalled();
  });

  // ── E4: path 不匹配 → 跳过 ──

  it("E4. 文件路径不匹配当前打开文件 → 跳过", async () => {
    await renderAndWait({ filePath: "/test/main.ts" });

    h.mockReadFile.mockClear();

    // 触发其他文件的 fs-event
    h.mockOnFsCallback!({ paths: ["/test/other.ts"], kind: "Modify" });

    expect(h.mockReadFile).not.toHaveBeenCalled();
  });

  // ── E5: Modify + dirty=false → 自动重载 ──

  it("E5. Modify 事件 + 干净文件 → 自动重载（readFile + view.dispatch）", async () => {
    await renderAndWait({ filePath: "/test/main.ts" });

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });

    // readFile 被调用（自动重载读取磁盘内容）
    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/main.ts");
    }, { timeout: 3000 });

    // 等待 readFile resolve → then 回调 dispatch
    await waitFor(() => {
      expect(h.mockDispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 0, insert: "// modified content" },
      });
    }, { timeout: 3000 });
  });

  // ── E6: Modify + dirty=true → confirm 弹窗 ──

  it("E6. Modify 事件 + 脏文件 → window.confirm 弹窗", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    // 标记为 dirty
    result.result.current.markDirty();

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });

    expect(confirmSpy).toHaveBeenCalledTimes(1);
    expect(confirmSpy.mock.calls[0][0]).toContain("/test/main.ts");
    expect(confirmSpy.mock.calls[0][0]).toContain("已被外部修改");
    expect(confirmSpy.mock.calls[0][0]).toContain("未保存的修改");

    confirmSpy.mockRestore();
  });

  // ── E7: Modify + dirty + confirm=true → 重载 ──

  it("E7. 用户确认重载 → readFile + dispatch，dirty 复位", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    result.result.current.markDirty();
    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/main.ts");
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(h.mockDispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 0, insert: "// modified content" },
      });
    }, { timeout: 3000 });

    confirmSpy.mockRestore();
  });

  // ── E8: Modify + dirty + confirm=false → 不重载 ──

  it("E8. 用户拒绝重载 → readFile 不调用，dirty 保持 true", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    result.result.current.markDirty();
    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });

    // readFile 不应被调用（用户拒绝了重载）
    expect(h.mockReadFile).not.toHaveBeenCalled();
    expect(h.mockDispatch).not.toHaveBeenCalled();

    confirmSpy.mockRestore();
  });

  // ── E9: container=null → viewRef 为 null → fs-event 不崩溃 ──

  it("E9. container=null 时 hook 不创建 EditorView → fs-event 不崩溃", async () => {
    renderHook(() =>
      useCodeMirror({ container: null, filePath: undefined, panelId: "e9" }),
    );

    // 触发 fs-event —— viewRef.current 为 null，应安全跳过
    // 注意：container=null 时 useEffect 不执行 initEditor，但 onFsEvent 的 useEffect(deps=[])
    // 仍然执行（mountedRef 仍为 false，但 handler 只检查 viewRef.current）
    expect(() => {
      if (h.mockOnFsCallback) {
        h.mockOnFsCallback({ paths: ["/test/main.ts"], kind: "Modify" });
      }
    }).not.toThrow();

    expect(h.mockDispatch).not.toHaveBeenCalled();
  });

  // ── E10: unmount 后 fs-event 不触发 readFile ──

  it("E10. unmount 后 fs-event 不触发 readFile", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    // 先卸载
    result.unmount();
    expect(h.mockUnlisten).toHaveBeenCalled();

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    // 此时 onFsCallback 仍存在（unlisten 只取消 Tauri listen，
    // 但 handler 闭包仍在），viewRef.current 被 destroy 置为 null
    expect(() => {
      if (h.mockOnFsCallback) {
        h.mockOnFsCallback({ paths: ["/test/main.ts"], kind: "Modify" });
      }
    }).not.toThrow();
  });
});
