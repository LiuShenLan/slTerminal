# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 存在理由

自绘窗口标题栏（TB-02/UI-301，UI 重设计 Stage 04）——`tauri.conf.json` `decorations: false` 后由本组件承担原生标题栏职责：窗口拖拽、双击最大化、最小化/最大化/关闭三钮（经 `src/ipc/window` wrapper）。中段标题显示活跃项目名 / 页面名。

## 关键约束与决策

### 拖拽与双击（TB-04）

- **拖拽**：左段 app 标识 + 中段标题区域标 `data-tauri-drag-region="deep"`（子树拖拽）——Tauri 2.11.3 裸属性语义只命中直接点击的元素本身，文字 span/svg logo 子元素会拦截拖拽；`deep` 使子树内任意处可拖。
- **拖拽区必须撑满全高**（`height: "100%"`，问题 6）：标题栏容器 34px + `alignItems: center`，拖拽区 div 无显式高度时收缩到内容高度，无项目时中段空 div 高度 0 → 点击死区拖不动。
- **双击最大化**：**Tauri 原生拖拽区脚本承担**（drag.js `detail===2` → `internal_toggle_maximize`）——本组件**不注册** `onDoubleClick`（React dblclick 与原生各 toggle 一次 → 最大化后立即还原，净无效果）。
- **右段窗口控制三钮不在拖拽区内**（保证可点击）。

### 窗口控制三钮（TB-03）

- 三钮 = 最小化 / 最大化/还原 / 关闭，38×26、图标 12px、`aria-label` + `title`。
- IPC 一律经 `src/ipc/window` wrapper：`minimizeWindow` / `toggleMaximizeWindow` / `closeWindow`。
- **关闭 = `closeWindow()`**——触发 `onCloseRequested` 事件，复用 `registerCloseHandler` 注册的 P1-19 关窗链路（遍历 TerminalRegistry 杀 PTY + 后端 Job Object 兜底），禁止 `destroy`/`process.exit` 绕过。
- **关闭钮 hover 例外**：hover 底 = `TITLEBAR_CLOSE_HOVER_BG`（`ui.titlebarCloseHover` 危险色，UI-301 定值，FE-07 token 化）。

### 标题数据与视觉

- **数据**：中段标题 = `useLayout.activePageId` → 所属项目名 / 页面名（projects store 直接推导，**禁止改 store**）；layout 无活跃页时回退第一个项目的 `activePageId` 页。
- **窄订阅（FE-21）**：标题推导经 `useShallow` 包装，selector 只返回 `{projectName, pageName}` 两原始值字段——无关项目变更结果浅相等，不触发 TitleBar 重渲染。
- **视觉**：34px 高、`TITLEBAR_BG` 底 + 底部 1px `SEPARATOR_BG` 发丝线、12px 文字、userSelect none；全部颜色经 `theme/colors.ts` token（硬约束 #6）。

## 测试模式

L2 测试：`src/__tests__/title-bar.test.tsx`（TB-06-1~9 + FE-21-1/2）：

- 三段结构渲染、中段标题按 store 种子、三钮点击调对应 IPC wrapper。
- 中段无 React 双击 handler；左/中段拖拽区 `data-tauri-drag-region="deep"` + `height: "100%"`。
- FE-21 窄订阅：无关项目变更不触发重渲染，切换 layout activePageId 标题响应更新。

## 运行

```bash
npx vitest run title-bar
npm test
```
