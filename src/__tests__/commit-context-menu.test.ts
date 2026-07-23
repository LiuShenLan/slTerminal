// commit-context-menu.test.ts — commitContextMenu 策略注册表 L2 测试
//
// 覆盖：状态→菜单映射、action 执行流程（ask → IPC → refresh）、
// ask 取消、操作失败不抛异常。
// 纯逻辑测试——mock 全部 IPC 和 dialog，直接调用 getContextMenuItems。

import { describe, it, expect, beforeEach, vi } from "vitest";

// ── vi.hoisted：mock 状态在模块级 vi.mock 执行前就绪 ──
const {
  mockGitRollback,
  mockGitUnstage,
  mockDeleteEntry,
  mockAsk,
  mockRefresh,
} = vi.hoisted(() => ({
  mockGitRollback: vi.fn(),
  mockGitUnstage: vi.fn(),
  mockDeleteEntry: vi.fn(),
  mockAsk: vi.fn(),
  mockRefresh: vi.fn(),
}));

vi.mock("../ipc/git", () => ({
  gitRollback: mockGitRollback,
  gitUnstage: mockGitUnstage,
}));

vi.mock("../ipc/fs", () => ({
  deleteEntry: mockDeleteEntry,
}));

vi.mock("../ipc/dialog", () => ({
  ask: mockAsk,
}));

import { getContextMenuItems } from "../features/commit/commitContextMenu";
import type { CommitMenuItem } from "../features/commit/commitContextMenu";

function makeEntry(path: string, status: string) {
  return { path, status, oldPath: null };
}

beforeEach(() => {
  mockGitRollback.mockReset();
  mockGitUnstage.mockReset();
  mockDeleteEntry.mockReset();
  mockAsk.mockReset();
  mockRefresh.mockReset();
  // 默认：确认弹窗用户点确定
  mockAsk.mockResolvedValue(true);
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
  it("回滚: ask 确认 → gitRollback → refresh", async () => {
    const action = getFirstAction(makeEntry("C:/repo/a.txt", "modified"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockAsk).toHaveBeenCalledWith(
      '确定回滚"a.txt" 到 HEAD 版本？此操作不可撤销。',
      { title: "确认回滚", kind: "warning" },
    );
    expect(mockGitRollback).toHaveBeenCalledWith("C:/repo", "C:/repo/a.txt");
    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("删除(added): ask → gitUnstage → deleteEntry → refresh", async () => {
    const action = getFirstAction(makeEntry("C:/repo/b.txt", "added"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockGitUnstage).toHaveBeenCalledWith("C:/repo", "C:/repo/b.txt");
    expect(mockDeleteEntry).toHaveBeenCalledWith("C:/repo/b.txt");
    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("删除(untracked): ask → deleteEntry → refresh（不调 gitUnstage）", async () => {
    const action = getFirstAction(makeEntry("C:/repo/c.txt", "untracked"));
    expect(action).not.toBeNull();

    await action!();

    expect(mockGitUnstage).not.toHaveBeenCalled();
    expect(mockDeleteEntry).toHaveBeenCalledWith("C:/repo/c.txt");
    expect(mockRefresh).toHaveBeenCalledTimes(1);
  });

  it("ask 取消后不执行任何 IPC", async () => {
    mockAsk.mockResolvedValue(false); // 用户点取消
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
});
