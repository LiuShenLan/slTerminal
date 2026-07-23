// FileTree.tsx — 递归文件树组件
//
// 职责：
// - 递归渲染文件/文件夹树
// - 单击选中 + 双击打开文件
// - 右键菜单 CRUD
// - git 状态色应用于文件名
// - 键盘快捷键（Del/Enter/F2）经 ExplorerPanel → ShortcutRegistry 派发

import React, { useState, useCallback } from "react";
import { FileIcon } from "./FileIcon";
import type { TreeNode } from "./useFileTree";
import {
  EXPLORER_COLORS,
  EXPLORER_SELECTION_BG,
  GIT_FILE_COLORS,
  SIDEBAR_BG,
  SIDEBAR_FG,
  SIDEBAR_COLORS,
  ACTIVE_SELECTION_BG,
  INPUT_BG,
  INPUT_BORDER,
  FOCUS_BORDER,
  CONTEXT_MENU_BORDER,
} from "../../theme";
import { ask } from "../../ipc/dialog";

// ---- 文件树布局几何常量 ----
// 用于计算各节点的 paddingLeft，对齐文件名文本起始位置

/** 节点行左侧基准内边距 (px)，与行样式的 padding: "1px 8px" 左侧一致 */
const PADDING_BASE = 8;
/** 每层深度缩进宽度 (px) */
const INDENT = 16;
/** 展开/折叠箭头占位宽度 (px)，对应 TreeNodeRow 中箭头 <span> 的 width */
const ARROW_WIDTH = 12;
/** 文件图标右侧外边距 (px)，对应图标 <span> 的 marginRight */
const ICON_MARGIN = 4;
/** 文件图标视觉宽度 (px)，对齐 FileIcon 渲染后的实际占用宽度 */
const ICON_WIDTH = 14;

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
        boxShadow: SIDEBAR_COLORS.contextMenuShadow,
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
  rootPath?: string; // 项目根路径，用于根级空白区域右键创建文件/文件夹
  // 选中模型（由 ExplorerPanel 管理）
  selectedPath: string | null;
  onSelect: (path: string | null) => void;
  // 重命名状态（由 ExplorerPanel 管理，从 FileTree 上提）
  renamingPath: string | null;
  renameValue: string;
  onRenameStart: (path: string, name: string) => void;
  onRenameCancel: () => void;
}

// ---- 单行节点 ----

const TreeNodeRow: React.FC<{
  node: TreeNode;
  depth: number;
  gitStatusMap: Map<string, string>;
  onToggleExpand: (path: string) => void;
  onOpenFile: (path: string) => void;
  onContextMenu: (e: React.MouseEvent) => void;
  isSelected: boolean;
  onSelect: (path: string) => void;
}> = ({ node, depth, gitStatusMap, onToggleExpand, onOpenFile, onContextMenu, isSelected, onSelect }) => {
  const { entry, expanded, loading } = node;
  // 渲染时实时查表，避免节点创建时写入 → 闭包陈旧/时序断裂问题
  const gitStatus = gitStatusMap.get(entry.path);
  const indent = depth * INDENT;

  return (
    <div
      onClick={() => {
        onSelect(entry.path);
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
        paddingLeft: PADDING_BASE + indent,
        cursor: "pointer",
        userSelect: "none",
        fontSize: 13,
        color: EXPLORER_COLORS.fg,
        height: 24,
        whiteSpace: "nowrap",
        // 选中态优先于 hover（style 内联优先级高于 onMouseEnter/Leave 动态设置）
        background: isSelected ? EXPLORER_SELECTION_BG : "transparent",
      }}
      onMouseEnter={(e) => {
        if (!isSelected) {
          (e.target as HTMLDivElement).style.background = EXPLORER_COLORS.hover;
        }
      }}
      onMouseLeave={(e) => {
        if (!isSelected) {
          (e.target as HTMLDivElement).style.background = "transparent";
        }
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
  rootPath,
  selectedPath,
  onSelect,
  renamingPath,
  renameValue,
  onRenameStart,
  onRenameCancel,
}) => {
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    items: [],
  });

  // 新建文件/文件夹的输入框状态（仍由 FileTree 本地管理）
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
              onRenameStart(node.entry.path, node.entry.name);
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
    [onOpenFile, onOpenInTerminal, onDelete, onRenameStart],
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
              onRenameStart(node.entry.path, node.entry.name);
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
    [onToggleExpand, onOpenInTerminal, onDelete, onRenameStart],
  );

  // rename input ref——读取用户实际输入值（renameValue prop 仅作 defaultValue，不追踪变化）
  const renameInputRef = React.useRef<HTMLInputElement>(null);

  const confirmRename = useCallback(() => {
    const newName = renameInputRef.current?.value.trim();
    if (renamingPath && newName) {
      onRename(renamingPath, newName);
    } else {
      onRenameCancel();
    }
  }, [renamingPath, onRename, onRenameCancel]);

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

  /** 构建根级空白区域右键菜单（depth === 0 且 rootPath 存在时） */
  const rootContextMenu = useCallback(
    (e: React.MouseEvent) => {
      if (!rootPath) return;
      e.preventDefault();
      setContextMenu({
        visible: true,
        x: e.clientX,
        y: e.clientY,
        items: [
          {
            label: "新建文件",
            action: () => setNewFileName(rootPath),
          },
          {
            label: "新建文件夹",
            action: () => setNewFolderName(rootPath),
          },
        ],
      });
    },
    [rootPath],
  );

  // 根级内联输入框渲染（复用已有样式，depth+1=1 缩进）
  const renderRootInlineInput = () => (
    <>
      {rootPath && newFileName === rootPath && (
        <div
          style={{
            display: "flex",
            paddingLeft: PADDING_BASE + (depth + 1) * INDENT + ARROW_WIDTH + ICON_MARGIN + ICON_WIDTH,
            paddingRight: 8,
            height: 24,
            alignItems: "center",
          }}
        >
          <input
            placeholder="文件名"
            onBlur={(e) => confirmNewFile(rootPath, e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter")
                confirmNewFile(
                  rootPath,
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
      {rootPath && newFolderName === rootPath && (
        <div
          style={{
            display: "flex",
            paddingLeft: PADDING_BASE + (depth + 1) * INDENT + ARROW_WIDTH + ICON_MARGIN + ICON_WIDTH,
            paddingRight: 8,
            height: 24,
            alignItems: "center",
          }}
        >
          <input
            placeholder="文件夹名"
            onBlur={(e) =>
              confirmNewFolder(rootPath, e.target.value)
            }
            onKeyDown={(e) => {
              if (e.key === "Enter")
                confirmNewFolder(
                  rootPath,
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
    </>
  );

  // 公共树节点渲染（depth > 0 直接返回，depth === 0 套 wrapper）
  const treeContent = (
    <>
      {nodes.map((node) => (
        <React.Fragment key={node.entry.path}>
          {/* 行 */}
          {renamingPath === node.entry.path ? (
            <div
              style={{
                display: "flex",
                paddingLeft: PADDING_BASE + depth * INDENT + ARROW_WIDTH + ICON_MARGIN + ICON_WIDTH,
                paddingRight: 8,
                height: 24,
                alignItems: "center",
              }}
            >
              <input
                ref={renameInputRef}
                defaultValue={renameValue}
                onBlur={confirmRename}
                onKeyDown={(e) => {
                  if (e.key === "Enter") confirmRename();
                  if (e.key === "Escape") {
                    onRenameCancel();
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
              isSelected={selectedPath === node.entry.path}
              onSelect={onSelect}
            />
          )}

          {/* 新建文件输入框 */}
          {newFileName === node.entry.path && (
            <div
              style={{
                display: "flex",
                paddingLeft: PADDING_BASE + (depth + 1) * INDENT + ARROW_WIDTH + ICON_MARGIN + ICON_WIDTH,
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
                paddingLeft: PADDING_BASE + (depth + 1) * INDENT + ARROW_WIDTH + ICON_MARGIN + ICON_WIDTH,
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
              selectedPath={selectedPath}
              onSelect={onSelect}
              renamingPath={renamingPath}
              renameValue={renameValue}
              onRenameStart={onRenameStart}
              onRenameCancel={onRenameCancel}
            />
          )}
        </React.Fragment>
      ))}

      {/* 右键菜单 */}
      <ContextMenu state={contextMenu} onClose={closeContextMenu} />
    </>
  );

  // 顶层（depth === 0）：wrapper div 捕获空白区域右键 + 单击空白取消选中
  if (depth === 0) {
    return (
      <div
        style={{ minHeight: "100%" }}
        onContextMenu={rootContextMenu}
        onClick={(e) => {
          // 仅在点击 wrapper 自身（非子节点）时取消选中
          if (e.target === e.currentTarget) {
            onSelect(null);
          }
        }}
      >
        {nodes.length === 0 && rootPath && (
          <div
            style={{
              padding: 16,
              color: INPUT_BORDER,
              fontSize: 12,
              textAlign: "center",
            }}
          >
            空目录
          </div>
        )}
        {renderRootInlineInput()}
        {treeContent}
      </div>
    );
  }

  return treeContent;
};
