// open-hooks-config-panel.test.ts — openHooksConfigPanel 单元测试
//
// 覆盖：C13-7 同页单例语义（面板 id = hooksConfig-{pageId}，命中 focus / 未命中 addPanel）、
//       getPageApi 延迟就绪轮询命中、5s 超时降级、pageId 变化 panelId 跟随。
// 真实 pageApis（不 mock），用 registerPageApi/unregisterPageApi 控制模块级 pageApiMap。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  openHooksConfigPanel,
  registerPageApi,
  unregisterPageApi,
} from "../workspace/pageApis";
import type { DockviewApi } from "dockview-react";

/** DockviewApi stub：getPanel/addPanel 共享内部 Map（照已删 hooks-config-entry.test 模式） */
function dockviewApiStub(): DockviewApi {
  const panels = new Map<string, { focus: ReturnType<typeof vi.fn> }>();
  return {
    getPanel: vi.fn((id: string) => panels.get(id)),
    addPanel: vi.fn((params: { id: string }) => {
      const panel = { focus: vi.fn() };
      panels.set(params.id, panel);
      return panel;
    }),
  } as unknown as DockviewApi;
}

describe("openHooksConfigPanel", () => {
  let api: DockviewApi;

  beforeEach(() => {
    vi.useFakeTimers();
    // 清理模块级 pageApiMap 跨用例残留
    unregisterPageApi("page-a");
    unregisterPageApi("page-b");
    api = dockviewApiStub();
  });

  afterEach(() => {
    vi.useRealTimers();
    unregisterPageApi("page-a");
    unregisterPageApi("page-b");
  });

  it("API 已注册 → 立即 addPanel + 参数精确（C13-7 单例 id 规则锁定）", async () => {
    registerPageApi("page-a", api);
    const ok = await openHooksConfigPanel("page-a");
    expect(ok).toBe(true);
    expect(api.addPanel).toHaveBeenCalledWith({
      id: "hooksConfig-page-a",
      component: "hooksConfig",
      title: "Hooks 配置",
      params: { panelId: "hooksConfig-page-a" },
    });
  });

  it("面板已存在 → focus 不新建（同页单例）", async () => {
    registerPageApi("page-a", api);
    await openHooksConfigPanel("page-a");
    const addCalls = (api.addPanel as ReturnType<typeof vi.fn>).mock.calls.length;
    await openHooksConfigPanel("page-a");
    expect((api.getPanel as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThanOrEqual(2);
    expect((api.addPanel as ReturnType<typeof vi.fn>).mock.calls.length).toBe(addCalls);
    // focus 被调用（面板已存在时聚焦）
    const results = (api.getPanel as ReturnType<typeof vi.fn>).mock.results;
    const panel = results[results.length - 1]?.value;
    expect(panel.focus).toHaveBeenCalled();
  });

  it("API 延迟注册（首次挂载页面）→ 轮询命中后 addPanel", async () => {
    const p = openHooksConfigPanel("page-a"); // 未注册 → 挂起 100ms 轮询
    registerPageApi("page-a", api);
    await vi.advanceTimersByTimeAsync(100);
    const ok = await p;
    expect(ok).toBe(true);
    expect(api.addPanel).toHaveBeenCalled();
  });

  it("API 永不注册 → 5s 超时降级（返回 false + console.warn）", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const p = openHooksConfigPanel("page-a");
    await vi.advanceTimersByTimeAsync(5100); // 50 次 × 100ms
    const ok = await p;
    expect(ok).toBe(false);
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("pageId 变化 → panelId 跟随（每页独立单例）", async () => {
    registerPageApi("page-a", api);
    registerPageApi("page-b", api);
    await openHooksConfigPanel("page-a");
    await openHooksConfigPanel("page-b");
    const calls = (api.addPanel as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls[0][0].id).toBe("hooksConfig-page-a");
    expect(calls[1][0].id).toBe("hooksConfig-page-b");
  });
});
