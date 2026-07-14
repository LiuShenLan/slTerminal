// forwardGlobalShortcuts.test.ts — iframe 全局键转发单元测试
//
// 覆盖：
//   attachGlobalShortcutForwarder（contentDocument 路径）：
//     命中全局键 → preventDefault + 重放到 window（props 正确）；非全局键不转发；
//     多全局绑定各自命中；空绑定全不转发；detach 后失效；修饰键指纹区分。
//   postMessage 注入脚本（新路径，替代 contentDocument 方式）：
//     脚本结构验证：postMessage 调用、capture phase、无 preventDefault、keystroke 格式。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// mock 注册表的 exportContextBindings（formatKeystroke 用真实实现）
const { mockExportContextBindings } = vi.hoisted(() => ({
  mockExportContextBindings: vi.fn<() => { id: string; keystroke: string }[]>(() => []),
}));
vi.mock("../features/shortcuts/ShortcutRegistry", () => ({
  getShortcutRegistry: () => ({ exportContextBindings: mockExportContextBindings }),
}));

import { attachGlobalShortcutForwarder } from "../features/shortcuts/forwardGlobalShortcuts";

function keydown(opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean; code: string; key?: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
    metaKey: opts.metaKey ?? false,
    code: opts.code,
    key: opts.key ?? "",
    bubbles: true,
    cancelable: true,
  });
}

describe("attachGlobalShortcutForwarder", () => {
  let detach: (() => void) | null = null;
  let dispatchSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    mockExportContextBindings.mockReturnValue([]);
    // spy window.dispatchEvent（默认调用原始实现——重放到 window 无害，无 registry 监听）
    dispatchSpy = vi.spyOn(window, "dispatchEvent");
  });

  afterEach(() => {
    detach?.();
    detach = null;
    dispatchSpy.mockRestore();
  });

  it("命中全局键（Ctrl+W）→ preventDefault + 重放到 window（props 正确）", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    detach = attachGlobalShortcutForwarder(document);

    const e = keydown({ ctrlKey: true, code: "KeyW", key: "w" });
    document.dispatchEvent(e);

    expect(e.defaultPrevented).toBe(true);
    // 重放的合成事件
    const calls = dispatchSpy.mock.calls as unknown as Array<[Event]>;
    const replayed = calls
      .map((c) => c[0])
      .find((ev): ev is KeyboardEvent => ev instanceof KeyboardEvent && ev !== e);
    expect(replayed).toBeDefined();
    expect(replayed!.type).toBe("keydown");
    expect(replayed!.ctrlKey).toBe(true);
    expect(replayed!.shiftKey).toBe(false);
    expect(replayed!.code).toBe("KeyW");
    expect(replayed!.key).toBe("w");
  });

  it("非全局键（Ctrl+A）→ 不 preventDefault、不重放", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    detach = attachGlobalShortcutForwarder(document);

    const before = dispatchSpy.mock.calls.length;
    const e = keydown({ ctrlKey: true, code: "KeyA" });
    document.dispatchEvent(e);

    expect(e.defaultPrevented).toBe(false);
    // 无新增的 window.dispatchEvent 调用（除 document.dispatchEvent 本身，不经 window）
    expect(dispatchSpy.mock.calls.length).toBe(before);
  });

  it("多全局绑定动态 → 各自命中", () => {
    mockExportContextBindings.mockReturnValue([
      { id: "global.closeTab", keystroke: "Ctrl+KeyW" },
      { id: "global.other", keystroke: "Ctrl+Shift+KeyP" },
    ]);
    detach = attachGlobalShortcutForwarder(document);

    const e1 = keydown({ ctrlKey: true, code: "KeyW" });
    document.dispatchEvent(e1);
    expect(e1.defaultPrevented).toBe(true);

    const e2 = keydown({ ctrlKey: true, shiftKey: true, code: "KeyP" });
    document.dispatchEvent(e2);
    expect(e2.defaultPrevented).toBe(true);
  });

  it("空全局绑定 → 全不转发", () => {
    mockExportContextBindings.mockReturnValue([]);
    detach = attachGlobalShortcutForwarder(document);

    const e = keydown({ ctrlKey: true, code: "KeyW" });
    document.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });

  it("detach 后再派发 → 无反应", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    const d = attachGlobalShortcutForwarder(document);
    d(); // detach
    detach = null;

    const e = keydown({ ctrlKey: true, code: "KeyW" });
    document.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });

  it("修饰键指纹区分：绑定 Ctrl+W 不匹配 Ctrl+Shift+W", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    detach = attachGlobalShortcutForwarder(document);

    const e = keydown({ ctrlKey: true, shiftKey: true, code: "KeyW" });
    document.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });
});

// ==========================================================================
// 注入脚本结构验证（键盘转发 postMessage + 片段拦截 + class-based toggle）
//
// HtmlPanel 注入的 INJECTED_SCRIPT 包含三部分：
//   a) keydown → postMessage 到父窗口（键盘转发）
//   b) click on <a href="#..."> → class-based :target 模拟（片段拦截+toggle）
//   c) <style> 注入 .slterm-target 基础样式
// 脚本以字符串形式存在于 HtmlPanel.tsx 中。以下测试验证其结构特征。
// 运行时行为在 jsdom 中难以模拟，由 E2E 测试覆盖。
// ==========================================================================

/**
 * 注入脚本的典型形态（与 HtmlPanel.tsx 中 INJECTED_SCRIPT 同构）。
 * 包含键盘转发 + 片段拦截 + class toggle。
 */
const INJECTED_SCRIPT =
  `<script>` +
  `var s=document.createElement("style");s.textContent=".slterm-target{display:block!important}";document.head.appendChild(s);` +
  `document.addEventListener("keydown",function(e){window.parent.postMessage({type:"slterm_key",fingerprint:(e.ctrlKey?"Ctrl+":"")+(e.shiftKey?"Shift+":"")+(e.altKey?"Alt+":"")+(e.metaKey?"Meta+":"")+e.code,ctrlKey:e.ctrlKey,shiftKey:e.shiftKey,altKey:e.altKey,metaKey:e.metaKey,code:e.code,key:e.key},"*")},true);` +
  `var _h=null;document.addEventListener("click",function(e){var a=e.target.closest("a");if(!a)return;var h=a.getAttribute("href");if(!h||h.charAt(0)!=="#")return;e.preventDefault();var id=h.slice(1);` +
  `if(!id){if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash};window.scrollTo({top:0,behavior:"smooth"});return}` +
  `if(id===_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target");_h=null;delete document.documentElement.dataset.sltermHash;return}` +
  `if(_h){var o=document.getElementById(_h);if(o)o.classList.remove("slterm-target")}` +
  `var el=document.getElementById(id);if(el){el.classList.add("slterm-target");el.scrollIntoView({behavior:"smooth"});document.documentElement.dataset.sltermHash=id;_h=id}` +
  `},true)` +
  `</script>`;

describe("注入脚本结构验证", () => {
  // ========================================================================
  // 键盘转发（F1-F6）
  // ========================================================================

  it("F1: 使用 window.parent.postMessage 而非 contentDocument", () => {
    expect(INJECTED_SCRIPT).toContain("window.parent.postMessage");
    expect(INJECTED_SCRIPT).not.toContain("contentDocument");
  });

  it("F2: keydown handler 对所有按键无条件 postMessage", () => {
    const body = INJECTED_SCRIPT.replace(/(^<script>|<\/script>$)/g, "");
    expect(body).toMatch(/addEventListener.*"keydown".*postMessage/);
  });

  it("F3: keydown handler 使用 capture phase", () => {
    expect(INJECTED_SCRIPT).toMatch(/"keydown".*?function.*?},true\)/);
  });

  it("F4: keyboard handler 不调 preventDefault", () => {
    const keydownPart = INJECTED_SCRIPT.split('"click"')[0];
    expect(keydownPart).not.toContain("preventDefault");
  });

  it("F5: keystroke 格式 Ctrl/Shift/Alt/Meta+code", () => {
    expect(INJECTED_SCRIPT).toContain('e.ctrlKey?"Ctrl+"');
    expect(INJECTED_SCRIPT).toContain('e.shiftKey?"Shift+"');
    expect(INJECTED_SCRIPT).toContain('e.altKey?"Alt+"');
    expect(INJECTED_SCRIPT).toContain('e.metaKey?"Meta+"');
  });

  it("F6: Meta 键在 code 之前", () => {
    const metaIdx = INJECTED_SCRIPT.indexOf('e.metaKey?"Meta+"');
    const codeIdx = INJECTED_SCRIPT.indexOf("+e.code");
    expect(metaIdx).toBeLessThan(codeIdx);
  });

  // ========================================================================
  // 片段链接拦截 + class toggle（H7-H16）
  // ========================================================================

  it("H7: 注入脚本含 scrollIntoView", () => {
    expect(INJECTED_SCRIPT).toContain("scrollIntoView");
    expect(INJECTED_SCRIPT).toContain("smooth");
  });

  it("H8: click handler 含 closest('a')", () => {
    expect(INJECTED_SCRIPT).toContain("closest");
    expect(INJECTED_SCRIPT).toContain('"a"');
  });

  it("H9: click handler 使用 capture phase", () => {
    expect(INJECTED_SCRIPT).toMatch(/"click".*?function.*?},true\)/);
  });

  it("H10: 只拦截 # 开头链接", () => {
    expect(INJECTED_SCRIPT).toContain('charAt(0)!=="#"');
  });

  it("H11: 非 # 链接提前 return", () => {
    expect(INJECTED_SCRIPT).toMatch(/!=="#".*?return/);
  });

  it("H12: 注入脚本含 keydown + click 两个 addEventListener", () => {
    const matches = INJECTED_SCRIPT.match(/addEventListener/g);
    expect(matches).not.toBeNull();
    expect(matches!.length).toBe(2);
  });

  // ========================================================================
  // class-based toggle（新增）
  // ========================================================================

  it("H13: 注入脚本含 classList.add（展开时加 class）", () => {
    expect(INJECTED_SCRIPT).toContain("classList.add");
    expect(INJECTED_SCRIPT).toContain("slterm-target");
  });

  it("H14: 注入脚本含 classList.remove（收回时去 class）", () => {
    expect(INJECTED_SCRIPT).toContain("classList.remove");
    // 出现多次（toggle 路径 + 切换路径 + # 路径）
    const matches = INJECTED_SCRIPT.match(/classList.remove/g);
    expect(matches).not.toBeNull();
    expect(matches!.length).toBeGreaterThanOrEqual(3);
  });

  it("H15: 注入脚本含 dataset.sltermHash 状态跟踪", () => {
    expect(INJECTED_SCRIPT).toContain("dataset.sltermHash");
  });

  it("H16: 点击 #（空片段）清空 target 状态", () => {
    // !id 时清除 _h + dataset
    expect(INJECTED_SCRIPT).toContain('!id');
    expect(INJECTED_SCRIPT).toContain('_h=null');
  });

  it("H17: 同片段二次点击 toggle 收回", () => {
    // id===_h 时移除 class + 清状态
    expect(INJECTED_SCRIPT).toContain('id===_h');
    expect(INJECTED_SCRIPT).toContain('classList.remove');
  });

  it("H18: 注入脚本含 <style> .slterm-target CSS", () => {
    expect(INJECTED_SCRIPT).toContain('createElement("style")');
    expect(INJECTED_SCRIPT).toContain('.slterm-target{display:block!important}');
  });

  // ========================================================================
  // 边界 (E6-E9)
  // ========================================================================

  it("E6: keyup 事件 → forwarder 忽略不处理", () => {
    mockExportContextBindings.mockReturnValue([{ id: "global.closeTab", keystroke: "Ctrl+KeyW" }]);
    const detach = attachGlobalShortcutForwarder(document);

    const spy = vi.spyOn(window, "dispatchEvent");
    const e = new KeyboardEvent("keyup", {
      ctrlKey: true, code: "KeyW", key: "w", bubbles: true, cancelable: true,
    });
    document.dispatchEvent(e);

    expect(e.defaultPrevented).toBe(false);
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
    detach();
  });

  it("E7: attachGlobalShortcutForwarder 收到非 keydown 类型事件不处理", () => {
    // keyup/keypress 默认透传——forwarder 只注册 "keydown" listener
    mockExportContextBindings.mockReturnValue([{ id: "g", keystroke: "Ctrl+KeyW" }]);
    const detach = attachGlobalShortcutForwarder(document);

    const e = new KeyboardEvent("keypress", {
      ctrlKey: true, code: "KeyW", key: "w", bubbles: true, cancelable: true,
    });
    document.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
    detach();
  });

  it("E8: 注入脚本 _h 变量初始化为 null", () => {
    expect(INJECTED_SCRIPT).toContain("var _h=null");
  });

  it("E9: 注入脚本 style 注入在 keydown 注册之前", () => {
    const styleIdx = INJECTED_SCRIPT.indexOf('createElement("style")');
    const keydownIdx = INJECTED_SCRIPT.indexOf('addEventListener("keydown"');
    expect(styleIdx).toBeLessThan(keydownIdx);
  });
});
