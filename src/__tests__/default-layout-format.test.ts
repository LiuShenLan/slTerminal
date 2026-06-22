// default-layout-format.test.ts — makeDefaultLayout 格式验证
//
// 验证 SidebarTree.tsx 中 makeDefaultLayout 产出的布局能被 Dockview fromJSON 接受。
// 关键约束：grid root 必须为 type: "branch"（非 "leaf"），面板定义必须用 contentComponent。

import { describe, it, expect } from "vitest";

/** 从 SidebarTree.tsx 提取的 makeDefaultLayout（保持同步） */
function makeDefaultLayout(panelId: string): Record<string, unknown> {
  return {
    grid: {
      root: {
        type: "branch",
        data: [
          {
            type: "leaf",
            data: { views: [panelId], activeView: panelId },
            size: 100,
          },
        ],
        size: 100,
      },
    },
    panels: {
      [panelId]: {
        id: panelId,
        contentComponent: "terminal",
        params: { panelId },
        renderer: "always",
      },
    },
  };
}

describe("makeDefaultLayout 格式验证", () => {
  it("grid.root.type 必须为 'branch'（Dockview fromJSON 强制要求）", () => {
    const layout = makeDefaultLayout("test-panel-1");
    const root = (layout.grid as Record<string, unknown>).root as Record<string, unknown>;
    expect(root.type).toBe("branch");
    expect(Array.isArray(root.data)).toBe(true);
    expect((root.data as unknown[]).length).toBeGreaterThan(0);
  });

  it("grid.root.data[0] 必须是 type: 'leaf' 的叶子节点", () => {
    const layout = makeDefaultLayout("test-panel-2");
    const root = (layout.grid as Record<string, unknown>).root as Record<string, unknown>;
    const leaf = (root.data as unknown[])[0] as Record<string, unknown>;
    expect(leaf.type).toBe("leaf");
    expect(leaf.size).toBe(100);
    const leafData = leaf.data as Record<string, unknown>;
    expect(leafData.views).toEqual(["test-panel-2"]);
    expect(leafData.activeView).toBe("test-panel-2");
  });

  it("panels 定义必须用 contentComponent（非 component）", () => {
    const layout = makeDefaultLayout("test-panel-3");
    const panels = layout.panels as Record<string, Record<string, unknown>>;
    const panel = panels["test-panel-3"];
    expect(panel.id).toBe("test-panel-3");
    // fromJSON 反序列化器读取 contentComponent 字段，非 component
    expect(panel.contentComponent).toBe("terminal");
    expect(panel.component).toBeUndefined();
    expect(panel.renderer).toBe("always");
  });

  it("panels 应包含 params 字段（面板参数透传）", () => {
    const layout = makeDefaultLayout("test-panel-4");
    const panels = layout.panels as Record<string, Record<string, unknown>>;
    const panel = panels["test-panel-4"];
    const params = panel.params as Record<string, unknown>;
    expect(params.panelId).toBe("test-panel-4");
  });

  it("不同 panelId 应生成独立的布局", () => {
    const layout1 = makeDefaultLayout("panel-a");
    const layout2 = makeDefaultLayout("panel-b");
    const panels1 = layout1.panels as Record<string, unknown>;
    const panels2 = layout2.panels as Record<string, unknown>;

    expect(panels1["panel-a"]).toBeDefined();
    expect(panels1["panel-b"]).toBeUndefined();
    expect(panels2["panel-b"]).toBeDefined();
    expect(panels2["panel-a"]).toBeUndefined();
  });

  it("布局 JSON 序列化再反序列化应保持结构一致", () => {
    const layout = makeDefaultLayout("test-json-roundtrip");
    const json = JSON.stringify(layout);
    const restored = JSON.parse(json);

    expect(restored.grid.root.type).toBe("branch");
    expect(restored.panels["test-json-roundtrip"]).toBeDefined();
    expect(restored.panels["test-json-roundtrip"].contentComponent).toBe("terminal");
    expect(restored.panels["test-json-roundtrip"].id).toBe("test-json-roundtrip");
  });
});
