// HistorySessionRow.tsx — 历史会话行组件（FE-07，双行式）
//
// 行1 = 四态状态标记（⚡🟡✅❌，运行中会话与活跃区同源）+ 粗体标题（title 为 null 时
// 显示 sessionId 前 8 位）+ 右上角相对时间；行2 = 首条 prompt 预览（单行截断省略）。
// 状态标记：status 非 null → STATUS_EMOJI 对应 emoji（问题 2 修复：历史区与活跃区
// 四态同源）；orphan → ✗（cwd 目录已删除，不可恢复）；noCwd（无 cwd，调用方跳过
// 孤儿判定，orphan 恒 false）不显示 ✗。
// 交互：单击选中 / 双击恢复分派 / 右键菜单，均回调 props 委托（纯受控展示组件，不碰 IPC）。
// 字号层级（问题 4）：行1 标题 12px 粗体 > 行2 11px 灰。
// 契约要点见 src/features/claudeHistory/CLAUDE.md。

import React from "react";
import type { HistorySession } from "../../types/claudeHistory";
import type { AgentStatus } from "../../lib/agentStatus";
import { STATUS_EMOJI } from "../../lib/agentStatus";
import { cliProfileRegistry } from "../cliProfiles";
import { CLAUDE_CLI_ID } from "../cliProfiles/profiles/claude";
import { formatRelativeTime } from "./historyModel";
import { EXPLORER_SELECTION_BG, SIDEBAR_COLORS, DIM_FG } from "../../theme";

/** 行组件契约（写死，见 src/features/claudeHistory/CLAUDE.md 测试模式——agent B 照此消费） */
export interface HistorySessionRowProps {
  session: HistorySession;
  /** 运行中会话四态（⚡🟡✅❌；null/undefined → 无标记） */
  status?: AgentStatus | null;
  /** 孤儿会话（cwd≠null 且 cwd 目录已删除，✗ 标记） */
  orphan: boolean;
  /** 无 cwd 会话（cwd===null，不显示 ✗，恢复类操作禁用） */
  noCwd: boolean;
  /** 选中态（背景高亮 EXPLORER_SELECTION_BG） */
  selected: boolean;
  /** 单击选中 */
  onSelect(id: string): void;
  /** 双击恢复分派 */
  onDoubleClick(session: HistorySession): void;
  /** 右键菜单（坐标 clientX/clientY） */
  onContextMenu(session: HistorySession, pos: { x: number; y: number }): void;
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
  const statusIcon = status != null ? (STATUS_EMOJI[status] ?? "") : "";
  // 过渡形态：HistorySession 暂无 cliId 字段（Stage 05 MC-311 数据侧就绪后回收），暂取 claude profile 的 iconSrc；
  // 未命中（claude profile 未注册）→ undefined → 无 logo 不报错（与原 cliIconRegistry.getSrc 语义一致）
  const logoSrc = cliProfileRegistry.get(CLAUDE_CLI_ID)?.iconSrc;

  return (
    <div
      data-e2e="agent-history-row"
      style={{
        display: "flex",
        flexDirection: "column",
        padding: "4px 8px",
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
      {/* 行1：四态标记 + 粗体标题 + 右上角相对时间（12px 层级） */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "4px",
          fontSize: "12px",
          color: SIDEBAR_COLORS.fg,
        }}
      >
        {/* CLI 品牌 logo 仅随 status emoji 渲染（status 为 null / 孤儿 ✗ 行不加图） */}
        {statusIcon && (
          <>
            <span style={{ flexShrink: 0 }}>{statusIcon}</span>
            {logoSrc && (
              <img src={logoSrc} width={16} height={16}
                style={{ flexShrink: 0, display: "block" }} alt="CLI 图标" />
            )}
          </>
        )}
        <span
          style={{
            flex: 1,
            fontWeight: "bold",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {title}
        </span>
        {/* ✗ 仅按 orphan prop 渲染；noCwd 时调用方不传 orphan=true（无 cwd 跳过孤儿判定） */}
        {orphan && (
          <span style={{ flexShrink: 0, fontSize: "11px", color: DIM_FG }}>
            ✗
          </span>
        )}
        <span style={{ flexShrink: 0, fontSize: "11px", color: DIM_FG }}>
          {timeStr}
        </span>
      </div>

      {/* 行2：首条 prompt 预览（单行截断省略；null 时不渲染，11px 层级） */}
      {session.firstPrompt != null && (
        <div
          style={{
            fontSize: "11px",
            color: DIM_FG,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {session.firstPrompt}
        </div>
      )}
    </div>
  );
};
