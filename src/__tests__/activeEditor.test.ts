// activeEditor.test.ts — 聚焦编辑器指针单元测试
//
// 覆盖：默认 null、set/get、覆盖、clear 仅在匹配时生效（防竞态）。

import { describe, it, expect, afterEach, vi } from "vitest";
import {
  setActiveEditor,
  clearActiveEditor,
  getActiveEditor,
  type EditorActions,
} from "../panels/editor/activeEditor";

function make(): EditorActions {
  return { save: vi.fn(), toggleWordWrap: vi.fn() };
}

afterEach(() => {
  const a = getActiveEditor();
  if (a) clearActiveEditor(a);
});

describe("activeEditor", () => {
  it("默认无聚焦编辑器", () => {
    expect(getActiveEditor()).toBeNull();
  });

  it("set 后 get 返回同实例", () => {
    const a = make();
    setActiveEditor(a);
    expect(getActiveEditor()).toBe(a);
  });

  it("set 覆盖前一个", () => {
    const a = make();
    const b = make();
    setActiveEditor(a);
    setActiveEditor(b);
    expect(getActiveEditor()).toBe(b);
  });

  it("toggleWordWrap 可被调用", () => {
    const a = make();
    setActiveEditor(a);
    getActiveEditor()!.toggleWordWrap();
    expect(a.toggleWordWrap).toHaveBeenCalledTimes(1);
  });

  it("clear 仅在匹配时生效（防竞态：A blur 不清 B）", () => {
    const a = make();
    const b = make();
    setActiveEditor(b);
    clearActiveEditor(a);
    expect(getActiveEditor()).toBe(b);
    clearActiveEditor(b);
    expect(getActiveEditor()).toBeNull();
  });
});
