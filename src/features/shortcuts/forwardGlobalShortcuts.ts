// forwardGlobalShortcuts.ts — iframe 键盘桥：把全局快捷键从子文档转发到父 window
//
// HTML 面板内容在 <iframe sandbox="allow-scripts allow-same-origin" srcDoc> 中，iframe 内 keydown
// 不冒泡到父 window，ShortcutRegistry 的 window-capture 监听器收不到 → 焦点在 iframe 内时全局键失效。
//
// 因 allow-same-origin 与父同源，父窗可直接给 iframe.contentDocument 挂监听（handler 运行在父上下文）。
// 命中"全局 context 绑定"（动态取自 exportContextBindings("global")）→ preventDefault + 合成 keydown
// 重放到父 window，由 ShortcutRegistry 正常分发（global.closeTab → 关活跃面板，即该 HTML 面板）。
// 仅重放命中全局绑定的键，页面自身其它按键不受影响。

import { formatKeystroke } from "./keystroke";
import { getShortcutRegistry } from "./ShortcutRegistry";

/**
 * 给一个文档（iframe.contentDocument）挂 capture-phase keydown 监听，
 * 将命中全局绑定的按键 preventDefault 并重放到父 window。
 * @returns detach 函数，移除监听
 */
export function attachGlobalShortcutForwarder(doc: Document): () => void {
  const handler = (e: KeyboardEvent) => {
    const fingerprint = formatKeystroke({
      ctrlKey: e.ctrlKey,
      shiftKey: e.shiftKey,
      altKey: e.altKey,
      metaKey: e.metaKey,
      code: e.code,
    });
    const globalBindings = getShortcutRegistry().exportContextBindings("global");
    if (globalBindings.some((b) => b.keystroke === fingerprint)) {
      e.preventDefault();
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          ctrlKey: e.ctrlKey,
          shiftKey: e.shiftKey,
          altKey: e.altKey,
          metaKey: e.metaKey,
          code: e.code,
          key: e.key,
          bubbles: true,
          cancelable: true,
        }),
      );
    }
  };
  doc.addEventListener("keydown", handler, true);
  return () => doc.removeEventListener("keydown", handler, true);
}
