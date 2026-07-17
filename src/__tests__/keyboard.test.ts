// keyboard.test.ts — 终端快捷键命令工厂单元测试
//
// 工厂 createTerminalShortcuts() 无参，handler 经 getActiveTerminal() 派发到聚焦终端。
// 覆盖：命令结构、copy/paste/newline 派发到 active、无 active 时返回 false 透传。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createTerminalShortcuts } from "../panels/terminal/keyboard";
import {
  setActiveTerminal,
  clearActiveTerminal,
  type TerminalActions,
} from "../panels/terminal/activeTerminal";
import type { Command } from "../features/shortcuts";

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

/** 构造聚焦终端动作 stub */
function makeActive(over?: Partial<TerminalActions>): TerminalActions {
  return {
    getSelection: () => "",
    paste: vi.fn(),
    writeToPty: vi.fn(),
    ...over,
  };
}

function keyEvent(opts: { ctrlKey?: boolean; shiftKey?: boolean; code?: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    code: opts.code ?? "",
    bubbles: true,
    cancelable: true,
  });
}

function findCommand(cmds: Command[], id: string): Command | undefined {
  return cmds.find((c) => c.id === id);
}

// ====== 测试套件 ======

describe("createTerminalShortcuts", () => {
  let current: TerminalActions | null = null;
  const cmds = createTerminalShortcuts();

  /** 设置聚焦终端（afterEach 会清） */
  function setActive(a: TerminalActions) {
    current = a;
    setActiveTerminal(a);
  }

  beforeEach(() => {
    writeTextMock.mockClear();
    readTextMock.mockClear();
    writeTextMock.mockResolvedValue(undefined);
    readTextMock.mockResolvedValue("");
  });

  afterEach(() => {
    if (current) clearActiveTerminal(current);
    current = null;
  });

  // ---- 1. 命令结构 ----

  describe("命令结构", () => {
    it("返回三个命令：terminal.copy / terminal.paste / terminal.newline", () => {
      expect(cmds).toHaveLength(3);
      expect(findCommand(cmds, "terminal.copy")).toBeDefined();
      expect(findCommand(cmds, "terminal.paste")).toBeDefined();
      expect(findCommand(cmds, "terminal.newline")).toBeDefined();
    });

    it("命令 context 为 terminal", () => {
      for (const cmd of cmds) expect(cmd.context).toBe("terminal");
    });

    it("Ctrl+C 未注册为命令（自然透传）", () => {
      const ctrlC = cmds.filter(
        (c) => c.defaultKey?.code === "KeyC" && c.defaultKey?.ctrlKey && !c.defaultKey?.shiftKey,
      );
      expect(ctrlC).toHaveLength(0);
    });
  });

  // ---- 2. 派发到聚焦终端 ----

  describe("派发到聚焦终端", () => {
    it("copy：有选区 → 写剪贴板，返回 true", () => {
      setActive(makeActive({ getSelection: () => "selected text" }));
      const result = findCommand(cmds, "terminal.copy")!.handler(keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" }));
      expect(writeTextMock).toHaveBeenCalledWith("selected text");
      expect(result).toBe(true);
    });

    it("copy：无选区 → 不写剪贴板，仍返回 true", () => {
      setActive(makeActive({ getSelection: () => "" }));
      const result = findCommand(cmds, "terminal.copy")!.handler(keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" }));
      expect(writeTextMock).not.toHaveBeenCalled();
      expect(result).toBe(true);
    });

    it("copy：writeText 拒绝被吞，仍返回 true", () => {
      writeTextMock.mockRejectedValue(new Error("NotAllowedError"));
      setActive(makeActive({ getSelection: () => "text" }));
      const result = findCommand(cmds, "terminal.copy")!.handler(keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" }));
      expect(writeTextMock).toHaveBeenCalledWith("text");
      expect(result).toBe(true);
    });

    it("paste：读剪贴板并 paste 到聚焦终端", async () => {
      readTextMock.mockResolvedValue("clipboard content");
      const pasteSpy = vi.fn();
      setActive(makeActive({ paste: pasteSpy }));
      const result = findCommand(cmds, "terminal.paste")!.handler(keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyV" }));
      expect(result).toBe(true);
      await vi.waitFor(() => expect(pasteSpy).toHaveBeenCalledWith("clipboard content"), { timeout: 3000 });
    });

    it("paste：readText 拒绝 → 不 paste", async () => {
      readTextMock.mockRejectedValue(new Error("NotAllowedError"));
      const pasteSpy = vi.fn();
      setActive(makeActive({ paste: pasteSpy }));
      findCommand(cmds, "terminal.paste")!.handler(keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyV" }));
      await new Promise<void>((r) => setTimeout(r, 0));
      expect(pasteSpy).not.toHaveBeenCalled();
    });

    it("newline：写 [0x0a] 到聚焦终端 PTY，返回 true", () => {
      const writeSpy = vi.fn();
      setActive(makeActive({ writeToPty: writeSpy }));
      const result = findCommand(cmds, "terminal.newline")!.handler(keyEvent({ ctrlKey: true, code: "Enter" }));
      expect(result).toBe(true);
      expect(writeSpy).toHaveBeenCalledTimes(1);
      expect(Array.from(writeSpy.mock.calls[0][0] as Uint8Array)).toEqual([0x0a]);
    });
  });

  // ---- 3. 无聚焦终端 ----

  describe("无聚焦终端 → 透传", () => {
    it("copy 无 active → 返回 false，不写剪贴板", () => {
      const result = findCommand(cmds, "terminal.copy")!.handler(keyEvent({ ctrlKey: true, shiftKey: true, code: "KeyC" }));
      expect(result).toBe(false);
      expect(writeTextMock).not.toHaveBeenCalled();
    });

    it("newline 无 active → 返回 false", () => {
      const result = findCommand(cmds, "terminal.newline")!.handler(keyEvent({ ctrlKey: true, code: "Enter" }));
      expect(result).toBe(false);
    });
  });

  // ---- 4. 默认键 ----

  it("terminal.newline 默认键为 Ctrl+Enter", () => {
    expect(findCommand(cmds, "terminal.newline")!.defaultKey).toEqual({
      ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, code: "Enter",
    });
  });
});
