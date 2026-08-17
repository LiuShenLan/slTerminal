// ConfirmDialog —— 全局确认弹窗（UI-801/803）
//
// 契约（Stage 07 跨边界写死）：confirmDialog(opts): Promise<boolean>
//   - 确认按钮 → true；取消按钮 / ESC / 遮罩点击 → false
//   - 视觉规格：遮罩 SHADOW_MENU；卡片 SIDEBAR_BG 底 + CONTEXT_MENU_BORDER 1px 描边
//     + 圆角 8 + contextMenuShadow 阴影；主按钮 FOCUS_BORDER 底 + ON_ACCENT_FG 字
//     （danger → ERROR_FG 底 + SIDEBAR_FG 字）；次按钮 SECONDARY_BG 底 + SIDEBAR_FG 字
//     （UI-803 仅底色/字色、无描边）；两钮圆角 6（UI-306 按钮档）
//   - 焦点管理（FE-28）：弹窗挂起后焦点落确认按钮；Tab/Shift+Tab 在确认/取消两钮间循环
//     （焦点陷阱，不逃出弹窗）；Enter 经按钮原生提交确认（焦点恒在钮上）
//
// 用法：调用方 import { confirmDialog } from "../lib" 即可（无需传组件）；
// 挂载点：ConfirmDialogHost 由 App.tsx 根部置入（本文件与调用点解耦）。
// 硬约束 #1：本组件为纯 UI，不涉及 OS/文件/进程调用。

import { useEffect, useReducer, useRef } from "react";
import {
  SHADOW_MENU,
  SIDEBAR_BG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
  FOCUS_BORDER,
  ON_ACCENT_FG,
  ERROR_FG,
  SIDEBAR_FG,
  SECONDARY_BG,
} from "../theme";

/** confirmDialog 选项（契约见文件头） */
export interface ConfirmDialogOptions {
  /** 标题（省略则不渲染标题行） */
  title?: string;
  /** 正文消息（必填） */
  message: string;
  /** 语义类型（契约字段；视觉区分由 danger 承担，本字段暂不驱动视觉） */
  kind?: "warning" | "error" | "info";
  /** 确认按钮文案（默认「确认」） */
  confirmText?: string;
  /** 取消按钮文案（默认「取消」） */
  cancelText?: string;
  /** true → 主按钮转危险红（ERROR_FG 底 + SIDEBAR_FG 字） */
  danger?: boolean;
}

/** 挂起中的弹窗请求（模块级单槽——同屏最多一个） */
interface PendingConfirm {
  opts: ConfirmDialogOptions;
  resolve: (value: boolean) => void;
}

let pending: PendingConfirm | null = null;
const listeners = new Set<() => void>();

function notifyListeners(): void {
  listeners.forEach((l) => l());
}

/** 以 false 关闭当前弹窗（取消按钮 / ESC / 遮罩点击共用） */
function dismiss(): void {
  if (!pending) return;
  const { resolve } = pending;
  pending = null;
  notifyListeners();
  resolve(false);
}

/** 以 true 关闭当前弹窗（确认按钮） */
function confirm(): void {
  if (!pending) return;
  const { resolve } = pending;
  pending = null;
  notifyListeners();
  resolve(true);
}

/**
 * 弹出确认弹窗，返回用户选择的 Promise。
 * 若已有挂起弹窗（理论上不会发生——调用方通常互斥），前一弹窗视为取消。
 */
export function confirmDialog(opts: ConfirmDialogOptions): Promise<boolean> {
  if (pending) dismiss();
  return new Promise<boolean>((resolve) => {
    pending = { opts, resolve };
    notifyListeners();
  });
}

/** 仅测试：清空挂起弹窗（不解析 Promise），防止用例间互相污染 */
export function _resetConfirmDialog(): void {
  pending = null;
  notifyListeners();
}

/** 挂载点组件——置入 App.tsx 根部；无挂起弹窗时渲染 null */
export function ConfirmDialogHost() {
  const [, setVersion] = useReducer((v: number) => v + 1, 0);
  const okRef = useRef<HTMLButtonElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);

  // 订阅模块级 pending 变更（imperative API → React 渲染桥）
  useEffect(() => {
    listeners.add(setVersion);
    return () => {
      listeners.delete(setVersion);
    };
  }, []);

  // 挂起期间监听全局 ESC → false
  useEffect(() => {
    if (!pending) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") dismiss();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [pending]);

  // 弹窗出现后焦点落到确认按钮（焦点陷阱入口；Enter 经按钮原生提交确认）
  useEffect(() => {
    if (pending) okRef.current?.focus();
  }, [pending]);

  if (!pending) return null;
  const { opts } = pending;
  const isDanger = opts.danger === true;

  return (
    <div
      data-e2e="confirm-dialog-mask"
      onMouseDown={(e) => {
        // 仅遮罩自身命中时关闭（点击卡片内部不触发）
        if (e.target === e.currentTarget) dismiss();
      }}
      style={{
        position: "fixed",
        inset: 0,
        background: SHADOW_MENU, // 遮罩 rgba(0,0,0,0.55)
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
    >
      <div
        data-e2e="confirm-dialog"
        onKeyDown={(e) => {
          // 焦点陷阱：Tab/Shift+Tab 在确认/取消两钮间循环，不逃出弹窗
          if (e.key !== "Tab") return;
          e.preventDefault();
          const next =
            document.activeElement === okRef.current
              ? cancelRef.current
              : okRef.current;
          next?.focus();
        }}
        style={{
          background: SIDEBAR_BG, // 卡片 #1a1a1e 底
          border: `1px solid ${CONTEXT_MENU_BORDER}`, // rgba(255,255,255,0.09) 描边
          borderRadius: 8,
          boxShadow: SIDEBAR_COLORS.contextMenuShadow, // 0 8px 32px rgba(0,0,0,0.35)
          padding: 16,
          minWidth: 320,
          maxWidth: 420,
          color: SIDEBAR_FG,
          fontSize: 13,
        }}
      >
        {opts.title !== undefined && (
          <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
            {/* UI-204/205：弹窗标题 13px + 500，与其它弹窗标题对齐 */}
            {opts.title}
          </div>
        )}
        <div style={{ lineHeight: 1.5, marginBottom: 16 }}>{opts.message}</div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button
            data-e2e="confirm-cancel"
            ref={cancelRef}
            onClick={dismiss}
            style={{
              background: SECONDARY_BG, // 次按钮 #222227 底
              color: SIDEBAR_FG, // #ece9e4 字
              // UI-803 只规定底色/字色——无描边
              borderRadius: 6,
              padding: "5px 14px",
              fontSize: 13,
              cursor: "pointer",
            }}
          >
            {opts.cancelText ?? "取消"}
          </button>
          <button
            data-e2e="confirm-ok"
            ref={okRef}
            onClick={confirm}
            style={{
              // danger → ERROR_FG 底 + SIDEBAR_FG 字；否则 FOCUS_BORDER 底 + ON_ACCENT_FG 字
              background: isDanger ? ERROR_FG : FOCUS_BORDER,
              color: isDanger ? SIDEBAR_FG : ON_ACCENT_FG,
              border: "none",
              borderRadius: 6,
              padding: "5px 14px",
              fontSize: 13,
              fontWeight: 500,
              cursor: "pointer",
            }}
          >
            {opts.confirmText ?? "确认"}
          </button>
        </div>
      </div>
    </div>
  );
}
