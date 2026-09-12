// doc-viewer-injection.test.ts — docViewer 注入脚本组装测试
//
// buildInjectedScript 基础段 + 可选段（fragmentNav/linkRouter/scrollReport）：
//   - 各段标记字符串存在、拼接可解析（H7 parse-only 同款——防段边界 SyntaxError）
//   - md 场景（linkRouter + scrollReport）与 html 场景（fragmentNav）段组合隔离
//   - 注入产物 script 标签计数锁（FE-06：三组合矩阵各恰一对 <script>/</script>）
//   - scrollRuntime 桩执行最小集：节流上行 + 下行恢复校验（zoom 同范式完整桩
//     见 doc-viewer-zoom-runtime.test.ts）

import { describe, it, expect, vi } from "vitest";
import {
  buildInjectedScript,
  buildKeyForwardSource,
} from "../panels/docViewer/buildInjectedScript";
import { buildScrollRuntimeSource } from "../panels/docViewer/scrollRuntime";
import {
  SCROLL_MSG_TYPE,
  SCROLL_SET_MSG_TYPE,
  NAV_MSG_TYPE,
  FONT_PROBE_MSG_TYPE,
  KEY_FWD_MSG_TYPE,
} from "../panels/docViewer/previewMessages";

const NONCE = "00ff00ff00ff00ff00ff00ff00ff00ff";
// 已退役类型名（拼接构造——CP-013 验证 grep 该名于 src/ 须零命中，勿写成字面量）
const RETIRED_KEY_TYPE = ["slterm", "key"].join("_");
const RETIRED_MARKER = ["__slterm", "postMessage"].join("_");

/** 取注入脚本体并 parse-only 校验（SyntaxError 防线） */
function parseScript(script: string): void {
  const body = /<script>([\s\S]*?)<\/script>/.exec(script)?.[1];
  expect(body).toBeDefined();
  expect(() => new Function(body!)).not.toThrow();
}

describe("buildInjectedScript 段组合", () => {
  it("无 extra：keyForward 基础段 + zoom 运行时恒注入（旧键转发类型零残留）", () => {
    const out = buildInjectedScript(NONCE, []);
    // keyForward 基础段（ADR-0021/D2 收窄转发）恒首段：keydown 捕获 + 表单跳过
    expect(out).toContain("sltermKeyForward(document,window)");
    expect(out).toContain(KEY_FWD_MSG_TYPE);
    expect(out).toContain('addEventListener("keydown"');
    expect(out).toContain("isContentEditable");
    expect(out).toContain("sltermZoom(document,window)");
    // 旧键转发通道（slterm_key 字面量，拼接构造 + 负向前瞻——新 keyfwd 类型
    // 含 "slterm_key" 前缀子串，须精确排除旧名完整出现）与信任标记零残留
    expect(out).not.toMatch(new RegExp(RETIRED_KEY_TYPE + "(?!fwd)"));
    expect(out).not.toContain(RETIRED_MARKER);
    expect(out).not.toContain("fingerprint");
    // 段序锁死：keyForward 恒首段、zoom 恒末位（衔接断言 /\},true\);var sltermZoom=/
    // 在 html-panel 控制流用例——extra 段与 zoom 段直接相邻的拼接边界不动）
    expect(out.indexOf("var sltermKeyForward=")).toBeLessThan(out.indexOf("var sltermZoom="));
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

  it("md 追加 fontProbe 段（TE-08 分支 b）：iframe 内 fonts.check 后上行宿主页", () => {
    const out = buildInjectedScript(NONCE, [
      { kind: "linkRouter" },
      { kind: "scrollReport" },
      { kind: "fontProbe" },
    ]);
    // 锚点取 iframe 内字体加载态（宿主 FontFaceSet 不覆盖 iframe 文档——实证
    // 宿主 check 双真空，只能自 iframe 内取后 postMessage 上行）
    expect(out).toContain("document.fonts.ready.then");
    expect(out).toContain("document.fonts.check('12px \"KaTeX_Main\"')");
    expect(out).toContain("parent.postMessage");
    // 类型 + nonce（JSON.stringify 拼入——拼接纪律 #5 双保险）
    expect(out).toContain(FONT_PROBE_MSG_TYPE);
    expect(out).toContain(`nonce:${JSON.stringify(NONCE)}`);
    // 未传段零注入（生产拼装不含该段）
    expect(buildInjectedScript(NONCE, [{ kind: "linkRouter" }])).not.toContain(FONT_PROBE_MSG_TYPE);
    parseScript(out);
  });

  it("注入产物不含提前闭合——<script> 与 </script> 各恰好一次（段组合矩阵）", () => {
    // FE-06：拼接纪律 #1 测试锁——非贪婪 parse 只取首段，提前闭合截断静默；
    // 计数断言锁死「注入产物整体恰一对 script 标签」（buildInjectedScript.ts:13-30 纪律 1）
    const combos: Array<[string, Parameters<typeof buildInjectedScript>[1]]> = [
      ["无 extra 段", []],
      ["html fragmentNav", [{ kind: "fragmentNav" }]],
      ["md linkRouter+scrollReport", [{ kind: "linkRouter" }, { kind: "scrollReport" }]],
      [
        "md linkRouter+scrollReport+fontProbe",
        [{ kind: "linkRouter" }, { kind: "scrollReport" }, { kind: "fontProbe" }],
      ],
    ];
    for (const [name, extra] of combos) {
      const out = buildInjectedScript(NONCE, extra);
      expect(out.match(/<script>/g), `${name}：开标签恰一次`).toHaveLength(1);
      expect(out.match(/<\/script>/g), `${name}：闭标签恰一次`).toHaveLength(1);
    }
  });

  it("nonce 拼入 nav 消息（SEC-04 防伪造）", () => {
    const out = buildInjectedScript(NONCE, [{ kind: "linkRouter" }]);
    expect(out).toContain(`nonce:"${NONCE}"`);
  });
});

describe("keyForward 基础段桩执行（ADR-0021/D2 收窄转发）", () => {
  /** 取回运行时函数（new Function——zoomRuntime/scrollRuntime 同范式） */
  function loadRuntime(): (doc: StubDoc, win: StubWin) => void {
    const fn = new Function(
      `return (${buildKeyForwardSource(NONCE)})`,
    ) as () => (doc: StubDoc, win: StubWin) => void;
    return fn();
  }

  interface KeyEvt {
    target: { tagName?: string; isContentEditable?: boolean };
    code: string;
    ctrlKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
    metaKey: boolean;
  }
  interface StubDoc {
    addEventListener(type: string, fn: (e: KeyEvt) => void, capture?: boolean): void;
    keyHandlers: Array<(e: KeyEvt) => void>;
    captureFlags: boolean[];
  }
  interface StubWin {
    parent: { postMessage: ReturnType<typeof vi.fn> };
  }

  function makeDoc(): StubDoc {
    const keyHandlers: Array<(e: KeyEvt) => void> = [];
    const captureFlags: boolean[] = [];
    return {
      addEventListener(type, fn, capture) {
        expect(type).toBe("keydown");
        keyHandlers.push(fn);
        captureFlags.push(capture === true);
      },
      keyHandlers,
      captureFlags,
    };
  }
  function makeWin(): StubWin {
    return { parent: { postMessage: vi.fn() } };
  }
  function keyEvent(target: KeyEvt["target"], over: Partial<KeyEvt> = {}): KeyEvt {
    return {
      target,
      code: "KeyW",
      ctrlKey: true,
      shiftKey: false,
      altKey: false,
      metaKey: false,
      ...over,
    };
  }

  it("非表单焦点 keydown → 上行按键描述（capture 段 + 完整修饰键）", () => {
    const doc = makeDoc();
    const win = makeWin();
    loadRuntime()(doc, win);
    expect(doc.captureFlags).toEqual([true]); // 捕获阶段
    doc.keyHandlers[0]!(keyEvent({ tagName: "DIV" }));
    expect(win.parent.postMessage).toHaveBeenCalledTimes(1);
    expect(win.parent.postMessage).toHaveBeenCalledWith(
      {
        type: KEY_FWD_MSG_TYPE,
        nonce: NONCE,
        code: "KeyW",
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        metaKey: false,
      },
      "*",
    );
  });

  it("表单焦点（INPUT/TEXTAREA/SELECT/contenteditable）不转发——键入/复制解禁", () => {
    const doc = makeDoc();
    const win = makeWin();
    loadRuntime()(doc, win);
    const handler = doc.keyHandlers[0]!;
    handler(keyEvent({ tagName: "INPUT" }));
    handler(keyEvent({ tagName: "TEXTAREA" }));
    handler(keyEvent({ tagName: "SELECT" }));
    handler(keyEvent({ tagName: "DIV", isContentEditable: true }));
    expect(win.parent.postMessage).not.toHaveBeenCalled();
  });

  it("无 target 兜底转发（document 级按键）+ 不 preventDefault 语义（无该调用）", () => {
    const doc = makeDoc();
    const win = makeWin();
    loadRuntime()(doc, win);
    doc.keyHandlers[0]!(keyEvent(null as unknown as KeyEvt["target"]));
    expect(win.parent.postMessage).toHaveBeenCalledTimes(1);
    // 段源码不含 preventDefault——文档内默认行为保留（不双重触发）
    expect(buildKeyForwardSource(NONCE)).not.toContain("preventDefault");
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
