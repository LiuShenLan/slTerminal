// DiffPanel — Git 双栏 diff 面板
//
// 职责：
// - 横向均分两栏：左 = HEAD 只读 CM + HEAD gutter，右 = 工作区可编辑 CM + workdir gutter
// - 占位对齐：computeAlignment → Decoration.widget 空白行，保持两侧视觉对齐
// - 垂直滚动同步（一侧滚动 → 另一侧 scrollTop 跟随，syncingRef 防循环），水平滚动独立
// - 右侧 Ctrl+S：usePanelFocus("editor") + setActiveEditor → fs.writeFile → gitDiff → 刷新
// - 左侧刷新：onFsEvent 检测 .git 路径 → 重取 HEAD
// - 右侧外部修改：净自动重载 / 脏弹窗（照 editor 语义）
// - 大文件阈值复用 useCodeMirror 导出常量
//
// params: { panelId, filePath, oldPath?, repoPath }
// data-e2e: diff-panel / diff-left / diff-right

import React, { useRef, useState, useEffect, useMemo, useCallback } from "react";
import { EditorView, Decoration, WidgetType, keymap, type DecorationSet } from "@codemirror/view";
import {
  EditorState,
  StateField,
  StateEffect,
  RangeSetBuilder,
  Compartment,
} from "@codemirror/state";
import { basicSetup } from "codemirror";
import { oneDark } from "@codemirror/theme-one-dark";
import { search, searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { indentWithTab } from "@codemirror/commands";
import type { DiffHunk } from "../../types/git";
import { gitFileAtHead, gitDiff } from "../../ipc/git";
import { fs } from "../../ipc";
import { onFsEvent } from "../../ipc/notify";
import {
  getLanguageExtension,
  MAX_FILE_SIZE_BYTES,
  LARGE_FILE_WARN_BYTES,
  createEditorFontExtension,
} from "../editor/useCodeMirror";
import {
  diffGutter,
  updateDiffGutter,
  clearDiffGutter,
  headDiffGutter,
  updateHeadDiffGutter,
  clearHeadDiffGutter,
} from "../editor/gitGutter";
import { usePanelFocus } from "../../features/shortcuts";
import { setActiveEditor, clearActiveEditor, type EditorActions } from "../editor/activeEditor";
import { useFontSize } from "../../stores";
import { useFontSizeWheel } from "../../lib/useFontSizeWheel";
import { FONT_SIZE_MIN, FONT_SIZE_MAX } from "../../stores/fontSize";
import { computeAlignment } from "./alignment";
import { EDITOR_BG, ERROR_FG, HTML_PANEL_LOADING_FG, PANEL_BG, SEPARATOR_BG } from "../../theme";

// ── 占位行 Widget ─────────────────────────────────────────────

/** 块级占位行：不可选中、不响应指针事件 */
class PlaceholderWidget extends WidgetType {
  constructor(private lineHeight: number) {
    super();
  }

  toDOM(): HTMLElement {
    const el = document.createElement("div");
    el.style.height = `${this.lineHeight}px`;
    el.style.userSelect = "none";
    el.style.pointerEvents = "none";
    return el;
  }

  eq(other: PlaceholderWidget): boolean {
    return this.lineHeight === other.lineHeight;
  }
}

// ── 占位 Decoration StateField ────────────────────────────────

const setPlaceholders = StateEffect.define<DecorationSet>();

/** 存储占位 Decoration 的 StateField */
const placeholderField = StateField.define<DecorationSet>({
  create(): DecorationSet {
    return Decoration.none;
  },

  update(value: DecorationSet, tr): DecorationSet {
    for (const e of tr.effects) {
      if (e.is(setPlaceholders)) return e.value;
    }
    return value.map(tr.changes);
  },

  provide: (field) =>
    EditorView.decorations.from(field),
});

/**
 * 根据 alignment 创建占位 Decoration 集合。
 * 多个 block widget 在同位置会垂直堆叠（CM6 行为）。
 *
 * 假设来源注释：占位 widget 的行高与真实渲染行的精确对齐，以及
 * 块级 widget 与滚动同步的交互行为，依赖于 CM6 内部布局引擎。
 * jsdom 环境无法验证像素级渲染——由 Stage 04 人工实测兜底（IDEA 交互参考）。
 */
function createPlaceholderDecorations(
  alignment: Map<number, number>,
  doc: { line: (n: number) => { from: number; to: number } },
  lines: number,
  lineHeight: number,
): DecorationSet {
  const widgets: { from: number; to: number; widget: Decoration }[] = [];

  for (const [afterLine, count] of alignment) {
    if (afterLine < 0 || afterLine > lines) continue;
    const pos = afterLine === 0 ? 0 : doc.line(afterLine).to;

    for (let i = 0; i < count; i++) {
      widgets.push({
        from: pos,
        to: pos,
        widget: Decoration.widget({
          widget: new PlaceholderWidget(lineHeight),
          block: true,
        }),
      });
    }
  }

  const builder = new RangeSetBuilder<Decoration>();
  widgets.sort((a, b) => a.from - b.from);
  for (const w of widgets) {
    builder.add(w.from, w.to, w.widget);
  }
  return builder.finish();
}

// ── 公共类型 ──────────────────────────────────────────────────

/** DiffPanel 接收的面板参数 */
export interface DiffPanelParams {
  panelId: string;
  filePath: string;
  oldPath?: string;
  repoPath: string;
}

interface DiffPanelProps {
  params: DiffPanelParams;
}

// ── 状态机 ────────────────────────────────────────────────────

type PanelState =
  | { kind: "loading" }
  | { kind: "ready"; headContent: string; workdirContent: string }
  | { kind: "error"; message: string };

/** 行高估值（字体大小 × 1.5） */
function estimateLineHeight(fontSize: number): number {
  return Math.round(fontSize * 1.5);
}

// ── 组件 ──────────────────────────────────────────────────────

const DiffPanel: React.FC<DiffPanelProps> = ({ params }) => {
  const { panelId, filePath, oldPath, repoPath } = params;
  const leftContainerRef = useRef<HTMLDivElement>(null);
  const rightContainerRef = useRef<HTMLDivElement>(null);
  const leftViewRef = useRef<EditorView | null>(null);
  const rightViewRef = useRef<EditorView | null>(null);
  const [state, setState] = useState<PanelState>({ kind: "loading" });
  const editorFontSize = useFontSize((s) => s.editorFontSize);
  const setEditorFontSize = useFontSize((s) => s.setEditorFontSize);
  const fontSizeRef = useRef(editorFontSize);
  fontSizeRef.current = editorFontSize;

  // Compartments：字体/自动换行热切换，左右栏各独立实例（CM6 Compartment 绑定到特定 EditorState，不能跨 view 共享）
  const leftFontCompartment = useRef(new Compartment());
  const rightFontCompartment = useRef(new Compartment());
  const leftWrapCompartment = useRef(new Compartment());
  const rightWrapCompartment = useRef(new Compartment());
  const wordWrapRef = useRef(false);

  // renderKey 桥接：容器 div 在 "ready" 态才挂载，DOM commit 后 ref 才非 null。
  // state.kind 变为 "ready" 后的 effect 触发额外渲染，使 hooks 以非 null 容器执行。
  // bridgedRef 防无限循环 + 支持 filePath 切换后重新桥接。
  const [, setRenderKey] = useState(0);
  const bridgedRef = useRef(false);

  useEffect(() => {
    if (state.kind === "ready") {
      if (!bridgedRef.current) {
        bridgedRef.current = true;
        setRenderKey((k) => k + 1);
      }
    } else {
      bridgedRef.current = false;
    }
  }, [state.kind]);

  // 滚动同步——syncingRef 防循环
  const syncingRef = useRef(false);

  // 脏状态 + 路径 ref（右侧外部修改检测 / 保存抑制）
  const dirtyRef = useRef(false);
  const filePathRef = useRef(filePath);
  const justSavedRef = useRef(false);

  // hunks 缓存 ref（右侧保存后重用，避免重复 gitDiff）
  const hunksRef = useRef<DiffHunk[]>([]);

  // 行高 ref（供 scroll callback 实时读取，避免闭包过期）
  const lineHeightRef = useRef(estimateLineHeight(editorFontSize));

  useEffect(() => {
    filePathRef.current = filePath;
    lineHeightRef.current = estimateLineHeight(editorFontSize);
  }, [filePath, editorFontSize]);

  // ── 加载内容 + diff ─────────────────────────────────────────

  useEffect(() => {
    const queryPath = oldPath ?? filePath;

    let cancelled = false;

    (async () => {
      setState({ kind: "loading" });

      try {
        const [headContent, workdirContent] = await Promise.all([
          gitFileAtHead(repoPath, queryPath),
          fs.readFile(filePath),
        ]);

        if (cancelled) return;

        // 大文件检查
        let displayHead = headContent;
        if (headContent.length > MAX_FILE_SIZE_BYTES) {
          displayHead = `// [slTerminal] 文件过大（约${(headContent.length / 1_000_000).toFixed(1)}MB），已拒绝打开以保护内存。`;
        } else if (headContent.length > LARGE_FILE_WARN_BYTES) {
          displayHead = `// [slTerminal] 大文件（约${(headContent.length / 1_000_000).toFixed(1)}MB），只读查看。\n// 语法高亮和搜索可能影响性能。\n\n` + headContent;
        }

        let displayWorkdir = workdirContent;
        if (workdirContent.length > MAX_FILE_SIZE_BYTES) {
          displayWorkdir = `// [slTerminal] 文件过大（约${(workdirContent.length / 1_000_000).toFixed(1)}MB），已拒绝打开以保护内存。`;
        } else if (workdirContent.length > LARGE_FILE_WARN_BYTES) {
          displayWorkdir = `// [slTerminal] 大文件（约${(workdirContent.length / 1_000_000).toFixed(1)}MB），编辑可能影响性能。\n\n` + workdirContent;
        }

        setState({ kind: "ready", headContent: displayHead, workdirContent: displayWorkdir });

        // 异步加载 diff hunks（不阻塞渲染）
        try {
          const hunks = await gitDiff(repoPath, filePath);
          if (!cancelled) hunksRef.current = hunks;
        } catch {
          hunksRef.current = [];
        }
      } catch {
        if (cancelled) return;
        // 任意错误 → 占位文案（契约：不解析错误内容）
        setState({ kind: "error", message: "该文件在 HEAD 中不存在" });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [repoPath, filePath, oldPath]);

  // ── 占位刷新 ────────────────────────────────────────────────

  const refreshPlaceholders = useCallback(() => {
    const hunks = hunksRef.current;
    const lh = lineHeightRef.current;

    const createAndDispatch = (
      view: EditorView | null,
      align: Map<number, number>,
    ) => {
      if (!view) return;
      if (align.size === 0) {
        view.dispatch({ effects: setPlaceholders.of(Decoration.none) });
        return;
      }
      const doc = view.state.doc;
      const deco = createPlaceholderDecorations(align, doc, doc.lines, lh);
      view.dispatch({ effects: setPlaceholders.of(deco) });
    };

    if (hunks.length === 0) {
      createAndDispatch(leftViewRef.current, new Map());
      createAndDispatch(rightViewRef.current, new Map());
      return;
    }

    const alignment = computeAlignment(hunks);
    createAndDispatch(leftViewRef.current, alignment.left);
    createAndDispatch(rightViewRef.current, alignment.right);
  }, []);

  // ── 滚动同步 ────────────────────────────────────────────────

  useEffect(() => {
    if (state.kind !== "ready") return;

    let removeListeners: (() => void) | null = null;

    // 等待两个 view 创建完成再绑定 scroll 监听
    const t1 = setTimeout(() => {
      const leftView = leftViewRef.current;
      const rightView = rightViewRef.current;
      if (!leftView || !rightView) return;

      const leftScroller = leftView.scrollDOM;
      const rightScroller = rightView.scrollDOM;

      const onLeftScroll = () => {
        if (syncingRef.current) return;
        syncingRef.current = true;
        rightScroller.scrollTop = leftScroller.scrollTop;
        syncingRef.current = false;
      };

      const onRightScroll = () => {
        if (syncingRef.current) return;
        syncingRef.current = true;
        leftScroller.scrollTop = rightScroller.scrollTop;
        syncingRef.current = false;
      };

      leftScroller.addEventListener("scroll", onLeftScroll, { passive: true });
      rightScroller.addEventListener("scroll", onRightScroll, { passive: true });

      removeListeners = () => {
        leftScroller.removeEventListener("scroll", onLeftScroll);
        rightScroller.removeEventListener("scroll", onRightScroll);
      };
    }, 100);

    return () => {
      clearTimeout(t1);
      removeListeners?.();
    };
  }, [state.kind]);

  // ── 右侧保存 ────────────────────────────────────────────────

  const handleSave = useCallback(async () => {
    const view = rightViewRef.current;
    if (!view) return;

    const path = filePathRef.current;
    if (!path) return;

    justSavedRef.current = true;

    const content = view.state.doc.toString();
    try {
      await fs.writeFile(path, content);
    } catch (err) {
      window.alert(`保存失败: ${err}`);
      return;
    }

    dirtyRef.current = false;

    // 重新加载 diff → 刷新双侧 gutter + 占位
    try {
      const hunks = await gitDiff(repoPath, path);
      hunksRef.current = hunks;
      if (hunks.length > 0) {
        updateDiffGutter(view, hunks);
        const leftView = leftViewRef.current;
        if (leftView) updateHeadDiffGutter(leftView, hunks);
      } else {
        clearDiffGutter(view);
        const leftView = leftViewRef.current;
        if (leftView) clearHeadDiffGutter(leftView);
      }
      refreshPlaceholders();
    } catch (err) {
      console.warn("[slTerminal] git diff 刷新失败:", err);
    }

    window.dispatchEvent(new CustomEvent("slterm:file-saved", {
      detail: { path: path.replace(/\\/g, "/"), panelId },
    }));
  }, [panelId, repoPath, refreshPlaceholders]);

  const handleSaveRef = useRef(handleSave);
  handleSaveRef.current = handleSave;

  // 自动换行 toggle——左右栏同步
  const toggleWordWrap = useCallback(() => {
    const leftView = leftViewRef.current;
    const rightView = rightViewRef.current;
    const wrapping = wordWrapRef.current;
    const ext = wrapping ? [] : EditorView.lineWrapping;
    leftView?.dispatch({ effects: leftWrapCompartment.current.reconfigure(ext) });
    rightView?.dispatch({ effects: rightWrapCompartment.current.reconfigure(ext) });
    wordWrapRef.current = !wrapping;
  }, []);

  // 注册为"当前聚焦编辑器"——Ctrl+S/Alt+Z 走 editor 命令派发到这里
  const editorActions = useMemo<EditorActions>(
    () => ({
      save: () => { void handleSaveRef.current(); },
      toggleWordWrap,
    }),
    [toggleWordWrap],
  );
  const activateEditor = useCallback(() => setActiveEditor(editorActions), [editorActions]);
  const deactivateEditor = useCallback(() => clearActiveEditor(editorActions), [editorActions]);

  // renderKey bridge effect 确保首次 "ready" 渲染后 ref.current 非 null
  usePanelFocus("editor", rightContainerRef.current, activateEditor, deactivateEditor);

  // 左栏也注册 focus——让 Alt+Z 在左栏聚焦时同样生效
  usePanelFocus("editor", leftContainerRef.current, activateEditor, deactivateEditor);

  // Ctrl+滚轮调节字体大小——左右栏容器各注册一次
  useFontSizeWheel(leftContainerRef.current, FONT_SIZE_MIN, FONT_SIZE_MAX, fontSizeRef, setEditorFontSize);
  useFontSizeWheel(rightContainerRef.current, FONT_SIZE_MIN, FONT_SIZE_MAX, fontSizeRef, setEditorFontSize);

  // 字体 Compartment 热切换：字号变化时仅 reconfigure，不销毁重建 EditorView
  useEffect(() => {
    const ext = createEditorFontExtension(editorFontSize);
    leftViewRef.current?.dispatch({ effects: leftFontCompartment.current.reconfigure(ext) });
    rightViewRef.current?.dispatch({ effects: rightFontCompartment.current.reconfigure(ext) });
  }, [editorFontSize]);

  // ── 右侧外部文件修改监听 ────────────────────────────────────

  useEffect(() => {
    const unlisten = onFsEvent((event) => {
      const currentPath = filePathRef.current;
      if (!currentPath) return;

      const normalizedCurrent = currentPath.replace(/\\/g, "/");
      if (justSavedRef.current) {
        justSavedRef.current = false;
        return;
      }

      const affected = event.paths.some(
        (p) => p.replace(/\\/g, "/") === normalizedCurrent,
      );
      if (!affected) return;
      if (event.kind !== "Modify") return;

      const view = rightViewRef.current;
      if (!view) return;

      if (dirtyRef.current) {
        const choice = window.confirm(
          `文件 "${currentPath}" 已被外部修改。\n\n当前编辑器有未保存的修改。\n\n确定 = 重载（丢弃本地修改）\n取消 = 保留本地修改`,
        );
        if (choice) {
          fs.readFile(currentPath).then((content) => {
            view.dispatch({
              changes: { from: 0, to: view.state.doc.length, insert: content },
            });
            dirtyRef.current = false;
          }).catch((err) => { console.warn("[slTerminal] 外部修改重载失败:", err); });
        }
      } else {
        fs.readFile(currentPath).then((content) => {
          view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: content },
          });
        }).catch((err) => { console.warn("[slTerminal] 外部修改重载失败:", err); });
      }
    });

    return () => { unlisten(); };
  }, []);

  // ── 左侧 .git 变更刷新 HEAD ─────────────────────────────────

  useEffect(() => {
    const unlisten = onFsEvent((event) => {
      const hasGitChange = event.paths.some(
        (p) => p.replace(/\\/g, "/").includes("/.git/"),
      );
      if (!hasGitChange) return;

      const queryPath = oldPath ?? filePath;
      gitFileAtHead(repoPath, queryPath).then((content) => {
        const leftView = leftViewRef.current;
        if (!leftView) return;
        leftView.dispatch({
          changes: { from: 0, to: leftView.state.doc.length, insert: content },
        });
      }).catch(() => { /* HEAD 不存在——保持当前展示 */ });
    });

    return () => { unlisten(); };
  }, [repoPath, filePath, oldPath]);

  // ── CM6 编辑器挂载 ──────────────────────────────────────────

  // 左侧只读 CM（HEAD）——内容变化时重建
  useEffect(() => {
    const container = leftContainerRef.current;
    if (!container || state.kind !== "ready") return;

    // 销毁旧 view
    leftViewRef.current?.destroy();
    leftViewRef.current = null;

    const { headContent } = state;

    const leftView = new EditorView({
      state: EditorState.create({
        doc: headContent,
        extensions: [
          basicSetup,
          oneDark,
          EditorView.theme({ "&": { height: "100%" } }),
          leftFontCompartment.current.of(createEditorFontExtension(editorFontSize)),
          leftWrapCompartment.current.of([]),
          search({ top: true }),
          highlightSelectionMatches(),
          keymap.of([...searchKeymap]),
          EditorState.readOnly.of(true),
          getLanguageExtension(filePath),
          headDiffGutter(),
          placeholderField,
        ],
      }),
      parent: container,
    });

    leftViewRef.current = leftView;

    // 初始应用 HEAD gutter
    if (hunksRef.current.length > 0) {
      updateHeadDiffGutter(leftView, hunksRef.current);
    }

    return () => {
      leftView.destroy();
      leftViewRef.current = null;
    };
  }, [state.kind, state.kind === "ready" ? (state as { headContent: string }).headContent : null, filePath]);

  // 右侧可编辑 CM（工作区）——内容变化时重建
  useEffect(() => {
    const container = rightContainerRef.current;
    if (!container || state.kind !== "ready") return;

    // 销毁旧 view
    rightViewRef.current?.destroy();
    rightViewRef.current = null;

    const { workdirContent } = state;

    const rightView = new EditorView({
      state: EditorState.create({
        doc: workdirContent,
        extensions: [
          basicSetup,
          oneDark,
          EditorView.theme({ "&": { height: "100%" } }),
          rightFontCompartment.current.of(createEditorFontExtension(editorFontSize)),
          rightWrapCompartment.current.of([]),
          search({ top: true }),
          highlightSelectionMatches(),
          keymap.of([...searchKeymap]),
          keymap.of([indentWithTab]),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) dirtyRef.current = true;
          }),
          getLanguageExtension(filePath),
          diffGutter(),
          placeholderField,
        ],
      }),
      parent: container,
    });

    rightViewRef.current = rightView;

    // 初始应用 workdir gutter + 占位
    if (hunksRef.current.length > 0) {
      updateDiffGutter(rightView, hunksRef.current);
    }

    // 延时应用占位——等待 CM6 布局完成
    const timer = setTimeout(() => {
      refreshPlaceholders();
    }, 50);

    return () => {
      clearTimeout(timer);
      rightView.destroy();
      rightViewRef.current = null;
    };
  }, [state.kind, state.kind === "ready" ? (state as { workdirContent: string }).workdirContent : null, filePath]);

  // ── 渲染 ─────────────────────────────────────────────────────

  if (state.kind === "loading") {
    return (
      <div style={{ ...centerStyle, flex: 1, display: "flex", alignItems: "center", justifyContent: "center", background: PANEL_BG }}>
        <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }}>加载中...</span>
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div style={{ ...centerStyle, flex: 1, display: "flex", alignItems: "center", justifyContent: "center", background: PANEL_BG }}>
        <span style={{ color: ERROR_FG, fontSize: 13 }}>{state.message}</span>
      </div>
    );
  }

  return (
    <div
      data-e2e="diff-panel"
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "row",
      }}
    >
      <div style={{ flex: "50%", display: "flex", minWidth: 0, borderRight: `1px solid ${SEPARATOR_BG}` }}>
        <div
          data-e2e="diff-left"
          ref={leftContainerRef}
          style={{ flex: 1, background: EDITOR_BG, overflow: "clip", minWidth: 0 }}
        />
      </div>
      <div style={{ flex: "50%", display: "flex", minWidth: 0 }}>
        <div
          data-e2e="diff-right"
          ref={rightContainerRef}
          style={{ flex: 1, background: EDITOR_BG, overflow: "clip", minWidth: 0 }}
        />
      </div>
    </div>
  );
};

const centerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

export default DiffPanel;
