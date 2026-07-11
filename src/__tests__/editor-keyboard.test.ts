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

  it("返回一条 editor.save 命令，元数据正确", () => {
    expect(cmds).toHaveLength(1);
    const save = cmds[0];
    expect(save.id).toBe("editor.save");
    expect(save.context).toBe("editor");
    expect(save.defaultKey).toEqual({
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyS",
    });
  });

  it("有聚焦编辑器 → 调 save 并返回 true", () => {
    const saveSpy = vi.fn();
    setActive({ save: saveSpy });
    const result = cmds[0].handler(keyEvent());
    expect(saveSpy).toHaveBeenCalledTimes(1);
    expect(result).toBe(true);
  });

  it("派发到当前聚焦编辑器（后设置的覆盖先前的）", () => {
    const saveA = vi.fn();
    const saveB = vi.fn();
    setActive({ save: saveA });
    setActive({ save: saveB });
    cmds[0].handler(keyEvent());
    expect(saveA).not.toHaveBeenCalled();
    expect(saveB).toHaveBeenCalledTimes(1);
  });

  it("无聚焦编辑器 → 返回 false，不调 save", () => {
    const result = cmds[0].handler(keyEvent());
    expect(result).toBe(false);
  });
});
