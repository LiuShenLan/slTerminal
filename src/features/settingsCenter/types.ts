// types.ts —— 设置中心公共类型（F11）
//
// SettingsPageGroup：配置页分组——global=应用级单例 / project=需项目上下文。
// SettingsPageProps：壳透传给配置页的 props——dirty 上报 + 页内状态持久化通道
//   （壳是 params 持久化单点，页内不直接碰 Dockview API）。
// SettingsPage：配置页注册项（SettingsPageRegistry 登记条目）。

import type React from "react";

/** 配置页分组（F11）：global=应用级单例 / project=需项目上下文 */
export type SettingsPageGroup = "global" | "project";
/** 壳透传给配置页的 props——dirty 上报 + 页内状态持久化通道（壳是 params 持久化单点） */
export interface SettingsPageProps {
  onDirtyChange?: (dirty: boolean) => void;
  pageParams?: Record<string, unknown>;
  /** 约定：组件 mount/首渲染期禁止调用（仅响应用户交互调用）——mount 期调用会误触发布局保存 */
  onPageParamsChange?: (patch: Record<string, unknown>) => void;
}
/** 配置页注册项 */
export interface SettingsPage {
  id: string;
  title: string;
  group: SettingsPageGroup;
  component: React.FC<SettingsPageProps>;
  order?: number;
}
