// editor-confirm.test.ts — useCodeMirror fs-event 集成测试（renderHook 真实驱动）
//
// 验证 onFsEvent handler 内全部路径：
//   - onFsEvent 订阅 / unmount 取消订阅
//   - event.kind 过滤（Modify/Create 放行——Create=原子替换写折叠产物；Remove/Access/Other 跳过）
//   - Modify/Create + dirty=false → 自动重载（readFile + view.dispatch）
//   - Modify/Create + dirty=true → confirmDialog 弹窗 / 确认=重载 / 取消=保留
//   - Rescan（paths=监听根）→ 忽略路径匹配复核已打开文件（poll 模式）
//
// 与旧版手动模拟 handler 逻辑不同，本文件通过 renderHook(useCodeMirror)
// 真实驱动 hook，mock onFsEvent 捕获回调后手动触发 fs-event 验证行为。
//
// CP-029: 打开后核对（OPEN_RECHECK_DELAY_MS=1500 真实计时，单次）会在文件打开
// 1.5s 后追加一次读盘——readFile 精确计数断言处须先 settleRecheck() 跨过
// （核对补偿专项用例见 use-code-mirror-reload-error.test.ts K 系列）

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
    // FE-08: stat 预检前置——默认小文件（0 字节）不触发警告/超限分支
    mockStatFile: vi.fn().mockResolvedValue({ sizeBytes: 0, mtimeMs: null }),
    mockGitDiff: vi.fn().mockResolvedValue([]),
    mockUpdateDiffGutter: vi.fn(),
    mockClearDiffGutter: vi.fn(),
    mockDialogSave: vi.fn<() => Promise<string | null>>().mockResolvedValue(null),
    mockReconfigure: vi.fn().mockReturnValue([]),
    mockUsePanelFocus: vi.fn(),
    // FE-01: 应用内浮层 mock（替代 window.confirm spy；参数签名对齐 ConfirmDialogOptions 子集）
    mockConfirmDialog: vi.fn<(opts: { title?: string; message: string; confirmText?: string }) => Promise<boolean>>().mockResolvedValue(false),
    mockToastShow: vi.fn(),
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
  onFsPoll: vi.fn(() => () => {}),
  startWatch: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc", () => ({
  fs: {
    readFile: h.mockReadFile,
    writeFile: h.mockWriteFile,
    statFile: h.mockStatFile,
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

// FE-01: mock 应用内浮层 confirmDialog/toast（importOriginal 保留其余导出，防破坏
// importOriginal 展开的 ../features/shortcuts 对 lib 的依赖）
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
import { useCodeMirror, OPEN_RECHECK_DELAY_MS } from "../panels/editor/useCodeMirror";

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

/**
 * CP-029: 跨过打开后核对（OPEN_RECHECK_DELAY_MS 后单次读盘）——readFile 精确
 * 计数断言前调用，使计数进入稳定态（核对补偿专项用例见
 * use-code-mirror-reload-error.test.ts K 系列）
 */
async function settleRecheck(): Promise<void> {
  await act(async () => {
    await new Promise((r) => setTimeout(r, OPEN_RECHECK_DELAY_MS + 150));
  });
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
    // FE-01: 默认取消（脏文件弹窗默认不重载，E7 覆盖为确认）
    h.mockConfirmDialog.mockResolvedValue(false);
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

  // ── E3: event.kind 过滤 —— Modify/Create 放行，其余跳过 ──

  it("E3. kind=Create（原子替换写折叠产物）→ 重读盘并替换内容", async () => {
    await renderAndWait();

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    // 防复发（2026-09 根因 1）：claude code 等工具「写临时文件+rename 原子替换」
    // 经 notify-debouncer-full 折叠为单条 Create(target)——旧闸门只放行 Modify
    // 致编辑器永不更新；断言对旧代码红（handler 为 async，须 waitFor 防 vacuous 通过）
    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Create" });

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/main.ts");
    }, { timeout: 3000 });
    await waitFor(() => {
      expect(h.mockDispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 0, insert: "// modified content" },
      });
    }, { timeout: 3000 });
  });

  it("E3b. kind=Remove 事件 → fs.readFile 不调用", async () => {
    await renderAndWait();

    h.mockReadFile.mockClear();
    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Remove" });

    expect(h.mockReadFile).not.toHaveBeenCalled();
  });

  it("E3c. kind=Access/Other 事件 → fs.readFile 不调用", async () => {
    await renderAndWait();

    h.mockReadFile.mockClear();
    // 放行集仅 Modify/Create——Access/Other 同步闸门丢弃（无需 waitFor）
    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Access" });
    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Other" });

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

  // ── E6: Modify + dirty=true → confirmDialog 弹窗 ──

  it("E6. Modify 事件 + 脏文件 → confirmDialog 弹窗（断言调用参数）", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    // 标记为 dirty
    result.result.current.markDirty();

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });

    // FE-01: confirmDialog 被调用且参数含路径/未保存提示/确认按钮文案
    expect(h.mockConfirmDialog).toHaveBeenCalledTimes(1);
    expect(h.mockConfirmDialog).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "外部修改",
        message: expect.stringContaining("/test/main.ts"),
        confirmText: "重载",
      }),
    );
    const msg = h.mockConfirmDialog.mock.calls[0][0]?.message ?? "";
    expect(msg).toContain("已被外部修改");
    expect(msg).toContain("未保存的修改");
  });

  // ── E6b: Create + dirty=true → confirmDialog 弹窗（event 语义对 Create 保持） ──

  it("E6b. Create 事件 + 脏文件 → confirmDialog 弹窗（断言调用参数）", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    result.result.current.markDirty();
    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Create" });

    // 脏确认在读盘前同步发起（event 模式语义对 Create 不变）
    expect(h.mockConfirmDialog).toHaveBeenCalledTimes(1);
    expect(h.mockConfirmDialog).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "外部修改",
        message: expect.stringContaining("/test/main.ts"),
        confirmText: "重载",
      }),
    );
    expect(h.mockReadFile).not.toHaveBeenCalled();
  });

  // ── E7: Modify + dirty + confirmDialog=true → 重载 ──

  it("E7. 用户确认重载 → readFile + dispatch，dirty 复位", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    // CP-029: 先跨过打开后核对读盘（否则 1.5s 定时可能在断言窗口内追加读盘）
    await settleRecheck();
    result.result.current.markDirty();
    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    h.mockConfirmDialog.mockResolvedValue(true);

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/main.ts");
    }, { timeout: 3000 });

    await waitFor(() => {
      expect(h.mockDispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 0, insert: "// modified content" },
      });
    }, { timeout: 3000 });

    // 重载路径磁盘读取恰好一次（确认后触发重新读取；此前 mockClear 已清空初始化读取）
    expect(h.mockReadFile).toHaveBeenCalledTimes(1);
  });

  // ── E8: Modify + dirty + confirmDialog=false → 不重载 ──

  it("E8. 用户拒绝重载 → readFile 不调用，dirty 保持 true", async () => {
    const { result } = await renderAndWait({ filePath: "/test/main.ts" });

    result.result.current.markDirty();
    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    h.mockConfirmDialog.mockResolvedValue(false);

    h.mockOnFsCallback!({ paths: ["/test/main.ts"], kind: "Modify" });

    // 等待回调内 await confirmDialog 完成（resolve false → 不重载）
    await waitFor(() => {
      expect(h.mockConfirmDialog).toHaveBeenCalled();
    }, { timeout: 3000 });

    // readFile 不应被调用（用户拒绝了重载）
    expect(h.mockReadFile).not.toHaveBeenCalled();
    expect(h.mockDispatch).not.toHaveBeenCalled();
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

  // ── E11: Rescan（队列溢出补漏，paths=监听根）→ 忽略路径匹配复核已打开文件 ──

  it("E11a. Rescan + 磁盘已变 → 忽略路径匹配重读盘并替换内容", async () => {
    await renderAndWait({ filePath: "/test/main.ts" });
    await settleRecheck();

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();
    h.mockReadFile.mockResolvedValue("// changed via rescan");

    // paths 为监听根（不含本文件）——旧语义按路径匹配必然落空整条丢弃；
    // 新语义忽略路径匹配对已打开文件走 poll 复核（handler async，须 waitFor）
    h.mockOnFsCallback!({ paths: ["/test"], kind: "Rescan" });

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/main.ts");
    }, { timeout: 3000 });
    await waitFor(() => {
      expect(h.mockDispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 0, insert: "// changed via rescan" },
      });
    }, { timeout: 3000 });
  });

  it("E11b. Rescan + 磁盘未变（=滚动基线）→ 读盘一次但零打扰", async () => {
    await renderAndWait({ filePath: "/test/main.ts" });
    await settleRecheck();

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();
    // 磁盘内容 = initEditor 建立的滚动基线（"// modified content"）——
    // poll 复核判等短路：读盘一次确认未变，无 dispatch/无弹窗
    h.mockOnFsCallback!({ paths: ["/test"], kind: "Rescan" });

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
    // readFile resolve 后判等短路为同步路径——等一拍确认无后续动作
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(h.mockDispatch).not.toHaveBeenCalled();
    expect(h.mockConfirmDialog).not.toHaveBeenCalled();
  });
});
