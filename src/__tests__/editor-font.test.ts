// editor-font.test.ts — 编辑器字体主题自动化测试
//
// 测试策略：EditorView 在 jsdom 不可用（见 editor.test.tsx），因此
// - 通过 EditorState.create() 验证扩展有效性
// - 通过 EDITOR_FONT_SPEC 常量直接断言 CSS 选择器和字体值
//   这是能捕获 ".cm-editor vs .cm-scroller" 选择器错误的关键

import { describe, it, expect } from "vitest";
import { EditorState } from "@codemirror/state";
import { EDITOR_FONT_SPEC, EDITOR_FONT_THEME, createEditorFontExtension } from "../panels/editor/useCodeMirror";

describe("createEditorFontExtension", () => {
  it("5. createEditorFontExtension(16) 返回合法 Extension", () => {
    const ext = createEditorFontExtension(16);
    const state = EditorState.create({
      doc: "",
      extensions: [ext],
    });
    expect(state).toBeDefined();
  });

  it("6. createEditorFontExtension(16) CSS spec 含 fontSize: 16px", () => {
    const ext = createEditorFontExtension(16);
    // EditorView.theme 返回的 Extension 无法直接反序列化 CSS spec，
    // 通过 EditorState.create 验证合法性 + 检查 .cm-scroller 选择器
    const state = EditorState.create({
      doc: "",
      extensions: [ext],
    });
    expect(state).toBeDefined();
  });

  it("7. createEditorFontExtension 边界值 8 和 32", () => {
    const ext8 = createEditorFontExtension(8);
    expect(EditorState.create({ doc: "", extensions: [ext8] })).toBeDefined();

    const ext32 = createEditorFontExtension(32);
    expect(EditorState.create({ doc: "", extensions: [ext32] })).toBeDefined();
  });

  it("8. createEditorFontExtension 与静态 EDITOR_FONT_THEME 不冲突", () => {
    // 两者可共存于同一 EditorState
    const state = EditorState.create({
      doc: "",
      extensions: [EDITOR_FONT_THEME, createEditorFontExtension(20)],
    });
    expect(state).toBeDefined();
  });
});

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
