// Workspace — 统一分屏页签区
//
// 右键菜单可新建终端/编辑器，addPanel 传 renderer: 'always' 保持 PTY 存活。
// 暴露 window.__dockviewApi 供 E2E 测试使用。

import React, { useCallback } from "react";
import {
  DockviewReact,
  type DockviewApi,
  type IDockviewPanelProps,
  type GetTabContextMenuItemsParams,
  type ReactContextMenuItemConfig,
  type BuiltInContextMenuItem,
  type IDockviewHeaderActionsProps,
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

/** 顶栏右侧 "+" 按钮 —— 点击新建终端 */
const RightHeaderActions: React.FC<IDockviewHeaderActionsProps> = ({
  containerApi,
}) => {
  const handleNewTerminal = () => {
    const id = `terminal-${Date.now()}`;
    containerApi.addPanel({
      id,
      component: "terminal",
      params: { panelId: id },
      renderer: "always",
    });
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        height: "100%",
        paddingRight: 4,
      }}
    >
      <button
        onClick={handleNewTerminal}
        style={{
          background: "none",
          border: "1px solid #444",
          color: "#ccc",
          cursor: "pointer",
          fontSize: 16,
          width: 24,
          height: 24,
          borderRadius: 4,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          lineHeight: 1,
        }}
        title="新建终端"
      >
        +
      </button>
    </div>
  );
};

const Workspace: React.FC = () => {
  const onReady = useCallback((event: { api: DockviewApi }) => {
    const { api } = event;

    // 暴露给 E2E 测试
    window.__dockviewApi = api;

    api.onDidLayoutChange(() => {
      const layout = saveLayout(api);
      console.debug("布局已变更", layout);
    });
  }, []);

  /** 页签右键菜单 */
  const getTabContextMenuItems = useCallback(
    (params: GetTabContextMenuItemsParams): (BuiltInContextMenuItem | ReactContextMenuItemConfig)[] => {
      const { api } = params;

      const newTerminalId = `terminal-${Date.now()}`;
      const newEditorId = `editor-${Date.now()}`;

      return [
        {
          label: "新建终端",
          action: () => {
            api.addPanel({
              id: newTerminalId,
              component: "terminal",
              params: { panelId: newTerminalId },
              renderer: "always",
            });
          },
        },
        {
          label: "新建编辑器",
          action: () => {
            api.addPanel({
              id: newEditorId,
              component: "editor",
              params: { panelId: newEditorId },
            });
          },
        },
        "separator",
        "close",
        "closeOthers",
        "closeAll",
      ];
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
      rightHeaderActionsComponent={RightHeaderActions}
      getTabContextMenuItems={getTabContextMenuItems}
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
