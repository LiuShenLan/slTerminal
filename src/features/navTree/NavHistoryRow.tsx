// NavHistoryRow.tsx —— 历史会话行（NAV-03 单行化）
//
// 单行 30px：StatusDot（运行中会话四态，activeStatuses 复合键 cliId|sessionId 查询；
// 无运行状态 → done 灰档——mockup .dot.idle 灰 = StatusDot done 档同色 PLACEHOLDER_FG，
// NAV-10 契约恒渲染圆点）+ CLI logo 14px（按 session.cliId 查 profile.iconSrc，MC-311）
// + 标题（fg-1）+ 右侧相对时间（11px fg-4，formatRelativeTime 与历史区口径统一）。
// prompt 预览 → 原生 title tooltip（决策 6——双行式行2 改造为 title 属性）。
// 标题 null → sessionId 前 8 位（原 HistorySessionRow（已删）同款）。
// 双击恢复三分支 / 右键菜单（复制恢复命令/分支恢复/删除）经回调委托 NavTree，
// 策略沿用 historyContextMenu（getHistoryContextMenuItems 直接引用）。

import React, { useState } from "react";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { AgentStatus } from "../../lib/agentStatus";
import { StatusDot } from "../../lib/StatusDot";
import { cliProfileRegistry } from "../cliProfiles";
import { formatRelativeTime } from "../agentHistory/historyModel";
import { PLACEHOLDER_FG, SIDEBAR_FG } from "../../theme";
import { nameStyle, rowBaseStyle, SESSION_ROW_HEIGHT } from "./navStyles";

interface NavHistoryRowProps {
  session: AgentHistorySession;
  /** 运行中会话四态（activeStatuses 复合键查询；null → 无圆点） */
  status?: AgentStatus | null;
  onDoubleClick(session: AgentHistorySession): void;
  onContextMenu(session: AgentHistorySession, pos: { x: number; y: number }): void;
}

export const NavHistoryRow: React.FC<NavHistoryRowProps> = ({
  session,
  status,
  onDoubleClick,
  onContextMenu,
}) => {
  const [hovered, setHovered] = useState(false);

  const title = session.title ?? session.sessionId.slice(0, 8);
  const timeStr = formatRelativeTime(session.mtimeMs, Date.now());
  // 行 logo（MC-311）：按 session.cliId 查 profile.iconSrc；未注册 → 无 logo 不报错
  const logoSrc = cliProfileRegistry.get(session.cliId)?.iconSrc;
  // 恒渲染圆点：无运行状态 → done 灰档（mockup .dot.idle，NAV-10 契约）
  const dotStatus: AgentStatus = status ?? "done";

  return (
    <div
      data-e2e="nav-row-session"
      onDoubleClick={() => onDoubleClick(session)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(session, { x: e.clientX, y: e.clientY });
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      title={session.firstPrompt ?? undefined} // prompt 预览 → 原生 title tooltip（决策 6）
      style={{
        ...rowBaseStyle(false, hovered, SESSION_ROW_HEIGHT),
        color: SIDEBAR_FG,
        fontSize: 12.5,
      }}
    >
      <StatusDot status={dotStatus} />
      {logoSrc && (
        <img
          src={logoSrc}
          width={14}
          height={14}
          style={{ flexShrink: 0, display: "block" }}
          alt="CLI 图标"
        />
      )}
      <span style={nameStyle}>{title}</span>
      <span style={{ flexShrink: 0, fontSize: 11, color: PLACEHOLDER_FG }}>
        {timeStr}
      </span>
    </div>
  );
};
