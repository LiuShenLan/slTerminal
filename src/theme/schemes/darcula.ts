// darcula — 内置默认配色方案（JetBrains IDEA Darcula 暗色主题）
//
// 交叉引用（启动链 fail-safe，spec §9.1）：React 挂载前的静态硬编码色不在方案系统内，
// 改本文件对应 ui 值时必须手动同步——
//   ui.appBgPrimary (#1e1e2e) ↔ index.html:10 body background
//   ui.appBgPrimary (#1e1e2e) ↔ src-tauri/tauri.conf.json:20 window backgroundColor
//   ui.panelBg (#1e1e1e) / ui.errorFg (#f44747) ↔ src/main.tsx:31 超时错误页
//
// 值一律搬运现状（D1 零视觉变化，禁止新造）；消费注释在 types.ts（决策 D8）。

import { oneDark } from "@codemirror/theme-one-dark";
import type { ColorScheme, UiTokens } from "./types";

// --- ui 段（值 = src/theme/colors.ts 现状 + App.css:6 appFg 收编 + JsonMode:213 onAccentFg 收编）---

const ui: UiTokens = {
  gitFile: {
    modified: "#6897BB",
    added: "#629755",
    untracked: "#D1675A",
    deleted: "#6C6C6C",
    renamed: "#3A8484",
    conflict: "#D5756C",
    ignored: "#848504",
  },
  gitGutter: {
    modified: "#374752",
    added: "#384C38",
    deleted: "#656E76",
  },
  explorer: {
    bg: "#1E1E1E",
    fg: "#D4D4D4",
    hover: "#2A2D2E",
    arrowClosed: "#6C6C6C",
    arrowOpen: "#D4D4D4",
  },
  sidebar: {
    bg: "#252526",
    fg: "#D4D4D4",
    hover: "#2A2D2E",
    selected: "#37373D",
    border: "#444",
    contextMenuBorder: "#454545",
    contextMenuShadow: "0 4px 12px rgba(0,0,0,0.5)",
    treeGuide: "#3C3C3C",
  },
  errorBanner: {
    bg: "#5A1D1D",
    border: "#8B0000",
    fg: "#F48771",
  },
  agentStatusUsage: {
    low: "#629755",
    medium: "#BBB529",
    high: "#F44747",
  },
  panelBg: "#1E1E1E",
  sidebarBg: "#252526",
  secondaryBg: "#2D2D2D",
  appBg: "#1e1e2e",
  appBgPrimary: "#1e1e2e",
  appFg: "#cdd6f4",
  editorBg: "#282C34",
  sidebarFg: "#D4D4D4",
  errorFg: "#F44747",
  placeholderFg: "#808080",
  buttonFg: "#CCCCCC",
  dimFg: "#999999",
  inputBg: "#3C3C3C",
  inputBorder: "#6C6C6C",
  focusBorder: "#007ACC",
  activeSelectionBg: "#094771",
  separatorBg: "#444",
  contextMenuBorder: "#454545",
  shadowMenu: "rgba(0,0,0,0.5)",
  htmlPanelLoadingFg: "#6C6C6C",
  htmlPanelIframeBg: "#FFFFFF",
  onAccentFg: "#FFFFFF",
  explorerSelectionBg: "#094771",
};

// --- 方案组装（四段）---

export const darcula: ColorScheme = {
  id: "darcula",
  label: "Darcula",
  ui,
  // terminal 段（值 = src/panels/terminal/theme.ts theme 段现状，25 键）
  terminal: {
    foreground: "#D4D4D4",
    background: "#1E1E1E",
    cursor: "#D4D4D4",
    cursorAccent: "#1E1E1E",
    selectionBackground: "#264F78",
    selectionForeground: "#D4D4D4",
    // 滚动条三键 = foreground 20%/40%/50% 等价（ITheme 显式化原运行期派生值，spec §4.3）
    scrollbarSliderBackground: "rgba(212,212,212,0.2)",
    scrollbarSliderHoverBackground: "rgba(212,212,212,0.4)",
    scrollbarSliderActiveBackground: "rgba(212,212,212,0.5)",
    black: "#000000",
    red: "#CD3131",
    green: "#0DBC79",
    yellow: "#E5E510",
    blue: "#2472C8",
    magenta: "#BC3FBC",
    cyan: "#11A8CD",
    white: "#E5E5E5",
    brightBlack: "#666666",
    brightRed: "#F14C4C",
    brightGreen: "#23D18B",
    brightYellow: "#F5F543",
    brightBlue: "#3B8EEA",
    brightMagenta: "#D670D6",
    brightCyan: "#29B8DB",
    brightWhite: "#FFFFFF",
  },
  // editor 段 = oneDark 直 import 透出（D6）+ lint/searchMatch/background 覆盖
  // （值 = spec §4.4 现行有效值，测试 mock @codemirror/theme-one-dark 包继续生效）
  editor: {
    theme: oneDark,
    overrides: {
      background: "#282C34",
      lint: {
        error: "#f11",
        warning: "orange",
        info: "#999",
        hint: "#66d",
        activeBackground: "#ffdd9980",
        tooltipBackground: "#2e343e",
        tooltipBorder: "#444",
      },
      searchMatch: {
        match: "#72a1ff59",
        matchOutline: "#457dff",
        selected: "#6199ff2f",
        selectionMatch: "#aafe661a",
      },
    },
  },
  // libraries 段 = dockview 20 条 + allotment 2 键（值 = spec §4.5 现状）；
  // 与 ui 同值的条目引用 ui 槽位构造，值单点定义
  libraries: {
    dockview: {
      "--dv-group-view-background-color": ui.panelBg,
      "--dv-tabs-and-actions-container-background-color": ui.sidebarBg,
      "--dv-activegroup-visiblepanel-tab-background-color": "#1E1E1E",
      "--dv-activegroup-hiddenpanel-tab-background-color": ui.secondaryBg,
      "--dv-inactivegroup-visiblepanel-tab-background-color": "#1E1E1E",
      "--dv-inactivegroup-hiddenpanel-tab-background-color": ui.secondaryBg,
      "--dv-tab-divider-color": "#1E1E1E",
      "--dv-separator-border": ui.separatorBg,
      "--dv-paneview-header-border-color": "rgba(204,204,204,0.2)",
      "--dv-activegroup-visiblepanel-tab-color": "#FFFFFF",
      "--dv-activegroup-hiddenpanel-tab-color": "#969696",
      "--dv-inactivegroup-visiblepanel-tab-color": "#8F8F8F",
      "--dv-inactivegroup-hiddenpanel-tab-color": "#626262",
      "--dv-drag-over-background-color": "rgba(83,89,93,0.5)",
      "--dv-icon-hover-background-color": "rgba(90,93,94,0.31)",
      "--dv-floating-box-shadow": "0 8px 32px rgba(0,0,0,0.4), 0 2px 8px rgba(0,0,0,0.25)",
      "--dv-floating-border": "1px solid rgba(255,255,255,0.1)",
      "--dv-tabs-container-scrollbar-color": "#888888",
      "--dv-scrollbar-background-color": "rgba(255,255,255,0.25)",
      "--dv-paneview-active-outline-color": "dodgerblue",
    },
    allotment: {
      separatorBorder: "rgba(128,128,128,0.35)",
      focusBorder: "#007fd4",
    },
  },
};
