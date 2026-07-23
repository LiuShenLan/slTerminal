// CommitView.tsx — commit 侧栏视图主组件
//
// 状态机（优先级自上而下）：
//   no-root → "选择一个项目以查看变更"
//   loading → "加载中…"
//   error   → "当前项目并非 git 项目"
//   ready   → Changes (N) + Unversioned Files (N) 两列表
//
// 标题栏 "COMMIT"（28px 高、大写、letterSpacing 1、fontSize 11）
// 样式照 ExplorerPanel 标题栏。

import React from "react";
import { useCommitStatus } from "./useCommitStatus";
import { CommitFileList } from "./CommitFileList";
import {
  SEPARATOR_BG,
  INPUT_BORDER,
  PANEL_BG,
  HTML_PANEL_LOADING_FG,
} from "../../theme";

/** 标题栏样式（照 ExplorerPanel） */
const headerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  padding: "4px 8px",
  borderBottom: `1px solid ${SEPARATOR_BG}`,
  height: 28,
  fontSize: 11,
  color: INPUT_BORDER,
  textTransform: "uppercase",
  letterSpacing: 1,
  userSelect: "none",
  flexShrink: 0,
};

/** 状态提示居中样式 */
const centerHintStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  flex: 1,
  padding: 16,
  color: INPUT_BORDER,
  fontSize: 12,
  textAlign: "center",
  userSelect: "none",
};

/** 列表区域滚动容器 */
const listContainerStyle: React.CSSProperties = {
  flex: 1,
  overflowY: "auto",
  overflowX: "hidden",
  padding: "2px 0",
};

/** Changes 列表容纳的状态 */
const CHANGES_STATUSES = new Set([
  "added",
  "modified",
  "deleted",
  "renamed",
  "conflict",
]);

export const CommitView: React.FC = () => {
  const { state, rootPath, refresh } = useCommitStatus();

  return (
    <div
      data-e2e="commit-view"
      style={{
        width: "100%",
        height: "100%",
        background: PANEL_BG,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      {/* 标题栏 */}
      <div style={headerStyle}>COMMIT</div>

      {/* 状态机渲染 */}
      {state.kind === "no-root" && (
        <div style={centerHintStyle}>选择一个项目以查看变更</div>
      )}

      {state.kind === "loading" && (
        <div
          style={{
            ...centerHintStyle,
            color: HTML_PANEL_LOADING_FG,
          }}
        >
          加载中…
        </div>
      )}

      {state.kind === "error" && (
        <div style={centerHintStyle}>当前项目并非 git 项目</div>
      )}

      {state.kind === "ready" && rootPath && (
        <div style={listContainerStyle}>
          {/* Changes 列表：added/modified/deleted/renamed/conflict */}
          <CommitFileList
            title="Changes"
            entries={state.entries.filter((e) =>
              CHANGES_STATUSES.has(e.status),
            )}
            rootPath={rootPath}
            e2eId="commit-changes"
            onRefresh={refresh}
          />

          {/* Unversioned Files 列表：untracked */}
          <CommitFileList
            title="Unversioned Files"
            entries={state.entries.filter((e) => e.status === "untracked")}
            rootPath={rootPath}
            e2eId="commit-unversioned"
            onRefresh={refresh}
          />
        </div>
      )}
    </div>
  );
};
