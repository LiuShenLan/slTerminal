# Phase 3 Review 发现项 — Hooks 双模式配置面板（F6）

> 2026-08-01 执行期记录。10 个 Stage 全部完成（含 2 轮 fix-loop），L1-L4 全绿。
> 本文件记录执行期发现但未纳入 Stage 范围的问题与待人工验证项。

## 执行概览

| Stage | 内容 | commit | 门禁结果 |
|-------|------|--------|----------|
| 01 | 后端 hooks 三层读写命令 | d847578 | L1 18 条 + clippy ✅ |
| 02 | IPC 封装 + eventsCatalog + matcher 引擎 + configModel | 989c991 | L2 74 条 + tsc + eslint ✅ |
| 03 | 面板骨架 + 注册 + store | 7f1a0c0 | L2 62 条 ✅ |
| 04 | Schema 内嵌 + JSON 模式 | 8bbffb1 | L2 17 条 + 全量 1808 + vite build ✅ |
| 05 | GUI 模式（Master-Detail） | 4954f5f | L2 59 条 + 全量 1867 ✅ |
| 06 | 双模式同步 + 保存安全 | 3877cc6 | L2 9 条 + hooks-config 全 189 ✅ |
| 07 | 单条启停 + F2 并入 | d0e6efe | L2 10 条 + hooks-config 全 205 ✅ |
| 08 | 入口命令（同页单例） | 90af351 | L2 21 条 + 全量 1900 ✅ |
| 09 | L4 E2E（project 层） | cc61783 | L4 26/26 + build:e2e ✅（fix-loop 1 轮） |
| 10 | 文档同步 + 契约回查 | ddc6a9c | 文档 ✅（fix-loop 1 轮，P3-DOC-06 回查结论落盘） |

**最终测试计数**（`.claude/test-inventory.md` 2026-08-01 全量重算）：L1 382 + L2 1809 + L3 116 + L4 26 = **2333**。

## 发现项（未纳入 Stage 范围，待决策）

### 1. hooksConfig 面板点击被吞（UX 瑕疵，**已修复** `db51a67`）

用户验收实测为**每次点击都失效**（比 fix-loop 估判的"首次点击"更严重——每次点击的是不同元素，焦点持续转移）。根因：`onFocus` 绑定根容器 → 元素间焦点转移的 focusin 也冒泡触发 reload；`load()` 无条件 `setLoading(true)` 将内容替换为 loading 占位 → 被点击按钮在 mouseup 前卸载 → click 丢失。

**修复（2026-08-01）**：① `handleFocus` 经 `relatedTarget` 判定——面板内焦点转移跳过重读，仅焦点从外部进入触发（保留 C13-8 聚焦重读语义）；② `load()` 增 `showLoading` 参数，reload 路径保留旧内容不 blank。L2 防回归 2 用例（内部转移不重读/外部进入重读），L4 26/26 无回归。

### 2. MessageDisplay handler 支持档为保守推断

`eventsCatalog.ts` 中 MessageDisplay 按 B 档（command/http/mcp_tool）实现——官方文档未明确其 prompt/agent 支持（依据 D1 §6.7 默认超时表含 command/http/mcp_tool）。执行期确认项 #3 保持保守值，官方明确后回改 eventsCatalog 与 `docs/hooks-dev/contract.md` C13-4。

### 3. `global.openHooksConfig` 默认键未真实环境实测

默认键 `Ctrl+Shift+H` 已静态确认非保留键（reserved.ts 两集并集不命中）。真实 WebView2 中是否被浏览器/Tauri 默认行为拦截**未实测**（L4 用例经 `__dockviewApi.addPanel` 打开面板，未走快捷键路径）。如被拦截需降级 `Ctrl+Alt+H`。

### 4. panels/CLAUDE.md:276 未转义管道符（既有内容）

`setClaudeSession(panelId, patch|null)` 中 `patch|null` 未转义，2 列表格单元格被拆 3 列。非本次引入（git diff 确认），可顺手修复为 `patch\|null`。

### 5. e2e-tests/CLAUDE.md 文件结构表过期

「25 条，23 active + 2 skip」应为「26 条，24 active + 2 skip」（P3-TE-18 新增）。不在 Stage 10 分配范围，未改。

### 6. Bundle 体积 2.6MB

主 chunk 2.6MB minified（schema 230KB + codemirror-json-schema 含 shiki/markdown-it 重依赖）。Tauri 本地加载无网络成本，未做代码分割；如后续需要可 dynamic import JsonMode。

## 契约偏差（已修订落盘）

- **C13-6 API 表述**：契约原文 `compileSchema(schema).validate(data)` 在 json-schema-library@9.3.5 不存在——真实 API 为 `new Draft07(schema).validate(data)`。代码按真实 API 实现（`schema/index.ts`），contract.md C13-6 已同步修订，结论落盘 `contract-recheck.md`。
- **执行期工具注记**：workflow 脚本原为 CRLF 行尾，权限校验器将 CR（0x0D）误判为控制字符导致 Workflow 无法启动——已统一转 LF（11 文件纯行尾修改，随本归档提交）。

## 人工验证点（未代替用户实测）

| # | 项 | 状态 |
|---|----|------|
| 1 | 阶段 3 端到端：GUI 配置 PreToolUse 拦截 hook → 重启 claude → 拦截真实生效 | **待用户实测** |
| 2 | Stage 04：真实 WebView2 中 schema 补全/悬停/错误波浪线效果 | 待实测 |
| 3 | Stage 05：高分辨率/窄窗口 Master-Detail 布局、表单滚动体验 | 待实测 |
| 4 | Stage 06：大配置（数百行 JSON）双模式同步性能、保存拒绝弹窗文案 | 待实测 |
| 5 | ~~Stage 07：slTerminal 外修改 settings.json 后聚焦重读、失效禁用记录准确性~~——**单条启停功能已删除**（验收后决策，ADR-0002 作废） | 已删除 |
| 6 | Stage 08：Ctrl+Shift+H 真实 WebView2 是否被拦截（发现项 #3） | 待实测 |
| 7 | Stage 03：面板占位 UI 视觉比例、层级切换器禁用态样式 | 待实测 |
