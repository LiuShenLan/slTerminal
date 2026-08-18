// useFileTree.ts — 文件树数据 hook
//
// 职责：
// - 调用 fs_read_dir 获取目录内容
// - 订阅 "fs-event" 进行增量刷新（200ms 去抖）
// - 订阅 slterm:file-saved 保存事件（300ms 去抖——FE-15：已知路径只刷新受影响子树）
// - 调用 git_status 获取 git 文件状态
// - 处理 need_rescan 全量刷新

import { useState, useEffect, useCallback, useRef } from "react";
import { onFsEvent } from "../../ipc/notify";
import { readDir } from "../../ipc/fs";
import { gitStatus } from "../../ipc/git";
import type { DirEntry } from "../../types/fs";
// FE-07: 错误消息统一经 getErrorMessage（契约：src/ipc/appError.ts，src/lib re-export）
import { getErrorMessage } from "../../lib";
import { normalizePath } from "../../lib/path";

/** 文件系统事件去抖延迟（ms） */
const FS_EVENT_DEBOUNCE_MS = 200;
/** file-saved 保存事件去抖延迟（ms）——Ctrl+S 连按高频，FE-15 由无去抖改 300ms */
const FILE_SAVED_DEBOUNCE_MS = 300;

export interface TreeNode {
  entry: DirEntry;
  expanded: boolean;
  children: TreeNode[];
  loading: boolean;
}

interface UseFileTreeOptions {
  rootPath: string | null;
}

export function useFileTree({ rootPath }: UseFileTreeOptions) {
  const [rootNodes, setRootNodes] = useState<TreeNode[]>([]);
  const [gitStatusMap, setGitStatusMap] = useState<Map<string, string>>(
    new Map(),
  );
  // FE-07: 目录加载错误按路径记录（路径 → 错误消息）。readDir 失败时
  // loadDirectory 仍返回 []（子目录容错不冒泡），但错误不再伪装成空目录
  const [dirErrors, setDirErrors] = useState<Map<string, string>>(new Map());
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // FE-15: file-saved 去抖定时器 + 去抖窗口内最后保存路径（已知路径 → 子树刷新）
  const fileSavedDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fileSavedPathRef = useRef<string | null>(null);
  const rootPathRef = useRef<string | null>(rootPath);
  rootPathRef.current = rootPath;
  // 镜像最新 rootNodes，供 reloadPreservingExpanded 异步回调读取「触发时刻的旧树」
  const rootNodesRef = useRef<TreeNode[]>(rootNodes);
  rootNodesRef.current = rootNodes;
  // generation 计数器：rootPath 每次变化时递增，异步回调中检查以丢弃旧请求
  const genRef = useRef(0);

  /** 读取目录内容并转换为 TreeNode。失败时记录按路径错误并返回 []（子目录容错不冒泡） */
  const loadDirectory = useCallback(
    async (dirPath: string): Promise<TreeNode[]> => {
      try {
        const entries = await readDir(dirPath);
        // 读取成功 → 清除该路径的加载错误
        setDirErrors((prev) => {
          if (!prev.has(dirPath)) return prev;
          const next = new Map(prev);
          next.delete(dirPath);
          return next;
        });
        return entries.map((entry) => ({
          entry,
          expanded: false,
          children: [],
          loading: false,
        }));
      } catch (err) {
        console.error("[slTerminal] readDir 失败:", dirPath, err);
        // FE-07: 错误按路径记录（不再伪装空目录），ExplorerPanel 据此渲染错误占位
        const msg = getErrorMessage(err);
        setDirErrors((prev) => {
          const next = new Map(prev);
          next.set(dirPath, msg);
          return next;
        });
        return [];
      }
    },
    [],
  );

  /** 加载根目录。gen 参数用于 rootPath 变化时丢弃旧请求的过期结果 */
  const loadRoot = useCallback(async (gen?: number) => {
    if (!rootPath) {
      if (gen === undefined || gen === genRef.current) setRootNodes([]);
      return;
    }
    const nodes = await loadDirectory(rootPath);
    // generation 检查：如果 gen 不匹配，说明 rootPath 已变化，丢弃此结果
    if (gen !== undefined && gen !== genRef.current) return;
    setRootNodes(nodes);
  }, [rootPath, loadDirectory]);

  /** 加载子目录 */
  const loadChildren = useCallback(
    async (parentPath: string): Promise<TreeNode[]> => {
      return loadDirectory(parentPath);
    },
    [loadDirectory],
  );

  /** 切换文件夹展开/折叠 */
  const toggleExpand = useCallback(
    async (nodePath: string) => {
      setRootNodes((prev) => {
        const updateNode = (nodes: TreeNode[]): TreeNode[] =>
          nodes.map((node) => {
            if (node.entry.path === nodePath) {
              if (node.expanded) {
                // 折叠
                return { ...node, expanded: false };
              }
              // 展开 → 返回带 loading 标记的节点，触发异步加载
              const newChildren = node.children.length === 0 && !node.loading;
              return {
                ...node,
                expanded: true,
                loading: newChildren,
              };
            }
            if (node.expanded && node.children.length > 0) {
              return { ...node, children: updateNode(node.children) };
            }
            return node;
          });
        return updateNode(prev);
      });

      // 异步加载子目录数据
      const children = await loadChildren(nodePath);
      setRootNodes((prev) => {
        const updateNode = (nodes: TreeNode[]): TreeNode[] =>
          nodes.map((node) => {
            if (node.entry.path === nodePath) {
              return {
                ...node,
                children: children.map((child) => ({
                  ...child,
                })),
                loading: false,
              };
            }
            if (node.expanded && node.children.length > 0) {
              return { ...node, children: updateNode(node.children) };
            }
            return node;
          });
        return updateNode(prev);
      });
    },
    [loadChildren],
  );

  /** 递归重载文件树：对旧树中所有已展开目录（任意深度）重新 readDir 重建子树，
   *  保留 expanded=true；同时反映子目录内文件增删。不传 gen（操作当前页数据）。 */
  const reloadPreservingExpanded = useCallback(async () => {
    const rp = rootPathRef.current;
    if (!rp) {
      setRootNodes([]);
      return;
    }

    // 递归重建 dirPath 一层：新一层与旧节点按 path 匹配，曾展开的目录递归下钻
    const rebuild = async (
      dirPath: string,
      oldNodes: TreeNode[],
    ): Promise<TreeNode[]> => {
      const fresh = await loadDirectory(dirPath); // 全新一层：expanded=false/children=[]/loading=false
      const oldByPath = new Map(oldNodes.map((n) => [n.entry.path, n]));
      return Promise.all(
        fresh.map(async (node) => {
          const old = oldByPath.get(node.entry.path);
          if (old?.expanded && node.entry.isDir) {
            const children = await rebuild(node.entry.path, old.children);
            return { ...node, expanded: true, children, loading: false };
          }
          return node; // 文件 / 新增项 / 曾折叠项：保持折叠
        }),
      );
    };

    const next = await rebuild(rp, rootNodesRef.current);
    setRootNodes(next);
  }, [loadDirectory]);

  /** 刷新 git 状态（generation 检查防竞态：rootPath 切换时丢弃旧请求结果）。
   *  FE-15 从 refreshExpanded 拆出——子树刷新路径同样需要 git 着色刷新。 */
  const refreshGitStatus = useCallback(async () => {
    const rp = rootPathRef.current;
    if (!rp) return;
    const gen = genRef.current;
    try {
      const statuses = await gitStatus(rp);
      if (gen !== genRef.current) return; // rootPath 已变化，丢弃过期结果
      const map = new Map<string, string>();
      for (const s of statuses) {
        map.set(s.path, s.status);
      }
      setGitStatusMap(map);
    } catch {
      if (gen !== genRef.current) return; // rootPath 已变化，丢弃过期错误
      setGitStatusMap(new Map());
    }
  }, []);

  /** 刷新展开的节点（文件变更时增量刷新，保留展开状态） */
  const refreshExpanded = useCallback(async () => {
    const rp = rootPathRef.current;
    if (!rp) return;

    // 保留展开态整树重载（替代原 loadRoot() 的整树折叠替换）
    await reloadPreservingExpanded();

    // 刷新 git 状态（generation 检查防竞态）
    await refreshGitStatus();
  }, [reloadPreservingExpanded, refreshGitStatus]);

  /** 按变更路径刷新受影响子树（FE-15）：在旧树中定位「变更路径的最近展开祖先」，
   *  仅重载该目录一层并原位合并（保留曾展开子目录的展开态与子树），
   *  不 refreshExpanded 全量重建（不再逐个 readDir 全部展开目录）。
   *  变更路径不在当前 rootPath 下 → 跳过并返回 false（调用方据此跳过 git 着色刷新）；
   *  位于未展开目录内 → 只刷新其父层（折叠目录行元数据同步，不深挖未加载子树）。
   *  返回是否实际刷新。 */
  const refreshSubtreeAt = useCallback(
    async (changedPath: string): Promise<boolean> => {
      const rp = rootPathRef.current;
      if (!rp) return false;

      // 规范化 + 忽略大小写 + 去尾部斜杠（照 isCwdUnderProject 同口径）
      const norm = (p: string) =>
        normalizePath(p).toLowerCase().replace(/\/+$/, "");
      const root = norm(rp);
      const changed = norm(changedPath);
      // 变更路径不属于当前项目（如保存了其他项目的文件）→ 无需刷新
      if (changed !== root && !changed.startsWith(`${root}/`)) return false;

      // 沿变更路径逐层下钻定位最近展开祖先：命中「已展开目录」才继续下钻；
      // 新条目（保存新建）/ 文件 / 折叠目录 → 当前层即刷新目标
      let targetPath = rp; // 待重载目录（最近展开祖先）
      let curNodes = rootNodesRef.current;
      const rest =
        changed === root ? [] : changed.slice(root.length + 1).split("/");
      for (const seg of rest) {
        const candidate = `${norm(targetPath)}/${seg}`;
        const node = curNodes.find((n) => norm(n.entry.path) === candidate);
        if (!node || !node.entry.isDir || !node.expanded) break;
        targetPath = node.entry.path;
        curNodes = node.children;
      }

      // 重载目标目录一层，原位合并（保留子节点展开态与子树）
      const fresh = await loadDirectory(targetPath);
      setRootNodes((prev) => {
        const mergeLayer = (
          freshNodes: TreeNode[],
          oldNodes: TreeNode[],
        ): TreeNode[] => {
          const oldByPath = new Map(oldNodes.map((n) => [n.entry.path, n]));
          return freshNodes.map((node) => {
            const old = oldByPath.get(node.entry.path);
            if (old?.expanded && node.entry.isDir) {
              return {
                ...node,
                expanded: true,
                children: old.children,
                loading: false,
              };
            }
            return node;
          });
        };
        if (targetPath === rp) return mergeLayer(fresh, prev); // 根层直接替换
        const apply = (nodes: TreeNode[]): TreeNode[] =>
          nodes.map((node) => {
            if (node.entry.path === targetPath) {
              return {
                ...node,
                children: mergeLayer(fresh, node.children),
                loading: false,
              };
            }
            if (node.expanded && node.children.length > 0) {
              return { ...node, children: apply(node.children) };
            }
            return node;
          });
        return apply(prev);
      });
      return true;
    },
    [loadDirectory],
  );

  // 根路径变更时重新加载
  useEffect(() => {
    const gen = ++genRef.current;
    // rootPath 变化时立即清空旧数据，避免残留旧项目的文件树
    if (!rootPath) {
      setRootNodes([]);
      setGitStatusMap(new Map());
      setDirErrors(new Map());
      return;
    }
    setRootNodes([]);
    setGitStatusMap(new Map());
    setDirErrors(new Map());
    loadRoot(gen);
    // 同时加载 git 状态
    gitStatus(rootPath)
      .then((statuses) => {
        if (gen !== genRef.current) return; // 丢弃旧请求（rootPath 已变化）
        const map = new Map<string, string>();
        for (const s of statuses) {
          map.set(s.path, s.status);
        }
        setGitStatusMap(map);
      })
      .catch((err) => {
        console.error("[slTerminal] gitStatus 失败:", rootPath, err);
        if (gen !== genRef.current) return; // 丢弃旧请求的错误处理
        setGitStatusMap(new Map());
      });
  }, [rootPath, loadRoot]);

  // 订阅文件系统事件（200ms 去抖增量刷新）
  useEffect(() => {
    const unlisten = onFsEvent(() => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      debounceRef.current = setTimeout(() => {
        refreshExpanded();
      }, FS_EVENT_DEBOUNCE_MS);
    });

    return () => {
      unlisten();
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [refreshExpanded]);

  // 监听编辑器保存事件（FE-15）：300ms 去抖后刷新 git 着色 + 受影响子树。
  // 已知路径（detail.path）→ 只刷新最近展开祖先子树；缺 path → 全量刷新。
  // 已保存文件的 git 状态立即清除（先显示白色，不延迟），解决 autocrlf 场景
  // 不依赖 fs-event 的时序竞态。
  useEffect(() => {
    const handler = (e: Event) => {
      const ce = e as CustomEvent<{ path?: string }>;
      const savedPath = ce.detail?.path;
      if (savedPath) {
        // 立即清除已保存文件的 git 状态（先显示白色）——即时反馈不延迟
        setGitStatusMap((prev) => {
          const next = new Map(prev);
          next.delete(savedPath);
          return next;
        });
        fileSavedPathRef.current = savedPath;
      }
      if (fileSavedDebounceRef.current) {
        clearTimeout(fileSavedDebounceRef.current);
      }
      fileSavedDebounceRef.current = setTimeout(() => {
        fileSavedDebounceRef.current = null;
        const p = fileSavedPathRef.current;
        fileSavedPathRef.current = null;
        if (p) {
          // 已知路径 → 子树刷新；路径归属当前项目才补 git 着色刷新
          // （不属于当前项目时树与着色均无需变——FE-15 范围收敛）
          void refreshSubtreeAt(p).then((refreshed) => {
            if (refreshed) void refreshGitStatus();
          });
        } else {
          // 缺路径 → 全量刷新（refreshExpanded 内含 git 着色）
          void refreshExpanded();
        }
      }, FILE_SAVED_DEBOUNCE_MS);
    };
    window.addEventListener("slterm:file-saved", handler);
    return () => {
      window.removeEventListener("slterm:file-saved", handler);
      if (fileSavedDebounceRef.current) {
        clearTimeout(fileSavedDebounceRef.current);
        fileSavedDebounceRef.current = null;
      }
      fileSavedPathRef.current = null;
    };
  }, [refreshSubtreeAt, refreshExpanded, refreshGitStatus]);

  // FE-07: 根目录加载错误（dirErrors 按路径记录，仅 rootPath 命中才暴露给面板）
  const rootError = rootPath ? (dirErrors.get(rootPath) ?? null) : null;

  return {
    rootNodes,
    gitStatusMap,
    rootError,
    toggleExpand,
    refresh: refreshExpanded,
  };
}
