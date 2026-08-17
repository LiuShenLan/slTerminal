// detectWebgl.test.ts — detectWebgl 纯函数测试
//
// detectWebgl 决定终端使用 WebGL 渲染器还是 DOM 兜底渲染器。
// 纯函数：无副作用（仅创建临时 canvas），可控的返回分支。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { detectWebgl, resetWebglCache } from "../panels/terminal/useXterm";

describe("detectWebgl", () => {
  // P2-44: detectWebgl 使用模块级缓存，每个测试前需重置
  beforeEach(() => {
    resetWebglCache();
  });
  it("1. WebGL2 可用 → 返回 true", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue({} as unknown as RenderingContext);
    try {
      expect(detectWebgl()).toBe(true);
    } finally {
      spy.mockRestore();
    }
  });

  it("2. WebGL2 不可用（getContext 返回 null）→ 返回 false", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue(null);
    try {
      expect(detectWebgl()).toBe(false);
    } finally {
      spy.mockRestore();
    }
  });

  it("3. getContext 抛异常 → 返回 false", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockImplementation(() => {
        throw new Error("WebGL not supported");
      });
    try {
      expect(detectWebgl()).toBe(false);
    } finally {
      spy.mockRestore();
    }
  });

  it("4. 检测不带 failIfMajorPerformanceCaveat（FE-26：blocklist 下拒软件渲染 → DOM 回退 → 快滚掉帧）", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue({} as unknown as RenderingContext);
    try {
      detectWebgl();
      const call = spy.mock.calls[0];
      expect(call[0]).toBe("webgl2");
      // 无第二参数或第二参数不含 failIfMajorPerformanceCaveat
      const opts = call[1] as Record<string, unknown> | undefined;
      expect(opts?.failIfMajorPerformanceCaveat).toBeUndefined();
    } finally {
      spy.mockRestore();
    }
  });
});
