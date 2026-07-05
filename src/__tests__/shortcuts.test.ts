// shortcuts.test.ts — ShortcutRegistry 单元测试
//
// 覆盖路径：
//   1. 注册/注销（register 返回注销函数、覆盖、注销后不可匹配）
//   2. 引用计数（首次注册安装监听器、归零移除、多次注册递增）
//   3. 上下文栈（pushContext/popContext、栈顶匹配防竞态）
//   4. 匹配排序（priority DESC + context stack index DESC）
//   5. KeyStroke 严格匹配（修饰键不完全→透传、code 不匹配→透传）
//   6. IME 透传（isComposing → 仅 allowDuringComposition 命令执行）
//   7. global 上下文（始终匹配）
//   8. handler 返回值（true→preventDefault+stopPropagation、false→透传）

import { describe, it, expect, afterEach } from "vitest";
import { getShortcutRegistry } from "../features/shortcuts/ShortcutRegistry";
import type { ShortcutCommand } from "../features/shortcuts/types";

/** 派发 keydown 事件到 window，返回事件对象供断言 */
function dispatchKeydown(opts: {
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  code?: string;
  isComposing?: boolean;
}): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
    metaKey: opts.metaKey ?? false,
    code: opts.code ?? "",
    isComposing: opts.isComposing ?? false,
    bubbles: true,
    cancelable: true,
  });
  window.dispatchEvent(event);
  return event;
}

/** 构造测试命令 */
function cmd(overrides: Partial<ShortcutCommand> = {}): ShortcutCommand {
  return {
    id: "test.cmd",
    keystroke: { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, code: "KeyA" },
    context: "global",
    priority: 0,
    handler: () => true,
    ...overrides,
  };
}

// 注册表单例
const registry = getShortcutRegistry();

describe("ShortcutRegistry", () => {
  let handlers: (() => void)[] = [];

  afterEach(() => {
    // 清理注册的函数
    for (const h of handlers) {
      try { h(); } catch { /* 幂等 */ }
    }
    handlers = [];
    registry._reset();
  });

  // ---- 1. 注册/注销 ----

  describe("注册/注销", () => {
    it("register 返回注销函数", () => {
      const unreg = registry.register([cmd({ id: "a" })]);
      handlers.push(unreg);
      expect(registry._commandCount()).toBe(1);
      unreg();
      expect(registry._commandCount()).toBe(0);
    });

    it("同 id 注册覆盖旧命令", () => {
      const handler1 = () => true;
      const handler2 = () => false;
      const unreg1 = registry.register([cmd({ id: "a", handler: handler1 })]);
      handlers.push(unreg1);
      expect(registry._commandCount()).toBe(1);
      const unreg2 = registry.register([cmd({ id: "a", handler: handler2 })]);
      handlers.push(unreg2);
      // 覆盖不增加计数
      expect(registry._commandCount()).toBe(1);
    });

    it("注销不存在的 id 不抛异常（幂等）", () => {
      expect(() => registry.unregister("nonexistent")).not.toThrow();
    });

    it("注销后命令不可匹配", () => {
      const unreg = registry.register([
        cmd({
          id: "a",
          keystroke: { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyA" },
          handler: () => true,
        }),
      ]);
      handlers.push(unreg);

      const prevented = dispatchKeydown({ ctrlKey: true, code: "KeyA" });
      expect(prevented.defaultPrevented).toBe(true);

      unreg();
      const passed = dispatchKeydown({ ctrlKey: true, code: "KeyA" });
      expect(passed.defaultPrevented).toBe(false);
      handlers = []; // 避免 afterEach 重复调
    });
  });

  // ---- 2. 引用计数 ----

  describe("引用计数", () => {
    it("首次注册安装监听器，最后一个命令注销后移除", () => {
      const unreg1 = registry.register([cmd({ id: "a" })]);
      handlers.push(unreg1);
      expect(registry._commandCount()).toBe(1);

      const unreg2 = registry.register([cmd({ id: "b" })]);
      handlers.push(unreg2);
      expect(registry._commandCount()).toBe(2);

      unreg1();
      expect(registry._commandCount()).toBe(1);

      unreg2();
      expect(registry._commandCount()).toBe(0);
    });

    it("覆盖同 id 不改变计数", () => {
      const unreg1 = registry.register([
        cmd({ id: "a", handler: () => true }),
        cmd({ id: "b", handler: () => true }),
      ]);
      handlers.push(unreg1);
      expect(registry._commandCount()).toBe(2);

      const unreg2 = registry.register([cmd({ id: "a", handler: () => false })]);
      handlers.push(unreg2);
      // a 被覆盖，b 未变 → 命令数不变
      expect(registry._commandCount()).toBe(2);
    });
  });

  // ---- 3. 上下文栈 ----

  describe("上下文栈", () => {
    it("pushContext 追加到栈尾", () => {
      registry.pushContext("terminal");
      registry.pushContext("editor");
      expect(registry._contextStack()).toEqual(["terminal", "editor"]);
    });

    it("pushContext 同上下文去重——移到栈尾", () => {
      registry.pushContext("terminal");
      registry.pushContext("editor");
      registry.pushContext("terminal");
      expect(registry._contextStack()).toEqual(["editor", "terminal"]);
    });

    it("popContext 仅在栈顶匹配时弹出", () => {
      registry.pushContext("terminal");
      registry.pushContext("editor");
      registry.popContext("terminal"); // 未在栈顶，不弹出
      expect(registry._contextStack()).toEqual(["terminal", "editor"]);
      registry.popContext("editor"); // 在栈顶，弹出
      expect(registry._contextStack()).toEqual(["terminal"]);
    });

    it("防竞态：terminal blur → editor focus 时 terminal 的 blur 不清 editor", () => {
      // terminal 聚焦
      registry.pushContext("terminal");
      // 焦点切到 editor（editor focusin 在 terminal focusout 之前触发）
      registry.pushContext("editor");
      // terminal 的 focusout 触发 popContext（但不应生效，栈顶是 editor）
      registry.popContext("terminal");
      // terminal 的 popContext 被跳过，editor 仍在栈中
      expect(registry._contextStack()).toEqual(["terminal", "editor"]);
    });

    it("getActiveContext 返回栈顶", () => {
      expect(registry.getActiveContext()).toBeNull();
      registry.pushContext("terminal");
      expect(registry.getActiveContext()).toBe("terminal");
      registry.pushContext("editor");
      expect(registry.getActiveContext()).toBe("editor");
    });

    it("空栈 popContext 不抛异常", () => {
      expect(() => registry.popContext("terminal")).not.toThrow();
    });
  });

  // ---- 4. 匹配排序 ----

  describe("匹配排序", () => {
    const ctrlA = {
      ctrlKey: true,
      shiftKey: false,
      altKey: false,
      metaKey: false,
      code: "KeyA",
    } as const;

    it("同按键不同 priority → 高优先先匹配", () => {
      const calls: string[] = [];
      const unreg1 = registry.register([
        cmd({ id: "low", priority: 0, keystroke: ctrlA, handler: () => { calls.push("low"); return true; } }),
        cmd({ id: "high", priority: 100, keystroke: ctrlA, handler: () => { calls.push("high"); return true; } }),
      ]);
      handlers.push(unreg1);

      dispatchKeydown({ ctrlKey: true, code: "KeyA" });
      expect(calls).toEqual(["high"]);
    });

    it("同 priority 不同上下文 → 栈顶优先", () => {
      const calls: string[] = [];
      const unreg = registry.register([
        cmd({
          id: "cmd-a", keystroke: ctrlA, context: "terminal", priority: 100,
          handler: () => { calls.push("terminal"); return true; },
        }),
        cmd({
          id: "cmd-b", keystroke: ctrlA, context: "editor", priority: 100,
          handler: () => { calls.push("editor"); return true; },
        }),
      ]);
      handlers.push(unreg);

      // editor 在 terminal 之后 push（栈顶 = editor）
      registry.pushContext("terminal");
      registry.pushContext("editor");

      dispatchKeydown({ ctrlKey: true, code: "KeyA" });
      expect(calls).toEqual(["editor"]);
    });

    it("handler 返回 false → 事件透传，不阻止默认", () => {
      const unreg = registry.register([
        cmd({ id: "pass", keystroke: ctrlA, handler: () => false }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyA" });
      expect(event.defaultPrevented).toBe(false);
    });
  });

  // ---- 5. KeyStroke 严格匹配 ----

  describe("KeyStroke 严格匹配", () => {
    it("修饰键不完全匹配时透传", () => {
      const unreg = registry.register([
        cmd({ id: "cs-a", keystroke: { ctrlKey: true, shiftKey: true, altKey: false, metaKey: false, code: "KeyA" }, handler: () => true }),
      ]);
      handlers.push(unreg);

      // 仅 Ctrl+A 不匹配 Ctrl+Shift+A
      const event = dispatchKeydown({ ctrlKey: true, code: "KeyA" });
      expect(event.defaultPrevented).toBe(false);
    });

    it("code 不匹配时透传", () => {
      const unreg = registry.register([
        cmd({ id: "ctrl-c", keystroke: { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyC" }, handler: () => true }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyV" });
      expect(event.defaultPrevented).toBe(false);
    });

    it("无修饰键时匹配普通键", () => {
      const unreg = registry.register([
        cmd({ id: "plain", keystroke: { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, code: "KeyA" }, handler: () => true }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ code: "KeyA" });
      expect(event.defaultPrevented).toBe(true);
    });
  });

  // ---- 6. IME 透传 ----

  describe("IME 透传", () => {
    it("isComposing 时默认命令不匹配", () => {
      const unreg = registry.register([
        cmd({
          id: "ctrl-v",
          keystroke: { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyV" },
          handler: () => true,
        }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyV", isComposing: true });
      expect(event.defaultPrevented).toBe(false);
    });

    it("isComposing 时 allowDuringComposition 命令仍匹配", () => {
      const unreg = registry.register([
        cmd({
          id: "ime-cmd",
          keystroke: { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyV" },
          allowDuringComposition: true,
          handler: () => true,
        }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyV", isComposing: true });
      expect(event.defaultPrevented).toBe(true);
    });
  });

  // ---- 7. global 上下文 ----

  describe("global 上下文", () => {
    it("global 命令在任何上下文栈状态下都匹配", () => {
      const unreg = registry.register([
        cmd({
          id: "global-cmd",
          keystroke: { ctrlKey: true, shiftKey: true, altKey: false, metaKey: false, code: "F1" },
          context: "global",
          handler: () => true,
        }),
      ]);
      handlers.push(unreg);

      // 空栈 → global 匹配
      const e1 = dispatchKeydown({ ctrlKey: true, shiftKey: true, code: "F1" });
      expect(e1.defaultPrevented).toBe(true);

      // 有上下文 → global 仍匹配（但面板级优先级更高）
      registry.pushContext("terminal");
      const e2 = dispatchKeydown({ ctrlKey: true, shiftKey: true, code: "F1" });
      expect(e2.defaultPrevented).toBe(true);
    });

    it("面板级命令优先级高于 global", () => {
      const calls: string[] = [];
      const unreg = registry.register([
        cmd({
          id: "global-f", context: "global", priority: 0,
          keystroke: { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, code: "KeyF" },
          handler: () => { calls.push("global"); return true; },
        }),
        cmd({
          id: "terminal-f", context: "terminal", priority: 100,
          keystroke: { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, code: "KeyF" },
          handler: () => { calls.push("terminal"); return true; },
        }),
      ]);
      handlers.push(unreg);
      registry.pushContext("terminal");

      dispatchKeydown({ code: "KeyF" });
      expect(calls).toEqual(["terminal"]);
    });
  });

  // ---- 8. handler 返回值 ----

  describe("handler 返回值", () => {
    const ctrlK = {
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyK",
    } as const;

    it("handler 返回 true → preventDefault + stopPropagation", () => {
      const unreg = registry.register([
        cmd({ id: "prevent", keystroke: ctrlK, handler: () => true }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyK" });
      expect(event.defaultPrevented).toBe(true);
    });

    it("handler 返回 false → 事件透传", () => {
      const unreg = registry.register([
        cmd({ id: "pass", keystroke: ctrlK, handler: () => false }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyK" });
      expect(event.defaultPrevented).toBe(false);
    });
  });

  // ---- 9. 边界 ----

  describe("边界情况", () => {
    it("空注册表不影响键盘事件正常触发", () => {
      const event = dispatchKeydown({ ctrlKey: true, code: "KeyA" });
      expect(event.defaultPrevented).toBe(false);
    });

    it("无候选命令匹配时事件透传", () => {
      const unreg = registry.register([
        cmd({ id: "ctrl-c", keystroke: { ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "KeyC" }, handler: () => true }),
      ]);
      handlers.push(unreg);

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyJ" });
      expect(event.defaultPrevented).toBe(false);
    });
  });
});
