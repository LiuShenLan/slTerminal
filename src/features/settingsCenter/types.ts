// types.ts —— 设置中心公共类型（F11；ADR-0023 全局/Agent 二分）
//
// SettingsPageGroup：配置页分组——global=应用级单例 / agent=CLI 专属（页 id 形态
//   `agent.<cliId>.<page>`，cliId 字段承载归属；ADR-0023 起「项目」组删除）。
// SettingsPageProps：壳透传给配置页的 props——dirty 上报 + 页内状态持久化通道
//   （壳是 params 持久化单点，页内不直接碰 Dockview API）。
// SettingsPage：配置页注册项（SettingsPageRegistry 登记条目）。

import type React from "react";

/** 配置页分组（ADR-0023）：global=应用级单例 / agent=CLI 专属 */
export type SettingsPageGroup = "global" | "agent";
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
  /** agent 组专属：所属 CLI profile id（导航按 agent 分节标题 + 缩进子页渲染） */
  cliId?: string;
  component: React.FC<SettingsPageProps>;
  order?: number;
}
