// workspace-cross-page-guard.test.ts — 组归属审计测试（ADR-0020）
//
// auditGroupMembership（onDidMovePanel/onDidAddPanel 事件源 + 恢复后全量）直测：
// - 合法布局（主组本页面板/自生组同页面板）→ no-op
// - 主组混入他页面板 → moveTo 回迁该页组 + console.warn；空主组保留（Watermark 载体）
// - 自生空壳组 → removeGroup 清理
// - 自生混组以首面板页为属主，少数派回迁各自页组
// - 回迁目标页无组 → 滞留现组不移动（删页路径收口）
// - moveTo 抛异常 → 仅 console.error 不抛（守卫不破坏宿主事件流）
// - 防复发（bug 2 根因）：移动期 onDidAddPanel 被 dockview `_moving` 门控吞掉——
//   审计挂在 onDidMovePanel（movingLock 外触发）才对真实拖拽生效；本文件模拟
//   「拖拽产生自生组」终态直调审计，等价 onDidMovePanel 处理器路径。

import { describe, it, expect, vi, afterEach } from "vitest";
import { auditGroupMembership } from "../workspace/WorkspaceDockHost";
import { pageGroupId, panelIdInPage } from "../workspace/pageGroups";

const PAGE_A = "page-a";
const PAGE_B = "page-b";

interface FakePanel {
  id: string;
  group: { id: string };
  api: { moveTo: ReturnType<typeof vi.fn> };
}

interface FakeGroup {
  id: string;
  panels: FakePanel[];
}

/** fake 宿主：组目录（api.groups 数组形态）+ removeGroup spy */
function makeFakeHost() {
  const groups: FakeGroup[] = [];
  const api = {
    get groups() { return groups; },
    getGroup: (id: string) => groups.find((g) => g.id === id),
    removeGroup: vi.fn((g: FakeGroup) => {
      const i = groups.indexOf(g);
      if (i >= 0) groups.splice(i, 1);
    }),
  };
  return { api, groups };
}

/** 造组并入宿主；面板经 addPanel 挂入（group 反向引用同步） */
function seedGroup(groups: FakeGroup[], id: string): FakeGroup {
  const g: FakeGroup = { id, panels: [] };
  groups.push(g);
  return g;
}

function addPanel(
  group: FakeGroup,
  panelId: string,
  moveTo?: ReturnType<typeof vi.fn>,
): FakePanel {
  const p: FakePanel = { id: panelId, group, api: { moveTo: moveTo ?? vi.fn() } };
  group.panels.push(p);
  return p;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("auditGroupMembership（组归属审计）", () => {
  it("合法布局（主组本页面板 + 自生组同页面板）→ no-op", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    const p1 = addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    const stray = seedGroup(groups, "4"); // 分屏自生组（同页面板——合法）
    const p2 = addPanel(stray, panelIdInPage(PAGE_A, "editor-1"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    auditGroupMembership(api as never);

    expect(warnSpy).not.toHaveBeenCalled();
    expect(p1.api.moveTo).not.toHaveBeenCalled();
    expect(p2.api.moveTo).not.toHaveBeenCalled();
    expect(api.removeGroup).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("主组保留空壳（Watermark 载体不删）", () => {
    const { api, groups } = makeFakeHost();
    seedGroup(groups, pageGroupId(PAGE_A));

    auditGroupMembership(api as never);

    expect(api.removeGroup).not.toHaveBeenCalled();
  });

  it("自生空壳组 → removeGroup 清理", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    const stray = seedGroup(groups, "4"); // 拖拽拆分遗留空壳

    auditGroupMembership(api as never);

    expect(api.removeGroup).toHaveBeenCalledTimes(1);
    expect(api.removeGroup.mock.calls[0][0]).toBe(stray);
    expect(groups).toHaveLength(1);
  });

  it("主组混入他页面板 → moveTo 回迁该页组 + warn", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    const gB = seedGroup(groups, pageGroupId(PAGE_B));
    // page-b 面板混入 page-a 主组
    const alien = addPanel(gA, panelIdInPage(PAGE_B, "terminal-0"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    auditGroupMembership(api as never);

    expect(warnSpy).toHaveBeenCalledTimes(1);
    expect(warnSpy.mock.calls[0][0]).toContain("跨页组面板回迁");
    expect(alien.api.moveTo).toHaveBeenCalledTimes(1);
    expect(alien.api.moveTo.mock.calls[0][0].group).toBe(gB);
    warnSpy.mockRestore();
  });

  it("自生混组以首面板页为属主，少数派回迁各自页组", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    const gB = seedGroup(groups, pageGroupId(PAGE_B));
    addPanel(gB, panelIdInPage(PAGE_B, "terminal-0"));
    // 混组：首面板 page-a，混入 page-b 面板
    const stray = seedGroup(groups, "4");
    addPanel(stray, panelIdInPage(PAGE_A, "editor-1"));
    const alien = addPanel(stray, panelIdInPage(PAGE_B, "editor-2"));
    vi.spyOn(console, "warn").mockImplementation(() => {});

    auditGroupMembership(api as never);

    expect(alien.api.moveTo).toHaveBeenCalledTimes(1);
    expect(alien.api.moveTo.mock.calls[0][0].group).toBe(gB);
  });

  it("回迁目标页无任何组 → 滞留现组不移动（删页路径收口）", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    // page-c 面板混入 page-a 主组，但 page-c 无任何组
    addPanel(gA, panelIdInPage("page-c", "terminal-0"));
    vi.spyOn(console, "warn").mockImplementation(() => {});

    auditGroupMembership(api as never);

    // 滞留现组：无任何 moveTo（gA 内既有面板也不动）
    expect(gA.panels.every((p) => !p.api.moveTo.mock.calls.length)).toBe(true);
    expect(api.removeGroup).not.toHaveBeenCalled();
  });

  it("moveTo 抛异常 → 仅 console.error 不向上抛（守卫不破坏宿主事件流）", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    seedGroup(groups, pageGroupId(PAGE_B));
    addPanel(gA, panelIdInPage(PAGE_B, "terminal-0"),
      vi.fn(() => { throw new Error("move failed"); }));
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => auditGroupMembership(api as never)).not.toThrow();
    expect(errorSpy).toHaveBeenCalledTimes(1);
    expect(errorSpy.mock.calls[0][0]).toContain("面板回迁失败");
  });

  it("无页归属面板（无冒号 id）不移动（理论不可达防御）", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    const legacy = addPanel(gA, "terminal-page-a-0"); // 旧格式裸 id
    vi.spyOn(console, "warn").mockImplementation(() => {});

    auditGroupMembership(api as never);

    expect(legacy.api.moveTo).not.toHaveBeenCalled();
  });

  it("防复发（bug 2）：拖拽产自生组（移动期 add 事件被吞的终态）→ 审计保留组与面板", () => {
    // 老代码：enforcePanelGroupMembership 挂 onDidAddPanel——dockview `_moving`
    // 门控吞移动期事件，守卫对真实拖拽从不触发；叠加 maximizePageGroup 隐藏
    // 自生组 → 被拖面板「消失」。本用例模拟拖拽终态（自生组含本页面板）直调
    // 审计：组保留、面板不移动、不删组——分屏产物合法存续。
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    const stray = seedGroup(groups, "4"); // dockview 自增 id 分屏组
    const moved = addPanel(stray, panelIdInPage(PAGE_A, "terminal-1"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    auditGroupMembership(api as never);

    expect(warnSpy).not.toHaveBeenCalled();
    expect(moved.api.moveTo).not.toHaveBeenCalled();
    expect(api.removeGroup).not.toHaveBeenCalled();
    expect(groups.map((g) => g.id)).toEqual([pageGroupId(PAGE_A), "4"]);
    warnSpy.mockRestore();
  });

  it("审计重入安全（moveTo 内递归触发审计 → 幂等空扫退出）", () => {
    const { api, groups } = makeFakeHost();
    const gA = seedGroup(groups, pageGroupId(PAGE_A));
    addPanel(gA, panelIdInPage(PAGE_A, "terminal-0"));
    const gB = seedGroup(groups, pageGroupId(PAGE_B));
    // moveTo 副作用：面板改挂目标组 + 递归触发审计（模拟 onDidMovePanel 重入）
    const alien = addPanel(gA, panelIdInPage(PAGE_B, "terminal-0"));
    alien.api.moveTo = vi.fn(() => {
      gA.panels.splice(gA.panels.indexOf(alien), 1);
      gB.panels.push(alien);
      alien.group = gB;
      auditGroupMembership(api as never); // 重入——旗标拦截空扫
    });
    vi.spyOn(console, "warn").mockImplementation(() => {});

    expect(() => auditGroupMembership(api as never)).not.toThrow();
    expect(alien.api.moveTo).toHaveBeenCalledTimes(1);
    expect(gB.panels.map((p) => p.id)).toContain(panelIdInPage(PAGE_B, "terminal-0"));
  });
});
