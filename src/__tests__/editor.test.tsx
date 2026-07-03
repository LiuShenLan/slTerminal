// Phase 1 L2 编辑器面板测试（v2——保留 useCodeMirror mock，改善断言质量）
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
});
