// dialog-e2e-hook.test.ts — src/ipc/dialog.ts 的 E2E ask 钩子守卫（claude-history Stage 06）
//
// 背景：embedded WDIO 无法操作原生对话框，E2E 删除用例依赖 ask 钩子
// （window.__slterm_e2e_dialogAsk）模拟用户点确认。vitest 环境 E2E_ENABLED=true
// （import.meta.env.DEV），钩子分支可实测。三条守卫：
//   1. 未设置钩子 → 走真实 @tauri-apps/plugin-dialog 的 ask（不改默认行为）
//   2. 设置 true → 直接返回 true，真实 ask 不被调用
//   3. 设置 false → 直接返回 false
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// vi.hoisted：mock 工厂在模块加载前执行，顶层变量须 hoisted 才能在工厂内引用
const realAsk = vi.hoisted(() => vi.fn(async () => true));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
  ask: realAsk,
}));

import { ask } from "../ipc/dialog";

describe("src/ipc/dialog ask E2E 钩子", () => {
  beforeEach(() => {
    realAsk.mockClear();
    delete (window as unknown as Record<string, unknown>)
      .__slterm_e2e_dialogAsk;
  });

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>)
      .__slterm_e2e_dialogAsk;
  });

  it("未设置钩子 → 透传真实 plugin-dialog 的 ask", async () => {
    const r = await ask("确认?", { kind: "warning" });
    expect(r).toBe(true);
    expect(realAsk).toHaveBeenCalledWith("确认?", { kind: "warning" });
  });

  it("设置 __slterm_e2e_dialogAsk=true → 直接返回 true，不调真实 ask", async () => {
    (window as unknown as Record<string, unknown>).__slterm_e2e_dialogAsk =
      true;
    const r = await ask("确认?", { kind: "warning" });
    expect(r).toBe(true);
    expect(realAsk).not.toHaveBeenCalled();
  });

  it("设置 __slterm_e2e_dialogAsk=false → 直接返回 false，不调真实 ask", async () => {
    (window as unknown as Record<string, unknown>).__slterm_e2e_dialogAsk =
      false;
    const r = await ask("确认?", { kind: "warning" });
    expect(r).toBe(false);
    expect(realAsk).not.toHaveBeenCalled();
  });
});
