// FileTree.tsx — 文件树组件（FE-30 虚拟化渲染）
//
// 职责：
// - 扁平化可见节点数组 + 固定行高 + overscan 滚动窗口渲染（零新依赖手实现虚拟化，FE-30）
// - 单击选中 + 双击打开文件
// - 右键菜单 CRUD
// - git 状态色应用于文件名
// - 键盘快捷键（Del/Enter/F2）经 ExplorerPanel → ShortcutRegistry 派发
//
// 虚拟化设计（FE-30）：
// - 整棵树按展开状态深度优先扁平化为可见行数组（每行固定 24px），
//   原「每层递归创建 FileTree 实例」的结构由一次扁平化取代——窗口切片在顶层一次完成。
// - 滚动容器为组件自持 div（height:100% + overflowY:auto），窗口切片以
//   scrollTop / 容器高度计算，上下各 OVERSCAN 行缓冲，content 用 paddingTop/Bottom 占位保持滚动条正确。
// - 容器高度未测得（clientHeight === 0，如 jsdom 测试环境/布局异常）时回退全量渲染，保证功能可用。

import React, { useState, useCallback, useMemo, useRef, useLayoutEffect } from "react";
import { FileIcon } from "./FileIcon";
import type { TreeNode } from "./useFileTree";
import {
  EXPLORER_COLORS,
  EXPLORER_SELECTION_BG,
  GIT_FILE_COLORS,
  SIDEBAR_BG,
  SIDEBAR_FG,
  SIDEBAR_COLORS,
  SECONDARY_BG,
  ERROR_FG,
  INPUT_BG,
  PLACEHOLDER_FG,
  DIM_FG,
  FOCUS_BORDER,
  CONTEXT_MENU_BORDER,
} from "../../theme";
import { confirmDialog } from "../../lib/ConfirmDialog";
import {
  IconChevronRight,
  IconChevronDown,
  IconEmptyBox,
} from "../../lib/icons";

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
/** 节点行高 (px)——与 TreeNodeRow 的 height 一致（UI-305 紧凑列表档）；虚拟化按此行高计算窗口 */
const ROW_HEIGHT = 24;
/** 滚动窗口上下 overscan 行数（缓冲渲染，防快速滚动白屏） */
const OVERSCAN = 8;

// ---- 右键菜单 ----

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  items: ContextMenuItem[];
}

interface ContextMenuItem {
  label: string;
  /** 危险项（删除类）——ERROR_FG 着色（UI-802） */
  danger?: boolean;
  action: () => void;
}

/** 菜单项行样式（项 28px、圆角 5——UI-802） */
const itemStyle: React.CSSProperties = {
  height: 28,
  margin: "0 4px",
  borderRadius: 5,
  display: "flex",
  alignItems: "center",
  padding: "0 12px",
  fontSize: 12,
  userSelect: "none",
};

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
        borderRadius: 5,
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
            ...itemStyle,
            cursor: "pointer",
            color: item.danger ? ERROR_FG : SIDEBAR_FG,
          }}
          onMouseEnter={(e) => {
            (e.target as HTMLDivElement).style.background = SECONDARY_BG;
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
  // hover 态用 React state 驱动（照 NavProjectRow/NavPageRow 既有模式，不直改 DOM）
  const [hovered, setHovered] = useState(false);
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
        height: ROW_HEIGHT,
        whiteSpace: "nowrap",
        // 选中态优先于 hover（渲染判定 isSelected 先于 hovered）
        background: isSelected
          ? EXPLORER_SELECTION_BG
          : hovered
            ? EXPLORER_COLORS.hover
            : "transparent",
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* 展开/折叠箭头（chevron 12px，色经 EXPLORER_COLORS arrow 槽位 token——IC-05） */}
      {entry.isDir ? (
        <span
          style={{
            width: 12,
            flexShrink: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: expanded
              ? EXPLORER_COLORS.arrowOpen
              : EXPLORER_COLORS.arrowClosed,
          }}
        >
          {/* 加载中 → 「…」三点（执行期定：icons.tsx 无 spinner 导出，不新建组件） */}
          {loading ? "…" : expanded ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
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

// ---- FE-30 虚拟化：扁平化可见行 ----

/** 扁平化后的可见行（节点行 + 新建输入框行；重命名是替换节点行，不额外占位） */
interface FlatRow {
  /** React key（节点行 = 节点路径；输入框行 = 路径 + 类型后缀） */
  key: string;
  /** 节点行时为 TreeNode，输入框行为 null */
  node: TreeNode | null;
  /** 行缩进深度（输入框行 = 父节点深度 + 1） */
  depth: number;
  /** 行类型 */
  kind: "node" | "newFile" | "newFolder";
  /** 新建输入框行的父目录路径（node 行为 undefined） */
  parentPath?: string;
}

/**
 * 深度优先扁平化可见节点数组：节点行 → 新建文件/文件夹输入框行 → 展开的子节点。
 * 取代原「每层递归创建 FileTree 实例」的结构——虚拟化窗口在顶层一次切片。
 */
function flattenVisible(
  nodes: TreeNode[],
  depth: number,
  newFileName: string | null,
  newFolderName: string | null,
  rows: FlatRow[] = [],
): FlatRow[] {
  for (const node of nodes) {
    rows.push({ key: node.entry.path, node, depth, kind: "node" });
    if (newFileName === node.entry.path) {
      rows.push({
        key: `${node.entry.path}::new-file`,
        node: null,
        depth: depth + 1,
        kind: "newFile",
        parentPath: node.entry.path,
      });
    }
    if (newFolderName === node.entry.path) {
      rows.push({
        key: `${node.entry.path}::new-folder`,
        node: null,
        depth: depth + 1,
        kind: "newFolder",
        parentPath: node.entry.path,
      });
    }
    if (node.expanded && node.children.length > 0) {
      flattenVisible(node.children, depth + 1, newFileName, newFolderName, rows);
    }
  }
  return rows;
}

/** 内联输入框行容器样式（重命名/新建共用，行高与节点行一致） */
const inlineInputRowStyle: React.CSSProperties = {
  display: "flex",
  paddingRight: 8,
  height: ROW_HEIGHT,
  alignItems: "center",
};

/** 内联输入框样式（UI-808：input 键盘可达，去 outline:none 让全局 :focus-visible 环生效） */
const inlineInputStyle: React.CSSProperties = {
  flex: 1,
  background: INPUT_BG,
  border: `1px solid ${FOCUS_BORDER}`,
  color: SIDEBAR_FG,
  fontSize: 13,
  padding: "0 4px",
  borderRadius: 8, // GL-03：输入框圆角收敛 2→8
  minWidth: 0,
};

/** 内联输入框行的缩进（对齐文件名文本起始位置） */
const inputRowPaddingLeft = (depth: number) =>
  PADDING_BASE + depth * INDENT + ARROW_WIDTH + ICON_MARGIN + ICON_WIDTH;

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
            danger: true,
            action: () => {
              const name = node.entry.name;
              confirmDialog({
                title: "确认删除",
                message: `确定删除 "${name}"？此操作不可撤销。`,
                kind: "warning",
                danger: true,
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
            danger: true,
            action: () => {
              const name = node.entry.name;
              confirmDialog({
                title: "确认删除",
                message: `确定删除文件夹 "${name}"？此操作不可撤销。`,
                kind: "warning",
                danger: true,
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
            ...inlineInputRowStyle,
            paddingLeft: inputRowPaddingLeft(depth + 1),
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
            style={inlineInputStyle}
          />
        </div>
      )}
      {rootPath && newFolderName === rootPath && (
        <div
          style={{
            ...inlineInputRowStyle,
            paddingLeft: inputRowPaddingLeft(depth + 1),
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
            style={inlineInputStyle}
          />
        </div>
      )}
    </>
  );

  // ---- FE-30 虚拟化：扁平化 + 滚动窗口 ----

  // 深度优先扁平化可见行（含新建输入框行；重命名是替换行不占位）
  const rows = useMemo(
    () => flattenVisible(nodes, depth, newFileName, newFolderName),
    [nodes, depth, newFileName, newFolderName],
  );

  // 滚动视口状态：scrollTop + 容器高度。
  // height === 0（未测得：jsdom 测试环境/布局异常）→ 窗口退化为全量渲染兜底。
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [viewport, setViewport] = useState({ scrollTop: 0, height: 0 });

  // 初始同步测量 + ResizeObserver 跟踪容器高度变化；
  // 卸载时 disconnect（侧栏视图卸载，S12/FE-21 兼容——无残留订阅）。
  // deps 含 hasList：根目录异步加载完成前 nodes 为空、容器尚未渲染，
  // 首次挂载时测量会落空——容器出现后须重跑测量，虚拟化才会生效。
  const hasList = nodes.length > 0;
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    // 容器重建（hasList false→true，如 rootPath 切换/加载失败重试）时重置滚动位置，
    // 避免残留 scrollTop 造成新树窗口偏移（滚动条与内容错位）
    setViewport((v) => (v.scrollTop === 0 ? v : { ...v, scrollTop: 0 }));
    const measure = () => {
      const h = el.clientHeight;
      setViewport((v) => (v.height === h ? v : { ...v, height: h }));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [hasList]);

  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const st = e.currentTarget.scrollTop;
    setViewport((v) => (v.scrollTop === st ? v : { ...v, scrollTop: st }));
  }, []);

  // 可见行切片：start/end 各含 OVERSCAN 缓冲行，clamp 到 [0, total]（树切换后 scrollTop 可能越界）
  const total = rows.length;
  const height = viewport.height;
  const start =
    height > 0
      ? Math.min(Math.max(0, Math.floor(viewport.scrollTop / ROW_HEIGHT) - OVERSCAN), total)
      : 0;
  const end =
    height > 0
      ? Math.min(total, Math.ceil((viewport.scrollTop + height) / ROW_HEIGHT) + OVERSCAN)
      : total;
  const visibleRows = rows.slice(start, end);

  // FE-40：程序式选中视口外行时滚动跟随——selectedPath 变化且对应行不在
  // [start, end] 窗口内 → scrollTop 定位使该行可见（虚拟化常见缺口补齐；
  // 鼠标点击天然落在可见区，本 effect 服务 explorer.open 等程序式选中路径）
  useLayoutEffect(() => {
    if (!selectedPath) return;
    const index = rows.findIndex(
      (row) => row.kind === "node" && row.node?.entry.path === selectedPath,
    );
    if (index < 0 || (index >= start && index < end)) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = index * ROW_HEIGHT;
  }, [selectedPath, rows, start, end]);

  /** 点击虚拟化内容区空白（padding 占位区域）→ 取消选中（与原「点击树下方空白取消选中」一致） */
  const handleContentBlankClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget) onSelect(null);
    },
    [onSelect],
  );

  // 单行渲染：新建输入框行 / 重命名输入框行（替换节点行）/ 节点行
  const renderRow = (row: FlatRow) => {
    // 新建文件/文件夹输入框行（父节点行之后追加）
    if (row.kind !== "node") {
      const isFile = row.kind === "newFile";
      const confirm = isFile ? confirmNewFile : confirmNewFolder;
      return (
        <div
          key={row.key}
          style={{
            ...inlineInputRowStyle,
            paddingLeft: inputRowPaddingLeft(row.depth),
          }}
        >
          <input
            placeholder={isFile ? "文件名" : "文件夹名"}
            onBlur={(e) => confirm(row.parentPath!, e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter")
                confirm(row.parentPath!, (e.target as HTMLInputElement).value);
              if (e.key === "Escape")
                (isFile ? setNewFileName : setNewFolderName)(null);
            }}
            autoFocus
            style={inlineInputStyle}
          />
        </div>
      );
    }
    const node = row.node!;
    // 重命名输入框行（替换节点行，不额外占位）
    if (renamingPath === node.entry.path) {
      return (
        <div
          key={row.key}
          style={{
            ...inlineInputRowStyle,
            paddingLeft: inputRowPaddingLeft(row.depth),
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
            style={inlineInputStyle}
          />
        </div>
      );
    }
    return (
      <TreeNodeRow
        key={row.key}
        node={node}
        depth={row.depth}
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
    );
  };

  // 虚拟化列表：自持滚动容器 + padding 占位（top/bottom spacer 用容器 padding 而非子 div——
  // 点击占位区域命中 content 自身，可触发空白取消选中）
  const virtualList = (
    <div
      ref={scrollRef}
      onScroll={handleScroll}
      style={{ height: "100%", overflowY: "auto", overflowX: "hidden" }}
    >
      <div
        onClick={handleContentBlankClick}
        style={{
          paddingTop: start * ROW_HEIGHT,
          paddingBottom: (total - end) * ROW_HEIGHT,
        }}
      >
        {visibleRows.map(renderRow)}
      </div>
    </div>
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
          // GL-05：空目录空态统一——15px 线性图标 fg-4 + 说明文字 fg-3，居中
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 8,
              padding: 16,
              fontSize: 12,
              color: DIM_FG, // 说明文字 fg-3
              textAlign: "center",
              userSelect: "none",
            }}
          >
            <span style={{ color: PLACEHOLDER_FG, display: "flex" }}>
              <IconEmptyBox size={15} />
            </span>
            <span>空目录</span>
          </div>
        )}
        {renderRootInlineInput()}
        {nodes.length > 0 && virtualList}

        {/* 右键菜单 */}
        <ContextMenu state={contextMenu} onClose={closeContextMenu} />
      </div>
    );
  }

  return virtualList;
};
