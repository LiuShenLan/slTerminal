// editor-viewless-reload.test.ts — useCodeMirror view 缺失外部变更回写测试（根因 2）
//
// 场景 = htmlviewer render 态：container=null 不建 CM view，但 onFsEvent/onFsPoll
// 订阅仍存活。外部修改到达时读盘内容须经 onDocContent("reload") 回写面板 docRef
// （预览随之重渲染），而非 `if (!view) return` 丢弃。
//
// 覆盖：
//   V1 viewless + Modify 命中 → cb("reload")【根因 2 防复发——对旧代码红】
//   V2 同内容再次事件 → lastReported 判等短路（cb 不重复调）
//   V3 无 cb（EditorPanel 形态）→ 不 throw，零影响
//   V4 viewless + Create（原子替换写）→ cb("reload")
//   V5 poll + dirty + 取消 → 不调 cb；同内容再次心跳拒绝标记命中不再弹

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

// ─── Hoisted mocks（getter/setter 不能解构，保留对象引用） ───
const h = vi.hoisted(() => {
  let _onFsCallback: ((event: { paths: string[]; kind: string }) => void) | null = null;
  let _onFsPollCallback: (() => void) | null = null;
  return {
    get mockOnFsCallback() { return _onFsCallback; },
    set mockOnFsCallback(cb: ((event: { paths: string[]; kind: string }) => void) | null) { _onFsCallback = cb; },
    get mockOnFsPollCallback() { return _onFsPollCallback; },
    set mockOnFsPollCallback(cb: (() => void) | null) { _onFsPollCallback = cb; },
    mockReadFile: vi.fn().mockResolvedValue(""),
    mockWriteFile: vi.fn().mockResolvedValue(undefined),
    // FE-08: stat 预检前置——默认小文件（0 字节）不触发警告/超限分支
    mockStatFile: vi.fn().mockResolvedValue({ sizeBytes: 0, mtimeMs: null }),
    mockGitDiff: vi.fn().mockResolvedValue([]),
    mockUsePanelFocus: vi.fn(),
    // 默认取消（脏文件弹窗默认不重载）
    mockConfirmDialog: vi.fn<(opts: { title?: string; message: string; confirmText?: string }) => Promise<boolean>>().mockResolvedValue(false),
    mockToastShow: vi.fn(),
  };
});

// ─── Module mocks（view 缺失场景不建 EditorView，CM mock 从简——
//     模块级 EDITOR_FONT_THEME = EditorView.theme(...) 仍需可用） ───

vi.mock("@codemirror/view", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@codemirror/view")>();
  const MockEditorView = class {
    static theme = vi.fn(() => []);
    static updateListener = { of: vi.fn(() => []) };
    static lineWrapping = Symbol("lineWrapping");
  };
  return { ...actual, EditorView: MockEditorView, keymap: { of: vi.fn(() => []) } };
});

vi.mock("../ipc/notify", () => ({
  onFsEvent: vi.fn((cb: (event: { paths: string[]; kind: string }) => void) => {
    h.mockOnFsCallback = cb;
    return () => {};
  }),
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
  updateDiffGutter: vi.fn(),
  clearDiffGutter: vi.fn(),
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
import { useCodeMirror } from "../panels/editor/useCodeMirror";

// ─── 辅助函数 ───

/** viewless 挂载（htmlviewer render 态：container=null + onDocContent 回写通道） */
async function renderViewless(props?: Partial<Parameters<typeof useCodeMirror>[0]>) {
  const onDocContent = vi.fn();
  const result = renderHook(() =>
    useCodeMirror({
      container: null,
      filePath: "/test/a.html",
      panelId: "viewless-test",
      onDocContent,
      ...props,
    }),
  );
  // 无 view 建锚——以 fs-event 订阅到位为挂载完成信号
  await waitFor(() => {
    expect(h.mockOnFsCallback).not.toBeNull();
  }, { timeout: 3000 });
  return { result, onDocContent };
}

/** 触发 fs-event（handler async——调用后须 waitFor 断言下游效果） */
async function fireFsEvent(event: { paths: string[]; kind: string }): Promise<void> {
  await act(async () => {
    h.mockOnFsCallback!(event);
    await Promise.resolve();
  });
}

/** 触发一次 fs-poll 心跳 */
async function firePoll(): Promise<void> {
  await act(async () => {
    h.mockOnFsPollCallback!();
    await Promise.resolve();
  });
}

// ─── 测试套件 ───

describe("useCodeMirror view 缺失外部变更回写（htmlviewer render 态）", () => {
  beforeEach(() => {
    h.mockOnFsCallback = null;
    h.mockOnFsPollCallback = null;
    vi.clearAllMocks();
    h.mockReadFile.mockResolvedValue("page v1");
    h.mockConfirmDialog.mockResolvedValue(false);
  });

  it("V1. viewless + Modify 命中 → onDocContent(content, \"reload\") 回写【防复发】", async () => {
    const { onDocContent } = await renderViewless();

    // 防复发（2026-09 根因 2）：旧实现 `if (!view) return` 把已读内容丢弃，
    // htmlviewer render 态永不更新——断言对旧代码红
    await fireFsEvent({ paths: ["/test/a.html"], kind: "Modify" });

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/a.html");
    }, { timeout: 3000 });
    await waitFor(() => {
      expect(onDocContent).toHaveBeenCalledWith("page v1", "reload");
    }, { timeout: 3000 });
  });

  it("V2. 同内容再次事件 → lastReported 判等短路（cb 不重复调）", async () => {
    const { onDocContent } = await renderViewless();

    await fireFsEvent({ paths: ["/test/a.html"], kind: "Modify" });
    await waitFor(() => {
      expect(onDocContent).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });

    onDocContent.mockClear();
    h.mockReadFile.mockClear();
    await fireFsEvent({ paths: ["/test/a.html"], kind: "Modify" });

    // 读盘照走（事件路径权威信号），但内容与上次上报一致 → 不再回写
    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(onDocContent).not.toHaveBeenCalled();
  });

  it("V3. 无 onDocContent（EditorPanel 形态）→ 不 throw，零影响", async () => {
    renderHook(() =>
      useCodeMirror({
        container: null,
        filePath: "/test/a.html",
        panelId: "viewless-test",
      }),
    );
    await waitFor(() => {
      expect(h.mockOnFsCallback).not.toBeNull();
    }, { timeout: 3000 });

    await fireFsEvent({ paths: ["/test/a.html"], kind: "Modify" });

    await waitFor(() => {
      expect(h.mockReadFile).toHaveBeenCalledWith("/test/a.html");
    }, { timeout: 3000 });
    // cb 缺失分支静默 return——无 unhandled rejection 即通过
  });

  it("V4. viewless + Create（原子替换写折叠产物）→ cb(\"reload\") 回写", async () => {
    const { onDocContent } = await renderViewless();

    await fireFsEvent({ paths: ["/test/a.html"], kind: "Create" });

    await waitFor(() => {
      expect(onDocContent).toHaveBeenCalledWith("page v1", "reload");
    }, { timeout: 3000 });
  });

  it("V5. poll + dirty + 取消 → 不调 cb；同内容再次心跳拒绝标记命中不再弹", async () => {
    const { result, onDocContent } = await renderViewless();

    act(() => {
      result.result.current.markDirty();
    });
    h.mockReadFile.mockResolvedValue("page v2");

    await firePoll();
    await waitFor(() => {
      expect(h.mockConfirmDialog).toHaveBeenCalledTimes(1);
    }, { timeout: 3000 });
    expect(onDocContent).not.toHaveBeenCalled();

    // 同一磁盘内容再次心跳 → 拒绝标记命中，不再弹
    await firePoll();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(h.mockConfirmDialog).toHaveBeenCalledTimes(1);
    expect(onDocContent).not.toHaveBeenCalled();
  });
});
