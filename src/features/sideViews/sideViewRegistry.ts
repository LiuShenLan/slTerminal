// SideViewRegistry — 侧栏视图注册表
//
// 模块级单例，管理侧栏视图定义。
// ActivityBar 通过此注册表渲染按钮，SideBarArea 通过它渲染视图槽。
// 同 CliProfileRegistry 模式——register/get/getAll/_reset。
//
// 新增侧栏视图只需实现组件 + 一行 register()，框架自动处理：
//   按钮渲染与开关、上区/下区拖拽归属、槽位展示、持久化。

import type React from "react";
import type { IconProps } from "../../lib/icons";

/** 侧栏视图组件的 props——与 SidebarTree props 精确匹配 */
export interface SideViewComponentProps {
  /** 切换到指定操作页面（async——切换完成后再开面板） */
  switchToPage: (projectId: string, pageId: string) => Promise<void>;
  /** 删除指定操作页面 */
  onDeletePage: (projectId: string, pageId: string) => void;
  /** 视图恢复状态（CP-016：槽位切换/换区重建后由注册表状态槽回填；无历史状态则 undefined） */
  viewState?: unknown;
  /** 视图状态上呼（组件内部状态变化时持久化；模块级存活，跨挂载不丢） */
  onViewStateChange?: (state: unknown) => void;
}

/** 侧栏视图定义 */
export interface SideViewDef {
  /** 唯一 id（持久化引用） */
  id: string;
  /** 视图名称（tooltip / 无障碍名称） */
  title: string;
  /** 活动栏按钮图标（icons.tsx 组件；15px 默认，currentColor 跟随按钮色——IC-06） */
  icon: React.ComponentType<IconProps>;
  /** 视图 React 组件 */
  component: React.ComponentType<SideViewComponentProps>;
}

/** 侧栏视图注册表——模块级单例 */
export class SideViewRegistry {
  private defs: Map<string, SideViewDef> = new Map();
  /** 视图状态槽（CP-016）：以视图 id 为键持有跨挂载状态——与 defs 同生命周期（模块级） */
  private viewStates: Map<string, unknown> = new Map();

  /** 注册一条侧栏视图定义（同 id 覆盖） */
  register(def: SideViewDef): void {
    this.defs.set(def.id, def);
  }

  /** 返回所有已注册定义（注册序） */
  getAll(): SideViewDef[] {
    return Array.from(this.defs.values());
  }

  /** 按 id 查询定义，未注册返回 undefined */
  get(id: string): SideViewDef | undefined {
    return this.defs.get(id);
  }

  /** 读取视图恢复状态（无条目返回 undefined） */
  getViewState<T>(id: string): T | undefined {
    return this.viewStates.get(id) as T | undefined;
  }

  /** 写入视图状态（组件经 onViewStateChange 上呼；同 id 覆盖） */
  setViewState(id: string, state: unknown): void {
    this.viewStates.set(id, state);
  }

  /** 清空所有定义 + 视图状态（仅测试用） */
  _reset(): void {
    this.defs.clear();
    this.viewStates.clear();
  }
}

/** 全局单例 */
export const sideViewRegistry = new SideViewRegistry();
