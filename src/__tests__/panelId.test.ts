// panelId 解析纯函数测试
// 验证 parseTerminalPageId 的各种输入 → 输出

import { describe, it, expect } from "vitest";
import { parseTerminalPageId } from "../lib/panelId";

describe("parseTerminalPageId", () => {
  it("正常格式：terminal-{pageId}-{seq} 返回 pageId", () => {
    expect(parseTerminalPageId("terminal-page1-0")).toBe("page1");
    expect(parseTerminalPageId("terminal-mypage-1")).toBe("mypage");
    expect(parseTerminalPageId("terminal-a-99")).toBe("a");
  });

  it("pageId 含连字符：中间多段合并", () => {
    expect(parseTerminalPageId("terminal-my-page-2")).toBe("my-page");
    expect(parseTerminalPageId("terminal-a-b-c-0")).toBe("a-b-c");
  });

  it("尾段非全数字 → null", () => {
    expect(parseTerminalPageId("terminal-foo-bar")).toBeNull();
    expect(parseTerminalPageId("terminal-page1-abc")).toBeNull();
    expect(parseTerminalPageId("terminal-x-12a")).toBeNull();
  });

  it("非 terminal 前缀 → null", () => {
    expect(parseTerminalPageId("editor-x-1")).toBeNull();
    expect(parseTerminalPageId("terminalx-page1-0")).toBeNull();
    expect(parseTerminalPageId("")).toBeNull();
  });

  it("不足三段 → null", () => {
    expect(parseTerminalPageId("terminal-abc")).toBeNull();
    expect(parseTerminalPageId("terminal")).toBeNull();
    expect(parseTerminalPageId("terminal-page1")).toBeNull();
  });
});
