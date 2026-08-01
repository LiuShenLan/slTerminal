// claude-history-model.test.ts — FE-05 纯函数全分支测试
//
// 纯函数零依赖（仅 mock TerminalRegistry 供 deriveActiveSessionIds 读注册表）：
// isCurrentProject（决策 24 匹配）/ groupByCwd（组内+组间排序/未知目录组）/
// matchesSearch（标题+prompt/大小写/空白）/ formatRelativeTime（六档边界+mtime=0）/
// deriveActiveSessionIds（有/无 transcriptPath/空注册表）。

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  isCurrentProject,
  groupByCwd,
  matchesSearch,
  formatRelativeTime,
  deriveActiveSessionIds,
} from "../features/claudeHistory/historyModel";
import type { HistorySession } from "../types/claudeHistory";

// ── vi.hoisted()：mock 状态在模块级 vi.mock 执行前就绪 ──
const h = vi.hoisted(() => {
  const all = new Map<string, Record<string, unknown>>();
  return {
    all,
    mockGetAll: vi.fn(() => new Map(all)),
    mockSubscribe: vi.fn(() => () => {}),
  };
});

// ── mock TerminalRegistry（仅派生函数读取 getAll） ──
vi.mock("../panels/terminal/TerminalRegistry", () => ({
  TerminalRegistry: {
    getAll: h.mockGetAll,
    subscribe: h.mockSubscribe,
  },
}));

/** 最小 HistorySession 工厂 */
function makeSession(overrides: Partial<HistorySession> = {}): HistorySession {
  return {
    sessionId: "session-1",
    cwd: null,
    title: null,
    titleSource: "none",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    ...overrides,
  };
}

describe("isCurrentProject", () => {
  it("规范化后精确相等 → true", () => {
    expect(isCurrentProject("D:/work/proj", "D:/work/proj")).toBe(true);
  });

  it("大小写差异 → true（忽略大小写，决策 24）", () => {
    expect(isCurrentProject("D:/Work/Proj", "d:/work/proj")).toBe(true);
  });

  it("反斜杠差异 → true（normalizePath 统一斜杠）", () => {
    expect(isCurrentProject("D:\\work\\proj", "D:/work/proj")).toBe(true);
    expect(isCurrentProject("D:\\work\\proj", "d:\\WORK\\PROJ")).toBe(true);
  });

  it("sessionCwd 为 null → false", () => {
    expect(isCurrentProject(null, "D:/work/proj")).toBe(false);
  });

  it("rootPath 为 null → false", () => {
    expect(isCurrentProject("D:/work/proj", null)).toBe(false);
  });

  it("两侧均为 null → false", () => {
    expect(isCurrentProject(null, null)).toBe(false);
  });

  it("空串 → false（任一侧）", () => {
    expect(isCurrentProject("", "D:/work/proj")).toBe(false);
    expect(isCurrentProject("D:/work/proj", "")).toBe(false);
    expect(isCurrentProject("", "")).toBe(false);
  });

  it("不相等 → false", () => {
    expect(isCurrentProject("D:/work/a", "D:/work/b")).toBe(false);
  });

  it("同前缀不同目录 → false（前缀比较陷阱）", () => {
    expect(isCurrentProject("D:/work/ab", "D:/work/a")).toBe(false);
  });
});

describe("groupByCwd", () => {
  it("组内按 mtimeMs 降序", () => {
    const sessions = [
      makeSession({ sessionId: "s1", cwd: "D:/proj", mtimeMs: 100 }),
      makeSession({ sessionId: "s2", cwd: "D:/proj", mtimeMs: 300 }),
      makeSession({ sessionId: "s3", cwd: "D:/proj", mtimeMs: 200 }),
    ];
    const groups = groupByCwd(sessions);
    expect(groups).toHaveLength(1);
    expect(groups[0].sessions.map((s) => s.sessionId)).toEqual(["s2", "s3", "s1"]);
  });

  it("组间按组内最大 mtimeMs 降序", () => {
    const sessions = [
      makeSession({ sessionId: "s1", cwd: "D:/a", mtimeMs: 500 }),
      makeSession({ sessionId: "s2", cwd: "D:/a", mtimeMs: 100 }),
      makeSession({ sessionId: "s3", cwd: "D:/b", mtimeMs: 400 }),
      makeSession({ sessionId: "s4", cwd: "D:/b", mtimeMs: 300 }),
    ];
    const groups = groupByCwd(sessions);
    expect(groups.map((g) => g.cwd)).toEqual(["D:/a", "D:/b"]);
  });

  it("无 cwd 会话归未知目录组（key 为 null）", () => {
    const sessions = [
      makeSession({ sessionId: "s1", cwd: null, mtimeMs: 900 }),
      makeSession({ sessionId: "s2", cwd: "D:/a", mtimeMs: 100 }),
    ];
    const groups = groupByCwd(sessions);
    expect(groups).toHaveLength(2);
    expect(groups[0].cwd).toBeNull(); // 未知目录组按 mtime 排最前
    expect(groups[0].sessions.map((s) => s.sessionId)).toEqual(["s1"]);
  });

  it("同目录不同大小写/斜杠写法归一组", () => {
    const sessions = [
      makeSession({ sessionId: "s1", cwd: "D:\\A\\B", mtimeMs: 100 }),
      makeSession({ sessionId: "s2", cwd: "d:/a/b", mtimeMs: 200 }),
    ];
    const groups = groupByCwd(sessions);
    expect(groups).toHaveLength(1);
    expect(groups[0].sessions).toHaveLength(2);
  });

  it("组展示 cwd 取组内最大 mtime 会话的原始写法", () => {
    const sessions = [
      makeSession({ sessionId: "s1", cwd: "D:\\A", mtimeMs: 500 }),
      makeSession({ sessionId: "s2", cwd: "d:/a", mtimeMs: 100 }),
    ];
    const groups = groupByCwd(sessions);
    expect(groups[0].cwd).toBe("D:\\A");
  });

  it("空数组 → 空结果", () => {
    expect(groupByCwd([])).toEqual([]);
  });
});

describe("matchesSearch", () => {
  it("标题命中", () => {
    const s = makeSession({ title: "修复登录 bug", firstPrompt: null });
    expect(matchesSearch(s, "登录")).toBe(true);
  });

  it("firstPrompt 命中", () => {
    const s = makeSession({ title: null, firstPrompt: "帮我看看这个报错" });
    expect(matchesSearch(s, "报错")).toBe(true);
  });

  it("大小写不敏感", () => {
    const s = makeSession({ title: "Refactor Parser", firstPrompt: null });
    expect(matchesSearch(s, "refactor")).toBe(true);
    expect(matchesSearch(s, "PARSER")).toBe(true);
  });

  it("query 空白 → 恒 true", () => {
    const s = makeSession({ title: "任意标题", firstPrompt: "任意内容" });
    expect(matchesSearch(s, "")).toBe(true);
    expect(matchesSearch(s, "   ")).toBe(true);
  });

  it("未命中 → false", () => {
    const s = makeSession({ title: "标题 A", firstPrompt: "提示 B" });
    expect(matchesSearch(s, "不存在的词")).toBe(false);
  });

  it("标题与 prompt 均为 null 且 query 非空白 → false（不抛错）", () => {
    const s = makeSession({ title: null, firstPrompt: null });
    expect(matchesSearch(s, "x")).toBe(false);
  });
});

describe("formatRelativeTime", () => {
  // 固定 now：2026-06-15 12:00（本地时区）
  const now = new Date(2026, 5, 15, 12, 0, 0).getTime();
  const MIN = 60_000;
  const HOUR = 60 * MIN;
  const DAY = 24 * HOUR;

  it("mtimeMs=0 → 「-」", () => {
    expect(formatRelativeTime(0, now)).toBe("-");
  });

  it("mtimeMs 为负 → 「-」", () => {
    expect(formatRelativeTime(-1, now)).toBe("-");
  });

  it("<1 分钟 → 「刚刚」", () => {
    expect(formatRelativeTime(now, now)).toBe("刚刚");
    expect(formatRelativeTime(now - 59_999, now)).toBe("刚刚");
  });

  it("边界 60s → 「1 分钟前」", () => {
    expect(formatRelativeTime(now - MIN, now)).toBe("1 分钟前");
  });

  it("<60 分钟 → 「N 分钟前」", () => {
    expect(formatRelativeTime(now - 59 * MIN, now)).toBe("59 分钟前");
  });

  it("边界 1 小时 → 「1 小时前」", () => {
    expect(formatRelativeTime(now - HOUR, now)).toBe("1 小时前");
  });

  it("<24 小时 → 「N 小时前」", () => {
    expect(formatRelativeTime(now - 23 * HOUR, now)).toBe("23 小时前");
  });

  it("边界 1 天 → 「1 天前」", () => {
    expect(formatRelativeTime(now - DAY, now)).toBe("1 天前");
  });

  it("<7 天 → 「N 天前」", () => {
    expect(formatRelativeTime(now - 6 * DAY, now)).toBe("6 天前");
    expect(formatRelativeTime(now - (7 * DAY - 1), now)).toBe("6 天前");
  });

  it("≥7 天同年 → 「MM-DD」", () => {
    expect(formatRelativeTime(now - 7 * DAY, now)).toBe("06-08");
    expect(formatRelativeTime(new Date(2026, 4, 20).getTime(), now)).toBe("05-20");
  });

  it("跨年 → 「YYYY-MM-DD」", () => {
    expect(formatRelativeTime(new Date(2025, 11, 5).getTime(), now)).toBe("2025-12-05");
  });
});

describe("deriveActiveSessionIds", () => {
  beforeEach(() => {
    h.all.clear();
  });

  it("有 transcriptPath → 产出 basename 去 .jsonl 后缀的 id", () => {
    h.all.set("panel-1", {
      term: {},
      sessionId: "p1",
      webglAddon: null,
      fitAddon: {},
      claudeSession: {
        transcriptPath: "C:\\Users\\x\\.claude\\projects\\proj-dir\\abc-123.jsonl",
        lastEventAt: 1,
      },
    });
    expect(deriveActiveSessionIds()).toEqual(new Set(["abc-123"]));
  });

  it("多条 → 集合含全部 id", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      claudeSession: { transcriptPath: "D:/proj/aaa.jsonl", lastEventAt: 1 },
    });
    h.all.set("panel-2", {
      term: {}, sessionId: "p2", webglAddon: null, fitAddon: {},
      claudeSession: { transcriptPath: "D:/proj/bbb.jsonl", lastEventAt: 1 },
    });
    expect(deriveActiveSessionIds()).toEqual(new Set(["aaa", "bbb"]));
  });

  it("无 transcriptPath（matchedCommand-only）→ 不产出 id", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      claudeSession: { matchedCommand: "claude", lastEventAt: 1 },
    });
    expect(deriveActiveSessionIds().size).toBe(0);
  });

  it("claudeSession 为 null / 未设置 → 不产出 id", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      claudeSession: null,
    });
    h.all.set("panel-2", {
      term: {}, sessionId: "p2", webglAddon: null, fitAddon: {},
    });
    expect(deriveActiveSessionIds().size).toBe(0);
  });

  it("空注册表 → 空 Set", () => {
    expect(deriveActiveSessionIds().size).toBe(0);
  });
});
