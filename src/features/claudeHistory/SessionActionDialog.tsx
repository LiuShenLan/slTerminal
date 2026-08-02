// SessionActionDialog.tsx — 自绘动作选项弹窗（问题 5 修复）
//
// Tauri 原生 dialog（ask/confirm）仅支持两按钮（okLabel/cancelLabel），无法表达
// 「分支恢复 / 切换到该会话操作页面 / 取消」等多选项，故自绘模态（照 InputDialog 模式）：
// 遮罩 + 标题 + 消息 + 动作按钮（actions）+ 底部取消按钮。
// Esc / 遮罩点击 / 取消按钮 → onCancel；动作按钮点击 → action()（弹窗关闭由调用方
// 经 onCancel/受控 props 决定——本组件无内部关闭逻辑，保持纯展示）。
// 纯受控展示组件，不碰 IPC。

import React, { useEffect } from "react";
import {
  SIDEBAR_BG,
  SIDEBAR_FG,
  BUTTON_FG,
  DIM_FG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
  SHADOW_MENU,
} from "../../theme";

/** 动作弹窗契约 */
export interface SessionActionDialogProps {
  /** 弹窗标题（如「会话运行中」） */
  title: string;
  /** 说明消息（可选） */
  message?: string;
  /** 动作按钮（竖排，自下而上为主操作置底） */
  actions: { label: string; action(): void }[];
  /** 取消（Escape / 遮罩点击 / 取消按钮） */
  onCancel(): void;
}

export const SessionActionDialog: React.FC<SessionActionDialogProps> = ({
  title,
  message,
  actions,
  onCancel,
}) => {
  // Esc 取消
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onCancel]);

  return (
    <div
      data-e2e="agent-history-action-dialog"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1000,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        backgroundColor: SHADOW_MENU,
      }}
      onMouseDown={(e) => {
        // 仅遮罩自身命中才取消（点面板内部不冒泡取消）
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          width: 320,
          padding: "14px 16px",
          background: SIDEBAR_BG,
          border: `1px solid ${CONTEXT_MENU_BORDER}`,
          borderRadius: 8,
          boxShadow: SIDEBAR_COLORS.contextMenuShadow,
        }}
      >
        {/* 弹窗标题 */}
        <div
          style={{
            fontSize: "13px",
            fontWeight: "bold",
            color: SIDEBAR_FG,
          }}
        >
          {title}
        </div>

        {/* 说明消息 */}
        {message != null && (
          <div
            style={{
              fontSize: "12px",
              color: DIM_FG,
              lineHeight: "1.5",
            }}
          >
            {message}
          </div>
        )}

        {/* 动作按钮（竖排） */}
        <div
          style={{ display: "flex", flexDirection: "column", gap: "6px" }}
        >
          {actions.map((a, i) => (
            <button
              key={i}
              type="button"
              onClick={a.action}
              style={{
                padding: "6px 12px",
                background: "transparent",
                color: BUTTON_FG,
                border: `1px solid ${CONTEXT_MENU_BORDER}`,
                borderRadius: 4,
                cursor: "pointer",
                fontSize: "13px",
                textAlign: "center",
              }}
            >
              {a.label}
            </button>
          ))}
        </div>

        {/* 取消按钮 */}
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <button
            type="button"
            onClick={onCancel}
            style={{
              padding: "4px 12px",
              background: "transparent",
              color: BUTTON_FG,
              border: "none",
              borderRadius: 4,
              cursor: "pointer",
              fontSize: "13px",
            }}
          >
            取消
          </button>
        </div>
      </div>
    </div>
  );
};
