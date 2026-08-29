// 套餐余量 IPC（F10）——快照拉取 / 立即刷新 / 更新事件订阅
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { PlanBalanceInfo } from "../types/planBalance";

/** 拉取当前快照（挂载时一次；后端尚未有快照 → 空数组） */
export function getPlanBalance(): Promise<PlanBalanceInfo[]> {
  return invoke("get_plan_balance");
}

/**
 * 立即刷新（点击余量行）：后端执行一轮拉取并返回最新快照。
 * 单来源失败按规格 §6 保留旧值不整体报错；调用方 catch 仅防御（console.error）。
 */
export function refreshPlanBalance(): Promise<PlanBalanceInfo[]> {
  return invoke("refresh_plan_balance");
}

/** 订阅余量更新（后端有变化才推送）；返回 unsubscribe，卸载时调用 */
export function onPlanBalanceUpdated(
  callback: (payload: PlanBalanceInfo[]) => void,
): () => void {
  const unlisten = listen<PlanBalanceInfo[]>("plan-balance-updated", (event) =>
    callback(event.payload),
  );
  return () => {
    unlisten.then((fn) => fn());
  };
}

/** 设置轮询间隔秒（F11）：后端校验 10–3600 → 落盘 + 更新内存值，立即生效 */
export function setPlanBalanceInterval(intervalSec: number): Promise<void> {
  return invoke("plan_balance_set_interval", { intervalSec });
}
