// L2 编辑器面板测试——保留 useCodeMirror mock（CodeMirror 6 在 jsdom 不可用），验证正确的参数传递
// CodeMirror 6 在 jsdom 中不可用，mock 是必需的；但测试应验证正确的参数传递
import { describe, it, expect, afterEach, vi } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";

const { mockUseCodeMirror } = vi.hoisted(() => ({
  mockUseCodeMirror: vi.fn(),
}));

vi.mock("../panels/editor/useCodeMirror", () => ({
  useCodeMirror: mockUseCodeMirror,
  getLanguageExtension: vi.fn(),
}));

import React from "react";
import { render } from "@testing-library/react";
import EditorPanel from "../panels/editor/EditorPanel";

afterEach(() => {
  clearMocks();
  mockUseCodeMirror.mockClear();
});

describe("EditorPanel", () => {
  it("渲染编辑器容器（暗色背景 #282C34）", () => {
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
});
