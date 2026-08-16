// NavSessionRow.tsx —— 活跃会话行（NAV-02 / UI-504 / 决策 6 单行化）
//
// 行结构（单行 30px）：StatusDot（F3 四态，null 不渲染）+ CLI logo 14px
// （cliProfileRegistry iconSrc，照 AgentStatusRow 逻辑——按行 cliId 查 profile，
// 未注册 cliId → 无 logo 不报错；F9 行为修订：行存在即显示）+ 标题（fg-1）
// + 右侧迷你用量条（32x3）+ 百分比（11px fg-4）。
// 用量口径不变：computeUsagePercent 经 profile.hooks 委托（claude = 官方
// used_percentage 取整 + 钳位；profile 缺失/无 hooks 能力/无数据 → "--"）；
// 四档分级 ≥90/≥70/≥50 逻辑不变（AGENT_STATUS_USAGE_COLORS）。
// 活跃会话（页面级最近事件行）默认 accent-dim 选中底（设计 6.3），hover → SELECTION_HOVER_BG。
// 点击行 → 聚焦对应终端页签（onFocus 委托 NavTree，B14 前缀解析兜底）。

import React, { useCallback, useState } from "react";
import type { AgentSessionRow } from "../agentStatus/useAgentStatus";
import { StatusDot } from "../../lib/StatusDot";
import { cliProfileRegistry } from "../cliProfiles";
import {
  AGENT_STATUS_USAGE_COLORS,
  DIM_FG,
  PLACEHOLDER_FG,
  SIDEBAR_COLORS,
  SIDEBAR_FG,
} from "../../theme";
import { nameStyle, rowBaseStyle, SESSION_ROW_HEIGHT } from "./navStyles";

interface NavSessionRowProps {
  row: AgentSessionRow;
  /** 活跃会话标记（accent-dim 选中底） */
  active: boolean;
  onFocus(panelId: string): void;
}

/** 用量条分段色（四档：≥90 critical / ≥70 high / ≥50 medium / else low——逻辑照 AgentStatusRow 不变） */
function usageBarColor(percent: number): string {
  if (percent >= 90) return AGENT_STATUS_USAGE_COLORS.critical;
  if (percent >= 70) return AGENT_STATUS_USAGE_COLORS.high;
  if (percent >= 50) return AGENT_STATUS_USAGE_COLORS.medium;
  return AGENT_STATUS_USAGE_COLORS.low;
}

export const NavSessionRow: React.FC<NavSessionRowProps> = ({
  row,
  active,
  onFocus,
}) => {
  const [hovered, setHovered] = useState(false);

  // 用量百分比（profile 能力域委托——照 AgentStatusRow 口径不变）
  const percent =
    cliProfileRegistry
      .get(row.cliId)
      ?.capabilities?.hooks?.computeUsagePercent(row.usage) ?? null;

  // CLI 品牌 logo（F9）：按行 cliId 查 profile.iconSrc；未注册 → 无 logo 不报错
  const logoSrc = cliProfileRegistry.get(row.cliId)?.iconSrc;

  const handleClick = useCallback(
    () => onFocus(row.panelId),
    [onFocus, row.panelId],
  );

  return (
    <div
      data-e2e="nav-row-session"
      data-panel-id={row.panelId}
      onClick={handleClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        ...rowBaseStyle(active, hovered, SESSION_ROW_HEIGHT),
        color: SIDEBAR_FG, // 会话行标题 fg-1（mockup .row.sess .nm）
        fontSize: 12.5,
      }}
    >
      {row.status != null && <StatusDot status={row.status} />}
      {logoSrc && (
        <img
          src={logoSrc}
          width={14}
          height={14}
          style={{ flexShrink: 0, display: "block" }}
          alt="CLI 图标"
        />
      )}
      <span style={nameStyle}>{row.title}</span>
      {/* 右侧：32x3 迷你用量条 + 百分比（11px fg-4——决策 6 契约；轨道用 div 承载，
          NAV-10 契约测试按 div 查询 32x3 容器） */}
      <div
        style={{
          width: 32,
          height: 3,
          borderRadius: 999, // GL-03：3px 高条形全圆角（pill 档，视觉等价 1.5）
          backgroundColor: SIDEBAR_COLORS.border,
          flexShrink: 0,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            width: percent != null ? `${percent}%` : "100%",
            height: "100%",
            borderRadius: 999, // GL-03：同上 pill 档
            backgroundColor:
              percent != null ? usageBarColor(percent) : PLACEHOLDER_FG,
          }}
        />
      </div>
      <span
        style={{
          width: 30,
          textAlign: "right",
          flexShrink: 0,
          fontSize: 11,
          color: DIM_FG, // 11px fg-4（NAV-10 契约映射——mockup fg 层级下最弱元数据档）
        }}
      >
        {percent != null ? `${percent}%` : "--"}
      </span>
    </div>
  );
};
