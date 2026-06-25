// keyboard.ts — 终端键盘事件处理
//
// 职责：
// - Ctrl+Shift+C → 复制终端选中文本到剪贴板
// - Ctrl+Shift+V → 从剪贴板粘贴到终端
// - Ctrl+C → 永不拦截（需求基线：Ctrl+C 保留为中断，发 \x03）
// - IME 合成中 → 全透传

import type { Terminal } from "@xterm/xterm";
import { writeText, readText } from "../../ipc/clipboard";

/** 当前活跃终端引用（由 useXterm 在 Terminal 创建后注册） */
let activeTerminal: Terminal | null = null;

/** 注册活跃终端 */
export function setActiveTerminal(term: Terminal | null): void {
  activeTerminal = term;
}

/** 是否已安装全局监听器 */
let installed = false;

/** 安装全局键盘处理器（capture 阶段，优先于 xterm.js 内置 handler） */
export function installKeyboardHandler(): void {
  if (installed) return;
  installed = true;
  window.addEventListener("keydown", handleKeyDown, { capture: true });
}

function handleKeyDown(e: KeyboardEvent): void {
  // IME 合成中 → 全透传，不拦截
  if (e.isComposing) return;

  const term = activeTerminal;
  if (!term) return;

  // Ctrl+Shift+C → 复制终端选中文本到剪贴板
  if (e.ctrlKey && e.shiftKey && e.code === "KeyC") {
    const selection = term.getSelection();
    if (selection) {
      writeText(selection);
    }
    e.preventDefault();
    e.stopPropagation();
    return;
  }

  // Ctrl+Shift+V → 粘贴剪贴板内容到终端
  if (e.ctrlKey && e.shiftKey && e.code === "KeyV") {
    readText().then((text) => {
      term.paste(text);
    }).catch(() => {});
    e.preventDefault();
    e.stopPropagation();
    return;
  }

  // Ctrl+C → 永不拦截（需求基线：Ctrl+C 保留为中断）
  // xterm.js _keyDown 自然发送 \x03 到 PTY
  // 浏览器 copy 事件独立于 keydown，有选区时自动填剪贴板
}
