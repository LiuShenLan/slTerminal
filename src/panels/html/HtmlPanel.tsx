// HtmlPanel — HTML 文件浏览器式预览面板（docViewer 预览家族，二态）
//
// 形态（viewMode，随 params 持久化，默认 render）：
//   - render：PreviewFrame 渲染（htmlviewer 传统行为——iframe srcDoc + 缩放/
//     键转发/片段拦截，注入与总线在 docViewer 共享层；HUD 与切换条在面板根
//     FloatingArea，2026-09-06 悬浮区收敛）
//   - edit：CodeMirror 6 源码编辑（lang-html；editor context 注册 Ctrl+S，
//     保存链路复用 useCodeMirror.handleSave）
//
// 文档真值源 = 面板级 docRef（草稿优先磁盘）：
//   - 磁盘内容读入 → doc；edit 态击键经 onDocContent 即时写回 doc；
//   - edit ↔ render 切换不丢草稿——edit 卸载前内容已在 doc（快照方案），
//     render 展示 doc（含未保存草稿，IDEA 缓冲语义）；回 edit 经 initialDoc
//     回填免二次读盘（光标/undo 随卸载重置为登记已知行为）；
//   - 外部修改重载（edit 态挂载时 useCodeMirror 内置处理，reload 源同步 doc）。
//   render 态无防抖（无编辑入口，doc 仅在形态切换时变化）。

import React, { useEffect, useRef, useState } from "react";
import type { DockviewPanelApi, DockviewApi } from "dockview-react";
import { fs } from "../../ipc";
import {
  PANEL_BG,
  ERROR_FG,
  HTML_PANEL_LOADING_FG,
  HTML_PANEL_IFRAME_BG,
} from "../../theme";
import { PreviewFrame, type PreviewFrameHandle } from "../docViewer/PreviewFrame";
import { FloatingArea } from "../docViewer/FloatingArea";
import { useZoomHud } from "../docViewer/useZoomHud";
import { ModeSwitcher } from "../docViewer/ModeSwitcher";
import { useCodeMirror } from "../editor/useCodeMirror";
import { useFontSize } from "../../stores";
import { persistPanelParams } from "../../workspace/persistPanelParams";

/** HtmlPanel 接收的面板参数（viewMode 随布局 params 持久化） */
interface HtmlPanelProps {
  /** Dockview 传入的面板 API（形态持久化；单测可不传 → 跳过落盘） */
  api?: DockviewPanelApi;
  /** Dockview 传入的容器 API（布局序列化源） */
  containerApi?: DockviewApi;
  params: {
    panelId: string;
    filePath?: string;
    viewMode?: "edit" | "render";
  };
}

/** 加载状态机 */
type LoadState =
  | { kind: "loading" }
  | { kind: "ready" }
  | { kind: "error"; message: string };

/** 形态白名单收窄（布局 JSON 可能含任意旧值 → 非法回退默认 render） */
function normalizeViewMode(v: unknown): "edit" | "render" {
  return v === "edit" ? "edit" : "render";
}

/** 编辑态根容器（overflow clip——CM6 滚动委托 .cm-scroller 红线，editor/CLAUDE.md） */
const editAreaStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  overflow: "clip",
};

/** 面板根：悬浮切换条定位锚 + 形态内容区 */
const rootStyle: React.CSSProperties = {
  position: "relative",
  width: "100%",
  height: "100%",
  background: PANEL_BG,
};

/** 居中容器样式（loading/error） */
const centerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

const MODES = [
  { id: "render" as const, label: "渲染", title: "渲染 HTML 页面" },
  { id: "edit" as const, label: "编辑", title: "编辑 HTML 源码" },
];

const HtmlPanel: React.FC<HtmlPanelProps> = ({ api, containerApi, params }) => {
  // ── 加载状态机：磁盘内容就绪（ready）后 doc 为真值源 ──
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  /** 文档真值源（草稿优先磁盘）：磁盘内容读入 / edit 击键 / reload 均写回此处 */
  const [doc, setDoc] = useState("");
  const docRef = useRef("");
  docRef.current = doc;

  // ── 形态：初值取 params（布局恢复），本地切换后经 persist 回写 params ──
  const [mode, setMode] = useState<"edit" | "render">(() =>
    normalizeViewMode(params.viewMode),
  );
  // 布局恢复/外部 params 变更（同面板重挂）同步形态
  useEffect(() => {
    setMode(normalizeViewMode(params.viewMode));
  }, [params.viewMode]);

  // ── 预览框命令句柄（悬浮区重置缩放下行）+ 缩放 HUD 状态机（悬浮区显示）──
  const frameRef = useRef<PreviewFrameHandle | null>(null);
  const zoomHud = useZoomHud();
  /** 悬浮区重置链：下行复位 + 立即隐藏（iframe 归 1 回声由 report 等值忽略） */
  const handleHudReset = () => {
    frameRef.current?.resetZoom();
    zoomHud.hide();
  };

  // ── 编辑态 CM 容器 ──
  const editContainerRef = useRef<HTMLDivElement | null>(null);
  // 容器 ref 桥接（DiffPanel 先例，panels/CLAUDE.md）：edit 容器首次挂载的
  // render 阶段 ref 尚为 null（commit 后才赋值）——useCodeMirror 会以 null
  // 建不出 view。effect 在容器就绪后触发一次额外渲染，使 hook 下次调用收到
  // 非 null DOM 元素。
  const [, bumpFrame] = useState(0);
  useEffect(() => {
    if (state.kind === "ready" && mode === "edit") {
      bumpFrame((f) => f + 1);
    }
  }, [state.kind, mode]);

  // 磁盘读取（cancelled 竞态防卸载/换路径覆盖）
  useEffect(() => {
    if (!params.filePath) {
      setState({ kind: "error", message: "未指定文件路径" });
      return;
    }

    let cancelled = false;

    (async () => {
      try {
        const content = await fs.readFile(params.filePath!);
        if (!cancelled) {
          setDoc(content);
          setState({ kind: "ready" });
        }
      } catch (err) {
        if (!cancelled) {
          setState({
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

  // ── 编辑桥（edit 形态挂载；container 为 null 时 hook 不建 view）──
  // 快照回填：docRef 非空（磁盘已读/草稿）→ initialDoc 免二次读盘；直接恢复
  // edit 形态的布局恢复场景 doc 为空 → useCodeMirror 自行读盘（语言扩展依赖
  // filePath 的 .html 判定，必须保留 filePath）
  // 字号 = 共享 editorFontSize store（Ctrl+滚轮缩放接线——EditorPanel 同款范本）
  const editorFontSize = useFontSize((s) => s.editorFontSize);
  const setEditorFontSize = useFontSize((s) => s.setEditorFontSize);
  useCodeMirror({
    container: mode === "edit" ? editContainerRef.current : null,
    filePath: params.filePath,
    panelId: params.panelId,
    initialDoc: docRef.current,
    onDocContent: (text) => setDoc(text),
    // 预览面板免 git gutter / git 读取（S3 useCodeMirror 扩展）
    gitGutterEnabled: false,
    fontSize: editorFontSize,
    onFontSizeChange: setEditorFontSize,
  });

  /** 形态切换：更新本地态 + params 持久化（api 缺省 = 单测环境，跳过落盘） */
  const handleModeChange = (next: "edit" | "render") => {
    setMode(next);
    if (api && containerApi) {
      persistPanelParams(containerApi, api, { viewMode: next });
    }
  };

  /** 形态切换条（两形态共用；恒经面板根悬浮区 FloatingArea 承载） */
  const switcher = (
    <ModeSwitcher
      modes={MODES}
      value={mode}
      onChange={handleModeChange}
      dataE2ePrefix="html"
    />
  );

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
          加载失败: {state.message}
        </span>
      </div>
    );
  }

  if (mode === "render") {
    return (
      <div style={rootStyle}>
        <PreviewFrame
          ref={frameRef}
          html={doc}
          title={`HTML 预览: ${params.filePath}`}
          segments={[{ kind: "fragmentNav" }]}
          iframeBg={HTML_PANEL_IFRAME_BG}
          onZoomChange={zoomHud.report}
          onZoomReset={zoomHud.hide}
        />
        {/* 悬浮区：切换条上 / 缩放 HUD 下（与 edit 形态同结构，坐标协调单点） */}
        <FloatingArea
          switcher={switcher}
          hud={{
            zoom: zoomHud.hud.zoom,
            visible: zoomHud.hud.visible,
            onReset: handleHudReset,
          }}
          dataE2ePrefix="html"
        />
      </div>
    );
  }

  // edit 形态：CM 全宽 + 悬浮区仅切换条（无 PreviewFrame 无缩放源，hud=null）
  return (
    <div style={rootStyle}>
      <div ref={editContainerRef} style={editAreaStyle} />
      <FloatingArea switcher={switcher} dataE2ePrefix="html" />
    </div>
  );
};

export default HtmlPanel;
