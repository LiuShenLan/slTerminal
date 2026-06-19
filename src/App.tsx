import { Workspace } from "./workspace";
import "dockview-react/dist/styles/dockview.css";

/**
 * App 根组件 — 暗色全窗口 Dockview 布局，注册 terminal/editor 面板
 */
function App() {
  return (
    <div style={{ width: "100vw", height: "100vh", background: "#1e1e2e" }}>
      <Workspace />
    </div>
  );
}

export default App;
