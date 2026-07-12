// usePtyResize — PTY resize 副作用 hook
//
// 职责：
// - ResizeObserver 监听容器尺寸变化 → fit + pty.resize
// - X/Y 分离 debounce（行变化立即 fit+resize；列变化 100ms debounce）
// - NaN 守卫（Number.isFinite() 防 xtermjs#4338）
// - 无变化跳过
//
// 初始 rAF 轮询 + PTY spawn 由 useXterm 编排层处理。

import { useEffect, useRef } from "react";
import type { MutableRefObject } from "react";
import { type Terminal } from "@xterm/xterm";
import { type FitAddon } from "@xterm/addon-fit";
import { pty } from "../../ipc";

/** 列数变化 resize debounce 时间（毫秒）：宽度变化需 re-wrap 所有行，成本高 */
const COL_RESIZE_DEBOUNCE_MS = 100;

/**
 * usePtyResize — 管理终端运行时 resize（ResizeObserver）
 *
 * 注意：初始 rAF 轮询 + PTY spawn 由 useXterm 编排层负责，本 hook 仅处理运行时 resize。
 *
 * @param container         终端容器 DOM 元素
 * @param terminal          xterm.js Terminal 实例（可为 null，null 时 effect 不执行）
 * @param fitAddon          FitAddon 实例（可为 null）
 * @param ptySessionIdRef   PTY 会话 ID ref（非 null 表示 PTY 已 spawn，允许 resize，动态读取最新值）
 * @param cancelPendingFlush 丢弃输出缓冲合帧的回调（resize 前调用，防止旧尺寸数据错位）
 * @param isReady           终端是否就绪（canFit + 未销毁等复合条件）
 * @param isDisposedRef     组件是否已销毁 ref（回调中防御性检查，防止 unmount 后回调触发）
 */
export function usePtyResize(
  container: HTMLElement | null,
  terminal: Terminal | null,
  fitAddon: FitAddon | null,
  ptySessionIdRef: MutableRefObject<string | null>,
  cancelPendingFlush: () => void,
  isReady: boolean,
  isDisposedRef?: MutableRefObject<boolean>,
): void {
  // ── ref 镜像：避免 effect 依赖变化导致 ResizeObserver 重建 ──

  /** 上次尺寸快照，用于区分行/列变化类型 */
  const prevDimsRef = useRef<{ cols: number; rows: number } | null>(null);
  /** 镜像 cancelPendingFlush，回调时读取最新值 */
  const cancelPendingFlushRef = useRef(cancelPendingFlush);
  cancelPendingFlushRef.current = cancelPendingFlush;
  /** 镜像 isReady，运行时检查终端是否可操作 */
  const isReadyRef = useRef(isReady);
  isReadyRef.current = isReady;

  useEffect(() => {
    // 前置条件不满足时不启动 ResizeObserver
    if (!container || !terminal || !fitAddon) return;

    // ═══════════════════════════════════════════════════════════════
    // ResizeObserver：尺寸变化 → fit + resize（X/Y 分离 debounce）
    // ═══════════════════════════════════════════════════════════════
    let resizeTimer: ReturnType<typeof setTimeout> | null = null;

    const resizeObserver = new ResizeObserver(() => {
      // 终端未就绪或已销毁时跳过
      if (!isReadyRef.current || isDisposedRef?.current) return;

      try {
        const dims = fitAddon.proposeDimensions();
        // NaN 守卫：proposeDimensions 可能返回 NaN（xtermjs#4338）
        const sid = ptySessionIdRef.current;
        if (!dims || !Number.isFinite(dims.cols) || !Number.isFinite(dims.rows) || !sid) {
          return;
        }

        const prev = prevDimsRef.current;
        const colsChanged = prev === null || dims.cols !== prev.cols;
        const rowsChanged = prev === null || dims.rows !== prev.rows;
        prevDimsRef.current = { cols: dims.cols, rows: dims.rows };

        // 无变化：跳过
        if (!colsChanged && !rowsChanged) return;

        // resize 前丢弃积压缓冲——旧尺寸数据渲染到新视口会造成错位/撕裂
        cancelPendingFlushRef.current();

        if (rowsChanged && !colsChanged) {
          // 仅行数变化 → 立即 fit + resize（廉价，无需 re-wrap）
          fitAddon.fit();
          pty.resize(sid, dims.cols, dims.rows).catch(() => {});
          return;
        }

        // 列数变化（或首次 resize）→ debounce
        if (resizeTimer !== null) clearTimeout(resizeTimer);
        resizeTimer = setTimeout(() => {
          // debounce 期间新积压的旧尺寸数据直接丢弃
          cancelPendingFlushRef.current();
          try {
            fitAddon.fit();
            const currentSid = ptySessionIdRef.current;
            if (currentSid) {
              pty.resize(currentSid, dims.cols, dims.rows).catch(() => {});
            }
          } catch {
            // fit 失败不影响渲染
          }
        }, COL_RESIZE_DEBOUNCE_MS);
      } catch {
        // fit 失败不影响渲染
      }
    });
    resizeObserver.observe(container);

    // ── 清理 ──
    return () => {
      if (resizeTimer !== null) {
        clearTimeout(resizeTimer);
      }
      resizeObserver.disconnect();
    };
  }, [container, terminal, fitAddon]);
}
