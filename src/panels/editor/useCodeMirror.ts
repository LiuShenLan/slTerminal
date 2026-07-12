// useCodeMirror — CodeMirror 6 生命周期管理 hook
//
// 职责：
// - 创建 EditorView（暗色 oneDark 主题 + basicSetup + search）
// - 打开文件时 ipc.fs.readFile → 填充内容 + 加载 diff 边栏
// - Ctrl+S → 有 filePath 直接保存，无 filePath 弹出"另存为"对话框（G3）
// - Ctrl+F 查找（@codemirror/search）
// - 监听外部文件改动（fs-event）→ 干净自动重载 / 脏弹窗选择
// - cleanup 中 view.destroy()（箭头函数调，防 this 丢失）

import { useEffect, useRef, useCallback, useMemo } from "react";
import { EditorView, keymap } from "@codemirror/view";
import {
  EditorState,
  Compartment,
  type Extension,
} from "@codemirror/state";
import { indentWithTab } from "@codemirror/commands";
import { basicSetup } from "codemirror";
import { oneDark } from "@codemirror/theme-one-dark";
import { search, searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { json } from "@codemirror/lang-json";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { markdown } from "@codemirror/lang-markdown";
import { xml } from "@codemirror/lang-xml";
import { save } from "../../ipc/dialog";
import { fs } from "../../ipc";
import { diffGutter, updateDiffGutter, clearDiffGutter } from "./gitGutter";
import { onFsEvent } from "../../ipc/notify";
import { gitDiff } from "../../ipc/git";
import { usePanelFocus } from "../../features/shortcuts";
import { setActiveEditor, clearActiveEditor, type EditorActions } from "./activeEditor";

/** 编辑器字体 CSS spec —— 可独立测试 */
export const EDITOR_FONT_SPEC = {
  ".cm-scroller": { fontFamily: `"JetBrains Mono", monospace` },
};
/** 编辑器字体主题 —— JetBrains Mono Regular */
export const EDITOR_FONT_THEME = EditorView.theme(EDITOR_FONT_SPEC);

/** 创建带 fontSize 的编辑器字体主题扩展（用于 Compartment 热切换） */
export function createEditorFontExtension(fontSize: number): Extension {
  return EditorView.theme({
    ".cm-scroller": {
      fontFamily: `"JetBrains Mono", monospace`,
      fontSize: `${fontSize}px`,
    },
  });
}

export interface UseCodeMirrorOptions {
  /** 容器 DOM 元素 */
  container: HTMLElement | null;
  /** 要打开的文件路径（可选，空则新建空白缓冲区） */
  filePath?: string;
  /** 面板 ID（用于 save-as 事件通知） */
  panelId?: string;
  /** 编辑器字体大小（运行时动态调节，默认 14） */
  fontSize?: number;
  /** 字体大小变更回调（Ctrl+Wheel 触发） */
  onFontSizeChange?: (size: number) => void;
}

/** 根据文件扩展名返回对应的 CodeMirror 语言扩展 */
export function getLanguageExtension(filename?: string): Extension {
  if (!filename) return javascript();

  const ext = filename.slice(filename.lastIndexOf(".")).toLowerCase();
  switch (ext) {
    case ".js":
    case ".ts":
    case ".tsx":
    case ".jsx":
    case ".mjs":
    case ".cjs":
      return javascript();
    case ".py":
    case ".pyw":
      return python();
    case ".rs":
      return rust();
    case ".json":
    case ".jsonc":
      return json();
    case ".html":
    case ".htm":
      return html();
    case ".css":
    case ".scss":
    case ".less":
      return css();
    case ".md":
    case ".markdown":
      return markdown();
    case ".xml":
    case ".svg":
      return xml();
    default:
      return javascript();
  }
}

export function useCodeMirror({ container, filePath, panelId, fontSize, onFontSizeChange }: UseCodeMirrorOptions) {
  const viewRef = useRef<EditorView | null>(null);
  const filePathRef = useRef<string | undefined>(filePath);
  const langCompartment = useRef(new Compartment());
  /** 字体 Compartment —— 热切换字体大小不丢文档状态 */
  const fontCompartment = useRef(new Compartment());
  /** 自动换行 Compartment —— Alt+Z 热切换，默认关闭 */
  const wrapCompartment = useRef(new Compartment());
  /** 自动换行当前状态 ref —— toggle 读取，避免 jsdom 中 view.lineWrapping 不可靠 */
  const wordWrapRef = useRef(false);
  /** 字体大小 ref —— wheel handler 中读取，避免闭包捕获过时值 */
  const fontSizeRef = useRef<number>(fontSize ?? 14);
  // 保存后短时间内抑制 fs-event auto-reload，避免将自己的写入误判为外部改动、
  // 执行全量文档替换从而破坏 diff gutter 的标记（RangeSet.map 会把所有 marker 清空）
  const justSavedRef = useRef(false);
  // P1-17: 组件卸载标记，防止 async initEditor 在 unmount 后操作 DOM
  const mountedRef = useRef(false);

  /** Ctrl+S 保存 — G3: 无 filePath 时弹出另存为对话框 */
  const handleSave = useCallback(async () => {
    const view = viewRef.current;
    if (!view) return;

    const oldPath = filePathRef.current;
    let path = oldPath;
    if (!path) {
      const selected = await save({
        defaultPath: "Untitled.txt",
        filters: [{ name: "所有文件", extensions: ["*"] }],
      });
      if (!selected) return;
      path = selected;
      filePathRef.current = path;
    }

    // 标记为自己保存，防止后续 fs-event 误判为外部改动而清空 diff 标记
    justSavedRef.current = true;

    const content = view.state.doc.toString();

    // 等待磁盘写入完成再刷新 diff 和 git 着色（避免 fire-and-forget 时序竞态）
    try {
      await fs.writeFile(path, content);
    } catch (err) {
      // P1-05: 保存失败时显示通知，保留编辑器内容不清空
      window.alert(`保存失败: ${err}`);
      return;
    }

    // P13: 保存后刷新 diff gutter
    const normalizedPath = path.replace(/\\/g, "/");
    const repoDir =
      normalizedPath.lastIndexOf("/") >= 0
        ? normalizedPath.slice(0, normalizedPath.lastIndexOf("/"))
        : ".";
    gitDiff(repoDir, normalizedPath)
      .then((hunks) => {
        if (hunks.length > 0) {
          if (viewRef.current) updateDiffGutter(viewRef.current, hunks);
        } else {
          // 文件已干净（匹配 HEAD）→ 清除旧 diff 标记
          if (viewRef.current) clearDiffGutter(viewRef.current);
        }
      })
      // P2-15: gitDiff 失败时 console.warn，不再静默吞错
      .catch((err) => { console.warn("[slTerminal] git diff 刷新失败:", err); });

    // 通知标题管理器：路径变更（空白编辑器首次保存 或 另存为到新路径）
    if (oldPath !== path && panelId) {
      const oldNormalized = oldPath ? oldPath.replace(/\\/g, "/") : null;
      window.dispatchEvent(new CustomEvent("slterm:file-saved-as", {
        detail: { panelId, oldPath: oldNormalized, newPath: normalizedPath },
      }));
    }

    // 通知文件浏览器刷新 git 着色
    window.dispatchEvent(new CustomEvent("slterm:file-saved", {
      detail: { path: normalizedPath, panelId },
    }));
  }, [panelId]);

  // Ctrl+S 迁入 ShortcutRegistry（editor context）：window capture 命中后 stopPropagation
  // 屏蔽 CodeMirror 的 keymap；Ctrl+F/撤销等未注册 → 注册表 miss → 冒泡回 CM 内部 keymap。
  // 命令在 App 一次性注册，本实例聚焦时经 setActiveEditor 设为派发目标（多编辑器下始终保存聚焦实例）。
  // 用 ref 保持 handleSave 最新引用（依赖 panelId 会变）。
  const handleSaveRef = useRef(handleSave);
  handleSaveRef.current = handleSave;
  const editorActions = useMemo<EditorActions>(
    () => ({
      save: () => { void handleSaveRef.current(); },
      /** Alt+Z 切换自动换行 */
      toggleWordWrap: () => {
        const view = viewRef.current;
        if (!view) return;
        const wrapping = wordWrapRef.current;
        view.dispatch({
          effects: wrapCompartment.current.reconfigure(
            wrapping ? [] : EditorView.lineWrapping,
          ),
        });
        wordWrapRef.current = !wrapping;
      },
    }),
    [],
  );
  const activateEditor = useCallback(() => setActiveEditor(editorActions), [editorActions]);
  const deactivateEditor = useCallback(() => clearActiveEditor(editorActions), [editorActions]);
  usePanelFocus("editor", container, activateEditor, deactivateEditor);

  useEffect(() => {
    if (!container) return;

    filePathRef.current = filePath;

    // 异步加载文件内容
    // P1-17: fire-and-forget async，开头标记 mounted，await 后检查标记再操作 DOM
    const initEditor = async () => {
      mountedRef.current = true;
      let doc = "";

      if (filePath) {
        try {
          doc = await fs.readFile(filePath);
          // P2-10: 大文件检查 — UTF-8 文本 length 近似文件字节数
          const sizeHint = doc.length;
          if (sizeHint > 10_000_000) {
            // >10MB：直接拒绝，将文档设为错误提示
            doc = `// [slTerminal] 文件过大（约${(sizeHint / 1_000_000).toFixed(1)}MB），已拒绝打开以保护内存。`;
            filePathRef.current = undefined; // 防止误保存覆盖原文件
          } else if (sizeHint > 1_000_000) {
            // >1MB：弹窗警告，用户可选择继续或取消
            const proceed = window.confirm(
              `文件较大（约${(sizeHint / 1_000_000).toFixed(1)}MB），打开可能影响性能。\n\n确定继续？`,
            );
            if (!proceed) {
              doc = `// [slTerminal] 用户取消打开大文件（约${(sizeHint / 1_000_000).toFixed(1)}MB）。`;
              filePathRef.current = undefined;
            }
          }
        } catch (err) {
          console.error("读取文件失败:", err);
          doc = `// 读取失败: ${err}\n`;
        }
      }

      // P1-17: 组件可能已在 await 期间卸载，检查后避免 EditorView DOM 泄漏
      if (!mountedRef.current) return;

      const view = new EditorView({
        state: EditorState.create({
          doc,
          extensions: [
            basicSetup,
            oneDark,
            // .cm-editor 高度→.cm-scroller height:100%约束→溢出→滚动条。
            // 如缺失，.cm-editor height:auto(=内容高)→scroller=内容高→无溢出→无滚动条。
            EditorView.theme({ "&": { height: "100%" } }),
            fontCompartment.current.of(createEditorFontExtension(fontSize ?? 14)),
            wrapCompartment.current.of([]), // 默认关闭自动换行
            search({ top: true }),
            highlightSelectionMatches(),
            keymap.of([...searchKeymap]),
            // Tab 缩进 / Shift+Tab 反缩进（basicSetup 出于无障碍默认不绑 Tab，此处显式启用）
            keymap.of([indentWithTab]),
            // D3: 跟踪文档修改
            EditorView.updateListener.of((update) => {
              if (update.docChanged) dirtyRef.current = true;
            }),
            langCompartment.current.of(getLanguageExtension(filePath)),
            diffGutter(),
          ],
        }),
        parent: container,
      });

      viewRef.current = view;

      // D1: 文件打开后加载 diff 边栏
      if (filePath) {
        const normalizedPath = filePath.replace(/\\/g, "/");
        const parentDir =
          normalizedPath.lastIndexOf("/") >= 0
            ? normalizedPath.slice(0, normalizedPath.lastIndexOf("/"))
            : ".";
        try {
          const loadedHunks = await gitDiff(parentDir, normalizedPath);
          if (loadedHunks.length > 0) {
            updateDiffGutter(view, loadedHunks);
          }
        } catch {
          // 非 git 仓库，diff 不可用，静默
        }
      }
    };

    initEditor();

    return () => {
      // P1-17: 标记组件已卸载，阻止 pending async 操作 DOM
      mountedRef.current = false;
      // 箭头函数调 destroy，防止 this 丢失
      const cleanup = () => {
        viewRef.current?.destroy();
        viewRef.current = null;
      };
      cleanup();
    };
  }, [container, filePath, handleSave]);

  // D3: filePath 变化时重新配置语言扩展（Compartment.reconfigure 不丢失文档状态）
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: langCompartment.current.reconfigure(getLanguageExtension(filePath)),
    });
    filePathRef.current = filePath;
  }, [filePath]);

  // D3: 脏状态跟踪
  const dirtyRef = useRef(false);

  // D3: 监听外部文件改动
  useEffect(() => {
    const unlisten = onFsEvent((event) => {
      // 跳过自己保存触发的文件事件，避免 auto-reload 清空 diff gutter 标记
      if (justSavedRef.current) {
        justSavedRef.current = false;
        return;
      }

      const currentPath = filePathRef.current;
      if (!currentPath) return;

      // 规范化路径比较
      const normalizedCurrent = currentPath.replace(/\\/g, "/");
      const affected = event.paths.some(
        (p) => p.replace(/\\/g, "/") === normalizedCurrent,
      );
      if (!affected) return;

      // 仅处理 Modify 事件
      if (event.kind !== "Modify") return;

      const view = viewRef.current;
      if (!view) return;

      if (dirtyRef.current) {
        // 有未保存修改 → 弹窗选择
        const choice = window.confirm(
          `文件 "${currentPath}" 已被外部修改。\n\n当前编辑器有未保存的修改。\n\n• 确定 = 重载（丢弃本地修改）\n• 取消 = 保留本地修改`,
        );
        if (choice) {
          // 重载
          fs.readFile(currentPath).then((content) => {
            view.dispatch({
              changes: {
                from: 0,
                to: view.state.doc.length,
                insert: content,
              },
            });
            dirtyRef.current = false;
          // P2-16: 外部修改重载失败时 console.warn
          }).catch((err) => { console.warn("[slTerminal] 外部修改重载失败:", err); });
        }
      } else {
        // 无修改 → 自动重载
        fs.readFile(currentPath).then((content) => {
          view.dispatch({
            changes: {
              from: 0,
              to: view.state.doc.length,
              insert: content,
            },
          });
          // P2-16: 外部修改重载失败时 console.warn
        }).catch((err) => { console.warn("[slTerminal] 外部修改重载失败:", err); });
      }
    });

    return () => {
      unlisten();
    };
  }, []);

  // 字体大小动态调节：fontSize 变化时通过 Compartment 热切换，不丢文档状态
  useEffect(() => {
    const view = viewRef.current;
    if (!view || fontSize === undefined) return;

    fontSizeRef.current = fontSize;
    view.dispatch({
      effects: fontCompartment.current.reconfigure(
        createEditorFontExtension(fontSize)
      ),
    });
  }, [fontSize]);

  // Ctrl+鼠标滚轮 调节字体大小
  useEffect(() => {
    if (!container || !onFontSizeChange) return;

    const CLAMP_MIN = 8;
    const CLAMP_MAX = 32;

    const handleWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return; // 非 Ctrl 滚轮透传

      e.preventDefault();
      e.stopPropagation();

      const currentSize = fontSizeRef.current;
      const direction = e.deltaY < 0 ? 1 : -1; // 上滚放大，下滚缩小
      const newSize = Math.max(CLAMP_MIN, Math.min(CLAMP_MAX, currentSize + direction));

      if (newSize !== currentSize) {
        onFontSizeChange(newSize);
      }
    };

    container.addEventListener("wheel", handleWheel, { capture: true, passive: false });

    return () => {
      container.removeEventListener("wheel", handleWheel, { capture: true });
    };
  }, [container, onFontSizeChange]);

  return {
    /** 获取当前编辑器内容 */
    getContent: useCallback((): string => {
      return viewRef.current?.state.doc.toString() ?? "";
    }, []),

    /** 标记为干净（Ctrl+S 保存后调用） */
    markClean: useCallback(() => {
      dirtyRef.current = false;
    }, []),

    /** 标记为脏（文档被修改时调用） */
    markDirty: useCallback(() => {
      dirtyRef.current = true;
    }, []),
  };
}
