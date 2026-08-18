import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/jetbrains-mono/400.css"; // FT-01: JetBrains Mono 400 字重随产物打包（断网可用）
import "@fontsource/jetbrains-mono/500.css"; // FT-01: JetBrains Mono 500 字重随产物打包（断网可用）

// 等待 Tauri IPC 就绪后再挂载 React（WebView2 注入 window.__TAURI_INTERNALS__ 是异步的）
async function bootstrap() {
  // ① IPC 就绪等待 + fail-safe
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
        `background:#0a0a0b;color:#ece9e4;font-family:'JetBrains Mono','Cascadia Mono',Consolas,'Microsoft YaHei UI',monospace;font-size:14px;padding:20px;">` +
        `<span style="color:#d9706b;">` + msg + `</span></div>`;
      return;
    }
  }

  // ② 配色方案解析——必须在 App 模块图求值前完成（colors.ts facade 求值时取 active 方案）
  const { loadSettings } = await import("./ipc/settings");
  // FE-03：启动链失败不再静默——降级兜底不变（null → linear），仅 console.warn 告警
  const settings = await loadSettings().catch((err) => {
    console.warn("[main] 加载设置失败，回退默认配色:", err);
    return null;
  });
  const { schemeRegistry } = await import("./theme/schemeRegistry");
  await import("./theme/schemes"); // side-effect：注册内置方案（linear）
  // 未知 id 回退 linear 由注册表内部保证；非字符串（脏数据）同样回退
  const schemeId = typeof settings?.colorScheme === "string" ? settings.colorScheme : "linear";
  schemeRegistry.setActive(schemeId);

  // ③ 将 ROOT_CSS_VARS 注入 document.documentElement，替代 App.css :root 硬编码 hex
  const { ROOT_CSS_VARS } = await import("./theme");
  for (const [prop, value] of Object.entries(ROOT_CSS_VARS)) {
    document.documentElement.style.setProperty(prop, value as string);
  }

  // ④ E2E 辅助仅在 dev serve 或 VITE_E2E=1 构建时动态导入——生产构建条件编译为 false，整块 DCE
  //    时序不变量：helpers 注入在 setActive 之后（E2E 测试依赖已激活的配色方案）
  //    门控须内联 import.meta.env 表达式而非引用 ./lib/e2eEnabled 的 E2E_ENABLED 常量：
  //    rolldown 不做跨模块常量折叠，动态 import 站点靠字面量折叠方可 DCE——实测
  //    `if (E2E_ENABLED)` 会使 helpers chunk 残留生产 dist（CI 生产剥离守卫 fail）；
  //    静态 import 站点（Workspace/useTerminalInstance/useXterm）无此问题（模块级 tree-shake）。
  //    表达式与 e2eEnabled.ts 的 E2E_ENABLED 定义逐字一致。
  if (import.meta.env.DEV || import.meta.env.VITE_E2E === "1") {
    import("../e2e-tests/helpers").then((m) => m.installAllE2eHelpers());
  }

  // ⑤ App 动态导入——App.tsx 模块图内静态引用 theme token，须在 ② setActive 之后求值
  const { default: App } = await import("./App");

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

bootstrap();
