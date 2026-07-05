// keyboard.ts — 终端快捷键命令工厂
//
// 职责：给定 Terminal 实例，返回终端上下文的 ShortcutCommand[]。
// 快捷键匹配和事件分发由 src/features/shortcuts/ShortcutRegistry 统一管理。
//
// Ctrl+C 明确不注册为命令——xterm.js 自然发送 \x03 到 PTY，供 claude 取消操作。

import type { Terminal } from "@xterm/xterm";
import type { ShortcutCommand } from "../../features/shortcuts";
import { writeText, readText } from "../../ipc/clipboard";

/**
 * 创建终端面板的快捷键命令列表。
 * @param getTerm 获取当前 Terminal 实例的函数（通过 ref 解引用，支持组件卸载后安全访问）
 */
export function createTerminalShortcuts(
  getTerm: () => Terminal | null,
): ShortcutCommand[] {
  return [
    {
      id: "terminal.copy",
      keystroke: {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        metaKey: false,
        code: "KeyC",
      },
      context: "terminal",
      priority: 100,
      handler: () => {
        const term = getTerm();
        const selection = term?.getSelection();
        if (selection) {
          writeText(selection).catch((err) => {
            console.error("[slTerminal] 复制到剪贴板失败:", err);
          });
        }
        // 总是阻止默认（防止 xterm.js 看到 Ctrl+Shift+C）
        return true;
      },
    },
    {
      id: "terminal.paste",
      keystroke: {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        metaKey: false,
        code: "KeyV",
      },
      context: "terminal",
      priority: 100,
      handler: () => {
        readText()
          .then((text) => {
            const term = getTerm();
            if (term) term.paste(text);
          })
          .catch(() => {});
        return true;
      },
    },
    // Ctrl+C 不注册命令 → 自然透传，xterm.js 发送 \x03 到 PTY
  ];
}
