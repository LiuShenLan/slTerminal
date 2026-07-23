// activeExplorer.ts — 模块级"当前聚焦文件浏览器"指针
//
// 解决 explorer 快捷键命令 id（explorer.delete/open/rename）的派发问题：
// 命令在注册表只注册一次，handler 经此指针派发到**当前聚焦**的 explorer 实例。
// Explorer 容器 focusin 时 setActiveExplorer(自身 actions)，focusout 时 clearActiveExplorer（仅在匹配时）。

import { createActivePointer } from "../../lib/activePointer";

/** 聚焦文件浏览器对外暴露的动作（供快捷键命令 handler 调用） */
export interface ExplorerActions {
  /** 获取当前选中的文件路径（null = 无选中） */
  getSelectedPath: () => string | null;
  /** 删除选中项（含确认弹窗） */
  deleteSelected: () => Promise<void>;
  /** 打开选中项（文件→编辑器，文件夹→展开/折叠） */
  openSelected: () => void;
  /** 进入内联重命名模式 */
  renameSelected: () => void;
  /** 是否正在重命名（用于 Enter/F2 透传判断） */
  isRenaming: () => boolean;
}

const ptr = createActivePointer<ExplorerActions>();

/** 设置当前聚焦 explorer */
export const setActiveExplorer = ptr.setActive;

/** 清除当前聚焦 explorer——仅当传入的 actions 正是当前 active 时才清（防竞态） */
export const clearActiveExplorer = ptr.clearActive;

/** 获取当前聚焦 explorer 动作（无聚焦 explorer 时为 null） */
export const getActiveExplorer = ptr.getActive;
