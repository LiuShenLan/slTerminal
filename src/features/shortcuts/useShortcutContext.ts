// useShortcutContext.ts — 面板级快捷键注册 + 焦点上下文 React hook
//
// 职责：
// - 挂载时向 ShortcutRegistry 注册命令
// - element 上绑定 focusin/focusout → pushContext/popContext
// - 卸载时注销命令 + 解绑事件 + 弹出上下文
//
// 使用方：terminal/useXterm.ts（终端 Ctrl+Shift+C/V），未来 editor/useCodeMirror.ts 等

import { useEffect } from "react";
import type { ShortcutCommand, ShortcutContext } from "./types";
import { getShortcutRegistry } from "./ShortcutRegistry";

/**
 * 注册面板级快捷键并管理焦点上下文。
 *
 * @param context   上下文标识符（如 "terminal"、"editor"）
 * @param commands  该面板的快捷键命令列表（应稳定，用 useMemo 包裹）
 * @param element   接收焦点的 DOM 元素（通过 focusin/focusout 冒泡跟踪焦点）
 *                  为 null 时 hook 无操作
 */
export function useShortcutContext(
  context: ShortcutContext,
  commands: ShortcutCommand[],
  element: HTMLElement | null,
): void {
  useEffect(() => {
    if (!element || commands.length === 0) return;

    const registry = getShortcutRegistry();
    const unregister = registry.register(commands);

    const onFocusIn = () => registry.pushContext(context);
    const onFocusOut = (e: FocusEvent) => {
      // 仅当焦点离开 element 子树的才算 blur（内部焦点转移不触发）
      if (!e.relatedTarget || !element.contains(e.relatedTarget as Node)) {
        registry.popContext(context);
      }
    };

    element.addEventListener("focusin", onFocusIn);
    element.addEventListener("focusout", onFocusOut);

    return () => {
      unregister();
      element.removeEventListener("focusin", onFocusIn);
      element.removeEventListener("focusout", onFocusOut);
      // 清理后尝试弹出（仅在栈顶匹配时才真正弹出）
      registry.popContext(context);
    };
  }, [context, commands, element]);
}
