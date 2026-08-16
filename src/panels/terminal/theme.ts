// xterm.js 终端主题 adapter — 终端配色映射 active 方案（schemeRegistry.getActive()）
// 的 terminal 段（25 键展开进 xterm ITheme），非独立主题定义（硬约束 #6）；
// 非色选项（drawBoldTextInBrightColors / vtExtensions.kittyKeyboard / scrollback 等）原位保留

import type { ITerminalOptions } from "@xterm/xterm";
import { schemeRegistry } from "../../theme";

/** xterm.js 终端选项（active 方案 terminal 段展开） */
export const terminalOptions: ITerminalOptions = {
  theme: { ...schemeRegistry.getActive().terminal },
  // 编译时默认值，运行时由 useFontSize store 覆盖（Ctrl+Wheel 动态调节）
  fontSize: 14,
  fontFamily: `"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace`,
  cursorBlink: true,
  cursorStyle: "bar",
  allowProposedApi: true,
  scrollback: 5000,
  // 显式声明，消除对 xterm.js 默认值 true 的隐式依赖（仅影响 ANSI 16 色粗体→亮色映射，不影响 True Color）
  drawBoldTextInBrightColors: true,
  // Kitty 键盘协议（CSI u）：允许子进程（如 Claude Code）通过 CSI>1u 激活 Disambiguate 模式，
  // 使 xterm.js 被动编码 Ctrl+Enter、Shift+Enter 等修饰键组合为独立的 CSI u 序列
  vtExtensions: { kittyKeyboard: true },
  // windowsPty 由 F3 useEffect 在获取真实 build 号后动态设置，不在此预设空对象
};
