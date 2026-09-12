// preview-frame-geometry-sync.test.tsx — 预览窗口几何同步防复发测试
//
// 回归锚点（2026-09 预览错位 bug）：syncNow 去重只比较锚点 CSS 视口矩形——
// 主窗移动/resize/scale 时矩形等值但物理换算基准已变，旧实现去重早退吞掉
// onMainWindowMoved（死代码）→ 预览窗停在创建时屏幕位置，不随主窗移动。
// 修复 = forceSync 旗标旁路去重 + 50ms 节流 + onMoved/onResized/onScaleChanged
// 三事件订阅。
//
// mock 边界：ipc/preview（previewSync 捕获）、ipc/window（三事件回调捕获驱动）、
// lib/injectScript（透传）；fake timers 控制 200ms 轮询与 50ms 节流。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { render, cleanup, act } from "@testing-library/react";

// ─── Hoisted mocks ───
const mocks = vi.hoisted(() => {
  return {
    previewSync: vi.fn<
      (
        label: string,
        x: number,
        y: number,
        w: number,
        h: number,
        visible: boolean,
        token: string,
      ) => Promise<void>
    >(),
    previewRender: vi.fn<(label: string, html: string, bg?: string) => Promise<void>>(),
    previewClose: vi.fn<(label: string, token: string) => Promise<void>>(),
    /** 三事件注册回调捕获（触发 = 调用 cb） */
    moved: { cb: null as null | (() => void) },
    resized: { cb: null as null | (() => void) },
    scaleChanged: { cb: null as null | (() => void) },
    /** ResizeObserver 回调捕获（PreviewFrame 锚点尺寸驱动） */
    ro: { cb: null as null | (() => void), disconnected: false },
  };
});

vi.mock("../ipc/preview", () => ({
  makePreviewLabel: (panelId: string) => `preview-${panelId}`,
  previewSync: mocks.previewSync,
  previewRender: mocks.previewRender,
  previewClose: mocks.previewClose,
  onPreviewUplink: () => () => {},
  onPreviewHostStatus: () => () => {},
  emitPreviewDownlink: vi.fn(() => Promise.resolve()),
}));

vi.mock("../ipc/window", () => ({
  onMainWindowMoved: (cb: () => void) => {
    mocks.moved.cb = cb;
    return () => {
      mocks.moved.cb = null;
    };
  },
  onMainWindowResized: (cb: () => void) => {
    mocks.resized.cb = cb;
    return () => {
      mocks.resized.cb = null;
    };
  },
  onMainWindowScaleChanged: (cb: () => void) => {
    mocks.scaleChanged.cb = cb;
    return () => {
      mocks.scaleChanged.cb = null;
    };
  },
}));

vi.mock("../lib", () => ({
  injectScript: (html: string) => html,
}));

// ─── 真实模块导入（mock 之后）───
import { PreviewFrame } from "../panels/docViewer/PreviewFrame";

/** 可控锚点矩形（getBoundingClientRect 全局 stub 的取值源） */
let rect = { x: 100, y: 50, width: 400, height: 300 };

function renderFrame() {
  return render(
    React.createElement(PreviewFrame, {
      panelId: "p1:doc-1",
      html: "<p>hi</p>",
      title: "预览",
    }),
  );
}

/** 第 n 次 previewSync 调用的几何参数（label/token 外的五元组） */
function syncArgs(callIndex: number) {
  const call = mocks.previewSync.mock.calls[callIndex];
  return {
    label: call[0],
    x: call[1],
    y: call[2],
    w: call[3],
    h: call[4],
    visible: call[5],
  };
}

describe("PreviewFrame 几何同步（force-sync 防复发）", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    // 无实现 vi.fn 调后返回 undefined——组件侧 .then 链需要真 Promise
    mocks.previewSync.mockResolvedValue(undefined);
    mocks.previewRender.mockResolvedValue(undefined);
    mocks.previewClose.mockResolvedValue(undefined);
    mocks.moved.cb = null;
    mocks.resized.cb = null;
    mocks.scaleChanged.cb = null;
    mocks.ro.cb = null;
    mocks.ro.disconnected = false;
    // 锚点 RO 捕获版（setup.ts 全局 stub 不触发回调，本文件需驱动语义）
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(cb: () => void) {
          mocks.ro.cb = cb;
        }
        observe() {}
        unobserve() {}
        disconnect() {
          mocks.ro.disconnected = true;
        }
      },
    );
    rect = { x: 100, y: 50, width: 400, height: 300 };
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
      () =>
        ({
          x: rect.x,
          y: rect.y,
          width: rect.width,
          height: rect.height,
          top: rect.y,
          left: rect.x,
          right: rect.x + rect.width,
          bottom: rect.y + rect.height,
          toJSON: () => ({}),
        }) as DOMRect,
    );
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("挂载即发一次 sync；等值轮询不重发（去重早退语义保留）", () => {
    renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);
    expect(syncArgs(0)).toEqual({
      label: "preview-p1:doc-1",
      x: 100,
      y: 50,
      w: 400,
      h: 300,
      visible: true,
    });

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);
  });

  it("防复发：主窗移动后同矩形强制重发（旧代码去重早退吞移动事件）", () => {
    renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);

    act(() => {
      mocks.moved.cb?.();
    });
    act(() => {
      vi.advanceTimersByTime(50);
    });

    expect(mocks.previewSync).toHaveBeenCalledTimes(2);
    // 同矩形重发（物理基准已变，后端按新主窗位置换算）
    expect(syncArgs(1)).toEqual(syncArgs(0));
  });

  it("主窗移动事件高频触发 → 50ms 节流合并为一次 sync", () => {
    renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);

    act(() => {
      for (let i = 0; i < 10; i++) mocks.moved.cb?.();
    });
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(2);

    // 后续轮询等值早退，无新增
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(2);
  });

  it("主窗 resize / scale 变化同样触发强制重发", () => {
    renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);

    act(() => {
      mocks.resized.cb?.();
    });
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(2);

    act(() => {
      mocks.scaleChanged.cb?.();
    });
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(3);
  });

  it("锚点几何变化经 200ms 轮询检测重发", () => {
    renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);

    rect = { x: 120, y: 70, width: 500, height: 300 };
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(2);
    expect(syncArgs(1)).toMatchObject({ x: 120, y: 70, w: 500, h: 300 });
  });

  it("锚点尺寸变化经 ResizeObserver 即时重发（不等 200ms 轮询）", () => {
    renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);
    expect(mocks.ro.cb).not.toBeNull();

    rect = { x: 100, y: 50, width: 600, height: 300 };
    act(() => {
      mocks.ro.cb?.();
    });
    // 不推进定时器——RO 回调同步驱动 syncNow
    expect(mocks.previewSync).toHaveBeenCalledTimes(2);
    expect(syncArgs(1)).toMatchObject({ x: 100, y: 50, w: 600, h: 300 });
  });

  it("防复发（split 空白）：首测 0×0 不可见 → RO 落定非零即刻补发 visible=true", () => {
    // split 形态时序：预览 pane 动态挂载（Allotment addView 在父 effect，
    // React 子 effect 先行）→ 首测 0×0 → visible=false（后端隐藏态不建窗）；
    // 布局落定 RO 驱动即刻补发（旧代码须等 200ms 轮询，叠加去重缺陷放大为空白）
    rect = { x: 0, y: 0, width: 0, height: 0 };
    renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);
    expect(syncArgs(0).visible).toBe(false);

    rect = { x: 100, y: 50, width: 400, height: 300 };
    act(() => {
      mocks.ro.cb?.();
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(2);
    expect(syncArgs(1)).toMatchObject({ x: 100, y: 50, w: 400, h: 300, visible: true });
  });

  it("卸载 → previewClose + 事件解绑 + 无后续 sync", () => {
    const { unmount } = renderFrame();
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);

    unmount();
    expect(mocks.previewClose).toHaveBeenCalledTimes(1);
    expect(mocks.previewClose.mock.calls[0][0]).toBe("preview-p1:doc-1");
    expect(mocks.moved.cb).toBeNull();
    expect(mocks.resized.cb).toBeNull();
    expect(mocks.scaleChanged.cb).toBeNull();

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(mocks.previewSync).toHaveBeenCalledTimes(1);
  });
});
