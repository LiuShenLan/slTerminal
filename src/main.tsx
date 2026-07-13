import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";
import { E2E_ENABLED } from "./lib";

// S3: 等待 Tauri IPC 就绪后再挂载 React（WebView2 注入 window.__TAURI_INTERNALS__ 是异步的）
async function bootstrap() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if (!(window as any).__TAURI_INTERNALS__) {
    await new Promise<void>((resolve) => {
      const id = setInterval(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        if ((window as any).__TAURI_INTERNALS__) {
          clearInterval(id);
          resolve();
        }
      }, 50);
    });
  }

  // E2E 辅助仅在 E2E_ENABLED 时动态导入（dev serve 或 VITE_E2E=1 构建）——生产构建 tree-shake 排除
  if (E2E_ENABLED) {
    import("../e2e-tests/helpers").then((m) => m.installAllE2eHelpers());
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

bootstrap();
