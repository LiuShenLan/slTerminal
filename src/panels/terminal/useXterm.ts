// useXterm — xterm.js 生命周期管理 hook
//
// 职责：
// - 创建 Terminal 实例（WebGL 优先 + DOM 兜底）
// - 管理 WebGL context（仅焦点终端持有，失焦 dispose 自动回退 DOM）
// - 挂载时 spawn PTY，卸载时 kill
// - 输出原子渲染（Idle+Max 双定时器合帧 + DEC 2026 同步更新）
// - StrictMode 防御（useRef guard clause）

import { useEffect, useRef, useCallback, useMemo } from "react";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { terminalOptions } from "./theme";
import { pty } from "../../ipc";
import { writeText } from "../../ipc/clipboard";
import type { PtyEvent } from "../../types";
import { setActiveTerminal, clearActiveTerminal, type TerminalActions } from "./activeTerminal";
import { usePanelFocus, getShortcutRegistry } from "../../features/shortcuts";
import { TerminalRegistry } from "./TerminalRegistry";
// 命令行→标题/图标规则（side-effect import 注册规则）
import "./tabRules";
import { tabTitleRegistry } from "./TabTitleRegistry";
import type { TabState } from "./TabTitleRegistry";

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
  /** 终端工作目录（来自操作页面 cwd） */
  cwd?: string;
  /** 面板是否可见（页面切换时 CSS display:none → WebGL 释放） */
  visible?: boolean;
  /** 终端字体大小（运行时动态调节，默认 14） */
  fontSize?: number;
  /** 字体大小变更回调（Ctrl+Wheel 触发） */
  onFontSizeChange?: (size: number) => void;
  /** 命令运行状态变更回调（OSC 133 检测到注册命令启动/退出时触发） */
  onTabStateChange?: (state: TabState) => void;
}

// P2-44: 模块级 WebGL 检测缓存，避免每次 mount 创建临时 canvas
let webglCache: boolean | null = null;

/** 检测 WebGL2 是否可用（首次调用检测并缓存，后续直接返回缓存结果） */
export function detectWebgl(): boolean {
  if (webglCache !== null) return webglCache;
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2", {
      failIfMajorPerformanceCaveat: true,
    });
    webglCache = gl !== null;
  } catch {
    webglCache = false;
  }
  return webglCache;
}

/** 重置 WebGL 检测缓存（仅测试使用，生产环境无需调用） */
export function resetWebglCache(): void {
  webglCache = null;
}

// P2-43: WebGL context loss 后延迟重建（指数退避，最多 5 次）
// 当前仅 dispose 后永久退化到 DOM 渲染器，改为延迟重建
const WEBGL_RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000]; // ms
const WEBGL_RETRY_MAX = WEBGL_RETRY_DELAYS.length;

function setupWebglWithRetry(
  term: Terminal,
  webglAddonRef: { current: WebglAddon | null },
  attempt: number = 0,
): void {
  if (!detectWebgl()) return; // WebGL 不可用，永久 DOM 兜底

  try {
    const webglAddon = new WebglAddon();
    webglAddonRef.current = webglAddon;
    term.loadAddon(webglAddon);

    webglAddon.onContextLoss(() => {
      webglAddonRef.current?.dispose();
      webglAddonRef.current = null;
      // 指数退避重试
      if (attempt < WEBGL_RETRY_MAX) {
        const delay = WEBGL_RETRY_DELAYS[attempt];
        console.warn(
          `[WebGL] context loss，${delay}ms 后第 ${attempt + 1}/${WEBGL_RETRY_MAX} 次重建...`,
        );
        setTimeout(() => {
          setupWebglWithRetry(term, webglAddonRef, attempt + 1);
        }, delay);
      } else {
        console.warn(
          `[WebGL] 已达最大重试次数 (${WEBGL_RETRY_MAX})，回退 DOM 渲染器`,
        );
      }
    });
  } catch {
    webglAddonRef.current = null;
    // 加载失败也重试（context 可能处于过渡状态）
    if (attempt < WEBGL_RETRY_MAX) {
      const delay = WEBGL_RETRY_DELAYS[attempt];
      setTimeout(() => {
        setupWebglWithRetry(term, webglAddonRef, attempt + 1);
      }, delay);
    }
  }
}

/** 检查终端是否可以安全执行 fit 操作（五条件守卫） */
export function canFit(
    terminal: Terminal | null | undefined,
    fitAddon: FitAddon | null | undefined,
    containerEl: HTMLElement | null | undefined,
    isDisposedRef: { current: boolean } | null | undefined
): boolean {
    if (!isDisposedRef) return false;              // 条件0: ref 存在
    if (!terminal || !fitAddon) return false;    // 条件1: 实例存在
    if (!containerEl) return false;               // 条件2: 容器存在
    if (containerEl.offsetWidth === 0 ||           // 条件3: 非 display:none
        containerEl.offsetHeight === 0) return false;
    if (!terminal.element) return false;           // 条件4: open() 已调用
    if (isDisposedRef.current) return false;       // 条件5: 未销毁
    return true;
}

export function useXterm({ container, cols, rows, panelId, windowsBuildNumber, cwd, visible, fontSize, onFontSizeChange, onTabStateChange }: UseXtermOptions) {
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const webglAddonRef = useRef<WebglAddon | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  // 输出合帧缓冲区（Uint8Array 替代字符串拼接，减少 GC 压力）
  const pendingBufferRef = useRef<Uint8Array[]>([]);
  // 缓冲区字节计数（限制 64KB 上限，防止非焦点终端内存无限增长）
  const pendingBufSizeRef = useRef(0);
  // Idle+Max 双定时器：适应 Ink 高频小块 burst 输出模式
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const maxTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // visible 快照 ref：避免 handlePtyOutput 依赖 visible 导致 PTY 回调重建
  const visibleRef = useRef(visible);
  visibleRef.current = visible;
  const e2eTextBufferRef = useRef<string[]>([]);
  const isDisposedRef = useRef(false);
  /** P2-02: 复用 TextDecoder 实例，避免每次 PTY 输出都 new */
  const decoderRef = useRef(new TextDecoder());
  /** P1-01/02: 记录最近一次 spawn 尺寸，供重试使用 */
  const lastColsRef = useRef<number>(80);
  const lastRowsRef = useRef<number>(24);
  /** P1-01/02: Enter 重试 disposable，确保重试前清理旧监听 */
  const retryDisposableRef = useRef<{ dispose: () => void } | null>(null);
  /** P1-01/02: 跨闭包共享 doSpawn / setupRetry（handlePtyOutput 通过 ref 触发重连） */
  const doSpawnRef = useRef<((cols: number, rows: number) => void) | null>(null);
  const setupRetryRef = useRef<((cols: number, rows: number) => void) | null>(null);
  /** resize X/Y 分离 debounce：跟踪上次尺寸，区分行/列变化 */
  const prevDimsRef = useRef<{ cols: number; rows: number } | null>(null);
  /** 当前是否有注册的命令在运行（如 claude） */
  const isCommandRunningRef = useRef(false);
  /** onTabStateChange 回调 ref（避免 handlePtyOutput 依赖回调导致重建） */
  const onTabStateChangeRef = useRef(onTabStateChange);
  onTabStateChangeRef.current = onTabStateChange;

  // 向 PTY 写原始字节（稳定引用，供 Ctrl+Enter 换行命令用）
  const writeToPty = useCallback((data: Uint8Array) => {
    if (sessionIdRef.current) {
      pty.write(sessionIdRef.current, data).catch(() => {});
    }
  }, []);
  // 本终端对外暴露的动作——聚焦时设为 active，快捷键命令经 getActiveTerminal() 派发到此
  // （通过 ref 获取 terminal，避免闭包捕获已销毁实例）
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
  // 焦点上下文 + 聚焦实例跟踪（命令在 App 一次性注册，此处只 push/pop context + set/clear active）
  usePanelFocus("terminal", container, activateTerminal, deactivateTerminal);

  // Idle+Max 定时器常量
  const IDLE_FLUSH_MS = 2;
  const MAX_FLUSH_MS = 16;
  const MAX_PENDING_BYTES = 65536; // 64KB 上限，防止非焦点终端内存无限增长

  /** 取消定时器 + 丢弃待 flush 数据（不渲染）——用于 resize 场景 */
  const cancelPendingFlush = useCallback(() => {
    if (idleTimerRef.current !== null) {
      clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
    if (maxTimerRef.current !== null) {
      clearTimeout(maxTimerRef.current);
      maxTimerRef.current = null;
    }
    pendingBufferRef.current = [];
    pendingBufSizeRef.current = 0;
  }, []);

  /** 将缓冲数据合并写入终端（DEC 2026 同步更新包裹，消除撕裂） */
  const flushBuffer = useCallback(() => {
    // 清除定时器
    if (idleTimerRef.current !== null) {
      clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
    if (maxTimerRef.current !== null) {
      clearTimeout(maxTimerRef.current);
      maxTimerRef.current = null;
    }

    const term = terminalRef.current;
    const chunks = pendingBufferRef.current;
    pendingBufferRef.current = [];
    pendingBufSizeRef.current = 0;
    if (!term || chunks.length === 0) return;

    // 合并 Uint8Array 块（零字符串分配，减少 GC 压力）
    const totalLen = chunks.reduce((sum, c) => sum + c.length, 0);
    const merged = new Uint8Array(totalLen);
    let offset = 0;
    for (const chunk of chunks) {
      merged.set(chunk, offset);
      offset += chunk.length;
    }

    // DEC 2026 包裹为 Uint8Array
    const enc = new TextEncoder();
    const prefix = enc.encode("\x1b[?2026h");
    const suffix = enc.encode("\x1b[?2026l");
    const wrapped = new Uint8Array(prefix.length + merged.length + suffix.length);
    wrapped.set(prefix, 0);
    wrapped.set(merged, prefix.length);
    wrapped.set(suffix, prefix.length + merged.length);

    term.write(wrapped);
  }, []);

  /** 处理 PTY 输出事件 */
  const handlePtyOutput = useCallback(
    (event: PtyEvent) => {
      if (event.type === "output") {
        const rawBytes = new Uint8Array(event.data.bytes);
        const text = decoderRef.current.decode(rawBytes);
        e2eTextBufferRef.current.push(text);
        // P2-01: 每 1000 行截断，防止 E2E 文本缓冲无限增长
        if (e2eTextBufferRef.current.length > 1000) {
          e2eTextBufferRef.current.splice(0, e2eTextBufferRef.current.length - 1000);
        }

        // 直写阈值 64 字节：Ink 逐 token 输出通常 64-200 字节，
        // 降到 64 确保绝大多数 Ink 小块走合帧 + DEC 2026 路径
        if (text.length < 64) {
          terminalRef.current?.write(text);
        } else {
          // 内存上限：超出 64KB 丢弃最旧数据块（非焦点终端保护）
          if (pendingBufSizeRef.current + rawBytes.length > MAX_PENDING_BYTES) {
            while (pendingBufSizeRef.current > 0 && pendingBufSizeRef.current + rawBytes.length > MAX_PENDING_BYTES) {
              const dropped = pendingBufferRef.current.shift();
              if (dropped) pendingBufSizeRef.current -= dropped.length;
              else break;
            }
          }

          pendingBufferRef.current.push(rawBytes);
          pendingBufSizeRef.current += rawBytes.length;

          // 非焦点终端：仅累积不 flush，切回可见时统一回放
          const isVisible = visibleRef.current !== false;
          if (isVisible) {
            // Idle 定时器：2ms 内无新数据则 flush（适应 Ink burst 模式）
            if (idleTimerRef.current !== null) clearTimeout(idleTimerRef.current);
            idleTimerRef.current = setTimeout(flushBuffer, IDLE_FLUSH_MS);

            // Max 定时器：确保 16ms 内至少 flush 一次（防止饥饿）
            if (maxTimerRef.current === null) {
              maxTimerRef.current = setTimeout(() => {
                maxTimerRef.current = null;
                flushBuffer();
              }, MAX_FLUSH_MS);
            }
          }
        }
      } else if (event.type === "exit") {
        // P1-02: 进程退出后清除 sessionId，提示按 Enter 重新连接
        sessionIdRef.current = null;
        // 进程退出时若命令正在运行，重置页签状态
        if (isCommandRunningRef.current) {
          isCommandRunningRef.current = false;
          onTabStateChangeRef.current?.({ active: false });
        }
        terminalRef.current?.writeln(
          `\r\n进程已退出 (退出码: ${event.data.code ?? "?"})\r\n按 Enter 重新连接...\r\n`,
        );
        setupRetryRef.current?.(lastColsRef.current, lastRowsRef.current);
      }
    },
    [flushBuffer],
  );

  // 主 useEffect：Terminal 生命周期 + PTY spawn + ResizeObserver
  useEffect(() => {
    if (!container) return;

    // StrictMode guard：防止双重挂载
    if (terminalRef.current) {
      return;
    }

    // F1: 重置销毁标记
    isDisposedRef.current = false;

    // xterm.js 不支持 term.open() 二次调用（GitHub Issue #4978）
    // 每次挂载均创建新 Terminal 实例，Phase 3 由 ring buffer + pty_reattach 恢复内容
    const term = new Terminal({ ...terminalOptions, cols: 80, rows: 24, fontSize: fontSize ?? 14 });
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

    // WebGL addon（P2-43: context loss 后延迟重建，非永久退化）
    if (detectWebgl()) {
      setupWebglWithRetry(term, webglAddonRef);
    }

    // 挂载到 DOM
    term.open(container);

    // ── 键盘事件委托（在 xterm.js 内部 keydown 之前拦截）──
    // 双重保障：ShortcutRegistry 窗口级 capture 路径若因 xterm 6.1 focusin 未冒泡而失效，
    //   此 handler 委托进注册表、用 terminal context 解析同一绑定表（单一真值源，可重绑）。
    //   命中（如 Ctrl+Shift+C/V、Ctrl+Enter 换行）→ 已由命令 handler 处理，不交给 xterm.js；
    //   未命中（如 Ctrl+C）→ 透传 xterm.js，编码为控制字节发往 PTY。
    term.attachCustomKeyEventHandler((event: KeyboardEvent) => {
      // 仅处理 keydown，忽略 keyup/keypress（xterm.js 对每次 keydown 均调用此 handler）
      if (event.type !== "keydown") return true;
      const consumed = getShortcutRegistry().resolve(event, "terminal");
      if (consumed) {
        event.preventDefault();
        return false; // 已处理 → 不交给 xterm.js
      }
      return true; // 未命中 → 透传 xterm.js（Ctrl+C 等控制字符）
    });

    // OSC 8 超链接：点击后通过系统默认浏览器打开
    // linkHandler 是行为/副作用，非视觉配置，与 OSC 52 handler 同模式
    term.options.linkHandler = {
      activate: (_event, url) => {
        import("@tauri-apps/plugin-opener").then(({ openUrl }) =>
          openUrl(url).catch(() => {})
        );
      },
    };

    // 字体预加载后刷新布局（包裹 rAF 确保布局稳定）
    document.fonts.ready.then(() => {
      requestAnimationFrame(() => {
        if (canFit(term, fitAddon, container, isDisposedRef)) {
          fitAddon.fit();
        }
      });
    });

    // E2E 测试辅助：暴露 PTY 读写钩子（挂在 container DOM 上）
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const e2eHelper: any = container;
    e2eHelper.__e2e_sessionReady = false;
    e2eHelper.__e2e_writeToPty = (data: string) => {
      if (sessionIdRef.current) {
        pty.write(sessionIdRef.current, new TextEncoder().encode(data))
          .catch((err) => console.error("E2E PTY write 失败:", err));
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

    // OSC 52 剪贴板：拦截 Claude Code /copy 命令，写入系统剪贴板
    // 安全策略：仅写入、仅系统剪贴板(c)、仅可见面板、payload≤1MB、禁止读
    const osc52Disposable = term.parser.registerOscHandler(52, (data: string) => {
      const semicolonIdx = data.indexOf(";");
      if (semicolonIdx === -1) return true;

      const selector = data.substring(0, semicolonIdx);
      const payload = data.substring(semicolonIdx + 1);

      // 仅系统剪贴板（c），忽略 p（primary）和 q（secondary）
      if (selector && selector !== "c") return true;
      // 禁止读请求
      if (payload === "?" || payload.length === 0) return true;
      // Payload 上限 1MB
      if (payload.length > 1048576) return true;
      // 焦点门控：非可见面板忽略
      if (visibleRef.current === false) return true;

      try {
        // atob 返回二进制字符串（每字符一个字节），需经 UTF-8 解码
        const binary = atob(payload);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) {
          bytes[i] = binary.charCodeAt(i);
        }
        const text = new TextDecoder().decode(bytes);
        writeText(text).catch((e) => console.error("[OSC52] 写剪贴板失败:", e));
      } catch {
        // base64 解码失败，静默忽略
      }
      return true;
    });

    // OSC 133 命令边界检测：shell-integration.ps1 发射 OSC 133 C（命令启动）和
    // OSC 133;D（命令退出），xterm.js 解析器剥离 OSC number 前缀（133），
    // handler 收到的 data 为 "C;claude" 或 "D;0"
    const osc133Disposable = term.parser.registerOscHandler(133, (data: string) => {
      const semicolonIndex = data.indexOf(";");
      const type = semicolonIndex >= 0 ? data.slice(0, semicolonIndex) : data;

      if (type === "C") {
        // OSC 133 C — 命令即将执行
        const command = semicolonIndex >= 0 ? data.slice(semicolonIndex + 1).trim() : "";
        const rule = tabTitleRegistry.match(command);
        if (rule) {
          isCommandRunningRef.current = true;
          onTabStateChangeRef.current?.({ active: true, title: rule.title, icon: rule.icon });
        }
      } else if (type === "D" && isCommandRunningRef.current) {
        // OSC 133 ; D — 命令执行完毕
        isCommandRunningRef.current = false;
        onTabStateChangeRef.current?.({ active: false });
      }

      // 返回 false 不消费序列，xterm.js 仍渲染提示符
      return false;
    });

    // P1: rAF 轮询 offsetWidth>0 → fit → proposeDimensions → pty.spawn(真实尺寸)
    // 30 帧 / 500ms 上限，超时回退 80x24
    let fitRafId: number | null = null;
    let fitFrames = 0;
    const MAX_FRAMES = 30;
    const FIT_TIMEOUT = 500;
    const fitStartTime = performance.now();

    const doSpawn = (realCols: number, realRows: number) => {
      // NaN 守卫：缓存 Terminal 复用后，proposeDimensions 可能返回 NaN（字符尺寸未重算）
      const cols = Number.isFinite(realCols) ? realCols : 80;
      const rows = Number.isFinite(realRows) ? realRows : 24;
      if (!Number.isFinite(realCols) || !Number.isFinite(realRows)) {
        console.warn(`[H6] ⚠️ proposeDimensions 返回 NaN，回退 80x24 (raw: cols=${realCols} rows=${realRows})`);
      }
      // P1-01/02: 记录尺寸供重试使用
      lastColsRef.current = cols;
      lastRowsRef.current = rows;
      // P2-14 终端数量软上限：当前无限制，每个面板创建一个 ConPTY + reader 线程。
      // 若有需要，可在此处检查活跃 session 数量：若超阈值（如 16 个），弹窗提示用户先关闭闲置终端。
      // 阈值建议通过 settings 配置，不硬编码。计数器通过 TerminalRegistry.size 或后端 PtyState.sessions.len() 获取。
      pty
        .spawn({ panelId, cols, rows, cwd }, handlePtyOutput)
        .then((sessionId) => {
          sessionIdRef.current = sessionId;
          // E1: 注册到 TerminalRegistry（跨页面切换时可供 reattach 查询）
          TerminalRegistry.register(panelId, {
            term,
            sessionId,
            webglAddon: webglAddonRef.current,
            fitAddon,
          });
          e2eHelper.__e2e_sessionReady = true;
          // PTY spawn 初始化：重置命令运行状态（覆盖持久化残留）
          onTabStateChangeRef.current?.({ active: false });
        })
        .catch((err) => {
          // P1-01: spawn 失败 → 提示按 Enter 重试
          term.writeln(`\r\n[重新连接] 按 Enter 重试...\r\n`);
          e2eHelper.__e2e_error = String(err);
          console.error(`[H6] ❌ spawn FAIL panelId="${panelId}"`, err);
          setupRetryRef.current?.(cols, rows);
        });
    };

    /** P1-01/02: 设置一次性 Enter 监听，触发后重新 spawn */
    const setupRetry = (t: Terminal, cols: number, rows: number) => {
      retryDisposableRef.current?.dispose();
      const disposable = t.onData((data) => {
        if (data === "\r") {
          // Enter 键 → 移除监听 → 重新 spawn
          disposable.dispose();
          retryDisposableRef.current = null;
          doSpawn(cols, rows);
        }
      });
      retryDisposableRef.current = disposable;
    };

    // P1-01/02: 暴露 doSpawn / setupRetry 供 handlePtyOutput（exit 事件）和 catch 块通过 ref 调用
    doSpawnRef.current = doSpawn;
    setupRetryRef.current = (cols, rows) => setupRetry(term, cols, rows);

    const pollFitAndSpawn = () => {
      fitFrames++;
      const elapsed = performance.now() - fitStartTime;

      if (container.offsetWidth > 0 && container.offsetHeight > 0) {
        if (canFit(term, fitAddon, container, isDisposedRef)) {
          try {
            fitAddon.fit();
            const dims = fitAddon.proposeDimensions();
            // NaN 守卫：缓存 Terminal 复用后 proposeDimensions 可能返回 cols/rows=NaN
            if (dims && Number.isFinite(dims.cols) && Number.isFinite(dims.rows)) {
              doSpawn(dims.cols, dims.rows);
              return;
            }
          } catch {
            // fit 失败 → 回退
          }
        }
        doSpawn(80, 24);
        return;
      }

      if (fitFrames >= MAX_FRAMES || elapsed >= FIT_TIMEOUT) {
        doSpawn(80, 24);
        return;
      }

      fitRafId = requestAnimationFrame(pollFitAndSpawn);
    };

    fitRafId = requestAnimationFrame(pollFitAndSpawn);

    // 终端输入 → 后端
    term.onData((data) => {
        if (sessionIdRef.current) {
          pty.write(
            sessionIdRef.current,
            new TextEncoder().encode(data),
          ).catch((err) => console.error("PTY write 失败:", err));
        }
      });

    // P2-03: ResizeObserver — fit() 与 resize 合并 debounce，X/Y 分离策略
    // - 仅行数变化（廉价，无需 re-wrap）：立即 resize
    // - 列数变化（昂贵，需 re-wrap 所有行）：100ms debounce
    // - resize 前先 flush 积压的旧尺寸 PTY 输出，防止错位
    let resizeTimer: ReturnType<typeof setTimeout> | null = null;

    const resizeObserver = new ResizeObserver(() => {
      if (!canFit(term, fitAddon, container, isDisposedRef)) return;

      try {
        const dims = fitAddon.proposeDimensions();
        // 修复 1：补 NaN 检查——proposeDimensions 可能返回 NaN（xtermjs#4338）
        if (!dims || !Number.isFinite(dims.cols) || !Number.isFinite(dims.rows) || !sessionIdRef.current) {
          return;
        }

        const prev = prevDimsRef.current;
        const colsChanged = prev === null || dims.cols !== prev.cols;
        const rowsChanged = prev === null || dims.rows !== prev.rows;
        prevDimsRef.current = dims;

        // 无变化：跳过
        if (!colsChanged && !rowsChanged) return;

        // resize 前丢弃积压缓冲区——旧尺寸数据渲染到新视口会造成错位/撕裂，
        // 直接丢弃由 Ink SIGWINCH 后全帧重绘替代
        cancelPendingFlush();

        if (rowsChanged && !colsChanged) {
          // 仅行数变化 → 立即 fit + resize（廉价，无需 re-wrap）
          fitAddon.fit();
          pty.resize(sessionIdRef.current, dims.cols, dims.rows)
            .catch((err) => console.error("PTY resize 失败:", err));
          return;
        }

        // 列数变化（或首次 resize）→ 100ms debounce
        if (resizeTimer !== null) clearTimeout(resizeTimer);
        resizeTimer = setTimeout(() => {
          cancelPendingFlush(); // debounce 期间新积压的旧尺寸数据直接丢弃
          try {
            fitAddon.fit();
            pty.resize(sessionIdRef.current!, dims.cols, dims.rows)
                .catch((err) => console.error("PTY resize 失败:", err));
          } catch {
            // fit 失败不影响渲染
          }
        }, 100);
      } catch {
        // fit 失败不影响渲染
      }
    });
    resizeObserver.observe(container);

    return () => {
      // 清理
      isDisposedRef.current = true;
      if (fitRafId !== null) {
        cancelAnimationFrame(fitRafId);
      }
      if (idleTimerRef.current !== null) {
        clearTimeout(idleTimerRef.current);
      }
      if (maxTimerRef.current !== null) {
        clearTimeout(maxTimerRef.current);
      }
      if (resizeTimer !== null) {
        clearTimeout(resizeTimer);
      }
      resizeObserver.disconnect();

      const sid = sessionIdRef.current;
      // 页面切换不再销毁终端（多 Dockview 实例 + CSS 显隐），仅用户关闭面板时走到此分支
      if (sid) {
        pty.kill(sid).catch(() => {}); // cleanup 中静默吞错，进程可能已退出
      }
      TerminalRegistry.remove(panelId);
      // OSC 52 剪贴板 handler 清理
      osc52Disposable.dispose();
      // OSC 133 handler 清理
      osc133Disposable.dispose();
      // P1-01/02: 清理重试 Enter 监听
      retryDisposableRef.current?.dispose();
      retryDisposableRef.current = null;
      // P1-01/02: 清空跨闭包 ref，防止卸载后 setRetryRef 回调访问已销毁的 term
      doSpawnRef.current = null;
      setupRetryRef.current = null;
      webglAddonRef.current?.dispose();
      fitAddonRef.current?.dispose();
      terminalRef.current?.dispose();
      terminalRef.current = null;
      // P2-01: 清空 E2E 文本缓冲
      e2eTextBufferRef.current.length = 0;
    };
  }, [container, panelId, cols, rows, flushBuffer, cancelPendingFlush, handlePtyOutput]);

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

  // P1-13: WebGL 按可见性管理 — 页面隐藏（display:none）时释放 WebGL 资源，
  // 重新可见时重检测并重建 WebGL addon + flush 累积的 PTY 输出。
  useEffect(() => {
    const term = terminalRef.current;
    if (!term) return;

    if (visible === false) {
      // 隐藏：释放 WebGL addon，回退 DOM 渲染器
      if (webglAddonRef.current) {
        webglAddonRef.current.dispose();
        webglAddonRef.current = null;
      }
    } else if (visible === true) {
      // P2-43: 可见时若 WebGL 未加载且可用，延迟重建（非一次性失败）
      if (!webglAddonRef.current && detectWebgl()) {
        setupWebglWithRetry(term, webglAddonRef);
      }
      // 切回可见时立即 flush 非焦点期间累积的 PTY 输出
      if (pendingBufferRef.current.length > 0) {
        flushBuffer();
      }
    }
  }, [visible, flushBuffer]);

  // 字体大小动态调节：当 fontSize 变化时更新终端并重新适配布局
  useEffect(() => {
    const term = terminalRef.current;
    if (!term || fontSize === undefined) return;

    term.options.fontSize = fontSize;

    if (!canFit(term, fitAddonRef.current, container, isDisposedRef)) return;

    try {
      fitAddonRef.current!.fit();
      const dims = fitAddonRef.current!.proposeDimensions();
      if (dims && sessionIdRef.current) {
        pty.resize(sessionIdRef.current, dims.cols, dims.rows)
          .catch((err) => console.error("PTY resize 失败:", err));
      }
    } catch {
      // fit 失败不影响渲染
    }
  }, [fontSize, container]);

  // Ctrl+鼠标滚轮 调节字体大小
  useEffect(() => {
    if (!container || !onFontSizeChange) return;

    const CLAMP_MIN = 8;
    const CLAMP_MAX = 32;

    const handleWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return; // 非 Ctrl 滚轮透传给 xterm.js scrollback

      e.preventDefault();
      e.stopPropagation();

      // 从 terminal 实例读取当前字体大小，避免闭包过时
      const currentSize = terminalRef.current?.options.fontSize ?? 14;
      const direction = e.deltaY < 0 ? 1 : -1; // 上滚放大，下滚缩小
      const newSize = Math.max(CLAMP_MIN, Math.min(CLAMP_MAX, currentSize + direction));

      if (newSize !== currentSize) {
        onFontSizeChange(newSize);
      }
    };

    container.addEventListener("wheel", handleWheel, { capture: true, passive: false });

    return () => {
      container.removeEventListener("wheel", handleWheel, { capture: true });
    };
  }, [container, onFontSizeChange]);

  return {
    /** 聚焦终端输入 */
    focus: useCallback(() => {
      terminalRef.current?.focus();
    }, []),
    /** 测试专用（生产代码忽略此字段） */
    _test: {
      cancelPendingFlush,
      flushBuffer,
      getPendingBuffer: () => pendingBufferRef.current,
    },
  };
}
