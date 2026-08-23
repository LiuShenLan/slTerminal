// diff-panel.test.tsx — DiffPanel 组件 L2 测试
//
// 覆盖：三态（loading/content/error）、data-e2e 容器渲染、gitDiff 调用、
// 滚动同步（模拟 scroll 断言对侧 scrollTop + syncingRef 防循环）、
// 保存后重新调 gitDiff + writeFile、外部修改重载

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor, cleanup } from "@testing-library/react";
import React from "react";

// ── mock 状态（vi.hoisted 确保模块级 mock 前就绪） ─────────

const { mockGitFileAtHead, mockGitDiff, mockReadFile, mockWriteFile, mockOnFsEvent,
  mockUseFontSizeWheel, mockSetEditorFontSize, mockUsePanelFocus,
  mockSetActiveEditor, mockClearActiveEditor, mockConfirmDialog, mockToastShow,
  mockGetErrorMessage } = vi.hoisted(
  () => ({
    mockGitFileAtHead: vi.fn(),
    mockGitDiff: vi.fn(),
    mockReadFile: vi.fn(),
    mockWriteFile: vi.fn(),
    mockOnFsEvent: vi.fn(),
    mockUseFontSizeWheel: vi.fn(),
    mockSetEditorFontSize: vi.fn(),
    mockUsePanelFocus: vi.fn(),
    mockSetActiveEditor: vi.fn(),
    mockClearActiveEditor: vi.fn(),
    mockConfirmDialog: vi.fn(),
    mockToastShow: vi.fn(),
    // 契约兜底（FE-43）：Error → message，其余 String
    mockGetErrorMessage: vi.fn((err: unknown) =>
      err instanceof Error ? err.message : String(err),
    ),
  }),
);

// ── 模块级 mock ──────────────────────────────────────────────

vi.mock("../ipc/git", () => ({
  gitFileAtHead: mockGitFileAtHead,
  gitDiff: mockGitDiff,
}));

vi.mock("../ipc", () => ({
  fs: {
    readFile: mockReadFile,
    writeFile: mockWriteFile,
  },
}));

vi.mock("../ipc/notify", () => ({
  onFsEvent: mockOnFsEvent,
}));

vi.mock("../lib/useFontSizeWheel", () => ({
  useFontSizeWheel: mockUseFontSizeWheel,
}));

// FE-02：浮层单点 mock——confirmDialog/toast（不 mock window）
// FE-43：getErrorMessage 同库导出，一并 mock（契约兜底见 vi.hoisted）
// TQ-A-05: importOriginal 保留 barrel 其余真实导出——新增 ../lib 引用即自动获得真实现，不再 undefined
vi.mock("../lib", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib")>();
  return {
    ...actual,
    confirmDialog: mockConfirmDialog,
    toast: { ...actual.toast, show: mockToastShow },
    getErrorMessage: mockGetErrorMessage,
  };
});

vi.mock("../stores/fontSize", () => ({
  FONT_SIZE_MIN: 8,
  FONT_SIZE_MAX: 32,
}));

vi.mock("../stores", () => ({
  useFontSize: (selector: (s: Record<string, unknown>) => unknown) => {
    const state = { editorFontSize: 14, setEditorFontSize: mockSetEditorFontSize };
    return typeof selector === "function" ? selector(state) : undefined;
  },
}));

vi.mock("../features/shortcuts", () => ({
  usePanelFocus: mockUsePanelFocus,
}));

// 注意路径：src/panels/editor/activeEditor（测试文件在 src/__tests__/，须补 panels 段）
vi.mock("../panels/editor/activeEditor", () => ({
  setActiveEditor: mockSetActiveEditor,
  clearActiveEditor: mockClearActiveEditor,
}));

// ── 导入 ──────────────────────────────────────────────────────

import DiffPanel from "../panels/diff/DiffPanel";
import { act } from "@testing-library/react";
import { EditorView } from "@codemirror/view";
import { diffMarkersField, headDiffMarkersField } from "../panels/editor/gitGutter";
import { MAX_FILE_SIZE_BYTES, LARGE_FILE_WARN_BYTES } from "../panels/editor/useCodeMirror";

/** 构造测试参数 */
function makeParams(overrides: Partial<{
  panelId: string;
  filePath: string;
  oldPath: string | undefined;
  repoPath: string;
}> = {}) {
  return {
    panelId: "diff-1",
    filePath: "D:/repo/src/test.ts",
    oldPath: undefined,
    repoPath: "D:/repo",
    ...overrides,
  };
}

/** 经 .cm-editor DOM 反查 EditorView 实例（CM6 公开 API findFromDOM） */
function getDiffView(
  container: HTMLElement,
  side: "diff-left" | "diff-right",
): EditorView | null {
  const el = container.querySelector(`[data-e2e="${side}"] .cm-editor`);
  return el ? EditorView.findFromDOM(el as HTMLElement) : null;
}

/** 占位 Decoration 数量——placeholderField 经 EditorView.decorations facet 提供 */
function placeholderCount(view: EditorView | null): number {
  if (!view) return -1;
  return view.state
    .facet(EditorView.decorations)
    .reduce((n, d) => n + (d && typeof d !== "function" ? d.size : 0), 0);
}

describe("DiffPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGitFileAtHead.mockResolvedValue("// HEAD\nline1\nline2\n");
    mockReadFile.mockResolvedValue("// workdir\nline1\nline2\n");
    mockGitDiff.mockResolvedValue([]);
    mockOnFsEvent.mockReturnValue(() => {});
  });

  afterEach(() => {
    // 项目未启用 RTL auto-cleanup（vitest 无 globals）——必须显式卸载，
    // 否则组件跨用例残留（旧组件重渲染仍产生 mock 调用，污染 find/次数断言）
    cleanup();
    vi.restoreAllMocks();
  });

  // ── 加载态 ──────────────────────────────────────────

  it("初始为 loading 状态——渲染加载中文本", () => {
    // 保持 Promise pending 以验证 loading 态
    const p = new Promise<never>(() => {});
    mockGitFileAtHead.mockReturnValue(p);
    mockReadFile.mockReturnValue(p);

    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    expect(container.textContent).toContain("加载中");
  });

  // ── 错误态 ──────────────────────────────────────────

  it("gitFileAtHead reject 时显示 HEAD 不存在错误文案", async () => {
    mockGitFileAtHead.mockRejectedValue(new Error("HEAD 中不存在"));

    const { findByText } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await findByText("该文件在 HEAD 中不存在");
  });

  // ── 正常渲染 ────────────────────────────────────────

  it("加载成功 → diff-panel 容器含 diff-left/diff-right", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
      expect(container.querySelector('[data-e2e="diff-left"]')).toBeTruthy();
      expect(container.querySelector('[data-e2e="diff-right"]')).toBeTruthy();
    });
  });

  it("加载后调用 gitDiff(repoPath, filePath) 获取 diff hunks", async () => {
    const params = makeParams();
    const { container } = render(
      React.createElement(DiffPanel, { params }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    expect(mockGitDiff).toHaveBeenCalledWith(params.repoPath, params.filePath);
  });

  it("oldPath 存在时用 oldPath 调 gitFileAtHead", async () => {
    const params = makeParams({
      oldPath: "D:/repo/src/old.ts",
      filePath: "D:/repo/src/new.ts",
    });
    const { container } = render(
      React.createElement(DiffPanel, { params }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    expect(mockGitFileAtHead).toHaveBeenCalledWith(
      params.repoPath,
      "D:/repo/src/old.ts",
    );
  });

  // ── 保存链验证 ──────────────────────────────────────

  it("保存→ writeFile 写盘 + gitDiff 重调 + 双侧 gutter/占位刷新全链", async () => {
    const params = makeParams();
    const { container } = render(
      React.createElement(DiffPanel, { params }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });

    // 初始 gitDiff 被调用一次
    expect(mockGitDiff).toHaveBeenCalledTimes(1);

    // 真实触发保存：经 usePanelFocus 的 activate 回调 → setActiveEditor → 取 editorActions.save()
    // 注意：非 null 容器调用发生在 renderKey bridge 重渲染后——须 waitFor 轮询
    let activate: (() => void) | undefined;
    await waitFor(() => {
      activate = mockUsePanelFocus.mock.calls.find((c) => c[1] !== null)?.[2];
      expect(activate).toBeTypeOf("function");
    });
    activate?.();
    const calls = mockSetActiveEditor.mock.calls;
    const actions = (calls[calls.length - 1]?.[0] ?? undefined) as
      | { save: () => void; toggleWordWrap: () => void }
      | undefined;
    expect(actions?.save).toBeTypeOf("function");

    // 保存路径：gitDiff 第二次返回非空 hunks（纯增 2 + 纯删 1，行号须落在 3 行文档内
    // → 双侧均有 gutter marker 与占位 Decoration）
    mockGitDiff.mockResolvedValue([
      { oldStart: 1, oldLines: 0, newStart: 2, newLines: 2 },
      { oldStart: 3, oldLines: 1, newStart: 3, newLines: 0 },
    ]);

    actions!.save();

    // 1) writeFile 写盘（内容 = 右栏文档全文）
    await waitFor(() => {
      expect(mockWriteFile).toHaveBeenCalledWith(
        params.filePath,
        "// workdir\nline1\nline2\n",
      );
    });

    // 2) gitDiff 重调（第二次，参数为 repoPath + filePath）
    await waitFor(() => {
      expect(mockGitDiff).toHaveBeenCalledTimes(2);
      expect(mockGitDiff).toHaveBeenLastCalledWith(params.repoPath, params.filePath);
    });

    // 3) 双侧 gutter 刷新（marker 从无到有——保存前初始 hunks 为空）
    await waitFor(() => {
      const leftView = getDiffView(container, "diff-left");
      const rightView = getDiffView(container, "diff-right");
      expect(leftView).toBeTruthy();
      expect(rightView).toBeTruthy();
      expect(leftView!.state.field(headDiffMarkersField).size).toBeGreaterThan(0);
      expect(rightView!.state.field(diffMarkersField).size).toBeGreaterThan(0);
    });

    // 4) 双侧占位刷新（纯增 → 左侧占位，纯删 → 右侧占位）
    await waitFor(() => {
      expect(placeholderCount(getDiffView(container, "diff-left"))).toBeGreaterThan(0);
      expect(placeholderCount(getDiffView(container, "diff-right"))).toBeGreaterThan(0);
    });
  });

  it("保存失败 → toast.show(\"error\") 提示（FE-02 浮层回归）", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });

    // 经 usePanelFocus activate 回调取得 editorActions（照保存链用例模式）
    let activate: (() => void) | undefined;
    await waitFor(() => {
      activate = mockUsePanelFocus.mock.calls.find((c) => c[1] !== null)?.[2];
      expect(activate).toBeTypeOf("function");
    });
    activate?.();
    const calls = mockSetActiveEditor.mock.calls;
    const actions = (calls[calls.length - 1]?.[0] ?? undefined) as
      | { save: () => void }
      | undefined;

    // 必须用 mockRejectedValueOnce——默认实现（mockRejectedValue）会残留到后续用例
    // （beforeEach 仅 clearAllMocks 不清 implementation），导致「保存后 gitDiff 重查」
    // 用例里 writeFile 提前 reject → save 早退 → gitDiff 未重调 → 断言失败
    mockWriteFile.mockRejectedValueOnce(new Error("磁盘只读"));
    actions!.save();

    // 保存失败不弹原生 alert——经 toast 提示（FE-02）
    // FE-43：文案为 getErrorMessage 解析后消息（Error → message）
    await waitFor(() => {
      expect(mockToastShow).toHaveBeenCalledWith("error", "保存失败: 磁盘只读");
    });
  });

  // ── 滚动同步（fake timers，消除固定等待） ──────────────

  describe("滚动同步（fake timers）", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    /**
     * 推进 fake timers 并 flush 微任务。
     * 注意：sinon tickAsync 在定时器队列为空时不 yield microtask——loading 的
     * promise 链（setState ready → 注册 100ms 滚动绑定/50ms 占位 timer）须分两段：
     * 第一段借 act flush 让 ready 渲染注册 timers，第二段触发它们。
     */
    async function flushAsync() {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(200);
      });
    }

    /** 渲染并推进全部初始异步 */
    async function renderReady() {
      const utils = render(
        React.createElement(DiffPanel, { params: makeParams() }),
      );
      await flushAsync();
      return utils;
    }

    it("左侧 .cm-scroller scroll → 右侧 scrollTop 跟随", async () => {
      const { container } = await renderReady();

      const leftScroller = container
        .querySelector('[data-e2e="diff-left"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      const rightScroller = container
        .querySelector('[data-e2e="diff-right"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      expect(leftScroller).toBeTruthy();
      expect(rightScroller).toBeTruthy();

      leftScroller.scrollTop = 150;
      leftScroller.dispatchEvent(new Event("scroll", { bubbles: true }));

      // scroll handler 为同步执行——推进 timer 后无需再等待
      expect(rightScroller.scrollTop).toBe(150);
    });

    it("右侧 .cm-scroller scroll → 左侧 scrollTop 跟随", async () => {
      const { container } = await renderReady();

      const leftScroller = container
        .querySelector('[data-e2e="diff-left"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      const rightScroller = container
        .querySelector('[data-e2e="diff-right"]')!
        .querySelector(".cm-scroller") as HTMLElement;

      rightScroller.scrollTop = 250;
      rightScroller.dispatchEvent(new Event("scroll", { bubbles: true }));

      expect(leftScroller.scrollTop).toBe(250);
    });

    it("syncingRef 防循环——同向滚动不产生回弹", async () => {
      const { container } = await renderReady();

      const leftScroller = container
        .querySelector('[data-e2e="diff-left"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      const rightScroller = container
        .querySelector('[data-e2e="diff-right"]')!
        .querySelector(".cm-scroller") as HTMLElement;

      // 设置初始滚动位置
      leftScroller.scrollTop = 300;
      rightScroller.scrollTop = 300;

      // 左侧滚动触发同步
      leftScroller.scrollTop = 500;
      leftScroller.dispatchEvent(new Event("scroll", { bubbles: true }));

      expect(rightScroller.scrollTop).toBe(500);
      // 左侧 scrollTop 保持在 500（未被右侧回写改变）——syncingRef 防循环
      expect(leftScroller.scrollTop).toBe(500);

      // 右侧再次滚动——左侧仍应同步而不会无限循环
      rightScroller.scrollTop = 800;
      rightScroller.dispatchEvent(new Event("scroll", { bubbles: true }));

      expect(leftScroller.scrollTop).toBe(800);
    });

    it("filePath 切换重载后滚动同步重绑定仍生效", async () => {
      const { container, rerender } = await renderReady();

      // 首次滚动同步生效
      let leftScroller = container
        .querySelector('[data-e2e="diff-left"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      let rightScroller = container
        .querySelector('[data-e2e="diff-right"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      leftScroller.scrollTop = 150;
      leftScroller.dispatchEvent(new Event("scroll", { bubbles: true }));
      expect(rightScroller.scrollTop).toBe(150);

      // 切换 filePath → 重新 loading → ready → view 重建 + 滚动 effect 重绑
      mockGitFileAtHead.mockResolvedValue("// HEAD v2\n");
      mockReadFile.mockResolvedValue("// workdir v2\n");
      rerender(
        React.createElement(DiffPanel, {
          params: makeParams({ filePath: "D:/repo/src/test2.ts" }),
        }),
      );
      // 两段推进：先让 reload 的 promise 链完成注册新 view，再触发新 100ms 滚动绑定 timer
      await flushAsync();

      // 新 scroller 重新绑定后滚动同步仍生效（state.kind 变化触发 effect 重跑）
      leftScroller = container
        .querySelector('[data-e2e="diff-left"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      rightScroller = container
        .querySelector('[data-e2e="diff-right"]')!
        .querySelector(".cm-scroller") as HTMLElement;
      expect(leftScroller).toBeTruthy();
      expect(rightScroller).toBeTruthy();

      leftScroller.scrollTop = 300;
      leftScroller.dispatchEvent(new Event("scroll", { bubbles: true }));
      expect(rightScroller.scrollTop).toBe(300);
    });
  });

  // ── 外部修改重载 ────────────────────────────────────

  it("净态外部 Modify → 自动重载 readFile 并替换右侧内容", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );

    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });

    // 捕获 onFsEvent 回调（第一个注册 = 外部修改监听）
    const fsEventCb = mockOnFsEvent.mock.calls[0]?.[0];
    expect(fsEventCb).toBeTypeOf("function");

    mockReadFile.mockResolvedValue("外部修改后的内容");

    fsEventCb({
      paths: ["D:/repo/src/test.ts"],
      kind: "Modify",
    });

    // readFile 被再次调用（重载）
    await waitFor(() => {
      expect(mockReadFile).toHaveBeenCalledTimes(2);
    });

    // 净态自动重载：右栏文档被替换为磁盘新内容
    await waitFor(() => {
      const rightView = getDiffView(container, "diff-right");
      expect(rightView?.state.doc.toString()).toBe("外部修改后的内容");
    });
  });

  it("非目标文件 Modify → 不触发重载", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );

    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });

    // 初始加载基线：ready 态依赖 Promise.all 中的 readFile——轮询等待恰 1 次（替代固定 200ms 等待）
    await waitFor(() => {
      expect(mockReadFile).toHaveBeenCalledTimes(1);
    });

    const fsEventCb = mockOnFsEvent.mock.calls[0]?.[0];

    // 修改的路径与当前文件不匹配
    fsEventCb({
      paths: ["D:/repo/src/other.ts"],
      kind: "Modify",
    });

    // 路径匹配判定在 handler 内同步执行（无 debounce）——不命中即同步 return，
    // 无任何挂起的重载调用，调用数立即可断言（替代固定 50ms 等待）
    expect(mockReadFile).toHaveBeenCalledTimes(1);
  });

  // ── 占位刷新同步（EDF-02） ────────────────────────────────

  it("初始 gitDiff 非空 → 双侧创建占位 Decoration（50ms 刷新 timer 路径）", async () => {
    mockGitDiff.mockResolvedValue([
      { oldStart: 1, oldLines: 0, newStart: 2, newLines: 2 }, // 纯增 → 左侧占位
      { oldStart: 3, oldLines: 1, newStart: 3, newLines: 0 }, // 纯删 → 右侧占位
    ]);
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    // 右侧挂载 effect 的 50ms timer 触发 refreshPlaceholders
    await waitFor(() => {
      expect(placeholderCount(getDiffView(container, "diff-left"))).toBeGreaterThan(0);
      expect(placeholderCount(getDiffView(container, "diff-right"))).toBeGreaterThan(0);
    });
  });

  it("保存后 gitDiff 返回空 hunks → 双侧占位与 gutter 清空", async () => {
    mockGitDiff.mockResolvedValue([
      { oldStart: 3, oldLines: 0, newStart: 3, newLines: 2 },
    ]);
    const params = makeParams();
    const { container } = render(
      React.createElement(DiffPanel, { params }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    // 初始有占位（纯增 → 左侧）
    await waitFor(() => {
      expect(placeholderCount(getDiffView(container, "diff-left"))).toBeGreaterThan(0);
    });

    // 经 usePanelFocus activate 回调触发保存（非 null 容器调用在 bridge 重渲染后——waitFor 轮询）
    let activate: (() => void) | undefined;
    await waitFor(() => {
      activate = mockUsePanelFocus.mock.calls.find((c) => c[1] !== null)?.[2];
      expect(activate).toBeTypeOf("function");
    });
    activate?.();
    const calls = mockSetActiveEditor.mock.calls;
    const actions = (calls[calls.length - 1]?.[0] ?? undefined) as
      | { save: () => void }
      | undefined;

    // 保存时 gitDiff 返回空 → 走 clear 分支
    mockGitDiff.mockResolvedValue([]);
    actions!.save();

    await waitFor(() => {
      expect(mockGitDiff).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      const leftView = getDiffView(container, "diff-left");
      const rightView = getDiffView(container, "diff-right");
      expect(leftView!.state.field(headDiffMarkersField).size).toBe(0);
      expect(rightView!.state.field(diffMarkersField).size).toBe(0);
      expect(placeholderCount(leftView)).toBe(0);
      expect(placeholderCount(rightView)).toBe(0);
    });
  });

  // ── 左侧 .git 变更刷新 HEAD（EDF-02） ────────────────────

  it("左侧 .git 路径变更 → gitFileAtHead 重取并刷新左侧内容", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    expect(mockGitFileAtHead).toHaveBeenCalledTimes(1);

    // 第二个 onFsEvent 注册 = 左侧 .git 监听（第一个为外部修改监听）
    const gitCb = mockOnFsEvent.mock.calls[1]?.[0];
    expect(gitCb).toBeTypeOf("function");

    mockGitFileAtHead.mockResolvedValue("// HEAD v2\nline1\nline2\n");
    gitCb({ paths: ["D:/repo/.git/index"], kind: "Modify" });

    await waitFor(() => {
      const leftView = getDiffView(container, "diff-left");
      expect(leftView?.state.doc.toString()).toContain("HEAD v2");
    });
  });

  it("左侧非 .git 路径变更 → 不重取 HEAD", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const gitCb = mockOnFsEvent.mock.calls[1]?.[0];

    // 非 .git 路径（如普通源文件）→ .git 监听不匹配，gitFileAtHead 不重调
    gitCb({ paths: ["D:/repo/src/other.ts"], kind: "Modify" });

    expect(mockGitFileAtHead).toHaveBeenCalledTimes(1); // 仅初始调用
  });

  // ── 外部修改：脏态弹窗（EDF-02） ─────────────────────────

  it("脏态外部 Modify → confirmDialog 弹窗；取消保留本地修改", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    // 等 CM6 编辑器实例挂载完成（组合运行时可能滞后于面板 DOM——实证 1/8 偶发失败）
    await waitFor(() => {
      expect(getDiffView(container, "diff-right")).toBeTruthy();
    });
    const rightView = getDiffView(container, "diff-right")!;

    // 制造脏：dispatch 文档变更（updateListener → dirtyRef=true）
    rightView.dispatch({ changes: { from: 0, insert: "dirty" } });

    mockConfirmDialog.mockResolvedValue(false);
    mockReadFile.mockResolvedValue("外部新内容");

    const fsCb = mockOnFsEvent.mock.calls[0]?.[0];
    fsCb({ paths: ["D:/repo/src/test.ts"], kind: "Modify" });

    // 脏态必弹窗：confirmDialog 参数含标题/路径/确认按钮语义（确认=重载）
    expect(mockConfirmDialog).toHaveBeenCalledTimes(1);
    expect(mockConfirmDialog.mock.calls[0][0]).toMatchObject({
      title: "外部修改",
      confirmText: "重载",
    });
    expect(mockConfirmDialog.mock.calls[0][0].message).toContain("已被外部修改");

    // 取消 → 不重载，本地修改保留（readFile 仍仅初始 1 次）
    await waitFor(() => {
      expect(mockReadFile).toHaveBeenCalledTimes(1);
    });
    expect(rightView.state.doc.toString()).toContain("dirty");
  });

  it("脏态外部 Modify 确认 → 重载磁盘内容", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    // 等 CM6 编辑器实例挂载完成（组合运行时可能滞后于面板 DOM——实证 1/8 偶发失败）
    await waitFor(() => {
      expect(getDiffView(container, "diff-right")).toBeTruthy();
    });
    const rightView = getDiffView(container, "diff-right")!;
    rightView.dispatch({ changes: { from: 0, insert: "dirty" } });

    mockConfirmDialog.mockResolvedValue(true);
    mockReadFile.mockResolvedValue("外部新内容");

    const fsCb = mockOnFsEvent.mock.calls[0]?.[0];
    fsCb({ paths: ["D:/repo/src/test.ts"], kind: "Modify" });

    await waitFor(() => {
      expect(rightView.state.doc.toString()).toBe("外部新内容");
    });
  });

  // ── 大文件阈值（EDF-02） ────────────────────────────────

  it("workdir 超过 MAX_FILE_SIZE_BYTES → 拒绝打开提示", async () => {
    mockReadFile.mockResolvedValue("x".repeat(MAX_FILE_SIZE_BYTES + 1));
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const rightView = getDiffView(container, "diff-right");
    expect(rightView?.state.doc.toString()).toContain("已拒绝打开以保护内存");
  });

  it("head 超过 LARGE_FILE_WARN_BYTES → 大文件只读警告", async () => {
    mockGitFileAtHead.mockResolvedValue(
      "// HEAD\n" + "y".repeat(LARGE_FILE_WARN_BYTES + 100),
    );
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const leftView = getDiffView(container, "diff-left");
    expect(leftView?.state.doc.toString()).toContain("大文件");
    expect(leftView?.state.doc.toString()).toContain("只读查看");
  });

  // ── 新增：快捷键支持测试（12-15）────────────────────────────

  // 12: 左栏不使用 editable.of(false)——验证编辑器可聚焦
  it("left panel does NOT use editable.of(false)", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    // 等待 CM6 挂载完成——.cm-content 存在后再取元素，避免 flaky
    await waitFor(() => {
      const el = container
        .querySelector('[data-e2e="diff-left"]')
        ?.querySelector(".cm-content");
      expect(el).toBeTruthy();
    });
    const leftContent = container
      .querySelector('[data-e2e="diff-left"]')!
      .querySelector(".cm-content") as HTMLElement;
    // editable.of(false) 会设 contentEditable="false"；
    // 修复后应为 "true"（CM6 默认）或 "inherit"
    expect(leftContent.contentEditable).not.toBe("false");
  });

  // 13: 左栏含搜索扩展——Ctrl+F 触发搜索面板出现
  it("left panel includes search extensions (Ctrl+F opens search)", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    // 等待 CM6 挂载完成——.cm-content 存在后再交互，避免 flaky
    await waitFor(() => {
      const el = container
        .querySelector('[data-e2e="diff-left"]')
        ?.querySelector(".cm-content");
      expect(el).toBeTruthy();
    });
    // 搜索面板初始隐藏，需 Ctrl+F 触发
    const leftContent = container
      .querySelector('[data-e2e="diff-left"]')!
      .querySelector(".cm-content") as HTMLElement;
    // 聚焦左栏内容区
    leftContent.focus();
    // 模拟 Ctrl+F
    leftContent.dispatchEvent(
      new KeyboardEvent("keydown", { key: "f", ctrlKey: true, bubbles: true }),
    );
    // CM6 searchKeymap 应打开搜索面板
    await waitFor(() => {
      const leftEditor = container
        .querySelector('[data-e2e="diff-left"]')!
        .querySelector(".cm-editor");
      const searchPanel = leftEditor?.querySelector(".cm-panel.cm-search");
      expect(searchPanel).toBeTruthy();
    });
  });

  // 14: useFontSizeWheel 左右各调用一次，且收到非 null 容器
  it("calls useFontSizeWheel for both panels with non-null containers", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    expect(mockUseFontSizeWheel).toHaveBeenCalled();
    // bridge effect 的 setRenderKey 触发第三次渲染为异步——
    // 等待 mock 收到非 null 容器调用（左右各一次）
    await waitFor(() => {
      const nonNullCalls = mockUseFontSizeWheel.mock.calls.filter((c) => c[0] !== null);
      expect(nonNullCalls.length).toBeGreaterThanOrEqual(2);
      nonNullCalls.forEach((c) => {
        expect(c[1]).toBe(8);   // FONT_SIZE_MIN
        expect(c[2]).toBe(32);  // FONT_SIZE_MAX
      });
    });
  });

  // 15: usePanelFocus 注册左栏和右栏，且收到非 null 容器
  it("usePanelFocus registers both left AND right containers with non-null elements", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    expect(mockUsePanelFocus).toHaveBeenCalled();
    // 等待 bridge effect 触发重渲染，使非 null 容器调用出现
    await waitFor(() => {
      const calls = mockUsePanelFocus.mock.calls;
      const nonNullCalls = calls.filter((c) => c[1] !== null);
      expect(nonNullCalls.length).toBeGreaterThanOrEqual(2);
      // c[0] = context "editor"，c[1] = container DOM 元素
      nonNullCalls.forEach((c) => expect(c[0]).toBe("editor"));
      nonNullCalls.forEach((c) => expect(c[1]).toBeInstanceOf(HTMLDivElement));
    });
  });

  // ── 布局回归守卫 (L1-L3) ─────────────────────────────

  // S08（FE-10）后 diff-panel 外层为 column 容器：提示条槽位（可选，children[0]）+ 内部 row 容器。
  // 取内部 row 容器（左右 wrapper 的父节点）——按 flexDirection 定位，banner 显隐均稳定。
  function getRowContainer(panel: HTMLElement): HTMLElement {
    const row = Array.from(panel.children).find(
      (el) => (el as HTMLElement).style.flexDirection === "row",
    ) as HTMLElement;
    expect(row).toBeTruthy();
    return row;
  }

  // L1: 外层容器 flexDirection 为 "column"（提示条槽位 + 内部 row 双栏）；内部 row 为 "row"
  it("diff-panel outer container is column with inner row container", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const panel = container.querySelector('[data-e2e="diff-panel"]') as HTMLElement;
    // 外层 = column：提示条（可选）在顶部，内部 row 容器在下
    expect(panel.style.flexDirection).toBe("column");
    // 内部 row 容器保持横向双栏
    expect(getRowContainer(panel).style.flexDirection).toBe("row");
    // row 容器 flex: 1 占满剩余高度（banner 存在时不挤压双栏）
    expect(getRowContainer(panel).style.flex).toContain("1");
  });

  // L2: 左栏 wrapper 有 borderRight 垂直分隔线
  it("left wrapper has borderRight separator", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const panel = container.querySelector('[data-e2e="diff-panel"]') as HTMLElement;
    const leftWrapper = getRowContainer(panel).children[0] as HTMLElement;
    // jsdom 可能将颜色 token 解析为 rgb() 形式，仅验证存在 border-right
    expect(leftWrapper.style.borderRight).toBeTruthy();
    expect(leftWrapper.style.borderRight).toContain("1px solid");
  });

  // L3: 左栏 wrapper 无 borderBottom（旧分隔线已清除）
  it("left wrapper has no borderBottom", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const panel = container.querySelector('[data-e2e="diff-panel"]') as HTMLElement;
    const leftWrapper = getRowContainer(panel).children[0] as HTMLElement;
    expect(leftWrapper.style.borderBottom).toBe("");
  });

  // ── 容器 ref 桥接守卫 (B1-B4) ────────────────────────

  // B1: 左栏 useFontSizeWheel 收到非 null 容器
  it("useFontSizeWheel receives non-null container for left panel", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    // 等待 bridge effect 触发第三次渲染
    await waitFor(() => {
      const nonNullCalls = mockUseFontSizeWheel.mock.calls.filter((c) => c[0] !== null);
      expect(nonNullCalls.length).toBeGreaterThanOrEqual(2);
      nonNullCalls.forEach((c) => expect(c[0]).toBeInstanceOf(HTMLDivElement));
    });
  });

  // B2: useFontSizeWheel 字体范围参数正确（min=8, max=32）
  it("useFontSizeWheel receives FONT_SIZE_MIN/MAX bounds", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    await waitFor(() => {
      const nonNullCalls = mockUseFontSizeWheel.mock.calls.filter((c) => c[0] !== null);
      expect(nonNullCalls.length).toBeGreaterThanOrEqual(2);
      nonNullCalls.forEach((c) => {
        expect(c[1]).toBe(8);  // FONT_SIZE_MIN
        expect(c[2]).toBe(32); // FONT_SIZE_MAX
      });
    });
  });

  // B3: usePanelFocus 收到非 null 容器（左栏和右栏均注册）
  // usePanelFocus 签名: (context, container, activate, deactivate) → c[0]="editor", c[1]=HTMLElement
  it("usePanelFocus receives non-null container for both panels", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    await waitFor(() => {
      const nonNullCalls = mockUsePanelFocus.mock.calls.filter((c) => c[1] !== null);
      expect(nonNullCalls.length).toBeGreaterThanOrEqual(2);
      nonNullCalls.forEach((c) => expect(c[1]).toBeInstanceOf(HTMLDivElement));
    });
  });

  // B4: usePanelFocus context 参数均为 "editor"
  it("usePanelFocus context is editor for all registrations", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    await waitFor(() => {
      const nonNullCalls = mockUsePanelFocus.mock.calls.filter((c) => c[1] !== null);
      expect(nonNullCalls.length).toBeGreaterThanOrEqual(2);
      nonNullCalls.forEach((c) => expect(c[0]).toBe("editor"));
    });
  });

  // ── 滚动功能守卫 (SC1) ───────────────────────────────

  // SC1: CM6 挂载后 .cm-scroller 存在于左右两侧（滚动容器可用）
  it(".cm-scroller exists in both left and right panels", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    // CM6 view 创建/挂载为异步——dockview 8 渲染时序下左侧 view 可能滞后，
    // 须将两侧 .cm-scroller 就绪并入 waitFor 整体轮询（仅等 diff-panel 容器不够）
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
      expect(
        container.querySelector('[data-e2e="diff-left"] .cm-scroller'),
      ).toBeTruthy();
      expect(
        container.querySelector('[data-e2e="diff-right"] .cm-scroller'),
      ).toBeTruthy();
    });
    const leftScroller = container
      .querySelector('[data-e2e="diff-left"]')!
      .querySelector(".cm-scroller") as HTMLElement;
    const rightScroller = container
      .querySelector('[data-e2e="diff-right"]')!
      .querySelector(".cm-scroller") as HTMLElement;
    // CM6 base theme 默认 .cm-scroller { overflow: auto }，若显式设 hidden 则滚动失效
    expect(leftScroller.style.overflowY).not.toBe("hidden");
    expect(rightScroller.style.overflowY).not.toBe("hidden");
    // 横向滚动守卫——CM6 base theme overflowX: auto，inline style 不覆盖
    expect(leftScroller.style.overflowX).not.toBe("hidden");
    expect(rightScroller.style.overflowX).not.toBe("hidden");
  });

  // ── minWidth 属性守卫 (M1-M4) ─────────────────────────

  // M1: 左 wrapper div 有 minWidth: 0（允许 flex 收缩到 50%）
  it("left wrapper div has minWidth 0", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const panel = container.querySelector('[data-e2e="diff-panel"]') as HTMLElement;
    const leftWrapper = getRowContainer(panel).children[0] as HTMLElement;
    expect(leftWrapper.style.minWidth).toBe("0px");
  });

  // M2: 右 wrapper div 有 minWidth: 0
  it("right wrapper div has minWidth 0", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const panel = container.querySelector('[data-e2e="diff-panel"]') as HTMLElement;
    const rightWrapper = getRowContainer(panel).children[1] as HTMLElement;
    expect(rightWrapper.style.minWidth).toBe("0px");
  });

  // M3: diff-left 容器有 minWidth: 0
  it("diff-left container has minWidth 0", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const leftContainer = container.querySelector('[data-e2e="diff-left"]') as HTMLElement;
    expect(leftContainer.style.minWidth).toBe("0px");
  });

  // M4: diff-right 容器有 minWidth: 0
  it("diff-right container has minWidth 0", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const rightContainer = container.querySelector('[data-e2e="diff-right"]') as HTMLElement;
    expect(rightContainer.style.minWidth).toBe("0px");
  });

  // ── overflow 属性回归守卫 (O1-O2) ──────────────────────

  // O1: diff-left 保持 overflow: clip（滚轮穿透到 .cm-scroller）
  it("diff-left container preserves overflow clip", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const leftContainer = container.querySelector('[data-e2e="diff-left"]') as HTMLElement;
    expect(leftContainer.style.overflow).toBe("clip");
  });

  // O2: diff-right 保持 overflow: clip
  it("diff-right container preserves overflow clip", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const rightContainer = container.querySelector('[data-e2e="diff-right"]') as HTMLElement;
    expect(rightContainer.style.overflow).toBe("clip");
  });

  // ── flex 结构守卫 (F1) ─────────────────────────────────

  // F1: 左右 wrapper 均为 flex: 50% + display: flex（等宽分配）
  it("both wrapper divs have flex 50% and display flex", async () => {
    const { container } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
    });
    const panel = container.querySelector('[data-e2e="diff-panel"]') as HTMLElement;
    const row = getRowContainer(panel);
    const leftWrapper = row.children[0] as HTMLElement;
    const rightWrapper = row.children[1] as HTMLElement;
    // 等宽分配（jsdom 将 flex: "50%" 展开为 "1 1 50%"）
    expect(leftWrapper.style.flex).toContain("50%");
    expect(rightWrapper.style.flex).toContain("50%");
    // 内层 flex 容器
    expect(leftWrapper.style.display).toBe("flex");
    expect(rightWrapper.style.display).toBe("flex");
  });
});
