// useFontSizeWheel.ts — Ctrl+滚轮调节字体大小共享 hook
//
// 提取 useXterm/useCodeMirror 中 Ctrl+Wheel（含 Mac Cmd+Wheel）的共同逻辑。
// 在 container 上注册 capture 阶段 wheel 监听，修饰键触发字体大小步进，
// 非修饰键滚轮透传（终端 scrollback / 编辑器正常滚动）。
//
// fontSizeRef 避免 wheel handler 闭包捕获过时值，
// setFontSize ref 避免 setter 变化导致 effect 重新注册。

import { useEffect, useRef } from "react";

/**
 * Ctrl+滚轮调节字体大小
 *
 * @param container   - 挂载 wheel 监听的 DOM 元素
 * @param min         - 字体大小下限
 * @param max         - 字体大小上限
 * @param fontSizeRef - 当前字体大小 ref（handler 中读取，避免闭包过时）
 * @param setFontSize - 字体大小变更回调（调用方决定写入 store 还是 props 回调）
 */
export function useFontSizeWheel(
  container: HTMLElement | null,
  min: number,
  max: number,
  fontSizeRef: React.MutableRefObject<number>,
  setFontSize: (size: number) => void,
): void {
  // ref 跟踪回调，避免 setter 变化导致 effect 重新注册
  const setterRef = useRef(setFontSize);
  setterRef.current = setFontSize;

  useEffect(() => {
    if (!container) return;

    const handleWheel = (e: WheelEvent) => {
      // 检测 Ctrl（Windows/Linux）或 Meta（Mac Cmd）修饰键
      if (!e.ctrlKey && !e.metaKey) return;

      e.preventDefault();
      e.stopPropagation();

      // 从 ref 读取当前字体大小，避免闭包过时
      const currentSize = fontSizeRef.current;
      // 上滚（deltaY < 0）放大，下滚（deltaY > 0）缩小
      const direction = e.deltaY < 0 ? 1 : -1;
      const newSize = Math.max(min, Math.min(max, currentSize + direction));

      if (newSize !== currentSize) {
        setterRef.current(newSize);
      }
    };

    // capture 阶段注册，passive:false 确保 preventDefault 生效
    container.addEventListener("wheel", handleWheel, { capture: true, passive: false });

    return () => {
      container.removeEventListener("wheel", handleWheel, { capture: true });
    };
  }, [container, min, max]);
}
