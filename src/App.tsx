import React, { useEffect } from "react";
import { Workspace } from "./workspace";
import { loadAllProjects, markPersistenceReady } from "./stores/projects";
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
  // H6 修复：启动时从磁盘恢复项目数据
  useEffect(() => {
    loadAllProjects().then(() => {
      markPersistenceReady();
    }).catch(() => {
      markPersistenceReady(); // 即使加载失败也标记就绪，允许后续保存
    });
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
