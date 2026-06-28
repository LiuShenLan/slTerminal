// keyboard.ts — 终端键盘事件处理
//
// 职责：
// - Ctrl+Shift+C → 复制终端选中文本到剪贴板
// - Ctrl+Shift+V → 从剪贴板粘贴到终端
// - Ctrl+C → 永不拦截（需求基线：Ctrl+C 保留为中断，发 \x03）
// - IME 合成中 → 全透传

import type { Terminal } from "@xterm/xterm";
import { writeText, readText } from "../../ipc/clipboard";

// P2-48: 使用 WeakRef 替代强引用，避免已销毁终端被活跃引用阻止 GC
// WeakRef.deref() 在对象被 GC 后返回 undefined，keydown 处理器通过 getActiveTerminal() 安全访问
let activeTerminalRef: WeakRef<Terminal> | null = null;

/** 获取当前活跃终端（WeakRef 解引用，已 GC 则返回 null） */
function getActiveTerminal(): Terminal | null {
  return activeTerminalRef?.deref() ?? null;
}

/** 注册活跃终端 */
export function setActiveTerminal(term: Terminal | null): void {
  activeTerminalRef = term ? new WeakRef(term) : null;
}

/** 仅当指定终端仍是活跃终端时才清空（防 blur/focus 竞态：A blur → B focus 时 A 的 blur 不清 B） */
export function clearActiveTerminalIfMine(term: Terminal): void {
  if (getActiveTerminal() === term) {
    activeTerminalRef = null;
  }
}

/** 是否已安装全局监听器 */
let installed = false;

/** 引用计数：多个终端可同时存在，install/uninstall 配对调用，count 归 0 才真正移除 listener */
let listenerCount = 0;

/** 安装全局键盘处理器（capture 阶段，优先于 xterm.js 内置 handler） */
export function installKeyboardHandler(): void {
  listenerCount++;
  if (installed) return;
  installed = true;
  window.addEventListener("keydown", handleKeyDown, { capture: true });
}

/** 卸载全局键盘处理器（引用计数归零时移除 listener） */
export function uninstallKeyboardHandler(): void {
  if (listenerCount > 0) {
    listenerCount--;
  }
  if (listenerCount === 0 && installed) {
    window.removeEventListener("keydown", handleKeyDown, { capture: true });
    installed = false;
  }
}

function handleKeyDown(e: KeyboardEvent): void {
  // IME 合成中 → 全透传，不拦截
  if (e.isComposing) return;

  const term = getActiveTerminal();
  if (!term) return;

  // Ctrl+Shift+C → 复制终端选中文本到剪贴板
  if (e.ctrlKey && e.shiftKey && e.code === "KeyC") {
    const selection = term.getSelection();
    if (selection) {
      writeText(selection).catch((err) => {
        console.error("[slTerminal] 复制到剪贴板失败:", err);
      });
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
