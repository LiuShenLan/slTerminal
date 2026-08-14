// AgentStatusRow.tsx — Agent 状态行组件（双行式）
// 行1 = 状态图标 + 标题（12px 粗体，截断）；行2 = 上下文用量条 + 百分比 + 相对时间
// （11px 灰，缩进对齐图标列）。点击行可通过 onFocus 跳转到对应终端面板。
// 时间口径与历史区统一（formatRelativeTime 相对时间，问题 1 修复——旧为
// toLocaleTimeString 同行挤压导致窄侧栏遮挡）。

import React, { useState, useCallback } from "react";
import type { AgentSessionRow } from "./useAgentStatus";
import { getStatusIcon } from "../../lib/agentStatus";
import { cliProfileRegistry } from "../cliProfiles";
import { formatRelativeTime } from "../agentHistory/historyModel";
import { AGENT_STATUS_USAGE_COLORS, SIDEBAR_COLORS, DIM_FG } from "../../theme/colors";

interface Props {
  row: AgentSessionRow;
  onFocus: (panelId: string) => void;
  /** 相对时间基准（60s ticker 驱动重算；缺省回退 Date.now()，向后兼容） */
  now?: number;
}

/** 根据用量百分比返回对应分段颜色（四档：≥90 红 / ≥70 橙 / ≥50 黄 / else 绿） */
function usageBarColor(percent: number): string {
  if (percent >= 90) return AGENT_STATUS_USAGE_COLORS.critical;
  if (percent >= 70) return AGENT_STATUS_USAGE_COLORS.high;
  if (percent >= 50) return AGENT_STATUS_USAGE_COLORS.medium;
  return AGENT_STATUS_USAGE_COLORS.low;
}

export const AgentStatusRow: React.FC<Props> = ({ row, onFocus, now }) => {
  const [hovered, setHovered] = useState(false);

  const handleClick = useCallback(() => {
    onFocus(row.panelId);
  }, [onFocus, row.panelId]);

  // ---- 用量百分比（profile 能力域委托——claude = 官方 used_percentage 取整+钳位） ----
  // 口径变更：原 transcript token 累加 ÷ 硬编码 contextLimit 已退役——官方
  // used_percentage 经 statusline 桥接信号（ContextUsage 事件）推送；
  // profile 缺失或无 hooks 能力 / 无数据 → 用量 "--"（现状语义保留）
  const percent =
    cliProfileRegistry.get(row.cliId)?.capabilities?.hooks?.computeUsagePercent(
      row.usage,
    ) ?? null;

  // ---- 图标与时间（相对时间，与历史区口径统一；now 由 60s ticker 驱动重算） ----
  const icon = getStatusIcon(row.status);
  // MC-411：CLI 品牌 logo 按行 cliId 查 profile.iconSrc（OSC 133-only 行同样有 cliId）；
  // 未注册 cliId → undefined → 无 logo 不报错。
  // F9 行为修订：logo 跟随会话名显示——行存在即显示，不依赖 icon（status=null 行同样有 logo）
  const logoSrc = cliProfileRegistry.get(row.cliId)?.iconSrc;
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
      {/* 行1：四态图标 + CLI logo + 标题（12px 粗体，超出截断） */}
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <span
          style={{
            width: "40px",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: "4px", // 列内收紧：emoji 与 logo 读作一个图标簇
            flexShrink: 0,
            fontSize: "14px",
          }}
        >
          {icon}
          {/* CLI 品牌 logo：跟随会话名显示（行存在即显示，F9 行为修订）；
              icon 为空时仍渲染空列占位（对齐恒定不漂移），logo 独立于 emoji */}
          {logoSrc && (
            <img src={logoSrc} width={16} height={16}
              style={{ flexShrink: 0, display: "block" }} alt="CLI 图标" />
          )}
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
          paddingLeft: "48px", // 对齐行1图标列（icon 40 + gap 8）
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
              width: percent != null ? `${percent}%` : "100%",
              height: "100%",
              borderRadius: "3px",
              backgroundColor:
                percent != null ? usageBarColor(percent) : DIM_FG,
              transition: "width 0.3s ease",
            }}
          />
        </div>
        <span
          style={{
            width: "36px",
            textAlign: "right",
            fontSize: "11px",
            color: percent != null ? SIDEBAR_COLORS.fg : DIM_FG,
            flexShrink: 0,
          }}
        >
          {percent != null ? `${percent}%` : "--"}
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
