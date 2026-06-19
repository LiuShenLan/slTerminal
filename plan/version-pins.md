# slTerminal 版本钉表

核验日期：2026-06-18 | 交叉核验状态：**已完成（红旗已记录，见底部）**

## 一、Rust 后端

| 包名 | 钉住版本 | 说明/用途 | 来源 URL | 兼容性注意事项 |
|------|----------|-----------|----------|---------------|
| tauri | 2.11.3 | Rust 后端框架（窗口/WebView2/IPC） | https://crates.io/crates/tauri | **前后端 patch 不同步**（Rust 2.11.3 vs JS API 2.11.1），IPC 契约可能不对齐，必须锁定互锁版本对。已知坑：MSI 更新循环 (#14828)、NSIS Access Denied (#14622)、bundle.resources 跨平台路径不一致 (#15342) |
| tauri-build | 2.6.3 | Tauri 构建时代码生成（Cargo build script） | https://crates.io/crates/tauri-build | 独立版本体系，不与 tauri crate 同步发版 |
| portable-pty | 0.9.0 | 跨平台 PTY 接口（Windows ConPTY） | https://crates.io/crates/portable-pty | Windows spawn **必须串行化**（一把 SPAWN_LOCK），并发 spawn 会卡死 ConPTY 输出管道；cwd 反斜杠规范化；cwd/命令边界需借助 OSC 7/133 跟踪 |
| git2 | 0.21.0 | libgit2 Rust 绑定（git 操作） | https://crates.io/crates/git2 | 依赖系统 libgit2 或 vendored 构建 |
| notify | 9.0.0-rc.4 | 文件系统变更监控（跨平台，Phase 0 占位） | https://crates.io/crates/notify | Phase 0 评估可接受（rc.4），正式发布后单独验证 |

## 二、Tauri JS — 运行时与 CLI

| 包名 | 钉住版本 | 说明/用途 | 来源 URL | 兼容性注意事项 |
|------|----------|-----------|----------|---------------|
| @tauri-apps/api | 2.11.1 | Tauri 前端 IPC 调用（invoke/event/window/path 等） | https://www.npmjs.com/package/@tauri-apps/api | mockIPC 事件 mock 需 v2.7.0+ 且 emitTo 不被支持 |
| @tauri-apps/cli | 2.11.2 | Tauri 开发/构建 CLI（tauri dev/build） | https://www.npmjs.com/package/@tauri-apps/cli | v2.11.0 存在 bundle.resources 跨平台路径不一致 bug (#15342)，已锁 >= 2.11.2 规避 |

## 三、前端框架

| 包名 | 钉住版本 | 说明/用途 | 来源 URL | 兼容性注意事项 |
|------|----------|-----------|----------|---------------|
| react | 19.2.7 | UI 框架 | https://www.npmjs.com/package/react | dockview peer deps 支持 react 16.8–19 |
| react-dom | 19.2.7 | React DOM 渲染器 | https://www.npmjs.com/package/react-dom | 与 react 版本锁定一致 |
| typescript | 6.0.3 | 类型系统 | https://www.npmjs.com/package/typescript | — |
| vite | 8.0.16 | 前端构建工具 | https://www.npmjs.com/package/vite | Tauri frontendDist 需正确指向 Vite 输出目录；生产环境必须配置 hash 路由避免白屏 |

## 四、终端渲染（xterm.js monorepo — v6 稳定线）

| 包名 | 钉住版本 | 说明/用途 | 来源 URL | 兼容性注意事项 |
|------|----------|-----------|----------|---------------|
| @xterm/xterm | 6.0.0 | 终端核心 | https://www.npmjs.com/package/@xterm/xterm | 锁 v6 稳定版，不追 beta 线。各 addon 必须配同发布线版本 |
| @xterm/addon-webgl | 0.19.0 | WebGL2 GPU 加速渲染器 | https://www.npmjs.com/package/@xterm/addon-webgl | WebView2 无 GPU 场景触发软件 WebGL2 模拟导致字符丢失 (#4574)；修复：{failIfMajorPerformanceCaveat: true} |
| @xterm/addon-fit | 0.11.0 | 终端尺寸自适应容器 | https://www.npmjs.com/package/@xterm/addon-fit | WebView2 缩放/布局变更时 resize 可能滞后，需配合 ResizeObserver |
| @xterm/addon-serialize | 0.14.0 | 终端状态序列化（L3 测试用） | https://www.npmjs.com/package/@xterm/addon-serialize | 配合 @xterm/headless 做 L3 渲染测试 |
| @xterm/headless | 6.0.0 | 无头终端（测试用） | https://www.npmjs.com/package/@xterm/headless | L3 测试核心：headless Terminal + serialize 断言 |
| @xterm/addon-canvas | 0.7.0 | Canvas 2D 后备渲染器 | https://www.npmjs.com/package/@xterm/addon-canvas | WebGL2 不可用时回退；上游 #4779 讨论移除 Canvas 渲染器——长期风险 |

## 五、布局与编辑器

| 包名 | 钉住版本 | 说明/用途 | 来源 URL | 兼容性注意事项 |
|------|----------|-----------|----------|---------------|
| dockview-react | 6.6.1 | Dockview React 封装 | https://www.npmjs.com/package/dockview-react | fromJSON() 无效组件名会损坏 API 状态 (#341)：必须 try/catch 包裹。自定义 tabComponent 时 setTitle() 不生效 (#1003) |
| zustand | 5.0.14 | 轻量状态管理 | https://www.npmjs.com/package/zustand | 会话真值只在 stores/sessions.ts，面板只订阅不自存 |

## 六、测试基建

| 包名 | 钉住版本 | 说明/用途 | 来源 URL | 兼容性注意事项 |
|------|----------|-----------|----------|---------------|
| vitest | 4.1.9 | 前端单元/组件测试框架 | https://www.npmjs.com/package/vitest | 需 jsdom 环境 + polyfill window.crypto.getRandomValues；afterEach 中同时调 clearMocks() + vitest cleanup |
| @wdio/cli | 9.28.0 | WDIO CLI 入口（L4 E2E） | https://www.npmjs.com/package/@wdio/cli | — |
| @wdio/tauri-service | 1.1.0 | Tauri E2E launcher（Phase 0 用 embedded driverProvider） | https://www.npmjs.com/package/@wdio/tauri-service | 与 @wdio/tauri-plugin 同版本 |
| @wdio/tauri-plugin | 1.1.0 | Tauri WDIO 插件（types/服务端交互） | https://www.npmjs.com/package/@wdio/tauri-plugin | 与 @wdio/tauri-service 同版本 |
| tauri-plugin-wdio-webdriver | 1.1.0 | Rust 端 embedded WebDriver（替代 tauri-driver，零 msedgedriver 依赖） | https://crates.io/crates/tauri-plugin-wdio-webdriver | Phase 0 起用，tauri-driver 已弃用 |
| webdriverio | 9.28.0 | E2E 浏览器自动化框架 | https://www.npmjs.com/package/webdriverio | 只参考 v9 API（ESM-only） |

## 七、脚手架

| 包名 | 钉住版本 | 说明/用途 | 来源 URL | 兼容性注意事项 |
|------|----------|-----------|----------|---------------|
| create-tauri-app | 4.6.2 | Tauri 2 项目脚手架 | https://www.npmjs.com/package/create-tauri-app | 生成后立即检查 tauri.conf.json 版本与钉表一致性 |

---

## W0 核验元信息

| 项目 | 内容 |
|------|------|
| 核验日期 | 2026-06-18 |
| 交叉核验状态 | 已完成 |

### 红旗清单（需在后续 phase 关注）

1. **Tauri 前后端 patch 不同步**（tauri crate 2.11.3 vs @tauri-apps/api 2.11.1）：**已收口（精确钉 2.11.3/2.11.1 + lockfile 锁版本 + tauri build 内置检测，`tauri info` 验证 pair 兼容）**，Phase 1+ 无需额外动作
2. **WebView2 最低版本数值模糊**：运行时最低 101.0.1210.39，示例值 110.0.1531.0。slTerminal 应选 110.0.1531.0 作为基线
3. **@xterm/addon-canvas 兼容性无官方互锁声明**：需在集成测试中验证与 xterm 6.0.0 的协同工作
4. **Tauri CLI v2.11.0 bundle.resources 跨平台路径 bug** (#15342)：已锁 CLI >= 2.11.2 规避
5. **xterm.js #4779 Canvas 渲染器可能移除**：WebView2 无 GPU 场景的唯一高性能后备消失风险
6. **Dockview fromJSON 损坏 API 状态** (#341)：layoutSerde.ts 必须 try/catch 包裹 + 校验组件名白名单
7. **msedgedriver 版本检测依赖社区工具**：chippers/msedgedriver-tool 为社区维护，非 Tauri 官方
8. **notify 9.0.0-rc.4 非 stable**：Phase 0 已评估可接受（rc.4 仅占位，无文件监听需求），9.x 正式发布后单独验证
9. **WebView2 软件渲染缺陷** (#4574)：无 GPU 加速时 WebGL2 回退到软件模拟导致字符丢失
10. **生产环境白屏风险**：Vite 构建产物绝对路径、hash 路由未配置、frontendDist 指向错误均会导致 WebView2 白屏
11. **DPI 缩放浮点精度模糊**：WebView2 在 125%/150%/175% DPI 下 canvas 后备存储可能多 1px 产生模糊
