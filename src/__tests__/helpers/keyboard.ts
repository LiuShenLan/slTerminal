// helpers/keyboard.ts — 共享键盘事件 helper（TQ-B-12）
//
// 统一 keydown 事件构造默认值与派发方式——shortcuts.test.ts / global-commands.test.ts /
// explorer-delete.test.tsx 复用，消除各文件各自 new KeyboardEvent 的默认字段不一致。

/** keydown 构造选项——缺省字段统一回退 false / "" / code */
export interface KeydownOptions {
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  code?: string;
  key?: string;
  isComposing?: boolean;
}

/** 构造 keydown 事件（不派发）——调用方需自行 dispatch 时用 */
export function makeKeydown(opts: KeydownOptions): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
    metaKey: opts.metaKey ?? false,
    code: opts.code ?? "",
    key: opts.key ?? opts.code ?? "",
    isComposing: opts.isComposing ?? false,
    bubbles: true,
    cancelable: true,
  });
}

/** 构造并派发到 window，返回事件 */
export function dispatchKeydown(opts: KeydownOptions): KeyboardEvent {
  const event = makeKeydown(opts);
  window.dispatchEvent(event);
  return event;
}
