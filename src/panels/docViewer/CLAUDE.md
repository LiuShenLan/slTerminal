# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/panels/docViewer` 是文档型预览面板（htmlviewer / markdownviewer）的共享基础设施层（非面板类型，不进 PANEL_TYPES——新增面板仍走 panels/CLAUDE.md 四步）。iframe 沙箱、postMessage 总线与缩放/滚动运行时是安全红线最密集处，收敛单点是复用而不复制的唯一方式；面板各自持有文件读取、loading/error 态与业务链接策略。

依赖方向：`html/`、`markdown/` → `docViewer/`；docViewer 不得反向引用面板实现（markdown 的 linkPolicy 分类在面板侧，PreviewFrame 只透传 href）。

## 关键约束与决策

### PreviewFrame（红线单点收容）

由原 panels/html/HtmlPanel 注入/总线迁出（HUD 显示层 2026-09-06 再迁 FloatingArea/useZoomHud）。红线逐条随迁（SEC-03/04 登记见 panels/CLAUDE.md「HTML 面板」节历史）：

- **sandbox="allow-scripts"** 不含 allow-same-origin（Tauri CVE-2024-35222）。
- **四层消息校验**：origin === "null" → source === iframe.contentWindow → type → nonce（SEC-04 128 位挂载期随机，惰性 ref 防漂移）。
- **postMessage targetOrigin 一律 "\*"**（SEC-03 实证：须匹配接收方 origin，opaque 源只影响 e.origin 序列化）。
- 上行通道：`slterm_key`（global 命令重放，仅 global.closeTab 等 global context——扩充前必读 ADR-0017）/ `slterm_zoom`（父侧镜像 + 经 onZoomChange 上报外层）/ `slterm_scroll`（keepScrollRatio 镜像）/ `slterm_nav`（透传 onNav，分类在面板）。
- 下行：`slterm_reset` / `slterm_zoom_set`（keepZoom）/ `slterm_scroll_set`（keepScrollRatio）——iframe 侧 source===parent + nonce + type 校验。
- **PreviewFrame 不渲染 HUD**：zoom 变化经 onZoomChange 上报（显示在面板根悬浮区）；iframe 重建归 1 经 onZoomReset 通知（静默隐藏，非「显示 100%」——report 只承载真实变化，回落 1.0 属变化需显示，语义分离红线）；重置下行经 ref 命令接口（PreviewFrameHandle.resetZoom）。
- 内部镜像 zoomRef = keepZoom 重建恢复取值源（上行变化/复位同步，等值不重复处理）。
- keepZoom/keepScrollRatio 仅会话级（iframe 文档存亡），不跨会话持久化（html 现状语义继承）。

### 右上悬浮区（FloatingArea / useZoomHud）

面板根右上角悬浮 UI 单点（2026-09-06 收敛——原 PreviewFrame overlay 槽 + 面板根 overlayBarStyle 双轨定位，md 切换条绕开 overlay 槽致 HUD 与切换条重叠，html 因进槽而幸免 = 复用断裂实证）：

- FloatingArea：absolute 右上（top 8/right 8/zIndex 20），flex 列排——切换条（上）/ HUD 气泡（下），容器 pointerEvents none 子项 auto；data-e2e 前缀由面板传（html-/markdown-zoom-hud 命名兼容历史）。无 44px 几何 hack——避让即列排布局本身。
- useZoomHud：HUD 状态机（report 变化显示含回落 100% / 等值回声不复活 / hide 静默归 1 / 3s 续期消失），flushSync 于 report 内（原生 message 事件内紧急 UI——自 PreviewFrame 注释语义随迁）。
- 装配链（两面板同构）：面板持 useZoomHud + PreviewFrame ref；zoom 上行 → onZoomChange={report}；重建/重置 → onZoomReset={hide} / reset 按钮 onReset = frameRef.resetZoom() + hide()。
- ModeSwitcher 仍受控纯展示（不含定位）；定位恒由面板根 FloatingArea 承载——任何形态不加双轨自摆。

### 注入脚本组装（buildInjectedScript）——拼接纪律

- 输出源码不得含 `</script>` 字面量；每段完整语句分号收尾（2026-09-06 SyntaxError 实证：click 段原无分号致追加段整段失效）。
- 字符串插值 JSON.stringify；数值常量十进制字面量直插。
- **段序固定：基础段（键转发）→ extra 段 → zoom 段（恒末位）**——html-panel 控制流断言正则 `/\},true\);var sltermZoom=/` 锁死 fragmentNav 与 zoom 衔接，勿调换。
- 可选段：fragmentNav（html：# 链接拦截 + :target 模拟）/ linkRouter（md：非 # 链接上行 slterm_nav）/ scrollReport（md：滚动节流上行 + 下行恢复）。
- zoomRuntime/scrollRuntime 为参数化函数源码（function(doc,win)）——L2 经 new Function 桩执行行为级覆盖（jsdom 不执行 srcdoc 脚本）。

### ModeSwitcher / overlay 定位

ModeSwitcher 受控纯展示（不含定位）；定位由宿主负责——悬浮区恒渲染于面板根右上角（FloatingArea），edit 形态同样如此；PreviewFrame 已无 overlay 槽（纯 iframe 容器）。

## 外部坑/红线

- **勿把面板渲染期 ref 当可用容器**：useCodeMirror 类消费方在 render 阶段读 ref 得 null——容器首挂需 commit 后 bump 重渲染（HtmlPanel/MarkdownPanel 的 bumpFrame 桥接，DiffPanel 先例）。
- **jsdom 无 crypto.randomUUID**（node 环境缺失实证）：占位/随机一律 crypto.getRandomValues hex（createNonce/randomHex 同款）。
- **勿重建「PreviewFrame 内 overlay 槽」类悬浮坐标机制**：切换条/HUD 坐标协调只有悬浮区单点一种合法形态（曾登记「pane 内坐标系错位勿用槽」——实为伪命题（Allotment 全高 + 预览 pane 贴右/顶缘，顶右几何重合），真问题在双轨定位本身，2026-09-06 已收敛）。
- **report 与 hide 语义分离**：report(1.0) 是回落变化需显示（Chrome 气泡语义）；复位/重建归 1 走 hide（静默），混用会复活或误显气泡。

## 测试模式

- 注入段字符串断言 + parse-only（doc-viewer-injection.test.ts）；zoom 桩执行（doc-viewer-zoom-runtime.test.ts）/ scroll 桩执行（doc-viewer-injection.test.ts 内）。
- PreviewFrame 四层校验负面用例在 html-panel.test.tsx / markdown-panel.test.tsx 集成面（dispatch MessageEvent 构造）。
- 真实 WebView2 postMessage 往返由 html.e2e.ts / markdown.e2e.ts 验收（jsdom 无法模拟 opaque origin 序列化）。

## 消息协议常量单点

previewMessages.ts 承载全部 iframe ↔ 父窗口消息类型与构造/守卫（zoom/scroll/key/nav/reset/set）；新增消息类型在此登记并同步注入段与 PreviewFrame 分派。
