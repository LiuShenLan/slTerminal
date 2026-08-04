/**
 * 编辑器域 E2E spec（E2E-09 拆分）：编辑器页签标题（basename/冲突相对路径/
 * 关闭恢复）、Ctrl+S capture 路径写盘（mtime 断言）、dirty→clean 保存。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, writeFileSync, readFileSync, statSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  waitForPanelTitle,
  createProject,
  getActivePageInfo,
} from "./specUtils";

describe("页签标题", () => {
  // 前置：创建 rootPath = C:\e2e-title-test 的项目并切换为活跃页。
  // 标题重算的冲突相对路径依赖活跃页 rootPath 与用例测试路径前缀匹配
  // （relativePath 不匹配返回 null → 标题保持 basename）——原 test.e2e.ts 中
  // 由前置"终端页签标题"用例创建同路径项目保证，拆分后 editor spec 自备。
  before(async () => {
    await waitForWorkspaceReady();
    const pageId = await createProject("C:\\e2e-title-test");
    if (!pageId) throw new Error("无法创建 C:\\e2e-title-test 项目");
    await waitForDockviewApi();
  });

  /** 获取活跃页面信息（编辑器标题用例前置） */
  async function requireActivePageInfo(): Promise<{ pageId: string; rootPath: string }> {
    const pageInfo = await getActivePageInfo();
    if (!pageInfo) throw new Error("无法获取活跃页面信息");
    return pageInfo;
  }

  it("编辑器页签标题为文件名", async () => {
    await waitForWorkspaceReady();

    const { pageId, rootPath } = await requireActivePageInfo();

    // 创建编辑器面板（带文件路径）
    const panelId = "e2e-title-editor-" + Date.now();
    const testFilePath = "C:\\e2e-title-test\\src\\main.ts";

    await browser.execute(
      (args: { pid: string; testPath: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid,
          component: "editor",
          title: "main.ts",
          params: { panelId: args.pid, filePath: args.testPath },
        });
      },
      { pid: panelId, testPath: testFilePath },
    );

    // 验证标题显示为文件名
    const title = await waitForPanelTitle(panelId, "main.ts", 10000);
    expect(title).toBe("main.ts");

    // 注册文件编辑器并验证标题重算（单文件无冲突，保持 basename）
    await browser.execute(
      (args: { pageId: string; rootPath: string; pid: string; testPath: string }) => {
        (window as any).__slterm_e2e_registerAndRecompute?.(
          args.pageId,
          args.rootPath,
          args.pid,
          args.testPath,
        );
      },
      { pageId, rootPath, pid: panelId, testPath: testFilePath },
    );

    const recomputedTitle = await waitForPanelTitle(panelId, "main.ts", 5000);
    expect(recomputedTitle).toBe("main.ts");
  });

  it("同名文件冲突时显示相对路径", async () => {
    await waitForWorkspaceReady();

    const { pageId, rootPath } = await requireActivePageInfo();

    // 创建第一个编辑器（src/index.ts）
    const pid1 = "e2e-conflict-1-" + Date.now();
    const path1 = "C:\\e2e-title-test\\src\\index.ts";
    await browser.execute(
      (args: { pid: string; path: string; pageId: string; root: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid, component: "editor", title: "index.ts",
          params: { panelId: args.pid, filePath: args.path },
        });
        (window as any).__slterm_e2e_registerAndRecompute?.(
          args.pageId, args.root, args.pid, args.path,
        );
      },
      { pid: pid1, path: path1, pageId, root: rootPath },
    );

    // 创建第二个编辑器（lib/index.ts）—— 同名不同路径
    const pid2 = "e2e-conflict-2-" + Date.now();
    const path2 = "C:\\e2e-title-test\\lib\\index.ts";
    await browser.execute(
      (args: { pid: string; path: string; pageId: string; root: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid, component: "editor", title: "lib/index.ts",
          params: { panelId: args.pid, filePath: args.path },
        });
        (window as any).__slterm_e2e_registerAndRecompute?.(
          args.pageId, args.root, args.pid, args.path,
        );
      },
      { pid: pid2, path: path2, pageId, root: rootPath },
    );

    // 验证两个编辑器都显示为相对路径
    const title1 = await waitForPanelTitle(pid1, "src/index.ts", 10000);
    const title2 = await waitForPanelTitle(pid2, "lib/index.ts", 5000);
    expect(title1).toBe("src/index.ts");
    expect(title2).toBe("lib/index.ts");
  });

  it("关闭同名面板后剩余面板切回 basename", async () => {
    await waitForWorkspaceReady();

    const { pageId, rootPath } = await requireActivePageInfo();

    // 创建两个同名编辑器
    const pid1 = "e2e-reclose-1-" + Date.now();
    const pid2 = "e2e-reclose-2-" + Date.now();
    const path1 = "C:\\e2e-title-test\\a\\utils.ts";
    const path2 = "C:\\e2e-title-test\\b\\utils.ts";

    await browser.execute(
      (args: {
        pid1: string; pid2: string; path1: string; path2: string;
        pageId: string; root: string;
      }) => {
        const api = window.__dockviewApi!;
        api.addPanel({
          id: args.pid1, component: "editor", title: "a/utils.ts",
          params: { panelId: args.pid1, filePath: args.path1 },
        });
        api.addPanel({
          id: args.pid2, component: "editor", title: "b/utils.ts",
          params: { panelId: args.pid2, filePath: args.path2 },
        });
        const reg = (window as any).__slterm_e2e_registerAndRecompute!;
        reg(args.pageId, args.root, args.pid1, args.path1);
        reg(args.pageId, args.root, args.pid2, args.path2);
      },
      { pid1, pid2, path1, path2, pageId, root: rootPath },
    );

    // 验证冲突状态
    expect(await waitForPanelTitle(pid1, "a/utils.ts", 5000)).toBe("a/utils.ts");
    expect(await waitForPanelTitle(pid2, "b/utils.ts", 5000)).toBe("b/utils.ts");

    // 关闭第二个面板（pid2）
    await browser.execute((pid: string) => {
      const panel = window.__dockviewApi?.getPanel(pid);
      panel?.api.close();
    }, pid2);

    // 验证 pid1 标题切回 basename（关闭冲突面板后自动重算）
    const finalTitle = await waitForPanelTitle(pid1, "utils.ts", 10000);
    expect(finalTitle).toBe("utils.ts");
  });
});

describe("编辑器保存 (Ctrl+S)", () => {
  // 说明：embedded WDIO 驱动无法把 OS 级按键（browser.keys）投递进 WebView2 页面
  //（终端 Ctrl+Shift+V 用例同样绕过——它直接写标记而非断言真实按键）。
  // 故本用例在页面内 dispatch 合成 keydown 到 window——由 ShortcutRegistry 的
  // window capture 监听器真实捕获（与生产同一路径）→ editor.save → 真实 IPC fs.writeFile 写盘，
  // 以文件 mtime 变化断言写盘发生。覆盖 C1：capture 监听 + context 栈匹配 + 命令 handler + 写盘全链路。
  it("聚焦编辑器后 Ctrl+S → 经 capture 路径真实写盘（mtime 更新）", async () => {
    // 0. Node 侧创建真实临时目录 + 文件（唯一 marker；后端 project_root 未设置 → 路径 sandbox 跳过）
    const marker = "e2e_save_" + Date.now();
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-save-"));
    const filePath = join(tempDir, "save.txt");
    writeFileSync(filePath, marker, "utf8");
    const mtimeBefore = statSync(filePath).mtimeMs;

    try {
      // 1. 等待 Workspace 就绪
      await waitForWorkspaceReady();

      // 2. 程序化创建项目（根 = 临时目录）
      await createProject(tempDir);

      // 3. 等待 Dockview API
      await waitForDockviewApi();

      // 4. 打开编辑器面板（加载临时文件）
      const panelId = "e2e-save-editor-" + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: "editor",
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: filePath },
      );

      // 5. 等待编辑器加载文件内容（某个 .cm-content 含唯一 marker）
      await browser.waitUntil(
        async () =>
          await browser.execute((m: string) => {
            const nodes = document.querySelectorAll(".cm-content");
            for (const n of nodes) {
              if ((n.textContent ?? "").includes(m)) return true;
            }
            return false;
          }, marker),
        { timeout: 15000, timeoutMsg: "编辑器未加载文件内容（.cm-content 未出现 marker）" },
      );

      // 6. 标记目标编辑器并真实点击聚焦 → 真实 focusin 冒泡到 container → pushContext("editor")
      const marked = await browser.execute((m: string) => {
        const nodes = document.querySelectorAll(".cm-content");
        for (const n of nodes) {
          if ((n.textContent ?? "").includes(m)) {
            (n as HTMLElement).setAttribute("data-e2e-save", "1");
            return true;
          }
        }
        return false;
      }, marker);
      expect(marked).toBe(true);
      // 触发 editor 焦点上下文：usePanelFocus 监听 container 的 focusin 事件 →
      // 在 .cm-content 上 dispatch 合成 focusin（bubbles）→ 冒泡到 container → pushContext + setActiveEditor。
      // 用合成事件而非 .click()——headless WebView2 中点击 CodeMirror 聚焦不稳定。
      await browser.execute(() => {
        const el = document.querySelector('[data-e2e-save="1"]');
        el?.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
      });

      // 7. 轮询等待 editor.save 已注册且 "editor" 上下文已激活
      await browser.waitUntil(
        async () => {
          const dbg = await browser.execute(() => (window as any).__slterm_e2e_shortcutDebug?.());
          return dbg?.commands?.includes("editor.save") && dbg?.stack?.includes("editor");
        },
        { timeout: 8000, timeoutMsg: "editor.save 未注册或 \"editor\" 上下文未激活" },
      );

      // 8-9. 每轮向 window dispatch 合成 Ctrl+S（capture 监听真实捕获）直到写盘：
      //      mtime 前进 + 内容仍为编辑器 doc（marker）。dispatch-in-loop 消除首发时序竞态。
      await browser.waitUntil(
        async () =>
          await browser.execute((): boolean => {
            window.dispatchEvent(new KeyboardEvent("keydown", {
              ctrlKey: true, code: "KeyS", key: "s", bubbles: true, cancelable: true,
            }));
            return true;
          }).then(() => {
            try {
              const st = statSync(filePath);
              return st.mtimeMs > mtimeBefore && readFileSync(filePath, "utf8").includes(marker);
            } catch {
              return false;
            }
          }),
        { timeout: 10000, timeoutMsg: "Ctrl+S 未经 capture 路径写盘（文件 mtime 未更新）" },
      );

      expect(statSync(filePath).mtimeMs).toBeGreaterThan(mtimeBefore);
      expect(readFileSync(filePath, "utf8")).toContain(marker);
    } finally {
      // 清理临时目录
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});

describe("编辑器 dirty→clean 保存", () => {
  // 验证编辑器修改内容后 Ctrl+S 将新内容写盘（区别于仅验证 mtime 变化）。
  // embedded 驱动无法键盘输入，故通过外部写盘触发编辑器 auto-reload，
  // 然后 Ctrl+S 保存当前内容，验证磁盘文件与编辑器内容一致。
  it("should persist modified content to disk after external change triggers reload then Ctrl+S save", async () => {
    const initialContent = "v1_initial_content_" + Date.now();
    const modifiedContent = "v2_modified_content_" + Date.now();
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-save-dirty-"));
    const filePath = join(tempDir, "dirty_save.txt");
    writeFileSync(filePath, initialContent, "utf8");
    const mtimeBefore = statSync(filePath).mtimeMs;

    try {
      // 1-3. 等待就绪 + 创建项目 + Dockview API
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();

      // 4. 打开编辑器面板
      const panelId = "e2e-dirty-save-" + Date.now();
      await browser.execute(
        (args: { pid: string; path: string }) => {
          window.__dockviewApi!.addPanel({
            id: args.pid,
            component: "editor",
            params: { panelId: args.pid, filePath: args.path },
          });
        },
        { pid: panelId, path: filePath },
      );

      // 5. 等待编辑器加载初始内容
      await browser.waitUntil(
        async () =>
          await browser.execute((m: string) => {
            const nodes = document.querySelectorAll(".cm-content");
            for (const n of nodes) {
              if ((n.textContent ?? "").includes(m)) return true;
            }
            return false;
          }, initialContent),
        { timeout: 15000, timeoutMsg: "编辑器未加载初始内容" },
      );

      // 6. 外部修改文件内容（模拟用户编辑后）
      writeFileSync(filePath, modifiedContent, "utf8");

      // 7. 等待编辑器 auto-reload 加载修改后内容（轮询 .cm-content）
      await browser.waitUntil(
        async () =>
          await browser.execute((m: string) => {
            const nodes = document.querySelectorAll(".cm-content");
            for (const n of nodes) {
              if ((n.textContent ?? "").includes(m)) return true;
            }
            return false;
          }, modifiedContent),
        { timeout: 15000, timeoutMsg: "编辑器未 auto-reload 修改后内容" },
      );

      // 8. 激活 editor 上下文（合成 focusin）
      await browser.execute((m: string) => {
        const nodes = document.querySelectorAll(".cm-content");
        for (const n of nodes) {
          if ((n.textContent ?? "").includes(m)) {
            (n as HTMLElement).setAttribute("data-e2e-dirty-save", "1");
            n.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
            return;
          }
        }
      }, modifiedContent);

      // 9. 等待 editor.save 可调度
      await browser.waitUntil(
        async () => {
          const dbg = await browser.execute(() => (window as any).__slterm_e2e_shortcutDebug?.());
          return dbg?.commands?.includes("editor.save") && dbg?.stack?.includes("editor");
        },
        { timeout: 8000, timeoutMsg: "editor.save 未就绪" },
      );

      // 10. 轮询 dispatch Ctrl+S 直到写盘
      await browser.waitUntil(
        async () =>
          await browser.execute((): boolean => {
            window.dispatchEvent(new KeyboardEvent("keydown", {
              ctrlKey: true, code: "KeyS", key: "s", bubbles: true, cancelable: true,
            }));
            return true;
          }).then(() => {
            try {
              const st = statSync(filePath);
              return st.mtimeMs > mtimeBefore && readFileSync(filePath, "utf8").includes(modifiedContent);
            } catch {
              return false;
            }
          }),
        { timeout: 10000, timeoutMsg: "Ctrl+S 未将修改后内容写盘" },
      );

      // 11. 断言磁盘文件包含修改后内容
      const diskContent = readFileSync(filePath, "utf8");
      expect(diskContent).toContain(modifiedContent);
      expect(statSync(filePath).mtimeMs).toBeGreaterThan(mtimeBefore);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });
});
