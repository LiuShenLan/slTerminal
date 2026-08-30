// settings-pages-registration.test.ts —— 真实注册触发点验证（F11，TE-04）
//
// 被测：src/features/settingsCenter/pages.ts 的 side-effect 注册（硬约束 #13：
// 注册经 side-effect import 触发，禁止隐式初始化）。本文件不 mock pages 自身，
// 仅把三个页面组件模块 mock 为 () => null（组件实现不属本测试面），
// import 真实 pages.ts 触发注册后，断言注册表精确包含三条 {id, group, order}。
// afterEach 调 _reset() 保证用例隔离（注册表家族契约）。

import { describe, it, expect, afterEach, vi } from "vitest";

// 页面组件实现不属本测试面——整模块 mock 为空组件桩，避免拉入真实组件依赖链
vi.mock("../panels/settings/pages/KeybindingsPage", () => ({ default: () => null }));
vi.mock("../panels/settings/pages/PlanBalancePage", () => ({ default: () => null }));
vi.mock("../panels/settings/pages/HooksSettingsPage", () => ({ default: () => null }));

import { getSettingsPageRegistry } from "../features/settingsCenter/SettingsPageRegistry";
// side-effect import：import 即完成全部配置页注册（pages.ts 注册触发点）
import "../features/settingsCenter/pages";

describe("配置页真实注册（pages.ts side-effect import）", () => {
  afterEach(() => {
    // 硬约束 #13 注册表契约：_reset() 清空全部条目，保证用例隔离
    getSettingsPageRegistry()._reset();
  });

  it("pages.ts import 后注册表精确包含三条配置页（id/group/order 逐一核对）", () => {
    const pages = getSettingsPageRegistry().getAll().map((p) => ({
      id: p.id,
      group: p.group,
      order: p.order,
    }));

    expect(pages).toEqual([
      { id: "keybindings", group: "global", order: 10 },
      { id: "planBalance", group: "global", order: 20 },
      { id: "hooks", group: "project", order: 100 },
    ]);
  });
});
