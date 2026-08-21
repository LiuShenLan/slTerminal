// useAgentNotifications — F4 通知调度核心（MC-420 更名）
//
// 订阅 agent-event 事件流，在窗口失焦时触发桌面 toast 通知 + 任务栏闪烁。
// 通用门控（失焦/去重/seenRef 截断）CLI 无关保留本模块；通知类别判定委托
// profile.capabilities.hooks.classifyNotification（claude 五映射已迁入
// profiles/claude/strategies.ts，MC-422）。
// toast 已失去点击路由能力（sendToastNotification 无 onclick）——
// 任务栏闪烁是唯一的回窗引导通道，三类事件全覆盖。
//
// P2-FE-05: 失焦门控 + 三类事件 toast + 任务栏闪烁
// P2-FE-09: 去路由化——删 routeToPanel/findPanelTitle/onClick 绑定，三类事件均闪烁

import { useEffect, useRef } from "react";
import { onAgentEvent } from "../../ipc/agentHooks";
import type { AgentEventPayload } from "../../types/agent";
import {
  ensureNotificationPermission,
  sendToastNotification,
} from "../../ipc/notification";
import {
  requestUserAttention,
  UserAttentionType,
} from "../../ipc/window";
import { useProjects } from "../../stores/projects";
import { parseTerminalPageId } from "../../lib/panelId";
import { cliProfileRegistry } from "../cliProfiles";
// ZQ-2: 来源 CLI 标识三级解析单点（契约 4）——空串/空白 cliId 同等回退
// （TerminalRegistry 不 import notifications——classifyEvent 纯函数导入无循环）
import { resolvePayloadCliId } from "../../panels/terminal/resolvePayloadCliId";

/** 通知事件类别 */
export type NotifyCategory = "permission" | "done" | "error";

/** 类别 → 中文标签（IC-07：通知标题纯文本——装饰 emoji 字符不进入通知正文） */
const CATEGORY_LABEL: Record<NotifyCategory, string> = {
  permission: "权限请求",
  done: "任务完成",
  error: "错误",
};

/**
 * 判定 agent-event payload 的通知类别（纯函数，MC-420 两段分解）
 * 导出为测试专用（notifications.test.ts 直测分类表与 MC-420 委托分支；
 * 生产消费方 = 本模块内部 useAgentNotifications effect）
 *
 * 类别判定知识委托 profile：按 MC-205 三级解析取 profile 后调
 * profile.capabilities.hooks.classifyNotification(payload)：
 *   - 三级解析经 resolvePayloadCliId 单点（ZQ-2，契约 4）：payload.cliId
 *     （trim 后非空）→ TerminalRegistry.get(panelId).agentSession.cliId（反查）
 *     → CLAUDE_CLI_ID（缺省回退，兼容旧信号）；空串/仅空白同等回退
 *   - 未注册 cliId（未知 CLI）→ console.warn + 返回 null（不通知，不抛异常，MC-206）
 *   - profile 无 hooks 能力 → 返回 null（不通知）
 *   - 其余 → 委托 hooks.classifyNotification，返回类别 permission/error/done/null
 */
export function classifyEvent(payload: AgentEventPayload): NotifyCategory | null {
  // MC-205 三级解析单点（三消费方同一 helper）
  const cliId = resolvePayloadCliId(payload);

  const profile = cliProfileRegistry.get(cliId);
  if (!profile) {
    // MC-206：未知 cliId 跳过不通知，不抛异常
    console.warn(`[notifications] 未知 cliId: ${cliId}——跳过通知`);
    return null;
  }

  // 无 hooks 能力 → 不通知
  return profile.capabilities?.hooks?.classifyNotification(payload) ?? null;
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
 * 发送关注态任务栏闪烁
 *
 * Windows 上 UserAttentionType.Critical = FLASHW_TIMERNOFG，
 * 持续闪烁直到窗口获得焦点。
 */
function flashTaskbar(): void {
  requestUserAttention(UserAttentionType.Critical).catch((err) => {
    console.error("flashTaskbar 失败:", err);
  });
}

/** 使用 ref 避免 effect 重复注册 */
let permissionEnsured = false;

/**
 * F4 通知调度 hook
 * 导出为测试专用（notifications.test.ts / mock-cli-profile.test.tsx 直测；
 * 生产消费方 = 本模块内部 NotificationListener 组件）
 *
 * 在 App.tsx 挂载的 NotificationListener 组件中调用。
 * 订阅 onAgentEvent，在窗口失焦时按事件类别触发通知。
 */
export function useAgentNotifications(): void {
  // 用于防止同一事件重复通知的去重 set（基于 sessionId + event + timestamp）
  const seenRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    // 懒初始化：首次调用时确保通知权限（仅一次）
    if (!permissionEnsured) {
      permissionEnsured = true;
      ensureNotificationPermission().catch(() => {
        // 权限被拒——后续 sendToastNotification 内部 catch 会输出 console.error
      });
    }

    const unlisten = onAgentEvent((payload) => {
      // 门控：窗口聚焦时不触发通知
      if (window.__slterm_windowFocused !== false) return;

      // 事件分类（类别判定委托 profile，MC-420）
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

      // 获取项目名：从 panelId 反查
      const pageId = parseTerminalPageId(payload.panelId);
      let projectName = "";
      if (pageId) {
        const projectId = findProjectIdForPage(pageId);
        if (projectId) {
          const { projects } = useProjects.getState();
          projectName = projects[projectId]?.name ?? "";
        }
      }

      // 构建 toast 正文：<项目名> · <事件类别> · <时间>（类别为纯文本标签——IC-07）
      const timeStr = new Date().toLocaleTimeString();
      const label = CATEGORY_LABEL[category];
      const bodyParts = [projectName, label, timeStr]
        .filter(Boolean)
        .join(" · ");

      // 三类事件均闪烁任务栏（toast 失去点击路由后，闪烁是唯一回窗引导通道）
      flashTaskbar();

      // 发送 toast（Tauri 原生通道，无点击路由）
      sendToastNotification("slTerminal", { body: bodyParts });
    });

    return unlisten;
  }, []);
}

/**
 * 通知监听器组件
 *
 * 在 App.tsx 中挂载一次，内部调用 useAgentNotifications()。
 * 无 UI 输出——纯副作用组件。
 */
export function NotificationListener(): null {
  useAgentNotifications();
  return null;
}
