// layoutSerde.test.ts — patchLegacyLayout + loadLayout + saveLayout 单元测试
//
// layoutSerde 覆盖率 0%，核心逻辑（旧格式修补、白名单校验、返回成败）完全无测试。

import { describe, it, expect, vi, beforeEach } from "vitest";
import type { DockviewApi } from "dockview-react";

// mock isValidPanelType（只认 terminal 和 editor）
vi.mock("../workspace/panelRegistry", () => ({
  isValidPanelType: (id: string) => id === "terminal" || id === "editor",
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
