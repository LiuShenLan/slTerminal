// HtmlPanel — HTML 文件浏览器式预览面板（docViewer 预览家族）
//
// 原自包含实现（注入脚本组装 / postMessage 总线 / 缩放 HUD / nonce）已迁入
// docViewer 共享层（PreviewFrame + buildInjectedScript + previewMessages +
// zoomRuntime），本面板保持三态壳职责：
//   - loading/error/loaded 状态机与文件读取（fs.readFile，cancelled 竞态）
//   - loaded → PreviewFrame 渲染：html 场景传 fragmentNav 段（# 片段链接拦截）
//   - 形态切换条（render ↔ edit）在 S4 接入（viewMode params）
//
// 安全红线（SEC-03/04、sandbox 无 allow-same-origin、targetOrigin "*"、
// CSP 依赖、宿主 script 静态化缺陷）详见 docViewer/CLAUDE.md 与 panels/CLAUDE.md。

import React, { useEffect, useState } from "react";
import { fs } from "../../ipc";
import {
  PANEL_BG,
  ERROR_FG,
  HTML_PANEL_LOADING_FG,
  HTML_PANEL_IFRAME_BG,
} from "../../theme";
import { PreviewFrame } from "../docViewer/PreviewFrame";

/** HtmlPanel 接收的面板参数 */
interface HtmlPanelProps {
  params: {
    panelId: string;
    filePath?: string;
  };
}

/** 加载状态机 */
type LoadState =
  | { kind: "loading" }
  | { kind: "loaded"; html: string }
  | { kind: "error"; message: string };

/** 居中容器样式 */
const centerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

const HtmlPanel: React.FC<HtmlPanelProps> = ({ params }) => {
  const [state, setState] = useState<LoadState>({ kind: "loading" });

  useEffect(() => {
    if (!params.filePath) {
      setState({ kind: "error", message: "未指定文件路径" });
      return;
    }

    let cancelled = false;

    (async () => {
      try {
        const content = await fs.readFile(params.filePath!);
        if (!cancelled) setState({ kind: "loaded", html: content });
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

  return (
    <PreviewFrame
      html={state.html}
      title={`HTML 预览: ${params.filePath}`}
      segments={[{ kind: "fragmentNav" }]}
      iframeBg={HTML_PANEL_IFRAME_BG}
      dataE2ePrefix="html"
    />
  );
};

export default HtmlPanel;
