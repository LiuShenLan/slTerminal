// use-code-mirror-reload-error.test.ts — 外部修改重载失败提示测试（FE-10）
//
// 覆盖：fs-event Modify → 重载 readFile 失败 → console.warn 保留 + toast 提示
// （干净自动重载分支 + 脏确认重载分支）；git diff 刷新失败消息统一经 getErrorMessage

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

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
    mockConfirmDialog: vi.fn<(opts: { title?: string; message: string; confirmText?: string }) => Promise<boolean>>().mockResolvedValue(false),
    mockToastShow: vi.fn(),
  };
});

let capturedStateExtensions: unknown | null = null;

// ─── 模块级 mocks（照 editor-confirm.test.ts 模式） ───

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

// mock 浮层 confirmDialog/toast（importOriginal 保留 getErrorMessage 等真实导出）
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return {
    ...actual,
    confirmDialog: h.mockConfirmDialog,
    toast: { ...actual.toast, show: h.mockToastShow },
  };
});

vi.mock("../features/shortcuts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../features/shortcuts")>();
  return { ...actual, usePanelFocus: h.mockUsePanelFocus };
});

// ─── 导入被测模块 ───
import { useCodeMirror } from "../panels/editor/useCodeMirror";

function createContainer(): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, configurable: true });
  return el;
}

/** 等待 hook 异步初始化完成 */
async function renderAndWait() {
  const container = createContainer();
  const result = renderHook(() =>
    useCodeMirror({
      container,
      filePath: "/test/main.ts",
      panelId: "reload-error",
    }),
  );
  await waitFor(() => {
    expect(capturedStateExtensions).toBeDefined();
  }, { timeout: 3000 });
  return { result };
}

/** 触发一次文件 Modify fs-event（命中 /test/main.ts） */
async function triggerModify() {
  await act(async () => {
    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  capturedStateExtensions = null;
  h.mockReadFile.mockResolvedValue(""); // initEditor 默认成功
});

describe("外部修改重载失败（FE-10）", () => {
  it("R1: 干净分支重载失败 → console.warn 保留 + toast.show('error')", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = await renderAndWait();

    // 首次 readFile 已由 initEditor 消费；fs-event 重载失败
    h.mockReadFile.mockRejectedValueOnce(new Error("磁盘错误"));
    await triggerModify();

    await waitFor(() => {
      expect(h.mockToastShow).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("外部修改重载失败"),
      );
    }, { timeout: 3000 });
    // 消息经 getErrorMessage（Error.message）
    expect(h.mockToastShow).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("磁盘错误"),
    );
    // console.warn 保留
    // S08 契约：错误消息统一经 getErrorMessage（Error → String(err) 含 "Error: " 前缀），
    // 断言用包含匹配而非精确相等
    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining("外部修改重载失败"),
      expect.stringContaining("磁盘错误"),
    );
    // 未走确认弹窗（干净分支）
    expect(h.mockConfirmDialog).not.toHaveBeenCalled();
    void result;

    warnSpy.mockRestore();
  });

  it("R2: 脏分支确认重载失败 → console.warn + toast", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = await renderAndWait();

    // 标记脏 → 外部修改 → 确认重载
    act(() => {
      result.result.current.markDirty();
    });
    h.mockConfirmDialog.mockResolvedValue(true);
    h.mockReadFile.mockRejectedValueOnce(new Error("磁盘错误"));

    await triggerModify();

    await waitFor(() => {
      expect(h.mockToastShow).toHaveBeenCalledWith(
        "error",
        expect.stringContaining("外部修改重载失败"),
      );
    }, { timeout: 3000 });
    expect(h.mockConfirmDialog).toHaveBeenCalled();
    expect(warnSpy).toHaveBeenCalled();

    warnSpy.mockRestore();
  });

  it("R3: 重载失败不影响后续外部修改重载（可恢复）", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { result } = await renderAndWait();

    // 第一次重载失败
    h.mockReadFile.mockRejectedValueOnce(new Error("瞬态错误"));
    await triggerModify();
    await waitFor(() => {
      expect(h.mockToastShow).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    // 第二次重载成功（readFile 默认 resolve 空串）→ dispatch 重载
    await triggerModify();
    await waitFor(() => {
      expect(h.mockDispatch).toHaveBeenCalled();
    }, { timeout: 3000 });
    // 无第二次 toast
    expect(h.mockToastShow).toHaveBeenCalledTimes(1);
    void result;

    warnSpy.mockRestore();
  });
});
