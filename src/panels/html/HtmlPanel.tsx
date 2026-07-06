// HtmlPanel — HTML 文件浏览器式预览面板
//
// 使用 iframe + srcDoc 渲染 HTML 文件内容，达到类浏览器视觉效果。
// sandbox 属性限制脚本能力：allow-scripts 允许 JS 执行，allow-same-origin
// 允许访问同源存储；但不含 allow-top-navigation / allow-popups / allow-modals。
//
// 三态：loading → loaded (iframe) / error
// 通过 cancelled 标志防止组件卸载或快速切换 filePath 时的竞态。

import React, { useEffect, useState } from "react";
import { fs } from "../../ipc";
import { PANEL_BG, ERROR_FG } from "../../theme";

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

/** iframe sandbox 权限：允许脚本执行和同源访问，禁止顶层导航/弹窗/模态框 */
const SANDBOX_FLAGS = "allow-scripts allow-same-origin";

/** 居中容器样式 */
const centerStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: PANEL_BG,
};

/** iframe 全容器样式（白底——HTML 页面默认背景） */
const iframeStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  border: "none",
  background: "#FFFFFF",
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
        <span style={{ color: "#6C6C6C", fontSize: 13 }}>加载中...</span>
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
    <iframe
      sandbox={SANDBOX_FLAGS}
      srcDoc={state.html}
      title={`HTML 预览: ${params.filePath}`}
      style={iframeStyle}
    />
  );
};

export default HtmlPanel;
