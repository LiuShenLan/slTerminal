// AgentStatusView.tsx — agent 状态侧栏视图主组件（FE-08 三下拉框改造）
//
// 三个可展开/收起区块（默认态：活跃展开、两历史区收起）：
//   1. 活跃会话 —— 现有逻辑零改动（useAgentStatus + AgentStatusRow，import 与行渲染原样保留）
//   2. 当前项目历史会话 —— ClaudeHistorySections 受控区（挂载其对应部分）
//   3. 全部项目历史会话 —— ClaudeHistorySections 受控区
// 三区展开 state 由本组件持有并下传；历史区首次展开触发 scan()（ClaudeHistorySections 内部，
// 仅首次，之后靠刷新按钮）。整视图可滚动。
//
// 状态机（活跃区，优先级自上而下，原文保留）：
//   no-root → "选择一个项目以查看 Agent 状态"
//   empty   → "当前项目无运行中的 claude 会话"
//   ready   → 渲染行列表
//
// E2E 兼容红线（逐字保留）：根容器 data-e2e="agent-status-view"、活跃行
// data-e2e="agent-status-row"（AgentStatusRow.tsx 内）、标题栏 "AGENT STATUS"、
// 空态文案「选择一个项目」「无运行中的 claude 会话」。
// 标题栏（28px 高、大写、letterSpacing 1、fontSize 11）样式照 CommitView.tsx。

import React, { useCallback, useState } from "react";
import type { SideViewComponentProps } from "../sideViews/sideViewRegistry";
import { useAgentStatus } from "./useAgentStatus";
import { AgentStatusRow } from "./AgentStatusRow";
import { ClaudeHistorySections } from "../claudeHistory/ClaudeHistorySections";
import { switchToPageAndFocus } from "../../workspace/pageApis";
import { parseTerminalPageId } from "../../lib/panelId";
import {
  SEPARATOR_BG,
  INPUT_BORDER,
  PANEL_BG,
  EXPLORER_COLORS,
} from "../../theme";

/** 标题栏样式（照 CommitView.tsx） */
const headerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "4px 8px",
  borderBottom: `1px solid ${SEPARATOR_BG}`,
  height: 28,
  fontSize: 11,
  color: INPUT_BORDER,
  textTransform: "uppercase",
  letterSpacing: 1,
  userSelect: "none",
  flexShrink: 0,
};

/** 区块标题栏（可点击展开/收起） */
const sectionHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "2px 8px",
  height: 22,
  cursor: "pointer",
  userSelect: "none",
  fontSize: 12,
  color: EXPLORER_COLORS.fg,
};

/** 折叠箭头 */
const arrowStyle: React.CSSProperties = {
  display: "inline-block",
  width: 16,
  fontSize: 10,
  color: EXPLORER_COLORS.arrowClosed,
  textAlign: "center",
  lineHeight: "22px",
  userSelect: "none",
  flexShrink: 0,
};

/** 状态提示样式 */
const centerHintStyle: React.CSSProperties = {
  padding: 12,
  color: INPUT_BORDER,
  fontSize: 12,
  textAlign: "center",
  userSelect: "none",
};

/** 活跃会话行列表区 */
const listContainerStyle: React.CSSProperties = {
  padding: "2px 0",
};

export const AgentStatusView: React.FC<SideViewComponentProps> = (_props) => { // eslint-disable-line @typescript-eslint/no-unused-vars -- SideViewComponentProps 必需但 handleFocus 已委托共享函数
  const { state, rows } = useAgentStatus();

  // 三区展开 state（默认态：活跃展开、两历史区收起）
  const [activeExpanded, setActiveExpanded] = useState(true);
  const [currentExpanded, setCurrentExpanded] = useState(false);
  const [allExpanded, setAllExpanded] = useState(false);

  // 点击行 → 解析 pageId → switchToPageAndFocus（共享函数处理切换+聚焦）
  const handleFocus = useCallback(
    async (panelId: string) => {
      const pageId = parseTerminalPageId(panelId);
      if (!pageId) return;
      await switchToPageAndFocus(pageId, panelId);
    },
    [],
  );

  return (
    <div
      data-e2e="agent-status-view"
      style={{
        width: "100%",
        height: "100%",
        background: PANEL_BG,
        display: "flex",
        flexDirection: "column",
        overflowY: "auto",
        overflowX: "hidden",
      }}
    >
      {/* 标题栏 */}
      <div style={headerStyle}>AGENT STATUS</div>

      {/* 区块 1：活跃会话（默认展开）——现有逻辑零改动 */}
      <div>
        <div
          style={sectionHeaderStyle}
          onClick={() => setActiveExpanded((v) => !v)}
        >
          <span style={arrowStyle}>{activeExpanded ? "▼" : "▶"}</span>
          <span>活跃会话</span>
        </div>
        {activeExpanded && (
          <div>
            {state.kind === "no-root" && (
              <div style={centerHintStyle}>选择一个项目以查看 Agent 状态</div>
            )}
            {state.kind === "empty" && (
              <div style={centerHintStyle}>
                当前项目无运行中的 claude 会话
              </div>
            )}
            {state.kind === "ready" && (
              <div style={listContainerStyle}>
                {rows.map((row) => (
                  <AgentStatusRow
                    key={row.panelId}
                    row={row}
                    onFocus={handleFocus}
                  />
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {/* 区块 2+3：历史区（受控展开；首次展开触发 scan() 在 ClaudeHistorySections 内部） */}
      <ClaudeHistorySections
        expandedCurrent={currentExpanded}
        expandedAll={allExpanded}
        onToggleCurrent={() => setCurrentExpanded((v) => !v)}
        onToggleAll={() => setAllExpanded((v) => !v)}
      />
    </div>
  );
};
