// AgentStatusView.tsx — agent 状态侧栏视图主组件
//
// 状态机（优先级自上而下）：
//   no-root → "选择一个项目以查看 Agent 状态"
//   empty   → "当前项目无运行中的 claude 会话"
//   ready   → 渲染行列表
//
// 标题栏 "AGENT STATUS"（28px 高、大写、letterSpacing 1、fontSize 11）
// 样式照 CommitView.tsx。

import React, { useCallback } from "react";
import type { SideViewComponentProps } from "../sideViews/sideViewRegistry";
import { useAgentStatus } from "./useAgentStatus";
import { AgentStatusRow } from "./AgentStatusRow";
import { switchToPageAndFocus } from "../../workspace/pageApis";
import {
  SEPARATOR_BG,
  INPUT_BORDER,
  PANEL_BG,
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

/** 状态提示居中样式 */
const centerHintStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  flex: 1,
  padding: 16,
  color: INPUT_BORDER,
  fontSize: 12,
  textAlign: "center",
  userSelect: "none",
};

/** 列表区域滚动容器 */
const listContainerStyle: React.CSSProperties = {
  flex: 1,
  overflowY: "auto",
  overflowX: "hidden",
  padding: "2px 0",
};

export const AgentStatusView: React.FC<SideViewComponentProps> = (_props) => { // eslint-disable-line @typescript-eslint/no-unused-vars -- SideViewComponentProps 必需但 handleFocus 已委托共享函数
  const { state, rows } = useAgentStatus();

  // 点击行 → 解析 pageId → switchToPageAndFocus（共享函数处理切换+聚焦）
  const handleFocus = useCallback(
    async (panelId: string) => {
      // 解析 pageId：格式 terminal-{pageId}-{seq}（Stage 02 收敛到 src/lib/panelId.ts）
      const withoutPrefix = panelId.startsWith("terminal-")
        ? panelId.slice("terminal-".length)
        : panelId;
      const lastDash = withoutPrefix.lastIndexOf("-");
      const pageId =
        lastDash > 0 && /^\d+$/.test(withoutPrefix.slice(lastDash + 1))
          ? withoutPrefix.slice(0, lastDash)
          : withoutPrefix;

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
        overflow: "hidden",
      }}
    >
      {/* 标题栏 */}
      <div style={headerStyle}>AGENT STATUS</div>

      {/* 状态机渲染 */}
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
  );
};
