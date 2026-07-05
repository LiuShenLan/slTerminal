// editor-font.test.ts — 编辑器字体主题自动化测试
//
// 测试策略：EditorView 在 jsdom 不可用（见 editor.test.tsx），因此
// - 通过 EditorState.create() 验证扩展有效性
// - 通过 EDITOR_FONT_SPEC 常量直接断言 CSS 选择器和字体值
//   这是能捕获 ".cm-editor vs .cm-scroller" 选择器错误的关键

import { describe, it, expect } from "vitest";
import { EditorState } from "@codemirror/state";
import { EDITOR_FONT_SPEC, EDITOR_FONT_THEME } from "../panels/editor/useCodeMirror";

describe("编辑器字体主题", () => {
  it("1. EDITOR_FONT_THEME 是合法的 EditorState 扩展", () => {
    const state = EditorState.create({
      doc: "",
      extensions: [EDITOR_FONT_THEME],
    });
    expect(state).toBeDefined();
  });

  it("2. 字体设为 JetBrains Mono", () => {
    expect(EDITOR_FONT_SPEC[".cm-scroller"].fontFamily).toBe(
      '"JetBrains Mono", monospace',
    );
  });

  it("3. 使用 .cm-scroller 选择器（非 .cm-editor）", () => {
    // .cm-editor 会被基主题的 .cm-scroller { font-family: "monospace" } 覆盖
    expect(EDITOR_FONT_SPEC).toHaveProperty(".cm-scroller");
    expect(EDITOR_FONT_SPEC).not.toHaveProperty(".cm-editor");
  });

  it("4. .cm-content 选择器不应被设置（冗余）", () => {
    // font-family 是继承属性，设在 .cm-scroller 即可
    expect(EDITOR_FONT_SPEC).not.toHaveProperty(".cm-content");
  });
});
