// wireKeybindings.test.ts — 覆盖层→注册表接线单元测试
//
// 覆盖：立即应用当前 overrides、store 变更触发重新应用、返回的 unsubscribe 生效。
// 用 fake store（getState/subscribe stub）隔离，不依赖真实 Zustand / 注册表。

import { describe, it, expect, vi } from "vitest";
import { wireKeybindings } from "../features/shortcuts/wireKeybindings";
import type { KeybindingOverrides } from "../features/shortcuts";

/** 构造 fake store：可变 overrides + 捕获 listener */
function makeFakeStore(initial: KeybindingOverrides) {
  let overrides = initial;
  let listener: (() => void) | null = null;
  const unsub = vi.fn();
  return {
    store: {
      getState: () => ({ overrides }),
      subscribe: (fn: () => void) => {
        listener = fn;
        return unsub;
      },
    },
    setOverrides: (o: KeybindingOverrides) => { overrides = o; },
    fire: () => listener?.(),
    unsub,
  };
}

describe("wireKeybindings", () => {
  it("调用即用当前 overrides 应用一次", () => {
    const setOverrides = vi.fn();
    const { store } = makeFakeStore({ "terminal.copy": "Ctrl+Alt+KeyC" });

    wireKeybindings({ setOverrides }, store);

    expect(setOverrides).toHaveBeenCalledTimes(1);
    expect(setOverrides).toHaveBeenCalledWith({ "terminal.copy": "Ctrl+Alt+KeyC" });
  });

  it("store 变更 → 重新应用最新 overrides", () => {
    const setOverrides = vi.fn();
    const fake = makeFakeStore({});

    wireKeybindings({ setOverrides }, fake.store);
    expect(setOverrides).toHaveBeenCalledTimes(1);
    expect(setOverrides).toHaveBeenLastCalledWith({});

    // 模拟 store overrides 更新后触发订阅
    fake.setOverrides({ "editor.save": null });
    fake.fire();

    expect(setOverrides).toHaveBeenCalledTimes(2);
    expect(setOverrides).toHaveBeenLastCalledWith({ "editor.save": null });
  });

  it("返回值为 store.subscribe 的 unsubscribe", () => {
    const setOverrides = vi.fn();
    const fake = makeFakeStore({});

    const dispose = wireKeybindings({ setOverrides }, fake.store);
    expect(typeof dispose).toBe("function");

    dispose();
    expect(fake.unsub).toHaveBeenCalledTimes(1);
  });
});
