// MarkdownPanel — .md 文档面板（docViewer 预览家族，三形态）
//
// 形态（viewMode，随 params 持久化，默认 edit）：
//   - edit：CM6 编辑全宽（lang-markdown）
//   - split：左编辑右预览（allotment 拖拽分栏，比例随 params 持久化）
//   - preview：渲染只读全宽
// 右上角悬浮切换条常驻（面板根定位——PreviewFrame 位于 allotment pane 内，
// overlay 槽坐标系不一致，恒由本面板根渲染）。
//
// 文档真值源 = 面板 docRef（草稿优先磁盘）：
//   - 磁盘读入 → doc；CM 击键经 onDocContent 即时写回；
//   - CM 仅 edit/split 挂载（allotment CM pane）；preview-only 卸载——快照在 doc，
//     回 edit/split 经 initialDoc 回填免二次读盘（光标/undo 重置为登记已知行为）；
//     edit↔split CM pane 不卸载（React 位置保活），undo/光标保留；
//   - 预览渲染源 = doc（草稿优先）：300ms 防抖（仅 split/preview 启动），切形态
//     时 stale 立即渲染；异步管线 gen 丢弃过期产物；外部修改 reload（CM 挂载时）
//     同步 doc 并触发刷新。
//
// 预览 iframe：PreviewFrame + linkRouter/scrollReport 段；keepZoom/keepScrollRatio
// 重建恢复；Ctrl+W（global.closeTab）与 Ctrl+滚轮缩放经注入桥自动生效。
// 链接点击：slterm_nav 上行 → classifyLink → external = 系统浏览器（opener）/
// local = 应用内打开链路（openFileInActivePage，失败静默）。

import React, { useEffect, useRef, useState, useCallback } from "react";
import type { DockviewPanelApi, DockviewApi } from "dockview-react";
import { Allotment } from "allotment";
import "allotment/dist/style.css";
import { fs, shell } from "../../ipc";
import {
  PANEL_BG,
  ERROR_FG,
  HTML_PANEL_LOADING_FG,
  HTML_PANEL_IFRAME_BG,
} from "../../theme";
import { PreviewFrame, overlayBarStyle } from "../docViewer/PreviewFrame";
import { ModeSwitcher } from "../docViewer/ModeSwitcher";
import { useCodeMirror } from "../editor/useCodeMirror";
import { renderMarkdownDocument } from "./mdRenderAsync";
import { classifyLink } from "./linkPolicy";
import { persistPanelParams } from "../../workspace/persistPanelParams";
import { openFileInActivePage } from "../../workspace/openFile";

/** 面板参数（viewMode/splitRatio 随布局 params 持久化） */
interface MarkdownPanelProps {
  api?: DockviewPanelApi;
  containerApi?: DockviewApi;
  params: {
    panelId: string;
    filePath?: string;
    viewMode?: "edit" | "split" | "preview";
    splitRatio?: [number, number];
  };
}

type ViewMode = "edit" | "split" | "preview";

/** 加载状态机 */
type LoadState =
  | { kind: "loading" }
  | { kind: "ready" }
  | { kind: "error"; message: string };

/** 预览防抖（ms）：停止输入后触发重渲染 */
const PREVIEW_DEBOUNCE_MS = 300;

const MODES: Array<{ id: ViewMode; label: string; title?: string }> = [
  { id: "edit", label: "编辑" },
  { id: "split", label: "编辑/预览", title: "左右分栏编辑与预览" },
  { id: "preview", label: "预览" },
];

/** 形态白名单收窄（布局 JSON 任意旧值 → 默认 edit） */
function normalizeMode(v: unknown): ViewMode {
  return v === "split" || v === "preview" ? v : "edit";
}

/** splitRatio 校验收窄（两元素非负有限数；非法回退 [50, 50]） */
function normalizeSplitRatio(v: unknown): [number, number] {
  if (
    Array.isArray(v) &&
    v.length === 2 &&
    v.every((x) => typeof x === "number" && Number.isFinite(x) && x > 0)
  ) {
    return [v[0] as number, v[1] as number];
  }
  return [50, 50];
}

/** 文件所在目录（相对资源 join 基；根目录文件 → 盘符 + "/"） */
function dirnameOf(filePath: string): string | null {
  const norm = filePath.replace(/\\/g, "/");
  const i = norm.lastIndexOf("/");
  if (i < 0) return null;
  const d = norm.slice(0, i);
  return /^[A-Za-z]:$/.test(d) ? `${d}/` : d;
}

/** CM 全宽区 / allotment pane 容器（overflow clip——CM 滚动委托红线） */
const cmAreaStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  overflow: "clip",
};

/** 预览 pane 容器（PreviewFrame 自撑满） */
const previewAreaStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
};

/** 渲染占位（首次渲染/渲染中——旧内容保留时以旧内容展示，不闪占位） */
const renderPlaceholderStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

const rootStyle: React.CSSProperties = {
  position: "relative",
  width: "100%",
  height: "100%",
  background: PANEL_BG,
};

const centerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

const MarkdownPanel: React.FC<MarkdownPanelProps> = ({
  api,
  containerApi,
  params,
}) => {
  // ── 加载状态机：磁盘就绪（ready）后 doc 为真值源 ──
  const [loadState, setLoadState] = useState<LoadState>({ kind: "loading" });
  const [doc, setDoc] = useState("");
  const docRef = useRef("");
  docRef.current = doc;

  // 形态：初值取 params（布局恢复）；本地切换经 persist 回写
  const [mode, setMode] = useState<ViewMode>(() => normalizeMode(params.viewMode));
  const modeRef = useRef(mode);
  modeRef.current = mode;
  useEffect(() => {
    setMode(normalizeMode(params.viewMode));
  }, [params.viewMode]);

  // 分栏比例（persist 于 allotment onDragEnd）
  const [splitRatio, setSplitRatio] = useState<[number, number]>(() =>
    normalizeSplitRatio(params.splitRatio),
  );

  // 预览渲染产物（null = 尚未渲染——split/preview 首次展示占位）
  const [previewHtml, setPreviewHtml] = useState<string | null>(null);
  // 文档脏标记：doc 变化后尚未渲染（切形态时若 stale 立即渲染一次）
  const staleRef = useRef(false);
  // 渲染 gen：异步管线产物过期丢弃（useCodeMirror genRef 同模式）
  const renderGenRef = useRef(0);
  // 防抖计时器
  const debounceTimerRef = useRef<number | null>(null);

  // 相对资源解析基（读盘时由 filePath 推出）
  const docDirRef = useRef<string | null>(null);

  // CM 容器（edit/split 的 allotment CM pane 内）
  const cmContainerRef = useRef<HTMLDivElement | null>(null);
  const [, bumpFrame] = useState(0);

  // ── 磁盘读取（cancelled 竞态；docDir 同步推出）──
  useEffect(() => {
    if (!params.filePath) {
      setLoadState({ kind: "error", message: "未指定文件路径" });
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const content = await fs.readFile(params.filePath!);
        if (!cancelled) {
          docDirRef.current = dirnameOf(params.filePath!);
          setDoc(content);
          staleRef.current = true;
          setLoadState({ kind: "ready" });
        }
      } catch (err) {
        if (!cancelled) {
          setLoadState({
            kind: "error",
            message: err instanceof Error ? err.message : String(err),
          });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [params.filePath]);

  // ── 异步渲染（产物流水：快照 → 资源/mermaid → 完整文档）──
  const runRender = useCallback(async () => {
    const gen = ++renderGenRef.current;
    const snapshot = docRef.current;
    try {
      const html = await renderMarkdownDocument({
        markdown: snapshot,
        docDir: docDirRef.current,
        isCancelled: () => gen !== renderGenRef.current,
      });
      if (gen !== renderGenRef.current) return; // 过期产物丢弃
      if (html === "") return; // 渲染中被取消
      staleRef.current = false;
      setPreviewHtml(html);
    } catch (err) {
      // 渲染管线内部失败不抛（资源/图表失败均已降级）；此处兜底记录
      console.warn("[slTerminal] markdown 预览渲染失败:", err);
    }
  }, []);

  /** 防抖调度（仅 split/preview 启动；edit 只标 stale，切形态时立即渲染） */
  const scheduleRender = useCallback(() => {
    staleRef.current = true;
    if (modeRef.current === "edit") return;
    if (debounceTimerRef.current !== null) return;
    debounceTimerRef.current = window.setTimeout(() => {
      debounceTimerRef.current = null;
      void runRender();
    }, PREVIEW_DEBOUNCE_MS);
  }, [runRender]);

  // 形态切换：split/preview 若 stale（含首入）立即渲染一次
  //（runRender 引用稳定——useCallback 空依赖；仅形态切换触发，防抖击键路径
  // 由 scheduleRender 负责）
  useEffect(() => {
    if (mode === "edit") return;
    if (staleRef.current || previewHtml === null) {
      void runRender();
    }
  }, [mode, runRender, previewHtml]);

  // 卸载清理防抖计时器
  useEffect(() => {
    return () => {
      if (debounceTimerRef.current !== null) {
        window.clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
    };
  }, []);

  // 容器 ref 桥接（HtmlPanel/DiffPanel 先例）：ready + 非 preview 时 CM pane
  // 首挂 render 阶段 ref 未赋值——commit 后 bump 一次，hook 收到非 null 容器
  useEffect(() => {
    if (loadState.kind === "ready" && mode !== "preview") {
      bumpFrame((f) => f + 1);
    }
  }, [loadState.kind, mode]);

  // ── 编辑桥（edit/split 挂载；preview 卸载——快照在 doc state，回填免读盘）──
  useCodeMirror({
    container: mode !== "preview" ? cmContainerRef.current : null,
    filePath: params.filePath,
    panelId: params.panelId,
    initialDoc: docRef.current,
    onDocContent: (text) => {
      setDoc(text);
      scheduleRender();
    },
    gitGutterEnabled: false,
  });

  // ── 链接点击（slterm_nav 上行 → 分类分派）──
  const handleNav = useCallback((href: string) => {
    const kind = classifyLink(href, docDirRef.current);
    if (kind.kind === "external") {
      void shell.openUrl(kind.url);
    } else if (kind.kind === "local") {
      // 应用内共享打开链路（复用文件浏览器双击分发；失败静默忽略）
      openFileInActivePage(kind.absPath);
    }
    // fragment/ignored：md 渲染标题无锚点 id——忽略点击
  }, []);

  /** 形态切换：更新本地态 + persist（api 缺省 = 单测环境，跳过落盘） */
  const handleModeChange = (next: ViewMode) => {
    setMode(next);
    if (api && containerApi) {
      persistPanelParams(containerApi, api, { viewMode: next });
    }
  };

  /** 分栏拖拽结束：比例持久化（拖拽过程不落盘） */
  const handleDragEnd = useCallback(
    (sizes: number[]) => {
      if (sizes.length !== 2 || sizes.some((s) => !Number.isFinite(s) || s <= 0)) {
        return;
      }
      setSplitRatio([sizes[0], sizes[1]]);
      if (api && containerApi) {
        persistPanelParams(containerApi, api, { splitRatio: sizes });
      }
    },
    [api, containerApi],
  );

  const switcher = (
    <ModeSwitcher
      modes={MODES}
      value={mode}
      onChange={handleModeChange}
      dataE2ePrefix="markdown"
    />
  );

  // ── 非就绪态：loading/error（无切换条——形态交互需文档就绪）──
  if (loadState.kind === "loading") {
    return (
      <div style={centerStyle}>
        <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }}>加载中...</span>
      </div>
    );
  }
  if (loadState.kind === "error") {
    return (
      <div style={centerStyle}>
        <span style={{ color: ERROR_FG, fontSize: 13 }}>
          加载失败: {loadState.message}
        </span>
      </div>
    );
  }

  return (
    <div style={rootStyle}>
      <Allotment
        proportionalLayout
        defaultSizes={splitRatio}
        onDragEnd={handleDragEnd}
        minSize={0}
      >
        {mode !== "preview" && (
          <Allotment.Pane minSize={160}>
            <div ref={cmContainerRef} style={cmAreaStyle} />
          </Allotment.Pane>
        )}
        {mode !== "edit" && (
          <Allotment.Pane minSize={120}>
            <div style={previewAreaStyle}>
              {previewHtml !== null ? (
                <PreviewFrame
                  html={previewHtml}
                  title={`Markdown 预览: ${params.filePath}`}
                  segments={[{ kind: "linkRouter" }, { kind: "scrollReport" }]}
                  keepZoom
                  keepScrollRatio
                  iframeBg={HTML_PANEL_IFRAME_BG}
                  dataE2ePrefix="markdown"
                  onNav={handleNav}
                />
              ) : (
                <div style={renderPlaceholderStyle}>
                  <span style={{ color: HTML_PANEL_LOADING_FG, fontSize: 13 }}>
                    渲染中...
                  </span>
                </div>
              )}
            </div>
          </Allotment.Pane>
        )}
      </Allotment>
      <div style={overlayBarStyle}>{switcher}</div>
    </div>
  );
};

export default MarkdownPanel;
