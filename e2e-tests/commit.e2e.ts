/**
 * Commit 视图域 E2E spec（E2E-09 拆分）：
 * 变更列表渲染（Changes / Unversioned Files）、双击 modified 打开 diff 页签。
 */

import { expect, browser } from "@wdio/globals";
import { waitForWorkspaceReady, waitForDockviewApi, createProject } from "./specUtils";
import { makeGitRepo, cleanupGitRepo } from "./gitScaffold";

describe("commit 视图", () => {
  /**
   * 用例 1：验证 commit 视图渲染变更列表与未跟踪文件列表。
   * makeGitRepo({ modified: ["a.txt"], untracked: ["new.txt"] }) 搭建仓库
   * → createProject → toggleSideView("commit") → 断言 DOM 含对应文件条目。
   */
  it("commit 视图渲染变更列表（Changes / Unversioned Files）", async () => {
    const repoPath = makeGitRepo({ modified: ["a.txt"], untracked: ["new.txt"] });

    try {
      // 0. 等待 Workspace 就绪
      await waitForWorkspaceReady();

      // 1. 程序化创建测试项目（根 = 临时 git 仓库）
      await createProject(repoPath);

      // 2. 等待 Dockview API 就绪
      await waitForDockviewApi();

      // 3. 打开 commit 侧栏视图
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("commit");
      });

      // 4. 等待 commit-changes 区域渲染
      await browser.waitUntil(
        async () => {
          return await browser.execute(() => {
            return !!document.querySelector('[data-e2e="commit-changes"]');
          });
        },
        { timeout: 10000, timeoutMsg: "commit-changes 区域未渲染" },
      );

      // 5a. 断言 commit-changes 下列表项含 "a.txt"
      const changesText = await browser.execute(() => {
        const el = document.querySelector('[data-e2e="commit-changes"]');
        return el?.textContent ?? "";
      });
      expect(changesText).toContain("a.txt");

      // 5b. commit-changes 下存在 commit-file-item
      const changesHasItem = await browser.execute(() => {
        const section = document.querySelector('[data-e2e="commit-changes"]');
        return section?.querySelector('[data-e2e="commit-file-item"]') !== null;
      });
      expect(changesHasItem).toBe(true);

      // 5c. 断言 commit-unversioned 下列表项含 "new.txt"
      const unversionedText = await browser.execute(() => {
        const el = document.querySelector('[data-e2e="commit-unversioned"]');
        return el?.textContent ?? "";
      });
      expect(unversionedText).toContain("new.txt");

      // 5d. commit-unversioned 下存在 commit-file-item
      const unvHasItem = await browser.execute(() => {
        const section = document.querySelector('[data-e2e="commit-unversioned"]');
        return section?.querySelector('[data-e2e="commit-file-item"]') !== null;
      });
      expect(unvHasItem).toBe(true);
    } finally {
      cleanupGitRepo(repoPath);
    }
  });

  /**
   * 用例 2：双击 modified 文件条目 → 打开 diff 页签（标题含 "(git diff)"）
   *        且页面存在 diff-left / diff-right 两侧面板。
   */
  it("双击 modified 文件打开 diff 页签", async () => {
    const repoPath = makeGitRepo({ modified: ["a.txt"] });

    try {
      // 0. 等待 Workspace 就绪
      await waitForWorkspaceReady();

      // 1. 创建项目
      await createProject(repoPath);

      // 2. 等待 Dockview API
      await waitForDockviewApi();

      // 3. 打开 commit 侧栏视图
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("commit");
      });

      // 4. 等待 commit-file-item 渲染
      await browser.waitUntil(
        async () => {
          return await browser.execute(() => {
            return !!document.querySelector('[data-e2e="commit-file-item"]');
          });
        },
        { timeout: 10000, timeoutMsg: "commit-file-item 未渲染" },
      );

      // 5. 在页面内 dispatch 合成 dblclick 到文本为 "a.txt" 的 commit-file-item
      //    （embedded 驱动无法可靠投递 OS 级鼠标事件，合成事件由事件处理器真实捕获）
      const dispatched = await browser.execute((fileName: string) => {
        const items = document.querySelectorAll('[data-e2e="commit-file-item"]');
        for (const item of items) {
          if ((item.textContent ?? "").includes(fileName)) {
            item.dispatchEvent(
              new MouseEvent("dblclick", { bubbles: true, cancelable: true }),
            );
            return true;
          }
        }
        return false;
      }, "a.txt");
      expect(dispatched).toBe(true);

      // 6. 等待 diff-left / diff-right 元素出现（证明 diff 面板已挂载）
      await browser.waitUntil(
        async () => {
          return await browser.execute(() => {
            const left = document.querySelector('[data-e2e="diff-left"]');
            const right = document.querySelector('[data-e2e="diff-right"]');
            return !!(left && right);
          });
        },
        { timeout: 10000, timeoutMsg: "diff-panel 的 diff-left/diff-right 未渲染" },
      );

      // 7. 断言 diff-left / diff-right 存在
      const leftExists = await browser.$('[data-e2e="diff-left"]').isExisting();
      const rightExists = await browser.$('[data-e2e="diff-right"]').isExisting();
      expect(leftExists).toBe(true);
      expect(rightExists).toBe(true);

      // 8. 通过 __dockviewApi 断言存在标题含 "(git diff)" 的面板
      const hasDiffPanel = await browser.execute(() => {
        const api = (window as any).__dockviewApi;
        if (!api || !api.groups) return false;
        // dockview-react：api.groups 是只读数组，每 group 有 panels 数组
        for (const group of api.groups) {
          if (!group.panels) continue;
          for (const panel of group.panels) {
            if (panel?.api?.title && panel.api.title.includes("(git diff)")) {
              return true;
            }
          }
        }
        return false;
      });
      expect(hasDiffPanel).toBe(true);
    } finally {
      cleanupGitRepo(repoPath);
    }
  });
});
