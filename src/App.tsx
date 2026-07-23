import { useEffect, useState } from "react";
import { registerCloseHandler } from "./ipc/window";
import { Workspace } from "./workspace";
import {
  loadAllProjects, markPersistenceReady, saveAllProjects, cancelPendingSave,
  useProjects,
} from "./stores/projects";
import { useLayout } from "./stores/layout";
import { useFontSize, cancelPendingSave as cancelFontSizeSave } from "./stores/fontSize";
import { useKeybindings, cancelPendingSave as cancelKeybindingsSave } from "./stores/keybindings";
import { useSideBar, cancelPendingSave as cancelSideBarSave } from "./stores/sideBar";
import { saveLayout } from "./workspace/layoutSerde";
import { pty } from "./ipc";
import { setProjectRoot } from "./ipc/fs";
import { TerminalRegistry } from "./panels/terminal/TerminalRegistry";
import { ErrorBoundary } from "./lib";
import { getShortcutRegistry, createGlobalShortcuts, wireKeybindings } from "./features/shortcuts";
import { createTerminalShortcuts } from "./panels/terminal/keyboard";
import { createEditorShortcuts } from "./panels/editor/keyboard";
import { createExplorerShortcuts } from "./features/explorer/keyboard";
import { PANEL_BG, INPUT_BORDER, APP_BG } from "./theme";
import "dockview-react/dist/styles/dockview.css";

/** 关闭等待超时（ms）——kill PTY / flush 持久化的最大等待时间 */
const SHUTDOWN_TIMEOUT_MS = 3000;
/** localStorage key：上次活跃页面 ID */
const LS_LAST_ACTIVE_PAGE_KEY = "slterm-last-active-page";

function App() {
  /** S4-D1: 数据就绪后才渲染 Workspace，消除启动竞态 */
  const [ready, setReady] = useState(false);

  // S4-D1: 启动加载（单一时序：loadAllProjects → 恢复 activePageId → setReady → 渲染 Workspace）
  useEffect(() => {
    const init = async () => {
      try {
        // 加载字体大小偏好（先于项目数据，确保面板渲染时已有正确值）
        await useFontSize.getState().loadFromDisk();
      } catch {
        // 首次启动或文件损坏，保持默认值
      }

      try {
        // 加载快捷键自定义绑定（覆盖层）——先于面板注册，确保注册表构建时已有覆盖
        await useKeybindings.getState().loadFromDisk();
      } catch {
        // 首次启动或文件损坏，保持默认（空覆盖）
      }

      try {
        // 加载侧栏视图状态——区划/开关/宽度/比例，先于项目数据确保 Workspace 渲染时已有正确值
        await useSideBar.getState().loadFromDisk();
      } catch {
        // 首次启动或文件损坏，保持默认值
      }

      try {
        // P2-07: loadAllProjects 内部 JSON.parse 当前数据量小无影响，
        // 若未来项目数据文件膨胀到 MB 级，可改为流式解析或 IndexedDB 存储。
        await loadAllProjects();
      } catch {
        // 首次启动或文件损坏，保持默认空状态
      }
      markPersistenceReady();

      // 数据就绪后恢复上次 activePageId（确保 pageId 对应的项目数据已加载）
      // E2E 模式跳过：__slterm_e2e_createProject 已设 window.__slterm_e2e_projectPending=true
      try {
        if (!window.__slterm_e2e_projectPending) {
          const lastPage = localStorage.getItem(LS_LAST_ACTIVE_PAGE_KEY);
          if (lastPage) {
            // DBG-6: setActivePage 前先同步项目根路径到后端（路径沙箱前置条件）
            const { projects: currentProjects } = useProjects.getState();
            for (const [, proj] of Object.entries(currentProjects)) {
              if (proj.pages.some((p) => p.pageId === lastPage)) {
                if (proj.rootPath) {
                  try {
                    await setProjectRoot(proj.rootPath);
                  } catch (err) {
                    console.error("[slTerminal] 启动恢复—设置项目根路径失败:", err);
                  }
                }
                break;
              }
            }
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
          allSessions.forEach((entry, panelId) => {
            if (entry.sessionId) {
              killPromises.push(
                pty.kill(entry.sessionId, panelId).catch((err) => {
                  console.error(`[slTerminal] 关闭时 kill PTY 失败 (${entry.sessionId}, ${panelId}):`, err);
                }),
              );
            }
          });
          // 并发 kill 所有 session，最长等待 3 秒
          await Promise.race([
            Promise.all(killPromises),
            new Promise<void>((resolve) => setTimeout(resolve, SHUTDOWN_TIMEOUT_MS)),
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
        // 2. 同步保存到磁盘（清除各 store debounce 定时器 + 3s 超时防挂起）
        cancelPendingSave();
        cancelFontSizeSave();
        cancelKeybindingsSave();
        cancelSideBarSave();
        await Promise.race([
          saveAllProjects(),
          new Promise<void>((resolve) => setTimeout(resolve, SHUTDOWN_TIMEOUT_MS)),
        ]);
        // 3. 保存 activePageId
        if (activePageId) {
          try {
            localStorage.setItem(LS_LAST_ACTIVE_PAGE_KEY, activePageId);
          } catch { /* localStorage 不可用时静默失败 */ }
        }
      } catch (err) {
        console.error("[slTerminal] 关闭保存失败:", err);
      }
    });
    return unlisten;
  }, []);

  // 注册全局 + 面板级快捷键（一次性；面板命令 handler 经 active 指针派发到聚焦实例）
  useEffect(() => {
    const registry = getShortcutRegistry();
    return registry.register([
      ...createGlobalShortcuts(() => window.__dockviewApi),
      ...createTerminalShortcuts(),
      ...createEditorShortcuts(),
      ...createExplorerShortcuts(),
    ]);
  }, []);

  // 将用户自定义绑定（覆盖层）持续同步到注册表
  useEffect(() => {
    return wireKeybindings(getShortcutRegistry(), useKeybindings);
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
