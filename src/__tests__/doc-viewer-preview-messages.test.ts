// doc-viewer-preview-messages.test.ts — docViewer 预览框消息协议常量与守卫测试
//
// 原 html-zoom-math.test.ts 随 zoomMath → docViewer/previewMessages 迁入。
// 纯函数测试：无 mock/jsdom/React 依赖（与 zoomRuntime 的行为级测试互补——
// 本文件锁常量值与边界数学，doc-viewer-zoom-runtime.test.ts 锁注入执行行为）。

import { describe, it, expect } from "vitest";
import {
  ZOOM_MSG_TYPE,
  RESET_MSG_TYPE,
  ZOOM_MIN,
  ZOOM_MAX,
  ZOOM_STEP,
  WHEEL_STEP_PX,
  ZOOM_ROUND,
  clampZoom,
  formatPercent,
  isFiniteZoom,
  buildZoomReport,
  buildResetRequest,
} from "../panels/docViewer/previewMessages";

describe("previewMessages zoom 常量", () => {
  it("协议类型与既有 slterm_key 命名风格同构（平铺字符串）", () => {
    expect(ZOOM_MSG_TYPE).toBe("slterm_zoom");
    expect(RESET_MSG_TYPE).toBe("slterm_reset");
    expect(ZOOM_MSG_TYPE.startsWith("slterm_")).toBe(true);
    expect(RESET_MSG_TYPE.startsWith("slterm_")).toBe(true);
  });

  it("缩放范围/步进/阈值常量与需求规格一致", () => {
    // 需求：25%–400%，等比 ±10%（×/÷1.1），每格 ≈1 滚轮刻度
    expect(ZOOM_MIN).toBe(0.25);
    expect(ZOOM_MAX).toBe(4);
    expect(ZOOM_STEP).toBeCloseTo(1.1, 12);
    expect(WHEEL_STEP_PX).toBe(100);
    expect(ZOOM_ROUND).toBe(1e6); // round6：浮点尾串收敛精度
  });

  it("协议类型两两互异（不与其他消息类型撞车）", () => {
    expect(ZOOM_MSG_TYPE).not.toBe(RESET_MSG_TYPE);
  });
});

describe("clampZoom", () => {
  it("范围内原样返回", () => {
    expect(clampZoom(1)).toBe(1);
    expect(clampZoom(1.331)).toBeCloseTo(1.331, 12);
  });

  it("低于下限夹到 0.25", () => {
    expect(clampZoom(0.2)).toBe(ZOOM_MIN);
    expect(clampZoom(0.25)).toBe(ZOOM_MIN); // 边界不夹
    expect(clampZoom(-1)).toBe(ZOOM_MIN);
  });

  it("高于上限夹到 4", () => {
    expect(clampZoom(5)).toBe(ZOOM_MAX);
    expect(clampZoom(4)).toBe(ZOOM_MAX); // 边界不夹
    expect(clampZoom(99)).toBe(ZOOM_MAX);
  });
});

describe("formatPercent", () => {
  it("整百分数直出", () => {
    expect(formatPercent(1)).toBe("100%");
    expect(formatPercent(0.25)).toBe("25%");
    expect(formatPercent(4)).toBe("400%");
  });

  it("等比步进序列取整显示（1.1 序列）", () => {
    expect(formatPercent(1.1)).toBe("110%");
    expect(formatPercent(1.21)).toBe("121%");
    expect(formatPercent(1.331)).toBe("133%");
  });

  it("浮点尾串容错取整", () => {
    expect(formatPercent(1.2100000000000002)).toBe("121%");
    expect(formatPercent(0.9949999999)).toBe("99%"); // 0.995 以下下取整
    expect(formatPercent(1.016)).toBe("102%"); // 0.5 以上上取整
  });
});

describe("isFiniteZoom", () => {
  it("接受有限正数", () => {
    expect(isFiniteZoom(1)).toBe(true);
    expect(isFiniteZoom(0.25)).toBe(true);
    expect(isFiniteZoom(4.0001)).toBe(true); // 数值合法，clamp 时才夹
    expect(isFiniteZoom(0)).toBe(true);
  });

  it("拒绝 NaN/Infinity/负数语义外数值", () => {
    expect(isFiniteZoom(NaN)).toBe(false);
    expect(isFiniteZoom(Infinity)).toBe(false);
    expect(isFiniteZoom(-Infinity)).toBe(false);
  });

  it("拒绝非 number 载荷", () => {
    expect(isFiniteZoom("1.1")).toBe(false);
    expect(isFiniteZoom(null)).toBe(false);
    expect(isFiniteZoom(undefined)).toBe(false);
    expect(isFiniteZoom({})).toBe(false);
    expect(isFiniteZoom(true)).toBe(false);
  });
});

describe("消息载荷构造器", () => {
  it("buildZoomReport 平铺结构与既有 slterm_key 同构", () => {
    expect(buildZoomReport("abc123", 1.21)).toEqual({
      type: ZOOM_MSG_TYPE,
      nonce: "abc123",
      zoom: 1.21,
    });
  });

  it("buildResetRequest 携带 type + nonce", () => {
    expect(buildResetRequest("deadbeef")).toEqual({
      type: RESET_MSG_TYPE,
      nonce: "deadbeef",
    });
  });
});
