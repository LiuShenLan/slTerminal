// workspace-cross-page-guard.test.ts — 跨页组拖拽回迁守卫测试（CP-004）
//
// enforcePanelGroupMembership（宿主 onDidAddPanel 订阅）直测：
// - 页归属一致 → no-op（不触发回迁）
// - 越界（panel 落他页组/dockview 自生组）→ moveTo 回迁原页组 + console.warn
// - 回迁后空的越界组被移除；回迁失败仅 console.error 不抛
// - 无页归属面板保留现组（理论不可达防御）

import { describe, it, expect, vi, afterEach } from "vitest";
import { enforcePanelGroupMembership } from "../workspace/WorkspaceDockHost";
import { pageGroupId, panelIdInPage } from "../workspace/pageGroups";

const PAGE_A = "page-a";
const PAGE_B = "page-b";

interface FakePanel {
  id: string;
  group: { id: string };
  api: { moveTo: ReturnType<typeof vi.fn> };
}

/** fake 宿主：组目录 + removeGroup/moveTo spy */
function makeFakeHost() {
  const groups = new Map<string, { id: string; panels: unknown[] }>();
  const api = {
    getGroup: (id: string) => groups.get(id) ?? undefined,
    removeGroup: vi.fn((g: { id: string }) => { groups.delete(g.id); }),
  };
  return { api, groups };
}

/** 构造带页归属的面板（group 指向宿主内组） */
function makePanel(id: string, groupId: string, moveTo: ReturnType<typeof vi.fn> = vi.fn()): FakePanel {
  return { id, group: { id: groupId }, api: { moveTo } };
}

function seedGroups(groups: Map<string, { id: string; panels: unknown[] }>, ...ids: string[]) {
  for (const id of ids) groups.set(id, { id, panels: [] });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("enforcePanelGroupMembership（跨页组拖拽回迁）", () => {
  it("面板页前缀与目标组一致 → no-op（不 warn 不移动不删组）", () => {
    const { api, groups } = makeFakeHost();
    seedGroups(groups, pageGroupId(PAGE_A));
    const panel = makePanel(panelIdInPage(PAGE_A, "terminal-0"), pageGroupId(PAGE_A));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    enforcePanelGroupMembership(api as never, panel as never);

    expect(warnSpy).not.toHaveBeenCalled();
    expect(panel.api.moveTo).not.toHaveBeenCalled();
    expect(api.removeGroup).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("越界（panel 落他页组）→ console.warn + moveTo 回迁原页组", () => {
    const { api, groups } = makeFakeHost();
    seedGroups(groups, pageGroupId(PAGE_A), pageGroupId(PAGE_B));
    const moveTo = vi.fn();
    // page-a 的面板被拖入 page-b 页组
    const panel = makePanel(panelIdInPage(PAGE_A, "terminal-0"), pageGroupId(PAGE_B), moveTo);
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    enforcePanelGroupMembership(api as never, panel as never);

    expect(warnSpy).toHaveBeenCalledTimes(1);
    expect(warnSpy.mock.calls[0][0]).toContain("跨页组拖拽回迁");
    expect(moveTo).toHaveBeenCalledTimes(1);
    // 回迁目标 = 面板属主页组（page-a 页组对象）
    expect(moveTo.mock.calls[0][0].group).toBe(groups.get(pageGroupId(PAGE_A)));
    warnSpy.mockRestore();
  });

  it("越界落 dockview 自生组（无页归属组）→ 同样回迁", () => {
    const { api, groups } = makeFakeHost();
    seedGroups(groups, pageGroupId(PAGE_A));
    groups.set("group-stray", { id: "group-stray", panels: [] });
    const moveTo = vi.fn();
    const panel = makePanel(panelIdInPage(PAGE_A, "terminal-0"), "group-stray", moveTo);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    enforcePanelGroupMembership(api as never, panel as never);

    expect(moveTo).toHaveBeenCalledTimes(1);
    expect(moveTo.mock.calls[0][0].group).toBe(groups.get(pageGroupId(PAGE_A)));
  });

  it("回迁后空的越界组被移除（dockview 拆分遗留壳清理）", () => {
    const { api, groups } = makeFakeHost();
    seedGroups(groups, pageGroupId(PAGE_A), pageGroupId(PAGE_B));
    const moveTo = vi.fn(() => {
      // 回迁成功后越界组已无面板（模拟 dockview 状态）
      groups.get(pageGroupId(PAGE_B))!.panels = [];
    });
    const panel = makePanel(panelIdInPage(PAGE_A, "terminal-0"), pageGroupId(PAGE_B), moveTo);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    const groupB = groups.get(pageGroupId(PAGE_B));
    enforcePanelGroupMembership(api as never, panel as never);

    expect(api.removeGroup).toHaveBeenCalledTimes(1);
    expect(api.removeGroup.mock.calls[0][0]).toBe(groupB);
    expect(groups.has(pageGroupId(PAGE_B))).toBe(false);
  });

  it("面板属主页组已不存在 → 不移动不删组（安静退出）", () => {
    const { api, groups } = makeFakeHost();
    seedGroups(groups, pageGroupId(PAGE_B)); // 属主页组缺失
    const moveTo = vi.fn();
    const panel = makePanel(panelIdInPage(PAGE_A, "terminal-0"), pageGroupId(PAGE_B), moveTo);
    vi.spyOn(console, "warn").mockImplementation(() => {});

    enforcePanelGroupMembership(api as never, panel as never);

    expect(moveTo).not.toHaveBeenCalled();
    expect(api.removeGroup).not.toHaveBeenCalled();
  });

  it("moveTo 抛异常 → 仅 console.error 不向上抛（守卫不破坏宿主事件流）", () => {
    const { api, groups } = makeFakeHost();
    seedGroups(groups, pageGroupId(PAGE_A), pageGroupId(PAGE_B));
    const moveTo = vi.fn(() => { throw new Error("move failed"); });
    const panel = makePanel(panelIdInPage(PAGE_A, "terminal-0"), pageGroupId(PAGE_B), moveTo);
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => enforcePanelGroupMembership(api as never, panel as never)).not.toThrow();
    expect(errorSpy).toHaveBeenCalledTimes(1);
    expect(errorSpy.mock.calls[0][0]).toContain("面板回迁失败");
    warnSpy.mockRestore();
    errorSpy.mockRestore();
  });

  it("无页归属面板（无冒号 id）落任意组 → 保留现组不移动（防御分支）", () => {
    const { api, groups } = makeFakeHost();
    seedGroups(groups, pageGroupId(PAGE_A));
    const moveTo = vi.fn();
    // 旧格式裸 id（理论上已迁移——防御性保留现组）
    const panel = makePanel("terminal-page-a-0", pageGroupId(PAGE_A), moveTo);
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    enforcePanelGroupMembership(api as never, panel as never);

    expect(warnSpy).toHaveBeenCalled();
    expect(moveTo).not.toHaveBeenCalled();
    expect(api.removeGroup).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });
});
