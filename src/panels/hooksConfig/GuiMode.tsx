// GuiMode.tsx — hooks 配置 GUI 表单模式（P3-FE-12）
//
// Master-Detail 布局：左侧 EventTree（事件树），右侧详情区。
// 状态：selectedEvent / selectedMatcherIndex / selectedHandlerIndex——选中事件
// 经派生守卫（事件被删除/重载后自动回退空态，索引越界归 null）。
// 变更回调：添加/删除事件、matcher 组、handler + matcher 值更新，全部构造
// 新 GUI 模型经 onChange 上抛（父层 guiToJson → updateConfigJson 同步，
// Stage 06 在此基础上做双模式同步与保存安全）。
// 注入段（isSltermManaged 命中）条目禁删（C13-8）——handler / 含托管 handler 的
// matcher 组 / 事件三层删除按钮均禁用并显示标记。

import React, { useCallback, useState } from "react";
import EventTree from "./EventTree";
import { HandlerForm } from "./HandlerForm";
import type { HookEventGui, HookHandlerGui, HookMatcherGroupGui, HooksConfigGui } from "./configModel";
import { isSltermManaged, UNKNOWN_EVENT_GROUP } from "./configModel";
import { getEventMeta, getSupportedHandlerTypes, HOOK_EVENTS } from "./eventsCatalog";
import type { HandlerType } from "./eventsCatalog";
import {
  PANEL_BG,
  SIDEBAR_FG,
  HTML_PANEL_LOADING_FG,
  ACTIVE_SELECTION_BG,
  ERROR_FG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  SECONDARY_BG,
} from "../../theme";

/** GuiMode props：GUI 模型 + 变更回调（外部驱动，父层负责转换/持久化） */
export interface GuiModeProps {
  /** GUI 模型（useHooksConfig.guiModel） */
  gui: HooksConfigGui;
  /** 变更回调（每次编辑构造新模型上抛） */
  onChange: (gui: HooksConfigGui) => void;
}

/** 详情区容器样式（flex:1 撑满，自身滚动） */
const detailStyle: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  overflowY: "auto",
  padding: "8px 12px",
  background: PANEL_BG,
  boxSizing: "border-box",
};

/** 添加条样式（事件下拉 + 添加事件按钮，顶部分隔） */
const addBarStyle: React.CSSProperties = {
  display: "flex",
  gap: 6,
  alignItems: "center",
  marginBottom: 10,
  paddingBottom: 8,
  borderBottom: `1px solid ${INPUT_BORDER}`,
};

/** 选中事件标题样式 */
const eventTitleStyle: React.CSSProperties = {
  fontSize: 13,
  color: SIDEBAR_FG,
  fontWeight: 600,
};

/** 次要提示样式（分组 / matcher 目标 / 空态） */
const hintStyle: React.CSSProperties = {
  fontSize: 11,
  color: HTML_PANEL_LOADING_FG,
  margin: "2px 0",
};

/** 小按钮样式（删除 / 添加） */
function smallButtonStyle(disabled: boolean): React.CSSProperties {
  return {
    padding: "2px 8px",
    fontSize: 11,
    cursor: disabled ? "default" : "pointer",
    background: "transparent",
    color: SIDEBAR_FG,
    border: `1px solid ${INPUT_BORDER}`,
    borderRadius: 3,
  };
}

/** matcher 输入框样式 */
const matcherInputStyle: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  background: INPUT_BG,
  color: SIDEBAR_FG,
  border: `1px solid ${INPUT_BORDER}`,
  borderRadius: 3,
  padding: "2px 6px",
  fontSize: 12,
};

/** matcher 组框样式（选中态边框高亮） */
function groupBoxStyle(selected: boolean): React.CSSProperties {
  return {
    border: `1px solid ${selected ? FOCUS_BORDER : INPUT_BORDER}`,
    borderRadius: 4,
    margin: "8px 0",
    padding: "6px 8px",
    background: SECONDARY_BG,
  };
}

/** handler 行样式（选中态背景高亮） */
function handlerRowStyle(selected: boolean): React.CSSProperties {
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

/** 「slTerminal 托管」标记样式（C13-8 注入段标识） */
const managedBadgeStyle: React.CSSProperties = {
  fontSize: 10,
  color: ERROR_FG,
  border: `1px solid ${ERROR_FG}`,
  borderRadius: 3,
  padding: "0 4px",
  flexShrink: 0,
};

/** matcher 组是否含注入段 handler（含则禁删整组，C13-8） */
function groupManaged(g: HookMatcherGroupGui): boolean {
  return g.handlers.some((h) => isSltermManaged(h));
}

/** 事件是否含注入段 handler（含则禁删事件，C13-8） */
function eventManaged(e: HookEventGui): boolean {
  return e.matcherGroups.some(groupManaged);
}

const GuiMode: React.FC<GuiModeProps> = ({ gui, onChange }) => {
  // 选中状态：事件 + matcher 组索引 + handler 索引
  const [selectedEvent, setSelectedEvent] = useState<string | null>(null);
  const [selectedMatcherIndex, setSelectedMatcherIndex] = useState<number | null>(null);
  const [selectedHandlerIndex, setSelectedHandlerIndex] = useState<number | null>(null);
  // 添加事件下拉草稿（空 = 取首个候选）
  const [draftEvent, setDraftEvent] = useState<string>("");
  // 添加 handler 类型下拉草稿（随选中事件支持矩阵重置）
  const [draftHandlerType, setDraftHandlerType] = useState<HandlerType>("command");

  // 派生守卫：选中事件被删除/重载后回退空态；索引越界归 null
  const selected = gui.events.find((e) => e.event === selectedEvent) ?? null;
  const matcherIndex =
    selected !== null &&
    selectedMatcherIndex !== null &&
    selectedMatcherIndex < selected.matcherGroups.length
      ? selectedMatcherIndex
      : null;
  const handlerIndex =
    selected !== null &&
    matcherIndex !== null &&
    selectedHandlerIndex !== null &&
    selectedHandlerIndex < selected.matcherGroups[matcherIndex].handlers.length
      ? selectedHandlerIndex
      : null;

  // 添加事件候选：目录 30 事件中尚未配置的
  const availableEvents = HOOK_EVENTS.map((m) => m.event).filter(
    (ev) => !gui.events.some((e) => e.event === ev),
  );
  const addEventTarget = draftEvent !== "" ? draftEvent : availableEvents[0] ?? "";

  /** 事件选中：重置 matcher/handler 索引 + handler 类型草稿 */
  const handleSelect = useCallback((event: string) => {
    setSelectedEvent(event);
    setSelectedMatcherIndex(null);
    setSelectedHandlerIndex(null);
    setDraftHandlerType(getSupportedHandlerTypes(event)[0]);
  }, []);

  /** 添加事件（候选为空时无操作）；成功后选中新事件 */
  const addEvent = useCallback(() => {
    if (!addEventTarget) return;
    const meta = getEventMeta(addEventTarget);
    onChange({
      events: [
        ...gui.events,
        {
          event: addEventTarget,
          group: meta?.group ?? UNKNOWN_EVENT_GROUP,
          matcherGroups: [],
        },
      ],
    });
    handleSelect(addEventTarget);
  }, [addEventTarget, gui.events, onChange, handleSelect]);

  /** 删除事件（选中事件被删则回退空态） */
  const deleteEvent = useCallback(
    (event: string) => {
      onChange({ events: gui.events.filter((e) => e.event !== event) });
      if (selectedEvent === event) {
        setSelectedEvent(null);
        setSelectedMatcherIndex(null);
        setSelectedHandlerIndex(null);
      }
    },
    [gui.events, onChange, selectedEvent],
  );

  /** 添加 matcher 组（空 matcher = 全匹配） */
  const addMatcherGroup = useCallback(
    (event: string) => {
      onChange({
        events: gui.events.map((e) =>
          e.event === event
            ? { ...e, matcherGroups: [...e.matcherGroups, { matcher: "", handlers: [] }] }
            : e,
        ),
      });
    },
    [gui.events, onChange],
  );

  /** 删除 matcher 组（越界索引回退） */
  const deleteMatcherGroup = useCallback(
    (event: string, index: number) => {
      onChange({
        events: gui.events.map((e) =>
          e.event === event
            ? { ...e, matcherGroups: e.matcherGroups.filter((_, i) => i !== index) }
            : e,
        ),
      });
      if (selectedEvent === event && selectedMatcherIndex === index) {
        setSelectedMatcherIndex(null);
        setSelectedHandlerIndex(null);
      }
    },
    [gui.events, onChange, selectedEvent, selectedMatcherIndex],
  );

  /** 添加 handler（type 经支持矩阵防御校验，事件切换后草稿已重置） */
  const addHandler = useCallback(
    (event: string, groupIndex: number) => {
      const supported = getSupportedHandlerTypes(event);
      if (!supported.includes(draftHandlerType)) return;
      onChange({
        events: gui.events.map((e) =>
          e.event === event
            ? {
                ...e,
                matcherGroups: e.matcherGroups.map((g, gi) =>
                  gi === groupIndex ? { ...g, handlers: [...g.handlers, { type: draftHandlerType }] } : g,
                ),
              }
            : e,
        ),
      });
    },
    [gui.events, onChange, draftHandlerType],
  );

  /** 删除 handler（越界索引回退） */
  const deleteHandler = useCallback(
    (event: string, groupIndex: number, handlerIndex: number) => {
      onChange({
        events: gui.events.map((e) =>
          e.event === event
            ? {
                ...e,
                matcherGroups: e.matcherGroups.map((g, gi) =>
                  gi === groupIndex
                    ? { ...g, handlers: g.handlers.filter((_, i) => i !== handlerIndex) }
                    : g,
                ),
              }
            : e,
        ),
      });
      if (
        selectedEvent === event &&
        selectedMatcherIndex === groupIndex &&
        selectedHandlerIndex === handlerIndex
      ) {
        setSelectedHandlerIndex(null);
      }
    },
    [gui.events, onChange, selectedEvent, selectedMatcherIndex, selectedHandlerIndex],
  );

  /** 更新 handler 字段（HandlerForm onChange 上抛完整新 handler，P3-FE-14 接入） */
  const updateHandler = useCallback(
    (event: string, groupIndex: number, handlerIndex: number, next: HookHandlerGui) => {
      onChange({
        events: gui.events.map((e) =>
          e.event === event
            ? {
                ...e,
                matcherGroups: e.matcherGroups.map((g, gi) =>
                  gi === groupIndex
                    ? { ...g, handlers: g.handlers.map((h, hi) => (hi === handlerIndex ? next : h)) }
                    : g,
                ),
              }
            : e,
        ),
      });
    },
    [gui.events, onChange],
  );

  /** matcher 值更新（受控输入，逐键上抛） */
  const updateMatcher = useCallback(
    (event: string, groupIndex: number, value: string) => {
      onChange({
        events: gui.events.map((e) =>
          e.event === event
            ? {
                ...e,
                matcherGroups: e.matcherGroups.map((g, gi) =>
                  gi === groupIndex ? { ...g, matcher: value } : g,
                ),
              }
            : e,
        ),
      });
    },
    [gui.events, onChange],
  );

  const meta = selected ? getEventMeta(selected.event) : undefined;

  return (
    <div style={{ flex: 1, display: "flex", minHeight: 0 }} data-e2e="hooks-gui-mode">
      {/* 左侧：事件树 */}
      <EventTree gui={gui} selectedEvent={selectedEvent} onSelect={handleSelect} />
      {/* 右侧：详情区 */}
      <div style={detailStyle} data-e2e="hooks-gui-detail">
        {/* 添加事件条 */}
        <div style={addBarStyle}>
          <select
            data-e2e="gui-add-event-select"
            value={addEventTarget}
            onChange={(e) => setDraftEvent(e.target.value)}
            style={{ background: INPUT_BG, color: SIDEBAR_FG, border: `1px solid ${INPUT_BORDER}`, borderRadius: 3, fontSize: 12, padding: "2px 4px" }}
          >
            {availableEvents.map((ev) => (
              <option key={ev} value={ev}>
                {ev}
              </option>
            ))}
          </select>
          <button
            type="button"
            data-e2e="gui-add-event"
            disabled={!addEventTarget}
            onClick={addEvent}
            style={smallButtonStyle(!addEventTarget)}
          >
            添加事件
          </button>
          <span style={{ flex: 1 }} />
        </div>

        {selected === null ? (
          <div style={hintStyle}>在左侧选择已配置事件，或从上方下拉添加新事件</div>
        ) : (
          <>
            {/* 事件标题行 */}
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={eventTitleStyle} data-e2e="gui-detail-event">
                {selected.event}
              </span>
              <span style={hintStyle}>{selected.group}</span>
              <span style={{ flex: 1 }} />
              <button
                type="button"
                data-e2e="gui-event-del"
                disabled={eventManaged(selected)}
                onClick={() => deleteEvent(selected.event)}
                style={smallButtonStyle(eventManaged(selected))}
              >
                删除事件
              </button>
            </div>
            <div style={hintStyle}>
              {meta ? (meta.supportsMatcher ? `matcher 目标：${meta.matcherTarget}` : "不支持 matcher") : "未知事件"}
            </div>

            {/* 添加 matcher 组入口 */}
            <div style={{ margin: "8px 0" }}>
              <button
                type="button"
                data-e2e="gui-group-add"
                onClick={() => addMatcherGroup(selected.event)}
                style={smallButtonStyle(false)}
              >
                添加 matcher 组
              </button>
            </div>

            {selected.matcherGroups.length === 0 && <div style={hintStyle}>暂无 matcher 组</div>}

            {/* matcher 组列表 */}
            {selected.matcherGroups.map((g, gi) => (
              <div key={gi} style={groupBoxStyle(matcherIndex === gi)} data-e2e={`gui-group-${selected.event}-${gi}`}>
                {/* 组头行：matcher 输入（不支持 matcher 事件省略）+ 删除组 */}
                <div
                  style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}
                  onClick={() => {
                    setSelectedMatcherIndex(gi);
                    setSelectedHandlerIndex(null);
                  }}
                  title="点击选中该 matcher 组"
                >
                  {meta?.supportsMatcher ? (
                    <>
                      <span style={{ fontSize: 11, color: HTML_PANEL_LOADING_FG }}>matcher</span>
                      <input
                        data-e2e={`gui-matcher-${selected.event}-${gi}`}
                        value={g.matcher}
                        onChange={(e) => updateMatcher(selected.event, gi, e.target.value)}
                        onClick={(e) => e.stopPropagation()}
                        style={matcherInputStyle}
                        placeholder="留空 = 全匹配"
                      />
                    </>
                  ) : (
                    <span style={{ fontSize: 11, color: HTML_PANEL_LOADING_FG }}>
                      全匹配（该事件不支持 matcher）
                    </span>
                  )}
                  <span style={{ flex: 1 }} />
                  <button
                    type="button"
                    data-e2e={`gui-group-del-${selected.event}-${gi}`}
                    disabled={groupManaged(g)}
                    onClick={(e) => {
                      e.stopPropagation();
                      deleteMatcherGroup(selected.event, gi);
                    }}
                    style={smallButtonStyle(groupManaged(g))}
                    title={groupManaged(g) ? "含 slTerminal 托管条目，不可删除" : "删除该 matcher 组"}
                  >
                    删除组
                  </button>
                </div>
                {/* handler 列表（P3-FE-14：选中 handler 时行下方展开 HandlerForm 完整字段表单） */}
                {g.handlers.length === 0 && <div style={hintStyle}>暂无 handler</div>}
                {g.handlers.map((h, hi) => (
                  <React.Fragment key={hi}>
                    <div
                      style={handlerRowStyle(matcherIndex === gi && handlerIndex === hi)}
                      data-e2e={`gui-handler-${selected.event}-${gi}-${hi}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelectedMatcherIndex(gi);
                        setSelectedHandlerIndex(hi);
                      }}
                    >
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {h.type}
                      </span>
                      {isSltermManaged(h) && <span style={managedBadgeStyle}>slTerminal 托管</span>}
                      <span style={{ flex: 1 }} />
                      <button
                        type="button"
                        data-e2e={`gui-handler-del-${selected.event}-${gi}-${hi}`}
                        disabled={isSltermManaged(h)}
                        onClick={(e) => {
                          e.stopPropagation();
                          deleteHandler(selected.event, gi, hi);
                        }}
                        style={smallButtonStyle(isSltermManaged(h))}
                        title={isSltermManaged(h) ? "slTerminal 托管条目，不可删除" : "删除该 handler"}
                      >
                        删除
                      </button>
                    </div>
                    {matcherIndex === gi && handlerIndex === hi && (
                      <div style={{ padding: "4px 6px 6px" }}>
                        <HandlerForm
                          handler={h}
                          event={selected.event}
                          onChange={(next) => updateHandler(selected.event, gi, hi, next)}
                        />
                      </div>
                    )}
                  </React.Fragment>
                ))}
                {/* 添加 handler 入口（type 支持矩阵经 eventsCatalog 过滤） */}
                <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 6 }}>
                  <select
                    data-e2e={`gui-handler-type-${selected.event}-${gi}`}
                    value={draftHandlerType}
                    onChange={(e) => setDraftHandlerType(e.target.value as HandlerType)}
                    style={{ background: INPUT_BG, color: SIDEBAR_FG, border: `1px solid ${INPUT_BORDER}`, borderRadius: 3, fontSize: 11, padding: "1px 4px" }}
                  >
                    {getSupportedHandlerTypes(selected.event).map((t) => (
                      <option key={t} value={t}>
                        {t}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    data-e2e={`gui-handler-add-${selected.event}-${gi}`}
                    onClick={() => addHandler(selected.event, gi)}
                    style={smallButtonStyle(false)}
                  >
                    添加 handler
                  </button>
                </div>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
};

export default GuiMode;
