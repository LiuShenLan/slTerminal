// explorer-keyboard.test.ts — 文件浏览器快捷键命令工厂测试
//
// 照 keyboard.test.ts / editor-keyboard.test.ts 模式：
// createExplorerShortcuts() 返回 3 条命令，handler 经 getActiveExplorer 派发

import { describe, it, expect, vi, beforeEach } from "vitest";
import { createExplorerShortcuts } from "../features/explorer/keyboard";
import * as activeExplorer from "../features/explorer/activeExplorer";
import type { ExplorerActions } from "../features/explorer/activeExplorer";

function makeActions(overrides: Partial<ExplorerActions> = {}): ExplorerActions {
  return {
    getSelectedPath: () => null,
    deleteSelected: vi.fn().mockResolvedValue(undefined),
    openSelected: vi.fn(),
    renameSelected: vi.fn(),
    isRenaming: () => false,
    ...overrides,
  };
}

describe("createExplorerShortcuts", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("返回 3 条命令（delete/open/rename）", () => {
    const cmds = createExplorerShortcuts();
    expect(cmds).toHaveLength(3);
    expect(cmds.map((c) => c.id)).toEqual([
      "explorer.delete",
      "explorer.open",
      "explorer.rename",
    ]);
  });

  // --- explorer.delete ---

  it("explorer.delete：有 active → 调 deleteSelected → 返回 true", () => {
    const a = makeActions();
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(a);
    const cmds = createExplorerShortcuts();
    const del = cmds.find((c) => c.id === "explorer.delete")!;
    expect(del.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(true);
    expect(a.deleteSelected).toHaveBeenCalledOnce();
  });

  it("explorer.delete：无 active → 返回 false", () => {
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(null);
    const cmds = createExplorerShortcuts();
    const del = cmds.find((c) => c.id === "explorer.delete")!;
    expect(del.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(false);
  });

  it("explorer.delete：isRenaming()=true → 返回 false", () => {
    const a = makeActions({ isRenaming: () => true });
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(a);
    const cmds = createExplorerShortcuts();
    const del = cmds.find((c) => c.id === "explorer.delete")!;
    expect(del.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(false);
    expect(a.deleteSelected).not.toHaveBeenCalled();
  });

  // --- explorer.open ---

  it("explorer.open：有 active → 调 openSelected → 返回 true", () => {
    const a = makeActions();
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(a);
    const cmds = createExplorerShortcuts();
    const open = cmds.find((c) => c.id === "explorer.open")!;
    expect(open.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(true);
    expect(a.openSelected).toHaveBeenCalledOnce();
  });

  it("explorer.open：无 active → 返回 false", () => {
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(null);
    const cmds = createExplorerShortcuts();
    const open = cmds.find((c) => c.id === "explorer.open")!;
    expect(open.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(false);
  });

  it("explorer.open：isRenaming()=true → 返回 false", () => {
    const a = makeActions({ isRenaming: () => true });
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(a);
    const cmds = createExplorerShortcuts();
    const open = cmds.find((c) => c.id === "explorer.open")!;
    expect(open.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(false);
    expect(a.openSelected).not.toHaveBeenCalled();
  });

  // --- explorer.rename ---

  it("explorer.rename：有 active → 调 renameSelected → 返回 true", () => {
    const a = makeActions();
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(a);
    const cmds = createExplorerShortcuts();
    const rename = cmds.find((c) => c.id === "explorer.rename")!;
    expect(rename.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(true);
    expect(a.renameSelected).toHaveBeenCalledOnce();
  });

  it("explorer.rename：无 active → 返回 false", () => {
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(null);
    const cmds = createExplorerShortcuts();
    const rename = cmds.find((c) => c.id === "explorer.rename")!;
    expect(rename.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(false);
  });

  it("explorer.rename：isRenaming()=true → 返回 false", () => {
    const a = makeActions({ isRenaming: () => true });
    vi.spyOn(activeExplorer, "getActiveExplorer").mockReturnValue(a);
    const cmds = createExplorerShortcuts();
    const rename = cmds.find((c) => c.id === "explorer.rename")!;
    expect(rename.handler(new KeyboardEvent("keydown", { code: "Delete" }))).toBe(false);
    expect(a.renameSelected).not.toHaveBeenCalled();
  });

  // --- 元数据 ---

  it("3 条命令 defaultKey 正确", () => {
    const cmds = createExplorerShortcuts();
    const del = cmds.find((c) => c.id === "explorer.delete")!;
    const open = cmds.find((c) => c.id === "explorer.open")!;
    const rename = cmds.find((c) => c.id === "explorer.rename")!;
    expect(del.defaultKey?.code).toBe("Delete");
    expect(open.defaultKey?.code).toBe("Enter");
    expect(rename.defaultKey?.code).toBe("F2");
  });

  it("3 条命令 context 均为 explorer", () => {
    const cmds = createExplorerShortcuts();
    for (const c of cmds) {
      expect(c.context).toBe("explorer");
    }
  });
});

// =====================================================================
// 回归测试：ref 模式闭包过期修复
// =====================================================================

describe("explorer actions ref 模式——闭包不过期", () => {
  const { getActiveExplorer, setActiveExplorer, clearActiveExplorer } = activeExplorer;

  beforeEach(() => {
    vi.restoreAllMocks();
    const a = getActiveExplorer();
    if (a) clearActiveExplorer(a);
  });

  it("selectedPath 变化后 getSelectedPath() 返回最新值（不重新 activate）", () => {
    // 模拟 ExplorerPanel 的 ref 模式：selectedPathRef 持有最新值
    const ref = { current: "/a/file1" as string | null };

    const actions: ExplorerActions = {
      getSelectedPath: () => ref.current,
      deleteSelected: vi.fn().mockResolvedValue(undefined),
      openSelected: vi.fn(),
      renameSelected: vi.fn(),
      isRenaming: () => false,
    };

    setActiveExplorer(actions);
    expect(getActiveExplorer()!.getSelectedPath()).toBe("/a/file1");

    // 模拟 React re-render：ref 更新为 file2，但不重新 setActiveExplorer
    ref.current = "/a/file2";
    // 同一 actions 对象，getSelectedPath 通过 ref 返回最新值
    expect(getActiveExplorer()!.getSelectedPath()).toBe("/a/file2");
  });

  it("selectedPath 变化后 deleteSelected 用最新路径（不重新 activate）", async () => {
    const ref = { current: "/a/file1" as string | null };
    const capturedPaths: (string | null)[] = [];
    const deleteSelected = vi.fn(async () => {
      capturedPaths.push(ref.current);
    });

    const actions: ExplorerActions = {
      getSelectedPath: () => ref.current,
      deleteSelected,
      openSelected: vi.fn(),
      renameSelected: vi.fn(),
      isRenaming: () => false,
    };

    setActiveExplorer(actions);
    ref.current = "/a/file2";
    await getActiveExplorer()!.deleteSelected();
    // deleteSelected 运行时读到 ref 最新值
    expect(capturedPaths).toEqual(["/a/file2"]);
  });

  it("explorerActions 引用稳定——重复 setActive 也不影响正确性", () => {
    const actions: ExplorerActions = {
      getSelectedPath: () => "/a/test.ts",
      deleteSelected: vi.fn().mockResolvedValue(undefined),
      openSelected: vi.fn(),
      renameSelected: vi.fn(),
      isRenaming: () => false,
    };

    // 设置两次同一对象——因为引用稳定（空 deps），两次 set 都指向同一对象
    setActiveExplorer(actions);
    setActiveExplorer(actions);
    expect(getActiveExplorer()).toBe(actions);
    expect(getActiveExplorer()!.getSelectedPath()).toBe("/a/test.ts");
  });
});
