// webgl-setup.test.ts — setupWebglWithRetry 指数退避重试测试
//
// 覆盖 checklist TRM-06 全分支：
// - WebGL 不可用 → 立即 onFail 回退 DOM
// - 成功加载 → onSuccess 收到实例
// - context loss → 指数退避重建（1000/2000ms 序列）
// - 重试耗尽 → onFail 回退 DOM
// - cancel() → 清除定时器 + dispose 当前 addon + cancelled 守卫
// - loadAddon 抛异常 → 同样退避重试
//
// L4 真实 context loss 场景归 E2E-04（本 Stage 不做）。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import type { Terminal } from "@xterm/xterm";
import { detectWebgl, resetWebglCache, setupWebglWithRetry } from "../panels/terminal/webgl";

// ─── Hoisted mock：WebglAddon 实例可控（onContextLoss 回调捕获 + 实例列表） ───
const mocks = vi.hoisted(() => {
  const addons: Array<{
    onContextLoss: ReturnType<typeof vi.fn>;
    _lossCb: (() => void) | null;
    dispose: ReturnType<typeof vi.fn>;
  }> = [];
  const WebglAddonMock = vi.fn(function () {
    const addon = {
      onContextLoss: vi.fn((cb: () => void) => {
        addon._lossCb = cb;
      }),
      dispose: vi.fn(),
      _lossCb: null as (() => void) | null,
    };
    addons.push(addon);
    return addon;
  });
  return { addons, WebglAddonMock };
});

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: mocks.WebglAddonMock,
}));

/** 构造最小 Terminal stub */
function makeTerm(): Terminal {
  return { loadAddon: vi.fn() } as unknown as Terminal;
}

describe("setupWebglWithRetry", () => {
  let getContextSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    resetWebglCache();
    // 默认 WebGL2 可用（测试 1 单独覆盖不可用分支）
    getContextSpy = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue({} as unknown as RenderingContext);
    mocks.addons.length = 0;
    mocks.WebglAddonMock.mockClear();
    vi.useFakeTimers();
  });

  afterEach(() => {
    getContextSpy.mockRestore();
    vi.useRealTimers();
  });

  it("1. WebGL 不可用 → 立即 onFail 回退 DOM，cancel 为 no-op", () => {
    getContextSpy.mockReturnValue(null);
    resetWebglCache(); // 缓存结果以 null 检测为准
    expect(detectWebgl()).toBe(false);

    const term = makeTerm();
    const onSuccess = vi.fn();
    const onFail = vi.fn();
    const { cancel } = setupWebglWithRetry(term, onSuccess, onFail);

    expect(onFail).toHaveBeenCalledTimes(1);
    expect(onSuccess).not.toHaveBeenCalled();
    expect(mocks.WebglAddonMock).not.toHaveBeenCalled();
    // cancel no-op 不抛
    expect(() => cancel()).not.toThrow();
  });

  it("2. WebGL 可用 → 加载成功，onSuccess 收到实例", () => {
    const term = makeTerm();
    const onSuccess = vi.fn();
    const onFail = vi.fn();
    setupWebglWithRetry(term, onSuccess, onFail);

    expect(mocks.WebglAddonMock).toHaveBeenCalledTimes(1);
    expect(term.loadAddon).toHaveBeenCalledTimes(1);
    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(onSuccess).toHaveBeenCalledWith(mocks.addons[0]);
    expect(onFail).not.toHaveBeenCalled();
  });

  it("3. context loss → 指数退避重建（1000ms → 2000ms 序列）", () => {
    const term = makeTerm();
    const onSuccess = vi.fn();
    const onFail = vi.fn();
    setupWebglWithRetry(term, onSuccess, onFail);
    expect(onSuccess).toHaveBeenCalledTimes(1);

    // 第一次 context loss → dispose 旧 addon + 1000ms 后重建
    mocks.addons[0]._lossCb!();
    expect(mocks.addons[0].dispose).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(999);
    expect(mocks.addons).toHaveLength(1); // 未到延迟不重建
    vi.advanceTimersByTime(1);
    expect(mocks.addons).toHaveLength(2); // 1000ms 到达 → 重建
    expect(onSuccess).toHaveBeenCalledTimes(2);
    expect(onFail).not.toHaveBeenCalled();

    // 第二次 context loss → 2000ms 退避
    mocks.addons[1]._lossCb!();
    expect(mocks.addons[1].dispose).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(2000);
    expect(mocks.addons).toHaveLength(3);
    expect(onSuccess).toHaveBeenCalledTimes(3);
  });

  it("4. 重试耗尽（6 次 context loss）→ onFail 回退 DOM", () => {
    const term = makeTerm();
    const onSuccess = vi.fn();
    const onFail = vi.fn();
    setupWebglWithRetry(term, onSuccess, onFail);

    // 5 次退避重建（大步长 16000ms 越过 1000/2000/4000/8000/16000 全序列）
    for (let i = 0; i < 5; i++) {
      mocks.addons[i]._lossCb!();
      vi.advanceTimersByTime(16000);
    }
    expect(onFail).not.toHaveBeenCalled(); // 第 5 次 loss 仍在退避窗口内
    expect(mocks.addons).toHaveLength(6);
    expect(onSuccess).toHaveBeenCalledTimes(6);

    // 第 6 次 context loss（attempt=5 达上限）→ 回退 DOM
    mocks.addons[5]._lossCb!();
    expect(onFail).toHaveBeenCalledTimes(1);
    expect(mocks.addons).toHaveLength(6); // 不再重建
  });

  it("5. cancel() 清除待触发定时器 + cancelled 守卫拦截后续回调", () => {
    const term = makeTerm();
    const onSuccess = vi.fn();
    const onFail = vi.fn();
    const { cancel } = setupWebglWithRetry(term, onSuccess, onFail);

    // 触发 context loss → 存在 1000ms 待触发重试
    mocks.addons[0]._lossCb!();
    expect(mocks.addons[0].dispose).toHaveBeenCalledTimes(1);

    cancel();
    vi.advanceTimersByTime(100000);
    expect(mocks.addons).toHaveLength(1); // 定时器已清除，不重建
    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(onFail).not.toHaveBeenCalled();

    // cancel 后 loss 回调再触发 → cancelled 守卫直接返回
    mocks.addons[0]._lossCb!();
    vi.advanceTimersByTime(100000);
    expect(mocks.addons).toHaveLength(1);
    expect(onFail).not.toHaveBeenCalled();
  });

  it("6. cancel() 在加载成功态 dispose 当前 addon（释放 GPU 资源）", () => {
    const term = makeTerm();
    const onSuccess = vi.fn();
    const onFail = vi.fn();
    const { cancel } = setupWebglWithRetry(term, onSuccess, onFail);

    expect(mocks.addons).toHaveLength(1);
    cancel();
    expect(mocks.addons[0].dispose).toHaveBeenCalledTimes(1);
    expect(onFail).not.toHaveBeenCalled();
  });

  it("7. loadAddon 抛异常（context 过渡态）→ 同样退避重试，成功后 onSuccess", () => {
    const term = makeTerm();
    // 首次加载失败（mockImplementationOnce），后续成功
    (term.loadAddon as ReturnType<typeof vi.fn>).mockImplementationOnce(() => {
      throw new Error("loadAddon failed");
    });
    const onSuccess = vi.fn();
    const onFail = vi.fn();
    setupWebglWithRetry(term, onSuccess, onFail);

    // 首次失败 → 不通知成功，进入 1000ms 退避
    expect(onSuccess).not.toHaveBeenCalled();
    expect(mocks.addons).toHaveLength(1);
    vi.advanceTimersByTime(1000);
    // 重试成功 → onSuccess
    expect(mocks.addons).toHaveLength(2);
    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(onFail).not.toHaveBeenCalled();
  });
});
