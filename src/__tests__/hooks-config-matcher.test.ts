// hooks-config-matcher.test.ts — matcher 语义全表（P3-TE-05）
//
// 覆盖 contract.md C13-5 全部分支：
//   精确匹配 OR（| / , / 空格分隔，大小写敏感）、JS 正则非锚定、
//   全匹配（* / "" / 省略）、FileChanged/StopFailure 受限窄字符集强制正则、
//   非法正则防御。每分支至少 2 条用例。

import { describe, it, expect } from "vitest";
import { matchHook } from "../features/cliProfiles/profiles/claude/configEditor/matcherEngine";

describe("全匹配（* / 空串 / 省略）", () => {
  it('"*" 匹配任意目标', () => {
    expect(matchHook("*", "anything")).toEqual({ matched: true, mode: "all" });
    expect(matchHook("*", "Bash")).toEqual({ matched: true, mode: "all" });
  });

  it('空串 "" 匹配任意目标', () => {
    expect(matchHook("", "foo")).toEqual({ matched: true, mode: "all" });
    expect(matchHook("", "bar")).toEqual({ matched: true, mode: "all" });
  });

  it("省略 matcher 参数匹配任意目标", () => {
    expect(matchHook(undefined, "foo")).toEqual({ matched: true, mode: "all" });
    expect(matchHook(undefined, "Bash", "PreToolUse")).toEqual({
      matched: true,
      mode: "all",
    });
  });
});

describe("精确匹配 OR（| 分隔）", () => {
  it("多值任一命中即匹配", () => {
    expect(matchHook("Bash|Read", "Bash")).toEqual({
      matched: true,
      mode: "exact-or",
    });
    expect(matchHook("Bash|Read", "Read")).toEqual({
      matched: true,
      mode: "exact-or",
    });
  });

  it("全部未命中则不匹配", () => {
    expect(matchHook("Bash|Read", "Write")).toEqual({
      matched: false,
      mode: "exact-or",
    });
    expect(matchHook("Bash", "Bash|Read")).toEqual({
      matched: false,
      mode: "exact-or",
    });
  });

  it("大小写敏感", () => {
    expect(matchHook("Bash", "bash")).toEqual({
      matched: false,
      mode: "exact-or",
    });
    expect(matchHook("bash", "Bash")).toEqual({
      matched: false,
      mode: "exact-or",
    });
  });
});

describe("精确匹配 OR（, 分隔，v2.1.191+）", () => {
  it("逗号分隔任一命中即匹配", () => {
    expect(matchHook("Bash,Read", "Bash")).toEqual({
      matched: true,
      mode: "exact-or",
    });
    expect(matchHook("Bash,Read", "Read")).toEqual({
      matched: true,
      mode: "exact-or",
    });
  });

  it("逗号分隔全部未命中则不匹配", () => {
    expect(matchHook("Bash,Read", "Write")).toEqual({
      matched: false,
      mode: "exact-or",
    });
    expect(matchHook("Bash,Read", "BashRead")).toEqual({
      matched: false,
      mode: "exact-or",
    });
  });
});

describe("精确匹配 OR（空格分隔，v2.1.191+）", () => {
  it("空格分隔任一命中即匹配", () => {
    expect(matchHook("Bash Read", "Bash")).toEqual({
      matched: true,
      mode: "exact-or",
    });
    expect(matchHook("Bash Read", "Read")).toEqual({
      matched: true,
      mode: "exact-or",
    });
  });

  it("空格分隔全部未命中则不匹配", () => {
    expect(matchHook("Bash Read", "Write")).toEqual({
      matched: false,
      mode: "exact-or",
    });
    expect(matchHook("Bash  Read", "BashRead")).toEqual({
      matched: false,
      mode: "exact-or",
    });
  });
});

describe("精确匹配 OR（连字符参与匹配值，v2.1.195+）", () => {
  it("含连字符的值精确匹配", () => {
    expect(matchHook("file-watch", "file-watch")).toEqual({
      matched: true,
      mode: "exact-or",
    });
    expect(matchHook("file-watch|edit-file", "edit-file")).toEqual({
      matched: true,
      mode: "exact-or",
    });
  });

  it("含连字符的值未命中不匹配", () => {
    expect(matchHook("file-watch", "file")).toEqual({
      matched: false,
      mode: "exact-or",
    });
    expect(matchHook("file-watch", "file-watch-2")).toEqual({
      matched: false,
      mode: "exact-or",
    });
  });
});

describe("JS 正则非锚定", () => {
  it("含其他字符（如 .）走正则，非锚定命中", () => {
    // "B.s" 含 "." → 正则模式；非锚定匹配 "Bash"（B + 任意 + s）
    expect(matchHook("B.s", "Bash")).toEqual({ matched: true, mode: "regex" });
    // 锚点仅正则模式可用："^Bash$" 精确锚定
    expect(matchHook("^Bash$", "Bash")).toEqual({
      matched: true,
      mode: "regex",
    });
  });

  it("正则模式大小写敏感", () => {
    expect(matchHook("B.s", "bash")).toEqual({ matched: false, mode: "regex" });
    expect(matchHook("^bash$", "Bash")).toEqual({
      matched: false,
      mode: "regex",
    });
  });

  it("正则未命中返回不匹配", () => {
    expect(matchHook("^Bash$", "BashTool")).toEqual({
      matched: false,
      mode: "regex",
    });
    expect(matchHook("Read.*", "Write")).toEqual({
      matched: false,
      mode: "regex",
    });
  });

  it("非法正则防御为不匹配，不抛错", () => {
    expect(matchHook("[", "Bash")).toEqual({ matched: false, mode: "regex" });
    expect(matchHook("(unclosed", "x")).toEqual({
      matched: false,
      mode: "regex",
    });
  });
});

describe("FileChanged/StopFailure 受限窄字符集（仅字母/数字/_/|）", () => {
  it("字母/数字/下划线/竖线仍走精确 OR", () => {
    expect(matchHook("src|lib", "lib", "FileChanged")).toEqual({
      matched: true,
      mode: "exact-or",
    });
    expect(matchHook("main_1", "main_1", "StopFailure")).toEqual({
      matched: true,
      mode: "exact-or",
    });
  });

  it("连字符强制走正则", () => {
    // "foo-bar" 含连字符 → FileChanged 下受限窄字符集不命中 → 正则非锚定仍可匹配
    expect(matchHook("foo-bar", "foo-bar", "FileChanged")).toEqual({
      matched: true,
      mode: "regex",
    });
    // 目标不含连字符子串 → 不匹配
    expect(matchHook("foo-bar", "foo", "FileChanged")).toEqual({
      matched: false,
      mode: "regex",
    });
  });

  it("空格强制走正则", () => {
    expect(matchHook("foo bar", "foo bar", "FileChanged")).toEqual({
      matched: true,
      mode: "regex",
    });
    expect(matchHook("foo bar", "foo", "FileChanged")).toEqual({
      matched: false,
      mode: "regex",
    });
  });

  it("逗号强制走正则", () => {
    expect(matchHook("a,b", "a,b", "StopFailure")).toEqual({
      matched: true,
      mode: "regex",
    });
    expect(matchHook("a,b", "a", "StopFailure")).toEqual({
      matched: false,
      mode: "regex",
    });
  });

  it("同一 matcher 在非受限事件下保持精确 OR（对照）", () => {
    // 连字符在普通事件（如 PreToolUse）窄字符集内 → 精确 OR
    expect(matchHook("foo-bar", "foo-bar", "PreToolUse")).toEqual({
      matched: true,
      mode: "exact-or",
    });
    expect(matchHook("a,b", "a", "Notification")).toEqual({
      matched: true,
      mode: "exact-or",
    });
  });
});
