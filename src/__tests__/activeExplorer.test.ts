// activeExplorer.test.ts — 文件浏览器 active pointer 单元测试
//
// 照 activeTerminal.test.ts / activeEditor.test.ts 模式：
// set/get/覆盖/clear 仅匹配/初始态

import { describe, it, expect, beforeEach } from "vitest";
import {
  setActiveExplorer,
  clearActiveExplorer,
  getActiveExplorer,
  type ExplorerActions,
} from "../features/explorer/activeExplorer";

function makeActions(): ExplorerActions {
  return {
    getSelectedPath: () => null,
    deleteSelected: async () => {},
    openSelected: () => {},
    renameSelected: () => {},
    isRenaming: () => false,
  };
}

describe("activeExplorer", () => {
  // 每个测试之间重置——通过 clear 自身已有的 active
  beforeEach(() => {
    const current = getActiveExplorer();
    if (current) clearActiveExplorer(current);
  });

  it("set → get 返回同一 actions", () => {
    const a = makeActions();
    setActiveExplorer(a);
    expect(getActiveExplorer()).toBe(a);
  });

  it("覆盖 set → get 返回最新的", () => {
    const a1 = makeActions();
    const a2 = makeActions();
    setActiveExplorer(a1);
    setActiveExplorer(a2);
    expect(getActiveExplorer()).toBe(a2);
  });

  it("clear 匹配 → get 返回 null", () => {
    const a = makeActions();
    setActiveExplorer(a);
    clearActiveExplorer(a);
    expect(getActiveExplorer()).toBeNull();
  });

  it("clear 不匹配 → get 不变（防竞态：A blur → B focus 后 A 的 blur 不清 B）", () => {
    const a1 = makeActions();
    const a2 = makeActions();
    setActiveExplorer(a1);
    // B focus: 覆盖为 a2
    setActiveExplorer(a2);
    // A blur: 尝试清 a1（应无效）
    clearActiveExplorer(a1);
    expect(getActiveExplorer()).toBe(a2);
  });

  it("未 set 时 get 返回 null", () => {
    expect(getActiveExplorer()).toBeNull();
  });

  it("ExplorerActions 接口：5 个方法存在", () => {
    const a = makeActions();
    expect(typeof a.getSelectedPath).toBe("function");
    expect(typeof a.deleteSelected).toBe("function");
    expect(typeof a.openSelected).toBe("function");
    expect(typeof a.renameSelected).toBe("function");
    expect(typeof a.isRenaming).toBe("function");
  });
});
