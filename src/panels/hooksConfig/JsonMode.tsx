// JsonMode.tsx — hooks 配置 JSON 模式编辑器（P3-FE-11）
//
// 布局：左 = 事件导航侧栏（30 事件按 eventsCatalog 十组渲染），
//       右 = CM6 编辑器（上）+ MatcherTester 内联试测工具（下）。
// CM6 扩展：@codemirror/lang-json 语言 + codemirror-json-schema
// （jsonSchemaHover 悬停 + jsonSchemaLinter 波浪线，
//  schema 用 hooks 子 schema——src/features/hooksConfig/schema）+ jsonParseLinter 语法波浪线。
// 无自动补全（Ctrl+Space）——验收后决策删除（2026-08-01）。
// 校验：非法 JSON / schema 违规经 onValidationChange(isValid, diagnostics) 通知父组件
// （与 Stage 06 保存校验共用 validateHooksJson）。
// 事件导航：点击事件名 → 简单文本搜索 `"EventName"` 定位 → setSelection + scrollIntoView。

import React, { useCallback, useEffect, useRef } from "react";
import { EditorView, hoverTooltip } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { linter } from "@codemirror/lint";
import {
  jsonSchemaHover,
  jsonSchemaLinter,
  stateExtensions,
  handleRefresh,
} from "codemirror-json-schema";
import type { JSONSchema7 } from "json-schema";
import {
  hooksSubSchema,
  validateHooksJson,
  type JsonDiagnostic,
} from "../../features/hooksConfig/schema";
import { getGroups, getEventsByGroup } from "./eventsCatalog";
import MatcherTester from "./MatcherTester";
import {
  PANEL_BG,
  EDITOR_BG,
  INPUT_BORDER,
  SIDEBAR_FG,
  FOCUS_BORDER,
  HTML_PANEL_LOADING_FG,
  ON_ACCENT_FG,
  editorTheme,
  editorColorOverrides,
} from "../../theme";

/** JsonMode props：value/onChange/onValidationChange（外部驱动 + 校验上报） */
export interface JsonModeProps {
  /** hooks 子树 JSON 文本（外部驱动：加载/保存/重载/切层后更新） */
  value: string;
  /** 用户编辑回调（每次 docChanged 触发） */
  onChange: (value: string) => void;
  /** 校验结果回调：isValid=false 表示非法 JSON 或 schema 违规 */
  onValidationChange: (isValid: boolean, diagnostics: JsonDiagnostic[]) => void;
}

/**
 * 事件键在 JSON 文本中的定位（简单文本搜索，P3-FE-11 约定）：
 * 返回 `"EventName"` 引号起始 index；未找到返回 -1。
 * 纯函数导出供测试与导航共用。
 */
export function findEventPosition(doc: string, event: string): number {
  return doc.indexOf(`"${event}"`);
}

/** 事件导航侧栏样式（宽 190px，右侧边框分隔） */
const navStyle: React.CSSProperties = {
  width: 190,
  flexShrink: 0,
  overflowY: "auto",
  borderRight: `1px solid ${INPUT_BORDER}`,
  background: PANEL_BG,
  padding: "6px 8px",
  boxSizing: "border-box",
};

/** 分组标题样式 */
const groupTitleStyle: React.CSSProperties = {
  fontSize: 11,
  color: HTML_PANEL_LOADING_FG,
  margin: "8px 0 4px",
  userSelect: "none",
};

/** 事件按钮样式 */
function eventButtonStyle(): React.CSSProperties {
  return {
    display: "block",
    width: "100%",
    textAlign: "left",
    padding: "2px 6px",
    fontSize: 12,
    cursor: "pointer",
    background: "transparent",
    color: SIDEBAR_FG,
    border: "none",
    borderRadius: 3,
  };
}

/** 主区样式（编辑器 + MatcherTester 纵向排布；minHeight:0 防 flex 链内容撑开塌陷） */
const mainStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  flexDirection: "column",
  minWidth: 0,
  minHeight: 0,
  background: EDITOR_BG,
};

/** 编辑器容器样式（CM6 自身滚动）——overflow:clip 裁剪但不创建滚动容器，
    滚轮穿透到 .cm-scroller（hidden 是 CSS 滚动容器会吸收滚轮，照编辑器滚动委托决策） */
const editorContainerStyle: React.CSSProperties = {
  flex: 1,
  overflow: "clip",
  minHeight: 0,
};

/** MatcherTester 容器分隔条样式 */
const dividerStyle: React.CSSProperties = {
  height: 1,
  background: INPUT_BORDER,
};

const JsonMode: React.FC<JsonModeProps> = ({ value, onChange, onValidationChange }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  // ref 镜像：updateListener 闭包永不重建，回调读取最新引用（照 useXterm handleSaveRef 模式）
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const onValidationChangeRef = useRef(onValidationChange);
  onValidationChangeRef.current = onValidationChange;

  /** 用户编辑：onChange 上报 + 校验上报（非法 JSON / schema 违规 → isValid=false） */
  const handleDocChanged = useCallback(
    (update: { docChanged: boolean; state: { doc: { toString(): string } } }) => {
      if (!update.docChanged) return;
      const text = update.state.doc.toString();
      onChangeRef.current(text);
      const result = validateHooksJson(text);
      onValidationChangeRef.current(result.isValid, result.diagnostics);
    },
    [],
  );

  // 挂载：创建 EditorView（StrictMode 双挂载由 effect cleanup destroy 正确兜底）
  // 扩展：语言 + jsonSchemaHover 悬停 + jsonSchemaLinter 波浪线（hooks 子 schema）
  // + jsonParseLinter 语法波浪线 + 暗色主题扩展 + height:100% theme
  //（.cm-editor 确定高度 → .cm-scroller 内容溢出 → 竖向滚动条出现，照编辑器滚动委托决策）
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const view = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          basicSetup,
          json(),
          linter(jsonParseLinter(), { delay: 300 }),
          linter(jsonSchemaLinter(), { needsRefresh: handleRefresh }),
          hoverTooltip(jsonSchemaHover()),
          stateExtensions(hooksSubSchema as unknown as JSONSchema7),
          EditorView.theme({ "&": { height: "100%" } }),
          editorTheme,
          editorColorOverrides(),
          EditorView.updateListener.of(handleDocChanged),
        ],
      }),
      parent: container,
    });
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // 空 deps 刻意：EditorView 仅挂载时创建一次，value 初始值经闭包捕获；
    // 后续外部 value 变化由下方「外部 value 同步」effect 负责 dispatch 更新（不重建视图）
  }, []);

  // 外部 value 同步：加载/保存/重载/切层后内容变化才 dispatch 更新（用户输入中不覆盖）
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({ changes: { from: 0, to: current.length, insert: value } });
    }
  }, [value]);

  /** 事件导航：文本搜索定位事件键 → 选区 + scrollIntoView */
  const navigateToEvent = useCallback((event: string) => {
    const view = viewRef.current;
    if (!view) return;
    const pos = findEventPosition(view.state.doc.toString(), event);
    if (pos < 0) return;
    view.dispatch({
      selection: { anchor: pos, head: pos + event.length + 2 },
      effects: EditorView.scrollIntoView(pos, { y: "center" }),
    });
    view.focus();
  }, []);

  return (
    <div style={{ flex: 1, display: "flex", minHeight: 0 }} data-e2e="hooks-json-mode">
      {/* 事件导航侧栏 */}
      <div style={navStyle} data-e2e="hooks-event-nav">
        {getGroups().map((group) => (
          <div key={group}>
            <div style={groupTitleStyle}>{group}</div>
            {getEventsByGroup(group).map((meta) => (
              <button
                key={meta.event}
                type="button"
                data-e2e={`hooks-nav-${meta.event}`}
                title={meta.supportsMatcher ? `matcher 目标：${meta.matcherTarget}` : "不支持 matcher"}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = FOCUS_BORDER;
                  e.currentTarget.style.color = ON_ACCENT_FG;
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                  e.currentTarget.style.color = SIDEBAR_FG;
                }}
                onClick={() => navigateToEvent(meta.event)}
                style={eventButtonStyle()}
              >
                {meta.event}
              </button>
            ))}
          </div>
        ))}
      </div>
      {/* 主区：编辑器 + MatcherTester */}
      <div style={mainStyle}>
        <div ref={containerRef} style={editorContainerStyle} data-e2e="hooks-json-editor" />
        <div style={dividerStyle} />
        <MatcherTester />
      </div>
    </div>
  );
};

export default JsonMode;
