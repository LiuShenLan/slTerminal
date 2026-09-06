// doc-viewer-zoom-runtime.test.ts — docViewer 注入缩放运行时行为级测试
//
// 原 html-zoom-runtime.test.ts 随 zoomRuntime → docViewer/ 迁入。
// jsdom 不执行 srcdoc iframe 内脚本（既有测试靠字符串/正则断言注入逻辑）。
// 本文件把 zoomRuntime 生成的匿名函数源码经 new Function 取回，在桩 doc/win
// 上真实执行——注入核心获得 L2 行为级覆盖（真实 WebView2 由 L4/手工验收）。
//
// 桩模型：doc 记录监听器（含捕获标志）并支持 stopImmediatePropagation 中止
// 语义（注入 wheel 监听先注册，页面后注册的同 doc 监听应被阻断）；win.parent
// 是 postMessage spy；下行 message 由 dispatchMessage 手动派发（模拟父来信）。
// 事件对象一律工厂构造，杜绝跨用例共享可变状态。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { buildZoomRuntimeSource } from "../panels/docViewer/zoomRuntime";
import {
  ZOOM_MSG_TYPE,
  RESET_MSG_TYPE,
  ZOOM_MAX,
  ZOOM_MIN,
} from "../panels/docViewer/previewMessages";

/** 测试 nonce（hex，符合 createNonce 形态） */
const NONCE = "00ff00ff00ff00ff00ff00ff00ff00ff";

/** 取回注入运行时函数（参数签名 function(doc, win)） */
function loadRuntime(): (doc: StubDoc, win: StubWin) => void {
  const fn = new Function(`return (${buildZoomRuntimeSource(NONCE)})`) as () => (
    doc: StubDoc,
    win: StubWin,
  ) => void;
  return fn();
}

// ─── 桩类型与构造 ───

interface StubWheelEvent {
  ctrlKey: boolean;
  metaKey: boolean;
  deltaY: number;
  deltaMode: number;
  defaultPrevented: boolean;
  stoppedImmediate: boolean;
  preventDefault(): void;
  stopImmediatePropagation(): void;
}

type WheelHandler = (e: StubWheelEvent) => void;
type MsgHandler = (e: { source: unknown; data: unknown }) => void;

interface WheelRegistration {
  fn: WheelHandler;
  capture: boolean;
}

interface StubDoc {
  documentElement: { style: { zoom: string | undefined } };
  addEventListener(type: string, fn: WheelHandler, capture?: boolean): void;
  /** 依注册序派发；handler 调 stopImmediatePropagation 后中止剩余 */
  dispatch(type: string, ev: StubWheelEvent): void;
  wheelRegistrations(): WheelRegistration[];
}

interface StubWin {
  parent: { postMessage: ReturnType<typeof vi.fn> };
  addEventListener(type: string, fn: MsgHandler, capture?: boolean): void;
  /** 模拟父窗口下行的 message 事件 */
  dispatchMessage(source: unknown, data: unknown): void;
}

function makeStubDoc(): StubDoc {
  const wheelRegs: WheelRegistration[] = [];
  return {
    documentElement: { style: { zoom: undefined } },
    addEventListener(type, fn, capture) {
      if (type === "wheel") wheelRegs.push({ fn, capture: capture ?? false });
    },
    dispatch(type, ev) {
      if (type !== "wheel") return;
      for (const { fn } of wheelRegs) {
        fn(ev);
        if (ev.stoppedImmediate) break;
      }
    },
    wheelRegistrations() {
      return wheelRegs;
    },
  };
}

function makeStubWin(): StubWin {
  const postMessage = vi.fn();
  const msgHandlers: MsgHandler[] = [];
  return {
    parent: { postMessage },
    addEventListener(type, fn) {
      if (type === "message") msgHandlers.push(fn);
    },
    dispatchMessage(source, data) {
      for (const fn of msgHandlers) fn({ source, data });
    },
  };
}

function makeWheelEvent(overrides: Partial<StubWheelEvent>): StubWheelEvent {
  const ev: StubWheelEvent = {
    ctrlKey: false,
    metaKey: false,
    deltaY: 0,
    deltaMode: 0,
    defaultPrevented: false,
    stoppedImmediate: false,
    preventDefault() {
      ev.defaultPrevented = true;
    },
    stopImmediatePropagation() {
      ev.stoppedImmediate = true;
    },
    ...overrides,
  };
  return ev;
}

/** ctrl+wheel 事件工厂（overrides 可叠加 deltaMode/deltaY 等） */
function ctrlWheel(deltaY: number, overrides: Partial<StubWheelEvent> = {}): StubWheelEvent {
  return makeWheelEvent({ ctrlKey: true, deltaY, ...overrides });
}

/** 挂载运行时并返回三桩 */
function mount() {
  const doc = makeStubDoc();
  const win = makeStubWin();
  loadRuntime()(doc, win);
  return { doc, win };
}

function zoomOf(doc: StubDoc): number {
  return Number(doc.documentElement.style.zoom);
}

/** 上行消息数 */
function reportCount(win: StubWin): number {
  return win.parent.postMessage.mock.calls.length;
}

/** 最后一条上行消息 */
function lastReport(win: StubWin): { type: string; nonce: string; zoom: number } {
  const calls = win.parent.postMessage.mock.calls;
  return calls[calls.length - 1][0] as { type: string; nonce: string; zoom: number };
}

describe("zoomRuntime 注入脚本", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("wheel 监听以捕获阶段注册（addEventListener 第三参 true）", () => {
    const { doc } = mount();
    const regs = doc.wheelRegistrations();
    expect(regs).toHaveLength(1);
    expect(regs[0].capture).toBe(true);
  });

  it("无修饰键 wheel 透传：不 preventDefault、不上报（页面正常滚动）", () => {
    const { doc, win } = mount();
    const up = makeWheelEvent({ deltaY: -120 });
    const down = makeWheelEvent({ deltaY: 120 });
    doc.dispatch("wheel", up);
    doc.dispatch("wheel", down);
    expect(up.defaultPrevented).toBe(false);
    expect(down.defaultPrevented).toBe(false);
    expect(reportCount(win)).toBe(0);
    expect(doc.documentElement.style.zoom).toBeUndefined();
  });

  it("Ctrl+wheel 上滚一档：接管 + style.zoom=1.1 + 上行 payload", () => {
    const { doc, win } = mount();
    const ev = ctrlWheel(-120);
    doc.dispatch("wheel", ev);
    expect(ev.defaultPrevented).toBe(true);
    expect(zoomOf(doc)).toBe(1.1);
    expect(win.parent.postMessage).toHaveBeenCalledTimes(1);
    expect(win.parent.postMessage).toHaveBeenCalledWith(
      { type: ZOOM_MSG_TYPE, nonce: NONCE, zoom: 1.1 },
      // targetOrigin "*"（2026-09-06 实证）：须匹配接收方窗口 origin；opaque 源
      // 只影响父侧 e.origin 序列化 "null"，与发送 targetOrigin 无关
      "*",
    );
  });

  it("Ctrl+wheel 下滚缩小一档（÷1.1），上滚再放大回原位", () => {
    const { doc, win } = mount();
    doc.dispatch("wheel", ctrlWheel(-120));
    doc.dispatch("wheel", ctrlWheel(120));
    expect(zoomOf(doc)).toBe(1);
    expect(reportCount(win)).toBe(2);
    expect(lastReport(win).zoom).toBe(1);
  });

  it("等比序列 110→121→133%（round6 收敛浮点尾串）", () => {
    const { doc } = mount();
    doc.dispatch("wheel", ctrlWheel(-120));
    expect(zoomOf(doc)).toBe(1.1);
    doc.dispatch("wheel", ctrlWheel(-120));
    expect(zoomOf(doc)).toBe(1.21);
    doc.dispatch("wheel", ctrlWheel(-120));
    expect(zoomOf(doc)).toBe(1.331);
  });

  it("clamp 上限 400%：封顶后继续滚动不再变化、不再上报", () => {
    const { doc, win } = mount();
    for (let i = 0; i < 60; i++) doc.dispatch("wheel", ctrlWheel(-120));
    expect(zoomOf(doc)).toBe(ZOOM_MAX);
    const countAtMax = reportCount(win);
    for (let i = 0; i < 5; i++) doc.dispatch("wheel", ctrlWheel(-120));
    expect(reportCount(win)).toBe(countAtMax);
    expect(zoomOf(doc)).toBe(ZOOM_MAX);
  });

  it("clamp 下限 25%：反向滚到底后不再上报", () => {
    const { doc, win } = mount();
    for (let i = 0; i < 60; i++) doc.dispatch("wheel", ctrlWheel(-120));
    for (let i = 0; i < 40; i++) doc.dispatch("wheel", ctrlWheel(120));
    expect(zoomOf(doc)).toBe(ZOOM_MIN);
    const countAtMin = reportCount(win);
    for (let i = 0; i < 5; i++) doc.dispatch("wheel", ctrlWheel(120));
    expect(reportCount(win)).toBe(countAtMin);
    expect(lastReport(win).zoom).toBe(ZOOM_MIN);
  });

  it("deltaMode=1（行）归一 ×16：累计达阈值触发一档", () => {
    const { doc, win } = mount();
    // 3 行 × 16 = 48 < 100：不触发
    doc.dispatch("wheel", ctrlWheel(-3, { deltaMode: 1 }));
    expect(reportCount(win)).toBe(0);
    // 累计到 7 行（×16 = 112 ≥ 100）：一档
    doc.dispatch("wheel", ctrlWheel(-7, { deltaMode: 1 }));
    expect(reportCount(win)).toBe(1);
    expect(zoomOf(doc)).toBe(1.1);
  });

  it("deltaMode=2（页）归一：一页触发一档", () => {
    const { doc, win } = mount();
    doc.dispatch("wheel", ctrlWheel(-1, { deltaMode: 2 }));
    expect(reportCount(win)).toBe(1);
    expect(zoomOf(doc)).toBe(1.1);
  });

  it("触控板小 delta 累计到阈值才步进（100px/格），方向翻转自然抵消", () => {
    const { doc, win } = mount();
    doc.dispatch("wheel", ctrlWheel(-40));
    expect(reportCount(win)).toBe(0);
    doc.dispatch("wheel", ctrlWheel(-40));
    expect(reportCount(win)).toBe(0);
    doc.dispatch("wheel", ctrlWheel(-40)); // 累计 -120 ≥ 100 → 一档
    expect(reportCount(win)).toBe(1);
    expect(zoomOf(doc)).toBe(1.1);
    // 反向翻转抵消：+60 再 -60 净 0，不触发
    doc.dispatch("wheel", ctrlWheel(60));
    doc.dispatch("wheel", ctrlWheel(-60));
    expect(reportCount(win)).toBe(1);
  });

  it("stopImmediatePropagation：页面后注册的同 doc wheel 监听被阻断（接管语义）", () => {
    const { doc } = mount();
    // 模拟页面 body 尾脚本后注册的监听（晚于注入运行时注册）
    const pageHandler = vi.fn();
    doc.addEventListener("wheel", pageHandler);
    // Ctrl+wheel：注入监听 stopImmediatePropagation → 页面监听收不到
    doc.dispatch("wheel", ctrlWheel(-120));
    expect(pageHandler).not.toHaveBeenCalled();
    // 无修饰键透传路径不阻断：页面监听正常收到（正常滚动不受影响）
    doc.dispatch("wheel", makeWheelEvent({ deltaY: -120 }));
    expect(pageHandler).toHaveBeenCalledTimes(1);
  });

  it("下行复位：source+nonce+type 三重校验，命中则复位并上报", () => {
    const { doc, win } = mount();
    doc.dispatch("wheel", ctrlWheel(-120)); // zoom → 1.1
    expect(zoomOf(doc)).toBe(1.1);

    const msg = { type: RESET_MSG_TYPE, nonce: NONCE };
    // source 非 parent → 拒
    win.dispatchMessage({ not: "parent" }, msg);
    expect(zoomOf(doc)).toBe(1.1);
    // nonce 不符 → 拒
    win.dispatchMessage(win.parent, { ...msg, nonce: "badbad" });
    expect(zoomOf(doc)).toBe(1.1);
    // type 不符 → 拒
    win.dispatchMessage(win.parent, { type: ZOOM_MSG_TYPE, nonce: NONCE });
    expect(zoomOf(doc)).toBe(1.1);
    // 合法复位 → zoom=1 + 上行上报
    const countBefore = reportCount(win);
    win.dispatchMessage(win.parent, msg);
    expect(zoomOf(doc)).toBe(1);
    expect(reportCount(win)).toBe(countBefore + 1);
    expect(win.parent.postMessage).toHaveBeenLastCalledWith(
      { type: ZOOM_MSG_TYPE, nonce: NONCE, zoom: 1 },
      "*",
    );
  });

  it("zoom 已为 1 时下行复位静默（无上报回声）", () => {
    const { doc, win } = mount();
    win.dispatchMessage(win.parent, { type: RESET_MSG_TYPE, nonce: NONCE });
    expect(reportCount(win)).toBe(0);
    expect(doc.documentElement.style.zoom).toBeUndefined();
  });

  it("metaKey（Mac ⌘）与 ctrlKey 等价触发", () => {
    const { doc, win } = mount();
    doc.dispatch("wheel", makeWheelEvent({ metaKey: true, deltaY: -120 }));
    expect(reportCount(win)).toBe(1);
    expect(zoomOf(doc)).toBe(1.1);
  });

  it("生成的源码不包含 </script> 字面量（防提前闭合注入脚本）", () => {
    const src = buildZoomRuntimeSource(NONCE);
    expect(src).not.toContain("</script");
    // 含 wheel 与 message 两处监听注册
    expect(src).toContain('addEventListener("wheel"');
    expect(src).toContain('addEventListener("message"');
  });
});
