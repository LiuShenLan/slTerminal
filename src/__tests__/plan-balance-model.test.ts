// plan-balance-model.test.ts — F10 套餐余量展示纯函数全分支测试（PB-FE-05）
//
// 纯函数零依赖（不 import theme、不触 IPC）：currencySymbol / planLogoSrc /
// formatResetTime（D12 六档边界 + 缺失/非法）/ formatUpdatedAt / rowText 四场景 /
// rowTooltip 四场景。

import { describe, it, expect } from "vitest";
import {
  currencySymbol,
  planLogoSrc,
  formatResetTime,
  formatUpdatedAt,
  rowText,
  rowTooltip,
} from "../features/navTree/planBalanceModel";
import type { PlanBalanceInfo } from "../types/planBalance";

/** 构造最小 PlanBalanceInfo（F10 DTO 六键全字段） */
function makeInfo(overrides: Partial<PlanBalanceInfo> = {}): PlanBalanceInfo {
  return {
    sourceId: "claude",
    planId: "deepseek",
    frozen: false,
    amount: null,
    windows: null,
    updatedAt: 0,
    ...overrides,
  };
}

describe("currencySymbol（§5.1 映射）", () => {
  it("CNY → ¥", () => {
    expect(currencySymbol("CNY")).toBe("¥");
  });

  it("USD → $", () => {
    expect(currencySymbol("USD")).toBe("$");
  });

  it("未知货币 → 原代码原样返回", () => {
    expect(currencySymbol("EUR")).toBe("EUR");
  });
});

describe("planLogoSrc（§10.3 路径约定）", () => {
  it("/plan-icons/<planId>.png（deepseek / kimi）", () => {
    expect(planLogoSrc("deepseek")).toBe("/plan-icons/deepseek.png");
    expect(planLogoSrc("kimi")).toBe("/plan-icons/kimi.png");
  });
});

describe("formatResetTime（D12 六档 + 缺失/非法）", () => {
  // 固定锚点：2026-09-02 14:00:00（本地时区无关——now 与 resetsAt 同构取相对差）
  const NOW = Date.parse("2026-09-02T14:00:00");
  const iso = (ms: number) => new Date(ms).toISOString();

  it("<1h 上取整：30s → 1m 后重置", () => {
    expect(formatResetTime(iso(NOW + 30_000), NOW)).toBe("1m 后重置");
  });

  it("<1h 上取整：59m59s → 60m 后重置（ceil 到分钟）", () => {
    expect(formatResetTime(iso(NOW + 59 * 60_000 + 59_000), NOW)).toBe("60m 后重置");
  });

  it("整 1h 边界归 <24h 档：1h 0m → 1h 后重置（m=0 省略）", () => {
    expect(formatResetTime(iso(NOW + 60 * 60_000), NOW)).toBe("1h 后重置");
  });

  it("<24h 且 m>0：3h42m → 3h 42m 后重置（规格 §8.2 示例）", () => {
    expect(formatResetTime(iso(NOW + 3 * 3_600_000 + 42 * 60_000), NOW)).toBe("3h 42m 后重置");
  });

  it("≥24h 绝对时间：月日无前导零、时分有前导零（规格 §8.2 示例）", () => {
    // 2026-09-05 14:00 → 9月5日 14:00 重置（跨 3 天）
    expect(formatResetTime(iso(NOW + 3 * 86_400_000), NOW)).toBe("9月5日 14:00 重置");
  });

  it("diff≤0 → 0m 后重置（clamp，规格未覆盖人工验证点）", () => {
    expect(formatResetTime(iso(NOW - 60_000), NOW)).toBe("0m 后重置");
    expect(formatResetTime(iso(NOW), NOW)).toBe("0m 后重置");
  });

  it("缺失 null → null（调用方省略该段）", () => {
    expect(formatResetTime(null, NOW)).toBeNull();
  });

  it("非法串解析失败 → null", () => {
    expect(formatResetTime("not-a-date", NOW)).toBeNull();
  });
});

describe("formatUpdatedAt", () => {
  it("正常值 → HH:mm:ss 带前导零", () => {
    // 2026-09-02 14:03:05 本地时间 → 14:03:05
    const t = new Date(2026, 8, 2, 14, 3, 5).getTime() / 1000;
    expect(formatUpdatedAt(t)).toBe("14:03:05");
  });

  it("updatedAt=0（尚无成功值）→ null", () => {
    expect(formatUpdatedAt(0)).toBeNull();
  });
});

describe("rowText（§8.2 四场景）", () => {
  it("frozen → 已冻结", () => {
    const info = makeInfo({ frozen: true, planId: "kimi" });
    expect(rowText(info)).toBe("已冻结");
  });

  it("有金额 → 货币符号 + 原样值（deepseek ¥12.34）", () => {
    const info = makeInfo({ amount: { value: "12.34", currency: "CNY" } });
    expect(rowText(info)).toBe("¥12.34");
  });

  it("双窗 → 5h X% · 7d Y%（2026-09 起 X/Y = 已用百分比，值不透明透传）", () => {
    const info = makeInfo({
      planId: "kimi",
      windows: {
        fiveHour: { usedPercent: 62, resetsAt: null },
        sevenDay: { usedPercent: 45, resetsAt: null },
      },
    });
    expect(rowText(info)).toBe("5h 62% · 7d 45%");
  });

  it("无数据（amount/windows 均空）→ --", () => {
    expect(rowText(makeInfo())).toBe("--");
  });
});

describe("rowTooltip（§8.2 四场景）", () => {
  const NOW = Date.parse("2026-09-02T14:00:00");
  const iso = (ms: number) => new Date(ms).toISOString();
  // 本地时间构造（formatUpdatedAt 按本地时区输出，避免 CI 机器时区差异）
  const UPDATED_SEC = new Date(2026, 8, 2, 14, 3, 5).getTime() / 1000;

  it("kimi 全段：5h 重置 + 7d 重置 + 上次更新", () => {
    const info = makeInfo({
      planId: "kimi",
      windows: {
        fiveHour: { usedPercent: 62, resetsAt: iso(NOW + 42 * 60_000) },
        sevenDay: { usedPercent: 45, resetsAt: iso(NOW + 86_400_000) },
      },
      updatedAt: UPDATED_SEC,
    });
    const tooltip = rowTooltip(info, NOW);
    expect(tooltip).toContain("5h 窗（42m 后重置）");
    expect(tooltip).toContain("7d 窗（9月3日 14:00 重置）");
    expect(tooltip).toContain("上次更新 14:03:05");
  });

  it("单窗 resetTime 缺失 → 该段省略（kimi windows 必带双窗但 resetsAt 可缺失）", () => {
    const info = makeInfo({
      planId: "kimi",
      windows: {
        fiveHour: { usedPercent: 62, resetsAt: null },
        sevenDay: { usedPercent: 45, resetsAt: iso(NOW + 60_000) },
      },
      updatedAt: UPDATED_SEC,
    });
    const tooltip = rowTooltip(info, NOW);
    expect(tooltip).not.toContain("5h");
    expect(tooltip).toContain("7d 窗（1m 后重置）");
    expect(tooltip).toContain("上次更新 14:03:05");
  });

  it("deepseek（仅金额）→ 仅上次更新", () => {
    const info = makeInfo({
      amount: { value: "12.34", currency: "CNY" },
      updatedAt: UPDATED_SEC,
    });
    expect(rowTooltip(info, NOW)).toBe("上次更新 14:03:05");
  });

  it("frozen / 无数据 → 固定文案（不含重置段）", () => {
    expect(rowTooltip(makeInfo({ frozen: true, planId: "kimi" }), NOW)).toBe(
      "月限额触顶，Kimi Code 已冻结",
    );
    expect(rowTooltip(makeInfo(), NOW)).toBe("查询中 / 查询失败重试中");
  });
});
