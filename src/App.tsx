import { useEffect, useState } from "react";
import { registerCloseHandler } from "./ipc/window";
import { Workspace } from "./workspace";
import {
  loadAllProjects, markPersistenceReady, saveAllProjects, cancelPendingSave,
  useProjects, createProjectId, createPageId,
} from "./stores/projects";
import type { OperationPage, Project } from "./stores/projects";
import { useLayout } from "./stores/layout";
import { saveLayout } from "./workspace/layoutSerde";
import { makeDefaultLayout } from "./features/sidebar/SidebarTree";
import { writeText } from "./ipc/clipboard";
import { pty } from "./ipc";
import { TerminalRegistry } from "./panels/terminal/TerminalRegistry";
import { ErrorBoundary } from "./lib";
import { PANEL_BG, INPUT_BORDER, APP_BG } from "./theme";
import "dockview-react/dist/styles/dockview.css";

// E2E pending 标记：__slterm_e2e_createProject 调用时设为 true，
// 阻止 init 序列的 localStorage 恢复覆盖 E2E 设置的 activePageId
let e2eProjectPending = false;

// E2E 测试辅助：暴露程序化创建项目的 API（绕过原生文件夹对话框）
if (typeof window !== "undefined") {
  // E2E 测试辅助：暴露 clipboard helper（静态 import，同步挂载，无竞态）
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__slterm_e2e_writeClipboard = writeText;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__slterm_e2e_createProject = (dirPath: string) => {
    e2eProjectPending = true; // 阻止 localStorage 恢复覆盖
    const name = dirPath.split(/[/\\]/).pop() || dirPath;
    const projectId = createProjectId();
    const pageId = createPageId();
    const defaultPanelId = `terminal-${pageId}-0`;

    const page: OperationPage = {
      pageId,
      name,
      layout: makeDefaultLayout(defaultPanelId),
      cwd: dirPath,
      createdAt: Date.now(),
      lastAccessedAt: Date.now(),
    };

    const project: Project = {
      projectId,
      name,
      rootPath: dirPath,
      pages: [page],
      activePageId: pageId,
      version: 1,
    };

    useProjects.getState().addProject(project);
    useLayout.getState().setActivePage(pageId);
    return pageId;
  };
}

function App() {
  /** S4-D1: 数据就绪后才渲染 Workspace，消除启动竞态 */
  const [ready, setReady] = useState(false);

  // S4-D1: 启动加载（单一时序：loadAllProjects → 恢复 activePageId → setReady → 渲染 Workspace）
  useEffect(() => {
    const init = async () => {
      try {
        // P2-07: loadAllProjects 内部 JSON.parse 当前数据量小无影响，
        // 若未来项目数据文件膨胀到 MB 级，可改为流式解析或 IndexedDB 存储。
        await loadAllProjects();
      } catch {
        // 首次启动或文件损坏，保持默认空状态
      }
      markPersistenceReady();

      // 数据就绪后恢复上次 activePageId（确保 pageId 对应的项目数据已加载）
      // E2E 模式跳过：__slterm_e2e_createProject 已设 e2eProjectPending=true
      try {
        if (!e2eProjectPending) {
          const lastPage = localStorage.getItem("slterm-last-active-page");
          if (lastPage) {
            useLayout.getState().setActivePage(lastPage);
          }
        }
      } catch {
        // localStorage 不可用时静默失败
      }

      setReady(true);
    };
    init();
  }, []);

  // S4-D4: 关闭前冲刷持久化（registerCloseHandler 封装 preventDefault + destroy）
  useEffect(() => {
    const unlisten = registerCloseHandler(async () => {
      try {
        // 0. 杀掉所有活跃 PTY session（P1-19：窗口关闭前清理子进程）
        //    通过 TerminalRegistry 遍历所有已注册终端，逐个 kill
        const allSessions = TerminalRegistry.getAll();
        if (allSessions.size > 0) {
          const killPromises: Promise<void>[] = [];
          allSessions.forEach((entry) => {
            if (entry.sessionId) {
              killPromises.push(
                pty.kill(entry.sessionId).catch((err) => {
                  console.error(`[slTerminal] 关闭时 kill PTY 失败 (${entry.sessionId}):`, err);
                }),
              );
            }
          });
          // 并发 kill 所有 session，最长等待 3 秒
          await Promise.race([
            Promise.all(killPromises),
            new Promise<void>((resolve) => setTimeout(resolve, 3000)),
          ]);
        }

        // 1. flush dirty layout
        const { activePageId } = useLayout.getState();
        if (activePageId && window.__dockviewApi) {
          const layout = saveLayout(window.__dockviewApi);
          const { projects } = useProjects.getState();
          for (const [, proj] of Object.entries(projects)) {
            if (proj.pages.some((p) => p.pageId === activePageId)) {
              useProjects.getState().updatePageLayout(
                proj.projectId,
                activePageId,
                layout as Record<string, unknown>,
              );
              break;
            }
          }
        }
        // 2. 同步保存到磁盘（清除 debounce 定时器 + 3s 超时防挂起）
        cancelPendingSave();
        await Promise.race([
          saveAllProjects(),
          new Promise<void>((resolve) => setTimeout(resolve, 3000)),
        ]);
        // 3. 保存 activePageId
        if (activePageId) {
          try {
            localStorage.setItem("slterm-last-active-page", activePageId);
          } catch { /* localStorage 不可用时静默失败 */ }
        }
      } catch (err) {
        console.error("[slTerminal] 关闭保存失败:", err);
      }
    });
    return unlisten;
  }, []);

  if (!ready) {
    return (
      <ErrorBoundary>
        <div
          style={{
            width: "100vw",
            height: "100vh",
            background: PANEL_BG,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: INPUT_BORDER,
            fontSize: 14,
            fontFamily: "monospace",
          }}
        >
          slTerminal 启动中…
        </div>
      </ErrorBoundary>
    );
  }

  return (
    <ErrorBoundary>
      <div style={{ width: "100vw", height: "100vh", background: APP_BG }}>
        <Workspace />
      </div>
    </ErrorBoundary>
  );
}

export default App;
