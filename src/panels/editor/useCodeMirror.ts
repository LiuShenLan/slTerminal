// useCodeMirror — CodeMirror 6 生命周期管理 hook
//
// 职责：
// - 创建 EditorView（暗色 oneDark 主题 + basicSetup）
// - 打开文件时 ipc.fs.readFile → 填充内容
// - Ctrl+S → 有 filePath 直接保存，无 filePath 弹出"另存为"对话框（G3）
// - cleanup 中 view.destroy()（箭头函数调，防 this 丢失）

import { useEffect, useRef, useCallback } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { basicSetup } from "codemirror";
import { oneDark } from "@codemirror/theme-one-dark";
import { save } from "@tauri-apps/plugin-dialog";
import { fs } from "../../ipc";

export interface UseCodeMirrorOptions {
  /** 容器 DOM 元素 */
  container: HTMLElement | null;
  /** 要打开的文件路径（可选，空则新建空白缓冲区） */
  filePath?: string;
}

export function useCodeMirror({ container, filePath }: UseCodeMirrorOptions) {
  const viewRef = useRef<EditorView | null>(null);
  const filePathRef = useRef<string | undefined>(filePath);

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
          extensions: [basicSetup, oneDark, saveKeymap],
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

  return {
    /** 获取当前编辑器内容 */
    getContent: useCallback((): string => {
      return viewRef.current?.state.doc.toString() ?? "";
    }, []),
  };
}
