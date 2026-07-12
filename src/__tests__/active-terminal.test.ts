// activeTerminal.test.ts — 聚焦终端指针单元测试
//
// 覆盖：默认 null、set/get、覆盖、clear 仅在匹配时生效（防竞态）。

import { describe, it, expect, afterEach, vi } from "vitest";
import {
  setActiveTerminal,
  clearActiveTerminal,
  getActiveTerminal,
  type TerminalActions,
} from "../panels/terminal/activeTerminal";

function make(): TerminalActions {
  return { getSelection: () => "", paste: vi.fn(), writeToPty: vi.fn() };
}

afterEach(() => {
  const a = getActiveTerminal();
  if (a) clearActiveTerminal(a);
});

describe("activeTerminal", () => {
  it("默认无聚焦终端", () => {
    expect(getActiveTerminal()).toBeNull();
  });

  it("set 后 get 返回同实例", () => {
    const a = make();
    setActiveTerminal(a);
    expect(getActiveTerminal()).toBe(a);
  });

  it("set 覆盖前一个", () => {
    const a = make();
    const b = make();
    setActiveTerminal(a);
    setActiveTerminal(b);
    expect(getActiveTerminal()).toBe(b);
  });

  it("clear 仅在匹配时生效（防竞态：A blur 不清 B）", () => {
    const a = make();
    const b = make();
    setActiveTerminal(b);
    clearActiveTerminal(a); // a 非当前 active → 不清
    expect(getActiveTerminal()).toBe(b);
    clearActiveTerminal(b); // 匹配 → 清
    expect(getActiveTerminal()).toBeNull();
  });
});
