// TerminalRenameDialog — 终端页签重命名弹窗（右键菜单「重命名」入口）
//
// 自绘模态（照 SessionActionDialog 模式）：遮罩 + 居中卡片 + 输入框 + 确定/取消。
// Enter 确认 / Esc / 遮罩点击 / 取消按钮 → 取消；提交时 trim 后为空 → 行内错误
// 提示并保持弹窗打开（拒绝语义）。纯受控展示组件，不碰 IPC。
//
// 交互契约：
// - 预填 initialTitle + autoFocus + 全选（方便直接覆盖）
// - 非空提交 → onConfirm(trimmed)；空/纯空白 → setError 不关弹窗
// - 输入后错误自动清除

import React, { useEffect, useRef, useState } from "react";
import {
  SIDEBAR_BG,
  SIDEBAR_FG,
  ERROR_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  ON_ACCENT_FG,
  SECONDARY_BG,
  CONTEXT_MENU_BORDER,
  SIDEBAR_COLORS,
  SHADOW_MENU,
} from "../theme";

/** 重命名弹窗契约 */
export interface TerminalRenameDialogProps {
  /** 预填标题（当前页签标题） */
  initialTitle: string;
  /** 确认（newTitle 已 trim 且非空） */
  onConfirm(newTitle: string): void;
  /** 取消（Escape / 遮罩点击 / 取消按钮） */
  onCancel(): void;
}

export const TerminalRenameDialog: React.FC<TerminalRenameDialogProps> = ({
  initialTitle,
  onConfirm,
  onCancel,
}) => {
  const [value, setValue] = useState(initialTitle);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // 预填值变化时跟随（面板切换重开弹窗场景）
  useEffect(() => {
    setValue(initialTitle);
  }, [initialTitle]);

  // 挂载聚焦 + 全选（方便直接覆盖原名）；StrictMode 双挂载下 select 幂等
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

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

  const handleSubmit = () => {
    const trimmed = value.trim();
    if (!trimmed) {
      setError("名称不能为空");
      return;
    }
    onConfirm(trimmed);
  };

  return (
    <div
      data-e2e="terminal-rename-dialog"
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
          重命名终端
        </div>

        {/* 输入框 */}
        <input
          ref={inputRef}
          data-e2e="terminal-rename-input"
          type="text"
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            // 输入后清除错误提示
            if (error) setError(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              handleSubmit();
            }
          }}
          style={{
            padding: "6px 8px",
            background: INPUT_BG,
            color: SIDEBAR_FG,
            border: `1px solid ${error ? ERROR_FG : INPUT_BORDER}`,
            borderRadius: 4,
            fontSize: "13px",
            outline: "none",
          }}
          onFocus={(e) => {
            e.target.select();
          }}
        />

        {/* 错误提示 */}
        {error && (
          <div
            style={{
              fontSize: "12px",
              color: ERROR_FG,
            }}
          >
            {error}
          </div>
        )}

        {/* 按钮行（UI-801 按钮规格：次钮 SECONDARY_BG 底 + SIDEBAR_FG 字，主钮 FOCUS_BORDER
            底 + ON_ACCENT_FG 字，照 ConfirmDialog 规格） */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px" }}>
          <button
            type="button"
            onClick={onCancel}
            style={{
              padding: "5px 14px",
              background: SECONDARY_BG,
              color: SIDEBAR_FG,
              border: `1px solid ${CONTEXT_MENU_BORDER}`,
              borderRadius: 4,
              cursor: "pointer",
              fontSize: "13px",
            }}
          >
            取消
          </button>
          <button
            type="button"
            onClick={handleSubmit}
            style={{
              padding: "5px 14px",
              background: FOCUS_BORDER,
              color: ON_ACCENT_FG,
              border: "none",
              borderRadius: 4,
              cursor: "pointer",
              fontSize: "13px",
              fontWeight: 500,
            }}
          >
            确定
          </button>
        </div>
      </div>
    </div>
  );
};
