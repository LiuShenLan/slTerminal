// 全局 Window 扩展类型声明
// 集中管理 window 上的自定义属性，消除代码中的 (window as any) 用法

import type { DockviewApi } from "dockview-react";

declare global {
  interface Window {
    /** Tauri IPC internals（WebView2 异步注入） */
    __TAURI_INTERNALS__?: unknown;

    /** 错误边界捕获的错误信息（ErrorBoundary.componentDidCatch 写入） */
    __sltermError?: {
      message: string;
      stack?: string;
      componentStack?: string | null;
    };

    /** Dockview 布局 API（活跃页面实例） */
    __dockviewApi?: DockviewApi;

    // ── E2E 辅助（仅在 DEV 或 VITE_E2E=1 时挂载） ──

    /** Workspace 就绪标志 */
    __slterm_e2e_workspaceReady?: boolean;
    /** 项目创建进行中标志（阻止 localStorage 恢复覆盖） */
    __slterm_e2e_projectPending?: boolean;
    /** 写入剪贴板 */
    __slterm_e2e_writeClipboard?: (text: string) => Promise<void>;
    /** 快捷键诊断 */
    __slterm_e2e_shortcutDebug?: () => { stack: string[]; commands: string[] };
    /** 程序化创建测试项目 */
    __slterm_e2e_createProject?: (dirPath: string) => string;
    /** 反查 pageId 所属 projectId */
    __slterm_e2e_getProjectIdForPage?: (pageId: string) => string | null;
    /** 在已有项目中新增操作页面 */
    __slterm_e2e_addPage?: (projectId: string, name: string, rootPath: string) => string;
    /** 切换活跃页面 */
    __slterm_e2e_switchToPage?: (pageId: string) => void;
    /** 注册编辑器并重算标题 */
    __slterm_e2e_registerAndRecompute?: (
      pageId: string,
      rootPath: string,
      panelId: string,
      filePath?: string,
    ) => void;
    /** 获取活跃页面信息 */
    __slterm_e2e_getActivePageInfo?: () => { pageId: string; rootPath: string } | null;
  }
}

export {};
