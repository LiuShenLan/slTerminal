// EventTree.tsx — hooks 配置事件树（P3-FE-13）
//
// 三级树：事件分组（可折叠）→ 事件名 → matcher 组 → handler 摘要。
// 分组与事件元数据从 eventsCatalog.ts 读取（Stage 02 已建）；仅渲染配置中
// 已存在的事件，按目录分组顺序归组（未知事件归「未知事件」附加组，round-trip 不丢）。
// 各事件行显示 hook 总数（跨 matcher 组求和）。
// 选中态高亮引用 theme/colors.ts token（硬约束 #6）。
// 注入段条目（isSltermManaged 命中，configModel.ts 已建）显示「slTerminal 托管」
// 标记（C13-8）。

import React, { useState } from "react";
import type { HookEventGui, HookHandlerGui, HooksConfigGui } from "./configModel";
import { isSltermManaged, isHandlerDisabled, UNKNOWN_EVENT_GROUP } from "./configModel";
import { EVENT_GROUPS } from "./eventsCatalog";
import type { DisabledHookKey } from "../../types/hooksConfig";
import {
  PANEL_BG,
  SIDEBAR_FG,
  HTML_PANEL_LOADING_FG,
  ACTIVE_SELECTION_BG,
  ERROR_FG,
  DIM_FG,
  INPUT_BORDER,
} from "../../theme";

/** 事件树 props */
export interface EventTreeProps {
  /** GUI 模型（配置中已存在的事件） */
  gui: HooksConfigGui;
  /** 当前选中事件（null = 未选中） */
  selectedEvent: string | null;
  /** 事件选中回调 */
  onSelect: (event: string) => void;
  /** 当前层禁用记录（P3-FE-19，useHooksConfig 已按层过滤）——启停 checkbox + 置灰/删除线 */
  disabledKeys?: readonly DisabledHookKey[];
  /** 启停切换回调（key 不含 layer，由 useHooksConfig 补全当前层） */
  onToggleDisabled?: (key: Omit<DisabledHookKey, "layer">) => void;
}

/** 摘要文本最长长度（超出截断加省略号） */
const SUMMARY_MAX = 40;

/** handler 摘要文本（type + 主字段，照 HandlerForm 字段矩阵语义） */
export function formatHandlerSummary(handler: HookHandlerGui): string {
  const truncate = (s: string) => (s.length > SUMMARY_MAX ? `${s.slice(0, SUMMARY_MAX)}…` : s);
  switch (handler.type) {
    case "command":
      return `command: ${truncate(handler.command ?? "")}`;
    case "http":
      return `http: ${truncate(handler.url ?? "")}`;
    case "mcp_tool":
      return `mcp_tool: ${handler.server ?? ""}/${handler.tool ?? ""}`;
    case "prompt":
      return `prompt: ${truncate(handler.prompt ?? "")}`;
    case "agent":
      return `agent: ${truncate(handler.prompt ?? "")}`;
  }
}

/** 树容器样式（宽 260px，右侧边框分隔，自身滚动） */
const treeStyle: React.CSSProperties = {
  width: 260,
  flexShrink: 0,
  overflowY: "auto",
  borderRight: `1px solid ${INPUT_BORDER}`,
  background: PANEL_BG,
  padding: "6px 8px",
  boxSizing: "border-box",
};

/** 分组标题样式（可折叠） */
const groupStyle: React.CSSProperties = {
  fontSize: 11,
  color: HTML_PANEL_LOADING_FG,
  margin: "8px 0 4px",
  cursor: "pointer",
  userSelect: "none",
};

/** 事件行样式（选中态高亮，token 引用） */
function eventRowStyle(selected: boolean): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: "2px 6px",
    fontSize: 12,
    cursor: "pointer",
    borderRadius: 3,
    color: SIDEBAR_FG,
    background: selected ? ACTIVE_SELECTION_BG : "transparent",
  };
}

/** matcher 组行样式（缩进一级，弱化色） */
const matcherRowStyle: React.CSSProperties = {
  padding: "1px 6px 1px 20px",
  fontSize: 11,
  color: HTML_PANEL_LOADING_FG,
  userSelect: "none",
};

/** handler 摘要行样式（缩进两级，单行省略） */
const handlerRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "1px 6px 1px 36px",
  fontSize: 11,
  color: SIDEBAR_FG,
  userSelect: "none",
};

/** 禁用条目行样式（P3-FE-19 视觉区分：置灰 + 文字删除线） */
const disabledHandlerRowStyle: React.CSSProperties = {
  ...handlerRowStyle,
  color: DIM_FG,
};

/** 启停 checkbox 样式（flexShrink:0 防摘要截断挤压） */
const toggleStyle: React.CSSProperties = {
  flexShrink: 0,
  cursor: "pointer",
  margin: 0,
};

/** 「slTerminal 托管」标记样式（C13-8 注入段标识） */
const managedBadgeStyle: React.CSSProperties = {
  fontSize: 10,
  color: ERROR_FG,
  border: `1px solid ${ERROR_FG}`,
  borderRadius: 3,
  padding: "0 4px",
  flexShrink: 0,
};

/** 事件 hook 总数（跨 matcher 组求和） */
function eventHookCount(ev: HookEventGui): number {
  return ev.matcherGroups.reduce((n, g) => n + g.handlers.length, 0);
}

const EventTree: React.FC<EventTreeProps> = ({
  gui,
  selectedEvent,
  onSelect,
  disabledKeys = [],
  onToggleDisabled = () => {},
}) => {
  // 折叠分组集合（默认全部展开；key = 分组名）
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const toggleGroup = (group: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(group)) {
        next.delete(group);
      } else {
        next.add(group);
      }
      return next;
    });
  };

  // 分组归类：目录十组顺序 + 未知事件附加组（round-trip 不丢事件）
  const groups: { group: string; events: HookEventGui[] }[] = EVENT_GROUPS.map((g) => ({
    group: g,
    events: [],
  }));
  for (const e of gui.events) {
    const target = groups.find((g) => g.group === e.group);
    if (target) {
      target.events.push(e);
    } else {
      let unknown = groups.find((g) => g.group === UNKNOWN_EVENT_GROUP);
      if (!unknown) {
        unknown = { group: UNKNOWN_EVENT_GROUP, events: [] };
        groups.push(unknown);
      }
      unknown.events.push(e);
    }
  }

  return (
    <div style={treeStyle} data-e2e="hooks-event-tree">
      {groups.map(({ group, events }) => {
        const isCollapsed = collapsed.has(group);
        return (
          <div key={group}>
            <div
              style={groupStyle}
              data-e2e={`gui-group-head-${group}`}
              onClick={() => toggleGroup(group)}
              title={isCollapsed ? "展开分组" : "折叠分组"}
            >
              {isCollapsed ? "▸" : "▾"} {group}
            </div>
            {!isCollapsed &&
              (events.length === 0 ? (
                <div style={matcherRowStyle}>(无配置)</div>
              ) : (
                events.map((ev) => (
                  <div key={ev.event}>
                    {/* 事件行：点击选中 */}
                    <div
                      style={eventRowStyle(ev.event === selectedEvent)}
                      data-e2e={`gui-event-${ev.event}`}
                      onClick={() => onSelect(ev.event)}
                    >
                      <span>{ev.event}</span>
                      <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 11 }}>
                        ({eventHookCount(ev)})
                      </span>
                    </div>
                    {/* matcher 组 + handler 摘要（三级树第 3 层） */}
                    {ev.matcherGroups.map((groupGui, gi) => (
                      <div key={gi}>
                        <div style={matcherRowStyle}>
                          {groupGui.matcher === "" ? "全匹配" : `matcher: ${groupGui.matcher}`}
                        </div>
                        {groupGui.handlers.map((h, hi) => {
                          const disabled = isHandlerDisabled(disabledKeys, ev.event, groupGui.matcher, h);
                          const managed = isSltermManaged(h);
                          return (
                            <div
                              key={hi}
                              style={disabled ? disabledHandlerRowStyle : handlerRowStyle}
                              data-e2e={`gui-tree-handler-${ev.event}-${gi}-${hi}`}
                            >
                              <span
                                style={{
                                  overflow: "hidden",
                                  textOverflow: "ellipsis",
                                  whiteSpace: "nowrap",
                                  textDecoration: disabled ? "line-through" : undefined,
                                }}
                              >
                                {formatHandlerSummary(h)}
                              </span>
                              {managed ? (
                                <span style={managedBadgeStyle}>slTerminal 托管</span>
                              ) : (
                                // 启停 checkbox（P3-FE-19）：注入段条目（isSltermManaged 命中）不渲染（C13-8 禁禁用）
                                <input
                                  type="checkbox"
                                  data-e2e={`gui-tree-disable-${ev.event}-${gi}-${hi}`}
                                  checked={disabled}
                                  style={toggleStyle}
                                  title={disabled ? "点击启用该 handler" : "点击禁用该 handler"}
                                  onChange={() =>
                                    onToggleDisabled({
                                      event: ev.event,
                                      matcher: groupGui.matcher === "" ? null : groupGui.matcher,
                                      command: h.type === "command" ? h.command ?? "" : "",
                                    })
                                  }
                                />
                              )}
                            </div>
                          );
                        })}
                      </div>
                    ))}
                  </div>
                ))
              ))}
          </div>
        );
      })}
    </div>
  );
};

export default EventTree;
