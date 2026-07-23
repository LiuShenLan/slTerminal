// reserved.test.ts — 保留键判定单元测试
//
// 覆盖：各 context（terminal/editor/global/未知）逐一断言、
//       每个保留键命中、非保留键放行、global = 两集并集。

import { describe, it, expect } from "vitest";
import { isReserved } from "../features/shortcuts/reserved";
import type { KeyStroke } from "../features/shortcuts/types";

function ks(o: Partial<KeyStroke> & { code: string }): KeyStroke {
  return { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...o };
}

describe("isReserved — terminal", () => {
  it("终端控制字符透传键被保留", () => {
    expect(isReserved(ks({ ctrlKey: true, code: "KeyC" }), "terminal")).toBe(true);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyV" }), "terminal")).toBe(true);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyX" }), "terminal")).toBe(true);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyZ" }), "terminal")).toBe(true);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyA" }), "terminal")).toBe(true);
  });

  it("终端非保留键放行", () => {
    // 终端复制/粘贴（带 Shift）、Ctrl+Enter 均可绑
    expect(isReserved(ks({ ctrlKey: true, shiftKey: true, code: "KeyC" }), "terminal")).toBe(false);
    expect(isReserved(ks({ ctrlKey: true, shiftKey: true, code: "KeyV" }), "terminal")).toBe(false);
    expect(isReserved(ks({ ctrlKey: true, code: "Enter" }), "terminal")).toBe(false);
    // 编辑器保留键在终端 context 不保留
    expect(isReserved(ks({ ctrlKey: true, code: "KeyF" }), "terminal")).toBe(false);
  });
});

describe("isReserved — editor", () => {
  it("CodeMirror 内部键被保留", () => {
    expect(isReserved(ks({ ctrlKey: true, code: "KeyF" }), "editor")).toBe(true);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyZ" }), "editor")).toBe(true);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyY" }), "editor")).toBe(true);
    expect(isReserved(ks({ ctrlKey: true, shiftKey: true, code: "KeyZ" }), "editor")).toBe(true);
    expect(isReserved(ks({ code: "Tab" }), "editor")).toBe(true);
    expect(isReserved(ks({ shiftKey: true, code: "Tab" }), "editor")).toBe(true);
  });

  it("编辑器非保留键放行", () => {
    // 保存可绑；终端保留键在编辑器 context 不保留
    expect(isReserved(ks({ ctrlKey: true, code: "KeyS" }), "editor")).toBe(false);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyC" }), "editor")).toBe(false);
  });
});

describe("isReserved — global (两集并集)", () => {
  it("终端保留键在 global 命中", () => {
    expect(isReserved(ks({ ctrlKey: true, code: "KeyC" }), "global")).toBe(true);
  });

  it("编辑器保留键在 global 命中", () => {
    expect(isReserved(ks({ ctrlKey: true, code: "KeyF" }), "global")).toBe(true);
  });

  it("两集之外的键在 global 放行（如 Ctrl+W、Ctrl+Enter）", () => {
    expect(isReserved(ks({ ctrlKey: true, code: "KeyW" }), "global")).toBe(false);
    expect(isReserved(ks({ ctrlKey: true, code: "Enter" }), "global")).toBe(false);
  });
});

describe("isReserved — explorer context", () => {
  it("Delete/Enter/F2 在 explorer context 中为非保留键", () => {
    expect(isReserved(ks({ code: "Delete" }), "explorer")).toBe(false);
    expect(isReserved(ks({ code: "Enter" }), "explorer")).toBe(false);
    expect(isReserved(ks({ code: "F2" }), "explorer")).toBe(false);
  });
});

describe("isReserved — 未知 context", () => {
  it("未知 context 一律不保留", () => {
    expect(isReserved(ks({ ctrlKey: true, code: "KeyC" }), "htmlviewer")).toBe(false);
    expect(isReserved(ks({ ctrlKey: true, code: "KeyF" }), "foobar")).toBe(false);
  });
});
