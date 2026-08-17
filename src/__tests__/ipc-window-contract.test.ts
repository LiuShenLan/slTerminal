// ipc-window-contract.test.ts — src/ipc/window.ts 最小契约测试（WRK-04 + WRK-08）
//
// 测试真实 window.ts 封装（不 mock 本模块），mock @tauri-apps/api/window：
// - registerCloseHandler（WRK-08）：关窗拦截——event.preventDefault + 回调完成后 destroy（finally 保证）
// - onFocusChanged / requestUserAttention / setFocus（WRK-04）：命令/参数/返回/异常传播四维

import { describe, it, expect, beforeEach, vi } from "vitest";

// ─── Hoisted：捕获 Tauri Window 回调 + spy ───
const mocks = vi.hoisted(() => {
  let focusCb: ((e: { payload: boolean }) => void) | null = null;
  let closeCb: ((e: { preventDefault: () => void }) => void) | null = null;
  const requestUserAttention = vi.fn().mockResolvedValue(undefined);
  const setFocus = vi.fn().mockResolvedValue(undefined);
  const destroy = vi.fn().mockResolvedValue(undefined);
  const unlisten = vi.fn();
  /** FE-26：置 true 时 onCloseRequested 返回 rejected Promise（模拟窗口已销毁场景） */
  let unlistenRejects = false;

  return {
    get focusCb() {
      return focusCb;
    },
    set focusCb(cb: ((e: { payload: boolean }) => void) | null) {
      focusCb = cb;
    },
    get closeCb() {
      return closeCb;
    },
    set closeCb(cb: ((e: { preventDefault: () => void }) => void) | null) {
      closeCb = cb;
    },
    get unlistenRejects() {
      return unlistenRejects;
    },
    set unlistenRejects(v: boolean) {
      unlistenRejects = v;
    },
    requestUserAttention,
    setFocus,
    destroy,
    unlisten,
    resetAll() {
      focusCb = null;
      closeCb = null;
      unlistenRejects = false;
      requestUserAttention.mockClear();
      setFocus.mockClear();
      destroy.mockClear();
      unlisten.mockClear();
    },
  };
});

// ─── Module mock：@tauri-apps/api/window（覆盖 setup.ts 全局 mock）───
vi.mock("@tauri-apps/api/window", () => ({
  UserAttentionType: { Critical: 1, Informational: 2 } as const,
  getCurrentWindow: vi.fn(() => ({
    onFocusChanged: vi.fn((cb: (e: { payload: boolean }) => void) => {
      mocks.focusCb = cb;
      return Promise.resolve(mocks.unlisten);
    }),
    onCloseRequested: vi.fn((cb: (e: { preventDefault: () => void }) => void) => {
      mocks.closeCb = cb;
      // FE-26：窗口已销毁场景——返回 rejected Promise（registerCloseHandler 内部须 .catch 兜底）
      return mocks.unlistenRejects
        ? Promise.reject(new Error("window destroyed"))
        : Promise.resolve(mocks.unlisten);
    }),
    requestUserAttention: mocks.requestUserAttention,
    setFocus: mocks.setFocus,
    destroy: mocks.destroy,
  })),
}));

// ─── 导入被测模块（真实实现）───
import {
  onFocusChanged,
  requestUserAttention,
  setFocus,
  registerCloseHandler,
  UserAttentionType,
} from "../ipc/window";

/** 模拟 Tauri 关闭事件对象 */
function mockCloseEvent() {
  return { preventDefault: vi.fn() };
}

describe("registerCloseHandler（WRK-08 关窗拦截）", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("1. 关闭事件触发时调用 event.preventDefault（阻止立即关闭）", async () => {
    registerCloseHandler(vi.fn(async () => {}));

    const event = mockCloseEvent();
    await mocks.closeCb!(event);

    expect(event.preventDefault).toHaveBeenCalledTimes(1);
  });

  it("2. 回调完成后 destroy 窗口（正常路径）", async () => {
    const cb = vi.fn(async () => {});
    registerCloseHandler(cb);

    await mocks.closeCb!(mockCloseEvent());

    expect(cb).toHaveBeenCalledTimes(1);
    expect(mocks.destroy).toHaveBeenCalledTimes(1);
  });

  it("3. 回调抛异常 → destroy 仍被调用（finally 保证），异常向外传播", async () => {
    registerCloseHandler(async () => {
      throw new Error("保存失败");
    });

    await expect(mocks.closeCb!(mockCloseEvent())).rejects.toThrow("保存失败");
    expect(mocks.destroy).toHaveBeenCalledTimes(1);
  });

  it("4. 清理函数解除监听（unlisten 被调用）", async () => {
    const cleanup = registerCloseHandler(vi.fn(async () => {}));

    cleanup();
    await vi.waitFor(() => {
      expect(mocks.unlisten).toHaveBeenCalledTimes(1);
    });
  });

  it("5. FE-26: unlisten Promise reject（窗口已销毁）→ 清理函数吞掉，无未处理 rejection", async () => {
    mocks.unlistenRejects = true;
    // 监听 unhandledrejection——实现缺 .catch 时 cleanup 会产生未处理 rejection 落入此数组
    const unhandled: unknown[] = [];
    const onUnhandled = (e: PromiseRejectionEvent) => { unhandled.push(e.reason); };
    window.addEventListener("unhandledrejection", onUnhandled);

    const cleanup = registerCloseHandler(vi.fn(async () => {}));
    expect(() => cleanup()).not.toThrow();

    // 等 unlisten 链 settle（微任务 + 事件循环），让潜在的未处理 rejection 派发
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(unhandled).toEqual([]);

    window.removeEventListener("unhandledrejection", onUnhandled);
  });
});

describe("onFocusChanged（WRK-04）", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("1. 焦点事件 payload 透传给回调（true/false）", async () => {
    const cb = vi.fn();
    onFocusChanged(cb);

    mocks.focusCb!({ payload: true });
    mocks.focusCb!({ payload: false });

    expect(cb).toHaveBeenNthCalledWith(1, true);
    expect(cb).toHaveBeenNthCalledWith(2, false);
  });

  it("2. 清理函数解除监听", async () => {
    const cleanup = onFocusChanged(vi.fn());

    cleanup();
    await vi.waitFor(() => {
      expect(mocks.unlisten).toHaveBeenCalledTimes(1);
    });
  });
});

describe("requestUserAttention（WRK-04）", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("1. 参数透传（null 停止闪烁 / Critical 请求关注）+ UserAttentionType re-export", async () => {
    await requestUserAttention(null);
    expect(mocks.requestUserAttention).toHaveBeenCalledWith(null);

    await requestUserAttention(UserAttentionType.Critical);
    expect(mocks.requestUserAttention).toHaveBeenCalledWith(UserAttentionType.Critical);
    expect(UserAttentionType.Critical).toBeDefined();
  });

  it("2. reject 传播（不吞异常）", async () => {
    mocks.requestUserAttention.mockRejectedValueOnce(new Error("window api fail"));

    await expect(requestUserAttention(null)).rejects.toThrow("window api fail");
  });
});

describe("setFocus（WRK-04，预留标注）", () => {
  beforeEach(() => {
    mocks.resetAll();
  });

  it("1. 正常调用透传至 appWindow.setFocus", async () => {
    await setFocus();
    expect(mocks.setFocus).toHaveBeenCalledTimes(1);
  });

  it("2. reject 传播（不吞异常）", async () => {
    mocks.setFocus.mockRejectedValueOnce(new Error("focus fail"));

    await expect(setFocus()).rejects.toThrow("focus fail");
  });
});
