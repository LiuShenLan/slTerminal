import React, { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Workspace } from "./workspace";
import { loadAllProjects, markPersistenceReady, saveAllProjects, cancelPendingSave, useProjects } from "./stores/projects";
import { useLayout } from "./stores/layout";
import { saveLayout } from "./workspace/layoutSerde";
import "dockview-react/dist/styles/dockview.css";

/** 错误边界 */
export class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  { error: Error | null }
> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = { error: null };
  }
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__sltermError = { message: error.message, stack: error.stack, componentStack: info.componentStack };
    console.error("[slTerminal] 渲染错误:", error, info);
  }
  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 20, color: "#F44747", background: "#1E1E1E", height: "100vh", fontFamily: "monospace", fontSize: 13 }}>
          <h2>应用渲染错误</h2>
          <pre style={{ whiteSpace: "pre-wrap" }}>{this.state.error.message}</pre>
          <pre style={{ whiteSpace: "pre-wrap", color: "#999", fontSize: 11 }}>{this.state.error.stack}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}

function App() {
  /** S4-D1: 数据就绪后才渲染 Workspace，消除启动竞态 */
  const [ready, setReady] = useState(false);

  // S4-D1: 启动加载（单一时序：loadAllProjects → 恢复 activePageId → setReady → 渲染 Workspace）
  useEffect(() => {
    const init = async () => {
      try {
        await loadAllProjects();
      } catch {
        // 首次启动或文件损坏，保持默认空状态
      }
      markPersistenceReady();

      // 数据就绪后恢复上次 activePageId（确保 pageId 对应的项目数据已加载）
      try {
        const lastPage = localStorage.getItem("slterm-last-active-page");
        if (lastPage) {
          useLayout.getState().setActivePage(lastPage);
        }
      } catch {
        // localStorage 不可用时静默失败
      }

      setReady(true);
    };
    init();
  }, []);

  // S4-D4: 关闭前冲刷持久化（onCloseRequested 双出口防线 + destroy 防循环）
  useEffect(() => {
    const appWindow = getCurrentWindow();
    const unlisten = appWindow.onCloseRequested(async (event) => {
      event.preventDefault();
      try {
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
      // 4. 强制销毁（destroy 在 try/catch 外，保存失败不阻塞关闭）
      // 注：destroy() 不重新触发 onCloseRequested（Tauri 2 保证）
      await appWindow.destroy();
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  if (!ready) {
    return (
      <ErrorBoundary>
        <div
          style={{
            width: "100vw",
            height: "100vh",
            background: "#1E1E1E",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "#6C6C6C",
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
      <div style={{ width: "100vw", height: "100vh", background: "#1e1e2e" }}>
        <Workspace />
      </div>
    </ErrorBoundary>
  );
}

export default App;
