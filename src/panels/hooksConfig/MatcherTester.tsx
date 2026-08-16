// MatcherTester.tsx — matcher 实时试测工具（P3-FE-11）
//
// 输入 matcher + toolName + 事件（事件感知窄字符集——FileChanged/StopFailure
// 受限窄字符集，连字符/空格/逗号强制走 JS 正则，C13-5），
// 实时调用 matcherEngine.matchHook 显示命中结果与匹配模式（exact-or / regex / all）。
// 自包含组件（无 props），作为 JsonMode 底部内联工具。

import React, { useMemo, useState } from "react";
import { matchHook } from "./matcherEngine";
import { HOOK_EVENTS, hasRestrictedMatcherCharset, getEventMeta } from "./eventsCatalog";
import { PANEL_BG, INPUT_BORDER, SIDEBAR_FG, ERROR_FG, HTML_PANEL_LOADING_FG } from "../../theme";

/** 匹配模式显示文案映射 */
const MODE_LABEL: Record<string, string> = {
  "exact-or": "精确匹配 OR",
  regex: "JS 正则（非锚定）",
  all: "全匹配",
};

/** 容器样式 */
const containerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 10px",
  background: PANEL_BG,
  flexWrap: "wrap",
};

/** 标签样式 */
const labelStyle: React.CSSProperties = {
  fontSize: 12,
  color: SIDEBAR_FG,
};

/** 输入框样式 */
const inputStyle: React.CSSProperties = {
  fontSize: 12,
  padding: "2px 6px",
  background: "transparent",
  color: SIDEBAR_FG,
  border: `1px solid ${INPUT_BORDER}`,
  borderRadius: 3,
};

/** 结果文案样式（命中/未命中着色） */
function resultStyle(matched: boolean): React.CSSProperties {
  return { fontSize: 12, color: matched ? SIDEBAR_FG : ERROR_FG, marginLeft: 4 };
}

const MatcherTester: React.FC = () => {
  // 默认事件 PreToolUse（支持 matcher、工具名目标、常规窄字符集）
  const [event, setEvent] = useState("PreToolUse");
  const [matcher, setMatcher] = useState("");
  const [toolName, setToolName] = useState("");

  // 实时命中结果（纯函数 matchHook，事件感知窄字符集）
  const result = useMemo(() => matchHook(matcher, toolName, event), [matcher, toolName, event]);
  const restricted = hasRestrictedMatcherCharset(event);
  const matcherTarget = getEventMeta(event)?.matcherTarget;

  return (
    <div style={containerStyle} data-e2e="hooks-matcher-tester">
      <span style={labelStyle}>Matcher 试测</span>
      {/* 事件选择：驱动窄字符集判定 + 目标提示 */}
      <select
        data-e2e="matcher-event"
        value={event}
        onChange={(e) => setEvent(e.target.value)}
        style={inputStyle}
      >
        {HOOK_EVENTS.filter((m) => m.supportsMatcher).map((m) => (
          <option key={m.event} value={m.event}>
            {m.event}
          </option>
        ))}
      </select>
      {/* matcher 输入 */}
      <input
        data-e2e="matcher-input"
        placeholder='matcher（"*" 或空 = 全匹配）'
        value={matcher}
        onChange={(e) => setMatcher(e.target.value)}
        style={{ ...inputStyle, width: 180 }}
      />
      {/* toolName 输入（匹配目标值） */}
      <input
        data-e2e="matcher-tool"
        placeholder={matcherTarget ? `目标值（${matcherTarget}）` : "目标值"}
        value={toolName}
        onChange={(e) => setToolName(e.target.value)}
        style={{ ...inputStyle, width: 140 }}
      />
      {/* 命中结果 + 匹配模式（纯文本——IC-08：装饰字符清除，结果色由 resultStyle 区分） */}
      <span data-e2e="matcher-result" style={resultStyle(result.matched)}>
        {result.matched ? "命中" : "未命中"}
      </span>
      <span style={{ fontSize: 11, color: HTML_PANEL_LOADING_FG }}>
        模式：{MODE_LABEL[result.mode]}
        {restricted && "（受限窄字符集：连字符/空格/逗号强制走正则）"}
      </span>
    </div>
  );
};

export default MatcherTester;
