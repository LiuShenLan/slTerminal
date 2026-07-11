// keystroke.test.ts — keystroke 字符串互转单元测试
//
// 覆盖：formatKeystroke（各修饰键组合 + 固定序规范化）、
//       parseKeystroke（全合法往返 + 各类非法 → null）、
//       isValidKeystrokeString、format∘parse 恒等。

import { describe, it, expect } from "vitest";
import { formatKeystroke, parseKeystroke, isValidKeystrokeString } from "../features/shortcuts/keystroke";
import type { KeyStroke } from "../features/shortcuts/types";

function ks(o: Partial<KeyStroke> & { code: string }): KeyStroke {
  return { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...o };
}

describe("formatKeystroke", () => {
  it("无修饰键直接返回 code", () => {
    expect(formatKeystroke(ks({ code: "F1" }))).toBe("F1");
    expect(formatKeystroke(ks({ code: "KeyA" }))).toBe("KeyA");
  });

  it("单修饰键", () => {
    expect(formatKeystroke(ks({ ctrlKey: true, code: "KeyS" }))).toBe("Ctrl+KeyS");
    expect(formatKeystroke(ks({ shiftKey: true, code: "Tab" }))).toBe("Shift+Tab");
    expect(formatKeystroke(ks({ altKey: true, code: "Enter" }))).toBe("Alt+Enter");
    expect(formatKeystroke(ks({ metaKey: true, code: "KeyK" }))).toBe("Meta+KeyK");
  });

  it("多修饰键按固定序 Ctrl→Shift→Alt→Meta 输出", () => {
    expect(formatKeystroke(ks({ ctrlKey: true, shiftKey: true, code: "KeyC" }))).toBe("Ctrl+Shift+KeyC");
    expect(
      formatKeystroke(ks({ ctrlKey: true, shiftKey: true, altKey: true, metaKey: true, code: "KeyX" })),
    ).toBe("Ctrl+Shift+Alt+Meta+KeyX");
  });

  it("修饰键内部布尔顺序不影响输出（始终规范序）", () => {
    // 无论字段设置顺序，输出恒为 Ctrl+Alt
    expect(formatKeystroke(ks({ altKey: true, ctrlKey: true, code: "KeyD" }))).toBe("Ctrl+Alt+KeyD");
  });
});

describe("parseKeystroke", () => {
  it("无修饰键往返", () => {
    expect(parseKeystroke("F5")).toEqual(ks({ code: "F5" }));
    expect(parseKeystroke("KeyA")).toEqual(ks({ code: "KeyA" }));
  });

  it("单/多修饰键往返", () => {
    expect(parseKeystroke("Ctrl+KeyS")).toEqual(ks({ ctrlKey: true, code: "KeyS" }));
    expect(parseKeystroke("Ctrl+Shift+KeyC")).toEqual(ks({ ctrlKey: true, shiftKey: true, code: "KeyC" }));
    expect(parseKeystroke("Ctrl+Shift+Alt+Meta+KeyX")).toEqual(
      ks({ ctrlKey: true, shiftKey: true, altKey: true, metaKey: true, code: "KeyX" }),
    );
  });

  it("空串 → null", () => {
    expect(parseKeystroke("")).toBeNull();
  });

  it("尾随 + → null", () => {
    expect(parseKeystroke("Ctrl+")).toBeNull();
    expect(parseKeystroke("Ctrl+Shift+")).toBeNull();
  });

  it("前导 + → null", () => {
    expect(parseKeystroke("+KeyA")).toBeNull();
  });

  it("未知修饰键 → null", () => {
    expect(parseKeystroke("Super+KeyA")).toBeNull();
    expect(parseKeystroke("Cmd+KeyA")).toBeNull();
  });

  it("重复修饰键 → null", () => {
    expect(parseKeystroke("Ctrl+Ctrl+KeyA")).toBeNull();
  });

  it("大小写不符 → null（要求精确 Ctrl/Shift/Alt/Meta）", () => {
    expect(parseKeystroke("ctrl+KeyA")).toBeNull();
    expect(parseKeystroke("CTRL+KeyA")).toBeNull();
  });
});

describe("isValidKeystrokeString", () => {
  it("合法串返回 true", () => {
    expect(isValidKeystrokeString("Ctrl+Shift+KeyC")).toBe(true);
    expect(isValidKeystrokeString("F1")).toBe(true);
  });

  it("非法串返回 false", () => {
    expect(isValidKeystrokeString("")).toBe(false);
    expect(isValidKeystrokeString("Ctrl+")).toBe(false);
    expect(isValidKeystrokeString("foo+bar")).toBe(false);
  });
});

describe("format ∘ parse 恒等", () => {
  const cases = ["KeyA", "Ctrl+KeyS", "Ctrl+Shift+KeyC", "Shift+Tab", "Ctrl+Shift+Alt+Meta+KeyX", "Enter", "Ctrl+Enter"];
  for (const s of cases) {
    it(`"${s}" 往返一致`, () => {
      const parsed = parseKeystroke(s);
      expect(parsed).not.toBeNull();
      expect(formatKeystroke(parsed!)).toBe(s);
    });
  }
});
