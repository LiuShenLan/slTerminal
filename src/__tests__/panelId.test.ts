// panelId 生成单点纯函数测试（B14 生成单点 + CP-004 页前缀协议化改造）
// 验证 makeTerminalPanelId（localId: terminal-{seq}）/ advanceTerminalPanelSeq /
// resetTerminalPanelSeq——完整面板 id = pageGroups.panelIdInPage(pageId, localId)

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  makeTerminalPanelId,
  advanceTerminalPanelSeq,
  resetTerminalPanelSeq,
} from "../lib/panelId";
import { panelIdInPage, pageOfPanelId } from "../workspace/pageGroups";

describe("makeTerminalPanelId（B14 生成单点——localId 形态）", () => {
  beforeEach(() => {
    resetTerminalPanelSeq();
  });

  it("格式：terminal-{seq}，seq 从 0 起每页独立递增；页前缀协议补全完整 id", () => {
    expect(makeTerminalPanelId("page-a")).toBe("terminal-0");
    expect(makeTerminalPanelId("page-a")).toBe("terminal-1");
    // 不同页独立计数
    expect(makeTerminalPanelId("page-b")).toBe("terminal-0");
  });

  it("pageId 含数字段（page-{ts}-{n} 真实形态）→ localId 不受影响", () => {
    expect(makeTerminalPanelId("page-1700000000000-3")).toBe("terminal-0");
  });

  it("显式 seq 不消费计数", () => {
    expect(makeTerminalPanelId("page-a", 5)).toBe("terminal-5");
    // 计数未被显式 seq 推进
    expect(makeTerminalPanelId("page-a")).toBe("terminal-0");
  });

  it("完整 id = panelIdInPage(pageId, localId)——页前缀可逆解析（协议往返）", () => {
    const full = panelIdInPage("page-1700000000000-3", makeTerminalPanelId("page-1700000000000-3"));
    expect(full).toBe("page-1700000000000-3:terminal-0");
    expect(pageOfPanelId(full)).toBe("page-1700000000000-3");
  });
});

describe("advanceTerminalPanelSeq（B14 布局恢复推进——按页前缀完整 id 扫描）", () => {
  beforeEach(() => {
    resetTerminalPanelSeq();
  });
  afterEach(() => {
    resetTerminalPanelSeq();
  });

  it("按现有面板 id 推进到 max+1（只认本页 :terminal-{数字} localId）", () => {
    advanceTerminalPanelSeq("page-a", [
      "page-a:terminal-0",
      "page-a:terminal-3",
      "page-a:editor-x",
      "page-b:terminal-9",
    ]);
    // 推进到 4，新建面板从 4 起（不与持久化面板重号）
    expect(makeTerminalPanelId("page-a")).toBe("terminal-4");
  });

  it("无匹配面板 → 不推进", () => {
    advanceTerminalPanelSeq("page-a", ["page-a:editor-x", "page-b:terminal-9"]);
    expect(makeTerminalPanelId("page-a")).toBe("terminal-0");
  });

  it("计数已更高 → 不回退", () => {
    expect(makeTerminalPanelId("page-a")).toBe("terminal-0");
    expect(makeTerminalPanelId("page-a")).toBe("terminal-1");
    advanceTerminalPanelSeq("page-a", ["page-a:terminal-0"]);
    // 当前计数 2 > 推进值 1 → 保持
    expect(makeTerminalPanelId("page-a")).toBe("terminal-2");
  });

  it("非数字尾段/非本页前缀跳过", () => {
    advanceTerminalPanelSeq("page-a", [
      "page-a:terminal-abc",
      "page-b:terminal-0",
      "terminal-page-a-0", // 旧格式全量 id 不含 ":" → 不识别（旧数据已迁移）
    ]);
    expect(makeTerminalPanelId("page-a")).toBe("terminal-0");
  });
});
