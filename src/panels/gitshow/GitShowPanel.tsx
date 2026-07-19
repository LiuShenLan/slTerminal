// GitShowPanel — HEAD 中文件只读查看面板
//
// 通过 gitFileAtHead 获取文件在 HEAD commit 中的内容，用 CM6 只读模式展示。
// 三态：loading → content / error（"该文件在 HEAD 中不存在"）。
// CM6 只读：EditorState.readOnly.of(true) + EditorView.editable.of(false)。
// 大文件阈值从 useCodeMirror 复用，禁止新造数值。

import React, { useRef, useState, useEffect } from "react";
import { EditorView } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { oneDark } from "@codemirror/theme-one-dark";
import { gitFileAtHead } from "../../ipc/git";
import { getLanguageExtension } from "../editor/useCodeMirror";
import {
  MAX_FILE_SIZE_BYTES,
  LARGE_FILE_WARN_BYTES,
  createEditorFontExtension,
} from "../editor/useCodeMirror";
import { useFontSize } from "../../stores";
import { EDITOR_BG, ERROR_FG, HTML_PANEL_LOADING_FG, PANEL_BG } from "../../theme";

/** GitShowPanel 接收的面板参数 */
interface GitShowPanelProps {
  params: {
    panelId: string;
    filePath: string;
    oldPath?: string;
    repoPath: string;
  };
}

/** 加载状态机 */
type LoadState =
  | { kind: "loading" }
  | { kind: "content"; text: string }
  | { kind: "error" };

/** 错误占位文案——见契约：catch 任意错误皆显示此文案，不解析错误内容 */
const HEAD_NOT_FOUND_TEXT = "该文件在 HEAD 中不存在";

/** 居中容器样式（loading / error 共用） */
const centerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

/** CM6 容器样式（同 editor：overflow clip 委托 .cm-scroller 管理滚动） */
const cmContainerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  background: EDITOR_BG,
  overflow: "clip",
};

const GitShowPanel: React.FC<GitShowPanelProps> = ({ params }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const editorFontSize = useFontSize((s) => s.editorFontSize);

  // 加载 HEAD 文件内容
  useEffect(() => {
    const { repoPath, filePath, oldPath } = params;
    const queryPath = oldPath ?? filePath;

    // 切换文件时先进入 loading 态——旧 CM6 视图由下方 useEffect 的 cleanup
    // 在 state.kind 从 "content" 变为 "loading" 时自动销毁
    setState({ kind: "loading" });

    let cancelled = false;

    (async () => {
      try {
        const text = await gitFileAtHead(repoPath, queryPath);
        if (cancelled) return;
        setState({ kind: "content", text });
      } catch {
        if (cancelled) return;
        setState({ kind: "error" });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [params.repoPath, params.filePath, params.oldPath]);

  // CM6 只读编辑器挂载
  useEffect(() => {
    const container = containerRef.current;
    if (!container || state.kind !== "content") return;

    const view = viewRef.current;
    if (view) return; // 已挂载，避免重复创建

    const { text } = state;

    // 大文件检查
    const sizeHint = text.length;
    let displayText = text;
    if (sizeHint > MAX_FILE_SIZE_BYTES) {
      displayText = `// [slTerminal] 文件过大（约${(sizeHint / 1_000_000).toFixed(1)}MB），已拒绝打开以保护内存。`;
    } else if (sizeHint > LARGE_FILE_WARN_BYTES) {
      // 大文件警告：在内容顶部插入注释提示（不弹窗——只读视图无保存风险）
      const header = `// [slTerminal] ⚠ 大文件（约${(sizeHint / 1_000_000).toFixed(1)}MB），只读查看。\n// 语法高亮和搜索可能影响性能。\n\n`;
      displayText = header + text;
    }

    const newView = new EditorView({
      state: EditorState.create({
        doc: displayText,
        extensions: [
          basicSetup,
          oneDark,
          // .cm-editor 高度→.cm-scroller height:100%约束→溢出→滚动条（同 editor）
          EditorView.theme({ "&": { height: "100%" } }),
          createEditorFontExtension(editorFontSize),
          // 只读：不可编辑 + 光标不可见
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
          getLanguageExtension(params.filePath),
        ],
      }),
      parent: container,
    });

    viewRef.current = newView;

    return () => {
      newView.destroy();
      viewRef.current = null;
    };
  }, [state.kind, params.filePath, editorFontSize]);

  if (state.kind === "loading") {
    return (
      <div style={centerStyle}>
        <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }}>加载中...</span>
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div style={centerStyle}>
        <span style={{ color: ERROR_FG, fontSize: 13 }}>
          {HEAD_NOT_FOUND_TEXT}
        </span>
      </div>
    );
  }

  return <div ref={containerRef} style={cmContainerStyle} />;
};

export default GitShowPanel;
