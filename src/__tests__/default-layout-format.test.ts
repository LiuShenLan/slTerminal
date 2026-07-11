// default-layout-format.test.ts — makeEmptyLayout 格式验证
//
// 验证 SidebarTree.tsx 中 makeEmptyLayout 产出空布局对象，
// 新建页面不包含任何默认面板（terminal 等），由 Watermark 组件接管显示。

import { describe, it, expect } from "vitest";
import { makeEmptyLayout } from "../features/sidebar/SidebarTree";

describe("makeEmptyLayout 空布局验证", () => {
  it("T1: 返回值为空对象", () => {
    const layout = makeEmptyLayout();
    expect(layout).toEqual({});
  });

  it("T2: 两次调用返回独立对象（修改一个不影响另一个）", () => {
    const a = makeEmptyLayout() as Record<string, unknown>;
    const b = makeEmptyLayout() as Record<string, unknown>;

    (a as Record<string, unknown>).mutated = true;
    expect(a.mutated).toBe(true);
    expect((b as Record<string, unknown>).mutated).toBeUndefined();
  });

  it("T3: JSON 序列化+反序列化往返后仍为 {}", () => {
    const layout = makeEmptyLayout();
    const json = JSON.stringify(layout);
    expect(json).toBe("{}");
    const restored = JSON.parse(json);
    expect(restored).toEqual({});
  });

  it("T4: Object.keys 长度为 0", () => {
    const layout = makeEmptyLayout();
    expect(Object.keys(layout)).toHaveLength(0);
  });

  it("T5: 返回值非 null", () => {
    const layout = makeEmptyLayout();
    expect(layout).not.toBeNull();
  });

  it("T6: 返回值为纯对象（constructor === Object）", () => {
    const layout = makeEmptyLayout();
    expect(layout.constructor).toBe(Object);
  });

  it("T7: 无原型链属性污染", () => {
    const layout = makeEmptyLayout();
    // 仅自有属性（非继承）
    expect(layout.hasOwnProperty("toString")).toBe(false);
    expect(layout.hasOwnProperty("__proto__")).toBe(false);
  });

  it("T8: typeof 为 'object'", () => {
    const layout = makeEmptyLayout();
    expect(typeof layout).toBe("object");
    // 排除数组和 null
    expect(Array.isArray(layout)).toBe(false);
    expect(layout).not.toBeNull();
  });
});
