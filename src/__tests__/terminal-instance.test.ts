// terminal-instance.test.ts — useTerminalInstance 生命周期分支测试
//
// 覆盖 checklist TRM-07 四分支：
// - document.fonts.ready 后 fit 抛异常 → catch 吞掉（不影响渲染）
// - fontSize undefined → 字体 effect 直接返回（不写 options.fontSize）
// - prevFontSize 相同 → 跳过重复写入（仅变化时写）
// - tryLoadWebgl → webglAddon 已存在不重复加载（含 term 为 null 短路）
//
// mock 策略照 terminal-lifecycle.test.ts：hoisted 共享 mock 实例 + 模块级 vi.mock。

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import type { WebglAddon } from "@xterm/addon-webgl";
import { useTerminalInstance } from "../panels/terminal/useTerminalInstance";

// ─── Hoisted mock 实例（vi.mock 工厂通过闭包引用） ───
const mocks = vi.hoisted(() => {
  const terminal = {
    open: vi.fn(),
    dispose: vi.fn(),
    loadAddon: vi.fn(),
    element: document.createElement("div"),
    options: {} as Record<string, unknown>,
  };
  const fitAddon = { fit: vi.fn(), dispose: vi.fn() };
  const webglAddon = { onContextLoss: vi.fn(), dispose: vi.fn() };
  const webgl = {
    detectWebgl: vi.fn(),
    setupWebglWithRetry: vi.fn(),
  };
  return {
    Terminal: vi.fn(function () {
      return terminal;
    }),
    terminal,
    fitAddon,
    webglAddon,
    webgl,
  };
});

vi.mock("@xterm/xterm", () => ({
  Terminal: mocks.Terminal,
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn(function () {
    return mocks.fitAddon;
  }),
}));

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: vi.fn(function () {
    return mocks.webglAddon;
  }),
}));

// useTerminalInstance 内部依赖 webgl.ts 的 detectWebgl/setupWebglWithRetry——
// mock 后 onSuccess 回调由测试手动触发（模拟加载完成），控制 tryLoadWebgl 短路分支
vi.mock("../panels/terminal/webgl", () => ({
  detectWebgl: mocks.webgl.detectWebgl,
  setupWebglWithRetry: mocks.webgl.setupWebglWithRetry,
}));

// ─── 测试辅助 ───

/** 最近一次 setupWebglWithRetry 收到的 onSuccess 回调（挂载时主 effect 注册） */
let lastOnSuccess: ((addon: WebglAddon) => void) | null = null;
/** setupWebglWithRetry 返回的 cancel 函数 */
let cancelFn: ReturnType<typeof vi.fn>;

function containerStub(): HTMLElement {
  const el = document.createElement("div");
  Object.defineProperty(el, "offsetWidth", { value: 800, writable: true, configurable: true });
  Object.defineProperty(el, "offsetHeight", { value: 600, writable: true, configurable: true });
  return el;
}

describe("useTerminalInstance 分支覆盖（TRM-07）", () => {
  beforeEach(() => {
    cancelFn = vi.fn();
    mocks.webgl.detectWebgl.mockClear().mockReturnValue(true);
    mocks.webgl.setupWebglWithRetry.mockClear();
    mocks.webgl.setupWebglWithRetry.mockImplementation((_term, onSuccess) => {
      lastOnSuccess = onSuccess;
      return { cancel: cancelFn };
    });
  });

  it("1. document.fonts.ready 后 fit 抛异常 → 被 catch 吞掉（不影响渲染）", async () => {
    mocks.fitAddon.fit.mockImplementation(() => {
      throw new Error("fit failed");
    });
    const container = containerStub();
    const { result } = renderHook(() => useTerminalInstance(container, {}));

    // fonts.ready.then → rAF → fit() 抛错 → try/catch 吞掉，不冒泡不中断
    await waitFor(() => expect(mocks.fitAddon.fit).toHaveBeenCalled());
    expect(mocks.terminal.dispose).not.toHaveBeenCalled();
    expect(result.current.isReady.current).toBe(true);
  });

  it("2. fontSize undefined → 字体 effect 直接返回（不写 options.fontSize）", () => {
    const container = containerStub();
    const setter = vi.fn();
    Object.defineProperty(mocks.terminal.options, "fontSize", {
      configurable: true,
      set: setter,
    });
    renderHook(() => useTerminalInstance(container, {}, undefined));

    expect(setter).not.toHaveBeenCalled();
    // 构造参数回退默认 14（DEFAULT_FONT_SIZE）
    expect(mocks.Terminal).toHaveBeenCalledWith(
      expect.objectContaining({ fontSize: 14 }),
    );
  });

  it("3. prevFontSize 相同 → 跳过重复写入；仅值变化时写入", () => {
    const container = containerStub();
    const setter = vi.fn();
    Object.defineProperty(mocks.terminal.options, "fontSize", {
      configurable: true,
      set: setter,
    });
    const { rerender } = renderHook(
      ({ fontSize }: { fontSize?: number }) => useTerminalInstance(container, {}, fontSize),
      { initialProps: { fontSize: 14 } },
    );
    expect(setter).toHaveBeenCalledTimes(1); // 首次 14 写入

    rerender({ fontSize: 14 }); // 相同值 → 跳过
    expect(setter).toHaveBeenCalledTimes(1);

    rerender({ fontSize: 16 }); // 新值 → 写入
    expect(setter).toHaveBeenCalledTimes(2);

    rerender({ fontSize: 16 }); // 相同值 → 跳过
    expect(setter).toHaveBeenCalledTimes(2);
  });

  it("4. tryLoadWebgl：webglAddon 已存在 → 不重复加载；term 为 null → 同样短路", () => {
    const container = containerStub();
    const { result } = renderHook(() => useTerminalInstance(container, {}));
    // 挂载时主 effect 已调用一次 setupWebglWithRetry
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(1);

    // 模拟首次加载成功：ref 被 onSuccess 设置
    act(() => {
      lastOnSuccess!(mocks.webglAddon as unknown as WebglAddon);
    });

    // 已加载 → tryLoadWebgl 短路（不重复 setupWebglWithRetry）
    act(() => {
      result.current.tryLoadWebgl();
    });
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(1);

    // dispose 后 term 为 null → 同样短路
    act(() => {
      result.current.dispose();
    });
    act(() => {
      result.current.tryLoadWebgl();
    });
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(1);
  });

  it("5. tryLoadWebgl：未加载且 WebGL 可用 → 加载（对照正向分支）", () => {
    const container = containerStub();
    const { result } = renderHook(() => useTerminalInstance(container, {}));
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(1);

    // 挂载后 onSuccess 未触发 → ref 为 null → tryLoadWebgl 发起新加载
    act(() => {
      result.current.tryLoadWebgl();
    });
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(2);
  });

  it("6. tryLoadWebgl：WebGL 不可用 → 短路（detectWebgl false）", () => {
    mocks.webgl.detectWebgl.mockReturnValue(false);
    const container = containerStub();
    const { result } = renderHook(() => useTerminalInstance(container, {}));
    // 挂载时主 effect 调用一次（setupWebglWithRetry 内部自行判定 detectWebgl，
    // mock 不判定——故调用计数含挂载 1 次）
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.tryLoadWebgl();
    });
    // detectWebgl false → tryLoadWebgl 短路，不新增加载
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(1);
  });

  it("7. setupWebglWithRetry 在 term.open 之后调用（win10 黑屏 FE-34 时序回归守卫）", () => {
    // FE-34 曾把 WebGL 加载挪到 term.open() 之前：WebglAddon 在 canvas 创建前
    // loadAddon → 渲染器绑定空 canvas → 静默黑渲染、不触发 context loss 兜底
    // （win10 终端纯黑屏根因）。修复 = 先 open 再 setupWebglWithRetry，
    // 本用例以 Vitest 调用序锁死该不变量，防止未来再调回错误时序。
    const container = containerStub();
    renderHook(() => useTerminalInstance(container, {}));

    const openOrder = mocks.terminal.open.mock.invocationCallOrder[0];
    const setupOrder = mocks.webgl.setupWebglWithRetry.mock.invocationCallOrder[0];
    expect(openOrder).toBeDefined();
    expect(setupOrder).toBeDefined();
    expect(openOrder).toBeLessThan(setupOrder);
  });
});
