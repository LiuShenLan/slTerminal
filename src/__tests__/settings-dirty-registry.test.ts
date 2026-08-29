// settings-dirty-registry.test.ts — 设置中心 dirty 汇聚真值源 L2 测试（F11，SC-FE-07）
//
// 覆盖：setSettingsDirty / isSettingsDirty / clearSettingsDirty 三 API 行为——
// 真值源被壳（SettingsPanel）与 DefaultTab × 关闭守卫共享（防两处状态漂移）。
// 测试隔离：beforeEach 清空 Map 条目（clearSettingsDirty 逐个清理，防用例串扰）。

import { describe, it, expect, beforeEach } from "vitest";
import {
  setSettingsDirty,
  isSettingsDirty,
  clearSettingsDirty,
} from "../features/settingsCenter/dirtyRegistry";

const PANEL_ID = "settings-page-a";

beforeEach(() => {
  // 清空所有条目（真值源无 _reset 接口——家族契约仅 set/is/clear，测试经 clear 隔离）
  clearSettingsDirty(PANEL_ID);
});

describe("settings dirty 真值源", () => {
  it("未设置（无条目）→ isSettingsDirty 返回 false", () => {
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
  });

  it("setSettingsDirty(panelId, true) → isSettingsDirty 返回 true", () => {
    setSettingsDirty(PANEL_ID, true);
    expect(isSettingsDirty(PANEL_ID)).toBe(true);
  });

  it("setSettingsDirty(panelId, false) → isSettingsDirty 返回 false（显式清 dirty）", () => {
    setSettingsDirty(PANEL_ID, true);
    setSettingsDirty(PANEL_ID, false);
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
  });

  it("clearSettingsDirty → 条目删除，isSettingsDirty 返回 false（壳卸载清理）", () => {
    setSettingsDirty(PANEL_ID, true);
    clearSettingsDirty(PANEL_ID);
    expect(isSettingsDirty(PANEL_ID)).toBe(false);
  });

  it("多面板隔离：一个面板 dirty 不影响其他面板", () => {
    setSettingsDirty(PANEL_ID, true);
    expect(isSettingsDirty("settings-page-b")).toBe(false);
    setSettingsDirty("settings-page-b", true);
    expect(isSettingsDirty(PANEL_ID)).toBe(true);
    expect(isSettingsDirty("settings-page-b")).toBe(true);
    clearSettingsDirty("settings-page-b");
    expect(isSettingsDirty(PANEL_ID)).toBe(true);
    expect(isSettingsDirty("settings-page-b")).toBe(false);
  });
});
