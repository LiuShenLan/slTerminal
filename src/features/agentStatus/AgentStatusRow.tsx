// AgentStatusRow.tsx — Agent 状态行组件
// 显示单个 Agent 会话的状态图标、标题、上下文用量条、最后事件时间。
// 点击行可通过 onFocus 跳转到对应终端面板。

import React, { useState, useCallback } from "react";
import type { AgentSessionRow } from "./useAgentStatus";
import { CLAUDE_CONTEXT_LIMIT } from "./consts";
import { getStatusIcon } from "../../lib/claudeStatus";
import { AGENT_STATUS_USAGE_COLORS, SIDEBAR_COLORS, DIM_FG } from "../../theme/colors";

interface Props {
  row: AgentSessionRow;
  onFocus: (panelId: string) => void;
}

/** 根据用量百分比返回对应分段颜色 */
function usageBarColor(percent: number): string {
  if (percent < 50) return AGENT_STATUS_USAGE_COLORS.low;
  if (percent <= 80) return AGENT_STATUS_USAGE_COLORS.medium;
  return AGENT_STATUS_USAGE_COLORS.high;
}

export const AgentStatusRow: React.FC<Props> = ({ row, onFocus }) => {
  const [hovered, setHovered] = useState(false);

  const handleClick = useCallback(() => {
    onFocus(row.panelId);
  }, [onFocus, row.panelId]);

  // ---- 用量计算（input + cacheRead + cacheCreation；output 不计占用，保留为信息字段） ----
  const total =
    row.usage != null
      ? row.usage.inputTokens +
        row.usage.cacheReadInputTokens +
        row.usage.cacheCreationInputTokens
      : 0;
  const percent = Math.min(100, (total / CLAUDE_CONTEXT_LIMIT) * 100);
  const usageAvailable = row.usage != null;

  // ---- 图标与时间 ----
  const icon = getStatusIcon(row.status);
  const timeStr = new Date(row.lastEventAt).toLocaleTimeString();

  // ---- 容器样式 ----
  const bgColor = hovered ? SIDEBAR_COLORS.hover : "transparent";

  return (
    <div
      data-e2e="agent-status-row"
      data-panel-id={row.panelId}
      style={{
        display: "flex",
        flexDirection: "row",
        alignItems: "center",
        padding: "4px 8px",
        gap: "8px",
        cursor: "pointer",
        backgroundColor: bgColor,
        userSelect: "none",
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={handleClick}
    >
      {/* 四态图标 */}
      <span
        style={{
          width: "20px",
          textAlign: "center",
          flexShrink: 0,
          fontSize: "14px",
        }}
      >
        {icon}
      </span>

      {/* 标题（超出截断） */}
      <span
        style={{
          flex: 1,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          color: SIDEBAR_COLORS.fg,
          fontSize: "13px",
        }}
      >
        {row.title}
      </span>

      {/* 上下文用量条 */}
      <div
        style={{
          width: "80px",
          height: "6px",
          borderRadius: "3px",
          backgroundColor: SIDEBAR_COLORS.border,
          flexShrink: 0,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            width: usageAvailable ? `${percent}%` : "100%",
            height: "100%",
            borderRadius: "3px",
            backgroundColor: usageAvailable
              ? usageBarColor(percent)
              : DIM_FG,
            transition: "width 0.3s ease",
          }}
        />
      </div>

      {/* 用量文本 */}
      <span
        style={{
          width: "36px",
          textAlign: "right",
          fontSize: "11px",
          color: usageAvailable ? SIDEBAR_COLORS.fg : DIM_FG,
          flexShrink: 0,
        }}
      >
        {usageAvailable ? `${Math.round(percent)}%` : "--"}
      </span>

      {/* 最后事件时间 */}
      <span
        style={{
          fontSize: "11px",
          color: SIDEBAR_COLORS.fg,
          opacity: 0.6,
          flexShrink: 0,
          marginLeft: "4px",
        }}
      >
        {timeStr}
      </span>
    </div>
  );
};
