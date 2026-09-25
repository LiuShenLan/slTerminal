// editor-fs-poll.test.ts — useCodeMirror fs-poll 心跳复核（poll 模式）测试
//
// fs-poll = 后端 watcher 10s 周期广播的事件丢失补漏通道（与 Rescan 复核同通道，
// 均走 applyExternalChange poll 模式）。核心语义 = 滚动磁盘基线（diskBaselineRef）
// 判变：磁盘未变零打扰（打字常态不弹窗）；磁盘真变 → 干净直接应用 / 脏弹确认
// （拒绝标记 pollRejectedRef 防 10s 弹窗循环）。
//
// 覆盖：
//   P1 磁盘未变 + dirty → 零打扰【核心语义锚】
//   P2 磁盘变 + clean → 静默重载
//   P3 磁盘变 + dirty → confirm / 取消后同内容不再弹 / 新内容再弹 / 切文件清标记
//   P4 基线滞后同步（doc==disk 但基线旧）→ 同步基线短路，后续脏态不误弹
//   P5 save 后 poll 同内容 → 静默（保存回声不打扰）
//
// CP-029: 打开后核对（OPEN_RECHECK_DELAY_MS 真实计时）会在打开 1.5s 后追加一次
// 读盘——计数断言处先 settleRecheck() 跨过（同 editor-confirm.test.ts 先例）

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

// ─── Hoisted mocks（getter/setter 不能解构，保留对象引用） ───
const h = vi.hoisted(() => {
  let _onFsPollCallback: (() => void) | null = null;
  return {
    mockDispatch: vi.fn(),
    mockDestroy: vi.fn(),
    get mockOnFsPollCallback() { return _onFsPollCallback; },
    set mockOnFsPollCallback(cb: (() => void) | null) { _onFsPollCallback = cb; },
    mockReadFile: vi.fn().mockResolvedValue(""),
    mockWriteFile: vi.fn().mockResolvedValue(undefined),
    // FE-08: stat 预检前置——默认小文件（0 字节）不触发警告/超限分支
    mockStatFile: vi.fn().mockResolvedValue({ sizeBytes: 0, mtimeMs: null }),
    mockGitDiff: vi.fn().mockResolvedValue([]),
    mockUpdateDiffGutter: vi.fn(),
    mockClearDiffGutter: vi.fn(),
    mockReconfigure: vi.fn().mockReturnValue([]),
    mockUsePanelFocus: vi.fn(),
    // 默认取消（脏文件弹窗默认不重载）
    mockConfirmDialog: vi.fn<(opts: { title?: string; message: string; confirmText?: string }) => Promise<boolean>>().mockResolvedValue(false),
    mockToastShow: vi.fn(),
  };
});

let capturedStateExtensions: unknown | null = null;

// ─── Module mocks（照 editor-confirm.test.ts 模式） ───

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
  onFsEvent: vi.fn(() => () => {}),
  // 捕获 fs-poll 心跳回调，用例手动触发
  onFsPoll: vi.fn((cb: () => void) => {
    h.mockOnFsPollCallback = cb;
    return () => {};
  }),
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
  save: vi.fn(),
}));

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
import { getActiveEditor } from "../panels/editor/activeEditor";

// ─── 辅助函数 ───

function createContainer(): HTMLDivElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, configurable: true });
  return el;
}

async function renderAndWait(props?: Partial<Parameters<typeof useCodeMirror>[0]>) {
  const container = createContainer();
  const result = renderHook((p) =>
    useCodeMirror({
      container,
      filePath: "/test/main.ts",
      panelId: "poll-test",
      ...p,
    }),
  { initialProps: props });
  await waitFor(() => {
    expect(capturedStateExtensions).toBeDefined();
  }, { timeout: 3000 });
  return { result, container };
}

/** CP-029: 跨过打开后核对读盘，计数进入稳定态 */
async function settleRecheck(): Promise<void> {
  await act(async () => {
    await new Promise((r) => setTimeout(r, OPEN_RECHECK_DELAY_MS + 150));
  });
}

/** 触发一次 fs-poll 心跳 */
async function firePoll(): Promise<void> {
  await act(async () => {
    h.mockOnFsPollCallback!();
    // poll handler 为 fire-and-forget（void applyExternalChange）——让出微任务
    // 队列使 stat/readFile 链推进
    await Promise.resolve();
  });
}

// ─── 测试套件 ───

describe("useCodeMirror fs-poll 心跳复核（poll 模式）", () => {
  beforeEach(() => {
    capturedStateExtensions = null;
    h.mockOnFsPollCallback = null;
    vi.clearAllMocks();
    h.mockReadFile.mockResolvedValue("// baseline");
    h.mockWriteFile.mockResolvedValue(undefined);
    h.mockGitDiff.mockResolvedValue([]);
    h.mockConfirmDialog.mockResolvedValue(false);
  });

  it("P1. 磁盘未变 + dirty → 零打扰（打字常态不弹窗）", async () => {
    const { result } = await renderAndWait();
    await settleRecheck();

    // 用户正在打字（脏）但磁盘未变（= 滚动基线 "// baseline"）
    act(() => {
      result.result.current.markDirty();
    });

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();

    await firePoll();

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
    // 判等短路为同步路径——等一拍确认无后续动作
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(h.mockConfirmDialog).not.toHaveBeenCalled();
    expect(h.mockDispatch).not.toHaveBeenCalled();
  });

  it("P2. 磁盘变 + clean → 静默重载（dispatch，无弹窗）", async () => {
    await renderAndWait();
    await settleRecheck();

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();
    h.mockReadFile.mockResolvedValue("// changed on disk");

    await firePoll();

    await waitFor(() => {
      expect(h.mockDispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 0, insert: "// changed on disk" },
      });
    }, { timeout: 3000 });
    expect(h.mockConfirmDialog).not.toHaveBeenCalled();
    expect(h.mockToastShow).not.toHaveBeenCalled();
  });

  it("P3. 磁盘变 + dirty → confirm；取消后同内容不再弹、新内容再弹、切文件清标记", async () => {const { result } = await renderAndWait();
    await settleRecheck();

    // 磁盘真变 + 脏 → 弹确认（取消 → 记拒绝标记）
    h.mockReadFile.mockResolvedValue("// disk v2");
    act(() => {
      result.result.current.markDirty();
    });
    await firePoll();
    await waitFor(() => {
      expect(h.mockConfirmDialog).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
    expect(h.mockDispatch).not.toHaveBeenCalled();

    // 同一磁盘内容再次心跳 → 拒绝标记命中，不再弹（防 10s 弹窗循环）
    await firePoll();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(h.mockConfirmDialog).toHaveBeenCalledTimes(1);

    // 磁盘又变（新内容）→ 拒绝标记不命中，再弹
    h.mockReadFile.mockResolvedValue("// disk v3");
    await firePoll();
    await waitFor(() => {
      expect(h.mockConfirmDialog).toHaveBeenCalledTimes(2);
    }, { timeout: 3000 });

    // 切文件 → 基线/拒绝标记同清（防跨文件泄漏）：切走再切回，磁盘回到 "// disk v2"
    // （= 步骤一拒绝过的内容）——若标记未清会被拒绝标记吞掉不弹
    h.mockReadFile.mockResolvedValue("// other file");
    act(() => {
      result.rerender({ filePath: "/test/other.ts" });
    });
    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/other.ts");
    }, { timeout: 3000 });
    await settleRecheck();

    h.mockReadFile.mockResolvedValue("// baseline v2");
    act(() => {
      result.rerender({ filePath: "/test/main.ts" });
    });
    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/main.ts");
    }, { timeout: 3000 });
    await settleRecheck();

    h.mockReadFile.mockResolvedValue("// disk v2");
    act(() => {
      result.result.current.markDirty();
    });
    await firePoll();
    await waitFor(() => {
      expect(h.mockConfirmDialog).toHaveBeenCalledTimes(3);
    }, { timeout: 3000 });
    // 负载裕度，非断言放宽：本用例含三次 settleRecheck 真实计时（≈5s），须显式超时
  }, 20000);

  it("P4. 基线滞后（doc==disk 但基线旧）→ 同步基线短路，后续脏态不误弹", async () => {
    const { result } = await renderAndWait();
    await settleRecheck();

    // 场景：缓冲内容已与磁盘一致（如打字回退到磁盘内容）但滚动基线滞后
    (capturedStateExtensions as { doc: unknown }).doc = { toString: () => "// aligned" };
    h.mockReadFile.mockResolvedValue("// aligned");

    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();
    await firePoll();
    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    // 干净 + 内容一致 → 同步基线短路，无 dispatch 无弹窗
    expect(h.mockDispatch).not.toHaveBeenCalled();
    expect(h.mockConfirmDialog).not.toHaveBeenCalled();

    // 基线已同步为 "// aligned"：此后脏态下同内容心跳 → 基线判等短路，不误弹
    // 【对未同步基线的实现红】：脏 + 内容≠旧基线 → 走 confirm 分支
    act(() => {
      result.result.current.markDirty();
    });
    await firePoll();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(h.mockConfirmDialog).not.toHaveBeenCalled();
  });

  it("P5. save 后 poll 同内容 → 静默（保存回声不打扰）", async () => {
    await renderAndWait();
    await settleRecheck();

    // 缓冲内容固定为可判等文本（mock doc 默认 vi.fn() 无法判等）
    (capturedStateExtensions as { doc: unknown }).doc = { toString: () => "// saved" };

    // 保存成功 → 滚动基线 = "// saved"
    const activate = h.mockUsePanelFocus.mock.calls[0]?.[2] as (() => void) | undefined;
    activate?.();
    await act(async () => {
      getActiveEditor()!.save();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(h.mockWriteFile).toHaveBeenCalledWith("/test/main.ts", "// saved");
    }, { timeout: 3000 });

    // 心跳读盘同内容（写盘已落盘）→ 基线判等短路，零打扰
    h.mockReadFile.mockResolvedValue("// saved");
    h.mockReadFile.mockClear();
    h.mockDispatch.mockClear();
    await firePoll();
    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(h.mockConfirmDialog).not.toHaveBeenCalled();
    expect(h.mockDispatch).not.toHaveBeenCalled();
  });
});
