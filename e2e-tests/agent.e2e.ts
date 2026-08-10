/**
 * Agent Status 域 E2E spec（E2E-09 拆分 + E2E-10 waitUntil 替换）：
 * 视图存在性、纯 shell 无行、动态四态、R2 用量保持、R3 删行不复活、
 * R4 关页签删行、toast 触发链路（skip——人工验证）。
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

describe("Agent Status 视图与 toast 通知", () => {
  /** 打开 agent-status 视图（幂等：已打开不重复 toggle，防 R2 关闭） */
  async function openAgentStatusView(): Promise<void> {
    await waitForWorkspaceReady();
    const s = await browser.execute(() => (window as any).__slterm_e2e_getSideBarState?.() ?? null);
    if (s?.open.top !== "agent-status") {
      await browser.execute(() => (window as any).__slterm_e2e_toggleSideView?.("agent-status"));
    }
    await browser.waitUntil(
      async () =>
        await browser.execute(() => !!document.querySelector('[data-e2e="agent-status-view"]')),
      { timeout: 10000, timeoutMsg: "agent-status 视图未渲染" },
    );
  }

  /** 等待 agent-status 行出现/消失（动态四态用例用） */
  async function waitForStatusRow(opts: { panelId?: string; emoji?: string; gone?: boolean }): Promise<void> {
    await browser.waitUntil(
      async () => {
        // 读取必须发生在 execute 内部返回纯数据——execute 返回 DOM 元素会被
        // WebDriver 序列化为元素引用对象，外层读 textContent/属性恒为 undefined
        const state = await browser.execute(() => {
          const row = document.querySelector('[data-e2e="agent-status-row"]');
          if (!row) return { exists: false, text: "", panelId: null };
          return {
            exists: true,
            text: (row as HTMLElement).textContent ?? "",
            panelId: (row as HTMLElement).getAttribute("data-panel-id"),
          };
        });
        if (!state.exists) return opts.gone === true;
        if (opts.gone) return state.panelId !== opts.panelId;
        if (opts.emoji && !state.text.includes(opts.emoji)) return false;
        if (opts.panelId && state.panelId !== opts.panelId) return false;
        return true;
      },
      { timeout: 15000, timeoutMsg: `agent-status-row 未在期限内${opts.gone ? "消失" : "出现"}` },
    );
  }

  /**
   * 用例 1：Agent Status 视图存在性验证。
   * 通过 __slterm_e2e_toggleSideView("agent-status") 打开视图，
   * 断言侧栏槽位 sidebar-slot-top-agent-status 可见 + AGENT STATUS 标题栏渲染。
   */
  it("Agent Status 视图可通过活动栏按钮打开", async () => {
    // 0. 等待 Workspace 就绪
    await waitForWorkspaceReady();

    // 1. 创建测试项目
    await createProject("C:\\e2e-agent-status");

    // 2. 等待活动栏按钮渲染（agent-status 在 DEFAULT_ZONES.top 中，按钮始终存在）
    await browser.waitUntil(
      async () => await browser.execute(() => {
        return !!document.querySelector('[data-e2e="activity-btn-agent-status"]');
      }),
      { timeout: 10000, timeoutMsg: "agent-status 活动栏按钮未渲染" },
    );

    // 3. 重置侧栏为已知状态（避免持久化残留导致 open.top 已有其他视图）
    await browser.execute(() => {
      const toggle = (window as any).__slterm_e2e_toggleSideView;
      const getState = (window as any).__slterm_e2e_getSideBarState;
      if (typeof toggle !== "function" || typeof getState !== "function") return;
      const s = getState();
      // 关闭 top 区非 agent-status 的视图
      if (s?.open.top && s.open.top !== "agent-status") toggle(s.open.top);
      // 若 top 为空则打开 agent-status
      if (!s?.open.top) toggle("agent-status");
    });

    // 4. 断言 open.top === "agent-status"（toggle 已生效）
    const sideBarState = await browser.execute(() => {
      const fn = (window as any).__slterm_e2e_getSideBarState;
      return typeof fn === "function" ? fn() : null;
    });
    expect(sideBarState).not.toBeNull();
    expect(sideBarState!.open.top).toBe("agent-status");

    // 5. 断言侧栏槽位存在且可见（display !== "none"）
    const slotVisible = await browser.execute(() => {
      const slot = document.querySelector('[data-e2e="sidebar-slot-top-agent-status"]');
      if (!slot) return false;
      const style = window.getComputedStyle(slot);
      return style.display !== "none";
    });
    expect(slotVisible).toBe(true);

    // 6. 断言 agent-status-view 存在（AgentStatusView 已挂载）
    const viewExists = await browser.execute(() => {
      return !!document.querySelector('[data-e2e="agent-status-view"]');
    });
    expect(viewExists).toBe(true);

    // 7. 断言 "AGENT STATUS" 标题栏文本存在
    const headerText = await browser.execute(() => {
      const view = document.querySelector('[data-e2e="agent-status-view"]');
      return view?.textContent ?? "";
    });
    expect(headerText).toContain("AGENT STATUS");

    // 8. 断言初始态为空态或 no-root 提示（此时无终端面板）
    const hasHint = await browser.execute(() => {
      const text = document.querySelector('[data-e2e="agent-status-view"]')?.textContent ?? "";
      return text.includes("选择一个项目") || text.includes("无运行中的编码 CLI 会话");
    });
    expect(hasHint).toBe(true);
  });

  /**
   * 用例 2a：Agent Status 纯 shell 终端无行（行建模改后语义——仅 agentSession 非 null 才建行）。
   *
   * 原理：useAgentStatus 初始扫描只建 agentSession 非 null 的行。
   * 纯 shell 终端（未运行编码 CLI、未注入 hooks）的 agentSession 为 null，
   * 因此 agent-status-row 不出现。用例 1 空态文案断言保留作回归。
   *
   * E2E-10：原 500ms 固定等待（等初始扫描）→ 改为 waitUntil 轮询
   * 视图挂载完成（useEffect 已触发扫描）——比固定时长更精确且更快。
   */
  it("Agent Status 纯 shell 终端无行（行建模新语义——不自动建行）", async () => {
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

      // 3. 打开 agent-status 视图
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("agent-status");
      });

      // 4. 等待视图挂载完成（E2E-10：轮询 agent-status-view 渲染，替代 pause(500)；
      //    视图挂载即 useAgentStatus useEffect 已执行——初始扫描已触发）
      await browser.waitUntil(
        async () =>
          await browser.execute(() => !!document.querySelector('[data-e2e="agent-status-view"]')),
        { timeout: 10000, timeoutMsg: "agent-status 视图未在 toggle 后渲染" },
      );

      // 5. 断言 agent-status-row 不存在——纯 shell 终端的 agentSession 为 null，不建行
      const rowExists = await browser.execute(() => {
        return !!document.querySelector('[data-e2e="agent-status-row"]');
      });
      expect(rowExists).toBe(false);

      // 6. 断言空态或 no-root 提示文案存在（用例 1 回归——纯 shell 项目应显示空态）
      const hasHint = await browser.execute(() => {
        const text = document.querySelector('[data-e2e="agent-status-view"]')?.textContent ?? "";
        return text.includes("选择一个项目") || text.includes("无运行中的编码 CLI 会话");
      });
      expect(hasHint).toBe(true);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  /**
   * 用例 2b：Agent Status 动态四态——Node 端原子写信号文件驱动状态流转。
   */
  it("Agent Status 动态四态（PreToolUse→⚡, Stop→✅, SessionEnd→行消失）", async () => {
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

      // 3. 打开 agent-status 视图
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("agent-status");
      });

      // 4. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 5. 原子写 PreToolUse 信号文件 → 断言行出现 ⚡（行建模改后首个 hook 事件即建行）
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-dyn",
        transcriptPath: "",
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      await waitForStatusRow({ panelId, emoji: "⚡" });

      // 6. 原子写 Stop 信号文件 → 断言行出现 ✅
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "Stop",
        timestamp: Date.now(),
        sessionId: "e2e-agent-dyn",
        transcriptPath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      }));

      await waitForStatusRow({ panelId, emoji: "✅" });

      // 7. 原子写 SessionEnd 信号文件 → 断言行消失
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "SessionEnd",
        timestamp: Date.now(),
        sessionId: "e2e-agent-dyn",
        transcriptPath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      }));

      await waitForStatusRow({ panelId, gone: true });
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
   * 用例 2c（R2 变体）：切项目往返后用量保持。
   *
   * 验证：假 transcript JSONL（含 message.usage 四字段）→ 信号文件携真实
   * transcriptPath 建行 → contextUsage 后端真实解析 → 行含量化百分比 →
   * 切项目往返（addPage → switchToPage → switchToPage 回）→ 用量数值保持。
   * L4 级覆盖：cache 口径全链路（后端 hooks_context_usage 真实解析，非 mock）。
   */
  it("R2 变体：切项目往返后用量保持（contextUsage 全链路 + cache 字段）", async () => {
    const eventsDir = join(homedir(), ".slterminal", "hooks-events");
    const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-agent-r2-"));
    const signalFiles: string[] = [];

    try {
      // 0a. Workspace 就绪
      await waitForWorkspaceReady();

      // 0b. 确保 hooks 已注入
      await ensureHooksInjected();

      // 0c. 写假 transcript JSONL——含 message.usage 四字段
      //     input=30000 + cacheRead=50000 + cacheCreation=20000 = 100000 / 200000 = 50%
      const transcriptDir = join(tempDir, ".claude", "transcripts");
      mkdirSync(transcriptDir, { recursive: true });
      const transcriptPath = join(transcriptDir, "e2e-r2-transcript.jsonl");
      const usageLine = JSON.stringify({
        message: {
          usage: {
            input_tokens: 30000,
            output_tokens: 1000,
            cache_read_input_tokens: 50000,
            cache_creation_input_tokens: 20000,
          },
        },
      });
      writeFileSync(transcriptPath, usageLine + "\n", "utf8");

      // 0d. 创建项目 → 获取 pageId
      const page1Id = await createProject(tempDir);

      // 0e. Dockview API
      await waitForDockviewApi();

      // 1. 创建终端面板
      const panelId = `terminal-${page1Id}-0`;
      await addTerminalPanel(panelId);

      // 2. 等待 PTY session 就绪
      await waitForPtySessionReady();

      // 3. 打开 agent-status 视图
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("agent-status");
      });

      // 4. 断言纯 shell 无行（E2E-10：轮询视图挂载完成替代 pause(500)——初始扫描已触发）
      await browser.waitUntil(
        async () =>
          await browser.execute(() => !!document.querySelector('[data-e2e="agent-status-view"]')),
        { timeout: 10000, timeoutMsg: "agent-status 视图未渲染" },
      );
      let rowExists = await browser.execute(() => {
        return !!document.querySelector('[data-e2e="agent-status-row"]');
      });
      expect(rowExists).toBe(false);

      // 5. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 6. 原子写 PreToolUse 信号文件——携真实 transcriptPath 建行 + usage 拉取
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r2",
        transcriptPath, // 真实 transcript 路径 → contextUsage 后端解析
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      // 7. 轮询行出现且含 ⚡
      await waitForStatusRow({ panelId, emoji: "⚡" });

      // 8. 等待用量异步拉取完成（contextUsage 是异步的，轮询直到不是 "--"）
      await browser.waitUntil(
        async () => await browser.execute(() => {
          const row = document.querySelector('[data-e2e="agent-status-row"]');
          const text = row?.textContent ?? "";
          // 用量文本应为 "50%"（不含 "--"）
          return text.includes("50%") && !text.includes("--");
        }),
        { timeout: 10000, timeoutMsg: "用量百分比未在 contextUsage 拉取后出现 50%" },
      );

      // 9. 获取 projectId，创建 page2
      const projectId = await getProjectIdForPage(page1Id);
      if (!projectId) throw new Error("无法获取 projectId");

      const page2Id = await addPage(projectId, "page2", tempDir);

      // 10-11. 切换往返（E2E-10：waitUntil 轮询 activePageId，替代固定 pause）
      await switchToPageAndWait(page2Id);
      await switchToPageAndWait(page1Id);

      // 12. 断言行仍存在且用量保持（50%——初始扫描携 transcriptPath 主动拉取）
      const usageAfterSwitch = await browser.execute(() => {
        const row = document.querySelector('[data-e2e="agent-status-row"]');
        if (!row) return null;
        return row.textContent ?? "";
      });
      expect(usageAfterSwitch).not.toBeNull();
      expect(usageAfterSwitch).toContain("50%");
      // 50% = (30000 + 50000 + 20000) / 200000 —— 四字段完整口径（input + cacheRead + cacheCreation）
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

      // 3. 打开 agent-status 视图
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("agent-status");
      });

      // 4. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 5. 原子写 PreToolUse 信号文件 → 建行（含 ⚡）
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r3",
        transcriptPath: "",
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      // 6. 等待行出现含 ⚡
      await waitForStatusRow({ panelId, emoji: "⚡" });

      // 7. 原子写 SessionEnd 信号文件 → 删行
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "SessionEnd",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r3",
        transcriptPath: "",
        cwd: tempDir,
        toolName: null,
        notificationType: null,
      }));

      // 8. 等待行消失
      await waitForStatusRow({ panelId, gone: true });

      // 9. 获取 projectId，创建 page2
      const projectId = await getProjectIdForPage(page1Id);
      if (!projectId) throw new Error("无法获取 projectId");

      const page2Id = await addPage(projectId, "page2", tempDir);

      // 10-11. 切换往返（E2E-10：waitUntil 轮询 activePageId，替代固定 pause）
      await switchToPageAndWait(page2Id);
      await switchToPageAndWait(page1Id);

      // 12. 断言行仍不存在——agentSession 已 null，初始扫描不建行
      const rowStillGone = await browser.execute(() => {
        return !document.querySelector('[data-e2e="agent-status-row"]');
      });
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

      // 3. 打开 agent-status 视图
      await browser.execute(() => {
        (window as any).__slterm_e2e_toggleSideView?.("agent-status");
      });

      // 4. 确保信号目录存在
      mkdirSync(eventsDir, { recursive: true });

      // 5. 原子写 PreToolUse 信号文件 → 建行（含 ⚡）
      signalFiles.push(writeSignalFile(eventsDir, {
        panelId,
        event: "PreToolUse",
        timestamp: Date.now(),
        sessionId: "e2e-agent-r4",
        transcriptPath: "",
        cwd: tempDir,
        toolName: "Bash",
        notificationType: null,
      }));

      // 6. 等待行出现含 ⚡
      await waitForStatusRow({ panelId, emoji: "⚡" });

      // 7. 断言行存在
      let rowExists = await browser.execute((pid: string) => {
        const row = document.querySelector('[data-e2e="agent-status-row"]');
        return row?.getAttribute("data-panel-id") === pid;
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
      await waitForStatusRow({ panelId, gone: true });

      // 10. 断言行不存在（R4 原始竞态不复现——deps [] 稳定订阅 + reconcile 兜底）
      const rowGone = await browser.execute(() => {
        return !document.querySelector('[data-e2e="agent-status-row"]');
      });
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
   *   后注入合成 hook-event，再验证 spy 被调用参数。
   */
  it.skip("toast 触发链路需人工验证（失焦 + 权限请求 / Stop / 错误）", async () => {
    // 骨架保留供未来自动化参考。
    //
    // 前置：
    //   1. hooks 已注入 → onHookEvent 正常工作
    //   2. 终端面板存在 → panelId 已知
    //   3. window.__slterm_windowFocused = false → 失焦门控通过
    //
    // 验证断言（自动化后启用）：
    //   1. inject PermissionRequest hook-event → sendClickableNotification 被调用
    //      参数 title="slTerminal"，body 含 "🔐 权限请求"
    //   2. inject Stop hook-event → sendClickableNotification 被调用
    //      参数 body 含 "✅ 任务完成"
    //   3. inject StopFailure hook-event → sendClickableNotification 被调用
    //      参数 body 含 "❌ 错误"
    //   4. Notification onclick → setFocus + setActivePage + panel.focus 被调用
    //   5. 非通知类事件（PreToolUse/PostToolUse/SessionStart/SessionEnd）
    //      → sendClickableNotification 不被调用
  });
});
