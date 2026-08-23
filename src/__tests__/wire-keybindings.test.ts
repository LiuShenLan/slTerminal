// wireKeybindings.test.ts — 覆盖层→注册表接线单元测试
//
// 覆盖：立即应用当前 overrides、store 变更触发重新应用、返回的 unsubscribe 生效。
// 用 fake store（getState/subscribe stub）隔离，不依赖真实 Zustand / 注册表。
// TQ-B-16：补一条与真实 useKeybindings 的集成用例，验证 subscribe/getState 签名契合。

import { describe, it, expect, vi, afterEach } from "vitest";
import { wireKeybindings } from "../features/shortcuts/wireKeybindings";
import { useKeybindings, cancelPendingSave } from "../stores/keybindings";
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
  afterEach(() => {
    // 真实 store 的 2s debounce 持久化订阅防残留 timer + 状态外泄（TQ-B-16）
    cancelPendingSave();
    useKeybindings.setState({ overrides: {}, loaded: false });
  });

  it("与真实 useKeybindings 集成：setBinding 触发 wireKeybindings 重应用", () => {
    useKeybindings.setState({ overrides: {}, loaded: true });
    const setOverrides = vi.fn();
    const unwire = wireKeybindings({ setOverrides }, useKeybindings);

    // 接线即用当前 overrides 应用一次（真实 getState 签名契合）
    expect(setOverrides).toHaveBeenCalledTimes(1);
    expect(setOverrides).toHaveBeenCalledWith({});

    // setBinding 走真实 store 的 subscribe 通知 → 重新应用最新 overrides
    useKeybindings.getState().setBinding("terminal.copy", "Ctrl+Shift+KeyC");
    expect(setOverrides).toHaveBeenCalledTimes(2);
    expect(setOverrides).toHaveBeenLastCalledWith({ "terminal.copy": "Ctrl+Shift+KeyC" });

    // null 解绑同样触发重应用
    useKeybindings.getState().setBinding("terminal.copy", null);
    expect(setOverrides).toHaveBeenCalledTimes(3);
    expect(setOverrides).toHaveBeenLastCalledWith({ "terminal.copy": null });

    // unsubscribe 生效：退订后 setBinding 不再触发重应用
    unwire();
    useKeybindings.getState().setBinding("editor.save", "Ctrl+KeyS");
    expect(setOverrides).toHaveBeenCalledTimes(3);
  });

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
