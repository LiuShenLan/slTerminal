/**
 * 页内分屏 E2E spec（ADR-0020）——真实二进制 dockview 分屏路径回归防护。
 *
 * 覆盖（bug 2 修复验收面）：
 *   ① moveTo 等价落点：addGroup(direction) + moveTo 分屏 → 两组同显、被拖面板
 *     不消失（老代码 maximizePageGroup 隐藏非主组 → 被拖面板 isVisible false）
 *   ② 分屏后终端缓冲保留（dockview 复用面板实例，xterm 不重建）
 *   ③ 切页：本页各组同隐 / 他页组同显，切回同显
 *   ④ 分屏形态切片持久化：slterminal-projects.json 页布局含两叶（重启恢复数据源）
 *
 * 手势边界：真实拖拽手势（pointer 序列）自动化豁免登记 .claude/test-exemptions.md
 * ——本 spec 用 api.addGroup + panel.api.moveTo 等价落点（与真实拖拽同一 dockview
 * 内部入口），属半端到端边界（e2e-tests/CLAUDE.md DOC-02 同族）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  createProject,
  addTerminalPanel,
  waitForPtySessionReady,
  addPage,
  getProjectIdForPage,
  switchToPageAndWait,
} from "./specUtils";

/** 宿主组快照（派生归属断言用） */
interface GroupSnap {
  id: string;
  visible: boolean;
  panelIds: string[];
}

/** 读取宿主全部组快照 */
async function groupSnapshots(): Promise<GroupSnap[]> {
  return browser.execute(() =>
    window.__dockviewApi!.groups.map((g: any) => ({
      id: g.id,
      visible: g.api.isVisible,
      panelIds: g.panels.map((p: any) => p.id),
    })),
  );
}

/** 精确向指定面板终端 PTY 写入（data-panel-id 定位，规避「最新容器」歧义） */
async function writeToPtyOf(panelId: string, text: string): Promise<void> {
  const ok = await browser.execute(
    (args: { pid: string; data: string }) => {
      const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
      for (const c of containers) {
        if (c.getAttribute("data-panel-id") !== args.pid) continue;
        const el = c as any;
        if (el.__e2e_writeToPty) {
          el.__e2e_writeToPty(args.data);
          return true;
        }
      }
      return false;
    },
    { pid: panelId, data: text },
  );
  expect(ok).toBe(true);
}

/** 精确读取指定面板终端缓冲（无容器/无 helper 返回 null） */
async function getTerminalTextOf(panelId: string): Promise<string | null> {
  return browser.execute((pid: string) => {
    const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
    for (const c of containers) {
      if (c.getAttribute("data-panel-id") !== pid) continue;
      const el = c as any;
      if (typeof el.__e2e_getTerminalText === "function") return el.__e2e_getTerminalText();
    }
    return null;
  }, panelId);
}

describe("页内分屏（workspace-split，ADR-0020）", () => {
  let rootDir: string;
  let pageId: string;
  let projectId: string;
  let term0: string;
  let term1: string;
  /** 分屏前写入 term1 的标记文本（缓冲保留断言锚点） */
  const MARKER = "E2E_SPLIT_KEEP";

  before(async () => {
    await waitForWorkspaceReady();
    rootDir = mkdtempSync(join(tmpdir(), "slterm-e2e-split-"));
    pageId = await createProject(rootDir);
    await waitForDockviewApi();
    projectId = (await getProjectIdForPage(pageId)) ?? "";
    expect(projectId).not.toBe("");
    term0 = `${pageId}:terminal-0`;
    term1 = `${pageId}:terminal-1`;

    // 两终端（specUtils addTerminalPanel 无 position → 落活跃组 = 本页主组）
    await addTerminalPanel(term0);
    await waitForPtySessionReady(25000, term0);
    await addTerminalPanel(term1);
    await waitForPtySessionReady(25000, term1);

    // 分屏前往被拖终端写标记（缓冲保留锚点）
    await writeToPtyOf(term1, `echo ${MARKER}\r`);
    await browser.waitUntil(
      async () => (await getTerminalTextOf(term1))?.includes(MARKER) === true,
      { timeout: 15000, timeoutMsg: `被拖终端缓冲未回显 ${MARKER}` },
    );

    // 等价真实拖拽落点：addGroup(direction=right) 建自生组 + moveTo 入组
    await browser.execute((pid: string) => {
      const api = window.__dockviewApi!;
      const panel = api.getPanel(pid);
      const newGroup = api.addGroup({ direction: "right" });
      panel.api.moveTo({ group: newGroup });
    }, term1);

    // 分屏落定（两组并存）
    await browser.waitUntil(
      async () => (await groupSnapshots()).length === 2,
      { timeout: 8000, timeoutMsg: "分屏后宿主未呈现两组" },
    );
  });

  after(() => {
    // 终端 PTY（pwsh 子进程）cwd = 项目根，存活期间持目录句柄——rmSync 可能 EPERM，
    // 照 withProjectAndTerminal cleanup 先例吞错（临时目录由 OS 回收）
    try { rmSync(rootDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
  });

  it("① 分屏产第二组：主组留 terminal-0、自生组含 terminal-1，两组同显（防复发——老代码被拖面板隐藏）", async () => {
    const groups = await groupSnapshots();
    const primary = groups.find((g) => g.id === `page-${pageId}`);
    const stray = groups.find((g) => g.id !== `page-${pageId}`);
    expect(primary).toBeDefined();
    expect(stray).toBeDefined();
    expect(primary!.panelIds).toContain(term0);
    expect(stray!.panelIds).toEqual([term1]);
    // 两组同显（老代码 maximizePageGroup 隐藏非主组 → stray.visible = false）
    expect(primary!.visible).toBe(true);
    expect(stray!.visible).toBe(true);
    // 被拖面板不消失
    const panelAlive = await browser.execute(
      (pid: string) => window.__dockviewApi!.getPanel(pid) !== undefined,
      term1,
    );
    expect(panelAlive).toBe(true);
  });

  it("② 分屏后被拖终端缓冲保留（面板实例复用，xterm 不重建）", async () => {
    const text = await getTerminalTextOf(term1);
    expect(text).not.toBeNull();
    expect(text!).toContain(MARKER);
  });

  it("③ 切页：本页分屏两组同隐、他页组同显；切回本页两组同显且缓冲仍在", async () => {
    const pageB = await addPage(projectId, "Page B");
    expect(pageB).toBeTruthy();

    await switchToPageAndWait(pageB);
    // A 页两组同隐、B 页组同显（可见性 effect 异步——轮询）
    await browser.waitUntil(
      async () => {
        const groups = await groupSnapshots();
        const ofA = groups.filter((g) =>
          g.id === `page-${pageId}` || g.panelIds.some((p) => p.startsWith(`${pageId}:`)));
        const ofB = groups.find((g) => g.id === `page-${pageB}`);
        return (
          ofA.length === 2 &&
          ofA.every((g) => !g.visible) &&
          ofB !== undefined && ofB.visible
        );
      },
      { timeout: 8000, timeoutMsg: "切页后本页分屏组未同隐/他页组未同显" },
    );

    await switchToPageAndWait(pageId);
    await browser.waitUntil(
      async () => {
        const groups = await groupSnapshots();
        const ofA = groups.filter((g) =>
          g.id === `page-${pageId}` || g.panelIds.some((p) => p.startsWith(`${pageId}:`)));
        return ofA.length === 2 && ofA.every((g) => g.visible);
      },
      { timeout: 8000, timeoutMsg: "切回本页后分屏两组未同显" },
    );
    // 切回后缓冲仍在（切页不重建终端）
    expect(await getTerminalTextOf(term1)).toContain(MARKER);
  });

  it("④ 分屏形态切片持久化：slterminal-projects.json 页布局根含两叶", async () => {
    // 持久化文件：SLTERM_DATA_DIR 隔离目录（run-wdio.cjs 注入）；2s debounce 落盘
    const dataDir =
      process.env.SLTERM_DATA_DIR ?? join(process.cwd(), "src-tauri", "target", "debug");
    const file = join(dataDir, "slterminal-projects.json");
    await browser.waitUntil(
      async () => {
        try {
          const data = JSON.parse(readFileSync(file, "utf-8"));
          const page = data.projects?.[projectId]?.pages?.find(
            (p: any) => p.pageId === pageId,
          );
          const rootData = page?.layout?.grid?.root?.data;
          return (
            Array.isArray(rootData) &&
            rootData.length === 2 &&
            rootData.every((n: any) => n.type === "leaf")
          );
        } catch {
          return false;
        }
      },
      { timeout: 10000, timeoutMsg: "页布局切片未持久化为两分屏叶（slterminal-projects.json）" },
    );
  });
});
