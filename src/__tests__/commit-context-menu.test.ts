// commit-context-menu.test.ts — commitContextMenu 策略注册表 L2 测试
//
// 覆盖：状态→菜单映射、action 执行流程（confirmDialog → IPC → refresh）、
// confirmDialog 取消、操作失败不抛异常。
// 纯逻辑测试——mock 全部 IPC 和 confirmDialog，直接调用 getContextMenuItems。

import { describe, it, expect, beforeEach, vi } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  mockGitRollback,
  mockGitUnstage,
  mockDeleteEntry,
  mockConfirmDialog,
  mockRefresh,
} = vi.hoisted(() => ({
  mockGitRollback: vi.fn(),
  mockGitUnstage: vi.fn(),
  mockDeleteEntry: vi.fn(),
  mockConfirmDialog: vi.fn(),
  mockRefresh: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitRollback: mockGitRollback,
  gitUnstage: mockGitUnstage,
}));

vi.mock("../ipc/fs", () => ({
  deleteEntry: mockDeleteEntry,
}));

// confirmDialog 经 src/lib barrel 导出（OV-02 契约）
vi.mock("../lib", () => ({
  confirmDialog: mockConfirmDialog,
}));

import { getContextMenuItems } from "../features/commit/commitContextMenu";
import type { CommitMenuItem } from "../features/commit/commitContextMenu";

function makeEntry(path: string, status: string, oldPath: string | null = null) {
  return { path, status, oldPath };
}

beforeEach(() => {
  mockGitRollback.mockReset();
  mockGitUnstage.mockReset();
  mockDeleteEntry.mockReset();
  mockConfirmDialog.mockReset();
  mockRefresh.mockReset();
  // 默认：确认弹窗用户点确定
  mockConfirmDialog.mockResolvedValue(true);
  // 默认：IPC 成功
  mockGitRollback.mockResolvedValue(undefined);
  mockGitUnstage.mockResolvedValue(undefined);
  mockDeleteEntry.mockResolvedValue(undefined);
});

function getLabels(
  entry: ReturnType<typeof makeEntry>,
): string[] {
  return getContextMenuItems(entry, "C:/repo", mockRefresh).map(
    (i) => i.label,
  );
}

function getFirstAction(
  entry: ReturnType<typeof makeEntry>,
): CommitMenuItem["action"] | null {
  const items = getContextMenuItems(entry, "C:/repo", mockRefresh);
  return items[0]?.action ?? null;
}

// ═══════════════════════════════════════════════════════
// 状态 → 菜单映射
// ═══════════════════════════════════════════════════════

describe("getContextMenuItems 状态→菜单映射", () => {
  it("modified → 回滚", () => {
    expect(getLabels(makeEntry("C:/repo/mod.ts", "modified"))).toEqual([
      "回滚",
    ]);
  });

  it("deleted → 回滾", () => {
    expect(getLabels(makeEntry("C:/repo/del.ts", "deleted"))).toEqual([
      "回滚",
    ]);
  });

  it("renamed → 回滾", () => {
    expect(getLabels(makeEntry("C:/repo/ren.ts", "renamed"))).toEqual([
      "回滚",
    ]);
  });

  it("conflict → 回滾", () => {
    expect(getLabels(makeEntry("C:/repo/conf.ts", "conflict"))).toEqual([
      "回滚",
    ]);
  });

  it("added → 删除", () => {
    expect(getLabels(makeEntry("C:/repo/add.ts", "added"))).toEqual([
      "删除",
    ]);
  });

  it("untracked → 删除", () => {
    expect(getLabels(makeEntry("C:/repo/new.ts", "untracked"))).toEqual([
      "删除",
    ]);
  });

  it("ignored → 空（不弹菜单）", () => {
    expect(getLabels(makeEntry("C:/repo/ignored.ts", "ignored"))).toEqual(
      [],
    );
  });

  it("未知状态 → 空", () => {
    expect(getLabels(makeEntry("C:/repo/x.ts", "unknown"))).toEqual([]);
  });
});

// ═══════════════════════════════════════════════════════
// action 执行流程
// ═══════════════════════════════════════════════════════

describe("getContextMenuItems action 执行流程", () => {
  it("回滚: confirmDialog 确认 → gitRollback → refresh", async () => {
    const action = getFirstAction(makeEntry("C:/repo/a.txt", "modified"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockConfirmDialog).toHaveBeenCalledWith({
      title: "确认回滚",
      message: '确定回滚"a.txt" 到 HEAD 版本？此操作不可撤销。',
      danger: true,
    });
    expect(mockGitRollback).toHaveBeenCalledWith("C:/repo", "C:/repo/a.txt");
    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("回滚菜单项标记 danger（UI-802 危险项）", () => {
    const items = getContextMenuItems(
      makeEntry("C:/repo/a.txt", "modified"),
      "C:/repo",
      mockRefresh,
    );
    expect(items[0].danger).toBe(true);
  });

  it("回滚(renamed): 以 oldPath 定位 HEAD 侧（git status 语义 path=当前路径后 HEAD 文件在旧路径）", async () => {
    // 防复发：修复前回滚传 entry.path（旧路径）；path 语义修复后传 path=新路径
    // 会命中后端「HEAD 中不存在」，必须传 oldPath
    const action = getFirstAction(makeEntry("C:/repo/ren.ts", "renamed", "C:/repo/old.ts"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockGitRollback).toHaveBeenCalledWith("C:/repo", "C:/repo/old.ts");
    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("回滚(renamed, 无 oldPath 兜底): 以 path 调用", async () => {
    const action = getFirstAction(makeEntry("C:/repo/ren.ts", "renamed"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockGitRollback).toHaveBeenCalledWith("C:/repo", "C:/repo/ren.ts");
  });

  it("删除(added): confirmDialog → gitUnstage → deleteEntry → refresh", async () => {
    const action = getFirstAction(makeEntry("C:/repo/b.txt", "added"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockGitUnstage).toHaveBeenCalledWith("C:/repo", "C:/repo/b.txt");
    expect(mockDeleteEntry).toHaveBeenCalledWith("C:/repo/b.txt");
    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("删除(untracked): confirmDialog → deleteEntry → refresh（不调 gitUnstage）", async () => {
    const action = getFirstAction(makeEntry("C:/repo/c.txt", "untracked"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockGitUnstage).not.toHaveBeenCalled();
    expect(mockDeleteEntry).toHaveBeenCalledWith("C:/repo/c.txt");
    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("删除菜单项标记 danger（UI-802 危险项）", () => {
    const items = getContextMenuItems(
      makeEntry("C:/repo/b.txt", "added"),
      "C:/repo",
      mockRefresh,
    );
    expect(items[0].danger).toBe(true);
  });

  it("confirmDialog 取消后不执行任何 IPC", async () => {
    mockConfirmDialog.mockResolvedValue(false); // 用户点取消
    const action = getFirstAction(makeEntry("C:/repo/a.txt", "modified"));

    await action!();

    expect(mockGitRollback).not.toHaveBeenCalled();
    expect(mockRefresh).not.toHaveBeenCalled();
  });

  it("回滚失败: console.error 不抛异常，不调 refresh", async () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockGitRollback.mockRejectedValue(new Error("HEAD 不存在"));

    const action = getFirstAction(makeEntry("C:/repo/a.txt", "modified"));
    // 不应 throw
    await expect(action!()).resolves.toBeUndefined();

    expect(mockRefresh).not.toHaveBeenCalled();
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });

  it("删除(added) gitUnstage 失败: console.error 不抛异常，不调 refresh 与 deleteEntry", async () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockGitUnstage.mockRejectedValue(new Error("index locked"));

    const action = getFirstAction(makeEntry("C:/repo/b.txt", "added"));
    // 不应 throw（菜单 action 契约：失败静默降级）
    await expect(action!()).resolves.toBeUndefined();

    expect(mockDeleteEntry).not.toHaveBeenCalled();
    expect(mockRefresh).not.toHaveBeenCalled();
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });

  it("删除(untracked) deleteEntry 失败: console.error 不抛异常，不调 refresh", async () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockDeleteEntry.mockRejectedValue(new Error("file locked"));

    const action = getFirstAction(makeEntry("C:/repo/c.txt", "untracked"));
    // 不应 throw（菜单 action 契约：失败静默降级）
    await expect(action!()).resolves.toBeUndefined();

    expect(mockRefresh).not.toHaveBeenCalled();
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});
