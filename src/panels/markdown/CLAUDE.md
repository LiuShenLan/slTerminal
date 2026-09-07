# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

`src/panels/markdown` 是 .md/.markdown 文档面板（markdownviewer）实现：三形态编排器（edit/split/preview）+ markdown 渲染纯管线。编辑走 CM6（useCodeMirror 扩展选项，editor/CLAUDE.md），预览走 docViewer 共享层（PreviewFrame + 注入段）。

## 关键约束与决策

### 三形态与草稿模型（决策 #1/#6）

- 形态 viewMode ∈ edit/split/preview（默认 edit；非法回退；随 params 持久化跨会话恢复）。splitRatio（allotment 拖拽比例）同样随 params 持久化（onDragEnd 才落盘，拖拽过程高频不写）。
- **文档真值源 = 面板 docRef**（草稿优先磁盘；预览永不直读磁盘）。CM 击键经 onDocContent 即时写回（S3 useCodeMirror 扩展 edit 源）。
- **CM 恒挂载（CP-037）**：allotment CM pane 恒 index 0 且全形态挂载——preview 态以 `visible=false` 隐藏保活（allotment 收拢不占空间，display:none 照 edit↔split 先例），undo/光标跨 edit/split/preview 全形态保留，回 edit/split 免 initialDoc 快照回填重建；代价 preview 常驻一个 CM 实例内存，已接受。原「preview-only 卸载（快照回填，光标/undo 重置）」登记行为随 CP-037 撤销。
- **preview 内容变化驱动**：onDocContent 标 stale + 300ms 防抖（仅 split/preview 启动计时）；切形态时 stale/首入立即渲染；异步渲染产物经 gen 丢弃（编辑继续时过期产物不落地）。外部修改 reload（useCodeMirror 内建 fs-event 链，view 存活即生效）经 reload 源同步 doc；CM 恒挂载后 preview 态与 edit/split 同路径（含脏文件确认弹窗），原「preview-only 不监听」句随 CP-037 撤销。

### 渲染管线分层

- **mdPipeline（同步纯函数）**：markdown-it（html:true 透传信任模型 ADR-0017 / linkify / GFM 表删除线 / task-lists / texmath-katex dollars 就地渲染 throwOnError=false / hljs 白名单语言静态 import 高亮别名归一）+ 资源收集（<img>/<source> src 后处理，markdown 图片语法产物同被扫到）+ mermaid fence 占位（randomHex 防碰撞）+ buildPreviewDocument（完整文档 head：排版 CSS 经 buildMdPreviewStyleCss 同源装配 + KaTeX 内联字体）。模块级单例解析器可复用于多渲染。
- **mdRenderAsync（异步编排）**：资源 data: URL 替换（LRU 50 缓存；失败回退原 src 不阻塞）/ mermaid 渲染替换 / isCancelled 丢弃。
- **mermaidHost**：dynamic import 单例（vite 分包 2MB 不进主包）+ 按 code Promise 缓存 + 失败占位卡；宿主侧渲染成 SVG 字符串注入（iframe 零重排版、CSP 零新增）。
- **KaTeX 字体**：generated/katexInlineCss.ts 为构建产物（scripts/gen-katex-inline.mjs 生成，woff2 data: 内联，woff/ttf 回退剔除）——katex 升级重跑脚本 + git diff 审阅，勿手改产物。

### 本地资源（决策 #9，ADR-0018）

- 相对 src → 绝对化（assets.ts 纯函数：docDir join、盘符直用、`..` 不越盘符根）→ MIME 白名单收集 → fs.readResourceBase64（后端沙箱/10MB/base64 分块）→ data: URL。沙箱越界由后端拒绝（前端不预判）。
- 盘符绝对路径与协议同形（`C:` vs `http:`）——isLocalRef/classifyLink 先判盘符再判协议（顺序红线，测试锁死）。

### 链接点击（决策 #10）

slterm_nav 上行 → classifyLink（linkPolicy 纯函数）：external（http/https/mailto/tel）→ shell.openUrl（opener）；local（相对/盘符绝对）→ openFileInActivePage（workspace 共享打开链路，复用双击分发；失败静默）；fragment/ignored 忽略（markdown-it 标题无锚点 id，本地文档内 # 不可达）。

### 预览框参数与悬浮区

PreviewFrame segments=[linkRouter, scrollReport]、keepZoom + keepScrollRatio（iframe 重建恢复）、onZoomChange={zoomHud.report} / onZoomReset={zoomHud.hide}（悬浮区显示层）。右上悬浮区（FloatingArea）恒面板根——切换条上 / 缩放 HUD 下列排，edit 态无缩放源传 hud=null；重置链 = PreviewFrame ref.resetZoom + zoomHud.hide（2026-09-06 收敛，docViewer/CLAUDE.md）。

### 编辑字号语义（2026-09-06 接线）

md/html 编辑形态字号 = 共享 `editorFontSize` store（Ctrl+滚轮缩放，EditorPanel 同款接线：useCodeMirror 的 fontSize/onFontSizeChange props——wheel 由 hook 无条件挂载，缺 props 会吞事件无效果）。原「恒锁默认 14」行为已变更：从此与 txt 编辑器同源（范围 [8,32] clamp、2s debounce 持久化）。split 态左 pane 滚轮=字号、右 iframe 滚轮=预览 zoom（pane 边界即语义边界，iframe 事件宿主侧物理不可达无冲突）。

### 预览配色单点（2026-09-06 收编）

md 预览经 srcdoc iframe 渲染无法引用宿主 CSS 变量，但配色值一律以 active 方案驱动：mdPreviewStyle 的 buildMdPreviewStyleCss() 每次渲染现拼——正文/底色 = editor.overrides.plainText/background、代码语法 = syntax 9 键（hljs 12 类映射）、结构色 = editor.overrides.preview 组（新增键在 schemes/types.ts 注释登记）、复选框强调引 ui.focusBorder、mermaid 错误文案引 ui.errorFg。改主题即跟随（比 CM editorTheme 常量更活）；**禁止在此手抄色值**（曾以字面量双轨登记豁免，已撤销）。

### 测试模式

- 纯管线直测（pipeline/assets/links/async 编排——mermaid 模块 mock，jsdom 无布局）。
- 面板集成（markdown-panel.test.tsx）：mock CM 桥（onDocContent 手动驱动）/allotment（透传 children）/ShortcutRegistry；iframe 消息派发走 MessageEvent 构造（extractNonce from srcdoc）。
- L4 markdown.e2e.ts：真实 WebView2 渲染产物/图片 data/mermaid SVG/Ctrl+W/缩放（事件属性通道——宿主 script 静态化存量缺陷，raw HTML 事件属性不受转义破坏）。

## 外部坑/红线

- **host 内联 `<script>` 不执行**（escapeScriptClose 转义存量缺陷继承，ADR-0017）——e2e 触发 md 行为用事件属性（img onerror 先例），不依赖宿主 script。
- **allotment 动态 pane**：CM pane 须恒 index 0 位置（保活依赖 React 位置协调，调换 pane 顺序会致 CM 卸载重建丢 undo）。
- **fake timers 下 waitFor 冻结**（轮询 setTimeout 被 fake）——防抖断言先切回真实时钟再 waitFor。
- hljs 语言经静态 import（非 require——ESM 项目无 require）；新增语言=HLJS_LANG_FNS 表 + import 一行。
