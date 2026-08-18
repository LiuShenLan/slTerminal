// diff-panel-stale-banner.test.tsx — git diff 失败「内容可能过时」提示条测试（FE-10）
//
// 覆盖：初始加载 gitDiff 失败 → 面板内提示条渲染；gitDiff 成功 → 无提示条；
// 保存后 gitDiff 失败 → 提示条出现

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
    // 契约兜底：Error → message，其余 String
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

vi.mock("../lib", () => ({
  confirmDialog: mockConfirmDialog,
  toast: { show: mockToastShow },
  getErrorMessage: mockGetErrorMessage,
}));

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

vi.mock("../panels/editor/activeEditor", () => ({
  setActiveEditor: mockSetActiveEditor,
  clearActiveEditor: mockClearActiveEditor,
}));

// ── 导入 ──────────────────────────────────────────────────────

import DiffPanel from "../panels/diff/DiffPanel";

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

/** 从 usePanelFocus mock 调用中取出 activate 回调（非 null 容器那次的第 3 参） */
function getActivate(): (() => void) | undefined {
  const activate = mockUsePanelFocus.mock.calls.find((c) => c[1] !== null)?.[2];
  return activate as (() => void) | undefined;
}

/** 渲染至 ready 态并返回保存 actions */
async function renderReady() {
  const { container } = render(
    React.createElement(DiffPanel, { params: makeParams() }),
  );
  await waitFor(() => {
    expect(container.querySelector('[data-e2e="diff-panel"]')).toBeTruthy();
  });
  // 容器 ref 桥接：ready 渲染后 usePanelFocus 需再等一轮重渲染才收到非 null 容器
  // （renderKey bridge effect），未等待时 getActivate 取不到 activate → actions 为空
  await waitFor(() => {
    expect(mockUsePanelFocus.mock.calls.some((c) => c[1] !== null)).toBe(true);
  }, { timeout: 3000 });
  getActivate()?.();
  const calls = mockSetActiveEditor.mock.calls;
  const actions = (calls[calls.length - 1]?.[0] ?? undefined) as
    | { save: () => void }
    | undefined;
  return { container, actions };
}

describe("DiffPanel 内容可能过时提示条（FE-10）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGitFileAtHead.mockResolvedValue("// HEAD\nline1\n");
    mockReadFile.mockResolvedValue("// workdir\nline1\n");
    mockGitDiff.mockResolvedValue([]);
    mockOnFsEvent.mockReturnValue(() => {});
    mockWriteFile.mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("S1: 初始加载 gitDiff 失败 → 渲染『内容可能过时』提示条", async () => {
    mockGitDiff.mockRejectedValue(new Error("git 不可用"));

    const { findByTestId, findByText } = render(
      React.createElement(DiffPanel, { params: makeParams() }),
    );

    await findByTestId("diff-stale-banner", undefined, { timeout: 3000 });
    await findByText("内容可能过时——git diff 获取失败");
  });

  it("S2: gitDiff 成功 → 不渲染提示条", async () => {
    const { container } = await renderReady();
    await waitFor(() => {
      expect(container.querySelector('[data-testid="diff-stale-banner"]')).toBeNull();
    });
  });

  it("S3: 保存后 gitDiff 失败 → 提示条出现", async () => {
    const { container, actions } = await renderReady();
    // 初始无提示条
    expect(container.querySelector('[data-testid="diff-stale-banner"]')).toBeNull();

    // 保存时 gitDiff 第二次调用失败
    mockGitDiff.mockRejectedValueOnce(new Error("diff 失败"));
    actions!.save();

    await waitFor(() => {
      expect(container.querySelector('[data-testid="diff-stale-banner"]')).toBeTruthy();
    }, { timeout: 3000 });
    expect(container.textContent).toContain("内容可能过时");
  });
});
