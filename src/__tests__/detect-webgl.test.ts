// detect-webgl.test.ts — detectWebgl / isSwiftShaderRenderer 检测纯函数测试
//
// detectWebgl 决定终端使用 WebGL 渲染器还是 DOM 兜底渲染器。
// isSwiftShaderRenderer（CP-018）判定软件渲染是否生效（toast 降级提示依据）。
// 纯函数：无副作用（仅创建临时 canvas），可控的返回分支。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { detectWebgl, resetWebglCache } from "../panels/terminal/useXterm";
import {
  isSwiftShaderRenderer,
  resetSwiftShaderCache,
} from "../panels/terminal/webgl";

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

describe("isSwiftShaderRenderer", () => {
  // CP-018:判定结果模块级缓存——每例前重置保证用例隔离
  beforeEach(() => {
    resetSwiftShaderCache();
  });

  /** 构造 WebGL2 context 桩；renderer 为 null 表示 WEBGL_debug_renderer_info 扩展缺失 */
  function makeGl(renderer: string | null): RenderingContext {
    const gl = {
      getExtension: () =>
        renderer === null ? null : { UNMASKED_RENDERER_WEBGL: 0x9246 },
      getParameter: () => renderer,
    };
    return gl as unknown as RenderingContext;
  }

  it("1. SwiftShader renderer 串 → 判定软件渲染 true", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue(
        makeGl("ANGLE (Intel, SwiftShader Device (D3D11 via ANGLE)"),
      );
    try {
      expect(isSwiftShaderRenderer()).toBe(true);
    } finally {
      spy.mockRestore();
    }
  });

  it("2. Intel 硬件 renderer 串 → false", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue(
        makeGl(
          "ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)",
        ),
      );
    try {
      expect(isSwiftShaderRenderer()).toBe(false);
    } finally {
      spy.mockRestore();
    }
  });

  it("3. 扩展缺失（getExtension 返回 null）→ false（保守不提示，避免误报）", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue(makeGl(null));
    try {
      expect(isSwiftShaderRenderer()).toBe(false);
    } finally {
      spy.mockRestore();
    }
  });

  it("4. 缓存复用：两次调用只检测一次（getContext 仅一次）", () => {
    const spy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue(
        makeGl("ANGLE (Intel, SwiftShader Device (D3D11 via ANGLE)"),
      );
    try {
      expect(isSwiftShaderRenderer()).toBe(true);
      expect(isSwiftShaderRenderer()).toBe(true);
      expect(spy.mock.calls).toHaveLength(1); // 命中模块级缓存，不重复建 canvas 检测
    } finally {
      spy.mockRestore();
    }
  });
});
