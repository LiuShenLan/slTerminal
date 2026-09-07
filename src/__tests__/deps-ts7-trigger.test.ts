// deps-ts7-trigger.test.ts — CP-001 双 TS 并存解除触发条件的机检判定(L2,CP-001)
//
// 防复发对照:修复前 ADR-0010 TE-07 触发条件为散文(「issue #10940 闭环 + TS7.1 稳定发布」),
// 无可执行判定,只能靠人读——本文件锁 scripts/check-ts7-trigger.mjs 的纯判定
// evaluateTrigger 语义(已闭环 + 7.1.x 无预发布后缀才 triggered),漂移即红。

import { describe, it, expect } from "vitest";
import { evaluateTrigger } from "../../scripts/check-ts7-trigger.mjs";

describe("evaluateTrigger(CP-001 双条件判定)", () => {
  it("evaluateTrigger_issue已闭环且TS71稳定_达成", () => {
    const r = evaluateTrigger("closed", "7.1.0");
    expect(r).toEqual({ issueClosed: true, ts71Stable: true, triggered: true });
  });

  it("evaluateTrigger_issue未闭环_不达成", () => {
    const r = evaluateTrigger("open", "7.1.0");
    expect(r.triggered).toBe(false);
    expect(r.ts71Stable).toBe(true);
  });

  it("evaluateTrigger_TS71未发布_不达成", () => {
    const r = evaluateTrigger("closed", "7.0.2");
    expect(r.triggered).toBe(false);
  });

  it("evaluateTrigger_预发布后缀_不视为稳定", () => {
    // 正则 ^(\d+)\.(\d+)\.(\d+)$ 拒预发布后缀
    const r = evaluateTrigger("closed", "7.1.0-beta.1");
    expect(r.ts71Stable).toBe(false);
    expect(r.triggered).toBe(false);
  });

  it("evaluateTrigger_查询失败入参null_不达成", () => {
    const r = evaluateTrigger(undefined, null as unknown as string);
    expect(r).toEqual({ issueClosed: false, ts71Stable: false, triggered: false });
  });
});
