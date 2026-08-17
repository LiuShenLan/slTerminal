// icons.tsx —— lucide-react 图标集中封装（IC-01 / UI-601 单点）
//
// 本应用所有 lucide 图标只允许从本文件引用，禁止在其他文件直接 import lucide-react（单点）。
// 统一规格：15px、1.5px 描边、currentColor（颜色跟随上下文）；
// 紧凑处允许调用方传 size 12-13（如树箭头、标题栏按钮）。
//
// lucide 选型对照（执行期实查 lucide-react 导出确认）：
//   导航 IconNav        = FolderTree
//   文件 IconFiles      = Folder
//   提交 IconCommit     = GitBranch
//   配置 IconConfig     = Settings 齿轮
//   时钟 IconHistory    = Clock
//   空态 IconEmptyBox   = FolderOpen（空态文件夹）
//   告警 IconAlertTriangle = TriangleAlert（lucide 2.x 实际导出名，旧别名 AlertTriangle 已移除）

import type { LucideIcon } from "lucide-react";
import {
  ChevronDown,
  ChevronRight,
  Clock,
  FileText,
  Folder,
  FolderOpen,
  FolderTree,
  GitBranch,
  Minus,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Square,
  TriangleAlert,
  X,
  XSquare,
} from "lucide-react";

/** 图标统一 props：size 默认 15（紧凑处传 12-13）；描边固定 1.5；颜色恒 currentColor */
export interface IconProps {
  size?: number;
  className?: string;
}

// 工厂：统一注入默认规格（15px / strokeWidth 1.5 / 装饰性图标隐屏）
function makeIcon(Comp: LucideIcon) {
  return function Icon({ size = 15, className }: IconProps) {
    return (
      <Comp size={size} strokeWidth={1.5} className={className} aria-hidden="true" />
    );
  };
}

export const IconNav = makeIcon(FolderTree); // 导航树（活动栏导航视图）
export const IconFiles = makeIcon(Folder); // 文件浏览器（活动栏文件视图）
export const IconCommit = makeIcon(GitBranch); // 提交（活动栏 commit 视图）
export const IconConfig = makeIcon(Settings); // 配置（活动栏底部「配置」钮）
export const IconChevronRight = makeIcon(ChevronRight); // 树折叠节点展开箭头（12px 用法）
export const IconChevronDown = makeIcon(ChevronDown); // 树展开节点收起箭头（12px 用法）
export const IconRefresh = makeIcon(RefreshCw); // 刷新
export const IconSearch = makeIcon(Search); // 搜索
export const IconHistory = makeIcon(Clock); // 历史会话
export const IconPage = makeIcon(FileText); // 操作页面行图标（与 IconHistory 时钟区分）
export const IconClose = makeIcon(X); // 关闭 ×（页签/横幅/浮层）
export const IconMin = makeIcon(Minus); // 窗口最小化（自绘标题栏）
export const IconMax = makeIcon(Square); // 窗口最大化（自绘标题栏）
export const IconCloseWin = makeIcon(XSquare); // 窗口关闭（自绘标题栏，与普通关闭区分）
export const IconPlus = makeIcon(Plus); // 新建（页签栏「+」钮）
export const IconFolder = makeIcon(Folder); // 文件夹通用
export const IconEmptyBox = makeIcon(FolderOpen); // 空态（空文件树/无搜索结果等）
export const IconAlertTriangle = makeIcon(TriangleAlert); // 大文件警告（gitshow 行首图标，13px 用法）
