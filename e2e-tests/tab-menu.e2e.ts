/**
 * 页签右键菜单 E2E spec——自研 TabMenuPopup 生产链路回归防护。
 *
 * 背景：dockview 8.1.0 free core 无 contextMenuService（页签右键菜单是 enterprise
 * 模块），「getTabContextMenuItems」路径恒短路——生产菜单由 DefaultTab onContextMenu
 * → PageDockview 状态 → TabMenuPopup 自绘。此前 2 周回归滞留根因之一：无任何真实
 * 二进制（L4）层覆盖页签右键；本 spec 在真实 WebView2 断言菜单弹出/关闭/动作。
 *
 * 右键手势：合成 contextmenu MouseEvent dispatch（embedded WDIO 无法 OS 级右键投递，
 * 半端到端边界登记见 e2e-tests/CLAUDE.md）；目标 = data-e2e="tab-close-{panelId}"
 * 按钮的父级（DefaultTab 内容根，真实用户命中区——对 .dv-tab 自身派发冒泡不经过
 * DefaultTab div）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  waitForPanelTitle,
  createProject,
  addTerminalPanel,
} from "./specUtils";

describe("页签右键菜单（tab-menu）", () => {
  let rootDir: string;
  let pageId: string;

  before(async () => {
    await waitForWorkspaceReady();
    // 真实临时项目目录（复制相对路径用例需真实 rootPath 前缀匹配）
    rootDir = mkdtempSync(join(tmpdir(), "slterm-e2e-tab-menu-"));
    pageId = await createProject(rootDir);
    await waitForDockviewApi();
  });

  after(() => {
    rmSync(rootDir, { recursive: true, force: true });
  });

  /** 右键指定面板的页签并等待菜单出现 */
  async function openTabMenu(
    panelId: string,
    x = 300,
    y = 150,
  ): Promise<void> {
    const ok = await browser.execute(
      (args: { panelId: string; x: number; y: number }) => {
        const btn = document.querySelector(
          `[data-e2e="tab-close-${args.panelId}"]`,
        );
        if (!btn) return false;
        (btn.parentElement as HTMLElement).dispatchEvent(
          new MouseEvent("contextmenu", {
            bubbles: true,
            cancelable: true,
            clientX: args.x,
            clientY: args.y,
          }),
        );
        return true;
      },
      { panelId, x, y },
    );
    expect(ok).toBe(true);
    await browser.waitUntil(
      async () => (await readMenuItems()).length > 0,
      { timeout: 5000, timeoutMsg: `页签 ${panelId} 右键菜单未弹出` },
    );
  }

  /** 读取当前菜单项文本列表（容器内 role=menuitem） */
  async function readMenuItems(): Promise<string[]> {
    return browser.execute(() => {
      const menu = document.querySelector("[data-e2e='tab-menu']");
      if (!menu) return [];
      return Array.from(menu.querySelectorAll("[role='menuitem']")).map(
        (m) => m.textContent ?? "",
      );
    });
  }

  /** 菜单是否存在（关闭断言用） */
  async function menuOpen(): Promise<boolean> {
    return browser.execute(
      () => document.querySelector("[data-e2e='tab-menu']") !== null,
    );
  }

  /** 点击菜单项（文本精确匹配，容器内） */
  async function clickMenuByLabel(label: string): Promise<void> {
    const ok = await browser.execute((l: string) => {
      const menu = document.querySelector("[data-e2e='tab-menu']");
      if (!menu) return false;
      const items = Array.from(menu.querySelectorAll("[role='menuitem']"));
      const target = items.find((m) => (m.textContent ?? "").trim() === l);
      if (!target) return false;
      (target as HTMLElement).click();
      return true;
    }, label);
    expect(ok).toBe(true);
  }

  /** 当前 dockview 全部面板 id（轮询关闭族结果） */
  async function panelIds(): Promise<string[]> {
    return browser.execute(() =>
      window.__dockviewApi!.panels.map((p) => p.id),
    );
  }

  /** 新增编辑器面板（editor.e2e.ts 先例：无需文件在盘，filePath 驱动标题/路径） */
  async function addEditorPanel(filePath: string): Promise<string> {
    const panelId = `e2e-menu-editor-${Date.now()}`;
    await browser.execute(
      (args: { pid: string; path: string }) => {
        window.__dockviewApi!.addPanel({
          id: args.pid,
          component: "editor",
          title: args.path.split(/[\\/]/).pop()!,
          params: { panelId: args.pid, filePath: args.path },
        });
      },
      { pid: panelId, path: filePath },
    );
    await waitForPanelTitle(panelId, filePath.split(/[\\/]/).pop()!, 5000);
    return panelId;
  }

  it("终端页签右键 → 菜单弹出（新建终端/重命名/关闭族）；Escape 关闭", async () => {
    const termId = `terminal-${pageId}-0`;
    await addTerminalPanel(termId);

    await openTabMenu(termId);
    expect(await readMenuItems()).toEqual([
      "新建终端", "重命名", "关闭", "关闭其他", "关闭全部",
    ]);

    // Escape 关闭菜单（真实 keydown → TabMenuPopup window 监听）
    await browser.execute(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    await browser.waitUntil(async () => !(await menuOpen()), {
      timeout: 5000,
      timeoutMsg: "Escape 未关闭菜单",
    });
  });

  it("「关闭」关闭右键页签（同组其它面板保留）", async () => {
    const editorId = await addEditorPanel(
      join(rootDir, "close-me.txt").replace(/\\/g, "/"),
    );
    const termId = `terminal-${pageId}-0`; // 前置用例已建（describe 共享页面）
    expect(await panelIds()).toContain(editorId);

    await openTabMenu(editorId);
    await clickMenuByLabel("关闭");
    await browser.waitUntil(
      async () => !(await panelIds()).includes(editorId),
      { timeout: 5000, timeoutMsg: "「关闭」未关闭右键编辑器页签" },
    );
    // 终端保留
    expect(await panelIds()).toContain(termId);
  });

  it("「关闭其他」保留右键页签", async () => {
    // 重建被上一用例关掉的编辑器
    const editorId = await addEditorPanel(
      join(rootDir, "keep-me.txt").replace(/\\/g, "/"),
    );
    const termId = `terminal-${pageId}-0`;
    expect(await panelIds()).toContain(termId);

    await openTabMenu(editorId);
    await clickMenuByLabel("关闭其他");
    await browser.waitUntil(async () => {
      const ids = await panelIds();
      return ids.length === 1 && ids[0] === editorId;
    }, { timeout: 5000, timeoutMsg: "「关闭其他」未保留右键面板" });
  });

  it("「新建终端」在右键面板同组新增（编号从 terminal-N 续用）", async () => {
    // 注意：addTerminalPanel 直传 id 不经 makeTerminalPanelId，不推进页内 seq——
    // 菜单「新建终端」action 生成的下一编号仍从 terminal-{pageId}-0 起。故本用例
    // 以 editor 页签为锚（seq 尚未被任何用户路径推进），且 terminal-0 已被
    // 「关闭其他」用例关闭（关闭不回收编号、dockview 无残留）——无重名冲突。
    const editorId = await addEditorPanel(
      join(rootDir, "new-term-anchor.txt").replace(/\\/g, "/"),
    );
    expect(await panelIds()).toContain(editorId);
    expect(await panelIds()).not.toContain(`terminal-${pageId}-0`);

    await openTabMenu(editorId);
    await clickMenuByLabel("新建终端");
    // 新终端与右键编辑器同组出现（addTerminalPanel 未推进 seq → 编号续用 terminal-0）
    await waitForPanelTitle(`terminal-${pageId}-0`, "terminal-0", 8000);
    expect(await panelIds()).toContain(editorId);
  });

  it("文件页签「复制相对路径」→ 剪贴板为相对项目根路径（Unix 正斜杠）", async () => {
    // 真实文件写盘（rootDir/src/a.ts）；项目根 = rootDir
    const srcDir = join(rootDir, "src");
    mkdirSync(srcDir, { recursive: true });
    writeFileSync(join(srcDir, "a.ts"), "// e2e tab-menu\n", "utf-8");

    const filePath = join(rootDir, "src", "a.ts").replace(/\\/g, "/");
    const editorId = await addEditorPanel(filePath);

    await openTabMenu(editorId);
    const items = await readMenuItems();
    // 文件页签：头部「复制相对路径」，无「重命名」（非终端）
    expect(items[0]).toBe("复制相对路径");
    expect(items).not.toContain("重命名");
    await clickMenuByLabel("复制相对路径");

    // 剪贴板读取：clipboard-manager 插件 read_text（capabilities 放行），
    // history.e2e.ts 同族先例——勿在 src/ 新增 readText 消费（clipboard-guard 守卫）
    const clip = await browser.waitUntil(
      async () => {
        const t = await browser.execute(() =>
          (window as any).__TAURI_INTERNALS__.invoke(
            "plugin:clipboard-manager|read_text",
          ),
        );
        return t ? t : false;
      },
      { timeout: 8000, timeoutMsg: "剪贴板读取失败" },
    );
    expect(clip).toBe("src/a.ts");
  });
});
