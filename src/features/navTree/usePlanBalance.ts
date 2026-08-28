// F10 余量数据 hook：挂载拉一次 + 订阅 plan-balance-updated + 点击刷新（前端节流 5s，D7）
import { useCallback, useEffect, useRef, useState } from "react";
import type { PlanBalanceInfo } from "../../types/planBalance";
import {
  getPlanBalance,
  onPlanBalanceUpdated,
  refreshPlanBalance,
} from "../../ipc/planBalance";

/** 点击刷新节流窗口（规格 §6：连点在窗口内忽略） */
export const REFRESH_THROTTLE_MS = 5_000;

export function usePlanBalance() {
  const [items, setItems] = useState<PlanBalanceInfo[]>([]);

  useEffect(() => {
    let cancelled = false;
    getPlanBalance()
      .then((v) => { if (!cancelled) setItems(v); })
      .catch((e) => { console.error("get_plan_balance 初始拉取失败", e); });
    const unlisten = onPlanBalanceUpdated(setItems);
    return () => { cancelled = true; unlisten(); };
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

  return { items, refresh };
}
