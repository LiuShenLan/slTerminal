// useZoomHud.ts — 缩放 HUD 状态机（docViewer 共享，2026-09-06 自 PreviewFrame 内嵌提升）
//
// 悬浮于面板根右上悬浮区（FloatingArea）的瞬态缩放气泡状态：zoom 变化 → 显示
// 「百分比 + 重置」（含回落 100% 的变化——Chrome 缩放气泡语义），
// ZOOM_HUD_HIDE_MS 无操作自动消失、期间缩放续期；等值回声（复位回声等）不复活
// 已隐藏的气泡。复位/iframe 重建的「静默归 1」不经过 report——由 hide() 直接
// 隐藏（面板经 PreviewFrame onZoomReset / 重置按钮组装链调用）。
//
// flushSync：report 多在 iframe postMessage 原生 message 事件内被调用（滚轮即时
// 反馈）——并发调度会延迟一帧致气泡滞后闪烁，紧急 UI 同步 commit（自原
// PreviewFrame 注释语义随迁）。

import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";

/** HUD 隐藏延迟（毫秒）：无缩放操作即消失 */
export const ZOOM_HUD_HIDE_MS = 3000;

export interface ZoomHudState {
  zoom: number;
  visible: boolean;
}

export interface ZoomHudApi {
  hud: ZoomHudState;
  /** 上报缩放变化（等值回声忽略；zoom=1 视为复位→隐藏） */
  report: (zoom: number) => void;
  /** 立即隐藏并复位基准（重置点击 / 卸载前清理） */
  hide: () => void;
}

export function useZoomHud(): ZoomHudApi {
  const [hud, setHud] = useState<ZoomHudState>({ zoom: 1, visible: false });
  // 逻辑真值源（同步读写，避免 setState 异步比较竞态）；hide/report 同步更新
  const lastZoomRef = useRef(1);
  const hideTimerRef = useRef<number | null>(null);

  const clearTimer = () => {
    if (hideTimerRef.current !== null) {
      window.clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
  };

  const hide = () => {
    lastZoomRef.current = 1;
    clearTimer();
    setHud({ zoom: 1, visible: false });
  };

  const report = (zoom: number) => {
    // 等值回声（复位回声等）不复活已隐藏的气泡；变化（含回落 100%）即显示
    if (zoom === lastZoomRef.current) return;
    lastZoomRef.current = zoom;
    clearTimer();
    // 缩放变化：显示 + 续期（原生 message 事件内，flushSync 同步 commit 防滞后闪烁）
    flushSync(() => setHud({ zoom, visible: true }));
    hideTimerRef.current = window.setTimeout(() => {
      hideTimerRef.current = null;
      setHud((prev) => (prev.visible ? { ...prev, visible: false } : prev));
    }, ZOOM_HUD_HIDE_MS);
  };

  // 卸载清理隐藏计时器（防跨实例泄漏）
  useEffect(() => {
    return () => clearTimer();
  }, []);

  return { hud, report, hide };
}
