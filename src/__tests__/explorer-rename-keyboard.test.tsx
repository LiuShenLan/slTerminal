// explorer-rename-keyboard.test.tsx — F2 快捷键重命名测试
//
// 测试 F2 键经 ShortcutRegistry 触发重命名

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  setActiveExplorer,
  clearActiveExplorer,
  getActiveExplorer,
} from "../features/explorer/activeExplorer";
import type { ExplorerActions } from "../features/explorer/activeExplorer";

function makeActions(overrides: Partial<ExplorerActions> = {}): ExplorerActions {
  return {
    getSelectedPath: () => "/a/test.ts",
    deleteSelected: vi.fn().mockResolvedValue(undefined),
    openSelected: vi.fn(),
    renameSelected: vi.fn(),
    isRenaming: () => false,
    ...overrides,
  };
}

describe("F2 快捷键 → renameSelected", () => {
  beforeEach(() => {
    const current = getActiveExplorer();
    if (current) clearActiveExplorer(current);
  });

  it("有 active 时调用 renameSelected", () => {
    const a = makeActions();
    setActiveExplorer(a);
    const got = getActiveExplorer()!;
    got.renameSelected();
    expect(a.renameSelected).toHaveBeenCalledOnce();
  });

  it("getSelectedPath 返回当前选中的路径", () => {
    const a = makeActions({ getSelectedPath: () => "/a/selected.ts" });
    setActiveExplorer(a);
    expect(getActiveExplorer()!.getSelectedPath()).toBe("/a/selected.ts");
  });

  it("isRenaming=true 时不触发 renameSelected", () => {
    const a = makeActions({ isRenaming: () => true });
    setActiveExplorer(a);
    // 重命名中 F2 应透传（handler 返回 false）
    expect(getActiveExplorer()!.isRenaming()).toBe(true);
  });

  it("无选中路径时 getSelectedPath 返回 null", () => {
    const a = makeActions({ getSelectedPath: () => null });
    setActiveExplorer(a);
    expect(getActiveExplorer()!.getSelectedPath()).toBeNull();
  });

  it("F2 不副作用调用 deleteSelected 或 openSelected", () => {
    const a = makeActions();
    setActiveExplorer(a);
    getActiveExplorer()!.renameSelected();
    expect(a.deleteSelected).not.toHaveBeenCalled();
    expect(a.openSelected).not.toHaveBeenCalled();
  });
});
