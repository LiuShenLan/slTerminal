// useXterm — PTY 通信 + 终端渲染协调 hook（编排层）
//
// 职责：
// - 编排各子 hook：Terminal 实例、PTY 输出、Resize、剪贴板、命令检测、字体
// - PTY spawn / write / kill（终端进程管理）
// - 快捷键委托 + OSC 8 超链接
// - visible 管理（WebGL 释放/重建 + 非焦点终端降频）
// - E2E 测试钩子
//
// 子 hook 分工：
// - useTerminalInstance.ts — Terminal 实例 + addon 生命周期
// - usePtyOutput.ts       — 输出合帧（Idle+Max 双定时器 + DEC 2026）
// - usePtyResize.ts       — ResizeObserver X/Y 分离 debounce
// - useClipboardHandler.ts — OSC 52 剪贴板拦截
// - useCommandDetection.ts — OSC 133 命令边界检测 + 页签标题
// - useFontSizeWheel.ts (src/lib/) — Ctrl+Wheel 共享 hook
// - webgl.ts              — WebGL 检测 + WebglAddon 加载

import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { terminalOptions } from "./theme";
import { useTerminalInstance } from "./useTerminalInstance";
import { usePtyOutput } from "./usePtyOutput";
import { usePtyResize } from "./usePtyResize";
import { useClipboardHandler } from "./useClipboardHandler";
import { useCommandDetection } from "./useCommandDetection";
export { detectWebgl, resetWebglCache } from "./webgl";
import { pty } from "../../ipc";
import { openUrl } from "../../ipc/shell";
import { setActiveTerminal, clearActiveTerminal, type TerminalActions } from "./activeTerminal";
import { usePanelFocus, getShortcutRegistry } from "../../features/shortcuts";
import { TerminalRegistry } from "./TerminalRegistry";
import { useFontSizeWheel } from "../../lib/useFontSizeWheel";
import { E2E_ENABLED } from "../../lib/e2eEnabled";
import { FONT_SIZE_MIN, FONT_SIZE_MAX } from "../../stores/fontSize";
// 命令行→标题/图标规则（side-effect import 注册规则）
import "./tabRules";
import type { TabState } from "./TabTitleRegistry";
import {
  installTerminalWriteToPty,
  setTerminalSessionReady,
  setTerminalSessionError,
} from "../../../e2e-tests/helpers";

export interface UseXtermOptions {
  /** 容器 DOM 元素 */
  container: HTMLElement | null;
  /** 终端列数（初始值，运行时由 fit 重算） */
  cols: number;
  /** 终端行数（初始值，运行时由 fit 重算） */
  rows: number;
  /** 面板 ID，用于关联 PTY 会话 */
  panelId: string;
  /** Windows 真实 build 号（动态检测），用于 ConPTY reflow 阈值 */
  windowsBuildNumber?: number;
  /** 终端工作目录（来自操作页面 cwd） */
  cwd?: string;
  /** 面板是否可见（页面切换时 CSS display:none → WebGL 释放） */
  visible?: boolean;
  /** 终端字体大小（运行时动态调节，默认 14） */
  fontSize?: number;
  /** 字体大小变更回调（Ctrl+Wheel 触发）——已由 useFontSizeWheel 内部处理，保留仅接口兼容 */
  onFontSizeChange?: (size: number) => void;
  /** 命令运行状态变更回调（OSC 133 检测到注册命令启动/退出时触发） */
  onTabStateChange?: (state: TabState) => void;
}

/** useXterm hook 返回类型 */
export interface UseXtermReturn {
  /** 聚焦终端输入 */
  focus: () => void;
  /** 测试专用接口（生产代码忽略） */
  _test: {
    cancelPendingFlush: () => void;
    flushBuffer: () => void;
    getPendingBuffer: () => Uint8Array[];
  };
}

/** fallback 终端尺寸 */
const DEFAULT_COLS = 80;
const DEFAULT_ROWS = 24;

/** 检查终端是否可以安全执行 fit 操作（五条件守卫） */
export function canFit(
    terminal: Terminal | null | undefined,
    fitAddon: FitAddon | null | undefined,
    containerEl: HTMLElement | null | undefined,
    isDisposedRef: { current: boolean } | null | undefined
): boolean {
    if (!isDisposedRef) return false;
    if (!terminal || !fitAddon) return false;
    if (!containerEl) return false;
    if (containerEl.offsetWidth === 0 ||
        containerEl.offsetHeight === 0) return false;
    if (!terminal.element) return false;
    if (isDisposedRef.current) return false;
    return true;
}

export function useXterm({
  container, panelId,
  windowsBuildNumber, cwd, visible, fontSize,
  onFontSizeChange, onTabStateChange,
}: UseXtermOptions): UseXtermReturn {

  // ═══════════════════════════════════════════════════════════════
  // 1. 字体大小（Ctrl+Wheel 共享 hook + store 订阅）
  //    fontSize prop 来自 TerminalPanel（store.terminalFontSize），fontSizeRef 跟踪当前值
  //    wheel handler 直接调 onFontSizeChange（即 store.setTerminalFontSize）
  // ═══════════════════════════════════════════════════════════════
  const fontSizeRef = useRef(fontSize ?? 14);
  fontSizeRef.current = fontSize ?? 14;
  useFontSizeWheel(container, FONT_SIZE_MIN, FONT_SIZE_MAX, fontSizeRef, (size) => {
    onFontSizeChange?.(size);
  });

  // ═══════════════════════════════════════════════════════════════
  // 2. Terminal 实例 + addon 生命周期（useTerminalInstance）
  // ═══════════════════════════════════════════════════════════════
  const {
    terminal: terminalRef,
    fitAddon: fitAddonRef,
    webglAddon: webglAddonRef,
    isDisposed: isDisposedRef,
    e2eTextBuffer: e2eTextBufferRef,
    tryLoadWebgl,
  } = useTerminalInstance(container, terminalOptions, fontSize);

  // ═══════════════════════════════════════════════════════════════
  // 3. 共享状态
  // ═══════════════════════════════════════════════════════════════

  /** Terminal 状态变量——当 Terminal 实例就绪时设置，触发下游 hook 的 useEffect 重新注册 */
  const [termState, setTermState] = useState<Terminal | null>(null);

  /** 最近一次 spawn 使用的尺寸（供 exit/retry 使用） */
  const lastColsRef = useRef(DEFAULT_COLS);
  const lastRowsRef = useRef(DEFAULT_ROWS);

  /** 共享的命令运行状态——useCommandDetection 写入，usePtyOutput exit 时读取 */
  const isCommandRunningRef = useRef(false);

  /** doSpawn 回调 ref（供 usePtyOutput 的 setupRetry / Enter 重连触发） */
  const doSpawnRef = useRef<((cols: number, rows: number) => void) | null>(null);

  // ═══════════════════════════════════════════════════════════════
  // 4. 终端快捷键上下文（active terminal + focus context）
  // ═══════════════════════════════════════════════════════════════
  const writeToPty = useCallback((data: Uint8Array) => {
    const sid = TerminalRegistry.get(panelId)?.sessionId;
    if (sid) {
      pty.write(sid, data).catch(() => {});
    }
  }, [panelId]);

  const terminalActions = useMemo<TerminalActions>(
    () => ({
      getSelection: () => terminalRef.current?.getSelection(),
      paste: (text: string) => terminalRef.current?.paste(text),
      writeToPty,
    }),
    [writeToPty],
  );
  const activateTerminal = useCallback(() => setActiveTerminal(terminalActions), [terminalActions]);
  const deactivateTerminal = useCallback(() => clearActiveTerminal(terminalActions), [terminalActions]);
  usePanelFocus("terminal", container, activateTerminal, deactivateTerminal);

  // ═══════════════════════════════════════════════════════════════
  // 5. 子 hook 调用
  // ═══════════════════════════════════════════════════════════════

  // PTY 输出处理（合帧缓冲 + Idle+Max 定时器 + 进程退出/重连）
  const { flushBuffer, cancelPendingFlush, handlePtyOutput, getPendingBuffer, setupRetry, retryDisposableRef } = usePtyOutput(
    terminalRef,
    panelId,
    visible ?? true,
    onTabStateChange,
    doSpawnRef,
    e2eTextBufferRef,
    isCommandRunningRef,
  );

  // 运行时 resize（ResizeObserver + X/Y 分离 debounce）
  const isReady = termState !== null && fitAddonRef.current !== null && !isDisposedRef.current;
  usePtyResize(
    container,
    termState,
    fitAddonRef.current,
    panelId,
    cancelPendingFlush,
    isReady,
    isDisposedRef,
  );

  // OSC 52 剪贴板拦截
  useClipboardHandler(termState, visible ?? true);

  // OSC 133 命令边界检测 → 页签标题/图标切换
  const { resetCommandState } = useCommandDetection(termState, onTabStateChange, isCommandRunningRef);

  // ═══════════════════════════════════════════════════════════════
  // 6. 主 useEffect：PTY spawn + onData + OSC 8 + 键盘委托 + E2E
  // ═══════════════════════════════════════════════════════════════
  useEffect(() => {
    if (!container) return;

    const term = terminalRef.current;
    const fitAddon = fitAddonRef.current;
    if (!term || !fitAddon) return;

    // 同步 termState 供下游 hook 使用
    setTermState(term);

    // F3: 动态设置 ConPTY buildNumber
    if (windowsBuildNumber !== undefined) {
      term.options.windowsPty = {
        backend: "conpty",
        buildNumber: windowsBuildNumber,
      };
    }

    // ── 键盘事件委托（xterm.js 内部 keydown 之前拦截）──
    term.attachCustomKeyEventHandler((event: KeyboardEvent) => {
      if (event.type !== "keydown") return true;
      const consumed = getShortcutRegistry().resolve(event, "terminal");
      if (consumed) {
        event.preventDefault();
        return false;
      }
      return true;
    });

    // OSC 8 超链接：点击后通过系统默认浏览器打开
    term.options.linkHandler = {
      activate: (_event, url) => {
        openUrl(url).catch(() => {});
      },
    };

    // E2E 测试辅助钩子——委托给 helpers.ts（E2E_ENABLED 时编译期保留，生产 tree-shake）
    // __e2e_writeToTerminal / __e2e_getTerminalText 由 useTerminalInstance 在 E2E_ENABLED 时安装
    if (E2E_ENABLED) {
      setTerminalSessionReady(container, false);
      installTerminalWriteToPty(container, (data: string) => {
        const sid = TerminalRegistry.get(panelId)?.sessionId;
        if (sid) {
          pty.write(sid, new TextEncoder().encode(data))
            .catch((err) => console.error("E2E PTY write 失败:", err));
        }
      });
    }

    // ── PTY spawn ──
    // rAF 轮询容器就绪 → fit → proposeDimensions → pty.spawn(真实尺寸)
    // 30 帧 / 500ms 上限，超时回退 80×24
    let fitRafId: number | null = null;
    let fitFrames = 0;
    const MAX_FRAMES = 30;
    const FIT_TIMEOUT = 500;
    const fitStartTime = performance.now();

    const doSpawn = (realCols: number, realRows: number) => {
      const cols = Number.isFinite(realCols) ? realCols : DEFAULT_COLS;
      const rows = Number.isFinite(realRows) ? realRows : DEFAULT_ROWS;
      if (!Number.isFinite(realCols) || !Number.isFinite(realRows)) {
        console.warn(
          `[H6] ⚠️ proposeDimensions 返回 NaN，回退 ${DEFAULT_COLS}x${DEFAULT_ROWS} (raw: cols=${realCols} rows=${realRows})`
        );
      }
      lastColsRef.current = cols;
      lastRowsRef.current = rows;

      pty
        .spawn({ panelId, cols, rows, cwd }, handlePtyOutput)
        .then((sessionId) => {
          // 注册到 TerminalRegistry（跨页面切换时可供 reattach 查询，约束 #8 单点元数据）
          TerminalRegistry.register(panelId, {
            term,
            sessionId,
            webglAddon: webglAddonRef.current,
            fitAddon,
          });
          if (E2E_ENABLED) setTerminalSessionReady(container, true);
          // PTY spawn 初始化：重置命令运行状态（覆盖持久化残留）
          resetCommandState();
        })
        .catch((err) => {
          term.writeln(`\r\n[重新连接] 按 Enter 重试...\r\n`);
          if (E2E_ENABLED) setTerminalSessionError(container, String(err));
          console.error(`[H6] ❌ spawn FAIL panelId="${panelId}"`, err);
          // 设置 Enter 重连监听（不立即重新 spawn，由用户按 Enter 触发）
          setupRetry(cols, rows);
        });
    };

    // 暴露 doSpawn 供 usePtyOutput 的 setupRetry 通过 doSpawnRef 触发
    doSpawnRef.current = doSpawn;

    const pollFitAndSpawn = () => {
      fitFrames++;
      const elapsed = performance.now() - fitStartTime;

      if (container.offsetWidth > 0 && container.offsetHeight > 0) {
        if (canFit(term, fitAddon, container, isDisposedRef)) {
          try {
            fitAddon.fit();
            const dims = fitAddon.proposeDimensions();
            if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
              doSpawn(dims.cols, dims.rows);
              return;
            }
          } catch {
            // fit 失败 → 回退
          }
        }
        doSpawn(DEFAULT_COLS, DEFAULT_ROWS);
        return;
      }

      if (fitFrames >= MAX_FRAMES || elapsed >= FIT_TIMEOUT) {
        doSpawn(DEFAULT_COLS, DEFAULT_ROWS);
        return;
      }

      fitRafId = requestAnimationFrame(pollFitAndSpawn);
    };

    fitRafId = requestAnimationFrame(pollFitAndSpawn);

    // 终端输入 → 后端
    term.onData((data) => {
      const sid = TerminalRegistry.get(panelId)?.sessionId;
      if (sid) {
        pty.write(
          sid,
          new TextEncoder().encode(data),
        ).catch((err) => console.error("PTY write 失败:", err));
      }
    });

    // ── 清理 ──
    return () => {
      isDisposedRef.current = true;
      if (fitRafId !== null) {
        cancelAnimationFrame(fitRafId);
      }

      // FE-08: 先清理 retry disposable（Terminal dispose 前的显式路径）
      retryDisposableRef.current?.dispose();
      retryDisposableRef.current = null;

      const entry = TerminalRegistry.get(panelId);
      if (entry) {
        pty.kill(entry.sessionId).catch(() => {});
      }
      TerminalRegistry.remove(panelId);
      doSpawnRef.current = null;
      // Terminal/addon 清理由 useTerminalInstance 的 useEffect cleanup 处理
    };
  }, [container, panelId, flushBuffer, cancelPendingFlush, handlePtyOutput, resetCommandState]);

  // F3 Bug 1 修复: 独立 useEffect 监听 windowsBuildNumber 异步更新
  useEffect(() => {
    const term = terminalRef.current;
    if (term && windowsBuildNumber !== undefined) {
      term.options.windowsPty = {
        backend: "conpty",
        buildNumber: windowsBuildNumber,
      };
    }
  }, [windowsBuildNumber]);

  // P1-13: WebGL 按可见性管理
  useEffect(() => {
    const term = terminalRef.current;
    if (!term) return;

    if (visible === false) {
      if (webglAddonRef.current) {
        webglAddonRef.current.dispose();
        webglAddonRef.current = null;
      }
    } else if (visible === true) {
      tryLoadWebgl();
      // 切回可见时 flush 非焦点期间累积的 PTY 输出
      flushBuffer();
    }
  }, [visible, flushBuffer, tryLoadWebgl]);

  // 字体大小变化 → fit + PTY resize
  useEffect(() => {
    const term = terminalRef.current;
    if (!term || fontSize === undefined) return;

    if (!canFit(term, fitAddonRef.current, container, isDisposedRef)) return;

    try {
      fitAddonRef.current!.fit();
      const dims = fitAddonRef.current!.proposeDimensions();
      const sid = TerminalRegistry.get(panelId)?.sessionId;
      if (dims && sid) {
        pty.resize(sid, dims.cols, dims.rows)
          .catch((err) => console.error("PTY resize 失败:", err));
      }
    } catch {
      // fit 失败不影响渲染
    }
  }, [fontSize, container]);

  // ═══════════════════════════════════════════════════════════════
  // 7. 返回对外接口
  // ═══════════════════════════════════════════════════════════════
  return {
    focus: useCallback(() => {
      terminalRef.current?.focus();
    }, []),
    _test: {
      cancelPendingFlush,
      flushBuffer,
      getPendingBuffer,
    },
  };
}
