// 套餐余量展示纯函数（F10 规格 §8.2）——全部颜色/布局在组件层，本文件零依赖 theme

import type { PlanBalanceInfo } from "../../types/planBalance";

/** 货币符号映射（§5.1）：CNY→¥、USD→$、未知 → 原货币代码 */
export function currencySymbol(currency: string): string {
  if (currency === "CNY") return "¥";
  if (currency === "USD") return "$";
  return currency;
}

/** logo 路径约定（§10.3）：/plan-icons/<planId>.png（无映射表） */
export function planLogoSrc(planId: string): string {
  return `/plan-icons/${planId}.png`;
}

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/**
 * 重置时间格式化（D12）：<1h → `Xm 后重置`；<24h → `Xh Ym 后重置`（m=0 → `Xh 后重置`）；
 * ≥24h → `M月d日 HH:mm 重置`（月日无前导零）；diff≤0 → `0m 后重置`（clamp）；
 * 缺失/解析失败 → null（调用方省略该段）
 */
export function formatResetTime(resetsAt: string | null, nowMs: number): string | null {
  if (!resetsAt) return null;
  const t = Date.parse(resetsAt);
  if (Number.isNaN(t)) return null;
  const diff = Math.max(0, t - nowMs);
  if (diff < HOUR_MS) return `${Math.ceil(diff / MINUTE_MS)}m 后重置`;
  if (diff < DAY_MS) {
    const h = Math.floor(diff / HOUR_MS);
    const m = Math.round((diff % HOUR_MS) / MINUTE_MS);
    return m > 0 ? `${h}h ${m}m 后重置` : `${h}h 后重置`;
  }
  const d = new Date(t);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getMonth() + 1}月${d.getDate()}日 ${pad(d.getHours())}:${pad(d.getMinutes())} 重置`;
}

/** 「上次更新 HH:mm:ss」（updatedAt=0 → null） */
export function formatUpdatedAt(updatedAt: number): string | null {
  if (updatedAt <= 0) return null;
  const d = new Date(updatedAt * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** 行文案四场景（§8.2）：frozen → 已冻结；金额；双窗；无数据 → -- */
export function rowText(info: PlanBalanceInfo): string {
  if (info.frozen) return "已冻结";
  if (info.amount) return `${currencySymbol(info.amount.currency)}${info.amount.value}`;
  if (info.windows) {
    return `5h ${info.windows.fiveHour.remainingPercent}% · 7d ${info.windows.sevenDay.remainingPercent}%`;
  }
  return "--";
}

/** tooltip 文案（§8.2）：kimi 双窗重置段（缺失省略）+ 上次更新；frozen/无数据固定文案 */
export function rowTooltip(info: PlanBalanceInfo, nowMs: number): string {
  if (info.frozen) return "月限额触顶，Kimi Code 已冻结";
  if (!info.amount && !info.windows) return "查询中 / 查询失败重试中";
  const parts: string[] = [];
  if (info.windows) {
    const f = formatResetTime(info.windows.fiveHour.resetsAt, nowMs);
    const s = formatResetTime(info.windows.sevenDay.resetsAt, nowMs);
    if (f) parts.push(`5h ${f}`);
    if (s) parts.push(`7d ${s}`);
  }
  const updated = formatUpdatedAt(info.updatedAt);
  if (updated) parts.push(`上次更新 ${updated}`);
  return parts.join(" · ");
}
