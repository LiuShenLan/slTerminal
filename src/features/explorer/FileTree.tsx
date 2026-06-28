// FileTree.tsx — 递归文件树组件
//
// 职责：
// - 递归渲染文件/文件夹树
// - 单击文件夹展开/折叠，双击文件打开编辑器
// - 右键菜单 CRUD
// - git 状态色应用于文件名

import React, { useState, useCallback } from "react";
import { FileIcon } from "./FileIcon";
import type { TreeNode } from "./useFileTree";
import {
  EXPLORER_COLORS,
  GIT_FILE_COLORS,
  SIDEBAR_BG,
  SIDEBAR_FG,
  ACTIVE_SELECTION_BG,
  INPUT_BG,
  FOCUS_BORDER,
  CONTEXT_MENU_BORDER,
} from "../../theme";
import { ask } from "../../ipc/dialog";

// ---- 右键菜单 ----

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  items: ContextMenuItem[];
}

interface ContextMenuItem {
  label: string;
  action: () => void;
}

const ContextMenu: React.FC<{
  state: ContextMenuState;
  onClose: () => void;
}> = ({ state, onClose }) => {
  const menuRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    if (!state.visible) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [state.visible, onClose]);

  if (!state.visible) return null;

  return (
    <div
      ref={menuRef}
      style={{
        position: "fixed",
        left: state.x,
        top: state.y,
        background: SIDEBAR_BG,
        border: `1px solid ${CONTEXT_MENU_BORDER}`,
        borderRadius: 4,
        padding: "4px 0",
        minWidth: 160,
        zIndex: 1000,
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
      }}
    >
      {state.items.map((item, i) => (
        <div
          key={i}
          onClick={() => {
            item.action();
            onClose();
          }}
          style={{
            padding: "4px 12px",
            cursor: "pointer",
            color: SIDEBAR_FG,
            fontSize: 13,
            userSelect: "none",
          }}
          onMouseEnter={(e) => {
            (e.target as HTMLDivElement).style.background = ACTIVE_SELECTION_BG;
          }}
          onMouseLeave={(e) => {
            (e.target as HTMLDivElement).style.background = "transparent";
          }}
        >
          {item.label}
        </div>
      ))}
    </div>
  );
};

// ---- Props ----

interface FileTreeProps {
  nodes: TreeNode[];
  depth: number;
  gitStatusMap: Map<string, string>;
  onToggleExpand: (path: string) => void;
  onOpenFile: (path: string) => void;
  onOpenInTerminal: (path: string) => void;
  onRename: (path: string, newName: string) => void;
  onDelete: (path: string) => void;
  onNewFile: (parentPath: string) => void;
  onNewFolder: (parentPath: string) => void;
}

// ---- 单行节点 ----

const TreeNodeRow: React.FC<{
  node: TreeNode;
  depth: number;
  gitStatusMap: Map<string, string>;
  onToggleExpand: (path: string) => void;
  onOpenFile: (path: string) => void;
  onContextMenu: (e: React.MouseEvent) => void;
}> = ({ node, depth, gitStatusMap, onToggleExpand, onOpenFile, onContextMenu }) => {
  const { entry, expanded, loading } = node;
  // 渲染时实时查表，避免节点创建时写入 → 闭包陈旧/时序断裂问题
  const gitStatus = gitStatusMap.get(entry.path);
  const indent = depth * 16;

  return (
    <div
      onClick={() => {
        if (entry.isDir) {
          onToggleExpand(entry.path);
        }
      }}
      onDoubleClick={() => {
        if (!entry.isDir) {
          onOpenFile(entry.path);
        }
      }}
      onContextMenu={onContextMenu}
      style={{
        display: "flex",
        alignItems: "center",
        padding: "1px 8px",
        paddingLeft: 8 + indent,
        cursor: "pointer",
        userSelect: "none",
        fontSize: 13,
        color: EXPLORER_COLORS.fg,
        height: 24,
        whiteSpace: "nowrap",
      }}
      onMouseEnter={(e) => {
        (e.target as HTMLDivElement).style.background =
          EXPLORER_COLORS.hover;
      }}
      onMouseLeave={(e) => {
        (e.target as HTMLDivElement).style.background = "transparent";
      }}
    >
      {/* 展开/折叠箭头 */}
      {entry.isDir ? (
        <span
          style={{
            width: 12,
            fontSize: 8,
            flexShrink: 0,
            color: expanded
              ? EXPLORER_COLORS.arrowOpen
              : EXPLORER_COLORS.arrowClosed,
          }}
        >
          {loading ? "⏳" : expanded ? "▼" : "▶"}
        </span>
      ) : (
        <span style={{ width: 12, flexShrink: 0 }} />
      )}

      {/* 图标 */}
      <span style={{ marginRight: 4, flexShrink: 0 }}>
        <FileIcon
          name={entry.name}
          isDir={entry.isDir}
          gitStatus={gitStatus}
        />
      </span>

      {/* 文件名 — git 状态色引用 GIT_FILE_COLORS token（配色单点） */}
      <span
        style={{
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          color: gitStatus
            ? (GIT_FILE_COLORS[gitStatus as keyof typeof GIT_FILE_COLORS] ?? EXPLORER_COLORS.fg)
            : EXPLORER_COLORS.fg,
        }}
      >
        {entry.name}
      </span>
    </div>
  );
};

// ---- 主组件 ----

export const FileTree: React.FC<FileTreeProps> = ({
  nodes,
  depth,
  gitStatusMap,
  onToggleExpand,
  onOpenFile,
  onOpenInTerminal,
  onRename,
  onDelete,
  onNewFile,
  onNewFolder,
}) => {
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  const [renamingPath, setRenamingPath] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [newFileName, setNewFileName] = useState<string | null>(null); // parent path
  const [newFolderName, setNewFolderName] = useState<string | null>(null);

  const closeContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  /** 构建文件右键菜单 */
  const fileContextMenu = useCallback(
    (e: React.MouseEvent, node: TreeNode) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({
        visible: true,
        x: e.clientX,
        y: e.clientY,
        items: [
          {
            label: "打开",
            action: () => onOpenFile(node.entry.path),
          },
          {
            label: "在终端中打开",
            action: () => onOpenInTerminal(node.entry.path),
          },
          {
            label: "重命名",
            action: () => {
              setRenamingPath(node.entry.path);
              setRenameValue(node.entry.name);
            },
          },
          {
            label: "删除",
            action: () => {
              const name = node.entry.name;
              ask(`确定删除 "${name}"？此操作不可撤销。`, {
                title: "确认删除",
                kind: "warning",
              }).then((ok) => {
                if (ok) onDelete(node.entry.path);
              });
            },
          },
        ],
      });
    },
    [onOpenFile, onOpenInTerminal, onDelete],
  );

  /** 构建文件夹右键菜单 */
  const folderContextMenu = useCallback(
    (e: React.MouseEvent, node: TreeNode) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({
        visible: true,
        x: e.clientX,
        y: e.clientY,
        items: [
          {
            label: node.expanded ? "折叠" : "展开",
            action: () => onToggleExpand(node.entry.path),
          },
          {
            label: "在终端中打开",
            action: () => onOpenInTerminal(node.entry.path),
          },
          {
            label: "新建文件",
            action: () => setNewFileName(node.entry.path),
          },
          {
            label: "新建文件夹",
            action: () => setNewFolderName(node.entry.path),
          },
          {
            label: "重命名",
            action: () => {
              setRenamingPath(node.entry.path);
              setRenameValue(node.entry.name);
            },
          },
          {
            label: "删除",
            action: () => {
              const name = node.entry.name;
              ask(`确定删除文件夹 "${name}"？此操作不可撤销。`, {
                title: "确认删除",
                kind: "warning",
              }).then((ok) => {
                if (ok) onDelete(node.entry.path);
              });
            },
          },
        ],
      });
    },
    [onToggleExpand, onOpenInTerminal, onDelete],
  );

  const confirmRename = useCallback(() => {
    if (renamingPath && renameValue.trim()) {
      onRename(renamingPath, renameValue.trim());
    }
    setRenamingPath(null);
    setRenameValue("");
  }, [renamingPath, renameValue, onRename]);

  const confirmNewFile = useCallback(
    (parentPath: string, name: string) => {
      if (name.trim()) {
        onNewFile(`${parentPath}/${name.trim()}`);
      }
      setNewFileName(null);
    },
    [onNewFile],
  );

  const confirmNewFolder = useCallback(
    (parentPath: string, name: string) => {
      if (name.trim()) {
        onNewFolder(`${parentPath}/${name.trim()}`);
      }
      setNewFolderName(null);
    },
    [onNewFolder],
  );

  return (
    <>
      {nodes.map((node) => (
        <React.Fragment key={node.entry.path}>
          {/* 行 */}
          {renamingPath === node.entry.path ? (
            <div
              style={{
                display: "flex",
                paddingLeft: 8 + depth * 16 + 12 + 4 + 14,
                paddingRight: 8,
                height: 24,
                alignItems: "center",
              }}
            >
              <input
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                onBlur={confirmRename}
                onKeyDown={(e) => {
                  if (e.key === "Enter") confirmRename();
                  if (e.key === "Escape") {
                    setRenamingPath(null);
                  }
                }}
                autoFocus
                style={{
                  flex: 1,
                  background: INPUT_BG,
                  border: `1px solid ${FOCUS_BORDER}`,
                  color: SIDEBAR_FG,
                  fontSize: 13,
                  padding: "0 4px",
                  outline: "none",
                  borderRadius: 2,
                  minWidth: 0,
                }}
              />
            </div>
          ) : (
            <TreeNodeRow
              node={node}
              depth={depth}
              gitStatusMap={gitStatusMap}
              onToggleExpand={onToggleExpand}
              onOpenFile={onOpenFile}
              onContextMenu={(e) => {
                if (node.entry.isDir) {
                  folderContextMenu(e, node);
                } else {
                  fileContextMenu(e, node);
                }
              }}
            />
          )}

          {/* 新建文件输入框 */}
          {newFileName === node.entry.path && (
            <div
              style={{
                display: "flex",
                paddingLeft: 8 + (depth + 1) * 16 + 12 + 4 + 14,
                paddingRight: 8,
                height: 24,
                alignItems: "center",
              }}
            >
              <input
                placeholder="文件名"
                onBlur={(e) => confirmNewFile(node.entry.path, e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter")
                    confirmNewFile(
                      node.entry.path,
                      (e.target as HTMLInputElement).value,
                    );
                  if (e.key === "Escape") setNewFileName(null);
                }}
                autoFocus
                style={{
                  flex: 1,
                  background: INPUT_BG,
                  border: `1px solid ${FOCUS_BORDER}`,
                  color: SIDEBAR_FG,
                  fontSize: 13,
                  padding: "0 4px",
                  outline: "none",
                  borderRadius: 2,
                  minWidth: 0,
                }}
              />
            </div>
          )}

          {/* 新建文件夹输入框 */}
          {newFolderName === node.entry.path && (
            <div
              style={{
                display: "flex",
                paddingLeft: 8 + (depth + 1) * 16 + 12 + 4 + 14,
                paddingRight: 8,
                height: 24,
                alignItems: "center",
              }}
            >
              <input
                placeholder="文件夹名"
                onBlur={(e) =>
                  confirmNewFolder(node.entry.path, e.target.value)
                }
                onKeyDown={(e) => {
                  if (e.key === "Enter")
                    confirmNewFolder(
                      node.entry.path,
                      (e.target as HTMLInputElement).value,
                    );
                  if (e.key === "Escape") setNewFolderName(null);
                }}
                autoFocus
                style={{
                  flex: 1,
                  background: INPUT_BG,
                  border: `1px solid ${FOCUS_BORDER}`,
                  color: SIDEBAR_FG,
                  fontSize: 13,
                  padding: "0 4px",
                  outline: "none",
                  borderRadius: 2,
                  minWidth: 0,
                }}
              />
            </div>
          )}

          {/* 递归渲染子节点 */}
          {node.expanded && node.children.length > 0 && (
            <FileTree
              nodes={node.children}
              depth={depth + 1}
              gitStatusMap={gitStatusMap}
              onToggleExpand={onToggleExpand}
              onOpenFile={onOpenFile}
              onOpenInTerminal={onOpenInTerminal}
              onRename={onRename}
              onDelete={onDelete}
              onNewFile={onNewFile}
              onNewFolder={onNewFolder}
            />
          )}
        </React.Fragment>
      ))}

      {/* 右键菜单 */}
      <ContextMenu state={contextMenu} onClose={closeContextMenu} />
    </>
  );
};
