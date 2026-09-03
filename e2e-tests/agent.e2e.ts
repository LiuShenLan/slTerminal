/**
 * Agent 会话域 E2E spec（NAV-08/NAV-10 重写）：
 * 视图存在性、纯 shell 无行、动态四态、R2 用量保持、R3 删行不复活、
 * R4 关页签删行、toast 触发链路（skip——人工验证）。
 *
 * NAV-08 迁移：旧状态视图退役（原组件已删除），
 * 活跃会话行承接方 = 导航树 NavTree（nav 视图）：
 *   - 视图打开：toggleSideView("nav")；容器 data-e2e="nav-tree"
 *   - 活跃会话行：data-e2e="nav-row-session" + data-panel-id（挂页面下，NavSessionRow）
 *   - 四态 emoji（⚡✅）→ 圆点存在性断言（StatusDot 7px 圆形 div，F3 四态渲染层）
 *   - 旧空态文案已废除——空态 = 无会话行
 *   - 行默认收起：项目/页面行点击展开（NavTree 组件内展开态，切换视图 display:none 保挂载不丢）
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";
import {
  waitForWorkspaceReady,
  waitForDockviewApi,
  waitForPtySessionReady,
  createProject,
  addTerminalPanel,
  ensureHooksInjected,
  writeSignalFile,
  addPage,
  switchToPageAndWait,
  getProjectIdForPage,
} from "./specUtils";

describe("Agent 会话视图与 toast 通知", () => {
  /** 打开 nav 视图（幂等：已打开不重复 toggle，防 R2 关闭） */
  async function openNavView(): Promise<void> {
    await waitForWorkspaceReady();
    const s = await browser.execute(() => (window as any).__slterm_e2e_getSideBarState?.() ?? null);
    if (s?.open.top !== "nav") {
      await browser.execute(() => (window as any).__slterm_e2e_toggleSideView?.("nav"));
    }
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="nav-tree"]')),
      { timeout: 10000, timeoutMsg: "nav 视图未渲染" },
    );
  }

  /**
   * 展开导航树到会话行可见（幂等，遍历全部项目/页面行）：
   * 项目持久化累积（slterminal-projects.json 跨 run 保留）——目标项目可能位于
   * 导航树任意位置，必须展开全部未展开行。
   * 展开态判定（DOM 结构）：项目行父容器 = [项目行, 展开时 +子级容器（页面行 +
   *   历史节点随项目展开渲染其中——NAV-10 修订 2026-09-03）] → children > 1 即展开；
   *   页面行父容器 = [页面行, 展开时 +子级容器] → children > 1 即展开。
   * 页面行点击 = 切换展开 + 切页（switchPage 幂等，切页不碍行归属渲染）。
   * NavTree 展开态组件内维护，nav 视图 display:none 保挂载不丢。
   */
  async function ensureTreeExpanded(): Promise<void> {
    // 只展开「当前活跃项目」（含「当前」pill）的行——页面行点击 = 切页 + 初始化
    // Dockview，点击其它项目的页面行会把 activePageId 切走（useAgentStatus 只建
    // 活跃项目行，切走后信号建行被拒——实测根因）。多轮收敛（React 重渲染异步——
    // 单轮内判定基于旧 DOM，已展开行会被误判折叠）。
    for (let i = 0; i < 6; i++) {
      const clicked = await browser.execute(() => {
        let any = false;
        const proj = Array.from(
          document.querySelectorAll('[data-e2e="nav-row-project"]'),
        ).find((p) => (p.textContent ?? "").includes("当前"));
        if (!proj) return false;
        const container = proj.parentElement as HTMLElement | null;
        if (!container) return false;
        // 项目行未展开（容器仅项目行 1 子级——历史节点随项目展开渲染，NAV-10 修订）→ 点击展开
        if (container.children.length <= 1) {
          (proj as HTMLElement).click();
          any = true;
        }
        // 仅当前项目容器内的页面行（点击 = 切换展开 + 切页——当前项目即活跃页所属，幂等）
        const pages = container.querySelectorAll(
          '[data-e2e="nav-row-page"]',
        );
        for (const pg of pages) {
          if ((pg.parentElement?.children.length ?? 0) <= 1) {
            (pg as HTMLElement).click();
            any = true;
          }
        }
        return any;
      });
      if (!clicked) return;
      // 条件等待展开结果出现（替代固定 350ms sleep——TQ-E-03）：
      // 条件 = 当前项目容器已展开（nav-row-page 已渲染）——本轮点击的 React 提交
      // 落地后，下一轮判定（querySelectorAll nav-row-page）与后续会话行断言才基于
      // 新 DOM。页面行无会话时展开不渲染子级容器（DOM 无变化），故以项目展开为统一
      // 收敛点；toggleExpand 为 functional setState 逐次生效，每轮点击各自提交后
      // 奇数次翻转必然到达展开稳态。
      await browser.waitUntil(
        async () =>
          await browser.execute(() => {
            const proj = Array.from(
              document.querySelectorAll('[data-e2e="nav-row-project"]'),
            ).find((p) => (p.textContent ?? "").includes("当前"));
            if (!proj) return false;
            const container = proj.parentElement as HTMLElement | null;
            if (!container) return false;
            // 展开结果：项目容器内已渲染页面行（收起态无页面容器）
            return container.querySelectorAll('[data-e2e="nav-row-page"]').length > 0;
          }),
        { timeout: 5000, interval: 100, timeoutMsg: "树节点展开超时" },
      );
    }
  }

  /**
   * 等待活跃会话行出现/消失（动态四态用例用）。
   * 导航树会话行 = nav-row-session（活跃行带 data-panel-id；历史行无——按 panelId 区分）。
   * ⚡/✅ emoji 断言 → 行内圆点（StatusDot 7px 圆形 div）存在性断言（NAV-10 契约）。
   */
  async function waitForSessionRow(opts: {
    panelId?: string;
    dot?: boolean;
    text?: string;
    gone?: boolean;
  }): Promise<void> {
    await browser.waitUntil(
      async () => {
        await ensureTreeExpanded();
        // 读取必须发生在 execute 内部返回纯数据——execute 返回 DOM 元素会被
        // WebDriver 序列化为元素引用对象，外层读 textContent/属性恒为 undefined
        const state = await browser.execute((pid: string) => {
          const rows = Array.from(
            document.querySelectorAll('[data-e2e="nav-row-session"]'),
          ) as HTMLElement[];
          // 活跃会话行按 data-panel-id 过滤（历史行无此属性）
          const row = pid
            ? rows.find((r) => r.getAttribute("data-panel-id") === pid)
            : rows[0];
          if (!row) return { exists: false, text: "", hasDot: false };
          return {
            exists: true,
            text: row.textContent ?? "",
            // StatusDot = 7px 圆形 div（borderRadius 50%，aria-hidden）
            hasDot: Array.from(row.querySelectorAll("div")).some(
              (d) =>
                (d as HTMLElement).style.borderRadius === "50%" &&
                (d as HTMLElement).style.width === "7px",
            ),
          };
        }, opts.panelId ?? "");
        if (!state.exists) return opts.gone === true;
        if (opts.gone) return false;
        if (opts.dot && !state.hasDot) return false;
        if (opts.text && !state.text.includes(opts.text)) return false;
        return true;
      },
      { timeout: 15000, timeoutMsg: `会话行未在期限内${opts.gone ? "消失" : "出现"}` },
    );
  }

  /**
   * 用例 1：导航树视图存在性验证。
   * 通过 __slterm_e2e_toggleSideView("nav") 打开视图，
   * 断言侧栏槽位 sidebar-slot-top-nav 可见 + 分组标题「导航」渲染。
   */
  it("nav 视图可通过活动栏按钮打开", async () => {
    // 0. 等待 Workspace 就绪
    await waitForWorkspaceReady();

    // 1. 创建测试项目
    await createProject("C:\\e2e-nav-tree");

    // 2. 等待活动栏按钮渲染（nav 在 DEFAULT_ZONES.top 中，按钮始终存在）
    await browser.waitUntil(
      async () => await browser.execute(() => {
        return !!document.querySelector('[data-e2e="activity-btn-nav"]');
      }),
      { timeout: 10000, timeoutMsg: "nav 活动栏按钮未渲染" },
    );

    // 3. 重置侧栏为已知状态（避免持久化残留导致 open.top 已有其他视图）
    await browser.execute(() => {
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const getState = (window as any).__slterm_e2e_getSideBarState;
      if (typeof toggle !== "function" || typeof getState !== "function") return;
      const s = getState();
      // 关闭 top 区非 nav 的视图
      if (s?.open.top && s.open.top !== "nav") toggle(s.open.top);
      // 若 top 为空则打开 nav
      if (!s?.open.top) toggle("nav");
    });

    // 4. 断言 open.top === "nav"（toggle 已生效）
    const sideBarState = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(sideBarState).not.toBeNull();
    expect(sideBarState!.open.top).toBe("nav");

    // 5. 断言侧栏槽位存在且可见（display !== "none"）
    const slotVisible = await browser.execute(() => {
      const slot = document.querySelector('[data-e2e="sidebar-slot-top-nav"]');
      if (!slot) return false;
      const style = window.getComputedStyle(slot);
      return style.display !== "none";
    });
    expect(slotVisible).toBe(true);

    // 6. 断言 nav-tree 存在（NavTree 已挂载）
    const viewExists = await browser.execute(() => {
      return !!document.querySelector('[data-e2e="nav-tree"]');
    });
    expect(viewExists).toBe(true);

    // 7. 断言分组标题「导航」存在
    const headerText = await browser.execute(() => {
      const view = document.querySelector('[data-e2e="nav-tree"]');
      return view?.textContent ?? "";
    });
    expect(headerText).toContain("导航");

    // 8. 断言初始态为空态（无终端面板 → 无项目树外行；项目行存在）
    const hasProjectRow = await browser.execute(() => {
      return !!document.querySelector('[data-e2e="nav-row-project"]');
    });
    expect(hasProjectRow).toBe(true);
  });

  /**
   * 用例 2a：纯 shell 终端无活跃会话行（行建模语义——仅 agentSession 非 null 才建行）。
   *
   * 原理：useAgentStatus 初始扫描只建 agentSession 非 null 的行。
   * 纯 shell 终端（未运行编码 CLI、未注入 hooks）的 agentSession 为 null，
   * 因此导航树无活跃会话行（nav-row-session + data-panel-id）。
   */
  it("纯 shell 终端无活跃会话行（行建模语义——不自动建行）", async () => {
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-pureshell-"));
    try {
      // 0a. Workspace 就绪
      await waitForWorkspaceReady();

      // 0b. 创建项目 → 获取 pageId
      const pageId = await createProject(tempDir);

      // 0c. Dockview API
      await waitForDockviewApi();

      // 1. 创建终端面板（纯 shell——不注入 hooks、不运行编码 CLI，agentSession 为 null）
      const panelId = `terminal-${pageId}-0`;
      await addTerminalPanel(panelId);

      // 2. 等待 PTY session 就绪（TerminalRegistry 注册）
      await waitForPtySessionReady();

      // 3. 打开 nav 视图
      await openNavView();

      // 4. 展开导航树到页面级
      await ensureTreeExpanded();

      // 5. 断言无该 panelId 的会话行——纯 shell 终端的 agentSession 为 null，不建行
      const rowExists = await browser.execute((pid: string) => {
        return Array.from(
          document.querySelectorAll('[data-e2e="nav-row-session"]'),
        ).some((r) => r.getAttribute("data-panel-id") === pid);
      }, panelId);
      expect(rowExists).toBe(false);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  /**
   * 用例 2b：动态四态——Node 端原子写信号文件驱动状态流转。
   * PreToolUse → 建行（圆点出现）、Stop → 行保持（圆点仍存在）、SessionEnd → 行消失。
   */
  it("动态四态（PreToolUse→建行, Stop→行保持, SessionEnd→行消失）", async () => {
    const eventsDir = join(homedir(), ".slterminal", "hooks-events");
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-dyn-"));
    const signalFiles: string[] = [];

    try {
      // 0a. Workspace 就绪
      await waitForWorkspaceReady();

      // 0b. 确保 hooks 已注入
      await ensureHooksInjected();

      // 0c. 创建项目 → 获取 pageId
      const pageId = await createProject(tempDir);

      // 0d. Dockview API
      await waitForDockviewApi();

      // 1. 创建终端面板
      const panelId = `terminal-${pageId}-0`;
      await addTerminalPanel(panelId);

      // 2. 等待 PTY session 就绪
      await waitForPtySessionReady();

      // 3. 打开 nav 视图
      await openNavView();

      // 4. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 5. 原子写 PreToolUse 信号文件 → 建行（首个 hook 事件即建行）+ 圆点出现
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-dyn",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      await waitForSessionRow({ panelId, dot: true });

      // 6. 原子写 Stop 信号文件 → 行保持（圆点仍存在，done 灰档）
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "Stop",
        timestamp: Date.now(),
        sessionId: "e2e-agent-dyn",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      }));

      await waitForSessionRow({ panelId, dot: true });

      // 7. 原子写 SessionEnd 信号文件 → 行消失
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "SessionEnd",
        timestamp: Date.now(),
        sessionId: "e2e-agent-dyn",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      }));

      await waitForSessionRow({ panelId, gone: true });
    } finally {
      // 清理信号文件
      for (const f of signalFiles) {
        try { rmSync(f, { force: true }); } catch { /* 忽略 */ }
      }
      // 清理临时目录
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /**
   * 用例 2c（R2 变体）：切项目往返后用量经 ContextUsage 信号恢复。
   *
   * 验证：PreToolUse 信号建行 → ContextUsage 信号（usedPercentage 官方口径）
   * → 行含量化百分比 → 切项目往返（行重建，usage 短暂 "--"）→ 再发
   * ContextUsage 信号 → 用量恢复。L4 级覆盖：statusline 桥接信号通道全链路
   * （信号文件 → watcher → agent-event → useAgentStatus 行更新，真实二进制非 mock）。
   */
  it("R2 变体：切项目往返后用量经 ContextUsage 信号恢复（官方 used_percentage 口径）", async () => {
    const eventsDir = join(homedir(), ".slterminal", "hooks-events");
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-r2-"));
    const signalFiles: string[] = [];

    try {
      // 0a. Workspace 就绪
      await waitForWorkspaceReady();

      // 0b. 确保 hooks 已注入
      await ensureHooksInjected();

      // 0c. 创建项目 → 获取 pageId
      const page1Id = await createProject(tempDir);

      // 0d. Dockview API
      await waitForDockviewApi();

      // 1. 创建终端面板
      const panelId = `terminal-${page1Id}-0`;
      await addTerminalPanel(panelId);

      // 2. 等待 PTY session 就绪
      await waitForPtySessionReady();

      // 3. 打开 nav 视图
      await openNavView();

      // 4. 断言纯 shell 无行（初始扫描已触发）
      await ensureTreeExpanded();
      let rowExists = await browser.execute((pid: string) => {
        return Array.from(
          document.querySelectorAll('[data-e2e="nav-row-session"]'),
        ).some((r) => r.getAttribute("data-panel-id") === pid);
      }, panelId);
      expect(rowExists).toBe(false);

      // 5. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 6. 原子写 PreToolUse 信号文件——建行（不含 usageSourcePath，transcript 链路已退役）
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r2",
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      // 7. 轮询行出现
      await waitForSessionRow({ panelId });

      // 8. ContextUsage 信号（官方 used_percentage 口径）→ 行用量 50%
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        cliId: "claude",
        event: "ContextUsage",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r2",
        cwd: tempDir,
        usedPercentage: 50,
      }));

      await waitForSessionRow({ panelId, text: "50%" });

      // 9. 获取 projectId，创建 page2
      const projectId = await getProjectIdForPage(page1Id);
      if (!projectId) throw new Error("无法获取 projectId");

      const page2Id = await addPage(projectId, "page2", tempDir);

      // 10-11. 切换往返（E2E-10：waitUntil 轮询 activePageId，替代固定 pause）
      await switchToPageAndWait(page2Id);
      await switchToPageAndWait(page1Id);

      // 12. 行重建后 usage 短暂 "--"，再发 ContextUsage 信号 → 50% 恢复
      //     （行重建 = usage undefined；桥接通道每 1s 节流推送，信号即数据）
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        cliId: "claude",
        event: "ContextUsage",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r2",
        cwd: tempDir,
        usedPercentage: 50,
      }));
      await waitForSessionRow({ panelId, text: "50%" });
    } finally {
      // 清理信号文件
      for (const f of signalFiles) {
        try { rmSync(f, { force: true }); } catch { /* 忽略 */ }
      }
      // 清理临时目录
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /**
   * 用例 2d（R3 变体）：SessionEnd 删行 + 切项目往返不复活。
   */
  it("R3 变体：SessionEnd 删行 + 切项目往返不复活", async () => {
    const eventsDir = join(homedir(), ".slterminal", "hooks-events");
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-r3-"));
    const signalFiles: string[] = [];

    try {
      // 0a. Workspace 就绪
      await waitForWorkspaceReady();

      // 0b. 确保 hooks 已注入
      await ensureHooksInjected();

      // 0c. 创建项目 → pageId
      const page1Id = await createProject(tempDir);

      // 0d. Dockview API
      await waitForDockviewApi();

      // 1. 创建终端面板
      const panelId = `terminal-${page1Id}-0`;
      await addTerminalPanel(panelId);

      // 2. 等待 PTY session 就绪
      await waitForPtySessionReady();

      // 3. 打开 nav 视图
      await openNavView();

      // 4. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 5. 原子写 PreToolUse 信号文件 → 建行
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r3",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      // 6. 等待行出现
      await waitForSessionRow({ panelId });

      // 7. 原子写 SessionEnd 信号文件 → 删行
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "SessionEnd",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r3",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      }));

      // 8. 等待行消失
      await waitForSessionRow({ panelId, gone: true });

      // 9. 获取 projectId，创建 page2
      const projectId = await getProjectIdForPage(page1Id);
      if (!projectId) throw new Error("无法获取 projectId");

      const page2Id = await addPage(projectId, "page2", tempDir);

      // 10-11. 切换往返（E2E-10：waitUntil 轮询 activePageId，替代固定 pause）
      await switchToPageAndWait(page2Id);
      await switchToPageAndWait(page1Id);

      // 12. 断言行仍不存在——agentSession 已 null，初始扫描不建行
      const rowStillGone = await browser.execute((pid: string) => {
        return !Array.from(
          document.querySelectorAll('[data-e2e="nav-row-session"]'),
        ).some((r) => r.getAttribute("data-panel-id") === pid);
      }, panelId);
      expect(rowStillGone).toBe(true);
    } finally {
      for (const f of signalFiles) {
        try { rmSync(f, { force: true }); } catch { /* 忽略 */ }
      }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /**
   * 用例 2e（R4 变体）：会话终端关页签删行（panel.api.close() 路径）。
   *
   * 验证：hook 事件建行 → panel.api.close()（照 globalCommands.ts closeTab 先例）→ 行消失。
   * R4 原始探针教训：panel?.close is not a function——close() 在 panel.api 上，不在 panel 上。
   * R4 原始竞态（remove 事件丢失）由 deps [] 稳定订阅 + reconcile 对账根治——本用例守卫不复现。
   */
  it("R4 变体：会话终端关页签删行（closePanel）", async () => {
    const eventsDir = join(homedir(), ".slterminal", "hooks-events");
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-r4-"));
    const signalFiles: string[] = [];

    try {
      // 0a. Workspace 就绪
      await waitForWorkspaceReady();

      // 0b. 确保 hooks 已注入
      await ensureHooksInjected();

      // 0c. 创建项目 → pageId
      const pageId = await createProject(tempDir);

      // 0d. Dockview API
      await waitForDockviewApi();

      // 1. 创建终端面板
      const panelId = `terminal-${pageId}-0`;
      await addTerminalPanel(panelId);

      // 2. 等待 PTY session 就绪
      await waitForPtySessionReady();

      // 3. 打开 nav 视图
      await openNavView();

      // 4. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 5. 原子写 PreToolUse 信号文件 → 建行
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r4",
        usageSourcePath: "",
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      // 6. 等待行出现
      await waitForSessionRow({ panelId });

      // 7. 断言行存在
      let rowExists = await browser.execute((pid: string) => {
        return Array.from(
          document.querySelectorAll('[data-e2e="nav-row-session"]'),
        ).some((r) => r.getAttribute("data-panel-id") === pid);
      }, panelId);
      expect(rowExists).toBe(true);

      // 8. 关闭终端面板页签——panel.api.close()（照 globalCommands.ts closeTab 先例）
      await browser.execute((pid: string) => {
        const panel = window.__dockviewApi?.getPanel(pid);
        if (panel) {
          // panel.api.close() = dockview-react PanelApi 的 close 方法
          // R4 原始探针教训：panel.close() 不存在——close() 在 panel.api 上
          panel.api.close();
        }
      }, panelId);

      // 9. 等待行消失（remove 事件 → useAgentStatus 删行，deps [] 稳定订阅不丢事件）
      await waitForSessionRow({ panelId, gone: true });

      // 10. 断言行不存在（R4 原始竞态不复现——deps [] 稳定订阅 + reconcile 兜底）
      const rowGone = await browser.execute((pid: string) => {
        return !Array.from(
          document.querySelectorAll('[data-e2e="nav-row-session"]'),
        ).some((r) => r.getAttribute("data-panel-id") === pid);
      }, panelId);
      expect(rowGone).toBe(true);
    } finally {
      for (const f of signalFiles) {
        try { rmSync(f, { force: true }); } catch { /* 忽略 */ }
      }
      try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    }
  });

  /**
   * 用例 3：toast 触发链路（失焦 + 权限请求 / Stop / 错误）。
   *
   * 真实验证步骤（人工）：
   *   1. 启动 slTerminal 并注入 hooks（设置 → 注入 Claude Code hooks）。
   *   2. 打开终端，运行 claude。
   *   3. 触发 PermissionRequest：在 claude 中输入需工具调用的 prompt，
   *      如 "请列出 C:\ 目录下的文件"。
   *   4. 立即切换到其他窗口（Alt+Tab）使 slTerminal 失焦。
   *   5. 观察系统通知中心 → 应弹出 slTerminal 通知，含 "🔐 权限请求" 字样。
   *   6. 点击该通知 → 窗口应聚焦回 slTerminal 并切换到对应终端页签。
   *   7. 继续在 claude 中等待任务完成（Stop 事件）：
   *      - 保持 slTerminal 失焦 → 系统通知中心出现 "✅ 任务完成" 通知。
   *   8. 构造错误场景：在 claude 中执行一个必然失败的工具调用 →
   *      系统通知中心出现 "❌ 错误" 通知。
   *   9. 点击停止（Stop）事件通知 → 验证窗口聚焦 + 路由到对应面板。
   *
   * E2E 自动化不可行原因：
   *   - embedded WDIO 驱动无法控制 WebView2 窗口焦点
   *     （onFocusChanged 事件由 OS 窗口管理器触发，不可合成）。
   *   - 系统通知中心不可编程访问（无法查询已发送的通知列表，
   *     无法模拟用户点击通知）。
   *   - Web Notification API 在 headless/自动化 WebView2 中
   *     不产生真实的桌面通知弹窗。
   *   - useClaudeNotifications 的门控条件
   *     window.__slterm_windowFocused === false 在自动化环境中
   *     始终为 true（窗口聚焦），通知绝不会触发。
   *
   * 未来自动化方向：
   *   待 @tauri-apps/plugin-notification 支持程序化查询/触发通知后，
   *   可修改 useClaudeNotifications 暴露 sendNotification 调用的 spy，
   *   在 E2E 中通过 browser.execute 设置 __slterm_windowFocused = false
   *   后注入合成 agent-event，再验证 spy 被调用参数。
   */
  it.skip("toast 触发链路需人工验证（失焦 + 权限请求 / Stop / 错误）", async () => {
    // 骨架保留供未来自动化参考。
    //
    // 前置：
    //   1. hooks 已注入 → onAgentEvent 正常工作
    //   2. 终端面板存在 → panelId 已知
    //   3. window.__slterm_windowFocused = false → 失焦门控通过
    //
    // 验证断言（自动化后启用）：
    //   1. inject PermissionRequest agent-event → sendClickableNotification 被调用
    //      参数 title="slTerminal"，body 含 "🔐 权限请求"
    //   2. inject Stop agent-event → sendClickableNotification 被调用
    //      参数 body 含 "✅ 任务完成"
    //   3. inject StopFailure agent-event → sendClickableNotification 被调用
    //      参数 body 含 "❌ 错误"
    //   4. Notification onclick → setFocus + setActivePage + panel.focus 被调用
    //   5. 非通知类事件（PreToolUse/PostToolUse/SessionStart/SessionEnd）
    //      → sendClickableNotification 不被调用
  });
});
