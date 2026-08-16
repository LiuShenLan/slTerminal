# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

自绘窗口标题栏（TB-02/UI-301，UI 重设计 Stage 04）——`tauri.conf.json` `decorations: false` 后由本组件承担原生标题栏职责：窗口拖拽、双击最大化、最小化/最大化/关闭三钮（经 `src/ipc/window` wrapper，契约见 TB-03）。中段标题显示活跃项目名 / 页面名。

## 架构决策

### 拖拽与双击（TB-04）

- **拖拽**：左段 app 标识 + 中段标题区域标 `data-tauri-drag-region="deep"`（子树拖拽）——Tauri 2.11.3 裸属性语义只命中直接点击的元素本身（`el === composedPath[0]`），文字 span/svg logo 子元素会拦截拖拽、无项目时几乎整栏拖不动（人工验证问题 1 根因）；`deep` 使子树内任意处可拖
- **拖拽区必须撑满全高**（`height: "100%"`，问题 6）：标题栏容器 34px + `alignItems: center`，拖拽区 div 无显式高度时收缩到内容高度（左段 ≈16px 窄带、**无项目时中段空 div 高度 0**）——点击死区落点在无 drag 属性的父容器，拖不动；中段 flex 居中（`display: flex` + `alignItems/justifyContent: center`）替代 `textAlign` 保持文字垂直居中
- **双击最大化**：**Tauri 原生拖拽区脚本承担**（drag.js `detail===2` → `internal_toggle_maximize`）——本组件**不注册** `onDoubleClick`（React dblclick 与原生各 toggle 一次 → 最大化后立即还原，净无效果；人工验证连带缺陷）
- **右段窗口控制三钮不在拖拽区内**（保证可点击）

### 窗口控制三钮（TB-03）

- 三钮 = 最小化（`IconMin`）/ 最大化/还原（`IconMax`）/ 关闭（`IconCloseWin`），38×26、图标 12px、`aria-label` + `title`；hover 底 `ui.secondaryBg`（**关闭钮例外**：hover 底 = 危险色 `#c04747`——设计定值，无 token 槽位，组件内常量 `CLOSE_HOVER_BG`）
- IPC 一律经 `src/ipc/window` wrapper：`minimizeWindow` / `toggleMaximizeWindow` / `closeWindow`
- **关闭 = `closeWindow()`**（`getCurrentWindow().close()`）——触发 `onCloseRequested` 事件，**复用 `registerCloseHandler` 注册的 P1-19 关窗链路**（遍历 TerminalRegistry 杀 PTY + 后端 Job Object KILL_ON_JOB_CLOSE 兜底），禁止 `destroy`/`process.exit` 绕过

### 标题数据与视觉

- **数据**：中段标题 = `useLayout.activePageId` → 所属项目名 / 页面名（projects store 推导，无现成 selector 直接推导——**禁止改 store**）；layout 无活跃页时回退第一个项目的 `activePageId` 页
- **视觉**：34px 高、`TITLEBAR_BG`（明度阶梯 l2）底 + 底部 1px `SEPARATOR_BG` 发丝线、12px 文字、userSelect none；左段 app 标识 = accent-dim 底（`ACTIVE_SELECTION_BG`）+ `ACCENT_FG` 终端提示符 SVG（lucide Terminal path，与 final-mockup 一致）+「slTerminal」500 字重；全部颜色经 `theme/colors.ts` token（硬约束 #6，仅关闭钮 hover 危险色为例外常量）

## 文件

| 文件 | 职责 |
|------|------|
| `TitleBar.tsx` | 自绘标题栏组件（单文件模块，无 index.ts）：拖拽区/中段标题/三钮 + `useActiveProjectPage` 标题推导 |

## 测试模式

L2 测试：`src/__tests__/title-bar.test.tsx`（9 用例，TB-06-1~9，用例数见 `.claude/test-inventory.md`）：

- TB-06-1 三段结构渲染（左段 app 标识 + 右段三窗口钮）
- TB-06-2 中段按 store 种子显示活跃项目名与活跃页面名
- TB-06-3/4/5 三钮点击 → `minimizeWindow`/`toggleMaximizeWindow`/`closeWindow` 各调用一次（mock `../ipc/window`）
- TB-06-6 中段无 React 双击 handler——双击不调 `toggleMaximizeWindow`（原生承担，TB-04 修订）
- TB-06-7 左/中段容器恰两处且属性值 `"deep"`（子树拖拽，TB-04 修订）
- TB-06-8 无项目时中段空 div 撑满全高可拖（deep 属性 + `style.height === "100%"` 双断言——问题 6 防回归，仅断言属性无法防「高度 0 点不到」）
- TB-06-9 左/中段拖拽区均 `height: "100%"`（有项目态，全高可拖）

### 运行

```bash
npx vitest run title-bar      # 仅标题栏测试
npm test                      # L2 全量
```
