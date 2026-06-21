import React, { useEffect } from "react";
import { Workspace } from "./workspace";
import { loadAllProjects, markPersistenceReady } from "./stores/projects";
import { useLayout } from "./stores/layout";
import "dockview-react/dist/styles/dockview.css";

/** 错误边界 */
class ErrorBoundary extends React.Component<
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
  // H7: 启动时从磁盘恢复项目数据（loadAllProjects 完成后 onReady 中已有恢复逻辑）
  useEffect(() => {
    const init = async () => {
      try {
        await loadAllProjects();
      } catch {
        // 首次启动或文件损坏，保持默认空状态
      }
      markPersistenceReady();
    };
    init();
  }, []);

  // S2: 关闭前冲刷持久化（2s debounce 是主防线，beforeunload 是补充防线）
  useEffect(() => {
    const handleBeforeUnload = () => {
      // 同步状态下尽力保存 activePageId（供下次启动恢复）
      const activePageId = useLayout.getState().activePageId;
      if (activePageId) {
        try {
          localStorage.setItem("slterm-last-active-page", activePageId);
        } catch {
          // localStorage 不可用时静默失败
        }
      }
    };
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, []);

  // H7: 启动时恢复上次 activePageId
  useEffect(() => {
    try {
      const lastPage = localStorage.getItem("slterm-last-active-page");
      if (lastPage) {
        useLayout.getState().setActivePage(lastPage);
      }
    } catch {
      // localStorage 不可用时静默失败
    }
  }, []);

  return (
    <ErrorBoundary>
      <div style={{ width: "100vw", height: "100vh", background: "#1e1e2e" }}>
        <Workspace />
      </div>
    </ErrorBoundary>
  );
}

export default App;
