// useTerminalInstance.ts — xterm.js Terminal 实例生命周期管理 hook
//
// 职责（从 useXterm.ts 提取）：
// - 创建 Terminal 实例（含 WebGL + FitAddon）
// - WebGL 检测与 WebglAddon 加载（context loss 后指数退避重建）
// - StrictMode 双挂载守卫（smGuardRef）
// - 字体大小动态更新（term.options.fontSize）
// - Terminal 实例清理链（dispose addon + 取消 WebGL 重试定时器）
// - E2E 测试辅助钩子（仅 E2E_ENABLED：dev serve 或 VITE_E2E=1 构建）
//
// 与 useXterm 的职责分离：
// - useTerminalInstance（本 hook）：Terminal 实例 + addon 管理
// - useXterm：PTY 通信 + 输出合帧 + OSC handler + 快捷键 + ResizeObserver

import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import type { ITerminalOptions } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";
import { detectWebgl, setupWebglWithRetry } from "./webgl";
import { initTerminalE2e } from "../../../e2e-tests/helpers";
import { E2E_ENABLED } from "../../lib/e2eEnabled";

// ---- 常量 ----

/** 回退终端尺寸 */
const DEFAULT_COLS = 80;
const DEFAULT_ROWS = 24;
/** 回退字体大小 */
const DEFAULT_FONT_SIZE = 14;
/** E2E 测试文本缓冲行数上限，防止内存无限增长 */
export const E2E_BUFFER_MAX_LINES = 1000;

// ---- Hook 返回类型 ----

export interface UseTerminalInstanceReturn {
  /** Terminal 实例 ref */
  terminal: React.MutableRefObject<Terminal | null>;
  /** FitAddon 实例 ref */
  fitAddon: React.MutableRefObject<FitAddon | null>;
  /** WebglAddon 实例 ref（WebGL 不可用时为 null） */
  webglAddon: React.MutableRefObject<WebglAddon | null>;
  /** Terminal 是否已创建并挂载到容器 */
  isReady: React.MutableRefObject<boolean>;
  /** 是否已销毁 */
  isDisposed: React.MutableRefObject<boolean>;
  /** E2E 文本缓冲 ref（供 useXterm 追加 PTY 输出） */
  e2eTextBuffer: React.MutableRefObject<string[]>;
  /** 销毁 Terminal 实例及其 addon（同步，幂等） */
  dispose: () => void;
  /** 尝试加载 WebGL addon（可见性恢复时调用，已加载或不可用时无操作） */
  tryLoadWebgl: () => void;
}

// ---- Hook 实现 ----

/**
 * 管理 xterm.js Terminal 实例的完整生命周期：
 * 创建 → 加载 addon → 挂载 DOM → 字体更新 → 销毁。
 *
 * @param container  - 终端挂载的 DOM 容器（null 时不初始化）
 * @param options    - xterm.js ITerminalOptions（主题、字体、光标等）
 * @param fontSize   - 运行时字体大小（变化时自动更新 term.options.fontSize）
 * @returns Terminal 相关 ref 和 dispose 函数
 */
export function useTerminalInstance(
  container: HTMLElement | null,
  options: ITerminalOptions,
  fontSize?: number,
): UseTerminalInstanceReturn {
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const webglAddonRef = useRef<WebglAddon | null>(null);
  /** StrictMode 双挂载守卫：同组件实例仅初始化一次 */
  const smGuardRef = useRef(false);
  const isDisposedRef = useRef(false);
  const isReadyRef = useRef(false);
  const e2eTextBufferRef = useRef<string[]>([]);
  /** WebGL 加载取消函数（cleanup 中调用，停止重试并释放 addon） */
  const webglCancelRef = useRef<(() => void) | null>(null);

  // 记录上次 fontSize 值，避免相同值触发 effect
  const prevFontSizeRef = useRef<number | undefined>(undefined);

  // 主生命周期：创建 Terminal + addon + 挂载 DOM
  useEffect(() => {
    if (!container) return;

    // StrictMode guard：防止双重挂载
    if (smGuardRef.current) return;
    smGuardRef.current = true;

    // 重置销毁标记
    isDisposedRef.current = false;

    // 创建 Terminal 实例（xterm.js 不支持二次 open()，每次挂载均新建）
    const term = new Terminal({
      ...options,
      cols: DEFAULT_COLS,
      rows: DEFAULT_ROWS,
      fontSize: fontSize ?? DEFAULT_FONT_SIZE,
    });
    terminalRef.current = term;

    // FitAddon
    const fitAddon = new FitAddon();
    fitAddonRef.current = fitAddon;
    term.loadAddon(fitAddon);

    // WebGL addon（context loss 后指数退避重试，非永久退化）
    // onSuccess/onFail 回调更新 webglAddonRef，cancel 函数存入 webglCancelRef 供 cleanup 调用
    const { cancel } = setupWebglWithRetry(
      term,
      (addon) => { webglAddonRef.current = addon; },
      () => { webglAddonRef.current = null; },
    );
    webglCancelRef.current = cancel;

    // 挂载到 DOM
    term.open(container);

    // 字体预加载后刷新布局（包裹 rAF 确保布局稳定）
    document.fonts.ready.then(() => {
      requestAnimationFrame(() => {
        if (isDisposedRef.current) return;
        // 容器尺寸为 0（display:none）时跳过，防止无效 fit
        if (!container || container.offsetWidth === 0 || container.offsetHeight === 0) return;
        if (!term.element) return;
        try {
          fitAddon.fit();
        } catch {
          // fit 失败不影响渲染
        }
      });
    });

    // 标记就绪
    isReadyRef.current = true;

    // E2E 测试辅助钩子（E2E_ENABLED：dev serve 或 VITE_E2E=1 构建）
    if (E2E_ENABLED) {
      initTerminalE2e(container, {
        writeToTerminal: (text: string) => {
          if (isDisposedRef.current) return;
          term.write(text);
          e2eTextBufferRef.current.push(text);
          // 防止内存无限增长
          if (e2eTextBufferRef.current.length > E2E_BUFFER_MAX_LINES) {
            e2eTextBufferRef.current.splice(
              0,
              e2eTextBufferRef.current.length - E2E_BUFFER_MAX_LINES,
            );
          }
        },
        getTerminalText: () => e2eTextBufferRef.current.join(""),
      });
    }

    return () => {
      // ── 清理链 ——
      isDisposedRef.current = true;
      isReadyRef.current = false;

      // 取消 WebGL 重试 + dispose addon（webgl.ts 的 cancel 负责清理）
      webglCancelRef.current?.();
      webglCancelRef.current = null;
      webglAddonRef.current = null;

      // dispose addon 和 Terminal
      fitAddonRef.current?.dispose();
      fitAddonRef.current = null;
      terminalRef.current?.dispose();
      terminalRef.current = null;

      // 清空 E2E 文本缓冲
      e2eTextBufferRef.current.length = 0;

      // 注意：不重置 smGuardRef——与 useXterm 原行为一致，StrictMode 双重挂载时
      // smGuardRef 保持 true 防止第二次 mount 重建 Terminal 实例
    };
  }, [container]);
  // options 和 fontSize 变化不重建 Terminal——字体由独立 effect 处理，
  // options 在挂载时确定，运行时不变

  // 字体大小动态调节：当 fontSize 变化时更新 Terminal.options.fontSize
  useEffect(() => {
    const term = terminalRef.current;
    if (!term || fontSize === undefined) return;
    // 避免相同值触发无意义写入
    if (prevFontSizeRef.current === fontSize) return;
    prevFontSizeRef.current = fontSize;
    term.options.fontSize = fontSize;
  }, [fontSize]);

  /** 尝试加载 WebGL addon（可见性恢复时调用，已加载或不可用时无操作） */
  const tryLoadWebgl = useCallback(() => {
    const term = terminalRef.current;
    if (!term || webglAddonRef.current || !detectWebgl()) return;
    const { cancel } = setupWebglWithRetry(
      term,
      (addon) => { webglAddonRef.current = addon; },
      () => { webglAddonRef.current = null; },
    );
    webglCancelRef.current = cancel;
  }, []);

  // dispose 回调（供外部主动销毁，幂等）
  const dispose = useCallback(() => {
    if (isDisposedRef.current) return;
    isDisposedRef.current = true;
    isReadyRef.current = false;

    // 取消 WebGL 重试 + dispose addon（webgl.ts 的 cancel 负责清理）
    webglCancelRef.current?.();
    webglCancelRef.current = null;
    webglAddonRef.current = null;
    fitAddonRef.current?.dispose();
    fitAddonRef.current = null;
    terminalRef.current?.dispose();
    terminalRef.current = null;

    e2eTextBufferRef.current.length = 0;
    // 注意：不重置 smGuardRef，与 useEffect cleanup 行为一致
  }, []);

  return {
    terminal: terminalRef,
    fitAddon: fitAddonRef,
    webglAddon: webglAddonRef,
    isReady: isReadyRef,
    isDisposed: isDisposedRef,
    e2eTextBuffer: e2eTextBufferRef,
    dispose,
    tryLoadWebgl,
  };
}
