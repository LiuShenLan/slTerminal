// open-settings-panel.test.ts — openSettingsPanel 单元测试（F11，CP-042 事件驱动，
// CP-004 单宿主语义适配）
//
// 覆盖：面板 id = panelIdInPage(pageId, "settings")（页前缀协议）、addPanel 参数
// 精确（component "settings"、title "设置"、renderer "always"（CP-017 接线点）、
// position.referenceGroup = 页组 id、params.panelId）、同页单例（命中 focus / 未命中
// addPanel）、pageId 变化 panelId 跟随、深链 settingsPageId 注入 params.selectedPage、
// 页组挂载事件驱动唤醒（markPageGroupMounted 派发）、事件 detail 非目标页不唤醒、
// 5s 超时降级（返回 false + console.warn + toast.show 各一次，可观测化）。
// 真实 pageApis（不 mock），用 registerHostApi/unregisterHostApi +
// markPageGroupMounted 控制宿主/页组挂载态；toast 经 lib mock。

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  openSettingsPanel,
  registerHostApi,
  unregisterHostApi,
  markPageGroupMounted,
} from "../workspace/pageApis";
import { pageGroupId } from "../workspace/pageGroups";
import type { DockviewApi } from "dockview-react";

const { toastShowMock } = vi.hoisted(() => ({
  toastShowMock: vi.fn(),
}));

vi.mock("../lib", () => ({ toast: { show: toastShowMock } }));

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
    getGroup: vi.fn((id: string) => ({ id })),
  } as unknown as DockviewApi;
}

/** 测试装配：注册宿主 + 标记页组挂载（页面就绪语义——旧 registerPageApi 替代） */
function markPageReady(pageId: string, api: DockviewApi): void {
  registerHostApi(api);
  markPageGroupMounted(pageId);
}

describe("openSettingsPanel", () => {
  let api: DockviewApi;

  beforeEach(() => {
    vi.useFakeTimers();
    toastShowMock.mockReset();
    unregisterHostApi();
    api = dockviewApiStub();
  });

  afterEach(() => {
    vi.useRealTimers();
    unregisterHostApi();
  });

  it("页组已挂载 → 立即 addPanel + 参数精确（页前缀协议 id + 显式页组 position）", async () => {
    markPageReady("page-a", api);
    const ok = await openSettingsPanel("page-a");
    expect(ok).toBe(true);
    expect(api.addPanel).toHaveBeenCalledWith({
      id: "page-a:settings",
      component: "settings",
      title: "设置",
      renderer: "always",
      position: { referenceGroup: pageGroupId("page-a") },
      params: { panelId: "page-a:settings" },
    });
  });

  it("面板已存在 → focus 不新建（同页单例）", async () => {
    markPageReady("page-a", api);
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

  it("页组延迟挂载（首次挂载页面）→ markPageGroupMounted 派发事件立即唤醒 addPanel（CP-042）", async () => {
    const p = openSettingsPanel("page-a"); // 未挂载 → 事件监听挂起（无轮询）
    markPageReady("page-a", api); // 就绪事件 → 立即唤醒
    const ok = await p;
    expect(ok).toBe(true);
    expect(api.addPanel).toHaveBeenCalledTimes(1);
    expect(api.addPanel).toHaveBeenCalledWith(
      expect.objectContaining({ id: "page-a:settings", renderer: "always" }),
    );
  });

  it("事件 detail 非目标 pageId 不唤醒（CP-042 事件过滤）", async () => {
    const p = openSettingsPanel("page-a");
    // 他页就绪（page-b 派发事件）→ detail 过滤，监听不唤醒
    markPageReady("page-b", api);
    await vi.advanceTimersByTimeAsync(100);
    expect(api.addPanel).not.toHaveBeenCalled();
    // 目标页挂载 → 事件驱动唤醒
    markPageGroupMounted("page-a");
    const ok = await p;
    expect(ok).toBe(true);
    expect(api.addPanel).toHaveBeenCalledTimes(1);
  });

  it("页面永不挂载 → 5s 超时降级（返回 false + console.warn + toast.show 各一次）", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const p = openSettingsPanel("page-a");
    await vi.advanceTimersByTimeAsync(5000); // 超时防御底线（原 50 次 × 100ms 轮询）
    const ok = await p;
    expect(ok).toBe(false);
    expect(warnSpy).toHaveBeenCalledTimes(1);
    expect(warnSpy.mock.calls[0][0]).toContain("5s 内未就绪");
    // CP-042: 超时经 toast 可观测化（原仅 console.warn 静默降级）
    expect(toastShowMock).toHaveBeenCalledTimes(1);
    expect(toastShowMock).toHaveBeenCalledWith(
      "warning",
      expect.stringContaining("设置中心打开失败"),
    );
    warnSpy.mockRestore();
  });

  it("pageId 变化 → panelId 跟随（每页独立单例——页前缀协议）", async () => {
    markPageReady("page-a", api);
    markPageReady("page-b", api);
    await openSettingsPanel("page-a");
    await openSettingsPanel("page-b");
    const calls = (api.addPanel as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls[0][0].id).toBe("page-a:settings");
    expect(calls[1][0].id).toBe("page-b:settings");
  });

  it("深链 settingsPageId → params.selectedPage 注入（未传则不注入）", async () => {
    markPageReady("page-a", api);
    await openSettingsPanel("page-a", "hooks");
    expect(api.addPanel).toHaveBeenCalledWith({
      id: "page-a:settings",
      component: "settings",
      title: "设置",
      renderer: "always",
      position: { referenceGroup: pageGroupId("page-a") },
      params: { panelId: "page-a:settings", selectedPage: "hooks" },
    });
  });

  it("getPanel 命中但面板对象无 focus 方法 → 降级不抛错、addPanel 不再调用", async () => {
    markPageReady("page-a", api);
    // 模拟 Dockview 边界：getPanel 返回无 focus 方法的面板对象（只有 id）
    (api.getPanel as ReturnType<typeof vi.fn>).mockReturnValue({
      id: "page-a:settings",
    } as unknown as { focus: ReturnType<typeof vi.fn> });
    const addSpy = api.addPanel as ReturnType<typeof vi.fn>;
    // 不抛错：视作已打开，返回 true
    await expect(openSettingsPanel("page-a")).resolves.toBe(true);
    // 不新建面板（同页单例语义保持）
    expect(addSpy).not.toHaveBeenCalled();
  });
});
