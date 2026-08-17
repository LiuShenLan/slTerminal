// L2 clampWindowsBuildForXterm 纯函数边界测试（ADR-0004）
//
// xterm.js 以 buildNumber < 21376 分叉 ConPTY 兼容行为（CoreTerminal.ts:283）：
// 旧分支启用 wrapping 启发式，claude 全屏高频重绘下误判 isWrapped 致 buffer 错乱。
// 钳制使 Win10 走「新 ConPTY」分支，与 Win11 行为对齐。
import { describe, it, expect } from "vitest";
import { clampWindowsBuildForXterm, XTERM_CONPTY_MIN_BUILD } from "../panels/terminal/useXterm";

describe("clampWindowsBuildForXterm", () => {
  it("Win10 19045 钳制至下界 21376", () => {
    expect(clampWindowsBuildForXterm(19045)).toBe(21376);
  });

  it("Win11 26100 不变（钳制后与现状一致，零回归）", () => {
    expect(clampWindowsBuildForXterm(26100)).toBe(26100);
  });

  it("边界值 21376 不变", () => {
    expect(clampWindowsBuildForXterm(21376)).toBe(21376);
  });

  it("0 与异常小值钳制至下界", () => {
    expect(clampWindowsBuildForXterm(0)).toBe(XTERM_CONPTY_MIN_BUILD);
  });
});
