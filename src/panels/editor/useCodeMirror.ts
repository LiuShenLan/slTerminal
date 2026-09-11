// useCodeMirror — CodeMirror 6 生命周期管理 hook
//
// 职责：
// - 创建 EditorView（暗色主题扩展 + basicSetup + search）
// - 打开文件时 ipc.fs.readFile → 填充内容 + 加载 diff 边栏
// - Ctrl+S → 有 filePath 直接保存，无 filePath 弹出"另存为"对话框（G3）
// - Ctrl+F 查找（@codemirror/search）
// - 监听外部文件改动（fs-event）→ 干净自动重载 / 脏弹窗选择
// - cleanup 中 view.destroy()（箭头函数调，防 this 丢失）

import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import {
  EditorState,
  Compartment,
  type Extension,
} from "@codemirror/state";
import { indentWithTab } from "@codemirror/commands";
import { basicSetup } from "codemirror";
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
import { normalizePath } from "../../lib/path";
// FE-01: 原生弹窗（window.alert/confirm）统一改为应用内浮层（toast / confirmDialog）
// FE-10: 错误消息统一经 getErrorMessage（契约：src/ipc/appError.ts，src/lib re-export）
import { confirmDialog, toast, getErrorMessage } from "../../lib";
import { fs } from "../../ipc";
import { diffGutter, updateDiffGutter, clearDiffGutter } from "./gitGutter";
import { repaintGuard } from "./repaintGuard";
import { onFsEvent } from "../../ipc/notify";
import { gitDiff } from "../../ipc/git";
import { usePanelFocus } from "../../features/shortcuts";
import { setActiveEditor, clearActiveEditor, type EditorActions } from "./activeEditor";
import { useFontSizeWheel } from "../../lib/useFontSizeWheel";
import { FONT_SIZE_MIN, FONT_SIZE_MAX } from "../../stores/fontSize";
import {
  createEditorThemeSlot,
  type EditorThemeSlot,
} from "../../theme/editorThemeSlot";

/** 文件大小上限（字节）——超过此值不再可编辑，改只读分片浏览（CP-022 引导语义） */
export const MAX_FILE_SIZE_BYTES = 10_000_000;

/**
 * 大文件信号（CP-022）——>10MB 文件向上报告,宿主面板据此以 LargeFileViewer
 * 只读分片浏览替代 CM 编辑区（同面板内形态切换,不经 panelRegistry）。
 * FE-04：仅 filePath——真实大小由查看器挂载 fs_stat 自取（原 sizeBytes 为
 * UTF-16 码元近似，CJK 文件系统性偏小）。
 */
export interface LargeFileSignal {
  filePath: string;
}
/** 大文件警告阈值（字节）——超过此值弹窗确认 */
export const LARGE_FILE_WARN_BYTES = 1_000_000;

/**
 * CP-029(S03): 打开后磁盘核对延迟——外部修改事件补偿窗口。
 * 定责取证(2026-09-07 E2E dirty→clean 用例二分定责)：两条事件丢失机制会让
 * 「打开后立即被外部改写」的文件停留在陈旧内容且无后续事件可触发 reload——
 * ① SEC-01 项目激活 → 后端 watcher 注册完成的空窗（双 setProjectRoot 串行 +
 * spawn_blocking 注册链实测约 50-200ms），空窗内写盘零事件；
 * ② notify-debouncer-full 对同路径 Create 后 300ms 去抖窗口内的 Modify 吞并不发
 * （crate 文档明示 "Doesn't emit Modify events after a Create event"，实测 259ms
 * 间隔 Create+Modify 只发 Create）。故编辑器打开磁盘文件后延迟复核一次磁盘内容，
 * 不一致即走与 fs-event 相同重载路径——事件到达时核对幂等（内容已一致则跳过）。
 * 取值须 > 注册链上限 + 去抖窗口(300ms) 的余量；过大会延迟补偿呈现。
 */
export const OPEN_RECHECK_DELAY_MS = 1500;

/** 编辑器字体 CSS spec —— 可独立测试 */
export const EDITOR_FONT_SPEC = {
  ".cm-scroller": { fontFamily: `"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace` },
};
/** 编辑器字体主题 —— JetBrains Mono Regular */
export const EDITOR_FONT_THEME = EditorView.theme(EDITOR_FONT_SPEC);

/** 创建带 fontSize 的编辑器字体主题扩展（用于 Compartment 热切换） */
export function createEditorFontExtension(fontSize: number): Extension {
  return EditorView.theme({
    ".cm-scroller": {
      fontFamily: `"JetBrains Mono", "Cascadia Mono", Consolas, "Microsoft YaHei UI", monospace`,
      fontSize: `${fontSize}px`,
    },
  });
}

/** 缓冲全文安全读取（CP-029；测试桩等非标准 doc 形态 toString 可能缺失/抛错） */
function safeDocText(view: EditorView): string {
  try {
    return view.state.doc.toString();
  } catch {
    return "";
  }
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
  /**
   * 初始文档快照（草稿回填，docViewer 面板形态切换恢复用）：有值则跳过磁盘
   * 读取直接以此建缓冲（免二次 IPC）。缺省走 filePath 读盘（EditorPanel 行为不变）。
   */
  initialDoc?: string;
  /**
   * 文档内容变更回调（三源：init = 缓冲建立完成 / edit = 每次 docChanged /
   * reload = 外部修改重载成功）——面板级 docRef 真值源同步用。
   * 回调仅在传入时挂载（EditorPanel 不传 → updateListener 零额外开销）。
   */
  onDocContent?: (text: string, source: "init" | "edit" | "reload") => void;
  /**
   * git diff gutter（默认 true = EditorPanel/DiffPanel 现行行为）。
   * docViewer 预览面板（md/html 编辑源码态）传 false：不加载 diff gutter
   * 扩展、不读 git、保存后不刷新 gutter。
   */
  gitGutterEnabled?: boolean;
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

/** 获取文件父目录路径；无有效父目录时返回 null（跳过 gitDiff） */
function getParentDir(filePath: string): string | null {
  const normalized = normalizePath(filePath);
  const lastSlash = normalized.lastIndexOf("/");
  if (lastSlash < 0) return null;
  const parent = normalized.slice(0, lastSlash);
  // 根目录下文件（如 "/file" → ""）
  if (parent === "") return null;
  // 仅盘符（如 "D:"）→ 补斜杠为 "D:/"
  if (/^[A-Za-z]:$/.test(parent)) return parent + "/";
  return parent;
}

export function useCodeMirror({
  container,
  filePath,
  panelId,
  fontSize,
  onFontSizeChange,
  initialDoc,
  onDocContent,
  gitGutterEnabled = true,
}: UseCodeMirrorOptions) {
  const viewRef = useRef<EditorView | null>(null);
  const filePathRef = useRef<string | undefined>(filePath);
  const langCompartment = useRef(new Compartment());
  /** 字体 Compartment —— 热切换字体大小不丢文档状态 */
  const fontCompartment = useRef(new Compartment());
  /** CP-039：主题热切换槽——view 存活期方案切换经 Compartment 重配置，不重建编辑器 */
  const themeSlotRef = useRef<EditorThemeSlot | null>(null);
  if (themeSlotRef.current === null) themeSlotRef.current = createEditorThemeSlot();
  /** 自动换行 Compartment —— Alt+Z 热切换，默认关闭 */
  const wrapCompartment = useRef(new Compartment());
  /** 自动换行当前状态 ref —— toggle 读取，避免 jsdom 中 view.lineWrapping 不可靠 */
  const wordWrapRef = useRef(false);
  /** 字体大小 ref —— wheel handler 中读取，避免闭包捕获过时值 */
  const fontSizeRef = useRef<number>(fontSize ?? 14);
  /** onDocContent ref —— 回调经 ref 转发，防 effect 闭包过期（fontSizeRef 同模式） */
  const onDocContentRef = useRef(onDocContent);
  onDocContentRef.current = onDocContent;
  /** gitGutterEnabled ref —— 保存刷新/加载分支经 ref 读取，不扩 effect 依赖 */
  const gitGutterEnabledRef = useRef(gitGutterEnabled);
  gitGutterEnabledRef.current = gitGutterEnabled;
  /** initialDoc ref —— effect 读取（依赖数组含 initialDoc，切换时重建缓冲） */
  const initialDocRef = useRef(initialDoc);
  initialDocRef.current = initialDoc;
  // 保存后按文件路径抑制 fs-event auto-reload，避免将自己的写入误判为外部改动、
  // 执行全量文档替换从而破坏 diff gutter 的标记（RangeSet.map 会把所有 marker 清空）
  // Set<string> 按路径去重：多编辑器同时保存时 boolean 标记会被他人清空，Set 各自独立
  const justSavedRef = useRef(new Set<string>());
  // P1-17: 组件卸载标记，防止 async initEditor 在 unmount 后操作 DOM
  const mountedRef = useRef(false);
  // generation 计数器：filePath 每次变化时递增，异步回调中比对以丢弃过期结果
  const genRef = useRef(0);
  /**
   * 大文件信号（CP-022）: >10MB 文件不再以错误 doc 展示——向上报告,宿主面板切换
   * LargeFileViewer 只读分片浏览（同面板内形态切换）。filePathRef 清空语义保留
   * （防误保存覆盖原文件）。
   */
  const [largeFile, setLargeFile] = useState<LargeFileSignal | null>(null);
  // 文件切换即清旧信号——独立于容器 effect（大文件形态下 CM 容器未挂载,容器驱动的
  // initEditor 不会执行;宿主据此切回 CM 编辑形态后再按新 filePath 正常打开）
  useEffect(() => {
    setLargeFile((cur) => (cur === null || cur.filePath === filePath ? cur : null));
  }, [filePath]);

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

    // 标记自己保存的文件路径，防止后续 fs-event 误判为外部改动而清空 diff 标记
    // Set 按路径去重：多编辑器同时保存时各自路径独立，不互相影响
    justSavedRef.current.add(path);

    const content = view.state.doc.toString();

    // 等待磁盘写入完成再刷新 diff 和 git 着色（避免 fire-and-forget 时序竞态）
    try {
      await fs.writeFile(path, content);
    } catch (err) {
      // P1-05: 保存失败时显示通知，保留编辑器内容不清空（FE-01: alert → toast）
      // FE-44: 错误消息统一经 getErrorMessage（解析 IPC AppError 结构化消息）
      toast.show("error", `保存失败: ${getErrorMessage(err)}`);
      return;
    }

    // P13: 保存后刷新 diff gutter（gitGutterEnabled=false 的预览面板跳过）
    const normalizedPath = normalizePath(path);
    const repoDir = getParentDir(normalizedPath);
    if (repoDir && gitGutterEnabledRef.current) {
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
        // FE-10: 消息统一经 getErrorMessage
        .catch((err) => { console.warn("[slTerminal] git diff 刷新失败:", getErrorMessage(err)); });
    }

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
    const gen = ++genRef.current;
    // CP-029(S03): 打开后核对定时器（cleanup 清除；见 initEditor 内调度点）
    let recheckTimer: ReturnType<typeof setTimeout> | undefined;
    // CP-039: 主题槽订阅取消函数——view 在 async initEditor 内创建，须以 effect 作用域
    // 变量桥接进 return cleanup（订阅先于 destroy 取消，防对已销毁 view dispatch）
    let unbindTheme: (() => void) | undefined;

    // 异步加载文件内容
    // P1-17: fire-and-forget async，开头标记 mounted，await 后检查标记再操作 DOM
    const initEditor = async () => {
      mountedRef.current = true;
      let doc = "";

      // initialDoc 快照（docViewer 草稿回填）：跳过磁盘读取（免二次 IPC）；无快照
      // 且无 filePath = 空白缓冲。大文件检查仅适用磁盘读取路径（快照已过检/回填源）
      const useSnapshot = initialDocRef.current !== undefined;
      if (useSnapshot) {
        doc = initialDocRef.current ?? "";
      } else if (filePath) {
        try {
          // FE-08: stat 预检前置——>10MB 零读盘直接引导只读浏览（原形态先全量读盘再拒绝,
          // 内存保护不覆盖读盘一步）；stat 失败直接落 catch（文件不存在/权限——readFile 必同败）
          const meta = await fs.statFile(filePath);
          // generation 检查：filePath 已切换则丢弃过期结果
          if (genRef.current !== gen) return;
          if (meta.sizeBytes > MAX_FILE_SIZE_BYTES) {
            // CP-022: >10MB 不再置错误 doc——向上报告 largeFile 信号,EditorPanel
            // 检测后以 LargeFileViewer 只读分片浏览替代 CM 编辑区;filePathRef 清空
            // 保留（防误保存覆盖原文件）。返回前不创建 EditorView（无编辑实例）。
            filePathRef.current = undefined;
            setLargeFile({ filePath });
            return;
          } else if (meta.sizeBytes > LARGE_FILE_WARN_BYTES) {
            // >1MB：弹窗警告，用户可选择继续或取消（FE-01: confirm → confirmDialog，确认=继续/取消=中止）
            // 文案用 stat 真实字节数（原 doc.length 为 UTF-16 码元近似，CJK 文件系统性偏小）
            const proceed = await confirmDialog({
              title: "打开大文件",
              message: `文件较大（约${(meta.sizeBytes / 1_000_000).toFixed(1)}MB），打开可能影响性能。`,
              confirmText: "继续",
            });
            if (!proceed) {
              doc = `// [slTerminal] 用户取消打开大文件（约${(meta.sizeBytes / 1_000_000).toFixed(1)}MB）。`;
              filePathRef.current = undefined;
            }
          }
          // FE-08: 预检拦截 / 用户取消（filePathRef.current === undefined——>MAX 分支
          // 已在上方 return,取消分支已清 ref）→ 不读盘；取消场景以取消文案建缓冲
          // （保留另存为出口,原语义）
          const precheckBlocked = filePathRef.current === undefined;
          if (!precheckBlocked) {
            doc = await fs.readFile(filePath);
            // generation 检查：filePath 已切换则丢弃过期结果
            if (genRef.current !== gen) return;
            // 读后复核（TOCTOU 防线）：stat 与读盘间文件长大超 10MB → 仍引导只读浏览
            if (doc.length > MAX_FILE_SIZE_BYTES) {
              filePathRef.current = undefined;
              setLargeFile({ filePath });
              return;
            }
          }
        } catch (err) {
          console.error("读取文件失败:", err);
          doc = `// 读取失败: ${err}\n`;
        }
      }

      // P1-17: 组件可能已在 await 期间卸载，检查后避免 EditorView DOM 泄漏
      if (!mountedRef.current) return;
      // generation 检查：filePath 已切换则丢弃过期结果
      if (genRef.current !== gen) return;

      const view = new EditorView({
        state: EditorState.create({
          doc,
          extensions: [
            basicSetup,
            // 主题热切换槽（CP-039）：syntax→theme→overrides 三扩展经槽内
            // editorThemeBundle() 单点固化顺序（ACC-05——syntax 先于 theme，
            // reverse 层叠后自定义规则排最后=恒胜）；方案切换 Compartment 重配置不重建
            themeSlotRef.current!.extension,
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
            // D3: 跟踪文档修改 + docRef 真值源回传（仅 onDocContent 挂载时付
            // toString 成本——EditorPanel 等不传回调的消费方零额外开销）
            EditorView.updateListener.of((update) => {
              if (update.docChanged) {
                dirtyRef.current = true;
                const cb = onDocContentRef.current;
                if (cb) cb(update.state.doc.toString(), "edit");
              }
            }),
            langCompartment.current.of(getLanguageExtension(filePath)),
            // gitGutterEnabled=false（docViewer 预览面板）不加载 diff gutter 扩展
            ...(gitGutterEnabledRef.current ? [diffGutter()] : []),
            // WebView2 陈旧光栅规避（见 repaintGuard.ts——引擎缺陷，撤销须复核）
            repaintGuard(),
          ],
        }),
        parent: container,
      });

      viewRef.current = view;
      // CP-039: 订阅方案变更——切换即 Compartment 重配置主题（view 存活期热切换）
      //（ref 惰性建槽于 render 期，effect/initEditor 运行时恒非空）
      unbindTheme = themeSlotRef.current!.bind(view);

      // 缓冲建立完成 → init 源回传（面板 docRef 初始化/草稿回填确认）
      const cb = onDocContentRef.current;
      if (cb) cb(view.state.doc.toString(), "init");

      // CP-029(S03): 打开后磁盘核对（外部修改事件补偿，见 OPEN_RECHECK_DELAY_MS
      // 注释——watcher 注册空窗/去抖窗口吞并会让打开瞬间的外部写盘零事件，编辑器
      // 停留在陈旧内容）。延迟复核磁盘与「打开时读到内容」的差异——baseline 比对：
      // 用户打字（doc≠baseline）而磁盘未变的常态不误报；磁盘真变了 → 走与 fs-event
      // 相同的重载/确认路径。仅磁盘读取路径生效（initialDoc 快照 = 草稿优先，跳过
      // 核对避免草稿被磁盘旧内容覆盖）；大文件拒绝/取消已清 filePathRef → 自动取消。
      if (!useSnapshot && filePath) {
        const openedPath = filePath;
        const baselineContent = doc; // 打开时读到的磁盘内容（含读失败占位文本）
        recheckTimer = setTimeout(() => {
          void (async () => {
            // 已切换文件（gen 变化）/已卸载/大文件拒绝或取消（ref 已清）→ 放弃核对
            if (genRef.current !== gen || !mountedRef.current) return;
            if (filePathRef.current !== openedPath) return;
            // 核对是静默补偿探测：读盘失败只 warn 不 toast（与事件路径区分）
            await applyExternalChangeRef.current(openedPath, {
              mode: "recheck",
              toastOnError: false,
              baseline: baselineContent,
            });
          })();
        }, OPEN_RECHECK_DELAY_MS);
      }

      // D1: 文件打开后加载 diff 边栏
      if (filePath && gitGutterEnabledRef.current) {
        const normalizedPath = normalizePath(filePath);
        const repoDir = getParentDir(normalizedPath);
        if (repoDir) {
          try {
            const loadedHunks = await gitDiff(repoDir, normalizedPath);
            // generation 检查：filePath 已切换则丢弃过期结果（view 可能已被 cleanup 销毁）
            if (genRef.current !== gen) return;
            if (loadedHunks.length > 0) {
              updateDiffGutter(view, loadedHunks);
            }
          } catch {
            // 非 git 仓库，diff 不可用，静默
          }
        }
      }
    };

    initEditor();

    return () => {
      // P1-17: 标记组件已卸载，阻止 pending async 操作 DOM
      mountedRef.current = false;
      // CP-029: 打开后核对随卸载/重建取消
      if (recheckTimer !== undefined) clearTimeout(recheckTimer);
      // CP-039: 先取消主题订阅再销毁 view——销毁后 dispatch 会抛错
      unbindTheme?.();
      // 箭头函数调 destroy，防止 this 丢失
      const cleanup = () => {
        viewRef.current?.destroy();
        viewRef.current = null;
      };
      cleanup();
    };
    // 重建由 container 变化驱动（docViewer 形态切换卸载/重挂同 hook 实例时 ref
    // 初值即最新快照）；initialDoc 不入 deps——面板层读盘完成回填会导致值变化
    // 触发无谓重建（闪烁+光标重置），快照只在 effect 执行瞬间消费（initialDocRef）
  }, [container, filePath]);

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

  // CP-029(S03): 外部内容变更落盘 → 缓冲刷新（fs-event 触发与打开后核对共用）。
  // 仅依赖 refs 与模块函数——经 ref 转发保最新实现（onDocContentRef 同模式，
  // 规避 effect 闭包捕获过期函数）。
  // 两调用方语义差异（定责取证 2026-09-07 后定稿）：
  // - mode "event"（fs-event = 权威外部变更信号）：脏文件先弹确认（取消 → 零读盘，
  //   历史语义不变）；干净文件读盘后与缓冲内容判等短路（touch/元数据 Modify 不做
  //   同内容替换——同内容 dispatch 会产生 docChanged 误标 dirty）。
  // - mode "recheck"（打开后核对 = 补偿探测，见 OPEN_RECHECK_DELAY_MS 注释）：先读
  //   盘与「打开时读到内容(baseline)」判变——用户开始打字（doc≠baseline）而磁盘
  //   未变的常态不误报；磁盘真变了才可能弹确认。
  const applyExternalChangeRef = useRef<
    (
      path: string,
      opts: { mode: "event" | "recheck"; toastOnError: boolean; baseline?: string },
    ) => Promise<void>
  >(async () => {});
  applyExternalChangeRef.current = async (path, opts) => {
    const dirty = dirtyRef.current;
    // 事件路径脏分支：确认弹窗先行（取消 → 不读盘，E6/E8 语义锁死）
    if (opts.mode === "event" && dirty) {
      // FE-01: 有未保存修改 → 弹窗选择（确认=重载/取消=保留）
      const choice = await confirmDialog({
        title: "外部修改",
        message: `文件 "${path}" 已被外部修改。当前编辑器有未保存的修改。确认将重载并丢弃本地修改，取消将保留当前内容。`,
        confirmText: "重载",
      });
      if (!choice) return;
    }
    // FE-08 重载面补齐：外部修改重载同走 stat 预检——>10MB 不全量读盘灌 CM，
    // 置 largeFile 信号引导只读浏览（与打开路径同语义）；stat 失败按重载失败处理
    try {
      const meta = await fs.statFile(path);
      if (meta.sizeBytes > MAX_FILE_SIZE_BYTES) {
        filePathRef.current = undefined; // 防误保存覆盖原文件（同打开路径 :364）
        setLargeFile({ filePath: path });
        return;
      }
    } catch (err) {
      const msg = getErrorMessage(err);
      console.warn("[slTerminal] 外部修改重载失败:", msg);
      if (opts.toastOnError) toast.show("error", `外部修改重载失败: ${msg}`);
      return;
    }
    let content: string;
    try {
      content = await fs.readFile(path);
    } catch (err) {
      // P2-16/FE-10: 读盘失败 console.warn；事件路径补 toast（用户可感知），
      // 打开后核对属静默补偿探测——只 warn 不打扰
      const msg = getErrorMessage(err);
      console.warn("[slTerminal] 外部修改重载失败:", msg);
      if (opts.toastOnError) toast.show("error", `外部修改重载失败: ${msg}`);
      return;
    }
    // 读后复核（TOCTOU 防线，同打开路径 :389-393）：stat 与读盘间文件长大超限 → 仍引导只读浏览
    if (content.length > MAX_FILE_SIZE_BYTES) {
      filePathRef.current = undefined;
      setLargeFile({ filePath: path });
      return;
    }
    // await 后重取（期间可能已卸载/重建/切文件）
    const view = viewRef.current;
    if (!view) return;
    // 与打开时基线比较（recheck）：磁盘未变 = 无外部修改（用户打字常态不误报）
    if (opts.mode === "recheck" && content === opts.baseline) return;
    // 事件路径 + 干净文件：同内容 Modify（touch）短路——磁盘内容与缓冲一致无需重载
    if (opts.mode === "event" && !dirty && content === safeDocText(view)) return;
    // recheck 路径脏分支此时才弹确认（先判变后弹窗，避免打字常态弹窗打扰）
    if (opts.mode === "recheck" && dirty) {
      const choice = await confirmDialog({
        title: "外部修改",
        message: `文件 "${path}" 已被外部修改。当前编辑器有未保存的修改。确认将重载并丢弃本地修改，取消将保留当前内容。`,
        confirmText: "重载",
      });
      if (!choice) return;
    }
    // 重载（view 可能在 confirm 等待期间被销毁 → 重取防御）
    const live = viewRef.current;
    if (!live) return;
    // 幂等：决策期间事件路径已把缓冲刷新到与磁盘一致 → 跳过（同内容 dispatch 会
    // 产生 docChanged 误标 dirty + 无谓 edit 回传）
    if (content === safeDocText(live)) return;
    live.dispatch({
      changes: {
        from: 0,
        to: live.state.doc.length,
        insert: content,
      },
    });
    dirtyRef.current = false;
    // docRef 真值源同步（reload 源）
    const cb = onDocContentRef.current;
    if (cb) cb(content, "reload");
  };

  // D3: 监听外部文件改动
  useEffect(() => {
    // FE-01: 回调改 async——脏文件分支需 await confirmDialog（确认=重载/取消=保留）
    const unlisten = onFsEvent(async (event) => {
      const currentPath = filePathRef.current;
      if (currentPath) {
        // 按文件路径去重：仅跳过该编辑器实例自己保存触发的文件事件
        const normalizedCurrent = currentPath.replace(/\\/g, "/");
        if (justSavedRef.current.has(normalizedCurrent)) {
          justSavedRef.current.delete(normalizedCurrent);
          return;
        }
      }

      if (!currentPath) return;

      // 规范化路径比较
      const normalizedCurrent = currentPath.replace(/\\/g, "/");
      const affected = event.paths.some(
        (p) => p.replace(/\\/g, "/") === normalizedCurrent,
      );
      if (!affected) return;

      // 仅处理 Modify 事件（Create/Remove/Rescan 不触发 reload——事件源语义；
      // CP-029 打开后核对补偿 Create 吞并 Modify 的窗口）
      if (event.kind !== "Modify") return;

      await applyExternalChangeRef.current(currentPath, {
        mode: "event",
        toastOnError: true,
      });
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

  // Ctrl+滚轮调节字体大小（共享 hook，含 Mac Cmd+Wheel）
  useFontSizeWheel(container, FONT_SIZE_MIN, FONT_SIZE_MAX, fontSizeRef, (size) => {
    onFontSizeChange?.(size);
  });

  return {
    /**
     * 大文件信号（CP-022）: 打开的 >10MB 文件只读浏览信息——宿主（EditorPanel）
     * 检测非 null 即切换 LargeFileViewer 形态;null = 正常 CM 编辑。切换文件后
     * 经 [filePath] effect 自动清空（见状态定义处注释）。
     */
    largeFile,

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
