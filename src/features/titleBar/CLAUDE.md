# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 模块职责

自绘窗口标题栏（TB-02/UI-301，UI 重设计 Stage 04）——`tauri.conf.json` `decorations: false` 后由本组件承担原生标题栏职责：窗口拖拽、双击最大化、最小化/最大化/关闭三钮（经 `src/ipc/window` wrapper，契约见 TB-03）。中段标题显示活跃项目名 / 页面名。

## 架构决策

### 拖拽与双击（TB-04）

- **拖拽**：左段 app 标识 + 中段标题区域标 `data-tauri-drag-region`（Tauri 原生拖拽属性，WebView2 拖拽命中区）
- **双击最大化**：中段 `onDoubleClick` → `toggleMaximizeWindow()`（与标题栏最大化钮共用 wrapper）
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

L2 测试：`src/__tests__/title-bar.test.tsx`（7 用例，TB-06-1~7，用例数见 `.claude/test-inventory.md`）：

- TB-06-1 三段结构渲染（左段 app 标识 + 右段三窗口钮）
- TB-06-2 中段按 store 种子显示活跃项目名与活跃页面名
- TB-06-3/4/5 三钮点击 → `minimizeWindow`/`toggleMaximizeWindow`/`closeWindow` 各调用一次（mock `../ipc/window`）
- TB-06-6 中段双击 → `toggleMaximizeWindow` 一次
- TB-06-7 容器含 `data-tauri-drag-region` 属性

### 运行

```bash
npx vitest run title-bar      # 仅标题栏测试
npm test                      # L2 全量
```
