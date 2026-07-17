// usePtyOutput.ts — PTY 输出处理 hook
//
// 职责：
// - handlePtyOutput 回调：PTY 输出事件 → 阈值分流 → 直写或合帧
// - Uint8Array 缓冲 + Idle+Max 双定时器 + DEC 2026 同步更新
// - 非焦点终端降频（visible 门控 + 64KB 上限）
// - PTY 进程退出处理与重连机制
// - spawn 失败错误恢复（Enter 重连 disposable 管理）

import { useRef, useCallback } from "react";
import type { MutableRefObject } from "react";
import type { Terminal } from "@xterm/xterm";
import type { PtyEvent } from "../../types";
import type { TabState } from "./TabTitleRegistry";

/** DEC 2026 同步更新 ANSI 转义序列 */
const DEC2026_PREFIX = "\x1b[?2026h";
const DEC2026_SUFFIX = "\x1b[?2026l";

/** 合帧分流阈值（字节）：小于此值直写终端，大于等于此值走合帧 + DEC 2026 路径 */
const DIRECT_WRITE_THRESHOLD = 64;

/** Idle 定时器间隔（毫秒）：2ms 无新数据则 flush，适应 Ink 高频 burst 输出 */
const IDLE_FLUSH_MS = 2;
/** Max 定时器间隔（毫秒）：最多 16ms 强制 flush 一次，防止饥饿 */
const MAX_FLUSH_MS = 16;
/** 非焦点终端待输出数据量上限（字节），防止内存无限增长 */
const MAX_PENDING_BYTES = 65536; // 64KB
/** E2E 测试文本缓冲行数上限 */
const E2E_BUFFER_MAX_LINES = 1000;

/** usePtyOutput hook 返回类型 */
export interface UsePtyOutputReturn {
  /** 冲刷待输出缓冲区到终端（DEC 2026 包裹） */
  flushBuffer: () => void;
  /** 丢弃待输出缓冲区并清理定时器（不渲染，用于 resize 前清理） */
  cancelPendingFlush: () => void;
  /** PTY 输出事件处理器（传给 pty.spawn 的 onOutput 回调） */
  handlePtyOutput: (event: PtyEvent) => void;
  /** 当前是否有注册命令在运行（如 claude） */
  isCommandRunning: MutableRefObject<boolean>;
  /** 设置 Enter 重连监听——调用方在 spawn 失败或进程退出后调用 */
  setupRetry: (cols: number, rows: number) => void;
  /** E2E 测试文本缓冲 ref（外部传入，与 __e2e_getTerminalText 共享） */
  e2eTextBufferRef: MutableRefObject<string[]>;
  /** 获取当前待 flush 缓冲快照（仅测试用） */
  getPendingBuffer: () => Uint8Array[];
  /** retry disposable ref（供 useXterm cleanup 显式清理，FE-08） */
  retryDisposableRef: MutableRefObject<{ dispose: () => void } | null>;
}

/**
 * PTY 输出处理 hook
 *
 * 将 PTY 输出事件解码后按阈值分流：小数据直写终端（低延迟打字回显），
 * 大数据走合帧缓冲 + DEC 2026 原子渲染（消除 Ink 高频帧撕裂）。
 * 非焦点终端仅积累不渲染（上限 64KB），切回时回放。
 *
 * 合帧管道：PTY 输出 → handlePtyOutput → 阈值分流 → 合帧缓冲 → 双定时器 → flushBuffer → xterm.js
 *
 * @param terminal        xterm.js Terminal 实例 ref
 * @param panelId         面板 ID（供日志/调试用）
 * @param visible         面板是否可见（非焦点终端降频积累）
 * @param onTabStateChange  页签状态变更回调（命令运行/退出时触发）
 * @param onRetrySpawn    重连 spawn 回调 ref（Enter 触发时调用，由 useXterm 设置）
 * @param e2eBuffer       E2E 测试文本缓冲 ref（外部传入，与 __e2e_getTerminalText 共享同一缓冲）
 * @param cmdRunningRef   共享的命令运行状态 ref（由 useXterm 创建，供 useCommandDetection 写入）
 */
export function usePtyOutput(
  terminal: MutableRefObject<Terminal | null>,
  _panelId: string,
  visible: boolean,
  onTabStateChange?: (state: TabState) => void,
  onRetrySpawn?: MutableRefObject<((cols: number, rows: number) => void) | null>,
  e2eBuffer?: MutableRefObject<string[]>,
  cmdRunningRef?: MutableRefObject<boolean>,
): UsePtyOutputReturn {
  // ── 输出合帧缓冲区（Uint8Array 替代字符串拼接，减少 GC 压力 60-80%）──
  const pendingBufferRef = useRef<Uint8Array[]>([]);
  // 缓冲区字节计数（限制 64KB 上限，防止非焦点终端内存无限增长）
  const pendingBufSizeRef = useRef(0);
  // Idle+Max 双定时器：适应 Ink 高频小块 burst 输出模式
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const maxTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // visible 快照 ref：避免 handlePtyOutput 依赖 visible 导致 PTY 回调重建
  const visibleRef = useRef(visible);
  visibleRef.current = visible;
  // 复用 TextDecoder 实例，避免每次 PTY 输出都 new
  const decoderRef = useRef(new TextDecoder());
  // E2E 测试文本缓冲（外部传入或内部创建，累积 PTY 输出文本）
  const e2eInternalRef = useRef<string[]>([]);
  const e2eTextBufferRef = e2eBuffer ?? e2eInternalRef;
  // 当前是否有注册命令在运行（如 claude）——使用外部共享 ref 或内部创建
  const _cmdInternalRef = useRef(false);
  const isCommandRunningRef = cmdRunningRef ?? _cmdInternalRef;
  // 回调 ref：避免 handlePtyOutput 依赖 onTabStateChange 导致重建
  const onTabStateChangeRef = useRef(onTabStateChange);
  onTabStateChangeRef.current = onTabStateChange;
  // Enter 重连 disposable（供 setupRetry 管理）
  const retryDisposableRef = useRef<{ dispose: () => void } | null>(null);
  // 最近一次 retry 使用的终端尺寸（供 exit 时 setupRetry 调用）
  const retryDimsRef = useRef({ cols: 80, rows: 24 });

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

    const term = terminal.current;
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

    // DEC 2026 包裹：xterm.js 6.0+ 原生支持，所有 grid 变更在单帧内原子渲染——消除撕裂
    const enc = new TextEncoder();
    const prefix = enc.encode(DEC2026_PREFIX);
    const suffix = enc.encode(DEC2026_SUFFIX);
    const wrapped = new Uint8Array(prefix.length + merged.length + suffix.length);
    wrapped.set(prefix, 0);
    wrapped.set(merged, prefix.length);
    wrapped.set(suffix, prefix.length + merged.length);

    term.write(wrapped);
  }, [terminal]);

  /** 设置 Enter 重连监听——调用方在 spawn 失败或进程退出后调用 */
  const setupRetry = useCallback(
    (cols: number, rows: number) => {
      const term = terminal.current;
      if (!term) return;

      // 记录尺寸供 exit 时使用
      retryDimsRef.current = { cols, rows };

      // 清理旧监听
      retryDisposableRef.current?.dispose();

      // 注册一次性 Enter 监听：Enter 键触发后自动移除 → 调用 spawn 回调
      const disposable = term.onData((data: string) => {
        if (data === "\r") {
          disposable.dispose();
          retryDisposableRef.current = null;
          onRetrySpawn?.current?.(cols, rows);
        }
      });
      retryDisposableRef.current = disposable;
    },
    [terminal, onRetrySpawn],
  );

  /** 处理 PTY 输出事件 */
  const handlePtyOutput = useCallback(
    (event: PtyEvent) => {
      if (event.type === "output") {
        const rawBytes = new Uint8Array(event.data.bytes);
        const text = decoderRef.current.decode(rawBytes);

        // E2E 文本缓冲记录（用于测试验证）
        e2eTextBufferRef.current.push(text);
        if (e2eTextBufferRef.current.length > E2E_BUFFER_MAX_LINES) {
          e2eTextBufferRef.current.splice(
            0,
            e2eTextBufferRef.current.length - E2E_BUFFER_MAX_LINES,
          );
        }

        // 直写阈值：使用 rawBytes.length（字节）而非 text.length（字符）
        // CJK 字符在 UTF-8 中每字符 3 字节，text.length 会低估实际数据量，
        // 导致含中文的 Ink 输出绕过合帧路径造成撕裂
        const isVisible = visibleRef.current !== false;
        if (rawBytes.length < DIRECT_WRITE_THRESHOLD && isVisible) {
          // 直写路径：极小数据且面板可见时低延迟优先
          terminal.current?.write(text);
        } else {
          // 合帧路径：≥64 字节走缓冲 + DEC 2026 原子渲染

          // 内存上限保护：超出 MAX_PENDING_BYTES 时丢弃最旧数据块
          if (pendingBufSizeRef.current + rawBytes.length > MAX_PENDING_BYTES) {
            while (
              pendingBufSizeRef.current > 0 &&
              pendingBufSizeRef.current + rawBytes.length > MAX_PENDING_BYTES
            ) {
              const dropped = pendingBufferRef.current.shift();
              if (dropped) pendingBufSizeRef.current -= dropped.length;
              else break;
            }
          }

          pendingBufferRef.current.push(rawBytes);
          pendingBufSizeRef.current += rawBytes.length;

          // 非焦点终端降频：仅累积不 flush，切回可见时统一回放
          if (isVisible) {
            // Idle 定时器：IDLE_FLUSH_MS 内无新数据则 flush（适应 Ink burst 模式）
            if (idleTimerRef.current !== null) clearTimeout(idleTimerRef.current);
            idleTimerRef.current = setTimeout(flushBuffer, IDLE_FLUSH_MS);

            // Max 定时器：确保 MAX_FLUSH_MS 内至少 flush 一次（防止饥饿）
            if (maxTimerRef.current === null) {
              maxTimerRef.current = setTimeout(() => {
                maxTimerRef.current = null;
                flushBuffer();
              }, MAX_FLUSH_MS);
            }
          }
        }
      } else if (event.type === "exit") {
        // 进程退出处理
        // 注意：不再自管 sessionId 副本——PTY session 生命周期由 TerminalRegistry 集中管理（约束 #8）

        // 若命令正在运行则重置页签状态（恢复原标题和图标）
        if (isCommandRunningRef.current) {
          isCommandRunningRef.current = false;
          onTabStateChangeRef.current?.({ active: false });
        }

        // 退出提示 + 重连入口
        terminal.current?.writeln(
          `\r\n进程已退出 (退出码: ${event.data.code ?? "?"})\r\n按 Enter 重新连接...\r\n`,
        );

        // 设置 Enter 重连监听（使用最近一次记录的终端尺寸）
        setupRetry(retryDimsRef.current.cols, retryDimsRef.current.rows);
      }
    },
    [flushBuffer, setupRetry, terminal],
  );

  return {
    flushBuffer,
    cancelPendingFlush,
    handlePtyOutput,
    isCommandRunning: isCommandRunningRef,
    setupRetry,
    e2eTextBufferRef,
    getPendingBuffer: () => pendingBufferRef.current,
    retryDisposableRef,
  };
}
