// ModeSwitcher.tsx — 形态切换条（docViewer 共享悬浮 UI）
//
// 文档面板（htmlviewer 二态 / markdownviewer 三态）右上角常驻半透明切换条：
// 受控组件（value/onChange），单选高亮当前查看形态。定位由宿主负责
// （PreviewFrame overlay 槽 / 面板根），本组件不持有绝对定位。
//
// 配色全走 theme facade token（硬约束 #6）：SECONDARY_BG 半透明底 +
// SEPARATOR_BG 描边；激活项 ACTIVE_SELECTION_BG 底 + ACCENT_FG 文字。
// 半透明常驻语义：默认 0.85 透明度，hover 提为 1（不透明度视觉反馈）。

import React from "react";
import {
  ACCENT_FG,
  ACTIVE_SELECTION_BG,
  SECONDARY_BG,
  SEPARATOR_BG,
  SIDEBAR_FG,
} from "../../theme";

/** 形态选项（id 为受控值，label 为显示文案） */
export interface ModeOption<T extends string> {
  id: T;
  label: string;
  title?: string;
}

export interface ModeSwitcherProps<T extends string> {
  modes: readonly ModeOption<T>[];
  value: T;
  onChange: (mode: T) => void;
  /** e2e 探针前缀（默认 "doc"，产出 doc-mode-switcher / doc-mode-<id>） */
  dataE2ePrefix?: string;
}

/** 胶囊容器：SECONDARY_BG 半透明底（默认 0.85，hover 提 1） */
export const switcherChipStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 2,
  background: SECONDARY_BG,
  opacity: 0.85,
  border: `1px solid ${SEPARATOR_BG}`,
  borderRadius: 6,
  padding: 2,
  fontSize: 12,
  color: SIDEBAR_FG,
};

/** 单项按钮样式工厂（active = 当前形态高亮） */
export function modeBtnStyle(active: boolean): React.CSSProperties {
  return {
    background: active ? ACTIVE_SELECTION_BG : "none",
    border: "none",
    borderRadius: 4,
    cursor: "pointer",
    color: active ? ACCENT_FG : SIDEBAR_FG,
    fontSize: 12,
    padding: "3px 10px",
    whiteSpace: "nowrap",
  };
}

export function ModeSwitcher<T extends string>({
  modes,
  value,
  onChange,
  dataE2ePrefix = "doc",
}: ModeSwitcherProps<T>): React.JSX.Element {
  return (
    <div
      data-e2e={`${dataE2ePrefix}-mode-switcher`}
      style={switcherChipStyle}
      onMouseEnter={(e) => {
        (e.currentTarget as HTMLDivElement).style.opacity = "1";
      }}
      onMouseLeave={(e) => {
        (e.currentTarget as HTMLDivElement).style.opacity = "0.85";
      }}
    >
      {modes.map((m) => (
        <button
          key={m.id}
          data-e2e={`${dataE2ePrefix}-mode-${m.id}`}
          title={m.title ?? m.label}
          style={modeBtnStyle(m.id === value)}
          onClick={() => {
            if (m.id !== value) onChange(m.id);
          }}
        >
          {m.label}
        </button>
      ))}
    </div>
  );
}
