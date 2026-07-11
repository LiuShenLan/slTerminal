// keyboard.ts — 终端快捷键命令工厂
//
// 职责：返回终端上下文的 Command[]，在 App.tsx 一次性注册（不随终端实例增删）。
// handler 经 getActiveTerminal() 派发到**当前聚焦**终端——多终端下始终作用于聚焦实例。
// 元数据（默认键/优先级）来自 commandCatalog，本文件只提供 handler。
//
// Ctrl+C 明确不注册为命令——xterm.js 自然发送 \x03 到 PTY，供 claude 取消操作。

import type { Command } from "../../features/shortcuts";
import { commandFromMeta } from "../../features/shortcuts";
import { writeText, readText } from "../../ipc/clipboard";
import { getActiveTerminal } from "./activeTerminal";

/**
 * 创建终端面板的快捷键命令列表（一次性注册，handler 派发到聚焦终端）。
 * 无聚焦终端时 handler 返回 false（透传）。
 */
export function createTerminalShortcuts(): Command[] {
  return [
    commandFromMeta("terminal.copy", () => {
      const t = getActiveTerminal();
      if (!t) return false;
      const selection = t.getSelection();
      if (selection) {
        writeText(selection).catch((err) => {
          console.error("[slTerminal] 复制到剪贴板失败:", err);
        });
      }
      // 总是阻止默认（防止 xterm.js 看到 Ctrl+Shift+C）
      return true;
    }),
    commandFromMeta("terminal.paste", () => {
      const t = getActiveTerminal();
      if (!t) return false;
      readText()
        .then((text) => t.paste(text))
        .catch(() => {});
      return true;
    }),
    commandFromMeta("terminal.newline", () => {
      // Ctrl+Enter → 写 \n（0x0a）到 PTY（Ctrl+J 等价，Ink 据此插入换行不提交）
      const t = getActiveTerminal();
      if (!t) return false;
      t.writeToPty(new Uint8Array([0x0a]));
      return true;
    }),
    // Ctrl+C 不注册命令 → 自然透传，xterm.js 发送 \x03 到 PTY
  ];
}
