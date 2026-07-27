// useClaudeNotifications — F4 通知调度核心
//
// 订阅 hook-event 事件流，在窗口失焦时触发桌面 toast 通知。
// 三类事件映射：权限请求 / 任务完成 / 错误。
// toast 点击后聚焦窗口并路由到对应终端面板。
//
// P2-FE-05: 失焦门控 + 三类事件 toast + 任务栏闪烁
// P2-FE-06: toast 点击路由（focus → switchToPage → focus panel）

import { useEffect, useRef } from "react";
import { onHookEvent, type HookEventPayload } from "../../ipc/hooks";
import {
  ensureNotificationPermission,
  sendClickableNotification,
} from "../../ipc/notification";
import {
  requestUserAttention,
  setFocus,
  UserAttentionType,
} from "../../ipc/window";
import { useProjects } from "../../stores/projects";
import { switchToPageAndFocus, getPageApi } from "../../workspace/pageApis";

/** 通知事件类别 */
type NotifyCategory = "permission" | "done" | "error";

/** 类别 → emoji 映射 */
const CATEGORY_EMOJI: Record<NotifyCategory, string> = {
  permission: "🔐",
  done: "✅",
  error: "❌",
};

/** 类别 → 中文标签 */
const CATEGORY_LABEL: Record<NotifyCategory, string> = {
  permission: "权限请求",
  done: "任务完成",
  error: "错误",
};

/**
 * 根据 hook-event payload 判断通知类别
 *
 * 规则（优先级自上而下）：
 *   - 权限请求：event === "PermissionRequest" 或 (event === "Notification" 且 notificationType === "permission_prompt")
 *   - 错误：event === "StopFailure" 或 "PostToolUseFailure"
 *   - 任务完成：event === "Stop"
 *   - 其他：不触发通知
 */
function classifyEvent(payload: HookEventPayload): NotifyCategory | null {
  // 权限请求
  if (payload.event === "PermissionRequest") return "permission";
  if (
    payload.event === "Notification" &&
    payload.notificationType === "permission_prompt"
  )
    return "permission";

  // 错误
  if (payload.event === "StopFailure" || payload.event === "PostToolUseFailure")
    return "error";

  // 任务完成
  if (payload.event === "Stop") return "done";

  // 其他事件（PreToolUse / PostToolUse / SessionStart / SessionEnd 等）不触发 toast
  return null;
}

/**
 * 从终端 panelId 解析 pageId
 *
 * 格式：terminal-{pageId}-{seq}（如 terminal-p1-0）
 * 按 "-" 分割取中间段，支持 pageId 本身含 "-"
 */
function parsePageId(panelId: string): string | null {
  const parts = panelId.split("-");
  if (parts.length < 3 || parts[0] !== "terminal") return null;
  return parts.slice(1, -1).join("-");
}

/**
 * 根据 pageId 查找所属 projectId
 */
function findProjectIdForPage(pageId: string): string | null {
  const { projects } = useProjects.getState();
  for (const [projId, proj] of Object.entries(projects)) {
    if (proj.pages.some((p) => p.pageId === pageId)) {
      return projId;
    }
  }
  return null;
}

/**
 * 根据 panelId 查找面板当前页签标题
 *
 * 经 pageApis 跨页面查 panel 的 title——不再依赖 __dockviewApi 恰好指向目标页。
 */
function findPanelTitle(panelId: string): string {
  try {
    const pageId = parsePageId(panelId);
    if (!pageId) return panelId;
    const api = getPageApi(pageId);
    if (!api) return panelId;
    const panel = api.getPanel(panelId);
    if (!panel) return panelId;
    return panel.title ?? panelId;
  } catch {
    return panelId;
  }
}

/**
 * Toast 点击路由：解析 pageId → 委托共享函数切换页面并聚焦面板
 */
async function routeToPanel(panelId: string): Promise<void> {
  const pageId = parsePageId(panelId);
  if (!pageId) return;
  await switchToPageAndFocus(pageId, panelId);
}

/**
 * 发送关注态任务栏闪烁
 *
 * Windows 上 UserAttentionType.Critical = FLASHW_TIMERNOFG，
 * 持续闪烁直到窗口获得焦点。
 */
function flashTaskbar(): void {
  requestUserAttention(UserAttentionType.Critical).catch(() => {
    // 非关键——静默失败
  });
}

/** 使用 ref 避免 effect 重复注册 */
let permissionEnsured = false;

/**
 * F4 通知调度 hook
 *
 * 在 App.tsx 挂载的 NotificationListener 组件中调用。
 * 订阅 onHookEvent，在窗口失焦时按事件类别触发通知。
 */
export function useClaudeNotifications(): void {
  // 用于防止同一事件重复通知的去重 set（基于 sessionId + event + timestamp）
  const seenRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    // 懒初始化：首次调用时确保通知权限（仅一次）
    if (!permissionEnsured) {
      permissionEnsured = true;
      ensureNotificationPermission().catch(() => {
        // 权限被拒，后续 sendClickableNotification 会走回退路径
      });
    }

    const unlisten = onHookEvent((payload) => {
      // 门控：窗口聚焦时不触发通知
      if (window.__slterm_windowFocused !== false) return;

      // 事件分类
      const category = classifyEvent(payload);
      if (!category) return;

      // 去重：同一信号文件重复投递去重（sessionId+event+timestamp 键）+ 缓存超 200 条截断保留最近 100 条
      const dedupKey = `${payload.sessionId}|${payload.event}|${payload.timestamp}`;
      if (seenRef.current.has(dedupKey)) return;
      seenRef.current.add(dedupKey);

      // 限制去重缓存大小（保留最近 200 条）
      if (seenRef.current.size > 200) {
        const entries = Array.from(seenRef.current);
        seenRef.current = new Set(entries.slice(-100));
      }

      // 获取面板标题
      const panelTitle = findPanelTitle(payload.panelId);

      // 获取项目名：从 panelId 反查
      const pageId = parsePageId(payload.panelId);
      let projectName = "";
      if (pageId) {
        const projectId = findProjectIdForPage(pageId);
        if (projectId) {
          const { projects } = useProjects.getState();
          projectName = projects[projectId]?.name ?? "";
        }
      }

      // 构建 toast 正文：<项目名> · <页签标题> · <事件类别> · <时间>
      const timeStr = new Date().toLocaleTimeString();
      const emoji = CATEGORY_EMOJI[category];
      const label = CATEGORY_LABEL[category];
      const bodyParts = [projectName, panelTitle, `${emoji} ${label}`, timeStr]
        .filter(Boolean)
        .join(" · ");

      // 权限请求类：闪烁任务栏
      if (category === "permission") {
        flashTaskbar();
      }

      // 发送可点击 toast
      sendClickableNotification("slTerminal", bodyParts, () => {
        // 聚焦窗口（也会自动停止任务栏闪烁）
        setFocus().catch(() => {});
        // 路由到对应面板
        routeToPanel(payload.panelId).catch(() => {});
      });
    });

    return unlisten;
  }, []);
}

/**
 * 通知监听器组件
 *
 * 在 App.tsx 中挂载一次，内部调用 useClaudeNotifications()。
 * 无 UI 输出——纯副作用组件。
 */
export function NotificationListener(): null {
  useClaudeNotifications();
  return null;
}
