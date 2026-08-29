// open-settings-panel.test.ts — openSettingsPanel 单元测试（F11）
//
// 覆盖：面板 id = settings-{pageId}、addPanel 参数精确（component "settings"、
//       title "设置"、params.panelId）、同页单例（命中 focus / 未命中 addPanel）、
//       pageId 变化 panelId 跟随、深链 settingsPageId 注入 params.selectedPage、
//       getPageApi 延迟就绪轮询命中、5s 超时降级。
// 真实 pageApis（不 mock），用 registerPageApi/unregisterPageApi 控制模块级 pageApiMap。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  openSettingsPanel,
  registerPageApi,
  unregisterPageApi,
} from "../workspace/pageApis";
import type { DockviewApi } from "dockview-react";

/** DockviewApi stub：getPanel/addPanel 共享内部 Map（照 open-hooks-config-panel 测试模式） */
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

describe("openSettingsPanel", () => {
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

  it("API 已注册 → 立即 addPanel + 参数精确（单例 id 规则锁定）", async () => {
    registerPageApi("page-a", api);
    const ok = await openSettingsPanel("page-a");
    expect(ok).toBe(true);
    expect(api.addPanel).toHaveBeenCalledWith({
      id: "settings-page-a",
      component: "settings",
      title: "设置",
      params: { panelId: "settings-page-a" },
    });
  });

  it("面板已存在 → focus 不新建（同页单例）", async () => {
    registerPageApi("page-a", api);
    await openSettingsPanel("page-a");
    const addCalls = (api.addPanel as ReturnType<typeof vi.fn>).mock.calls.length;
    await openSettingsPanel("page-a");
    expect((api.getPanel as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThanOrEqual(2);
    expect((api.addPanel as ReturnType<typeof vi.fn>).mock.calls.length).toBe(addCalls);
    // focus 被调用（面板已存在时聚焦）
    const results = (api.getPanel as ReturnType<typeof vi.fn>).mock.results;
    const panel = results[results.length - 1]?.value;
    expect(panel.focus).toHaveBeenCalled();
  });

  it("API 延迟注册（首次挂载页面）→ 轮询命中后 addPanel", async () => {
    const p = openSettingsPanel("page-a"); // 未注册 → 挂起 100ms 轮询
    registerPageApi("page-a", api);
    await vi.advanceTimersByTimeAsync(100);
    const ok = await p;
    expect(ok).toBe(true);
    expect(api.addPanel).toHaveBeenCalled();
  });

  it("API 永不注册 → 5s 超时降级（返回 false + console.warn）", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const p = openSettingsPanel("page-a");
    await vi.advanceTimersByTimeAsync(5100); // 50 次 × 100ms
    const ok = await p;
    expect(ok).toBe(false);
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("pageId 变化 → panelId 跟随（每页独立单例）", async () => {
    registerPageApi("page-a", api);
    registerPageApi("page-b", api);
    await openSettingsPanel("page-a");
    await openSettingsPanel("page-b");
    const calls = (api.addPanel as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls[0][0].id).toBe("settings-page-a");
    expect(calls[1][0].id).toBe("settings-page-b");
  });

  it("深链 settingsPageId → params.selectedPage 注入（未传则不注入）", async () => {
    registerPageApi("page-a", api);
    await openSettingsPanel("page-a", "hooks");
    expect(api.addPanel).toHaveBeenCalledWith({
      id: "settings-page-a",
      component: "settings",
      title: "设置",
      params: { panelId: "settings-page-a", selectedPage: "hooks" },
    });
  });

  it("getPanel 命中但面板对象无 focus 方法 → 降级不抛错、addPanel 不再调用", async () => {
    registerPageApi("page-a", api);
    // 模拟 Dockview 边界：getPanel 返回无 focus 方法的面板对象（只有 id）
    (api.getPanel as ReturnType<typeof vi.fn>).mockReturnValue({
      id: "settings-page-a",
    } as unknown as { focus: ReturnType<typeof vi.fn> });
    const addSpy = api.addPanel as ReturnType<typeof vi.fn>;
    // 不抛错：视作已打开，返回 true
    await expect(openSettingsPanel("page-a")).resolves.toBe(true);
    // 不新建面板（同页单例语义保持）
    expect(addSpy).not.toHaveBeenCalled();
  });
});
