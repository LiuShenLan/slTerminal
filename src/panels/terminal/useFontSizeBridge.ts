// useFontSizeBridge — 字体大小桥接 hook
//
// 职责：
// - 从 useFontSize store 订阅终端字体大小
// - Ctrl+鼠标滚轮（或 Meta+滚轮）调节字体大小
// - 在 container 上注册/注销 wheel 事件监听
//
// 提取自 useXterm.ts，与 useCodeMirror.ts 中字体大小逻辑保持一致。

import { useEffect, useRef } from "react";
import { useFontSize } from "../../stores";

/** 字体大小 clamp 范围 */
const CLAMP_MIN = 8;
const CLAMP_MAX = 32;

/**
 * 终端字体大小桥接 hook
 *
 * 直接订阅 useFontSize store，替代 props 传递 fontSize + onFontSizeChange 模式。
 * Ctrl+Wheel 处理器与 useCodeMirror.ts 中对应逻辑保持一致的实现策略：
 * - fontSizeRef 跟踪当前值，wheel handler 中读取避免闭包捕获过时值
 * - capture 阶段注册、passive:false 确保 preventDefault 生效
 *
 * @param container - 终端容器 DOM 元素，wheel 事件挂载目标
 * @param onChange  - 字体大小变更回调（Ctrl+Wheel 触发时调用，兼容旧 onFontSizeChange 接口）
 * @returns 当前终端字体大小，供 useXterm 消费
 */
export function useFontSizeBridge(
  container: HTMLElement | null,
  onChange?: (size: number) => void,
): { fontSize: number } {
  // 从 Zustand store 订阅终端字体大小及 setter
  const fontSize = useFontSize((s) => s.terminalFontSize);
  const setFontSize = useFontSize((s) => s.setTerminalFontSize);

  // 字体大小 ref：wheel handler 中读取，避免闭包捕获过时值
  // （与 useCodeMirror.ts 中 fontSizeRef 策略一致）
  const fontSizeRef = useRef(fontSize);
  fontSizeRef.current = fontSize;

  // 回调 ref：避免 wheel handler 闭包捕获过时的 onChange
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  // Ctrl+鼠标滚轮（或 Mac Cmd+滚轮）调节字体大小
  // 非修饰键滚轮透传给 xterm.js scrollback
  useEffect(() => {
    if (!container) return;

    const handleWheel = (e: WheelEvent) => {
      // 检测 Ctrl 或 Meta（Mac Cmd）修饰键
      if (!e.ctrlKey && !e.metaKey) return;

      e.preventDefault();
      e.stopPropagation();

      // 从 ref 读取当前字体大小，避免闭包过时
      const currentSize = fontSizeRef.current;
      // 上滚（deltaY < 0）放大，下滚（deltaY > 0）缩小
      const direction = e.deltaY < 0 ? 1 : -1;
      const newSize = Math.max(CLAMP_MIN, Math.min(CLAMP_MAX, currentSize + direction));

      if (newSize !== currentSize) {
        setFontSize(newSize);
        onChangeRef.current?.(newSize);
      }
    };

    // capture 阶段注册，passive:false 确保 preventDefault 生效
    container.addEventListener("wheel", handleWheel, { capture: true, passive: false });

    return () => {
      container.removeEventListener("wheel", handleWheel, { capture: true });
    };
  }, [container, setFontSize]);

  return { fontSize };
}
