// F10 余量数据 hook：挂载拉一次 + 订阅 plan-balance-updated + 点击刷新（前端节流 5s，D7）
// F12：enabled 感知通道 = background_tasks_list 读取 + background-tasks-updated 事件订阅——
// planBalance 任务禁用 → footer 整块不渲染（快照保留，重启用即重显）
import { useCallback, useEffect, useRef, useState } from "react";
import type { PlanBalanceInfo } from "../../types/planBalance";
import {
  getPlanBalance,
  onPlanBalanceUpdated,
  refreshPlanBalance,
} from "../../ipc/planBalance";
import {
  listBackgroundTasks,
  onBackgroundTasksUpdated,
} from "../../ipc/backgroundTasks";
import { PLAN_BALANCE_TASK_ID } from "../../types/backgroundTasks";

/** 点击刷新节流窗口（规格 §6：连点在窗口内忽略） */
export const REFRESH_THROTTLE_MS = 5_000;

export function usePlanBalance() {
  const [items, setItems] = useState<PlanBalanceInfo[]>([]);
  // null = 配置未加载（footer 不渲染，防「先渲染后隐藏」闪烁）
  const [enabled, setEnabled] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    getPlanBalance()
      .then((v) => { if (!cancelled) setItems(v); })
      .catch((e) => { console.error("get_plan_balance 初始拉取失败", e); });
    // F12：enabled 感知（与首拉并行）——list 读 + 事件订阅双通道同口径
    listBackgroundTasks()
      .then((list) => {
        if (cancelled) return;
        setEnabled(
          list.find((t) => t.taskId === PLAN_BALANCE_TASK_ID)?.enabled ?? true,
        );
      })
      .catch((e) => {
        // 配置读取失败回退启用（宁可显示，与现行行为一致）
        console.error("background_tasks_list 读取失败，按启用处理", e);
        if (!cancelled) setEnabled(true);
      });
    const unlisten = onPlanBalanceUpdated(setItems);
    const unlistenConfig = onBackgroundTasksUpdated((list) => {
      setEnabled(
        list.find((t) => t.taskId === PLAN_BALANCE_TASK_ID)?.enabled ?? true,
      );
    });
    return () => { cancelled = true; unlisten(); unlistenConfig(); };
  }, []);

  const lastRefreshRef = useRef(0);
  const refresh = useCallback(() => {
    const now = Date.now();
    if (now - lastRefreshRef.current < REFRESH_THROTTLE_MS) return;
    lastRefreshRef.current = now;
    refreshPlanBalance()
      .then(setItems)
      .catch((e) => { console.error("refresh_plan_balance 失败，保留旧值", e); });
  }, []);

  return { items, refresh, enabled };
}
