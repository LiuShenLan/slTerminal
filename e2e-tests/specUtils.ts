/**
 * E2E spec 共享工具（Node 侧，仅被 *.e2e.ts spec 引用）。
 *
 * 重要：本文件与应用侧 `helpers.ts` 相互独立——helpers.ts 运行在前端 JS
 * 环境（被 main.tsx 动态 import，注入 window 全局 helper），本文件运行在
 * wdio/Node 环境（被 spec 文件 import）。二者禁止互相 import。
 *
 * 提供：Workspace/Dockview 就绪等待、项目/终端创建、PTY session 等待、
 * hooks 注入、信号文件原子写与消费等待、页面切换等待（E2E-10 用）、
 * 共享 setup `withProjectAndTerminal`（E2E-09 提取）。
 */

import { expect, browser } from "@wdio/globals";
import { mkdtempSync, rmSync, writeFileSync, renameSync, existsSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join } from "node:path";

// ── Window 全局类型扩展（E2E helper 由应用侧 helpers.ts 注入） ──

declare global {
  interface Window {
    /** Dockview 布局 API（workspace 层挂载，始终指向活跃页面） */
    __dockviewApi?: any;
  }
}

// ── 就绪等待 ──

/** 等待 Workspace 就绪（__slterm_e2e_workspaceReady === true） */
export async function waitForWorkspaceReady(timeout = 15000): Promise<void> {
  await browser.waitUntil(
    async () => await browser.execute(() => (window as any).__slterm_e2e_workspaceReady === true),
    { timeout, timeoutMsg: "Workspace 未就绪（__slterm_e2e_workspaceReady 超时）" },
  );
}

/** 等待 Dockview API 就绪 */
export async function waitForDockviewApi(timeout = 20000): Promise<void> {
  await browser.waitUntil(
    async () => await browser.execute(() => typeof window.__dockviewApi !== "undefined"),
    { timeout, timeoutMsg: "Dockview API 未就绪" },
  );
}

/** 等待面板标题变为指定值（轮询 Dockview panel title） */
export async function waitForPanelTitle(
  panelId: string,
  expectedTitle: string,
  timeout = 10000,
): Promise<string> {
  const title = await browser.waitUntil(
    async () => {
      const t = await browser.execute((pid: string) => {
        const panel = window.__dockviewApi?.getPanel(pid);
        return panel?.api.title ?? null;
      }, panelId);
      if (t === expectedTitle) return t;
      return false;
    },
    { timeout, timeoutMsg: `面板 ${panelId} 标题未在 ${timeout}ms 内变为 "${expectedTitle}"` },
  );
  return title as string;
}

/** 获取活跃页面信息（activePageId 反查 rootPath；无活跃页面返回 null） */
export async function getActivePageInfo(): Promise<{ pageId: string; rootPath: string } | null> {
  return browser.execute(() => (window as any).__slterm_e2e_getActivePageInfo?.() ?? null);
}

// ── 项目/终端 setup ──

/** 程序化创建测试项目（绕过原生对话框），返回 pageId */
export async function createProject(dirPath: string): Promise<string> {
  const pageId = await browser.execute((dir: string) => {
    return (window as any).__slterm_e2e_createProject?.(dir) ?? null;
  }, dirPath);
  if (!pageId) throw new Error(`__slterm_e2e_createProject 失败（dir=${dirPath}）`);
  return pageId;
}

/** 通过 __dockviewApi 添加终端面板 */
export async function addTerminalPanel(panelId: string): Promise<void> {
  await browser.execute((pid: string) => {
    window.__dockviewApi!.addPanel({
      id: pid,
      component: "terminal",
      params: { panelId: pid },
      renderer: "always" as const,
    });
  }, panelId);
}

/** 等待任一 terminal-container 的 PTY session 就绪；存在 __e2e_error 时抛出原因 */
export async function waitForPtySessionReady(timeout = 25000): Promise<void> {
  const state = await browser.waitUntil(
    async () => {
      const result = await browser.execute(() => {
        const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
        for (const c of containers) {
          const el = c as any;
          if (el.__e2e_sessionReady) return { ready: true };
          if (el.__e2e_error) return { error: el.__e2e_error };
        }
        return null;
      });
      return result;
    },
    { timeout, timeoutMsg: "PTY session 未就绪" },
  );
  if (state.error) {
    throw new Error(`PTY spawn 失败: ${state.error}`);
  }
}

/** 向第一个可用终端容器的 __e2e_writeToPty 写入文本（返回是否命中容器） */
export async function writeToPty(text: string): Promise<boolean> {
  return browser.execute((data: string) => {
    const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
    for (const c of containers) {
      const el = c as any;
      if (el.__e2e_writeToPty) {
        el.__e2e_writeToPty(data);
        return true;
      }
    }
    return false;
  }, text);
}

/** 读取第一个可用终端容器的缓冲文本（无容器返回 null） */
export async function getTerminalText(): Promise<string | null> {
  return browser.execute(() => {
    const containers = document.querySelectorAll('[data-e2e="terminal-container"]');
    for (const c of containers) {
      const el = c as any;
      if (typeof el.__e2e_getTerminalText === "function") {
        return el.__e2e_getTerminalText();
      }
    }
    return null;
  });
}

/** 轮询终端缓冲包含指定文本（返回文本；超时抛错） */
export async function waitForTerminalText(
  fragment: string,
  timeout = 25000,
  timeoutMsg?: string,
): Promise<string> {
  const text = await browser.waitUntil(
    async () => {
      const t = await getTerminalText();
      if (t && t.includes(fragment)) return t;
      return false;
    },
    { timeout, timeoutMsg: timeoutMsg ?? `终端缓冲未包含 ${fragment}` },
  );
  return text as string;
}

/** 在已有项目上新增操作页面（H6/agent 用例用），返回 page2Id */
export async function addPage(projectId: string, name: string, rootPath: string): Promise<string> {
  const pageId = await browser.execute(
    (args: { projId: string; name: string; rootPath: string }) => {
      return (window as any).__slterm_e2e_addPage?.(args.projId, args.name, args.rootPath) ?? null;
    },
    { projId: projectId, name, rootPath },
  );
  if (!pageId) throw new Error(`无法创建页面 ${name}`);
  return pageId;
}

/** 反查 pageId 所属 projectId */
export async function getProjectIdForPage(pageId: string): Promise<string | null> {
  return browser.execute((pid: string) => {
    return (window as any).__slterm_e2e_getProjectIdForPage?.(pid) ?? null;
  }, pageId);
}

/**
 * 切换页面并等待生效（E2E-10：替代 500ms 固定等待）。
 * 等待条件 = store 状态（activePageId 已指向目标页）——比固定时长更精确，
 * switchToPageShared 内 setProjectRoot 前置完成后才置 activePageId。
 */
export async function switchToPageAndWait(pageId: string, timeout = 10000): Promise<void> {
  await browser.execute((pid: string) => {
    (window as any).__slterm_e2e_switchToPage?.(pid);
  }, pageId);
  await browser.waitUntil(
    async () => {
      const info = await getActivePageInfo();
      return info?.pageId === pageId;
    },
    { timeout, timeoutMsg: `页面切换未生效（activePageId 未变为 ${pageId}）` },
  );
}

// ── hooks 注入（泛化命令 agent_hooks_* 六命令全表，PREAMBLE 契约段 1） ──
//
// spec 侧不直接 invoke——一律经应用侧 window helper（__slterm_e2e_*，helpers.ts）：
// helper 内 cliId 实参固定 "claude"（E2E 辅助代码属测试基建，字面量合法），
// 本文件无命令名字面量。六命令全表：agent_hooks_inject / agent_hooks_uninstall /
// agent_hooks_injection_status / agent_hooks_restore_statusline / agent_hooks_config_read /
// agent_hooks_config_write。

/** 确保 hooks 已注入（幂等：已注入跳过；注入是 spawn_blocking 异步，轮询状态；底层经 agent_hooks_inject → agent_hooks_injection_status） */
export async function ensureHooksInjected(timeout = 15000): Promise<void> {
  await browser.execute(() => (window as any).__slterm_e2e_injectHooks?.());
  await browser.waitUntil(
    async () => {
      const s = await browser.execute(() => (window as any).__slterm_e2e_getHookInjectionStatus?.());
      return s?.status === "injected";
    },
    { timeout, timeoutMsg: "hooks 未在期限内完成注入" },
  );
}

// ── 信号文件 ──

/** 信号文件原子写（.tmp → rename .json，与 hook 脚本同款 C2 备选 A 模式），返回最终路径 */
export function writeSignalFile(eventsDir: string, payload: Record<string, unknown>): string {
  const fileName = `${payload.panelId}-${payload.event}-${Date.now()}.json`;
  const tmpPath = join(eventsDir, fileName + ".tmp");
  const filePath = join(eventsDir, fileName);
  writeFileSync(tmpPath, JSON.stringify(payload), "utf8");
  renameSync(tmpPath, filePath);
  return filePath;
}

/** 等待信号文件被 watcher 消费（文件消失——notify 实时 + 3s 轮询兜底双路径） */
export async function waitForSignalConsumed(filePath: string, timeout = 8000): Promise<void> {
  await browser.waitUntil(
    async () => !existsSync(filePath),
    { timeout, timeoutMsg: `信号文件未被 watcher 消费（残留）: ${filePath}` },
  );
}

// ── 页签 emoji（F3 四态） ──

/**
 * 等待指定面板页签参数 tabIcon 变为期望值（null = 无图标）。
 *
 * 轮询 `getPanel(panelId).params.tabIcon`（updateParameters 更新的是面板参数，
 * 与 DefaultTab 渲染同一真值源）——精确匹配单一面板，免疫其他面板页签
 * 状态污染（如 Agent Status R2 用例无 SessionEnd 信号、页签 ⚡ 滞留的场景）。
 * Dockview 页签 DOM（.dv-tab）无面板标识属性，无法从 DOM 精确定位单面板页签，
 * 故经 params 断言流转；DefaultTab 的 emoji 渲染由 L2 workspace-defaulttab 覆盖。
 */
export async function waitForPanelTabIcon(
  panelId: string,
  expected: string | null,
  timeout = 15000,
): Promise<void> {
  await browser.waitUntil(
    async () => {
      const icon = await browser.execute((pid: string) => {
        const panel = window.__dockviewApi?.getPanel(pid);
        const v = panel?.params?.tabIcon;
        return v === undefined ? null : v;
      }, panelId);
      return icon === expected;
    },
    { timeout, timeoutMsg: `面板 ${panelId} tabIcon 未在期限内变为 ${expected}` },
  );
}

// ── 共享 setup（E2E-09） ──

/** withProjectAndTerminal 的返回结果 */
export interface WithProjectAndTerminalResult {
  /** 终端面板 id（约定格式 terminal-{pageId}-0，与 TerminalRegistry/useAgentStatus 解析一致） */
  panelId: string;
  /** 活跃页面 id */
  pageId: string;
  /** 所属项目 id */
  projectId: string;
  /** 项目根目录（临时目录，调用方 finally 调 cleanup） */
  tempDir: string;
  /** 递归清理临时目录（幂等） */
  cleanup: () => void;
}

/**
 * 共享 setup：创建临时项目 + 终端面板 + 等待 PTY session 就绪。
 * - `hooks: true` 时先确保 hooks 注入（Agent Status/历史四态类用例前置）。
 * - 返回 panelId/pageId/projectId/tempDir/cleanup。
 */
export async function withProjectAndTerminal(opts: { hooks?: boolean } = {}): Promise<WithProjectAndTerminalResult> {
  const tempDir = mkdtempSync(join(tmpdir(), "slterm-e2e-"));
  try {
    await waitForWorkspaceReady();
    if (opts.hooks) {
      await ensureHooksInjected();
    }
    const pageId = await createProject(tempDir);
    await waitForDockviewApi();
    const projectId = (await getProjectIdForPage(pageId)) ?? "";
    if (!projectId) throw new Error(`无法获取 projectId（pageId=${pageId}）`);
    const panelId = `terminal-${pageId}-0`;
    await addTerminalPanel(panelId);
    await waitForPtySessionReady();
    return {
      panelId,
      pageId,
      projectId,
      tempDir,
      cleanup: () => {
        try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
      },
    };
  } catch (err) {
    // setup 失败也要清理临时目录
    try { rmSync(tempDir, { recursive: true, force: true }); } catch { /* 忽略 */ }
    throw err;
  }
}

// ── 断言快捷 ──

/** 断言当前活跃页面 rootPath 等于预期值（恢复编排等用例用） */
export async function expectActivePageRootPath(rootPath: string): Promise<void> {
  const info = await browser.waitUntil(
    async () => {
      const i = await getActivePageInfo();
      return i?.rootPath === rootPath ? i : false;
    },
    { timeout: 15000, timeoutMsg: `活跃页面 rootPath 未在期限内变为 ${rootPath}` },
  );
  expect((info as { pageId: string; rootPath: string }).rootPath).toBe(rootPath);
}

/** 默认信号目录（~/.slterminal/hooks-events） */
export function defaultEventsDir(): string {
  return join(homedir(), ".slterminal", "hooks-events");
}
