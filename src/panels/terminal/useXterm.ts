// useXterm — xterm.js 生命周期管理 hook
//
// 职责：
// - 创建 Terminal 实例（WebGL 优先 + DOM 兜底）
// - 管理 WebGL context（仅焦点终端持有，失焦 dispose 自动回退 DOM）
// - 挂载时 spawn PTY，卸载时 kill
// - 输出原子渲染（rAF 合帧）
// - StrictMode 防御（useRef guard clause）

import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { terminalOptions } from "./theme";
import { pty } from "../../ipc";
import type { PtyEvent } from "../../types";
import { setActiveTerminal, installKeyboardHandler } from "./keyboard";

export interface UseXtermOptions {
  /** 容器 DOM 元素 */
  container: HTMLElement | null;
  /** 终端列数 */
  cols: number;
  /** 终端行数 */
  rows: number;
  /** 面板 ID，用于关联 PTY 会话 */
  panelId: string;
  /** Windows 真实 build 号（F3 动态检测），用于 ConPTY reflow 阈值 */
  windowsBuildNumber?: number;
}

/** 检查终端是否可以安全执行 fit 操作（五条件守卫） */
export function canFit(
    terminal: Terminal | null | undefined,
    fitAddon: FitAddon | null | undefined,
    containerEl: HTMLElement | null | undefined,
    isDisposedRef: { current: boolean }
): boolean {
    if (!terminal || !fitAddon) return false;    // 条件1: 实例存在
    if (!containerEl) return false;               // 条件2: 容器存在
    if (containerEl.offsetWidth === 0 ||           // 条件3: 非 display:none
        containerEl.offsetHeight === 0) return false;
    if (!terminal.element) return false;           // 条件4: open() 已调用
    if (isDisposedRef.current) return false;       // 条件5: 未销毁
    return true;
}

export function useXterm({ container, cols, rows, panelId, windowsBuildNumber }: UseXtermOptions) {
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const webglAddonRef = useRef<WebglAddon | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const pendingBufferRef = useRef<string[]>([]);
  const rafIdRef = useRef<number | null>(null);
  const e2eTextBufferRef = useRef<string[]>([]);
  const isDisposedRef = useRef(false);

  /** 检测 WebGL2 是否可用 */
  const detectWebgl = useCallback((): boolean => {
    try {
      const canvas = document.createElement("canvas");
      const gl = canvas.getContext("webgl2", {
        failIfMajorPerformanceCaveat: true,
      });
      return gl !== null;
    } catch {
      return false;
    }
  }, []);

  /** 将缓冲数据合并写入终端 */
  const flushBuffer = useCallback(() => {
    rafIdRef.current = null;
    const term = terminalRef.current;
    if (!term) return;
    const data = pendingBufferRef.current.join("");
    pendingBufferRef.current = [];
    if (data) {
      term.write(data);
    }
  }, []);

  /** 处理 PTY 输出事件 */
  const handlePtyOutput = useCallback(
    (event: PtyEvent) => {
      if (event.type === "Output") {
        const text = new TextDecoder().decode(
          new Uint8Array(event.data.bytes),
        );
        // 累积到 E2E 文本缓冲
        e2eTextBufferRef.current.push(text);

        // 小数据立即写，大数据 rAF 合帧
        if (text.length < 256) {
          terminalRef.current?.write(text);
        } else {
          pendingBufferRef.current.push(text);
          if (rafIdRef.current === null) {
            rafIdRef.current = requestAnimationFrame(flushBuffer);
          }
        }
      } else if (event.type === "Exit") {
        terminalRef.current?.writeln(
          `\r\n[进程已退出，代码: ${event.data.code ?? "?"}]`,
        );
      }
    },
    [flushBuffer],
  );

  useEffect(() => {
    // StrictMode guard: 防止双重挂载
    if (!container || terminalRef.current) return;

    // 创建 Terminal
    const term = new Terminal({ ...terminalOptions, cols: 80, rows: 24 });
    terminalRef.current = term;

    // F3: 动态设置 ConPTY buildNumber（真实 OS build，覆盖 theme.ts 硬编码）
    if (windowsBuildNumber !== undefined) {
      term.options.windowsPty = {
        backend: "conpty",
        buildNumber: windowsBuildNumber,
      };
    }

    // FitAddon
    const fitAddon = new FitAddon();
    fitAddonRef.current = fitAddon;
    term.loadAddon(fitAddon);

    // WebGL addon（预检 → 通过则加载，不可用则 DOM 兜底）
    if (detectWebgl()) {
      try {
        const webglAddon = new WebglAddon();
        webglAddonRef.current = webglAddon;
        term.loadAddon(webglAddon);

        // 监听 context loss → dispose 自动回退 DOM 渲染器
        webglAddon.onContextLoss(() => {
          webglAddonRef.current?.dispose();
          webglAddonRef.current = null;
        });
      } catch {
        // WebGL 加载失败，DOM 兜底
        webglAddonRef.current = null;
      }
    }

    // 挂载到 DOM
    term.open(container);

    // 注册键盘处理器（Ctrl+Shift+C/V 复制粘贴）
    installKeyboardHandler();
    setActiveTerminal(term);

    // 字体预加载后刷新布局
    document.fonts.ready.then(() => {
      if (canFit(term, fitAddon, container, isDisposedRef)) {
        fitAddon.fit();
      }
    });

    // E2E 测试辅助：暴露 PTY 读写钩子（挂在 container DOM 上）
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const e2eHelper: any = container;
    e2eHelper.__e2e_sessionReady = false;
    e2eHelper.__e2e_writeToPty = (data: string) => {
      if (sessionIdRef.current) {
        pty.write(sessionIdRef.current, new TextEncoder().encode(data));
      }
    };
    // 文本缓冲读取（累积 PTY 输出 + 直接写入）
    e2eHelper.__e2e_getTerminalText = () =>
      e2eTextBufferRef.current.join("");
    // 直接写入终端（绕过 PTY，用于验证缓冲机制）
    e2eHelper.__e2e_writeToTerminal = (text: string) => {
      term.write(text);
      e2eTextBufferRef.current.push(text);
    };

    // 挂载时 spawn PTY
    pty
      .spawn({ panelId, cols, rows }, handlePtyOutput)
      .then((sessionId) => {
        sessionIdRef.current = sessionId;
        e2eHelper.__e2e_sessionReady = true;
      })
      .catch((err) => {
        term.writeln(`\r\n[PTY spawn 失败: ${err}]`);
        e2eHelper.__e2e_error = String(err);
      });

    // 终端输入 → 后端
    term.onData((data) => {
      if (sessionIdRef.current) {
        pty.write(
          sessionIdRef.current,
          new TextEncoder().encode(data),
        );
      }
    });

    return () => {
      // 清理
      isDisposedRef.current = true;
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
      }
      if (sessionIdRef.current) {
        pty.kill(sessionIdRef.current);
      }
      setActiveTerminal(null);
      webglAddonRef.current?.dispose();
      fitAddonRef.current?.dispose();
      term.dispose();
      terminalRef.current = null;
    };
  }, [container, panelId, cols, rows, windowsBuildNumber, detectWebgl, flushBuffer, handlePtyOutput]);

  // G2 Layer 2+4: ResizeObserver 独立注册——与 Terminal 创建解耦
  // 门控：首次同步 fit，后续 rAF 去重
  useEffect(() => {
    if (!container || !terminalRef.current || !fitAddonRef.current) return;

    const term = terminalRef.current;
    const fitAddon = fitAddonRef.current;
    let rafPending = false;
    let initialFitDone = false;

    const doFit = () => {
      if (!canFit(term, fitAddon, container, isDisposedRef)) return;
      try {
        fitAddon.fit();
        initialFitDone = true;
        const dims = fitAddon.proposeDimensions();
        if (dims && sessionIdRef.current) {
          pty.resize(sessionIdRef.current, dims.cols, dims.rows);
        }
      } catch {
        // fit 失败不影响渲染
      }
    };

    const resizeObserver = new ResizeObserver(() => {
      if (rafPending) return;
      rafPending = true;
      requestAnimationFrame(() => {
        rafPending = false;
        doFit();
      });
    });
    resizeObserver.observe(container);

    // 首次同步 fit（门控：容器就绪后立即 fit）
    if (!initialFitDone) {
      requestAnimationFrame(() => doFit());
    }

    // G2 Layer 5: setTimeout 回退——WebView2 ResizeObserver 可能触发 {0,0} 后静默
    const delays = [0, 50, 200];
    const timers: ReturnType<typeof setTimeout>[] = [];
    delays.forEach((delay) => {
      timers.push(setTimeout(() => doFit(), delay));
    });

    return () => {
      resizeObserver.disconnect();
      timers.forEach(clearTimeout);
    };
  }, [container]);

  // F3 Bug 1 修复: 独立 useEffect 监听 windowsBuildNumber 异步更新
  // 主 useEffect 的 StrictMode 守卫不拦截此 effect，确保 Terminal 创建后 windowsPty 生效
  useEffect(() => {
    const term = terminalRef.current;
    if (term && windowsBuildNumber !== undefined) {
      term.options.windowsPty = {
        backend: "conpty",
        buildNumber: windowsBuildNumber,
      };
    }
  }, [windowsBuildNumber]);

  return {
    /** 聚焦终端输入 */
    focus: useCallback(() => {
      terminalRef.current?.focus();
    }, []),
  };
}
