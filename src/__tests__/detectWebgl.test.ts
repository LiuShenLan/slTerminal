// detectWebgl.test.ts — detectWebgl 纯函数测试
//
// detectWebgl 决定终端使用 WebGL 渲染器还是 DOM 兜底渲染器。
// 纯函数：无副作用（仅创建临时 canvas），可控的返回分支。

import { describe, it, expect, vi } from "vitest";
import { detectWebgl } from "../panels/terminal/useXterm";

describe("detectWebgl", () => {
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
});
