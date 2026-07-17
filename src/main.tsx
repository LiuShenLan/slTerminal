import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";
import { E2E_ENABLED } from "./lib";
import { ROOT_CSS_VARS } from "./theme";

// 等待 Tauri IPC 就绪后再挂载 React（WebView2 注入 window.__TAURI_INTERNALS__ 是异步的）
async function bootstrap() {
  if (!window.__TAURI_INTERNALS__) {
    try {
      await new Promise<void>((resolve, reject) => {
        const MAX_ATTEMPTS = 200; // 200 × 50ms = 10s
        let attempts = 0;
        const id = setInterval(() => {
          attempts++;
          if (window.__TAURI_INTERNALS__) {
            clearInterval(id);
            resolve();
          } else if (attempts >= MAX_ATTEMPTS) {
            clearInterval(id);
            reject(new Error("Tauri IPC 初始化超时（等待 10s 后仍未就绪）"));
          }
        }, 50);
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("[slTerminal]", msg);
      document.body.innerHTML =
        `<div style="display:flex;align-items:center;justify-content:center;height:100vh;` +
        `background:#1e1e1e;color:#f44747;font-family:monospace;font-size:14px;padding:20px;">` +
        msg + "</div>";
      return;
    }
  }

  // 将 colors.ts ROOT_CSS_VARS 注入 document.documentElement，替代 App.css :root 硬编码 hex
  for (const [prop, value] of Object.entries(ROOT_CSS_VARS)) {
    document.documentElement.style.setProperty(prop, value as string);
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
