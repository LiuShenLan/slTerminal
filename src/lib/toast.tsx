// toast —— 全局右上角通知（UI-804）
//
// 契约（Stage 07 跨边界写死）：toast.show(type: "success"|"warning"|"error", message: string): void
//   - 视觉规格：右上堆叠；单条 = 语义色 12% 底 + 1px 语义描边 + fg-1 文字 + 圆角 8
//   - 自动消失时长（执行期定值）：success 3s / warning 4s / error 5s
//
// 用法：调用方 import { toast } from "../lib" 即可；
// 挂载点：ToastHost 由 App.tsx 根部置入（本文件与调用点解耦）。
// 硬约束 #1：本组件为纯 UI，不涉及 OS/文件/进程调用。

import { useEffect, useReducer } from "react";
import {
  SIDEBAR_FG,
  ERROR_FG,
  AGENT_STATUS_USAGE_COLORS,
  GIT_FILE_COLORS,
} from "../theme";

/** 通知类型（契约见文件头） */
export type ToastType = "success" | "warning" | "error";

/** 单条通知（模块级队列——右上角堆叠） */
interface ToastItem {
  id: number;
  type: ToastType;
  message: string;
}

let items: ToastItem[] = [];
let nextId = 1;
const listeners = new Set<() => void>();

// 语义色单点映射：success→用量低档绿、warning→git 修改黄、error→错误红
const TOAST_COLORS: Record<ToastType, string> = {
  success: AGENT_STATUS_USAGE_COLORS.low,
  warning: GIT_FILE_COLORS.modified,
  error: ERROR_FG,
};

// 自动消失时长（执行期定值）：success 3s / warning 4s / error 5s
const TOAST_DURATION_MS: Record<ToastType, number> = {
  success: 3000,
  warning: 4000,
  error: 5000,
};

/** #rrggbb → rgba(r,g,b,alpha)；toast 底色按语义色 12% 派生 */
function withAlpha(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

function notifyListeners(): void {
  listeners.forEach((l) => l());
}

function dismiss(id: number): void {
  items = items.filter((t) => t.id !== id);
  notifyListeners();
}

export const toast = {
  /** 弹出右上角通知，按类型自动消失（success 3s / warning 4s / error 5s） */
  show(type: ToastType, message: string): void {
    const id = nextId++;
    items = [...items, { id, type, message }];
    notifyListeners();
    setTimeout(() => dismiss(id), TOAST_DURATION_MS[type]);
  },
  /** 仅测试：清空全部条目 */
  _reset(): void {
    items = [];
    notifyListeners();
  },
};

/** 挂载点组件——置入 App.tsx 根部；无通知时渲染 null */
export function ToastHost() {
  const [, setVersion] = useReducer((v: number) => v + 1, 0);

  // 订阅模块级队列变更（imperative API → React 渲染桥）
  useEffect(() => {
    listeners.add(setVersion);
    return () => {
      listeners.delete(setVersion);
    };
  }, []);

  if (items.length === 0) return null;

  return (
    <div
      data-e2e="toast-container"
      role="status"
      aria-live="polite"
      style={{
        position: "fixed",
        top: 16,
        right: 16,
        display: "flex",
        flexDirection: "column",
        alignItems: "flex-end",
        gap: 8,
        zIndex: 1200,
      }}
    >
      {items.map((t) => (
        <div
          key={t.id}
          data-e2e={`toast-${t.type}`}
          style={{
            background: withAlpha(TOAST_COLORS[t.type], 0.12), // 语义色 12% 底（派生自语义色）
            border: `1px solid ${TOAST_COLORS[t.type]}`, // 1px 语义描边
            color: SIDEBAR_FG, // fg-1 文字
            borderRadius: 8,
            padding: "8px 12px",
            fontSize: 13,
            maxWidth: 320,
          }}
        >
          {t.message}
        </div>
      ))}
    </div>
  );
}
