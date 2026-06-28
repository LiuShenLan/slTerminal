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
//   9. navigator.clipboard 未定义时不崩溃（Tauri/WebView2 环境）

import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from "vitest";
import { setActiveTerminal, clearActiveTerminalIfMine, installKeyboardHandler, uninstallKeyboardHandler } from "../panels/terminal/keyboard";
import type { Terminal } from "@xterm/xterm";

// ====== mock tauri clipboard plugin ======

const { writeTextMock, readTextMock } = vi.hoisted(() => ({
  writeTextMock: vi.fn(),
  readTextMock: vi.fn(),
}));

vi.mock("../ipc/clipboard", () => ({
  writeText: writeTextMock,
  readText: readTextMock,
}));

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

// ====== 测试套件 ======

describe("keyboard 终端键盘事件处理", () => {
  beforeAll(() => {
    // mock 已在 vi.hoisted() 中创建，此处配置返回值
    writeTextMock.mockResolvedValue(undefined);
    readTextMock.mockResolvedValue("");
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
    // 重置键盘处理器状态：卸载（平衡 beforeAll 中的安装）→ 重新安装
    // 确保每个测试都以 handler 已安装的状态开始
    uninstallKeyboardHandler();
    installKeyboardHandler();
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

  // ========== 7a. Tauri 无 navigator.clipboard 场景 ==========

  describe("navigator.clipboard 在 Tauri/WebView2 中不可用时", () => {
    it("Ctrl+Shift+C 不依赖 navigator.clipboard，仍正常阻止默认事件", () => {
      // 模拟 Tauri 环境：移除 navigator.clipboard
      const originalClipboard = navigator.clipboard;
      Object.defineProperty(navigator, "clipboard", {
        value: undefined,
        writable: true,
        configurable: true,
      });

      setActiveTerminal(terminalStub({ getSelection: () => "selected" }));

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });

      expect(event.defaultPrevented).toBe(true);

      // 恢复 clipboard
      Object.defineProperty(navigator, "clipboard", {
        value: originalClipboard,
        writable: true,
        configurable: true,
      });
    });

    it("Ctrl+Shift+V 不依赖 navigator.clipboard，仍正常阻止默认事件", () => {
      const originalClipboard = navigator.clipboard;
      Object.defineProperty(navigator, "clipboard", {
        value: undefined,
        writable: true,
        configurable: true,
      });

      setActiveTerminal(terminalStub());

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyV",
      });

      expect(event.defaultPrevented).toBe(true);

      Object.defineProperty(navigator, "clipboard", {
        value: originalClipboard,
        writable: true,
        configurable: true,
      });
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

  // ========== 9. clearActiveTerminalIfMine 身份校验清空 ==========

  describe("clearActiveTerminalIfMine 身份校验", () => {
    it("28. 匹配终端 → 清空 activeTerminal", () => {
      const termA = terminalStub({ getSelection: () => "A" });
      setActiveTerminal(termA);

      clearActiveTerminalIfMine(termA);

      // 清空后 Ctrl+Shift+C 透传
      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(false);
    });

    it("29. 不匹配终端 → 不清空 activeTerminal", () => {
      const termA = terminalStub({ getSelection: () => "A" });
      const termB = terminalStub({ getSelection: () => "B" });
      setActiveTerminal(termA);

      // B 失焦，但 activeTerminal 是 A → 不应清空
      clearActiveTerminalIfMine(termB);

      // activeTerminal 仍是 A，Ctrl+Shift+C 仍操作 A
      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(true);
      expect(writeTextMock).toHaveBeenCalledWith("A");
    });

    it("30. activeTerminal 已为 null → 无操作（幂等）", () => {
      setActiveTerminal(null);
      const term = terminalStub();

      // 不应抛出异常
      expect(() => clearActiveTerminalIfMine(term)).not.toThrow();

      // activeTerminal 仍是 null
      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(false);
    });
  });

  // ========== 10. 多终端焦点迁移 ==========

  describe("多终端焦点迁移", () => {
    it("31. 焦点从终端 A 切换到 B → Ctrl+Shift+C 操作 B 的选中", () => {
      const termA = terminalStub({ getSelection: () => "text-from-A" });
      const termB = terminalStub({ getSelection: () => "text-from-B" });

      // 初始：A 创建 → A 聚焦
      setActiveTerminal(termA);
      const eventA = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(writeTextMock).toHaveBeenCalledWith("text-from-A");
      expect(eventA.defaultPrevented).toBe(true);

      // 切换到 B（A blur → B focus）
      setActiveTerminal(termB);
      writeTextMock.mockClear();
      const eventB = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(writeTextMock).toHaveBeenCalledWith("text-from-B");
      expect(eventB.defaultPrevented).toBe(true);
    });

    it("32. 终端 A 失焦后 Ctrl+Shift+C 透传", () => {
      const termA = terminalStub({ getSelection: () => "A" });
      setActiveTerminal(termA);

      // A 失焦（用户点击文件树等非终端区域）
      clearActiveTerminalIfMine(termA);

      const event = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(false);
      expect(writeTextMock).not.toHaveBeenCalled();
    });

    it("33. 焦点回到终端 A → Ctrl+Shift+C 再次操作 A", () => {
      const termA = terminalStub({ getSelection: () => "A-returns" });

      // A 聚焦 → 失焦 → 再次聚焦
      setActiveTerminal(termA);
      clearActiveTerminalIfMine(termA); // blur
      setActiveTerminal(termA);         // focus again
      writeTextMock.mockClear();

      const event = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(true);
      expect(writeTextMock).toHaveBeenCalledWith("A-returns");
    });

    it("35. 两个终端各有选中 → 只复制当前活跃终端的", () => {
      const termA = terminalStub({ getSelection: () => "A-selection" });
      const termB = terminalStub({ getSelection: () => "B-selection" });

      // A 活跃
      setActiveTerminal(termA);
      writeTextMock.mockClear();
      dispatchKeydown({ ctrlKey: true, shiftKey: true, code: "KeyC" });
      expect(writeTextMock).toHaveBeenCalledWith("A-selection");
      expect(writeTextMock).toHaveBeenCalledTimes(1);

      // 切换到 B
      setActiveTerminal(termB);
      writeTextMock.mockClear();
      dispatchKeydown({ ctrlKey: true, shiftKey: true, code: "KeyC" });
      expect(writeTextMock).toHaveBeenCalledWith("B-selection");
      expect(writeTextMock).toHaveBeenCalledTimes(1);
    });
  });

  // ========== 11. writeText 拒绝（生产级错误处理） ==========

  describe("Ctrl+Shift+C writeText 拒绝", () => {
    it("34. writeText 拒绝 → 事件仍阻止默认，不向上抛出", async () => {
      // 模拟剪贴板写入失败（如权限不足）
      writeTextMock.mockRejectedValue(new Error("NotAllowedError"));
      setActiveTerminal(terminalStub({ getSelection: () => "text" }));

      const event = dispatchKeydown({
        ctrlKey: true,
        shiftKey: true,
        code: "KeyC",
      });

      expect(event.defaultPrevented).toBe(true);
      expect(writeTextMock).toHaveBeenCalledWith("text");

      // 等待微任务清空，确保 .catch() 消费了 rejection（不向上抛出）
      await new Promise<void>((r) => setTimeout(r, 0));
      // 未触发 unhandledrejection 即视为通过（vitest 默认对未捕获 rejection 报错）
    });
  });

  // ========== 12. uninstall 后事件不再被处理 ==========

  describe("uninstall 后事件不再被处理", () => {
    it("36. 卸载全局监听器后 Ctrl+Shift+C 透传（即使 activeTerminal 有值）", () => {
      setActiveTerminal(terminalStub({ getSelection: () => "text" }));

      // 确认安装状态下事件被拦截
      const event1 = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(event1.defaultPrevented).toBe(true);

      // 卸载全局监听器（afterEach 已重置，当前 count=1）
      uninstallKeyboardHandler();

      // 卸载后事件透传
      const event2 = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(event2.defaultPrevented).toBe(false);
    });

    it("37. 卸载后重新安装 → 事件再次被拦截", () => {
      setActiveTerminal(terminalStub({ getSelection: () => "text" }));

      // 卸载
      uninstallKeyboardHandler();
      // 重新安装
      installKeyboardHandler();

      const event = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(true);
    });
  });

  // ========== 13. 跨测试 installed 标志不污染 ==========

  describe("installed 标志跨测试不污染", () => {
    it("38. afterEach 重置后新测试以已安装状态开始", () => {
      // afterEach 已调用 uninstall + reinstall，handler 应处于已安装状态
      setActiveTerminal(terminalStub({ getSelection: () => "text" }));

      const event = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(true);
    });

    it("39. 多次 install/uninstall 配对后 handler 仍在预期状态", () => {
      // 模拟多次配对调用（类似 StrictMode 或面板反复开关）
      installKeyboardHandler();
      uninstallKeyboardHandler();
      installKeyboardHandler();
      uninstallKeyboardHandler();
      // 此时 count=0，handler 已卸载

      // 手动恢复到已安装状态
      installKeyboardHandler();

      setActiveTerminal(terminalStub({ getSelection: () => "text" }));
      const event = dispatchKeydown({
        ctrlKey: true, shiftKey: true, code: "KeyC",
      });
      expect(event.defaultPrevented).toBe(true);

      // 平衡手动安装（afterEach 会再 uninstall+reinstall）
      uninstallKeyboardHandler();
    });
  });
});
