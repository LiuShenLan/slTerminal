# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/panels/docViewer` 是文档型预览面板（htmlviewer / markdownviewer）的共享基础设施层（非面板类型，不进 PANEL_TYPES——新增面板仍走 panels/CLAUDE.md 四步）。预览窗口编排、消息桥、注入脚本组装、缩放/滚动运行时与悬浮 UI 是安全红线最密集处，收敛单点是复用而不复制的唯一方式；面板各自持有文件读取、loading/error 态与业务链接策略。

依赖方向：`html/`、`markdown/` → `docViewer/`；docViewer 不得反向引用面板实现（markdown 的 linkPolicy 分类在面板侧，PreviewFrame 只透传 href）。事件名单点登记于 `src/ipc/preview.ts`（通信层，跨窗口通道属 IPC 域）；文档层消息类型与上/下行类型白名单单点登记于 previewMessages.ts。

## 关键约束与决策

### 渲染架构：独立预览 webview（S10-② 迁移，ADR-0019）

预览内容不再经主窗口内 sandbox iframe srcDoc，改在**独立 Tauri WebviewWindow**（label = `preview-<panelId>`）中渲染：

- **宿主页 = 自定义协议域静态页**（scheme `slterm-preview`，Windows 实为 `http://slterm-preview.localhost/…`，src-tauri/src/preview.rs 内嵌）——tauri 2.11 无 per-webview CSP（spike 实证），资产协议页恒被注入全局 CSP → 主窗口收紧（CP-012）后内联注入全灭；自定义协议响应不带全局 CSP——域级 CSP 由宿主页 meta 承载（`default-src 'none'`；script/style `'unsafe-inline'`；img/font `data:`，SEC-02；srcdoc iframe 继承宿主 CSP）→ **「预览 CSP 域」即本域**，注入机制（injectScript + buildInjectedScript + nonce）原样迁入执行（内联脚本/样式与 data: img/font 放行，外部出网默认全断）。
- 宿主页桥（纯 JS，raw `__TAURI_INTERNALS__`——域内无打包模块）：建 sandbox iframe（allow-scripts、无 allow-same-origin，CVE-2024-35222 红线延续）→ 内容经 `preview_render`（后端存储 + 定向通知）→ 宿主 `preview_pull` 拉取置 srcdoc → iframe 文档消息（zoom/scroll/nav 上行、reset/zoom_set/scroll_set 下行）经 Tauri event 与主窗中继。
- **消息桥 = Tauri event/IPC**（跨独立窗口无 window.postMessage，spike 实证）——CP-044「通道退役」分支落地：上行/下行类型白名单 + label 归属守卫 + nonce（previewMessages.ts 单点，守卫测试锁死）。
- **窗口形态**：主窗口 owned 无边框窗口，几何 = 主窗 inner 原点 + 面板内容区矩形 × scale（PreviewFrame 200ms 轮询 + 主窗移动/resize/scale 三事件强制同步驱动 preview_sync——去重早退只比 CSS 视口矩形，主窗事件经 forceSync 旗标旁路 + 50ms 节流，否则移动主窗预览不跟随）；面板隐藏（页签/页面切换 display:none）→ 窗口 hide 不销毁——缩放/滚动态保活（CP-037 复核语义：CM 保活在面板内照旧，预览窗口保活 = hide/show，两机制并行，workspace CSS 显隐对 webview 不适用）。
- **键盘语义**：预览窗口 focusable(false)——键盘焦点恒在主窗口 ShortcutRegistry 域（预览聚焦不吞全局快捷键，CP-013 步骤 4 口径）；代价：预览文档内表单键入/系统复制快捷键不可达（已知行为登记，WebView2 鼠标交互不受影响）。
- **宿主内联 `<script>` 真实执行（CP-031）**：injectScript 不再做字符串级转义（存量缺陷转义函数已删）——宿主脚本段原样进入渲染文档；预览与主窗口 CSP 隔离，收紧主窗口 CSP 不影响预览。

### PreviewFrame（主窗侧预览窗口编排单点）

- 职责：窗口创建/几何同步/销毁（preview_sync/close）、内容装配推送（injectScript + buildInjectedScript + nonce → preview_render）、上行消息处理（label 归属 + 类型白名单 + nonce + 数值守卫——旧 iframe 通道四层校验的 label 化迁移）、下行下发（reset/zoom_set/scroll_set）、keepZoom/keepScrollRatio 镜像与恢复（iframe-loaded 宿主状态事件触发）。
- **面板根工具条带约定（S10-②）**：预览窗口锚定面板内容区矩形（PreviewFrame 自带锚点 div），FloatingArea 恒位于内容区上方 40px 工具条带内（面板根列排布局）——不落入窗口覆盖范围，交互可用；`direction="row"`（切换条/HUD 同行）。
- PreviewFrame 不渲染 HUD：zoom 变化经 onZoomChange 上报（显示在面板根悬浮区）；内容重建归 1 经 onZoomReset 通知（静默隐藏，非「显示 100%」——report 只承载真实变化，回落 1.0 属变化需显示，语义分离红线）；重置下行经 ref 命令接口（PreviewFrameHandle.resetZoom）。
- 内部镜像 zoomRef/ratioRef = keepZoom/keepScrollRatio 重建恢复取值源（上行变化/复位同步，等值不重复处理）。
- keepZoom/keepScrollRatio 仅会话级（预览窗口内容存亡），不跨会话持久化（html 现状语义继承）。

### 右上悬浮区（FloatingArea / useZoomHud）

面板根右上悬浮 UI 单点（2026-09-06 收敛；S10-② 起承载于工具条带内，direction="row" 同行侧排——切换条 + HUD 芯片，适配 40px 条带高约束）：

- FloatingArea：absolute 右上（top 8/right 8/zIndex 20），flex 排布（direction prop：column 纵向列排 = 切换条上/HUD 下；row 横向侧排 = 条带内同行）；容器 pointerEvents none 子项 auto；data-e2e 前缀由面板传（html-/markdown-zoom-hud 命名兼容历史）。
- useZoomHud：HUD 状态机（report 变化显示含回落 100% / 等值回声不复活 / hide 静默归 1 / 3s 续期消失），flushSync 于 report 内（原生 message 事件内紧急 UI）。
- 装配链（两面板同构）：面板持 useZoomHud + PreviewFrame ref；zoom 上行 → onZoomChange={report}；重建/重置 → onZoomReset={hide} / reset 按钮 onReset = frameRef.resetZoom() + hide()。
- ModeSwitcher 仍受控纯展示（不含定位）；定位恒由面板工具条带承载——任何形态不加双轨自摆。

### 注入脚本组装（buildInjectedScript）——拼接纪律

- 输出源码不得含 `</script>` 字面量（宿主内容不转义后，注入段自身更要避免——宿主自带 `</script>` 属其自身脚本正常闭合，与注入段互不干扰，CP-031）（FE-06 起有计数断言测试锁，doc-viewer-injection.test.ts）。
- 每段必须以完整语句 + 分号收尾（2026-09-06 实证：click 段原无分号致追加 zoom 段后同串拼接 SyntaxError——L2 全绿仅 L4 暴露）。
- 字符串插值一律 JSON.stringify；数值常量以十进制字面量直插（iframe 内独立运行）。
- postMessage 仅存在于 iframe ↔ 宿主页窗口树内（不跨窗口），targetOrigin "*"（SEC-03 实证语义；宿主页侧 source===iframe.contentWindow + 白名单 + nonce 校验兜底）。
- **无基础段（S10-② 起）**：keydown 转发段与信任标记随键盘不跨窗口整体退役（CP-013），注入产物 = extra 段 → zoom 段（恒末位，fragmentNav 的 click 段 "},true);" 收尾衔接 "var sltermZoom="——html-panel 控制流断言正则锁死，勿调换）。
- 可选段：fragmentNav（html：# 链接拦截 + :target 模拟）/ linkRouter（md：非 # 链接上行 slterm_nav）/ scrollReport（md：滚动节流上行 + 下行恢复）/ fontProbe（E2E 专用字体加载探针，TE-08——仅 VITE_E2E 拼装传入，生产零注入面；iframe 内 fonts.ready 后 check('12px "KaTeX_Main"') 上行宿主桥，结果写主窗 `__slterm_e2e_fontProbe` 供 L4 断言。宿主页 FontFaceSet 不覆盖 iframe 文档且 opaque origin 不可读——2026-09-09 实证 hostSize=0、check 对任意族名恒 true，故锚点必须自 iframe 内取）。
- zoomRuntime/scrollRuntime 为参数化函数源码（function(doc,win)——仅与宿主页窗口树内对话）——L2 经 new Function 桩执行行为级覆盖（jsdom 不执行 srcdoc 脚本）。

### 消息协议常量单点

previewMessages.ts 承载全部「iframe ↔ 宿主页」消息类型与构造/守卫（zoom/scroll/nav/reset/set + E2E 字体探针 fontProbe/slterm_font_probe，TE-08）+ 上/下行类型白名单（CP-013/044 终态 + 探针为第四上行类型——仅 VITE_E2E 构建注入可达，PreviewFrame 侧 E2E_ENABLED 门控收束）；新增消息类型在此登记并同步注入段与 PreviewFrame 分派。**宿主页桥（src-tauri/src/preview.rs HOST_PAGE）按字段白名单式转发**（zoom/ratio/href/loaded——新增上行字段须同步桥脚本，否则载荷静默丢弃，Rust 侧 `host_page_bridge_forwards_*` 测试锁）。窗口层事件名单点登记于 `src/ipc/preview.ts`（变更须三处同步：TS 常量 / src-tauri 宿主页桥脚本 / 测试守卫）。

## 外部坑/红线

- **勿把面板渲染期 ref 当可用容器**：useCodeMirror 类消费方在 render 阶段读 ref 得 null——容器首挂需 commit 后 bump 重渲染（HtmlPanel/MarkdownPanel 的 bumpFrame 桥接，DiffPanel 先例）。
- **jsdom 无 crypto.randomUUID**（node 环境缺失实证）：占位/随机一律 crypto.getRandomValues hex（createNonce/randomHex 同款）。
- **勿重建「悬浮区坐标机制」多轨形态**：切换条/HUD 坐标协调只有悬浮区单点一种合法形态，S10-② 起恒承载于面板工具条带（row）——任何形态不加双轨自摆。
- **report 与 hide 语义分离**：report(1.0) 是回落变化需显示（Chrome 气泡语义）；复位/重建归 1 走 hide（静默），混用会复活或误显气泡。
- **预览窗口 focusable=false 的键盘边界**：预览文档内键盘键入/系统复制快捷键不可达（已知行为）；任何改为可聚焦的尝试必须先解决「预览聚焦吞全局快捷键」问题（CP-013 步骤 4 口径）。
- **预览窗口为真实 OS 窗口**：几何由主窗侧驱动（200ms 轮询 + 主窗移动/resize/scale 事件强制同步），拖拽分栏等连续尺寸变化存在亚秒级跟随延迟——不允许替代方案（如常显浮窗）破坏「内容锚定面板」的产品语义。

## 测试模式

- 注入段字符串断言 + parse-only（doc-viewer-injection.test.ts）；zoom 桩执行（doc-viewer-zoom-runtime.test.ts）/ scroll 桩执行（doc-viewer-injection.test.ts 内）。
- PreviewFrame 编排/消息校验在 html-panel.test.tsx / markdown-panel.test.tsx 集成面（ipc/preview mock 边界：捕获推送产物/事件订阅回调驱动）。
- 真实 WebView2 往返（窗口创建/宿主页桥/iframe 执行/事件中继）由 html.e2e.ts / markdown.e2e.ts 验收（switchToWindow 驱动契约，e2e-tests/CLAUDE.md）。
