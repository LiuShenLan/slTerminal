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
    // 类型放宽为 HTMLElement | null——用例 11 需模拟 element 未挂载（置 null）；
    // 字面量推断会收窄为 HTMLDivElement，赋 null 报 TS2322
    element: document.createElement("div") as HTMLElement | null,
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
/** 最近一次 setupWebglWithRetry 收到的 onFail 回调（重试耗尽/不可用回退时触发） */
let lastOnFail: (() => void) | null = null;
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
    mocks.webgl.setupWebglWithRetry.mockImplementation((_term, onSuccess, onFail) => {
      lastOnSuccess = onSuccess;
      lastOnFail = onFail;
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

  it("8. onFail 回调接线——重试耗尽/不可用回退 → webglAddonRef 置 null，tryLoadWebgl 可重新发起加载", () => {
    // TQ-COV-07：WebGL 重试耗尽时 webgl.ts 调 onFail，本用例验证 useTerminalInstance
    // 的 onFail 接线（ref 置 null = 回退 DOM 渲染器），并验证回退后可见性恢复路径
    // tryLoadWebgl 能重新发起加载（ref 已空不短路）。
    const container = containerStub();
    const { result } = renderHook(() => useTerminalInstance(container, {}));
    // 挂载时主 effect 已调用一次 setupWebglWithRetry
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(1);

    // 主 effect 注册的 onFail 触发（重试耗尽）→ ref 置 null
    act(() => {
      lastOnFail!();
    });
    expect(result.current.webglAddon.current).toBeNull();

    // ref 已空 → tryLoadWebgl 重新发起加载（回退后可见性恢复再试 WebGL）
    act(() => {
      result.current.tryLoadWebgl();
    });
    expect(mocks.webgl.setupWebglWithRetry).toHaveBeenCalledTimes(2);

    // tryLoadWebgl 注册的 onSuccess 触发 → ref 恢复（覆盖 tryLoadWebgl 的 onSuccess 箭头）
    act(() => {
      lastOnSuccess!(mocks.webglAddon as unknown as WebglAddon);
    });
    expect(result.current.webglAddon.current).toBe(mocks.webglAddon);

    // tryLoadWebgl 注册的 onFail 触发 → ref 再次置 null（覆盖 tryLoadWebgl 的 onFail 箭头）
    act(() => {
      lastOnFail!();
    });
    expect(result.current.webglAddon.current).toBeNull();
  });

  it("9. dispose 幂等——二次调用经 isDisposedRef 提前返回（dispose/cancel 仅执行一次）", () => {
    // performDispose 入口守卫（isDisposedRef.current → return）：二次 dispose 不再重复
    // 清理——Terminal.dispose / WebGL cancel 只应各执行一次
    // （terminal.dispose 为跨用例共享 mock——先清计数再断言）
    mocks.terminal.dispose.mockClear();
    const container = containerStub();
    const { result } = renderHook(() => useTerminalInstance(container, {}));

    act(() => {
      result.current.dispose();
    });
    act(() => {
      result.current.dispose();
    });

    expect(mocks.terminal.dispose).toHaveBeenCalledTimes(1);
    expect(cancelFn).toHaveBeenCalledTimes(1);
    expect(result.current.isDisposed.current).toBe(true);
    expect(result.current.isReady.current).toBe(false);
  });

  it("10. dispose 后 fonts.ready rAF 回调触发 → 取消守卫提前返回（fit 不再执行）", async () => {
    // fonts.ready.then 回调内双重检查 fontsReadyCancelledRef/isDisposedRef：
    // 已销毁组件不得再 fit（防卸载后对已 dispose 的 addon 操作）。
    // 手动接管 rAF：捕获回调后手动触发，不依赖真实定时器（规避跨用例积压的
    // 真实 rAF 定时器在 await 期间误触发共享 fit mock 计数）
    const origRAF = globalThis.requestAnimationFrame;
    const origCAF = globalThis.cancelAnimationFrame;
    let rafCallback: FrameRequestCallback | null = null;
    globalThis.requestAnimationFrame = vi.fn(
      (cb: FrameRequestCallback) => {
        rafCallback = cb;
        return 1;
      },
    ) as unknown as typeof requestAnimationFrame;
    globalThis.cancelAnimationFrame = vi.fn();

    try {
      const container = containerStub();
      const { result } = renderHook(() => useTerminalInstance(container, {}));
      // 排空微任务让 fonts.ready.then 执行（其内 requestAnimationFrame 被捕获入队列）
      await act(async () => {});
      expect(rafCallback).not.toBeNull();

      // 同步 dispose 置取消标志（fontsReadyCancelledRef + isDisposedRef）
      act(() => {
        result.current.dispose();
      });
      mocks.fitAddon.fit.mockClear();

      // 手动触发 fonts.ready 的 rAF 回调：取消守卫提前返回，fit 不被调用
      act(() => {
        rafCallback!(performance.now());
      });
      expect(mocks.fitAddon.fit).not.toHaveBeenCalled();
      expect(mocks.terminal.dispose).toHaveBeenCalled();
    } finally {
      globalThis.requestAnimationFrame = origRAF;
      globalThis.cancelAnimationFrame = origCAF;
    }
  });

  it("11. term.element 为空 → fonts.ready rAF 回调跳过 fit（!term.element 守卫）", async () => {
    // 与用例 10 同模式接管 rAF：手动触发回调；term.element 为 null 时
    // 回调在 line 161 提前返回（element 未挂载不 fit）
    const origRAF = globalThis.requestAnimationFrame;
    const origCAF = globalThis.cancelAnimationFrame;
    const origElement = mocks.terminal.element;
    let rafCallback: FrameRequestCallback | null = null;
    globalThis.requestAnimationFrame = vi.fn(
      (cb: FrameRequestCallback) => {
        rafCallback = cb;
        return 1;
      },
    ) as unknown as typeof requestAnimationFrame;
    globalThis.cancelAnimationFrame = vi.fn();

    try {
      const container = containerStub();
      renderHook(() => useTerminalInstance(container, {}));
      // 排空微任务让 fonts.ready.then 执行（rAF 回调被捕获）；
      // await 期间积压的真实 rAF 定时器可能已触发——清计数后再手动触发
      await act(async () => {});
      expect(rafCallback).not.toBeNull();
      mocks.fitAddon.fit.mockClear();

      // 模拟 element 未挂载（term.element 为 null）→ 回调提前返回，fit 不执行
      mocks.terminal.element = null;
      act(() => {
        rafCallback!(performance.now());
      });
      expect(mocks.fitAddon.fit).not.toHaveBeenCalled();
    } finally {
      globalThis.requestAnimationFrame = origRAF;
      globalThis.cancelAnimationFrame = origCAF;
      mocks.terminal.element = origElement;
    }
  });

  it("12. fontSize 经 undefined 再恢复同值 → prevFontSize 相同分支跳过写入（effect 重建路径）", () => {
    // 用例 3 的「相同值跳过」由 React deps 相等性短路（effect 不重跑）；
    // 本用例经 fontSize 14→undefined→14 强制 effect 每次重建，走代码内
    // prevFontSizeRef 相同值 return 分支（line 202）——setter 仍只写入 1 次
    const container = containerStub();
    const setter = vi.fn();
    Object.defineProperty(mocks.terminal.options, "fontSize", {
      configurable: true,
      set: setter,
    });
    const { rerender } = renderHook(
      ({ fontSize }: { fontSize?: number }) => useTerminalInstance(container, {}, fontSize),
      // initialProps 显式标注可空类型——否则 TS 从字面量推断 Props 为
      // { fontSize: number }，后续 rerender({ fontSize: undefined }) 报 TS2322
      { initialProps: { fontSize: 14 } as { fontSize?: number } },
    );
    expect(setter).toHaveBeenCalledTimes(1); // 首次 14 写入

    rerender({ fontSize: undefined }); // undefined → 跳过（fontSize === undefined 分支）
    expect(setter).toHaveBeenCalledTimes(1);

    rerender({ fontSize: 14 }); // 恢复 14 → prev 相同 → 跳过写入
    expect(setter).toHaveBeenCalledTimes(1);
  });
});
