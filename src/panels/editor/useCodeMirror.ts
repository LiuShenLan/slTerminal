// useCodeMirror — CodeMirror 6 生命周期管理 hook
//
// 职责：
// - 创建 EditorView（暗色 oneDark 主题 + basicSetup）
// - 打开文件时 ipc.fs.readFile → 填充内容
// - Ctrl+S → 有 filePath 直接保存，无 filePath 弹出"另存为"对话框（G3）
// - cleanup 中 view.destroy()（箭头函数调，防 this 丢失）

import { useEffect, useRef, useCallback } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState, Compartment, type Extension } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { oneDark } from "@codemirror/theme-one-dark";
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

export interface UseCodeMirrorOptions {
  /** 容器 DOM 元素 */
  container: HTMLElement | null;
  /** 要打开的文件路径（可选，空则新建空白缓冲区） */
  filePath?: string;
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

export function useCodeMirror({ container, filePath }: UseCodeMirrorOptions) {
  const viewRef = useRef<EditorView | null>(null);
  const filePathRef = useRef<string | undefined>(filePath);
  const langCompartment = useRef(new Compartment());

  /** Ctrl+S 保存 — G3: 无 filePath 时弹出另存为对话框 */
  const handleSave = useCallback(async () => {
    const view = viewRef.current;
    if (!view) return;

    let path = filePathRef.current;
    if (!path) {
      const selected = await save({
        defaultPath: "Untitled.txt",
        filters: [{ name: "所有文件", extensions: ["*"] }],
      });
      if (!selected) return;
      path = selected;
      filePathRef.current = path;
    }

    const content = view.state.doc.toString();
    fs.writeFile(path, content).catch((err) => {
      console.error("保存失败:", err);
    });
  }, []);

  useEffect(() => {
    if (!container) return;

    filePathRef.current = filePath;

    // 异步加载文件内容
    const initEditor = async () => {
      let doc = "";
      if (filePath) {
        try {
          doc = await fs.readFile(filePath);
        } catch (err) {
          console.error("读取文件失败:", err);
          doc = `// 读取失败: ${err}\n`;
        }
      }

      const saveKeymap = keymap.of([
        {
          key: "Mod-s",
          run: () => {
            handleSave();
            return true;
          },
          preventDefault: true,
        },
      ]);

      const view = new EditorView({
        state: EditorState.create({
          doc,
          extensions: [basicSetup, oneDark, langCompartment.current.of(getLanguageExtension(filePath)), saveKeymap],
        }),
        parent: container,
      });

      viewRef.current = view;
    };

    initEditor();

    return () => {
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

  return {
    /** 获取当前编辑器内容 */
    getContent: useCallback((): string => {
      return viewRef.current?.state.doc.toString() ?? "";
    }, []),
  };
}
