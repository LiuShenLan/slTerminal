// useCommitStatus.ts — commit 视图数据加载 hook
//
// 职责：
// - 从 activePageId 推导 rootPath（照 ExplorerPanel 模式）
// - gitStatus(rootPath) 加载一次
// - onFsEvent + 200ms debounce 自动刷新
// - rootPath 变化立即清空旧数据 + 重载（generation 取消模式）

import { useState, useEffect, useRef, useCallback } from "react";
import { onFsEvent } from "../../ipc/notify";
import { gitStatus } from "../../ipc/git";
import { useProjects } from "../../stores/projects";
import { useLayout } from "../../stores/layout";
import type { GitStatusEntry } from "../../types/git";

/** 文件系统事件去抖延迟（ms） */
const FS_EVENT_DEBOUNCE_MS = 200;

/** 加载状态机 */
export type CommitLoadState =
  | { kind: "no-root" }
  | { kind: "loading" }
  | { kind: "error" }
  | { kind: "ready"; entries: GitStatusEntry[] };

export function useCommitStatus() {
  const projects = useProjects((s) => s.projects);
  const activePageId = useLayout((s) => s.activePageId);

  // 推导 rootPath（照 ExplorerPanel）
  let rootPath: string | null = null;
  if (activePageId) {
    for (const [, proj] of Object.entries(projects)) {
      const activePage = proj.pages.find((p) => p.pageId === activePageId);
      if (activePage) {
        rootPath = activePage.cwd || proj.rootPath;
        break;
      }
    }
  }

  const [state, setState] = useState<CommitLoadState>({ kind: "no-root" });
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const rootPathRef = useRef<string | null>(rootPath);
  rootPathRef.current = rootPath;
  const genRef = useRef(0);

  /** 加载 git status */
  const loadStatus = useCallback(async (gen?: number) => {
    const rp = rootPathRef.current;
    if (!rp) {
      if (gen === undefined || gen === genRef.current) {
        setState({ kind: "no-root" });
      }
      return;
    }

    try {
      const statuses = await gitStatus(rp);
      if (gen !== undefined && gen !== genRef.current) return;
      setState({ kind: "ready", entries: statuses });
    } catch (err) {
      console.error("[slTerminal] gitStatus 失败:", rp, err);
      if (gen !== undefined && gen !== genRef.current) return;
      setState({ kind: "error" });
    }
  }, []);

  // rootPath 变化时清空旧数据 + 重载
  useEffect(() => {
    const gen = ++genRef.current;
    if (!rootPath) {
      setState({ kind: "no-root" });
      return;
    }
    setState({ kind: "loading" });
    loadStatus(gen);
  }, [rootPath, loadStatus]);

  // 订阅文件系统事件（200ms 去抖刷新）
  useEffect(() => {
    const unlisten = onFsEvent(() => {
      const rp = rootPathRef.current;
      if (!rp) return;

      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      debounceRef.current = setTimeout(() => {
        const gen = ++genRef.current;
        loadStatus(gen);
      }, FS_EVENT_DEBOUNCE_MS);
    });

    return () => {
      unlisten();
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [loadStatus]);

  return { state, rootPath };
}
