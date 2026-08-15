// panelId 生成/解析单点纯函数测试（B14：生成与解析成对收口）
// 验证 parseTerminalPageId / makeTerminalPanelId / advanceTerminalPanelSeq / resetTerminalPanelSeq

import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import {
  parseTerminalPageId,
  makeTerminalPanelId,
  advanceTerminalPanelSeq,
  resetTerminalPanelSeq,
} from "../lib/panelId";

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

describe("makeTerminalPanelId（B14 生成单点）", () => {
  beforeEach(() => {
    resetTerminalPanelSeq();
  });

  it("格式：terminal-{pageId}-{seq}，seq 从 0 起每页独立递增", () => {
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-0");
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-1");
    // 不同页独立计数
    expect(makeTerminalPanelId("page-b")).toBe("terminal-page-b-0");
  });

  it("pageId 含数字段（page-{ts}-{n} 真实形态）→ 末段 seq 仍可解析（roundtrip）", () => {
    const id = makeTerminalPanelId("page-1700000000000-3");
    expect(id).toBe("terminal-page-1700000000000-3-0");
    expect(parseTerminalPageId(id)).toBe("page-1700000000000-3");
  });

  it("显式 seq 不消费计数", () => {
    expect(makeTerminalPanelId("page-a", 5)).toBe("terminal-page-a-5");
    // 计数未被显式 seq 推进
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-0");
  });
});

describe("advanceTerminalPanelSeq（B14 布局恢复推进）", () => {
  beforeEach(() => {
    resetTerminalPanelSeq();
  });
  afterEach(() => {
    resetTerminalPanelSeq();
  });

  it("按现有面板 id 推进到 max+1", () => {
    advanceTerminalPanelSeq("page-a", [
      "terminal-page-a-0",
      "terminal-page-a-3",
      "editor-x",
      "terminal-page-b-9",
    ]);
    // 推进到 4，新建面板从 4 起（不与持久化面板重号）
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-4");
  });

  it("无匹配面板 → 不推进", () => {
    advanceTerminalPanelSeq("page-a", ["editor-x", "terminal-page-b-9"]);
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-0");
  });

  it("计数已更高 → 不回退", () => {
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-0");
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-1");
    advanceTerminalPanelSeq("page-a", ["terminal-page-a-0"]);
    // 当前计数 2 > 推进值 1 → 保持
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-2");
  });

  it("非数字尾段面板跳过", () => {
    advanceTerminalPanelSeq("page-a", ["terminal-page-a-abc"]);
    expect(makeTerminalPanelId("page-a")).toBe("terminal-page-a-0");
  });
});

describe("parseTerminalPageId 空 pageId 防御（B14）", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("解析出空 pageId → null + console.warn", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    // terminal--0 → 中间段为 "-" → 合并后空串
    expect(parseTerminalPageId("terminal--0")).toBeNull();
    expect(warnSpy).toHaveBeenCalled();
  });
});
