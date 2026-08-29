// settings-page-registry.test.ts — SettingsPageRegistry 单测（F11，硬约束 #13 家族契约）
//
// 覆盖：注册/getAll 分组过滤/order 排序缺省注册序/重复 id 覆盖/_reset 隔离/惰性单例。
// 契约要点：register 同 id 幂等覆盖（后注册者胜）；getAll(group?) 按 order ?? 注册序
// 稳定排序；_reset 仅测试用（beforeEach 调用保证用例隔离）。

import { describe, it, expect, beforeEach, vi } from "vitest";
import { getSettingsPageRegistry } from "../features/settingsCenter/SettingsPageRegistry";
import type { SettingsPage, SettingsPageGroup } from "../features/settingsCenter/types";

/** 构造测试配置页——component 用空组件桩 */
function makePage(
  id: string,
  group: SettingsPageGroup,
  overrides: Partial<SettingsPage> = {},
): SettingsPage {
  return {
    id,
    title: `页-${id}`,
    group,
    component: vi.fn(() => null),
    ...overrides,
  };
}

describe("SettingsPageRegistry", () => {
  beforeEach(() => {
    getSettingsPageRegistry()._reset();
  });

  it("注册后 getAll 按注册序返回（缺省 order）", () => {
    const reg = getSettingsPageRegistry();
    reg.register(makePage("a", "global"));
    reg.register(makePage("b", "project"));
    reg.register(makePage("c", "global"));

    const all = reg.getAll();
    expect(all.map((p) => p.id)).toEqual(["a", "b", "c"]);
  });

  it("getAll(group) 分组过滤——global/project 各只含本组", () => {
    const reg = getSettingsPageRegistry();
    reg.register(makePage("a", "global"));
    reg.register(makePage("b", "project"));
    reg.register(makePage("c", "project"));

    expect(reg.getAll("global").map((p) => p.id)).toEqual(["a"]);
    expect(reg.getAll("project").map((p) => p.id)).toEqual(["b", "c"]);
  });

  it("order 排序——有序页按 order 升序在前，缺省 order 页保持注册序在后", () => {
    const reg = getSettingsPageRegistry();
    reg.register(makePage("a", "global", { order: 20 }));
    reg.register(makePage("b", "global")); // 缺省 order
    reg.register(makePage("c", "global", { order: 10 }));
    reg.register(makePage("d", "global")); // 缺省 order

    expect(reg.getAll().map((p) => p.id)).toEqual(["c", "a", "b", "d"]);
  });

  it("重复 id 注册 → 幂等覆盖（后注册者胜，无重复条目）", () => {
    const reg = getSettingsPageRegistry();
    reg.register(makePage("a", "global"));
    reg.register(makePage("a", "project", { title: "覆盖版" }));

    const all = reg.getAll();
    expect(all).toHaveLength(1);
    expect(all[0].title).toBe("覆盖版");
    expect(all[0].group).toBe("project");
    expect(reg.get("a")?.title).toBe("覆盖版");
  });

  it("get(id) 命中注册项；未注册 id → undefined", () => {
    const reg = getSettingsPageRegistry();
    reg.register(makePage("a", "global"));

    expect(reg.get("a")?.id).toBe("a");
    expect(reg.get("不存在")).toBeUndefined();
  });

  it("_reset 清空全部条目（用例隔离）", () => {
    const reg = getSettingsPageRegistry();
    reg.register(makePage("a", "global"));
    reg._reset();

    expect(reg.getAll()).toEqual([]);
    expect(reg.get("a")).toBeUndefined();
  });

  it("惰性单例——多次 getSettingsPageRegistry() 同引用", () => {
    expect(getSettingsPageRegistry()).toBe(getSettingsPageRegistry());
  });
});
