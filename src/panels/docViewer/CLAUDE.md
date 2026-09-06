# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/panels/docViewer` 是文档型预览面板（htmlviewer / markdownviewer）的共享基础设施层（非面板类型，不进 PANEL_TYPES——新增面板仍走 panels/CLAUDE.md 四步）。iframe 沙箱、postMessage 总线与缩放/滚动运行时是安全红线最密集处，收敛单点是复用而不复制的唯一方式；面板各自持有文件读取、loading/error 态与业务链接策略。

依赖方向：`html/`、`markdown/` → `docViewer/`；docViewer 不得反向引用面板实现（markdown 的 linkPolicy 分类在面板侧，PreviewFrame 只透传 href）。

## 关键约束与决策

### PreviewFrame（红线单点收容）

由原 panels/html/HtmlPanel 注入/总线/HUD 迁出。红线逐条随迁（SEC-03/04 登记见 panels/CLAUDE.md「HTML 面板」节历史）：

- **sandbox="allow-scripts"** 不含 allow-same-origin（Tauri CVE-2024-35222）。
- **四层消息校验**：origin === "null" → source === iframe.contentWindow → type → nonce（SEC-04 128 位挂载期随机，惰性 ref 防漂移）。
- **postMessage targetOrigin 一律 "\*"**（SEC-03 实证：须匹配接收方 origin，opaque 源只影响 e.origin 序列化）。
- 上行通道：`slterm_key`（global 命令重放，仅 global.closeTab 等 global context——扩充前必读 ADR-0017）/ `slterm_zoom`（HUD 驱动 + 父侧镜像）/ `slterm_scroll`（keepScrollRatio 镜像）/ `slterm_nav`（透传 onNav，分类在面板）。
- 下行：`slterm_reset` / `slterm_zoom_set`（keepZoom）/ `slterm_scroll_set`（keepScrollRatio）——iframe 侧 source===parent + nonce + type 校验。
- zoom 上行 setHud 用 **flushSync**（原生 message 事件内紧急 UI 反馈，避免并发调度延迟帧——S1 修复记录）。
- keepZoom/keepScrollRatio 仅会话级（iframe 文档存亡），不跨会话持久化（html 现状语义继承）。

### 注入脚本组装（buildInjectedScript）——拼接纪律

- 输出源码不得含 `</script>` 字面量；每段完整语句分号收尾（2026-09-06 SyntaxError 实证：click 段原无分号致追加段整段失效）。
- 字符串插值 JSON.stringify；数值常量十进制字面量直插。
- **段序固定：基础段（键转发）→ extra 段 → zoom 段（恒末位）**——html-panel 控制流断言正则 `/\},true\);var sltermZoom=/` 锁死 fragmentNav 与 zoom 衔接，勿调换。
- 可选段：fragmentNav（html：# 链接拦截 + :target 模拟）/ linkRouter（md：非 # 链接上行 slterm_nav）/ scrollReport（md：滚动节流上行 + 下行恢复）。
- zoomRuntime/scrollRuntime 为参数化函数源码（function(doc,win)）——L2 经 new Function 桩执行行为级覆盖（jsdom 不执行 srcdoc 脚本）。

### ModeSwitcher / overlay 定位

- ModeSwitcher 受控纯展示（不含定位）；定位由宿主负责——PreviewFrame overlay 槽（overlayBarStyle）仅适用于 PreviewFrame 占满面板根的形态（HtmlPanel）；MarkdownPanel 的 PreviewFrame 在 allotment pane 内（坐标系不一致）——切换条恒由面板根渲染。
- HUD 气泡在 overlay 在场时下移避让（paddingTop 44）。

## 外部坑/红线

- **勿把面板渲染期 ref 当可用容器**：useCodeMirror 类消费方在 render 阶段读 ref 得 null——容器首挂需 commit 后 bump 重渲染（HtmlPanel/MarkdownPanel 的 bumpFrame 桥接，DiffPanel 先例）。
- **jsdom 无 crypto.randomUUID**（node 环境缺失实证）：占位/随机一律 crypto.getRandomValues hex（createNonce/randomHex 同款）。
- **md 预览布局里勿用 PreviewFrame overlay 槽放切换条**：pane 内坐标系错位。

## 测试模式

- 注入段字符串断言 + parse-only（doc-viewer-injection.test.ts）；zoom 桩执行（doc-viewer-zoom-runtime.test.ts）/ scroll 桩执行（doc-viewer-injection.test.ts 内）。
- PreviewFrame 四层校验负面用例在 html-panel.test.tsx / markdown-panel.test.tsx 集成面（dispatch MessageEvent 构造）。
- 真实 WebView2 postMessage 往返由 html.e2e.ts / markdown.e2e.ts 验收（jsdom 无法模拟 opaque origin 序列化）。

## 消息协议常量单点

previewMessages.ts 承载全部 iframe ↔ 父窗口消息类型与构造/守卫（zoom/scroll/key/nav/reset/set）；新增消息类型在此登记并同步注入段与 PreviewFrame 分派。
