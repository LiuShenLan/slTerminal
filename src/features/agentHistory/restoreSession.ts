// restoreSession — 历史会话恢复四步编排（FE-06，决策 6/25）
//
// 恢复流程（全部复用既有原语，不改 workspace/stores 代码）：
//   1. 项目入列：useProjects 查 rootPath 与 session.cwd 规范化相等（normalizePath + 忽略大小写，决策 24）
//      的项目，无则 addProject（字段形状照 SidebarTree.handleAddProject）
//   2. 页面保障：项目 pages 为空则 addPage（「页面-N」+ makeEmptyLayout 空布局，照 handleNewPage 模式）
//   3. 页面切换：switchToPageShared(pages[0].pageId)——setProjectRoot 前置 await 由其内部保证（DBG-5）；
//      新建页面由 Workspace 的 activePageId effect 触发惰性初始化，Dockview API 在 onReady 后注册
//   4. 终端恢复：轮询 getPageApi（100ms×50，照 openHooksConfigPanel）→ addPanel(terminal，
//      title = profile.tabTitle) → 轮询 TerminalRegistry 注册 → pty.write 注入
//      profile.history.buildRestoreInput(session, { fork })（MC-315 委托——注入内容
//      含 fork 追加与 \r 结尾，由各 CLI 的 history 能力实现负责）
//
// 失败路径：任一步骤异常 → sendToastNotification + console.error，不静默吞错、不中断其他流程（场景 10）。
// 孤儿行（cwdExists=false）/无 cwd 行的禁用判定由调用方（Stage 05 菜单/双击分派）负责，本函数不判定。

import { useProjects, createProjectId, createPageId } from "../../stores/projects";
import type { OperationPage } from "../../stores/projects";
import { makeEmptyLayout } from "../sidebar";
import { switchToPageShared, getPageApi } from "../../workspace/pageApis";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import { write as ptyWrite } from "../../ipc/pty";
import { sendToastNotification } from "../../ipc/notification";
import { normalizePath, basename } from "../../lib/path";
import { makeTerminalPanelId } from "../../lib/panelId";
import { cliProfileRegistry } from "../cliProfiles";
import type { AgentHistorySession } from "../../types/agentHistory";

/** 轮询上限：100ms × 50 = 5s（照 openHooksConfigPanel / switchToPageAndFocus 模式） */
const POLL_COUNT = 50;
const POLL_INTERVAL_MS = 100;

/** 轮询等待条件满足（probe 返回非 undefined），超时抛错 */
async function waitFor<T>(probe: () => T | undefined, label: string): Promise<T> {
  for (let i = 0; i < POLL_COUNT; i++) {
    const value = probe();
    if (value !== undefined) return value;
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
  }
  throw new Error(
    `${label} 在 ${(POLL_COUNT * POLL_INTERVAL_MS) / 1000}s 内未就绪`,
  );
}

/** 模块级恢复进行中标记——防重入（并发双击同一会话行时，第二次调用直接返回） */
let restoring = false;

/**
 * 四步恢复编排：项目入列 → 页面保障 → 页面切换 → 终端恢复注入
 * profile.history.buildRestoreInput 输出（MC-315）。
 * @param session 历史会话（cwd 为 null 时调用方已前置拦截，此处仍防御性 throw）
 * @param opts.fork 分支恢复（注入内容由 profile.history 实现追加 fork 语义）
 */
export async function restoreHistorySession(
  session: AgentHistorySession,
  opts?: { fork?: boolean },
): Promise<void> {
  // 防重入：恢复编排进行中，并发调用直接返回（如快速双击同一历史行）
  if (restoring) return;
  restoring = true;
  try {
    await doRestore(session, opts?.fork ?? false);
  } catch (err) {
    // 失败路径：toast + 日志，不静默吞错、不中断其他流程（场景 10）
    sendToastNotification("恢复会话失败", {
      body: `会话 ${session.sessionId.slice(0, 8)} 恢复失败：${
        err instanceof Error ? err.message : String(err)
      }`,
    });
    console.error("[slTerminal] 恢复历史会话失败:", err);
  } finally {
    restoring = false;
  }
}

async function doRestore(session: AgentHistorySession, fork: boolean): Promise<void> {
  // 防御性拦截：cwd 为 null 无法编排（调用方已前置拦截，此处双保险）
  if (session.cwd == null) {
    throw new Error("会话缺少工作目录（cwd），无法恢复");
  }
  // 恢复策略（MC-315）：按 session.cliId 查 profile——无 history 能力（含 profile
  // 未注册）→ 防御性失败，走统一失败 toast 路径（能力未声明 = 该域不可用）
  const profile = cliProfileRegistry.get(session.cliId);
  const historyCap = profile?.capabilities?.history;
  if (!profile || !historyCap) {
    throw new Error(`CLI "${session.cliId}" 不支持历史会话恢复`);
  }
  const cwd = session.cwd;

  // 步骤 1：项目入列——rootPath 与 cwd 规范化（反斜杠→/）+ 忽略大小写后精确相等则复用（决策 24）
  const { projects, addProject, addPage } = useProjects.getState();
  const normalizedCwd = normalizePath(cwd).toLowerCase();
  let project = Object.values(projects).find(
    (p) => normalizePath(p.rootPath).toLowerCase() === normalizedCwd,
  );
  if (!project) {
    // 无匹配项目：按会话 cwd 入列（字段形状照 SidebarTree.handleAddProject）
    project = {
      projectId: createProjectId(),
      name: basename(cwd),
      rootPath: cwd,
      pages: [],
      activePageId: null,
      version: 1,
    };
    addProject(project);
  }

  // 步骤 2：页面保障——项目无页面则新建空布局页面（照 SidebarTree.handleNewPage 模式）
  let targetPageId: string;
  if (project.pages.length > 0) {
    targetPageId = project.pages[0].pageId;
  } else {
    const page: OperationPage = {
      pageId: createPageId(),
      name: `页面-${Date.now() % 10000}`,
      layout: makeEmptyLayout(),
      cwd,
      createdAt: Date.now(),
      lastAccessedAt: Date.now(),
    };
    addPage(project.projectId, page);
    targetPageId = page.pageId;
  }

  // 步骤 3：页面切换——setProjectRoot 前置 await 由 switchToPageShared 内部保证（DBG-5）
  await switchToPageShared(targetPageId);

  // 步骤 4：终端恢复——轮询 API 就绪 → addPanel → 轮询 TerminalRegistry → 注入恢复命令
  const api = await waitFor(
    () => getPageApi(targetPageId),
    `页面 ${targetPageId} 的 DockviewApi`,
  );
  // B14: panelId 经生成单点 makeTerminalPanelId（terminal-{pageId}-{seq}，模块级
  // 每页计数与 PageDockviewHost 共享）——旧格式含 Date.now 数字段，破坏贪婪正则/
  // 切分解析（visible 恒 false 黑屏 + 幽灵页面导航根因）
  const panelId = makeTerminalPanelId(targetPageId);
  api.addPanel({
    id: panelId,
    component: "terminal",
    title: profile.tabTitle,
    params: { panelId, cwd },
    renderer: "always",
  });

  const entry = await waitFor(
    () => TerminalRegistry.get(panelId),
    `终端面板 ${panelId} 的 PTY 会话`,
  );

  // 注入恢复内容（决策 25，MC-315 委托）：profile.history.buildRestoreInput——
  // OSC 133 / hooks 全链路随终端自然生效，零后端改动
  const command = historyCap.buildRestoreInput(session, { fork });
  await ptyWrite(entry.sessionId, panelId, new TextEncoder().encode(command));
}
