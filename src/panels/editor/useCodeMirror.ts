// useCodeMirror — CodeMirror 6 生命周期管理 hook
//
// 职责：
// - 创建 EditorView（暗色 oneDark 主题 + basicSetup + search）
// - 打开文件时 ipc.fs.readFile → 填充内容 + 加载 diff 边栏
// - Ctrl+S → 有 filePath 直接保存，无 filePath 弹出"另存为"对话框（G3）
// - Ctrl+F 查找（@codemirror/search）
// - 监听外部文件改动（fs-event）→ 干净自动重载 / 脏弹窗选择
// - cleanup 中 view.destroy()（箭头函数调，防 this 丢失）

import { useEffect, useRef, useCallback } from "react";
import { EditorView, keymap } from "@codemirror/view";
import {
  EditorState,
  Compartment,
  type Extension,
} from "@codemirror/state";
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
import { diffGutter, updateDiffGutter } from "./gitGutter";
import { listen } from "@tauri-apps/api/event";
import { gitDiff } from "../../ipc/git";

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

    // P13: 保存后刷新 diff gutter
    const normalizedPath = path.replace(/\\/g, "/");
    const repoDir =
      normalizedPath.lastIndexOf("/") >= 0
        ? normalizedPath.slice(0, normalizedPath.lastIndexOf("/"))
        : ".";
    gitDiff(repoDir, normalizedPath)
      .then((hunks) => {
        if (hunks.length > 0 && viewRef.current) {
          updateDiffGutter(viewRef.current, hunks);
        }
      })
      .catch(() => {});
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
          extensions: [
            basicSetup,
            oneDark,
            search({ top: true }),
            highlightSelectionMatches(),
            keymap.of([...searchKeymap]),
            // D3: 跟踪文档修改
            EditorView.updateListener.of((update) => {
              if (update.docChanged) dirtyRef.current = true;
            }),
            langCompartment.current.of(getLanguageExtension(filePath)),
            diffGutter(),
            saveKeymap,
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
        } catch (err) {
          // 非 git 仓库静默（git2 打开仓库失败），其他错误 console.warn
          const msg = String(err ?? "");
          if (!msg.includes("打开仓库失败")) {
            console.warn("加载 diff 边栏失败:", err);
          }
        }
      }
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

  // D3: 脏状态跟踪
  const dirtyRef = useRef(false);

  // D3: 监听外部文件改动
  useEffect(() => {
    const unlisten = listen<{ paths: string[]; kind: string }>(
      "fs-event",
      (event) => {
        const currentPath = filePathRef.current;
        if (!currentPath) return;

        // 规范化路径比较
        const normalizedCurrent = currentPath.replace(/\\/g, "/");
        const affected = event.payload.paths.some(
          (p) => p.replace(/\\/g, "/") === normalizedCurrent,
        );
        if (!affected) return;

        // 仅处理 Modify 事件
        if (event.payload.kind !== "Modify") return;

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
            }).catch(() => {});
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
          }).catch(() => {});
        }
      },
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

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
