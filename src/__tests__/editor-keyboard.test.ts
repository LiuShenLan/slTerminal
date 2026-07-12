// editor-keyboard.test.ts — 编辑器快捷键命令工厂单元测试
//
// 工厂 createEditorShortcuts() 无参，handler 经 getActiveEditor() 派发到聚焦编辑器。
// 覆盖：命令结构、save 派发到 active、无 active 返回 false。

import { describe, it, expect, vi, afterEach } from "vitest";
import { createEditorShortcuts } from "../panels/editor/keyboard";
import {
  setActiveEditor,
  clearActiveEditor,
  type EditorActions,
} from "../panels/editor/activeEditor";

function keyEvent(): KeyboardEvent {
  return new KeyboardEvent("keydown", { ctrlKey: true, code: "KeyS", bubbles: true, cancelable: true });
}

describe("createEditorShortcuts", () => {
  const cmds = createEditorShortcuts();
  let current: EditorActions | null = null;

  function setActive(a: EditorActions) {
    current = a;
    setActiveEditor(a);
  }

  afterEach(() => {
    if (current) clearActiveEditor(current);
    current = null;
  });

  it("返回两条命令，元数据正确", () => {
    expect(cmds).toHaveLength(2);
    const save = cmds[0];
    expect(save.id).toBe("editor.save");
    expect(save.context).toBe("editor");
    expect(save.defaultKey).toEqual({
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyS",
    });

    const toggleWrap = cmds[1];
    expect(toggleWrap.id).toBe("editor.toggleWordWrap");
    expect(toggleWrap.context).toBe("editor");
    expect(toggleWrap.defaultKey).toEqual({
      ctrlKey: false, shiftKey: false, altKey: true, metaKey: false, code: "KeyZ",
    });
  });

  it("有聚焦编辑器 → 调 save 并返回 true", () => {
    const saveSpy = vi.fn();
    setActive({ save: saveSpy, toggleWordWrap: vi.fn() });
    const result = cmds[0].handler(keyEvent());
    expect(saveSpy).toHaveBeenCalledTimes(1);
    expect(result).toBe(true);
  });

  it("派发到当前聚焦编辑器（后设置的覆盖先前的）", () => {
    const saveA = vi.fn();
    const saveB = vi.fn();
    setActive({ save: saveA, toggleWordWrap: vi.fn() });
    setActive({ save: saveB, toggleWordWrap: vi.fn() });
    cmds[0].handler(keyEvent());
    expect(saveA).not.toHaveBeenCalled();
    expect(saveB).toHaveBeenCalledTimes(1);
  });

  it("无聚焦编辑器 → 返回 false，不调 save", () => {
    const result = cmds[0].handler(keyEvent());
    expect(result).toBe(false);
  });

  // ── editor.toggleWordWrap ──

  function altZEvent(): KeyboardEvent {
    return new KeyboardEvent("keydown", { altKey: true, code: "KeyZ", bubbles: true, cancelable: true });
  }

  it("有聚焦编辑器 → 调 toggleWordWrap 并返回 true", () => {
    const toggleSpy = vi.fn();
    setActive({ save: vi.fn(), toggleWordWrap: toggleSpy });
    const result = cmds[1].handler(altZEvent());
    expect(toggleSpy).toHaveBeenCalledTimes(1);
    expect(result).toBe(true);
  });

  it("toggleWordWrap 派发到当前聚焦编辑器（后设置的覆盖先前的）", () => {
    const toggleA = vi.fn();
    const toggleB = vi.fn();
    setActive({ save: vi.fn(), toggleWordWrap: toggleA });
    setActive({ save: vi.fn(), toggleWordWrap: toggleB });
    cmds[1].handler(altZEvent());
    expect(toggleA).not.toHaveBeenCalled();
    expect(toggleB).toHaveBeenCalledTimes(1);
  });

  it("无聚焦编辑器时 toggleWordWrap → 返回 false", () => {
    const result = cmds[1].handler(altZEvent());
    expect(result).toBe(false);
  });
});
