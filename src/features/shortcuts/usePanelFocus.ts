// usePanelFocus.ts — 面板焦点上下文 + 聚焦实例跟踪 React hook
//
// 职责：
// - element focusin → pushContext(context) + onActivate（设置本实例为聚焦实例）
// - element focusout（离开子树）→ popContext(context) + onDeactivate（清除本实例）
// - 卸载时同样 popContext + onDeactivate
//
// 与旧 useShortcutContext 的区别：不再每实例注册命令（命令在 App 一次性注册），
// 只跟踪焦点上下文并把"当前聚焦实例"经 onActivate/onDeactivate 交给面板设置模块级 active 指针。
// 命令 handler 经该 active 指针派发到聚焦实例，从根本上解决多实例共享命令 id 的派发错误。

import { useEffect, useRef } from "react";
import type { ShortcutContext } from "./types";
import { getShortcutRegistry } from "./ShortcutRegistry";

/**
 * 跟踪面板焦点上下文与聚焦实例。
 *
 * @param context      上下文标识符（如 "terminal"、"editor"）
 * @param element      接收焦点的 DOM 元素（通过 focusin/focusout 冒泡跟踪焦点），为 null 时无操作
 * @param onActivate   focusin 时调用（设置本实例为聚焦实例）
 * @param onDeactivate focusout / 卸载时调用（清除本实例，内部应仅在仍为 active 时清）
 */
export function usePanelFocus(
  context: ShortcutContext,
  element: HTMLElement | null,
  onActivate?: () => void,
  onDeactivate?: () => void,
): void {
  // 用 ref 持有回调，避免回调引用变化导致 effect 重绑（deps 仅 [context, element]）
  const activateRef = useRef(onActivate);
  activateRef.current = onActivate;
  const deactivateRef = useRef(onDeactivate);
  deactivateRef.current = onDeactivate;

  useEffect(() => {
    if (!element) return;

    const registry = getShortcutRegistry();

    const onFocusIn = () => {
      registry.pushContext(context);
      activateRef.current?.();
    };
    const onFocusOut = (e: FocusEvent) => {
      // 仅当焦点离开 element 子树才算 blur（内部焦点转移不触发）
      if (!e.relatedTarget || !element.contains(e.relatedTarget as Node)) {
        registry.popContext(context);
        deactivateRef.current?.();
      }
    };

    element.addEventListener("focusin", onFocusIn);
    element.addEventListener("focusout", onFocusOut);

    return () => {
      element.removeEventListener("focusin", onFocusIn);
      element.removeEventListener("focusout", onFocusOut);
      registry.popContext(context);
      deactivateRef.current?.();
    };
  }, [context, element]);
}
