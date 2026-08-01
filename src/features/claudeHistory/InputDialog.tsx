// InputDialog.tsx — 自绘输入弹窗（FE-07）
//
// Tauri 原生 dialog 无输入框，故自绘模态：遮罩 + 居中输入框 + 确认/取消按钮。
// 挂载自动 focus + 全选；Enter 提交（trim 后）/ Escape、遮罩点击、取消按钮 → onCancel；
// 空值（trim 后）禁止确认（按钮 disabled + Enter 双保险）。
// 纯受控展示组件，不碰 IPC。契约见 docs/claude-history-view/stages.md「跨 Stage 契约」。

import React, { useEffect, useRef, useState } from "react";
import {
  SIDEBAR_BG,
  SIDEBAR_FG,
  BUTTON_FG,
  DIM_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
  SHADOW_MENU,
} from "../../theme";

/** 输入弹窗契约（写死，见 stages.md 跨 Stage 契约——agent B 照此消费） */
export interface InputDialogProps {
  /** 弹窗标题（如「重命名会话」） */
  title: string;
  /** 输入框初始值（如当前标题） */
  initialValue: string;
  /** 确认（值已 trim） */
  onSubmit(value: string): void;
  /** 取消（Escape / 遮罩点击 / 取消按钮） */
  onCancel(): void;
}

export const InputDialog: React.FC<InputDialogProps> = ({
  title,
  initialValue,
  onSubmit,
  onCancel,
}) => {
  const [value, setValue] = useState(initialValue);
  const [focused, setFocused] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // 挂载自动 focus + 全选
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const trimmed = value.trim();

  const handleConfirm = () => {
    if (!trimmed) return; // 空值禁止确认（按钮 disabled 之外的双保险）
    onSubmit(trimmed);
  };

  return (
    <div
      data-e2e="agent-history-input-dialog"
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

        {/* 输入框：受控 + 焦点边框高亮 */}
        <input
          ref={inputRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              handleConfirm();
            } else if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            }
          }}
          style={{
            padding: "4px 8px",
            background: INPUT_BG,
            color: SIDEBAR_FG,
            border: `1px solid ${focused ? FOCUS_BORDER : INPUT_BORDER}`,
            borderRadius: 4,
            outline: "none",
            fontSize: "13px",
          }}
        />

        {/* 取消 / 确认按钮（空值禁确认） */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px" }}>
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
          <button
            type="button"
            disabled={!trimmed}
            onClick={handleConfirm}
            style={{
              padding: "4px 12px",
              background: "transparent",
              color: trimmed ? BUTTON_FG : DIM_FG,
              border: "none",
              borderRadius: 4,
              cursor: trimmed ? "pointer" : "default",
              fontSize: "13px",
            }}
          >
            确认
          </button>
        </div>
      </div>
    </div>
  );
};
