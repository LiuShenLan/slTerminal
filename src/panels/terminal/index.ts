// terminal 面板 barrel export——仅导出 TerminalPanel 组件。
// terminalOptions re-export 已清理（FE-35）：全仓消费方（useXterm.ts /
// theme.test.ts / use-xterm-lifecycle.test.ts）均直接导入 `./theme`，
// 无经本 barrel 的消费方。
export { default as TerminalPanel } from "./TerminalPanel";
