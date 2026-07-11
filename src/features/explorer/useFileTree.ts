// useFileTree.ts — 文件树数据 hook
//
// 职责：
// - 调用 fs_read_dir 获取目录内容
// - 订阅 "fs-event" 进行增量刷新（200ms 去抖）
// - 调用 git_status 获取 git 文件状态
// - 处理 need_rescan 全量刷新

import { useState, useEffect, useCallback, useRef } from "react";
import { onFsEvent } from "../../ipc/notify";
import { readDir } from "../../ipc/fs";
import { gitStatus } from "../../ipc/git";
import type { DirEntry } from "../../types/fs";

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
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const rootPathRef = useRef<string | null>(rootPath);
  rootPathRef.current = rootPath;
  // 镜像最新 rootNodes，供 reloadPreservingExpanded 异步回调读取「触发时刻的旧树」
  const rootNodesRef = useRef<TreeNode[]>(rootNodes);
  rootNodesRef.current = rootNodes;
  // generation 计数器：rootPath 每次变化时递增，异步回调中检查以丢弃旧请求
  const genRef = useRef(0);

  /** 读取目录内容并转换为 TreeNode */
  const loadDirectory = useCallback(
    async (dirPath: string): Promise<TreeNode[]> => {
      try {
        const entries = await readDir(dirPath);
        return entries.map((entry) => ({
          entry,
          expanded: false,
          children: [],
          loading: false,
        }));
      } catch {
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

  /** 刷新展开的节点（文件变更时增量刷新，保留展开状态） */
  const refreshExpanded = useCallback(async () => {
    const rp = rootPathRef.current;
    if (!rp) return;

    // 保留展开态整树重载（替代原 loadRoot() 的整树折叠替换）
    await reloadPreservingExpanded();

    // 刷新 git 状态
    try {
      const statuses = await gitStatus(rp);
      const map = new Map<string, string>();
      for (const s of statuses) {
        map.set(s.path, s.status);
      }
      setGitStatusMap(map);
    } catch {
      setGitStatusMap(new Map());
    }
  }, [reloadPreservingExpanded]);

  /** 全量刷新（need_rescan 触发） */
  const fullRefresh = useCallback(async () => {
    await loadRoot();
    const rp = rootPathRef.current;
    if (rp) {
      try {
        const statuses = await gitStatus(rp);
        const map = new Map<string, string>();
        for (const s of statuses) {
          map.set(s.path, s.status);
        }
        setGitStatusMap(map);
      } catch {
        setGitStatusMap(new Map());
      }
    }
  }, [loadRoot]);

  // 根路径变更时重新加载
  useEffect(() => {
    const gen = ++genRef.current;
    loadRoot(gen);
    // 同时加载 git 状态
    if (rootPath) {
      gitStatus(rootPath)
        .then((statuses) => {
          if (gen !== genRef.current) return; // 丢弃旧请求（rootPath 已变化）
          const map = new Map<string, string>();
          for (const s of statuses) {
            map.set(s.path, s.status);
          }
          setGitStatusMap(map);
        })
        .catch(() => {
          if (gen !== genRef.current) return; // 丢弃旧请求的错误处理
          setGitStatusMap(new Map());
        });
    }
  }, [rootPath, loadRoot]);

  // 订阅文件系统事件（200ms 去抖增量刷新）
  useEffect(() => {
    const unlisten = onFsEvent(() => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      debounceRef.current = setTimeout(() => {
        refreshExpanded();
      }, 200);
    });

    return () => {
      unlisten();
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [refreshExpanded]);

  // 监听编辑器保存事件，立即刷新 git 着色（不依赖 fs-event 的时序竞态）
  useEffect(() => {
    const handler = (e: Event) => {
      const ce = e as CustomEvent<{ path?: string }>;
      const savedPath = ce.detail?.path;
      if (savedPath) {
        // 立即清除已保存文件的 git 状态（先显示白色），
        // 解决 autocrlf 导致 git2 始终报告 modified 的场景
        setGitStatusMap((prev) => {
          const next = new Map(prev);
          next.delete(savedPath);
          return next;
        });
      }
      refreshExpanded();
    };
    window.addEventListener("slterm:file-saved", handler);
    return () => window.removeEventListener("slterm:file-saved", handler);
  }, [refreshExpanded]);

  return {
    rootNodes,
    gitStatusMap,
    toggleExpand,
    refresh: refreshExpanded,
    fullRefresh,
  };
}
