// keyboard.test.ts — 终端键盘事件处理单元测试
//
// 覆盖路径：
//   1. IME 组合态透传
//   2. Ctrl+Shift+C 选中复制
//   3. Ctrl+Shift+C 无选中不写剪贴板
//   4. Ctrl+Shift+V 粘贴
//   5. 终端 null 守卫
//   6. 非目标组合键透传
//   7. 剪贴板 API 拒绝时的 fallback
//   8. setActiveTerminal(null) 注销后透传

import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
import { setActiveTerminal, installKeyboardHandler } from "../panels/terminal/keyboard";
import type { Terminal } from "@xterm/xterm";

// ====== 辅助工具 ======

/** 构造满足测试需求的 Terminal stub */
function terminalStub(opts?: {
  getSelection?: () => string;
  paste?: (text: string) => void;
}): Terminal {
  return {
    getSelection: opts?.getSelection ?? (() => ""),
    paste: opts?.paste ?? vi.fn(),
  } as unknown as Terminal;
}

/** 派发 keydown 事件到 window，返回事件对象以供断言 */
function dispatchKeydown(opts: {
  ctrlKey?: boolean;
  shiftKey?: boolean;
  code?: string;
  isComposing?: boolean;
}): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    code: opts.code ?? "",
    isComposing: opts.isComposing ?? false,
    bubbles: true,
    cancelable: true,
  });
  window.dispatchEvent(event);
  return event;
}

// ====== 全局 clipboard mock ======

let writeTextMock: ReturnType<typeof vi.fn>;
let readTextMock: ReturnType<typeof vi.fn>;

// ====== 测试套件 ======

describe("keyboard 终端键盘事件处理", () => {
  beforeAll(() => {
    // jsdom 中 navigator.clipboard.writeText/readText 未实现，手动提供
    writeTextMock = vi.fn().mockResolvedValue(undefined);
    readTextMock = vi.fn().mockResolvedValue("");
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: writeTextMock, readText: readTextMock },
      writable: true,
      configurable: true,
    });
    // 安装全局捕获阶段键盘处理器（installed 守卫防止重复安装）
    installKeyboardHandler();
  });

  beforeEach(() => {
    writeTextMock.mockClear();
    readTextMock.mockClear();
    writeTextMock.mockResolvedValue(undefined);
    readTextMock.mockResolvedValue("");
  });

  afterEach(() => {
    // 每个测试后注销终端引用，避免测试间交叉污染
    setActiveTerminal(null);
  });

  // ========== 1. IME 组合态透传 ==========

  describe("IME 组合态", () => {
    it("isComposing 为 true → 不阻止默认，全透传", () => {
      setActiveTerminal(terminalStub());

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
        isComposing: true,
      });

      expect(event.defaultPrevented).toBe(false);
    });
  });

  // ========== 5. 终端 null 守卫 ==========

  describe("终端为 null", () => {
    it("activeTerminal 未注册 → 所有按键透传", () => {
      setActiveTerminal(null);

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });

      expect(event.defaultPrevented).toBe(false);
    });
  });

  // ========== 2/3/7. Ctrl+Shift+C 复制 ==========

  describe("Ctrl+Shift+C 复制选中文本", () => {
    it("有选中文本 → 写入剪贴板并阻止默认行为", () => {
      setActiveTerminal(terminalStub({ getSelection: () => "selected text" }));

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });

      expect(event.defaultPrevented).toBe(true);
      expect(writeTextMock).toHaveBeenCalledWith("selected text");
    });

    it("无选中文本（空字符串）→ 不调用 writeText，但仍阻止默认", () => {
      setActiveTerminal(terminalStub({ getSelection: () => "" }));

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });

      expect(event.defaultPrevented).toBe(true);
      expect(writeTextMock).not.toHaveBeenCalled();
    });

    it("writeText 拒绝（如剪贴板权限不足）→ 错误被 .catch() 吞掉，事件仍阻止", () => {
      writeTextMock.mockRejectedValue(new Error("NotAllowedError"));
      setActiveTerminal(terminalStub({ getSelection: () => "text" }));

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });

      expect(event.defaultPrevented).toBe(true);
      expect(writeTextMock).toHaveBeenCalledWith("text");
      // 拒绝被 .catch(() => {}) 吞掉，不向上抛出
    });
  });

  // ========== 4/7. Ctrl+Shift+V 粘贴 ==========

  describe("Ctrl+Shift+V 粘贴", () => {
    it("读取剪贴板内容并 paste 到终端，阻止默认", async () => {
      readTextMock.mockResolvedValue("clipboard content");
      const pasteSpy = vi.fn();
      setActiveTerminal(terminalStub({ paste: pasteSpy }));

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyV",
      });

      expect(event.defaultPrevented).toBe(true);
      // paste 在 .then() 回调中异步执行
      await vi.waitFor(() => {
        expect(pasteSpy).toHaveBeenCalledWith("clipboard content");
      });
    });

    it("readText 拒绝 → 错误被 .catch() 吞掉，不调用 paste", async () => {
      readTextMock.mockRejectedValue(new Error("NotAllowedError"));
      const pasteSpy = vi.fn();
      setActiveTerminal(terminalStub({ paste: pasteSpy }));

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyV",
      });

      expect(event.defaultPrevented).toBe(true);
      // 微任务清空后 paste 不应被调用
      await new Promise<void>((r) => setTimeout(r, 0));
      expect(pasteSpy).not.toHaveBeenCalled();
    });
  });

  // ========== 6. 非目标组合键透传 ==========

  describe("非 Ctrl+Shift+C/V 组合键透传", () => {
    it("普通字母键 → 透传", () => {
      setActiveTerminal(terminalStub());

      const event = dispatchKeydown({ code: "KeyA" });
      expect(event.defaultPrevented).toBe(false);
    });

    it("Ctrl+C 无 Shift → 透传，保留为 SIGINT 中断", () => {
      setActiveTerminal(terminalStub());

      const event = dispatchKeydown({ ctrlKey: true, code: "KeyC" });
      expect(event.defaultPrevented).toBe(false);
    });

    it("Shift+C 无 Ctrl → 透传", () => {
      setActiveTerminal(terminalStub());

      const event = dispatchKeydown({ shiftKey: true, code: "KeyC" });
      expect(event.defaultPrevented).toBe(false);
    });

    it("Ctrl+Shift+N（非 C/V）→ 透传", () => {
      setActiveTerminal(terminalStub());

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyN",
      });
      expect(event.defaultPrevented).toBe(false);
    });
  });

  // ========== 8. setActiveTerminal(null) 注销 ==========

  describe("setActiveTerminal(null) 注销", () => {
    it("注销后先前被拦截的组合键恢复透传", () => {
      setActiveTerminal(terminalStub({ getSelection: () => "text" }));

      // 注册状态下 Ctrl+Shift+C 被拦截
      const prevented = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });
      expect(prevented.defaultPrevented).toBe(true);

      // 注销终端引用
      setActiveTerminal(null);

      // 注销后同一组合键透传
      const passed = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });
      expect(passed.defaultPrevented).toBe(false);
    });
  });
});
