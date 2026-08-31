// commitContextMenu.ts — commit view 右键菜单策略注册表
//
// 策略模式：git 状态 → 右键菜单项（照 openCommitFile.ts 的 STATUS_PANEL_MAP 模式）。
// 职责：
// - 声明哪些状态对应哪些菜单项（ROLLBACK_STATES / DELETE_STATES 集合）
// - 构造菜单项的 action 闭包（confirmDialog 确认 → IPC 调用 → refresh）
//
// 新增菜单类型只需在本文件追加新 Set + 新构造分支，UI 组件零改动。

import { confirmDialog } from "../../lib";
import { basename } from "../../lib/path";
import { gitRollback, gitUnstage } from "../../ipc/git";
import { deleteEntry } from "../../ipc/fs";
import type { GitStatusEntry } from "../../types/git";

/** 右键菜单项 */
export interface CommitMenuItem {
  label: string;
  /** 危险项（回滚/删除类）——菜单渲染 ERROR_FG 着色（UI-802） */
  danger?: boolean;
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
      danger: true, // 不可撤销操作——危险项着色（UI-802）
      action: async () => {
        const ok = await confirmDialog({
          title: "确认回滚",
          message: `确定回滚"${name}" 到 HEAD 版本？此操作不可撤销。`,
          danger: true,
        });
        if (!ok) return;
        try {
          // renamed：HEAD 侧文件位于旧路径（git status 语义 path=当前路径）。
          // 若传 entry.path（现为新路径），git_rollback_impl 会报「HEAD 中不存在」——
          // 回滚须以 oldPath 定位 HEAD 内容（等价 `git checkout HEAD -- <旧路径>`；
          // 新路径文件残留为 untracked，与 git CLI 行为一致）。
          const rollbackPath =
            entry.status === "renamed" && entry.oldPath ? entry.oldPath : entry.path;
          await gitRollback(rootPath, rollbackPath);
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
      danger: true, // 不可撤销操作——危险项着色（UI-802）
      action: async () => {
        const ok = await confirmDialog({
          title: "确认删除",
          message: `确定删除"${name}"？此操作不可撤销。`,
          danger: true,
        });
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
