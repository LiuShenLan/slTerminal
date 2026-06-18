import { DockviewReact } from "dockview-react";
import "dockview-react/dist/styles/dockview.css";

/**
 * App 根组件 — 渲染填满整个窗口的暗色空 Dockview 布局
 */
function App() {
  const components = {}; // 暂无面板类型，Phase 1 起注册

  const onReady = () => {
    // 空布局，面板将在后续 phase 通过布局序列化加载
  };

  return (
    <div style={{ width: "100vw", height: "100vh", background: "#1e1e2e" }}>
      <DockviewReact
        className="dockview-theme-dark"
        components={components}
        onReady={onReady}
      />
    </div>
  );
}

export default App;
