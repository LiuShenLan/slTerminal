// doc-viewer-injection.test.ts — docViewer 注入脚本组装测试
//
// buildInjectedScript 基础段 + 可选段（fragmentNav/linkRouter/scrollReport）：
//   - 各段标记字符串存在、拼接可解析（H7 parse-only 同款——防段边界 SyntaxError）
//   - md 场景（linkRouter + scrollReport）与 html 场景（fragmentNav）段组合隔离
//   - scrollRuntime 桩执行最小集：节流上行 + 下行恢复校验（zoom 同范式完整桩
//     见 doc-viewer-zoom-runtime.test.ts）

import { describe, it, expect, vi } from "vitest";
import { buildInjectedScript } from "../panels/docViewer/buildInjectedScript";
import { buildScrollRuntimeSource } from "../panels/docViewer/scrollRuntime";
import {
  SCROLL_MSG_TYPE,
  SCROLL_SET_MSG_TYPE,
  NAV_MSG_TYPE,
} from "../panels/docViewer/previewMessages";

const NONCE = "00ff00ff00ff00ff00ff00ff00ff00ff";

/** 取注入脚本体并 parse-only 校验（SyntaxError 防线） */
function parseScript(script: string): void {
  const body = /<script>([\s\S]*?)<\/script>/.exec(script)?.[1];
  expect(body).toBeDefined();
  expect(() => new Function(body!)).not.toThrow();
}

describe("buildInjectedScript 段组合", () => {
  it("基础段恒在：keydown 转发 + zoom 运行时（无 extra）", () => {
    const out = buildInjectedScript(NONCE, []);
    expect(out).toContain("slterm_key");
    expect(out).toContain("sltermZoom(document,window)");
    parseScript(out);
  });

  it("html 场景（fragmentNav）：片段拦截 + 无 linkRouter/scroll 标记", () => {
    const out = buildInjectedScript(NONCE, [{ kind: "fragmentNav" }]);
    expect(out).toContain("slterm-target");
    expect(out).toContain("scrollIntoView");
    expect(out).not.toContain(NAV_MSG_TYPE);
    expect(out).not.toContain(SCROLL_MSG_TYPE);
    parseScript(out);
  });

  it("md 场景（linkRouter + scrollReport）：nav/scroll 标记 + 无 fragment 段", () => {
    const out = buildInjectedScript(NONCE, [
      { kind: "linkRouter" },
      { kind: "scrollReport" },
    ]);
    // linkRouter：拦截非 # 链接 preventDefault + 上行 slterm_nav
    expect(out).toContain(NAV_MSG_TYPE);
    expect(out).toContain('href:h},"*")');
    expect(out).toMatch(/closest\("a"\)/);
    // scrollReport：sltermScroll 挂载
    expect(out).toContain("sltermScroll(document,window)");
    expect(out).toContain(SCROLL_MSG_TYPE);
    expect(out).toContain(SCROLL_SET_MSG_TYPE);
    // fragment 段不注入
    expect(out).not.toContain("slterm-target");
    parseScript(out);
  });

  it("nonce 拼入 nav 消息（SEC-04 防伪造）", () => {
    const out = buildInjectedScript(NONCE, [{ kind: "linkRouter" }]);
    expect(out).toContain(`nonce:"${NONCE}"`);
  });
});

describe("scrollRuntime 桩执行", () => {
  /** 取回运行时函数（new Function——doc-viewer-zoom-runtime 同范式） */
  function loadRuntime(): (doc: StubDoc, win: StubWin) => void {
    const fn = new Function(
      `return (${buildScrollRuntimeSource(NONCE)})`,
    ) as () => (doc: StubDoc, win: StubWin) => void;
    return fn();
  }

  interface StubDoc {
    documentElement: { scrollTop: number; scrollHeight: number; clientHeight: number };
    addEventListener(type: string, fn: (e?: unknown) => void): void;
    scrollHandlers: Array<(e?: unknown) => void>;
  }
  interface StubWin {
    parent: { postMessage: ReturnType<typeof vi.fn> };
    addEventListener(type: string, fn: (e: { source: unknown; data: unknown }) => void): void;
    msgHandlers: Array<(e: { source: unknown; data: unknown }) => void>;
  }

  function makeDoc(scrollHeight = 2000): StubDoc {
    const scrollHandlers: Array<(e?: unknown) => void> = [];
    return {
      documentElement: { scrollTop: 0, scrollHeight, clientHeight: 800 },
      addEventListener(_type, fn) {
        scrollHandlers.push(fn);
      },
      scrollHandlers,
    };
  }
  function makeWin(): StubWin {
    const msgHandlers: Array<(e: { source: unknown; data: unknown }) => void> = [];
    return {
      parent: { postMessage: vi.fn() },
      addEventListener(_type, fn) {
        msgHandlers.push(fn);
      },
      msgHandlers,
    };
  }

  it("scroll 事件节流上报比例（120ms 防抖收敛）", () => {
    vi.useFakeTimers();
    try {
      const doc = makeDoc();
      const win = makeWin();
      loadRuntime()(doc, win);
      doc.documentElement.scrollTop = 600;
      // 连续 scroll 事件（拖动）：节流期内只调度一次上报
      doc.scrollHandlers[0]!();
      doc.scrollHandlers[0]!();
      expect(win.parent.postMessage).not.toHaveBeenCalled();
      vi.advanceTimersByTime(120);
      expect(win.parent.postMessage).toHaveBeenCalledTimes(1);
      // 载荷：比例 = 600/(2000-800) = 0.5 + nonce（SEC-04）
      expect(win.parent.postMessage).toHaveBeenCalledWith(
        { type: SCROLL_MSG_TYPE, nonce: NONCE, ratio: 0.5 },
        "*",
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it("下行恢复：slterm_scroll_set 校验通过 → 60ms 后按比例设 scrollTop", () => {
    vi.useFakeTimers();
    try {
      const doc = makeDoc();
      const win = makeWin();
      loadRuntime()(doc, win);
      // 下行（source = 父窗口 + nonce + type 匹配）
      const handler = win.msgHandlers[0]!;
      handler({
        source: win.parent,
        data: { type: SCROLL_SET_MSG_TYPE, nonce: NONCE, ratio: 0.25 },
      });
      // 60ms 延时后应用
      vi.advanceTimersByTime(60);
      expect(doc.documentElement.scrollTop).toBeCloseTo(0.25 * 1200);
    } finally {
      vi.useRealTimers();
    }
  });

  it("下行校验负面：伪造 nonce / 越界比例不生效（合法 source 下仍拒绝）", () => {
    vi.useFakeTimers();
    try {
      const doc = makeDoc();
      const win = makeWin();
      loadRuntime()(doc, win);
      const handler = win.msgHandlers[0]!;
      handler({
        source: win.parent,
        data: { type: SCROLL_SET_MSG_TYPE, nonce: "bad", ratio: 0.5 },
      });
      handler({
        source: win.parent,
        data: { type: SCROLL_SET_MSG_TYPE, nonce: NONCE, ratio: 5 },
      });
      vi.advanceTimersByTime(120);
      expect(doc.documentElement.scrollTop).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });
});
