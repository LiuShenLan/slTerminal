// xterm.js 暗色主题配置 — JetBrains 暗色配色
// 配色单点：所有终端颜色在此定义，组件引用此 token（硬约束 #6）

import type { ITerminalOptions } from "@xterm/xterm";

/** xterm.js 终端选项（暗色主题） */
export const terminalOptions: ITerminalOptions = {
  theme: {
    foreground: "#D4D4D4",
    background: "#1E1E1E",
    cursor: "#D4D4D4",
    cursorAccent: "#1E1E1E",
    selectionBackground: "#264F78",
    selectionForeground: "#D4D4D4",
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
  // 编译时默认值，运行时由 useFontSize store 覆盖（Ctrl+Wheel 动态调节）
  fontSize: 14,
  fontFamily: `"JetBrains Mono", monospace`,
  cursorBlink: true,
  cursorStyle: "bar",
  allowProposedApi: true,
  scrollback: 5000,
  // windowsPty 由 F3 useEffect 在获取真实 build 号后动态设置，不在此预设空对象
};
