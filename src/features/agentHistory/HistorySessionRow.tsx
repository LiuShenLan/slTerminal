// HistorySessionRow.tsx — 历史会话行组件（FE-07，单行式）
//
// NAV-08 单行化改造（30px 行高，供导航树复用）：行 = 四态状态圆点（StatusDot，
// working/attention/done/error，运行中会话与活跃区同源）+ CLI logo 14px + 粗体标题
// （title 为 null 时显示 sessionId 前 8 位）+ 右侧相对时间（11px fg-4）；prompt 预览
// 不再渲染为第二行，改放行容器原生 title 属性（悬停 tooltip）。
// 状态标记（IC-03）：status 非 null → StatusDot 圆点（问题 2 修复：历史区与活跃区
// 四态同源）；orphan → 孤儿标记 IconClose（cwd 目录已删除，不可恢复，IC-08）；
// noCwd（无 cwd，调用方跳过孤儿判定，orphan 恒 false）不显示孤儿标记。
// 交互：单击选中 / 双击恢复分派 / 右键菜单，均回调 props 委托（纯受控展示组件，不碰 IPC）。
// props 签名保持兼容（HistorySessionList 调用零改动，NAV-08）。
// 契约要点见 src/features/agentHistory/CLAUDE.md。

import React from "react";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { AgentStatus } from "../../lib/agentStatus";
import { StatusDot } from "../../lib/StatusDot";
import { cliProfileRegistry } from "../cliProfiles";
import { formatRelativeTime } from "./historyModel";
import { EXPLORER_SELECTION_BG, SIDEBAR_COLORS, DIM_FG, ERROR_FG } from "../../theme";
import { IconClose } from "../../lib/icons";

/** 行组件契约（写死，见 src/features/agentHistory/CLAUDE.md 测试模式——agent B 照此消费） */
export interface HistorySessionRowProps {
  session: AgentHistorySession;
  /** 运行中会话四态状态（working/attention/done/error；null/undefined → 无标记） */
  status?: AgentStatus | null;
  /** 孤儿会话（cwd≠null 且 cwd 目录已删除，孤儿标记 IconClose） */
  orphan: boolean;
  /** 无 cwd 会话（cwd===null，不显示孤儿标记，恢复类操作禁用） */
  noCwd: boolean;
  /** 选中态（背景高亮 EXPLORER_SELECTION_BG） */
  selected: boolean;
  /** 单击选中 */
  onSelect(id: string): void;
  /** 双击恢复分派 */
  onDoubleClick(session: AgentHistorySession): void;
  /** 右键菜单（坐标 clientX/clientY） */
  onContextMenu(session: AgentHistorySession, pos: { x: number; y: number }): void;
}

export const HistorySessionRow: React.FC<HistorySessionRowProps> = ({
  session,
  status,
  orphan,
  selected,
  onSelect,
  onDoubleClick,
  onContextMenu,
}) => {
  // title 为 null（降级/无标题条目）→ 显示 sessionId 前 8 位
  const title = session.title ?? session.sessionId.slice(0, 8);
  const timeStr = formatRelativeTime(session.mtimeMs, Date.now());
  // 行 logo（MC-311）：按 session.cliId 查对应 profile 的 iconSrc——不同 CLI 同目录
  // 同组时经行级 logo 区分（MC-312）；未注册 cliId → undefined → 无 logo 不报错
  const logoSrc = cliProfileRegistry.get(session.cliId)?.iconSrc;

  return (
    <div
      data-e2e="agent-history-row"
      // NAV-08 单行化：prompt 预览改放行容器原生 title（悬停 tooltip）；null 时不设
      title={session.firstPrompt ?? undefined}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "4px",
        height: 30, // 会话行高 30px（NAV-01 契约，NAV-08 单行化）
        padding: "0 8px",
        cursor: "pointer",
        userSelect: "none",
        backgroundColor: selected ? EXPLORER_SELECTION_BG : "transparent",
      }}
      onClick={() => onSelect(session.sessionId)}
      onDoubleClick={() => onDoubleClick(session)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(session, { x: e.clientX, y: e.clientY });
      }}
    >
      {/* 四态状态圆点仅随 status 渲染（status 为 null / 孤儿行无圆点） */}
      {status != null && <StatusDot status={status} />}
      {/* CLI 品牌 logo 跟随会话名显示（F9 行为修订）：行存在即显示，
          不依赖状态圆点——孤儿行同样按 cliId 加图 */}
      {logoSrc && (
        <img src={logoSrc} width={14} height={14}
          style={{ flexShrink: 0, display: "block" }} alt="CLI 图标" />
      )}
      <span
        style={{
          flex: 1,
          fontSize: "12px",
          fontWeight: 500, // UI-205：bold → 500（字重仅 400/500）
          color: SIDEBAR_COLORS.fg,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {title}
      </span>
      {/* 孤儿标记 IconClose（12px 错误色）仅按 orphan prop 渲染；noCwd 时调用方
          不传 orphan=true（无 cwd 跳过孤儿判定）——IC-08：装饰字符清除 */}
      {orphan && (
        <span
          data-e2e="agent-history-orphan"
          style={{ flexShrink: 0, display: "flex", color: ERROR_FG }}
        >
          <IconClose size={12} />
        </span>
      )}
      {/* 右侧相对时间（11px fg-4） */}
      <span style={{ flexShrink: 0, fontSize: "11px", color: DIM_FG }}>
        {timeStr}
      </span>
    </div>
  );
};
