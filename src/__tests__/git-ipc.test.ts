import { describe, it, expect, afterEach } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
// eslint-disable-next-line no-restricted-imports
import { invoke } from "@tauri-apps/api/core";

afterEach(() => {
  clearMocks();
});

const MOCK_WORKTREE = {
  path: "/tmp/repo",
  branch: "main",
  head: "abc123",
  isBare: false,
  isDetached: false,
  isMain: true,
};

const MOCK_WORKTREE_LIST = [
  MOCK_WORKTREE,
  {
    path: "/tmp/repo/.claude/worktrees/feat",
    branch: "worktree-feat",
    head: "def456",
    isBare: false,
    isDetached: false,
    isMain: false,
  },
];

describe("git IPC mock 测试", () => {
  it("git_is_repo 应对仓库路径返回 true", async () => {
    mockIPC((cmd, args) => {
      if (cmd === "git_is_repo" && (args as { path: string }).path === "/tmp/repo") {
        return true;
      }
    });
    const result = await invoke("git_is_repo", { path: "/tmp/repo" });
    expect(result).toBe(true);
  });

  it("git_is_repo 应对非仓库路径返回 false", async () => {
    mockIPC((cmd, args) => {
      if (cmd === "git_is_repo" && (args as { path: string }).path === "/tmp/not-repo") {
        return false;
      }
    });
    const result = await invoke("git_is_repo", { path: "/tmp/not-repo" });
    expect(result).toBe(false);
  });

  it("git_root 应返回仓库根目录", async () => {
    mockIPC((cmd, args) => {
      if (cmd === "git_root" && (args as { path: string }).path === "/tmp/repo/sub") {
        return "/tmp/repo";
      }
    });
    const result = await invoke("git_root", { path: "/tmp/repo/sub" });
    expect(result).toBe("/tmp/repo");
  });

  it("git_worktree_list 应返回 worktree 列表", async () => {
    mockIPC((cmd, args) => {
      if (
        cmd === "git_worktree_list" &&
        (args as { repoPath: string }).repoPath === "/tmp/repo"
      ) {
        return MOCK_WORKTREE_LIST;
      }
    });
    const result = await invoke("git_worktree_list", { repoPath: "/tmp/repo" });
    expect(result).toHaveLength(2);
    expect((result as typeof MOCK_WORKTREE_LIST)[0].branch).toBe("main");
  });

  it("git_worktree_add 应返回新 worktree 信息", async () => {
    mockIPC((cmd, args) => {
      if (
        cmd === "git_worktree_add" &&
        (args as { repoPath: string; name: string }).name === "feat"
      ) {
        return MOCK_WORKTREE_LIST[1];
      }
    });
    const result = await invoke("git_worktree_add", {
      repoPath: "/tmp/repo",
      name: "feat",
    });
    expect(result).toBeDefined();
    expect((result as typeof MOCK_WORKTREE_LIST[1]).branch).toBe("worktree-feat");
  });

  it("git_worktree_remove 不应抛出错误", async () => {
    mockIPC((cmd) => {
      if (cmd === "git_worktree_remove") return null;
    });
    await expect(
      invoke("git_worktree_remove", {
        repoPath: "/tmp/repo",
        name: "feat",
      }),
    ).resolves.toBeNull();
  });
});
