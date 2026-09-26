/**
 * Agent 全局文件视图（F1）L4 spec：
 *
 * 全链路真实：假屋（USERPROFILE 隔离，run-wdio 注入）预造 .claude 配置文件 →
 * 活动栏 agentFiles 钮打开视图 → claude 节点渲染（logo + displayName "Claude Code"）→
 * 展开 → FileTreeExplorer 树渲染（settings.json 可见）+ 运行时目录默认隐藏
 * （runtimePaths 首段 projects 不出现）→ 设置中心 agent.claude.basic 勾选
 * 「显示运行时文件」→ 树即时出现 projects（rootFilter 引用变化刷新联动）→
 * 双击 settings.json → openFileInPage 项目外形态打开（编辑器标题 = basename）。
 *
 * 隔离：假屋 .claude 由本 spec Node 侧预造/自净（USERPROFILE 为 run-wdio 临时假屋，
 * 真实用户目录零接触——ADR-0016）；settings.json 数据面（app 配置）经 SLTERM_DATA_DIR
 * 隔离，suite 级快照还原（勾选运行时开关会真实落盘 agentGlobalFiles 段）。
 */

import { expect, browser } from "@wdio/globals";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { waitForWorkspaceReady, waitForDockviewApi, createProject } from "./specUtils";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";

declare global {
  interface Window {
    __dockviewApi?: any;
    __slterm_e2e_toggleSideView?: (id: string) => void;
    __slterm_e2e_openSettings?: () => Promise<void>;
    __slterm_e2e_switchSettingsPage?: (id: string) => boolean;
    __slterm_e2e_closeAllSettingsPanels?: () => void;
  }
}

/** 假屋 .claude 目录（USERPROFILE = run-wdio 临时假屋） */
const fakeClaudeDir = join(process.env.USERPROFILE ?? "", ".claude");
/** 后端 settings.json（SLTERM_DATA_DIR 隔离）——suite 级快照还原 */
const settingsJsonPath = join(
  process.env.SLTERM_DATA_DIR ?? join(process.cwd(), "src-tauri", "target", "debug"),
  "settings.json",
);

/** 展开 claude 节点并等待树容器渲染 */
async function expandClaudeNode(): Promise<void> {
  // 幂等：mocha 重试时 app 侧组件状态保留（同 session 不重挂）——已展开则不再
  // 点击（toggle 语义会折叠）
  const alreadyExpanded = await browser.execute(
    () => !!document.querySelector('[data-e2e="agent-files-claude-tree-container"]'),
  );
  if (!alreadyExpanded) {
    await browser.execute(() => {
      (document.querySelector('[data-e2e="agent-files-node-claude"]') as HTMLElement | null)?.click();
    });
  }
  await browser.waitUntil(
    async () =>
      (await browser.execute(
        () => !!document.querySelector('[data-e2e="agent-files-claude-tree-container"]'),
      )) === true,
    { timeout: 10000, timeoutMsg: "claude 文件树未渲染" },
  );
}

/** 读取 claude 树根层可见节点文本集（tree-node-row 作用域限定 agent 树容器） */
async function readTreeNodeTexts(): Promise<string[]> {
  return browser.execute(() => {
    const container = document.querySelector('[data-e2e="agent-files-claude-tree-container"]');
    if (!container) return [];
    return Array.from(container.querySelectorAll('[data-testid="tree-node-row"]')).map(
      (n) => n.textContent ?? "",
    );
  });
}

describe("Agent 全局文件视图（F1）", function () {
  // $()/elementClick 每命令确定性 +5s focus 惩罚（e2e-tests/CLAUDE.md）——本 spec 全走
  // execute 程序化点击，仍留足长链预算
  this.timeout(120000);
  let settingsSnapshot: { existed: boolean; content: string | null };

  before(() => {
    settingsSnapshot = {
      existed: existsSync(settingsJsonPath),
      content: existsSync(settingsJsonPath) ? readFileSync(settingsJsonPath, "utf8") : null,
    };
    // 假屋预造：常规配置文件 + 运行时目录（runtimePaths 首段 projects）
    mkdirSync(fakeClaudeDir, { recursive: true });
    writeFileSync(join(fakeClaudeDir, "settings.json"), "{}", "utf8");
    mkdirSync(join(fakeClaudeDir, "projects"), { recursive: true });
    writeFileSync(join(fakeClaudeDir, "projects", "x.jsonl"), "", "utf8");
  });

  after(() => {
    try {
      rmSync(fakeClaudeDir, { recursive: true, force: true });
    } catch (err) {
      console.warn("[agent-files.e2e] 清理假屋 .claude 失败:", err);
    }
    try {
      if (settingsSnapshot.existed) {
        writeFileSync(settingsJsonPath, settingsSnapshot.content ?? "", "utf8");
      } else {
        rmSync(settingsJsonPath, { force: true });
      }
    } catch (err) {
      console.warn("[agent-files.e2e] 还原 settings.json 失败:", err);
    }
  });

  it("视图打开 → claude 节点渲染 → 展开树（运行时隐藏）→ 运行时开关联动 → 双击打开", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-files-"));
    try {
      // ── 1. 项目 + Dockview 就绪（双击打开链路需要活跃页面宿主） ──
      await waitForWorkspaceReady();
      await createProject(tempDir);
      await waitForDockviewApi();

      // ── 2. 活动栏 agentFiles 钮存在 → 打开视图（幂等：重试时视图可能已开，
      // toggle 会反转成关闭——先查状态，仅在未打开时 toggle） ──
      const hasBtn = await browser.execute(
        () => !!document.querySelector('[data-e2e="activity-btn-agentFiles"]'),
      );
      expect(hasBtn).toBe(true);
      await browser.execute(() => {
        const s = (window as any).__slterm_e2e_getSideBarState?.();
        if (s?.open?.top !== "agentFiles" && s?.open?.bottom !== "agentFiles") {
          (window as any).__slterm_e2e_toggleSideView?.("agentFiles");
        }
      });
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="agent-files-panel"]'),
          )) === true,
        { timeout: 10000, timeoutMsg: "Agent 全局文件视图未打开" },
      );

      // ── 3. claude 节点渲染（logo + displayName "Claude Code"，能力过滤命中） ──
      const node = await browser.execute(() => {
        const n = document.querySelector('[data-e2e="agent-files-node-claude"]');
        return {
          exists: !!n,
          text: n?.textContent ?? "",
          logoSrc: n?.querySelector("img")?.getAttribute("src") ?? null,
        };
      });
      expect(node.exists).toBe(true);
      expect(node.text).toContain("Claude Code");
      expect(node.logoSrc).toBe("/cli-icons/claude.png");

      // ── 4. 展开 → pinned watcher + 树渲染：settings.json 可见、projects 默认隐藏 ──
      await expandClaudeNode();
      await browser.waitUntil(
        async () => (await readTreeNodeTexts()).some((t) => t.includes("settings.json")),
        { timeout: 10000, timeoutMsg: "settings.json 未出现在 claude 树根层" },
      );
      const initialTexts = await readTreeNodeTexts();
      expect(initialTexts.some((t) => t.includes("projects"))).toBe(false);

      // ── 5. 设置中心勾选「显示运行时文件」→ 树即时出现 projects（配置联动） ──
      await browser.execute(() => (window as any).__slterm_e2e_openSettings?.());
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="settings-panel"]'),
          )) === true,
        { timeout: 15000, timeoutMsg: "设置面板未就绪" },
      );
      const switched = await browser.execute(
        () => (window as any).__slterm_e2e_switchSettingsPage?.("agent.claude.basic") ?? false,
      );
      expect(switched).toBe(true);
      await browser.waitUntil(
        async () =>
          (await browser.execute(
            () => !!document.querySelector('[data-e2e="agent-basic-page-claude"]'),
          )) === true,
        { timeout: 10000, timeoutMsg: "claude 基础配置页未渲染" },
      );
      await browser.execute(() => {
        const label = document.querySelector(
          '[data-e2e="agent-basic-claude-runtime"]',
        ) as HTMLElement | null;
        label?.querySelector("input")?.click();
      });
      // 树即时刷新（rootFilter 引用变化 → refreshExpanded）
      await browser.waitUntil(
        async () => (await readTreeNodeTexts()).some((t) => t.includes("projects")),
        { timeout: 10000, timeoutMsg: "勾选运行时文件后 projects 未出现（过滤联动失败）" },
      );
      // 注意：此处**不能关设置面板**——本 spec 项目无任何面板，设置面板是页内唯一
      // dockview 组；关光后 resolveFocusedGroupForAdd 兜底链返 null，双击打开静默失败
      // （openFileInPage 既有语义）。留着它，步骤 6 双击落该组。

      // ── 6. 双击 settings.json → 编辑器打开（标题 = basename，项目外形态） ──
      const dblclicked = await browser.execute(() => {
        const container = document.querySelector('[data-e2e="agent-files-claude-tree-container"]');
        const row = Array.from(
          container?.querySelectorAll('[data-testid="tree-node-row"]') ?? [],
        ).find((n) => (n.textContent ?? "").includes("settings.json"));
        if (!row) return false;
        row.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true }));
        return true;
      });
      expect(dblclicked).toBe(true);
      await browser.waitUntil(
        async () =>
          (await browser.execute(() => {
            const panels = window.__dockviewApi?.panels ?? [];
            // component 属性在当前 dockview 版本实测为 null（既有 spec 的
            // p.component==="settings" 形态同源失效）——按面板 id 协议前缀 + 标题断言；
            // 限定当前活跃页面前缀，防重试时命中上一遍残留面板（假阳性）
            const info = (window as any).__slterm_e2e_getActivePageInfo?.();
            const prefix = info?.pageId ? `${info.pageId}:` : "\0";
            return panels.some(
              (p: any) =>
                p.id.startsWith(prefix) &&
                p.id.includes(":editor-") &&
                p.title === "settings.json",
            );
          })) === true,
        { timeout: 10000, timeoutMsg: "双击未打开 settings.json 编辑器面板" },
      );
    } finally {
      // 收尾：还原运行时开关（取消勾选——防持久化残留影响后续 spec；落盘有 2s
      // debounce，after 快照还原兜底）
      try {
        await browser.execute(() => (window as any).__slterm_e2e_openSettings?.());
        await browser.execute(
          () => (window as any).__slterm_e2e_switchSettingsPage?.("agent.claude.basic"),
        );
        await browser.execute(() => {
          const label = document.querySelector(
            '[data-e2e="agent-basic-claude-runtime"]',
          ) as HTMLElement | null;
          const input = label?.querySelector("input");
          if (input?.checked) input.click();
        });
      } catch { /* 忽略 */ }
      try {
        await browser.execute(() => (window as any).__slterm_e2e_closeAllSettingsPanels?.());
      } catch { /* 忽略 */ }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });
});
