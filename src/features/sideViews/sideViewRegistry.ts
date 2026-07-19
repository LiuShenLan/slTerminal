// SideViewRegistry — 侧栏视图注册表
//
// 模块级单例，管理侧栏视图定义。
// ActivityBar 通过此注册表渲染按钮，SideBarArea 通过它渲染视图槽。
// 同 TabTitleRegistry 模式——register/get/getAll/_reset。
//
// 新增侧栏视图只需实现组件 + 一行 register()，框架自动处理：
//   按钮渲染与开关、上区/下区拖拽归属、槽位展示、持久化。

import type React from "react";

/** 侧栏视图组件的 props——与 SidebarTree props 精确匹配 */
export interface SideViewComponentProps {
  /** 切换到指定操作页面 */
  switchToPage: (projectId: string, pageId: string) => void;
  /** 删除指定操作页面 */
  onDeletePage: (projectId: string, pageId: string) => void;
}

/** 侧栏视图定义 */
export interface SideViewDef {
  /** 唯一 id（持久化引用） */
  id: string;
  /** 视图名称（tooltip / 无障碍名称） */
  title: string;
  /** 活动栏按钮图标（emoji） */
  icon: string;
  /** 视图 React 组件 */
  component: React.ComponentType<SideViewComponentProps>;
}

/** 侧栏视图注册表——模块级单例 */
export class SideViewRegistry {
  private defs: Map<string, SideViewDef> = new Map();

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

  /** 清空所有定义（仅测试用） */
  _reset(): void {
    this.defs.clear();
  }
}

/** 全局单例 */
export const sideViewRegistry = new SideViewRegistry();
