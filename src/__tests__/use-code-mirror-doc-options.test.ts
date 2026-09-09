// use-code-mirror-doc-options.test.ts — useCodeMirror docViewer 扩展选项测试
//
// 覆盖 S3 增量（EditorPanel 默认行为零变化由既有 use-code-mirror.test.ts 守护）：
//   1. initialDoc 快照 → 跳过磁盘读取（草稿回填免二次 IPC）
//   2. onDocContent init 源回调（缓冲建立完成即回传）
//   3. gitGutterEnabled=false → 不读 git（docViewer 预览面板）
//   4. 默认（不传新选项）→ readFile/gitDiff 现行行为保持
//
// edit/reload 源回调与真实文档流由 S6 markdown-panel / S4 html-panel 集成测试
// 覆盖（updateListener 真实触发需要真实 EditorView，mock 链不可驱动）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// ─── Hoisted mocks ───
const { mockReadFile, mockWriteFile, mockGitDiff } = vi.hoisted(() => ({
  mockReadFile: vi.fn().mockResolvedValue("file-content"),
  mockWriteFile: vi.fn().mockResolvedValue(undefined),
  mockGitDiff: vi.fn().mockResolvedValue([]),
}));

vi.mock("../ipc/fs", () => ({
  readFile: mockReadFile,
  writeFile: mockWriteFile,
  // FE-08: stat 预检前置——默认小文件（0 字节）不触发警告/超限分支
  statFile: vi.fn().mockResolvedValue({ sizeBytes: 0, mtimeMs: null }),
}));
vi.mock("../ipc/git", () => ({ gitDiff: mockGitDiff }));
vi.mock("../ipc/notify", () => ({
  onFsEvent: vi.fn(() => () => {}),
  startWatch: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../ipc/dialog", () => ({ save: vi.fn() }));
vi.mock("../panels/editor/gitGutter", () => ({
  diffGutter: vi.fn(() => []),
  updateDiffGutter: vi.fn(),
  clearDiffGutter: vi.fn(),
}));
vi.mock("../features/shortcuts", () => ({
  usePanelFocus: vi.fn(),
}));
vi.mock("@codemirror/view", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@codemirror/view")>();
  const MockEditorView = class {
    dom: HTMLDivElement;
    dispatch = vi.fn();
    destroy = vi.fn();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    state: any;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    constructor(config: any) {
      this.dom = document.createElement("div");
      config.parent.appendChild(this.dom);
      this.state = config.state;
    }
    static theme = vi.fn(() => []);
    static updateListener = { of: vi.fn(() => []) };
    static lineWrapping = Symbol("lineWrapping");
  };
  return {
    ...actual,
    EditorView: MockEditorView,
    keymap: { of: vi.fn(() => []) },
  };
});

import { useCodeMirror } from "../panels/editor/useCodeMirror";

function createContainer(): HTMLDivElement {
  const div = document.createElement("div");
  document.body.appendChild(div);
  return div;
}

describe("useCodeMirror docViewer 选项", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = createContainer();
    mockReadFile.mockClear();
    mockReadFile.mockResolvedValue("file-content");
    mockGitDiff.mockClear();
    mockGitDiff.mockResolvedValue([]);
  });

  afterEach(() => {
    container.remove();
    vi.clearAllMocks();
  });

  it("initialDoc 有值 → 跳过磁盘读取（草稿回填免二次 IPC）", async () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/doc.md",
        panelId: "p1",
        initialDoc: "草稿快照内容",
      }),
    );
    // initEditor 异步路径 flush（view 创建含 readFile 分支判断）
    await act(async () => {});
    expect(mockReadFile).not.toHaveBeenCalled();
  });

  it("仅 filePath（无 initialDoc）→ 现行行为：磁盘读取一次", async () => {
    renderHook(() =>
      useCodeMirror({ container, filePath: "/test/doc.md", panelId: "p2" }),
    );
    await waitFor(() => {
      expect(mockReadFile).toHaveBeenCalledTimes(1);
    });
    expect(mockReadFile).toHaveBeenCalledWith("/test/doc.md");
  });

  it("onDocContent init 源：缓冲建立完成即回传一次", async () => {
    const cb = vi.fn();
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/test/doc.md",
        panelId: "p3",
        onDocContent: cb,
      }),
    );
    await act(async () => {});
    expect(cb).toHaveBeenCalledTimes(1);
    expect(cb.mock.calls[0]?.[1]).toBe("init");
  });

  it("gitGutterEnabled=false → 不调用 gitDiff（docViewer 预览面板）", async () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/repo/src/doc.md",
        panelId: "p4",
        gitGutterEnabled: false,
      }),
    );
    await act(async () => {});
    expect(mockGitDiff).not.toHaveBeenCalled();
  });

  it("默认 gitGutterEnabled=true → gitDiff 调用（EditorPanel 行为不变）", async () => {
    renderHook(() =>
      useCodeMirror({
        container,
        filePath: "/repo/src/doc.md",
        panelId: "p5",
      }),
    );
    await waitFor(() => {
      expect(mockGitDiff).toHaveBeenCalledTimes(1);
    });
  });
});
