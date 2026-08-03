// AgentStatusRow.tsx — Agent 状态行组件（双行式）
// 行1 = 状态图标 + 标题（12px 粗体，截断）；行2 = 上下文用量条 + 百分比 + 相对时间
// （11px 灰，缩进对齐图标列）。点击行可通过 onFocus 跳转到对应终端面板。
// 时间口径与历史区统一（formatRelativeTime 相对时间，问题 1 修复——旧为
// toLocaleTimeString 同行挤压导致窄侧栏遮挡）。

import React, { useState, useCallback } from "react";
import type { AgentSessionRow } from "./useAgentStatus";
import { CLAUDE_CONTEXT_LIMIT } from "./consts";
import { getStatusIcon } from "../../lib/claudeStatus";
import { formatRelativeTime } from "../claudeHistory/historyModel";
import { AGENT_STATUS_USAGE_COLORS, SIDEBAR_COLORS, DIM_FG } from "../../theme/colors";

interface Props {
  row: AgentSessionRow;
  onFocus: (panelId: string) => void;
  /** 相对时间基准（60s ticker 驱动重算；缺省回退 Date.now()，向后兼容） */
  now?: number;
}

/** 根据用量百分比返回对应分段颜色 */
function usageBarColor(percent: number): string {
  if (percent < 50) return AGENT_STATUS_USAGE_COLORS.low;
  if (percent <= 80) return AGENT_STATUS_USAGE_COLORS.medium;
  return AGENT_STATUS_USAGE_COLORS.high;
}

export const AgentStatusRow: React.FC<Props> = ({ row, onFocus, now }) => {
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

  // ---- 图标与时间（相对时间，与历史区口径统一；now 由 60s ticker 驱动重算） ----
  const icon = getStatusIcon(row.status);
  const timeStr = formatRelativeTime(row.lastEventAt, now ?? Date.now());

  // ---- 容器样式 ----
  const bgColor = hovered ? SIDEBAR_COLORS.hover : "transparent";

  return (
    <div
      data-e2e="agent-status-row"
      data-panel-id={row.panelId}
      style={{
        display: "flex",
        flexDirection: "column",
        padding: "4px 8px",
        gap: "2px",
        cursor: "pointer",
        backgroundColor: bgColor,
        userSelect: "none",
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={handleClick}
    >
      {/* 行1：四态图标 + 标题（12px 粗体，超出截断） */}
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
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
        <span
          style={{
            flex: 1,
            fontWeight: "bold",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            color: SIDEBAR_COLORS.fg,
            fontSize: "12px",
          }}
        >
          {row.title}
        </span>
      </div>

      {/* 行2：上下文用量条 + 百分比 + 相对时间（11px 灰，缩进对齐图标列） */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "8px",
          paddingLeft: "28px", // 对齐行1图标列（icon 20 + gap 8）
        }}
      >
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
        <span
          style={{
            fontSize: "11px",
            color: DIM_FG,
            flexShrink: 0,
          }}
        >
          {timeStr}
        </span>
      </div>
    </div>
  );
};
