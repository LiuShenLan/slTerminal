// layoutSerde.test.ts — patchLegacyLayout + loadLayout + saveLayout 单元测试

import { describe, it, expect, vi, beforeEach } from "vitest";
import type { DockviewApi } from "dockview-react";

// mock isValidPanelType（与真实 PANEL_TYPES 对齐：terminal / editor / htmlviewer）
vi.mock("../panelRegistry", () => ({
  isValidPanelType: (id: string) => ["terminal", "editor", "htmlviewer"].includes(id),
}));

import { loadLayout, saveLayout } from "../workspace/layoutSerde";

/** 构造 mock DockviewApi */
function mockApi(): DockviewApi {
  return {
    toJSON: vi.fn(() => ({ mock: "layout" })),
    fromJSON: vi.fn(),
    clear: vi.fn(),
    dispose: vi.fn(),
  } as unknown as DockviewApi;
}

// ====== patchLegacyLayout（通过 loadLayout 间接测试）======

describe("patchLegacyLayout — 旧格式修补（通过 loadLayout）", () => {
  let api: DockviewApi;

  beforeEach(() => {
    api = mockApi();
  });

  it("1. 旧格式 component→contentComponent 迁移", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["p1"], activeView: "p1", id: "g1" },
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        p1: { id: "p1", component: "terminal", params: {}, renderer: "always" },
      },
      activeGroup: "g1",
    };

    const result = loadLayout(api, layout);
    expect(result).toBe(true);

    // 验证 fromJSON 收到的 layout 中 component 已迁移为 contentComponent
    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(callArg.panels.p1.contentComponent).toBe("terminal");
    expect(callArg.panels.p1.component).toBeUndefined();
  });

  it("2. 已有 contentComponent 的 panel 不受影响", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["p2"], activeView: "p2", id: "g2" },
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        p2: { id: "p2", contentComponent: "terminal", params: {}, renderer: "always" },
      },
      activeGroup: "g2",
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(callArg.panels.p2.contentComponent).toBe("terminal");
    expect(callArg.panels.p2.component).toBeUndefined();
  });

  it("3. leaf.data 缺少 id → 根据 views[0] 生成 group id", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["p3"], activeView: "p3" }, // 无 id
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        p3: { id: "p3", contentComponent: "terminal", params: {}, renderer: "always" },
      },
      // 无 activeGroup
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(callArg.grid.root.data[0].data.id).toBe("group-p3");
    expect(callArg.activeGroup).toBe("group-p3");
  });

  it("4. leaf.data 缺少 id 且 views 为空 → 回退 group-default", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: [], activeView: "" }, // views 为空数组
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {},
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(callArg.grid.root.data[0].data.id).toBe("group-default");
  });

  it("5. 顶层 activeGroup 缺失 → 用第一个 leaf 的 id 填充", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { views: ["p5a"], activeView: "p5a", id: "first-group" }, size: 50 },
            { type: "leaf", data: { views: ["p5b"], activeView: "p5b", id: "second-group" }, size: 50 },
          ],
          size: 100,
        },
      },
      panels: {
        p5a: { id: "p5a", contentComponent: "terminal", params: {} },
        p5b: { id: "p5b", contentComponent: "terminal", params: {} },
      },
      // 无 activeGroup
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    // 第一个 leaf 的 id 用于 activeGroup
    expect(callArg.activeGroup).toBe("first-group");
  });

  it("6. grid.root 缺失 — 不崩溃", () => {
    const layout: Record<string, unknown> = {
      panels: { p6: { id: "p6", component: "terminal" } },
    };

    expect(() => loadLayout(api, layout)).not.toThrow();
  });

  it("7. grid.root.type 为 leaf（非 branch）— 不崩溃", () => {
    const layout = {
      grid: { root: { type: "leaf", data: {} } },
      panels: {},
    };

    expect(() => loadLayout(api, layout)).not.toThrow();
  });

  it("7a. grid.orientation 缺失 → 自动补为 'HORIZONTAL'", () => {
    const layout = {
      grid: {
        // 无 orientation 字段 — 旧版本 makeDefaultLayout 产出的格式
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["p7a"], activeView: "p7a", id: "g7a" },
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        p7a: { id: "p7a", contentComponent: "terminal", params: {}, renderer: "always" },
      },
      activeGroup: "g7a",
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    const grid = callArg.grid as Record<string, unknown>;
    expect(grid.orientation).toBe("HORIZONTAL");
  });

  it("7b. grid.orientation 已存在 → 原值保留", () => {
    const layout = {
      grid: {
        orientation: "VERTICAL", // 刻意非 HORIZONTAL
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["p7b"], activeView: "p7b", id: "g7b" },
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        p7b: { id: "p7b", contentComponent: "terminal", params: {} },
      },
      activeGroup: "g7b",
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    const grid = callArg.grid as Record<string, unknown>;
    expect(grid.orientation).toBe("VERTICAL");
  });
});

// ====== loadLayout 返回值和校验 ======

describe("loadLayout — 返回值 + 白名单校验", () => {
  let api: DockviewApi;

  beforeEach(() => {
    api = mockApi();
  });

  it("8. 有效布局 → 返回 true + 调用 fromJSON", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["test1"], activeView: "test1", id: "g1" },
            size: 100,
          }],
          size: 100,
        },
      },
      panels: { test1: { id: "test1", contentComponent: "terminal", params: {} } },
      activeGroup: "g1",
    };

    const result = loadLayout(api, layout);
    expect(result).toBe(true);
    expect(api.fromJSON).toHaveBeenCalledTimes(1);
  });

  it("9. panels 中包含无效组件 → 过滤后 fromJSON 仍调用", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["valid"], activeView: "valid", id: "g2" },
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        valid: { id: "valid", contentComponent: "terminal" },
        invalid: { id: "invalid", contentComponent: "unknown_component" },
      },
      activeGroup: "g2",
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(callArg.panels.valid).toBeDefined();
    expect(callArg.panels.invalid).toBeUndefined();
  });

  it("10. fromJSON 抛异常 → 返回 false", () => {
    (api.fromJSON as ReturnType<typeof vi.fn>).mockImplementation(() => {
      throw new Error("dockview: grid error");
    });

    const layout = {
      grid: { root: { type: "branch", data: [], size: 0 } },
      panels: {},
    };

    const result = loadLayout(api, layout);
    expect(result).toBe(false);
  });

  it("11. 空 layout 对象 → 返回 true（无 panels 不需过滤）", () => {
    const result = loadLayout(api, {} as object);
    expect(result).toBe(true);
    expect(api.fromJSON).toHaveBeenCalledTimes(1);
  });

  it("12. 深拷贝：loadLayout 不修改原始 saved 对象", () => {
    const original: Record<string, unknown> = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["cp"], activeView: "cp" }, // 缺 id
            size: 100,
          }],
          size: 100,
        },
      },
      panels: { cp: { id: "cp", component: "terminal" } },
    };

    const saved = JSON.parse(JSON.stringify(original));
    loadLayout(api, saved);

    // 原始对象不应被修改
    const leafData = (
      (original.grid as Record<string, unknown>).root as Record<string, unknown>
    ).data as Record<string, unknown>[];
    expect(leafData[0].data).not.toHaveProperty("id");
    // activeGroup 不应出现
    expect(original).not.toHaveProperty("activeGroup");
  });

  it("13. 同时缺 component/contentComponent → 不过滤该 panel", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "leaf",
            data: { views: ["nc"], activeView: "nc", id: "gx" },
            size: 100,
          }],
          size: 100,
        },
      },
      panels: { nc: { id: "nc" } },
      activeGroup: "gx",
    };
    const result = loadLayout(api, layout);
    expect(result).toBe(true);
  });
});

// ====== saveLayout ======

describe("saveLayout", () => {
  it("14. saveLayout 委托 api.toJSON()", () => {
    const api = mockApi();
    const result = saveLayout(api);
    expect(api.toJSON).toHaveBeenCalledTimes(1);
    expect(result).toEqual({ mock: "layout" });
  });
});

// ====== TE-06: 嵌套 branch 修补 + activeGroup 保留 ======

describe("patchLegacyLayout — 嵌套 branch + activeGroup 保留", () => {
  let api: DockviewApi;

  beforeEach(() => {
    api = mockApi();
  });

  it("15. 两层嵌套 branch（root branch → child branch → leaf）：不崩溃，嵌套 leaf 不修补", () => {
    // patchLegacyLayout 只遍历 root.data 的直接子节点，嵌套 branch 被跳过
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [{
            type: "branch",
            data: [{
              type: "leaf",
              data: { views: ["nested-p1"], activeView: "nested-p1" }, // 缺 id，不会被修补
              size: 100,
            }],
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        "nested-p1": { id: "nested-p1", component: "terminal", params: {} },
      },
      // 无 activeGroup — 不会被设置（无直接 leaf 子节点）
    };

    expect(() => loadLayout(api, layout)).not.toThrow();

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    const callLayout = callArg as Record<string, unknown>;

    // grid.orientation 仍补齐
    expect((callLayout.grid as Record<string, unknown>).orientation).toBe("HORIZONTAL");

    // 嵌套 branch 内的 leaf.data.id 未被修补
    const root = (callLayout.grid as Record<string, unknown>).root as Record<string, unknown>;
    const rootChildren = root.data as Record<string, unknown>[];
    const nestedBranch = rootChildren[0] as Record<string, unknown>;
    const nestedChildren = nestedBranch.data as Record<string, unknown>[];
    const nestedLeaf = nestedChildren[0].data as Record<string, unknown>;
    expect(nestedLeaf.id).toBeUndefined();

    // activeGroup 未被设置（无直接 leaf）
    expect(callLayout.activeGroup).toBeUndefined();
  });

  it("16. 同层混合 branch + leaf：直接 leaf 被修补，嵌套 leaf 跳过，activeGroup 来自直接 leaf", () => {
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [
            {
              type: "leaf",
              data: { views: ["direct-p1"], activeView: "direct-p1" }, // 缺 id → 会被修补
              size: 50,
            },
            {
              type: "branch",
              data: [{
                type: "leaf",
                data: { views: ["nested-p2"], activeView: "nested-p2" }, // 缺 id → 不会被修补
                size: 100,
              }],
              size: 50,
            },
          ],
          size: 100,
        },
      },
      panels: {
        "direct-p1": { id: "direct-p1", component: "terminal", params: {} },
        "nested-p2": { id: "nested-p2", component: "terminal", params: {} },
      },
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    const root = (callArg.grid as Record<string, unknown>).root as Record<string, unknown>;
    const children = root.data as Record<string, unknown>[];

    // 直接 leaf 的 id 被修补
    const directLeaf = children[0].data as Record<string, unknown>;
    expect(directLeaf.id).toBe("group-direct-p1");

    // 嵌套 branch 内的 leaf 未被修补
    const nestedBranch = children[1] as Record<string, unknown>;
    const nestedChildren = nestedBranch.data as Record<string, unknown>[];
    const nestedLeaf = nestedChildren[0].data as Record<string, unknown>;
    expect(nestedLeaf.id).toBeUndefined();

    // activeGroup 来自第一个直接 leaf
    const callLayout = callArg as Record<string, unknown>;
    expect(callLayout.activeGroup).toBe("group-direct-p1");
  });

  it("17. activeGroup 已存在 → 保留原值不被覆盖", () => {
    // 布局中第二个 leaf 的 id 更靠前，但 activeGroup 已指定为第一个 leaf
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { views: ["p1"], activeView: "p1", id: "group-p1" }, size: 50 },
            { type: "leaf", data: { views: ["p2"], activeView: "p2", id: "group-p2" }, size: 50 },
          ],
          size: 100,
        },
      },
      panels: {
        p1: { id: "p1", contentComponent: "terminal", params: {} },
        p2: { id: "p2", contentComponent: "terminal", params: {} },
      },
      activeGroup: "group-p2", // 已存在 → 不被覆盖
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    const callLayout = callArg as Record<string, unknown>;
    expect(callLayout.activeGroup).toBe("group-p2");
  });

  it("18. activeGroup 缺失 → 设为第一个直接 leaf 的 id", () => {
    // 三个 leaf，activeGroup 应设为第一个的 id
    const layout = {
      grid: {
        root: {
          type: "branch",
          data: [
            { type: "leaf", data: { views: ["a"], activeView: "a", id: "ga" }, size: 33 },
            { type: "leaf", data: { views: ["b"], activeView: "b", id: "gb" }, size: 33 },
            { type: "leaf", data: { views: ["c"], activeView: "c", id: "gc" }, size: 34 },
          ],
          size: 100,
        },
      },
      panels: {
        a: { id: "a", contentComponent: "terminal", params: {} },
        b: { id: "b", contentComponent: "terminal", params: {} },
        c: { id: "c", contentComponent: "terminal", params: {} },
      },
      // 无 activeGroup
    };

    loadLayout(api, layout);

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    const callLayout = callArg as Record<string, unknown>;
    // 始终取第一个 leaf 的 id
    expect(callLayout.activeGroup).toBe("ga");
  });

  it("19. 三层嵌套 branch 不崩溃，grid.orientation 补齐", () => {
    const layout = {
      grid: {
        // 无 orientation
        root: {
          type: "branch",
          data: [{
            type: "branch",
            data: [{
              type: "branch",
              data: [{
                type: "leaf",
                data: { views: ["deep"], activeView: "deep" },
                size: 100,
              }],
              size: 100,
            }],
            size: 100,
          }],
          size: 100,
        },
      },
      panels: {
        deep: { id: "deep", component: "terminal", params: {} },
      },
    };

    expect(() => loadLayout(api, layout)).not.toThrow();

    const callArg = (api.fromJSON as ReturnType<typeof vi.fn>).mock.calls[0][0];
    const grid = (callArg as Record<string, unknown>).grid as Record<string, unknown>;
    expect(grid.orientation).toBe("HORIZONTAL");
  });
});
