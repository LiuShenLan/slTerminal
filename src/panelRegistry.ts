// panelRegistry — 面板类型注册表
//
// 面板只能是此处注册过的类型（硬约束 #5）。
// 新增面板类型 = 加目录 + 在此注册。

import React from "react";
import { TerminalPanel } from "./panels/terminal";
import { EditorPanel } from "./panels/editor";
import { HtmlPanel } from "./panels/html";
import { GitShowPanel } from "./panels/gitshow";
import { DiffPanel } from "./panels/diff";
import { SettingsPanel } from "./panels/settings";
import { ErrorBoundary } from "./lib";

/** 终端面板类型标识 */
export const PANEL_TERMINAL = "terminal" as const;
/** 编辑器面板类型标识 */
export const PANEL_EDITOR = "editor" as const;
/** HTML 预览面板类型标识 */
export const PANEL_HTML_VIEWER = "htmlviewer" as const;
// PANEL_GIT_SHOW/PANEL_DIFF/PANEL_HOOKS_CONFIG 已删除（FE-35）——
// 全仓零外部消费（grep 无 import），内部 PANEL_TYPES/FILE_PANEL_TYPES 改字面量。

/**
 * 面板级错误边界 HOC（FE-22）
 *
 * 单点包裹：面板渲染错误降级为 inline 占位，不再扩大为整页崩溃，
 * 同页其他面板（同 Dockview 实例内兄弟组件）不受影响。
 * 模块加载时一次性生成包裹组件，身份稳定——Dockview 不会因包裹重建面板。
 * 导出供 L2 测试直接构造抛错/正常面板验证边界隔离。
 */
export function withPanelBoundary<P extends object>(Component: React.FC<P>): React.FC<P> {
  // 本文件为 .ts（非 .tsx），用 createElement 而非 JSX；children 经 props 显式传入
  const Wrapped: React.FC<P> = (props) =>
    React.createElement(ErrorBoundary, {
      variant: "inline",
      children: React.createElement(Component, props),
    });
  // FE-22: displayName 改惰性 getter——模块顶层立即读取 Component.displayName 会在
  // 循环依赖（SettingsPanel → layoutSerde → panelRegistry → SettingsPanel TDZ）中
  // 抛 ReferenceError 致三个 suite 加载崩溃；getter 在访问时（渲染/devtools/测试断言，
  // 此时全部模块已加载完成）才读取，TDZ 安全且 displayName 前缀断言（/^Boundary\(/）不变
  Object.defineProperty(Wrapped, "displayName", {
    configurable: true,
    get: () => `Boundary(${Component.displayName ?? Component.name ?? "Panel"})`,
  });
  return Wrapped;
}

/** Dockview 面板组件注册表（FE-22：全部经 withPanelBoundary 单点包裹错误边界） */
export const panelRegistry = {
  terminal: withPanelBoundary(TerminalPanel as React.FC<{
    params: { panelId: string; cwd?: string };
  }>),
  editor: withPanelBoundary(EditorPanel as React.FC<{
    params: { panelId: string; filePath?: string; cwd?: string };
  }>),
  htmlviewer: withPanelBoundary(HtmlPanel as React.FC<{
    params: { panelId: string; filePath?: string };
  }>),
  gitshow: withPanelBoundary(GitShowPanel as React.FC<{
    params: { panelId: string; filePath: string; oldPath?: string; repoPath: string };
  }>),
  diff: withPanelBoundary(DiffPanel as React.FC<{
    params: { panelId: string; filePath: string; oldPath?: string; repoPath: string };
  }>),
  settings: withPanelBoundary(SettingsPanel as React.FC<{
    params: { panelId: string };
  }>),
};

/** 面板类型列表（用于 fromJSON 校验白名单） */
export const PANEL_TYPES = [
  PANEL_TERMINAL,
  PANEL_EDITOR,
  PANEL_HTML_VIEWER,
  "gitshow",
  "diff",
  "settings",
] as const;
export type PanelType = (typeof PANEL_TYPES)[number];

/** 文件型面板类型集合——有 filePath、参与标题计算的面板 */
export const FILE_PANEL_TYPES: ReadonlySet<string> = new Set([
  PANEL_EDITOR,
  PANEL_HTML_VIEWER,
  "gitshow",
  "diff",
]);

/** 检查面板类型是否有效 */
export function isValidPanelType(type: string): type is PanelType {
  return PANEL_TYPES.includes(type as PanelType);
}

/**
 * 检查面板是否应使用 renderer="always" 模式。
 * 显式白名单：terminal（保持 PTY 存活）+ htmlviewer（避免 iframe browsing context 销毁重建导致白屏）。
 * editor / gitshow / diff 故意排除——CM6 重建无视觉闪屏，且大文件编辑器若始终挂载会显著增加内存开销。
 */
export function isAlwaysRenderPanel(type: string): boolean {
  return type === PANEL_TERMINAL || type === PANEL_HTML_VIEWER;
}
