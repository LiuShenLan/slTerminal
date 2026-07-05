// keyboard.test.ts — 终端快捷键命令工厂单元测试
//
// 测试 createTerminalShortcuts 返回的 ShortcutCommand[]：
//   1. Ctrl+Shift+C 选中复制
//   2. Ctrl+Shift+C 无选中不写剪贴板
//   3. Ctrl+Shift+V 粘贴
//   4. 剪贴板 API 拒绝时的 fallback
//   5. Ctrl+C 不注册命令（自然透传）
//   6. Terminal 为 null 时安全处理

import { describe, it, expect, vi, beforeEach } from "vitest";
import { createTerminalShortcuts } from "../panels/terminal/keyboard";
import type { Terminal } from "@xterm/xterm";
import type { ShortcutCommand } from "../features/shortcuts";

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

/** 构造 KeyboardEvent stub */
function keyEvent(opts: {
  ctrlKey?: boolean;
  shiftKey?: boolean;
  code?: string;
}): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    code: opts.code ?? "",
    bubbles: true,
    cancelable: true,
  });
}

/** 从命令列表按 id 查找命令 */
function findCommand(cmds: ShortcutCommand[], id: string): ShortcutCommand | undefined {
  return cmds.find((c) => c.id === id);
}

// ====== 测试套件 ======

describe("createTerminalShortcuts", () => {
  let term: Terminal;
  let pasteSpy: ReturnType<typeof vi.fn<(text: string) => void>>;

  beforeEach(() => {
    pasteSpy = vi.fn();
    term = terminalStub({ getSelection: () => "", paste: pasteSpy as unknown as (text: string) => void });
    writeTextMock.mockClear();
    readTextMock.mockClear();
    writeTextMock.mockResolvedValue(undefined);
    readTextMock.mockResolvedValue("");
  });

  // ---- 1. 命令结构 ----

  describe("命令结构", () => {
    it("返回两个命令：terminal.copy 和 terminal.paste", () => {
      const cmds = createTerminalShortcuts(() => term);
      expect(cmds).toHaveLength(2);
      expect(findCommand(cmds, "terminal.copy")).toBeDefined();
      expect(findCommand(cmds, "terminal.paste")).toBeDefined();
    });

    it("命令 context 为 terminal", () => {
      const cmds = createTerminalShortcuts(() => term);
      for (const cmd of cmds) {
        expect(cmd.context).toBe("terminal");
      }
    });

    it("Ctrl+C 未注册为命令（自然透传）", () => {
      const cmds = createTerminalShortcuts(() => term);
      const ctrlC = cmds.filter(
        (c) => c.keystroke.code === "KeyC" && c.keystroke.ctrlKey && !c.keystroke.shiftKey,
      );
      expect(ctrlC).toHaveLength(0);
    });
  });

  // ---- 2. Ctrl+Shift+C 复制 ----

  describe("Ctrl+Shift+C 复制", () => {
    it("有选中文本 → 写入剪贴板", () => {
      const termWithSelection = terminalStub({ getSelection: () => "selected text" });
      const cmds = createTerminalShortcuts(() => termWithSelection);
      const copyCmd = findCommand(cmds, "terminal.copy")!;
      const e = keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" });

      const result = copyCmd.handler(e);

      expect(writeTextMock).toHaveBeenCalledWith("selected text");
      // handler 返回 true → 阻止默认
      expect(result).toBe(true);
    });

    it("无选中文本（空字符串）→ 不调用 writeText，仍阻止默认", () => {
      const cmds = createTerminalShortcuts(() => term);
      const copyCmd = findCommand(cmds, "terminal.copy")!;
      const e = keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" });

      const result = copyCmd.handler(e);

      expect(writeTextMock).not.toHaveBeenCalled();
      expect(result).toBe(true);
    });

    it("writeText 拒绝 → 错误被 .catch() 吞掉，仍阻止默认", () => {
      writeTextMock.mockRejectedValue(new Error("NotAllowedError"));
      const termWithSelection = terminalStub({ getSelection: () => "text" });
      const cmds = createTerminalShortcuts(() => termWithSelection);
      const copyCmd = findCommand(cmds, "terminal.copy")!;
      const e = keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" });

      const result = copyCmd.handler(e);

      expect(writeTextMock).toHaveBeenCalledWith("text");
      expect(result).toBe(true);
      // 拒绝被 .catch(() => {}) 吞掉，不向上抛出
    });

    it("Terminal 为 null → 不调用 writeText，仍阻止默认", () => {
      const cmds = createTerminalShortcuts(() => null);
      const copyCmd = findCommand(cmds, "terminal.copy")!;
      const e = keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" });

      const result = copyCmd.handler(e);

      expect(writeTextMock).not.toHaveBeenCalled();
      expect(result).toBe(true);
    });
  });

  // ---- 3. Ctrl+Shift+V 粘贴 ----

  describe("Ctrl+Shift+V 粘贴", () => {
    it("读取剪贴板内容并 paste 到终端", async () => {
      readTextMock.mockResolvedValue("clipboard content");
      const cmds = createTerminalShortcuts(() => term);
      const pasteCmd = findCommand(cmds, "terminal.paste")!;
      const e = keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyV" });

      const result = pasteCmd.handler(e);

      expect(result).toBe(true);
      // paste 在 .then() 回调中异步执行
      await vi.waitFor(() => {
        expect(pasteSpy).toHaveBeenCalledWith("clipboard content");
      });
    });

    it("readText 拒绝 → 不调用 paste", async () => {
      readTextMock.mockRejectedValue(new Error("NotAllowedError"));
      const cmds = createTerminalShortcuts(() => term);
      const pasteCmd = findCommand(cmds, "terminal.paste")!;
      const e = keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyV" });

      const result = pasteCmd.handler(e);

      expect(result).toBe(true);
      // 微任务清空后 paste 不应被调用
      await new Promise<void>((r) => setTimeout(r, 0));
      expect(pasteSpy).not.toHaveBeenCalled();
    });

    it("Terminal 为 null → 不调用 paste（readText resolve 后安全退出）", async () => {
      readTextMock.mockResolvedValue("content");
      const cmds = createTerminalShortcuts(() => null);
      const pasteCmd = findCommand(cmds, "terminal.paste")!;
      const e = keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyV" });

      pasteCmd.handler(e);

      await new Promise<void>((r) => setTimeout(r, 0));
      expect(pasteSpy).not.toHaveBeenCalled();
    });
  });
});
