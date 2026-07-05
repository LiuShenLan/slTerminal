# Embedded WebDriver + WDIO（零外部依赖）

Status: Accepted

E2E 测试使用 Tauri 内嵌 WebDriver + WDIO，通过 `webview2-com` COM 接口直接驱动 `ICoreWebView2`，不依赖 msedgedriver 或任何外部二进制文件。

**为什么不是 Playwright？** Playwright 期望标准的 DevTools Protocol 端点，但 Tauri 的 WebView2 不暴露该协议。Playwright 也无法控制 Tauri 的原生 Rust 侧（文件对话框、菜单、系统托盘）。

**为什么不是 Selenium + msedgedriver？** 需要匹配 Edge 版本的 msedgedriver，引入外部版本耦合。Tauri 1.1 起提供 embedded driver 模式，完全消除外部依赖。

**备选方案**：仅前端单元测试（无法验证 IPC 完整链路和 PTY 通信）；Tauri `tauri-driver` + msedgedriver（需要额外安装和管理 Edge WebDriver）。

**后果**：测试通过 `browser.execute()` 在 WebView2 JS 上下文中运行，与应用通过注入的 window 全局对象通信。测试和应用共享同一进程，需 `#[cfg(debug_assertions)]` 门控测试代码，确保发布构建不含 E2E 注入。
