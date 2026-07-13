// e2e-enabled.test.ts — E2E 门控开关逻辑测试（T1）
//
// 覆盖 src/lib/e2eEnabled.ts：computeE2eEnabled 纯逻辑全表 +
// E2E_ENABLED 内联常量与纯逻辑一致性（防内联式/纯函数漂移）。
// 生产构建"剥离 helper"的 tree-shaking 属性 jsdom 无法验证，由 CI grep 守卫（见 ci.yml）。

import { describe, it, expect } from "vitest";
import { computeE2eEnabled, E2E_ENABLED } from "../lib/e2eEnabled";

describe("computeE2eEnabled — 门控真值表", () => {
  it.each<[boolean, string | undefined, boolean]>([
    [true, undefined, true], // dev serve
    [true, "1", true], // dev serve + E2E 构建
    [false, "1", true], // E2E 构建（build:e2e）
    [false, undefined, false], // 生产发布构建
    [false, "0", false], // 显式关闭
    [false, "", false], // 空串非 "1"
    [false, "true", false], // 仅 "1" 视为开启
  ])("dev=%s, viteE2e=%o → %s", (dev, viteE2e, expected) => {
    expect(computeE2eEnabled(dev, viteE2e)).toBe(expected);
  });
});

describe("E2E_ENABLED — 常量", () => {
  it("与 computeE2eEnabled(当前 env) 一致（防漂移）", () => {
    expect(E2E_ENABLED).toBe(
      computeE2eEnabled(import.meta.env.DEV, import.meta.env.VITE_E2E),
    );
  });

  it("测试环境（vitest DEV=true）下为 true —— 既有 E2E helper 相关用例依赖此", () => {
    expect(E2E_ENABLED).toBe(true);
  });
});
