// L2 编辑器面板测试——保留 useCodeMirror mock（CodeMirror 6 在 jsdom 不可用），验证正确的参数传递
// CodeMirror 6 在 jsdom 中不可用，mock 是必需的；但测试应验证正确的参数传递
import { describe, it, expect, afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";

const { mockUseCodeMirror, mockReadFileRange } = vi.hoisted(() => ({
  mockUseCodeMirror: vi.fn(),
  mockReadFileRange: vi.fn(),
}));

// CP-022: EditorPanel 从 useCodeMirror 解构 largeFile 信号——mock 须返回完整形状
// （默认 null = 正常 CM 形态;超限用例用 mockReturnValueOnce 覆盖）
const defaultHookResult = () => ({
  largeFile: null as { filePath: string; sizeBytes: number } | null,
  getContent: vi.fn(() => ""),
  markClean: vi.fn(),
  markDirty: vi.fn(),
});
mockUseCodeMirror.mockImplementation(defaultHookResult);

vi.mock("../panels/editor/useCodeMirror", () => ({
  useCodeMirror: mockUseCodeMirror,
  getLanguageExtension: vi.fn(),
  // LargeFileViewer 复用编辑器字体单点（EDITOR_FONT_SPEC）——mock 需含该导出
  EDITOR_FONT_SPEC: { ".cm-scroller": { fontFamily: "monospace" } },
  MAX_FILE_SIZE_BYTES: 10_000_000,
  LARGE_FILE_WARN_BYTES: 1_000_000,
}));

// LargeFileViewer 经 blockCache → ipc/fs.readFileRange 按需读块——mock 防真实 invoke
vi.mock("../ipc", () => ({
  fs: { readFileRange: mockReadFileRange },
}));

import React from "react";
import { render, waitFor } from "@testing-library/react";
import EditorPanel from "../panels/editor/EditorPanel";

afterEach(() => {
  clearMocks();
  mockUseCodeMirror.mockClear();
  mockReadFileRange.mockClear();
});

describe("EditorPanel", () => {
  it("渲染编辑器容器（暗色背景）", () => {
    const { container } = render(
      React.createElement(EditorPanel, { params: { panelId: "editor-1" } }),
    );
    const el = container.querySelector('div[style*="background"]');
    expect(el).toBeTruthy();
  });

  it("将 panelId 传递给 useCodeMirror", () => {
    render(
      React.createElement(EditorPanel, { params: { panelId: "editor-2" } }),
    );
    expect(mockUseCodeMirror).toHaveBeenCalledWith(
      expect.objectContaining({ panelId: "editor-2" }),
    );
  });

  it("将 filePath 正确传递给 useCodeMirror", () => {
    render(
      React.createElement(EditorPanel, {
        params: { panelId: "editor-3", filePath: "C:\\test\\demo.rs" },
      }),
    );
    expect(mockUseCodeMirror).toHaveBeenCalledWith(
      expect.objectContaining({ filePath: "C:\\test\\demo.rs" }),
    );
  });

  it("同时传递 filePath 和 panelId 给 useCodeMirror", () => {
    render(
      React.createElement(EditorPanel, {
        params: { panelId: "editor-4", filePath: "/home/user/main.py" },
      }),
    );
    expect(mockUseCodeMirror).toHaveBeenCalledWith(
      expect.objectContaining({
        panelId: "editor-4",
        filePath: "/home/user/main.py",
      }),
    );
  });

  it("空白编辑器（无 filePath）正确渲染容器", () => {
    const { container } = render(
      React.createElement(EditorPanel, { params: { panelId: "editor-0" } }),
    );
    // 仍渲染容器
    const el = container.querySelector('div[style*="background"]');
    expect(el).toBeTruthy();
    // 传 undefined filePath 给 useCodeMirror
    expect(mockUseCodeMirror).toHaveBeenCalledWith(
      expect.objectContaining({ filePath: undefined, panelId: "editor-0" }),
    );
  });

  // ── 横向滚动条修复：容器 overflow 样式 ──

  it("容器 div 设置 overflow: clip（非 overflow: auto/hidden）", () => {
    const { container } = render(
      React.createElement(EditorPanel, { params: { panelId: "editor-5" } }),
    );
    const el = container.querySelector('div[style*="overflow"]');
    expect(el).toBeTruthy();
    // 内联 style 字符串包含 overflow: clip
    expect(el!.getAttribute("style")).toContain("overflow: clip");
  });

  it("容器 div 不包含 overflow: auto（防回归，确保双滚动上下文已消除）", () => {
    const { container } = render(
      React.createElement(EditorPanel, { params: { panelId: "editor-6" } }),
    );
    const el = container.querySelector('div[style*="overflow"]');
    expect(el).toBeTruthy();
    const style = el!.getAttribute("style")!;
    expect(style).not.toContain("overflow: auto");
    expect(style).not.toContain("overflow: hidden");
  });

  it("容器 div 保留 width: 100% 和 height: 100%", () => {
    const { container } = render(
      React.createElement(EditorPanel, { params: { panelId: "editor-7" } }),
    );
    const el = container.querySelector('div[style*="overflow"]');
    expect(el).toBeTruthy();
    const style = el!.getAttribute("style")!;
    expect(style).toContain("width: 100%");
    expect(style).toContain("height: 100%");
  });

  it("容器 div 保留 background 暗色编辑器背景", () => {
    const { container } = render(
      React.createElement(EditorPanel, { params: { panelId: "editor-8" } }),
    );
    const el = container.querySelector('div[style*="overflow"]');
    expect(el).toBeTruthy();
    expect(el!.getAttribute("style")).toContain("background");
  });

  // ── CP-022: largeFile 信号 → LargeFileViewer 形态切换 ──

  it("largeFile 信号非空时渲染 LargeFileViewer（替代 CM 编辑区）", async () => {
    mockUseCodeMirror.mockReturnValueOnce({
      ...defaultHookResult(),
      largeFile: { filePath: "C:/big/log.txt", sizeBytes: 12_000_000 },
    });
    mockReadFileRange.mockResolvedValue("line1\nline2\n");

    const { container } = render(
      React.createElement(EditorPanel, {
        params: { panelId: "editor-lf", filePath: "C:/big/log.txt" },
      }),
    );

    // 只读浏览锚 + 信息条口径（「只读浏览(文件大小),可编辑上限 10MB」）
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="large-file-viewer"]')).toBeTruthy();
    });
    const infoBar = container.querySelector('[data-e2e="lfv-info-bar"]');
    expect(infoBar?.textContent).toContain("只读浏览");
    expect(infoBar?.textContent).toContain("可编辑上限 10MB");
    // 防复发: 原拒绝文案形态不再出现
    expect(container.textContent).not.toContain("文件过大");
    expect(container.textContent).not.toContain("已拒绝打开以保护内存");
    // 编辑容器 div 不渲染（CM 形态容器 = overflow: clip 的 div）
    expect(container.querySelector('div[style*="overflow: clip"]')).toBeNull();
  });

  it("信号清空（文件切换后 hook 返回 null）→ 形态切回 CM 容器", async () => {
    // 首次渲染返回大文件信号（viewer 形态）;rerender 后信号清空（hook 切文件行为）
    // → EditorPanel 以 null 信号重新渲染 → CM 容器回挂
    const params = { panelId: "editor-lf2", filePath: "C:/big/b.log" };
    mockUseCodeMirror
      .mockReturnValueOnce({
        ...defaultHookResult(),
        largeFile: { filePath: "C:/big/b.log", sizeBytes: 11_000_000 },
      })
      .mockReturnValueOnce(defaultHookResult());

    const { rerender, container } = render(
      React.createElement(EditorPanel, { params }),
    );
    await waitFor(() => {
      expect(container.querySelector('[data-e2e="large-file-viewer"]')).toBeTruthy();
    });

    // 第二次渲染（同 params——真实切换场景由 hook 内部清信号驱动）→ CM 形态回挂
    rerender(React.createElement(EditorPanel, { params }));
    await waitFor(() => {
      expect(container.querySelector('div[style*="overflow: clip"]')).toBeTruthy();
    });
    expect(container.querySelector('[data-e2e="large-file-viewer"]')).toBeNull();
    // CM 容器重挂后 useCodeMirror 应收到非 null container（重新捕获生效）
    const lastCall = mockUseCodeMirror.mock.calls[mockUseCodeMirror.mock.calls.length - 1];
    expect(lastCall?.[0]?.container).toBeInstanceOf(HTMLElement);
  });
});
