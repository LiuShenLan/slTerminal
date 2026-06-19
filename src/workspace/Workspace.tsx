// Workspace — 统一分屏页签区
//
// 暴露 window.__dockviewApi 供 E2E 测试使用

import React, { useCallback } from "react";
import {
  DockviewReact,
  type DockviewApi,
  type IDockviewPanelProps,
} from "dockview-react";
import { panelRegistry } from "./panelRegistry";
import { saveLayout } from "./layoutSerde";

// E2E 测试用全局 API 类型声明
declare global {
  interface Window {
    __dockviewApi?: DockviewApi;
  }
}

const WATERMARK_TEXT = "打开终端或编辑器开始工作";

const Watermark: React.FC = () => (
  <div
    style={{
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      height: "100%",
      color: "#6C6C6C",
      fontSize: 14,
      userSelect: "none",
    }}
  >
    {WATERMARK_TEXT}
  </div>
);

const Workspace: React.FC = () => {
  const onReady = useCallback(
    (event: { api: DockviewApi }) => {
      const { api } = event;

      // 暴露给 E2E 测试
      window.__dockviewApi = api;

      api.onDidLayoutChange(() => {
        const layout = saveLayout(api);
        console.debug("布局已变更", layout);
      });
    },
    [],
  );

  return (
    <DockviewReact
      className="dockview-theme-dark"
      components={panelRegistry}
      onReady={onReady}
      watermarkComponent={Watermark}
      defaultTabComponent={DefaultTab}
    />
  );
};

const DefaultTab: React.FC<IDockviewPanelProps> = (props) => {
  const { api } = props;
  const title = api.title || api.component || "";

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation();
    api.close();
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        height: "100%",
        padding: "0 8px",
        gap: 6,
        userSelect: "none",
      }}
    >
      <span style={{ fontSize: 13 }}>{title}</span>
      <button
        onClick={handleClose}
        style={{
          background: "none",
          border: "none",
          color: "#808080",
          cursor: "pointer",
          padding: "0 2px",
          fontSize: 14,
          lineHeight: 1,
        }}
        title="关闭"
      >
        ×
      </button>
    </div>
  );
};

export default Workspace;
