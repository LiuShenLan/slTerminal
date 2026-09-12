/* eslint-disable @typescript-eslint/no-explicit-any */
// layoutSerde 页组契约测试（CP-004/S11）——normalizePageLayout 旧多实例格式迁移
// （归组成功/脏面板丢弃两分支）、composeHostLayout 宿主汇编、slicePageLayout 切分
// 往返、loadPageGroup 运行期挂载、emptyPageLayout 空页切片
import { describe, it, expect, vi, beforeEach } from "vitest";
import type { DockviewApi } from "dockview-react";
import {
  emptyPageLayout,
  normalizePageLayout,
  composeHostLayout,
  slicePageLayout,
  loadPageGroup,
  saveLayout,
} from "../workspace/layoutSerde";
import { pageGroupId, panelIdInPage } from "../workspace/pageGroups";

const PAGE = "page-1788531106918-2";
const GID = pageGroupId(PAGE);

/** 构造 mock DockviewApi */
function mockApi(panels?: Record<string, unknown>): DockviewApi {
  return {
    toJSON: vi.fn(() => ({
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { id: GID, views: Object.keys(panels ?? {}) }, size: 50 },
            { type: "leaf", data: { id: pageGroupId("page-b"), views: [] }, size: 50 },
          ],
        },
      },
      panels: panels ?? {},
      activeGroup: GID,
    })),
    fromJSON: vi.fn(),
    clear: vi.fn(),
    dispose: vi.fn(),
  } as unknown as DockviewApi;
}

describe("emptyPageLayout — 空页切片", () => {
  it("空页切片含 page- 前缀空 views 页组", () => {
    const s = emptyPageLayout(PAGE) as { grid: any; panels: unknown; activeGroup: string };
    expect(s.activeGroup).toBe(GID);
    expect(s.grid.root.data[0].data.id).toBe(GID);
    expect(s.grid.root.data[0].data.views).toEqual([]);
    expect(s.panels).toEqual({});
  });
});

describe("normalizePageLayout — 旧多实例格式迁移", () => {
  it("旧格式多组布局压平归组成功(组 id 重写 + 面板 id 页前缀化 + params.panelId 同步)", () => {
    const legacy = {
      grid: {
        orientation: "VERTICAL",
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { id: "group-t", views: ["terminal-1-0"] }, size: 50 },
            {
              type: "branch",
              data: [
                { type: "leaf", data: { id: "group-e", views: ["editor-x"] }, size: 50 },
              ],
              size: 50,
            },
          ],
        },
      },
      panels: {
        "terminal-1-0": {
          id: "terminal-1-0", contentComponent: "terminal", title: "terminal-0",
          params: { panelId: "terminal-1-0" }, renderer: "always",
        },
        "editor-x": {
          id: "editor-x", contentComponent: "editor", title: "x.ts",
          params: { panelId: "editor-x", filePath: "C:/x.ts" },
        },
      },
      activeGroup: "group-t",
    };

    const slice = normalizePageLayout(PAGE, legacy) as any;
    // 单页组 + 全部面板按序遍历并入
    expect(slice.grid.root.data).toHaveLength(1);
    const leaf = slice.grid.root.data[0];
    expect(leaf.data.id).toBe(GID);
    expect(leaf.data.views).toEqual([
      panelIdInPage(PAGE, "terminal-1-0"),
      panelIdInPage(PAGE, "editor-x"),
    ]);
    expect(Object.keys(slice.panels).sort()).toEqual([
      panelIdInPage(PAGE, "editor-x"),
      panelIdInPage(PAGE, "terminal-1-0"),
    ]);
    // 面板状态 id 与 params.panelId 同步改写
    expect(slice.panels[panelIdInPage(PAGE, "terminal-1-0")].id)
      .toBe(panelIdInPage(PAGE, "terminal-1-0"));
    expect(slice.panels[panelIdInPage(PAGE, "terminal-1-0")].params.panelId)
      .toBe(panelIdInPage(PAGE, "terminal-1-0"));
    expect(slice.panels[panelIdInPage(PAGE, "editor-x")].params.filePath).toBe("C:/x.ts");
    expect(slice.activeGroup).toBe(GID);
  });

  it("迁移不修改原始 saved 对象(深拷贝)", () => {
    const legacy = {
      grid: {
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { id: "g1", views: ["terminal-1-0"] }, size: 100 },
          ],
        },
      },
      panels: {
        "terminal-1-0": {
          id: "terminal-1-0", component: "terminal", // 旧 component 字段
          params: { panelId: "terminal-1-0" },
        },
      },
      activeGroup: "g1",
    };
    const original = JSON.parse(JSON.stringify(legacy));
    normalizePageLayout(PAGE, legacy);
    expect(legacy).toEqual(original);
  });

  it("脏面板丢弃分支:白名单外类型 + 幽灵视图引用 → 剔除 + console.error,不阻断", () => {
    const legacy = {
      grid: {
        root: {
          type: "branch",
          data: [
            {
              type: "leaf", id: undefined,
              data: { id: "g1", views: ["terminal-1-0", "ghost-ref", "bad-comp"] },
              size: 100,
            },
          ],
        },
      },
      panels: {
        "terminal-1-0": {
          id: "terminal-1-0", contentComponent: "terminal", params: { panelId: "terminal-1-0" },
        },
        "bad-comp": {
          id: "bad-comp", contentComponent: "unknown_component", params: { panelId: "bad-comp" },
        },
        // ghost-ref 无 panels 条目
      },
      activeGroup: "g1",
    };
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const slice = normalizePageLayout(PAGE, legacy) as any;
    // 归组成功部分保留;幽灵引用与白名单外类型丢弃
    expect(slice.grid.root.data[0].data.views).toEqual([panelIdInPage(PAGE, "terminal-1-0")]);
    expect(Object.keys(slice.panels)).toEqual([panelIdInPage(PAGE, "terminal-1-0")]);
    expect(errorSpy).toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("空占位({}/无 grid 无 panels)→ 空页切片", () => {
    expect((normalizePageLayout(PAGE, {}) as any).grid.root.data[0].data.views).toEqual([]);
    const s = normalizePageLayout(PAGE, { panels: {} }) as any;
    expect(s.grid.root.data[0].data.views).toEqual([]);
  });

  it("新格式切片(含 page- 页组)→ 规范化保留视图顺序", () => {
    const pa = panelIdInPage(PAGE, "a");
    const pb = panelIdInPage(PAGE, "b");
    const slice = {
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { id: GID, views: [pa, pb] }, size: 30 },
            { type: "leaf", data: { id: pageGroupId("other"), views: [] }, size: 70 },
          ],
        },
      },
      panels: {
        [pa]: { id: pa, contentComponent: "terminal", params: {} },
        [pb]: { id: pb, contentComponent: "terminal", params: {} },
      },
      activeGroup: GID,
    };
    const out = normalizePageLayout(PAGE, slice) as any;
    expect(out.grid.root.data).toHaveLength(1);
    expect(out.grid.root.data[0].data.views).toEqual([pa, pb]);
    expect(out.grid.root.data[0].data.activeView).toBe(pa);
    // 他页组面板不混入
    expect(Object.keys(out.panels).sort()).toEqual([pa, pb]);
  });
});

describe("composeHostLayout — 宿主全量汇编", () => {
  it("多页切片合并为根 branch 多页组 + panels 并集 + activeGroup 指向活跃页", () => {
    const p1 = emptyPageLayout(PAGE);
    const p2Slice = normalizePageLayout("page-b", {
      grid: {
        root: {
          type: "branch",
          data: [{ type: "leaf", data: { id: "gx", views: ["terminal-b-0"] }, size: 100 }],
        },
      },
      panels: {
        "terminal-b-0": {
          id: "terminal-b-0", contentComponent: "terminal", params: { panelId: "terminal-b-0" },
        },
      },
      activeGroup: "gx",
    });
    const host = composeHostLayout(
      [{ pageId: PAGE, layout: p1 }, { pageId: "page-b", layout: p2Slice }],
      PAGE,
    ) as any;
    const ids = host.grid.root.data.map((l: any) => l.data.id);
    expect(ids).toEqual([GID, pageGroupId("page-b")]);
    expect(host.panels).toHaveProperty(panelIdInPage("page-b", "terminal-b-0"));
    expect(host.activeGroup).toBe(GID);
  });

  it("无活跃页 → activeGroup 指向第一页页组", () => {
    const host = composeHostLayout(
      [{ pageId: PAGE, layout: emptyPageLayout(PAGE) }],
      null,
    ) as any;
    expect(host.activeGroup).toBe(GID);
  });

  it("页切片损坏(null/数组)→ 该页按空页组并入,不阻断其余页", () => {
    const host = composeHostLayout(
      [{ pageId: PAGE, layout: null }, { pageId: "page-b", layout: {} }],
      "page-b",
    ) as any;
    expect(host.grid.root.data).toHaveLength(2);
    expect(host.activeGroup).toBe(pageGroupId("page-b"));
  });
});

describe("slicePageLayout — 全量 JSON 按页切分", () => {
  it("从宿主全量提取目标页切片(往返:compose → slice 各页自洽)", () => {
    const a = emptyPageLayout(PAGE);
    const bSlice = normalizePageLayout("page-b", {
      grid: {
        root: {
          type: "branch",
          data: [
            {
              type: "leaf",
              data: { id: "g1", views: ["terminal-b-0"], activeView: "terminal-b-0" },
              size: 100,
            },
          ],
        },
      },
      panels: {
        "terminal-b-0": {
          id: "terminal-b-0", contentComponent: "terminal", params: { panelId: "terminal-b-0" },
        },
      },
      activeGroup: "g1",
    }) as Record<string, unknown>;
    const host = composeHostLayout(
      [{ pageId: PAGE, layout: a }, { pageId: "page-b", layout: bSlice }],
      PAGE,
    );
    const sliceB = slicePageLayout("page-b", host) as any;
    expect(sliceB.grid.root.data[0].data.id).toBe(pageGroupId("page-b"));
    expect(sliceB.grid.root.data[0].data.views).toEqual([panelIdInPage("page-b", "terminal-b-0")]);
    expect(sliceB.panels).toHaveProperty(panelIdInPage("page-b", "terminal-b-0"));
    expect(sliceB.panels).not.toHaveProperty(GID);
  });

  it("宿主无该页页组 → 空页切片", () => {
    const s = slicePageLayout("page-nope", { grid: { root: { type: "branch", data: [] } }, panels: {} }) as any;
    expect(s.grid.root.data[0].data.views).toEqual([]);
  });

  it("页内分屏多叶切片（ADR-0020）：自生组叶按 views 归属保留 + 保序 + activeGroup 属本页保留", () => {
    const t0 = panelIdInPage(PAGE, "terminal-0");
    const e1 = panelIdInPage(PAGE, "editor-1");
    const host = {
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { id: GID, views: [t0], activeView: t0 } },
            // 分屏自生组（dockview 自增 id 无 page- 前缀）——views 慢车道归属本页
            { type: "leaf", data: { id: "4", views: [e1], activeView: e1 } },
            { type: "leaf", data: { id: pageGroupId("page-b"), views: [panelIdInPage("page-b", "terminal-0")] } },
          ],
        },
      },
      panels: {
        [t0]: { id: t0, contentComponent: "terminal", params: {} },
        [e1]: { id: e1, contentComponent: "editor", params: {} },
        [panelIdInPage("page-b", "terminal-0")]: { id: panelIdInPage("page-b", "terminal-0"), contentComponent: "terminal", params: {} },
      },
      activeGroup: "4",
    };
    const slice = slicePageLayout(PAGE, host) as any;
    // 本页两叶保序（主组 + 自生组），他页叶剔除
    expect(slice.grid.root.data.map((l: any) => l.data.id)).toEqual([GID, "4"]);
    expect(Object.keys(slice.panels).sort()).toEqual([e1, t0].sort());
    // 宿主 activeGroup "4" 属本页 → 保留
    expect(slice.activeGroup).toBe("4");
  });

  it("切片剥 visible 标记 + activeView 归位 views[0]（防恢复恒隐藏/激活不确定）", () => {
    const t0 = panelIdInPage(PAGE, "terminal-0");
    const e1 = panelIdInPage(PAGE, "editor-1");
    const host = {
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [
            // 宿主 toJSON 隐藏叶带 visible:false；activeView 失效（不在 views）
            { type: "leaf", data: { id: GID, views: [t0, e1], activeView: "ghost", visible: false } },
          ],
        },
      },
      panels: {
        [t0]: { id: t0, contentComponent: "terminal", params: {} },
        [e1]: { id: e1, contentComponent: "editor", params: {} },
      },
      activeGroup: GID,
    };
    const slice = slicePageLayout(PAGE, host) as any;
    const leaf = slice.grid.root.data[0];
    expect(leaf.data.visible).toBeUndefined();
    expect(leaf.data.activeView).toBe(t0);
    // 输入不被污染（共享引用防御——宿主全量多页切片共用）
    expect((host.grid.root.data[0].data as any).visible).toBe(false);
  });

  it("嵌套 branch 剪枝：他页叶剔除 + 全空 branch 清除 + 单子 branch 展平", () => {
    const t0 = panelIdInPage(PAGE, "terminal-0");
    const host = {
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [
            {
              type: "branch",
              data: [
                { type: "leaf", data: { id: GID, views: [t0] } },
                { type: "leaf", data: { id: pageGroupId("page-b"), views: [] } },
              ],
            },
            { type: "branch", data: [{ type: "leaf", data: { id: pageGroupId("page-c"), views: [] } }] },
          ],
        },
      },
      panels: { [t0]: { id: t0, contentComponent: "terminal", params: {} } },
      activeGroup: GID,
    };
    const slice = slicePageLayout(PAGE, host) as any;
    // 嵌套 branch 内他页叶剔除 → 单子展平；全空 branch 整支清除
    expect(slice.grid.root.data).toHaveLength(1);
    expect(slice.grid.root.data[0].data.id).toBe(GID);
  });

  it("分屏切片往返恒等：compose 摊平直挂 → slice 回切形态自洽", () => {
    const t0 = panelIdInPage(PAGE, "terminal-0");
    const e1 = panelIdInPage(PAGE, "editor-1");
    const splitSlice = {
      grid: {
        orientation: "HORIZONTAL",
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { id: GID, views: [t0], activeView: t0 } },
            { type: "leaf", data: { id: "4", views: [e1], activeView: e1 } },
          ],
        },
      },
      panels: {
        [t0]: { id: t0, contentComponent: "terminal", params: {} },
        [e1]: { id: e1, contentComponent: "editor", params: {} },
      },
      activeGroup: "4",
    };
    const host = composeHostLayout([{ pageId: PAGE, layout: splitSlice }], PAGE);
    const back = slicePageLayout(PAGE, host) as any;
    expect(back.grid.root.data.map((l: any) => l.data.id)).toEqual([GID, "4"]);
    expect(back.activeGroup).toBe("4");
    // 再 normalize 幂等（存取链路 normalize → slice 双闸自洽）
    const renorm = normalizePageLayout(PAGE, back) as any;
    expect(renorm.grid.root.data.map((l: any) => l.data.id)).toEqual([GID, "4"]);
  });
});

describe("loadPageGroup — 运行期页组挂载", () => {
  let api: DockviewApi;
  beforeEach(() => { api = mockApi(); });

  it("页切片并入当前宿主后 fromJSON(reuseExistingPanels) 且 activeGroup 指向新页", () => {
    api = mockApi({
      "p-existing": { id: "p-existing", contentComponent: "terminal", params: {} },
    });
    const slice = normalizePageLayout("page-c", {
      grid: {
        root: {
          type: "branch",
          data: [{ type: "leaf", data: { id: "g1", views: ["terminal-c-0"] }, size: 100 }],
        },
      },
      panels: {
        "terminal-c-0": {
          id: "terminal-c-0", contentComponent: "terminal", params: { panelId: "terminal-c-0" },
        },
      },
      activeGroup: "g1",
    });
    const ok = loadPageGroup(api, "page-c", slice);
    expect(ok).toBe(true);
    expect(api.fromJSON).toHaveBeenCalledTimes(1);
    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0];
    // reuseExistingPanels 语义不变
    expect(callArg[1]).toEqual({ reuseExistingPanels: true });
    const merged = callArg[0];
    const ids = merged.grid.root.data.map((l: any) => l.data.id);
    // 既有页组保留 + 新页组并入
    expect(ids).toEqual([GID, pageGroupId("page-b"), pageGroupId("page-c")]);
    expect(merged.activeGroup).toBe(pageGroupId("page-c"));
    expect(merged.panels).toHaveProperty(panelIdInPage("page-c", "terminal-c-0"));
    // 既有面板状态保留(来自 toJSON mock)
    expect(merged.panels).toHaveProperty("p-existing");
  });

  it("重复挂载同一页幂等(不产生双叶)", () => {
    const slice = emptyPageLayout("page-d");
    loadPageGroup(api, "page-d", slice);
    loadPageGroup(api, "page-d", slice);
    const allCalls = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls;
    const lastArg = allCalls[allCalls.length - 1][0];
    const ids = lastArg.grid.root.data.map((l: any) => l.data.id);
    expect(ids.filter((x: string) => x === pageGroupId("page-d"))).toHaveLength(1);
  });

  it("空占位 {} 布局 → 空页组挂载成功", () => {
    expect(loadPageGroup(api, "page-e", {})).toBe(true);
  });
});

describe("saveLayout — 全量委托不变", () => {
  it("saveLayout 委托 api.toJSON()", () => {
    const api = mockApi();
    const result = saveLayout(api);
    expect(api.toJSON).toHaveBeenCalledTimes(1);
    expect(result).toBeDefined();
  });
});
