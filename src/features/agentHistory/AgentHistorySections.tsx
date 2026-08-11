// AgentHistorySections.tsx — 历史区组合件（FE-07 集成部分 + FE-09 空态）
//
// 结构（自上而下，README 4.3.4/4.3.5）：
//   搜索框（agent-history-search）—— 过滤两区当前展开的列表（标题 + 首条 prompt，大小写不敏感）
//   刷新按钮（agent-history-refresh）—— 手动全量重扫
//   当前项目历史会话（agent-history-section-current）—— rootPath 过滤平铺（HistorySessionList current）
//   全部项目历史会话（agent-history-section-all）—— groupByCwd 二级折叠（HistorySessionList all）
//
// 受控组件（问题 6 修复：useAgentHistory 上提至 AgentStatusView 单实例，本组件经 props
// 接收全部数据与回调——刷新后活跃区标题同步依赖此数据通路）。
// 展开/收起 state 受控（AgentStatusView 持有三区展开态，本组件经 props 接收）；
// 历史区首次展开触发 scan()（仅首次，之后靠刷新按钮——README 4.3.5）；
// 选中态 selectedId 由本组件持有。
//
// 字号层级（问题 4）：区块标题 13px 粗体（折叠框名）> 行标题 12px > 第二行 11px；
// 区块内容缩进 12px + 左侧树形引导线。
//
// 空态文案（FE-09）：当前项目区空 →「该项目暂无历史会话」；全部项目区空 →「暂无历史会话」；
// 无活跃项目（rootPath null）→ 当前项目区「无活跃项目」；搜索无结果 →「无匹配的会话」。
// 配色全部 theme/colors.ts token（硬约束 #6）。

import React, { useEffect, useRef, useState } from "react";
import { HistorySessionList } from "./HistorySessionList";
import type { AgentHistoryState } from "./useAgentHistory";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { AgentStatus } from "../../lib/agentStatus";
import {
  EXPLORER_COLORS,
  INPUT_BG,
  INPUT_BORDER,
  SIDEBAR_FG,
  SIDEBAR_COLORS,
} from "../../theme";

export interface AgentHistorySectionsProps {
  /** 当前项目历史会话区展开态（受控，AgentStatusView 持有） */
  expandedCurrent: boolean;
  /** 全部项目历史会话区展开态（受控，AgentStatusView 持有） */
  expandedAll: boolean;
  /** 切换当前项目区展开 */
  onToggleCurrent(): void;
  /** 切换全部项目区展开 */
  onToggleAll(): void;
  // ── 数据与回调（useAgentHistory 上提至 AgentStatusView 后经 props 注入） ──
  /** 扫描状态机（idle/loading/ready/error） */
  historyState: AgentHistoryState;
  /** 全部历史会话（扫描结果） */
  sessions: AgentHistorySession[];
  /** 运行中会话四态映射（Map<cliId|sessionId, status>，复合键 MC-313——键构造经 historyModel.keyOf 单点） */
  activeStatuses: Map<string, AgentStatus>;
  /** 当前项目 rootPath（null → 当前项目区显示「无活跃项目」） */
  rootPath: string | null;
  /** 手动全量重扫 */
  scan(): void;
  /** 删除成功后的即时局部刷新（不重扫） */
  removeLocal(id: string): void;
}

/** 折叠箭头样式 */
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

/** 区块标题栏样式（13px 粗体——问题 4 折叠框名层级） */
const sectionHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "2px 8px",
  height: 22,
  cursor: "pointer",
  userSelect: "none",
  fontSize: 13,
  fontWeight: "bold",
  color: EXPLORER_COLORS.fg,
};

/** 区块内容容器（缩进 12px + 左侧树形引导线——问题 4） */
const sectionBodyStyle: React.CSSProperties = {
  paddingLeft: 12,
  borderLeft: `1px solid ${SIDEBAR_COLORS.treeGuide}`,
  marginLeft: 7, // 对齐区块标题箭头右侧内容
};

/** 搜索行（搜索框 + 刷新按钮，位于两个历史下拉框之上） */
const searchRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 4,
  padding: "4px 8px",
};

/** 搜索输入框 */
const searchInputStyle: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  background: INPUT_BG,
  color: SIDEBAR_FG,
  border: `1px solid ${INPUT_BORDER}`,
  borderRadius: 4,
  padding: "2px 6px",
  fontSize: 12,
  outline: "none",
};

/** 刷新按钮 */
const refreshButtonStyle: React.CSSProperties = {
  background: "transparent",
  color: SIDEBAR_FG,
  border: `1px solid ${INPUT_BORDER}`,
  borderRadius: 4,
  padding: "2px 8px",
  fontSize: 12,
  cursor: "pointer",
  flexShrink: 0,
};

/** 空态/提示文案样式 */
const emptyHintStyle: React.CSSProperties = {
  padding: "4px 8px",
  color: INPUT_BORDER,
  fontSize: 11,
  fontStyle: "italic",
  userSelect: "none",
};

export const AgentHistorySections: React.FC<AgentHistorySectionsProps> = ({
  expandedCurrent,
  expandedAll,
  onToggleCurrent,
  onToggleAll,
  historyState,
  sessions,
  activeStatuses,
  rootPath,
  scan,
  removeLocal,
}) => {
  const [search, setSearch] = useState("");
  /** 选中会话 id（本组件持有，行单击选中高亮） */
  const [selectedId, setSelectedId] = useState<string | null>(null);

  /** 首次展开触发标记——仅首次展开时 scan()，之后靠刷新按钮（README 4.3.5） */
  const scanTriggeredRef = useRef(false);
  useEffect(() => {
    if ((expandedCurrent || expandedAll) && !scanTriggeredRef.current) {
      scanTriggeredRef.current = true;
      scan();
    }
  }, [expandedCurrent, expandedAll, scan]);

  const loading = historyState === "loading";

  return (
    <div>
      {/* 搜索框 + 刷新按钮（位于两个历史下拉框之上，README 4.3.4/4.3.5） */}
      <div style={searchRowStyle}>
        <input
          data-e2e="agent-history-search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="搜索历史会话…"
          style={searchInputStyle}
        />
        <button
          data-e2e="agent-history-refresh"
          onClick={() => scan()}
          style={refreshButtonStyle}
        >
          刷新
        </button>
      </div>

      {/* 区块：当前项目历史会话（rootPath 为 null →「无活跃项目」，FE-09 场景 7） */}
      <div data-e2e="agent-history-section-current">
        <div style={sectionHeaderStyle} onClick={onToggleCurrent}>
          <span style={arrowStyle}>{expandedCurrent ? "▼" : "▶"}</span>
          <span>当前项目历史会话</span>
        </div>
        {expandedCurrent && (
          <div style={sectionBodyStyle}>
            {rootPath === null ? (
              <div style={emptyHintStyle}>无活跃项目</div>
            ) : loading ? (
              <div style={emptyHintStyle}>扫描中…</div>
            ) : (
              <HistorySessionList
                mode="current"
                sessions={sessions}
                rootPath={rootPath}
                search={search}
                activeStatuses={activeStatuses}
                selectedId={selectedId}
                onSelect={setSelectedId}
                removeLocal={removeLocal}
              />
            )}
          </div>
        )}
      </div>

      {/* 区块：全部项目历史会话 */}
      <div data-e2e="agent-history-section-all">
        <div style={sectionHeaderStyle} onClick={onToggleAll}>
          <span style={arrowStyle}>{expandedAll ? "▼" : "▶"}</span>
          <span>全部项目历史会话</span>
        </div>
        {expandedAll && (
          <div style={sectionBodyStyle}>
            {loading ? (
              <div style={emptyHintStyle}>扫描中…</div>
            ) : (
              <HistorySessionList
                mode="all"
                sessions={sessions}
                rootPath={rootPath}
                search={search}
                activeStatuses={activeStatuses}
                selectedId={selectedId}
                onSelect={setSelectedId}
                removeLocal={removeLocal}
              />
            )}
          </div>
        )}
      </div>
    </div>
  );
};
