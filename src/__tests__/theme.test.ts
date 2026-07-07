// theme.test.ts — terminalOptions 配色与配置项验证
//
// 配色单点（硬约束 #6）：所有终端颜色在 theme/colors 中定义，此处验证合法性。

import { describe, it, expect } from "vitest";
import { terminalOptions } from "../panels/terminal/theme";

const HEX_COLOR = /^#[0-9A-Fa-f]{6}$/;

describe("terminalOptions 配色验证", () => {
  it("1. theme 对象存在", () => {
    expect(terminalOptions.theme).toBeDefined();
  });

  it("2. 前景色 + 背景色为合法 hex", () => {
    const t = terminalOptions.theme!;
    expect(t.foreground).toMatch(HEX_COLOR);
    expect(t.background).toMatch(HEX_COLOR);
  });

  it("3. 光标色 + 光标强调色为合法 hex", () => {
    const t = terminalOptions.theme!;
    expect(t.cursor).toMatch(HEX_COLOR);
    expect(t.cursorAccent).toMatch(HEX_COLOR);
  });

  it("4. 选区色为合法 hex", () => {
    const t = terminalOptions.theme!;
    expect(t.selectionBackground).toMatch(HEX_COLOR);
    expect(t.selectionForeground).toMatch(HEX_COLOR);
  });

  it("5. 16 个 ANSI 标准色为合法 hex", () => {
    const t = terminalOptions.theme!;
    const ansi = [
      t.black, t.red, t.green, t.yellow,
      t.blue, t.magenta, t.cyan, t.white,
      t.brightBlack, t.brightRed, t.brightGreen, t.brightYellow,
      t.brightBlue, t.brightMagenta, t.brightCyan, t.brightWhite,
    ];
    for (const color of ansi) {
      expect(color).toMatch(HEX_COLOR);
    }
  });

  it("6. fontSize 为合法正数", () => {
    expect(terminalOptions.fontSize).toBeGreaterThan(0);
    expect(Number.isInteger(terminalOptions.fontSize)).toBe(true);
  });

  it("7. fontFamily 为 JetBrains Mono", () => {
    expect(terminalOptions.fontFamily).toContain("JetBrains Mono");
  });

  it("8. cursorBlink 为 true", () => {
    expect(terminalOptions.cursorBlink).toBe(true);
  });

  it("9. cursorStyle 为 bar", () => {
    expect(terminalOptions.cursorStyle).toBe("bar");
  });

  it("10. scrollback 为合法正数", () => {
    expect(terminalOptions.scrollback).toBeGreaterThan(0);
  });

  it("11. allowProposedApi 为 true", () => {
    expect(terminalOptions.allowProposedApi).toBe(true);
  });

  it("12. drawBoldTextInBrightColors 显式为 true", () => {
    expect(terminalOptions.drawBoldTextInBrightColors).toBe(true);
  });
});
