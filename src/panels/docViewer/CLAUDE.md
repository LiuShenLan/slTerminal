# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/panels/docViewer` 是文档型预览面板（htmlviewer / markdownviewer）的共享基础设施层（非面板类型，不进 PANEL_TYPES——新增面板仍走 panels/CLAUDE.md 四步）。宿主 iframe（PreviewFrame）、消息桥、注入脚本组装、缩放/滚动运行时与悬浮 UI 是安全红线最密集处，收敛单点是复用而不复制的唯一方式；面板各自持有文件读取、loading/error 态与业务链接策略。

依赖方向：`html/`、`markdown/` → `docViewer/`；docViewer 不得反向引用面板实现（markdown 的 linkPolicy 分类在面板侧，PreviewFrame 只透传 href）。文档层消息类型与上/下行类型白名单单点登记于 previewMessages.ts（三层通道全貌见其头注，ADR-0021）。

## 关键约束与决策

### 渲染架构：主窗内跨源沙箱宿主 iframe（ADR-0021）

预览内容渲染于**主窗 DOM 内跨源沙箱 iframe**（旧独立 WebviewWindow 载体已推翻——拖动跟随延迟结构性不可归零；安全域决策全保留）：

- **拓扑**：面板内容区 → `<iframe sandbox="allow-scripts" src="http://slterm-preview.localhost/preview-host.html" data-e2e="preview-frame-<panelId>">`（宿主 iframe，opaque origin）→ 宿主页内嵌 `<iframe sandbox="allow-scripts">` srcdoc = 内容文档。与编辑器页签同属主窗 WebView2 DOM——OS 移窗/resize 天然像素级跟随，无窗口编排/几何同步/IPC 跟随链。
- **宿主页 = 自定义协议域静态页**（scheme `slterm-preview`，Windows 实为 `http://slterm-preview.localhost/…`，src-tauri/src/preview.rs 内嵌）——tauri 2.11 无 per-webview CSP（spike 实证），资产协议页恒被注入全局 CSP → 主窗口收紧（CP-012）后内联注入全灭；自定义协议响应不带全局 CSP——域级 CSP 由宿主页 meta 承载（`default-src 'none'`；script/style `'unsafe-inline'`；img/font `data:`，SEC-02；srcdoc iframe 继承宿主 CSP）→ **「预览 CSP 域」即本域**，注入机制（injectScript + buildInjectedScript + nonce）原样迁入执行。主窗 CSP 经 `frame-src http://slterm-preview.localhost` 收窄放行该域（csp-config.test.ts 锁死）。
- **消息桥 = 纯 window.postMessage 三层通道**（previewMessages.ts 单点）：文档层（内容 iframe ↔ 主窗，经宿主桥 relay）上行 {zoom, scroll, nav, font_probe, keyfwd}、下行 {reset, zoom_set, scroll_set}；宿主层（桥生命周期信号）上行 {host_ready, iframe_loaded}、下行 {host_content}。origin 实证 = "null"（opaque 序列化）——主窗→宿主 targetOrigin 只能 `"*"`；主窗校验 = `event.source === iframe.contentWindow` + origin "null"；宿主桥下行校验 = source 归属 + 主窗 origin 白名单（tauri.localhost / localhost:1420）双闸。
- **键盘语义 = keyfwd 收窄转发（D2）**：内容 iframe keydown（焦点在 input/textarea/select/contenteditable 时注入段跳过不转发）→ slterm_keyfwd 上行（code + 四修饰键，不含 key）→ 主窗合成 KeyboardEvent 经 ShortcutRegistry `resolve(ev, "global")` 消费——预览聚焦时全局快捷键可用；表单键入/系统复制快捷键解禁（旧 focusable=false 窗口的「表单不可达」已知行为消亡）。**威胁面登记**：nonce 明文内联 → 内容脚本可提取伪造 keyfwd；危害边界 = global 命令集（仅 global.closeTab，command-catalog.test.ts 锁死），global 集扩充须重估。
- **宿主内联 `<script>` 真实执行（CP-031）**：injectScript 不再做字符串级转义——宿主脚本段原样进入渲染文档；预览与主窗口 CSP 隔离，收紧主窗口 CSP 不影响预览。

### PreviewFrame（主窗侧宿主 iframe 单点）

- 职责：渲染宿主 iframe 直填内容区（width/height 100%——显隐/几何由 Dockview/workspace CSS 天然驱动）；内容装配（injectScript + buildInjectedScript + nonce 原样）→ slterm_host_content 直推（**host_ready 每次到达均重推**——宿主 iframe 可因 dockview 面板 DOM reparent 二次加载，布尔 state 幂等会吃掉二次 ready 的重推致内容 iframe 恒空白，2026-09-13 E2E 实证；就绪序号计数实现 + 内容变化重推）；消息分派（source 归属 + origin "null" → 类型分派 → nonce + 数值守卫）；下行 reset/zoom_set/scroll_set postMessage（宿主桥 relay 进内容 iframe）；keepZoom/keepScrollRatio 镜像与恢复（slterm_iframe_loaded 触发）；keyfwd 上行 → resolve(ev, "global")。
- **面板根工具条带约定**：宿主 iframe 直填面板内容区，FloatingArea 恒位于内容区上方 40px 工具条带内（面板根列排布局）；`direction="row"`（切换条/HUD 同行）。
- PreviewFrame 不渲染 HUD：zoom 变化经 onZoomChange 上报（显示在面板根悬浮区）；内容重建归 1 经 onZoomReset 通知（静默隐藏，非「显示 100%」——report 只承载真实变化，回落 1.0 属变化需显示，语义分离红线）；重置下行经 ref 命令接口（PreviewFrameHandle.resetZoom）。
- 内部镜像 zoomRef/ratioRef = keepZoom/keepScrollRatio 重建恢复取值源（上行变化/复位同步，等值不重复处理）。
- keepZoom/keepScrollRatio 仅会话级（宿主 iframe 内容存亡），不跨会话持久化（html 现状语义继承）。
- **E2E 探针（spike Q4 裁决——embedded driver frame 内 execute 全灭）**：E2E_ENABLED 门控写主窗全局 `__slterm_e2e_previewDoc`（panelId → 最近推送产物）/ `__slterm_e2e_iframeLoaded`（panelId → 加载完成计数）/ `__slterm_e2e_fontProbe`（TE-08 上行收束）——L4 内容/加载断言全走主窗探针全局。

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
- **keyForward 基础段（恒首，ADR-0021/D2）**：keydown 收窄转发段（表单焦点跳过）随预览回迁恢复——产物 = keyForward 段 → extra 段 → zoom 段（恒末位，fragmentNav 的 click 段 "},true);" 收尾衔接 "var sltermZoom="——html-panel 控制流断言正则锁死，勿调换）。旧 slterm_key 命令重放语义不复活（CP-013）——keyfwd 仅经 resolve(ev,"global") 解析消费。
- 可选段：fragmentNav（html：# 链接拦截 + :target 模拟）/ linkRouter（md：非 # 链接上行 slterm_nav）/ scrollReport（md：滚动节流上行 + 下行恢复）/ fontProbe（E2E 专用字体加载探针，TE-08——仅 VITE_E2E 拼装传入，生产零注入面；iframe 内 fonts.ready 后 check('12px "KaTeX_Main"') 上行宿主桥，结果写主窗 `__slterm_e2e_fontProbe` 供 L4 断言。宿主页 FontFaceSet 不覆盖 iframe 文档且 opaque origin 不可读——2026-09-09 实证 hostSize=0、check 对任意族名恒 true，故锚点必须自 iframe 内取）。
- zoomRuntime/scrollRuntime 为参数化函数源码（function(doc,win)——仅与宿主页窗口树内对话）——L2 经 new Function 桩执行行为级覆盖（jsdom 不执行 srcdoc 脚本）。

### 消息协议常量单点

previewMessages.ts 承载全部消息类型与构造/守卫（三层通道，ADR-0021 头注）：文档层（zoom/scroll/nav/keyfwd/reset/set + E2E 字体探针 fontProbe/slterm_font_probe，TE-08）+ 上/下行类型白名单（keyfwd 为第五上行类型——ADR-0021/D2；探针仅 VITE_E2E 构建注入可达，PreviewFrame 侧 E2E_ENABLED 门控收束）+ 宿主层（host_ready/iframe_loaded/host_content + PREVIEW_HOST_URL）；新增消息类型在此登记并同步注入段与 PreviewFrame 分派。**宿主页桥（src-tauri/src/preview.rs HOST_PAGE）按字段白名单式转发**（zoom/ratio/href/loaded + keyfwd 五字段 code/ctrlKey/shiftKey/altKey/metaKey——新增上行字段须同步桥脚本，否则载荷静默丢弃，Rust 侧 `host_page_bridge_*` 测试锁）。

## 外部坑/红线

- **勿把面板渲染期 ref 当可用容器**：useCodeMirror 类消费方在 render 阶段读 ref 得 null——容器首挂需 commit 后 bump 重渲染（HtmlPanel/MarkdownPanel 的 bumpFrame 桥接，DiffPanel 先例）。
- **jsdom 无 crypto.randomUUID**（node 环境缺失实证）：占位/随机一律 crypto.getRandomValues hex（createNonce/randomHex 同款）。
- **勿重建「悬浮区坐标机制」多轨形态**：切换条/HUD 坐标协调只有悬浮区单点一种合法形态，S10-② 起恒承载于面板工具条带（row）——任何形态不加双轨自摆。
- **report 与 hide 语义分离**：report(1.0) 是回落变化需显示（Chrome 气泡语义）；复位/重建归 1 走 hide（静默），混用会复活或误显气泡。
- **keyfwd 收窄边界勿扩**：内容 iframe 上行只经 resolve(ev,"global") 消费——global 命令集扩充（command-catalog）须先重估 keyfwd 伪造威胁面（nonce 明文内联，内容脚本可提取；现危害边界 = 仅 global.closeTab）；表单焦点跳过名单（input/textarea/select/contenteditable）勿删——删即吞表单键入。
- **宿主 iframe 跨源即不可读**：主窗 JS 无法 contentDocument 访问宿主文档（slterm-preview.localhost vs tauri.localhost）——内容断言/交互一律经消息通道或 E2E 探针，勿尝试 DOM 穿透。
- **dockview 面板 DOM reparent → 宿主 iframe 重载**：iframe 同文档 reparent 即重新加载（浏览器行为）；dockview `setActive`/布局重排可 reparent 面板内容 DOM（2026-09-13 E2E 实证：activatePanel 后 host_ready 二次到达）——宿主 iframe 内容文档随之销毁重建，内容侧脚本状态（注入段闭包/已排队定时器/fixture script）全部丢失。主窗侧韧性 = host_ready 每次到达均重推（就绪序号计数，勿改回布尔）；E2E 用例侧注意——activatePanel 后 fixture 的延时派发以**重推后的新文档**为执行体，时序预算从第二次加载起算。

## 测试模式

- 注入段字符串断言 + parse-only（doc-viewer-injection.test.ts）；zoom 桩执行（doc-viewer-zoom-runtime.test.ts）/ scroll 桩执行（doc-viewer-injection.test.ts 内）；keyForward 桩执行（doc-viewer-injection.test.ts——非表单上行完整载荷/表单四形态跳过）。
- PreviewFrame 宿主桥单元测试 = preview-frame-host-bridge.test.tsx（jsdom 真实 iframe + MessageEvent 构造边界；下行捕获 = contentWindow postMessage spy（WeakMap 缓存防叠加）——**spy 必须先于 host_ready dispatch 安装**，否则推送在捕获前落地）；面板集成面（html-panel.test.tsx / markdown-panel.test.tsx）同模式。
- 宿主页桥脚本行为 = Rust 侧 host_page_bridge_* 字符串断言（jsdom 不加载跨源 src、不执行宿主页脚本）；真实 WebView2 往返（宿主桥 relay/iframe 执行/keyfwd 消费）由 html.e2e.ts / markdown.e2e.ts 验收（探针模式，e2e-tests/CLAUDE.md）。
