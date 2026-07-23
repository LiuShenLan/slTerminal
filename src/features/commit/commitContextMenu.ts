// commitContextMenu.ts — commit view 右键菜单策略注册表
//
// 策略模式：git 状态 → 右键菜单项（照 openCommitFile.ts 的 STATUS_PANEL_MAP 模式）。
// 职责：
// - 声明哪些状态对应哪些菜单项（ROLLBACK_STATES / DELETE_STATES 集合）
// - 构造菜单项的 action 闭包（ask 确认 → IPC 调用 → refresh）
//
// 新增菜单类型只需在本文件追加新 Set + 新构造分支，UI 组件零改动。

import { basename } from "../../lib/path";
import { ask } from "../../ipc/dialog";
import { gitRollback, gitUnstage } from "../../ipc/git";
import { deleteEntry } from "../../ipc/fs";
import type { GitStatusEntry } from "../../types/git";

/** 右键菜单项 */
export interface CommitMenuItem {
  label: string;
  action: () => Promise<void>;
}

/** 回滚菜单适用的 git 状态 */
const ROLLBACK_STATES = new Set([
  "modified",
  "deleted",
  "renamed",
  "conflict",
]);

/** 删除菜单适用的 git 状态 */
const DELETE_STATES = new Set(["added", "untracked"]);

/**
 * 根据 git 状态返回右键菜单项（策略查询）。
 *
 * @param entry   文件状态条目
 * @param rootPath 项目根路径（用于 IPC 调用）
 * @param onRefresh 操作完成后的列表刷新回调
 * @returns 菜单项数组，无适用菜单时返回 []
 */
export function getContextMenuItems(
  entry: GitStatusEntry,
  rootPath: string,
  onRefresh: () => void,
): CommitMenuItem[] {
  const items: CommitMenuItem[] = [];
  const name = basename(entry.path);

  if (ROLLBACK_STATES.has(entry.status)) {
    items.push({
      label: "回滚",
      action: async () => {
        const ok = await ask(
          `确定回滚"${name}" 到 HEAD 版本？此操作不可撤销。`,
          { title: "确认回滚", kind: "warning" },
        );
        if (!ok) return;
        try {
          await gitRollback(rootPath, entry.path);
          onRefresh();
        } catch (err) {
          console.error("[slTerminal] 回滚文件失败:", entry.path, err);
        }
      },
    });
  }

  if (DELETE_STATES.has(entry.status)) {
    items.push({
      label: "删除",
      action: async () => {
        const ok = await ask(
          `确定删除"${name}"？此操作不可撤销。`,
          { title: "确认删除", kind: "warning" },
        );
        if (!ok) return;
        try {
          // added（staged 新文件）：先取消暂存，再删除磁盘
          if (entry.status === "added") {
            await gitUnstage(rootPath, entry.path);
          }
          await deleteEntry(entry.path);
          onRefresh();
        } catch (err) {
          console.error("[slTerminal] 删除文件失败:", entry.path, err);
        }
      },
    });
  }

  return items;
}
