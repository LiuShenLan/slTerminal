// agent-history-model.test.ts — FE-05 纯函数全分支测试
//
// 纯函数零依赖（仅 mock TerminalRegistry 供 deriveActiveSessionStatuses 读注册表）：
// isCurrentProject（决策 24 匹配）/ groupByCwd（组内+组间排序/未知目录组）/
// matchesSearch（标题+prompt/大小写/空白）/ formatRelativeTime（六档边界+mtime=0）/
// deriveActiveSessionStatuses（复合键 cliId|sessionId——MC-313：sessionId 优先/
// basename 回退/无 cliId 按 CLAUDE_CLI_ID 回退/双无跳过/空注册表/status 透传）。

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  isCurrentProject,
  groupByCwd,
  matchesSearch,
  formatRelativeTime,
  deriveActiveSessionStatuses,
  keyOf,
} from "../features/agentHistory/historyModel";
import type { AgentHistorySession } from "../types/agentHistory";

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

/** 最小 AgentHistorySession 工厂 */
function makeSession(
  overrides: Partial<AgentHistorySession> = {},
): AgentHistorySession {
  return {
    sessionId: "session-1",
    cwd: null,
    title: null,
    titleSource: "none",
    firstPrompt: null,
    mtimeMs: 0,
    cwdExists: false,
    cliId: "claude",
    ...overrides,
  };
}

/** TerminalRegistry 条目 agentSession 工厂（模块级——deriveActiveSessionStatuses 与 keyOf 用例共用） */
function makeAgentSession(overrides: Record<string, unknown> = {}) {
  return {
    sessionId: "abc-123",
    usageSourcePath: "C:\\Users\\x\\.claude\\projects\\proj-dir\\abc-123.jsonl",
    status: "attention",
    lastEventAt: 1,
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

describe("deriveActiveSessionStatuses", () => {
  beforeEach(() => {
    h.all.clear();
  });

  it("sessionId 优先 → Map 键 = `cliId|sessionId` 复合键（MC-313），值为 status", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ status: "working" }),
    });
    expect(deriveActiveSessionStatuses()).toEqual(
      new Map([["claude|abc-123", "working"]]),
    );
  });

  it("显式 cliId → 键 = `${cliId}|${sessionId}`（不依赖缺省回退）", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ cliId: "mockcli", status: "working" }),
    });
    expect(deriveActiveSessionStatuses()).toEqual(
      new Map([["mockcli|abc-123", "working"]]),
    );
  });

  it("同 sessionId 不同 cliId → 两条键互不覆盖（复合键防跨 CLI 冲突）", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ cliId: "claude", sessionId: "dup-1" }),
    });
    h.all.set("panel-2", {
      term: {}, sessionId: "p2", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ cliId: "mockcli", sessionId: "dup-1" }),
    });
    expect(deriveActiveSessionStatuses()).toEqual(
      new Map([
        ["claude|dup-1", "attention"],
        ["mockcli|dup-1", "attention"],
      ]),
    );
  });

  it("旧数据无 cliId → 按 CLAUDE_CLI_ID 常量回退（非字面量，AC-5 兼容）", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ sessionId: "old-1", status: "working" }),
    });
    expect(deriveActiveSessionStatuses().get("claude|old-1")).toBe("working");
  });

  it("无 sessionId 有 usageSourcePath → basename 去 .jsonl 回退（旧数据兼容）", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({
        sessionId: undefined,
        status: "done",
      }),
    });
    expect(deriveActiveSessionStatuses()).toEqual(new Map([["claude|abc-123", "done"]]));
  });

  it("sessionId 为 null → basename 去 .jsonl 回退（NAH-01）", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: {
        sessionId: null,
        usageSourcePath: "C:/x/abc.jsonl",
        status: "working",
      },
    });
    expect(deriveActiveSessionStatuses().get("claude|abc")).toBe("working");
  });

  it("sessionId 与 usageSourcePath 均无（matchedCommand-only）→ 跳过不产出", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: {
        matchedCommand: "claude",
        status: "attention",
        lastEventAt: 1,
      },
    });
    expect(deriveActiveSessionStatuses().size).toBe(0);
  });

  it("status 为 null / undefined → 不产出键（历史区无标记，与活跃区 null 无图标一致）", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ status: null }),
    });
    h.all.set("panel-2", {
      term: {}, sessionId: "p2", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ sessionId: "def-456", status: undefined }),
    });
    expect(deriveActiveSessionStatuses().size).toBe(0);
  });

  it("agentSession 为 null / 未设置 → 不产出", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: null,
    });
    h.all.set("panel-2", {
      term: {}, sessionId: "p2", webglAddon: null, fitAddon: {},
    });
    expect(deriveActiveSessionStatuses().size).toBe(0);
  });

  it("四态透传（working/attention/done/error）", () => {
    const statuses = ["working", "attention", "done", "error"] as const;
    statuses.forEach((status, i) => {
      h.all.set(`panel-${i}`, {
        term: {}, sessionId: `p${i}`, webglAddon: null, fitAddon: {},
        agentSession: makeAgentSession({
          sessionId: `id-${i}`,
          status,
        }),
      });
    });
    const map = deriveActiveSessionStatuses();
    statuses.forEach((status, i) => {
      expect(map.get(`claude|id-${i}`)).toBe(status);
    });
  });

  it("多条 → Map 含全部复合键 → status", () => {
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ sessionId: "aaa", status: "working" }),
    });
    h.all.set("panel-2", {
      term: {}, sessionId: "p2", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({ sessionId: "bbb", status: "error" }),
    });
    expect(deriveActiveSessionStatuses()).toEqual(
      new Map([
        ["claude|aaa", "working"],
        ["claude|bbb", "error"],
      ]),
    );
  });

  it("空注册表 → 空 Map", () => {
    expect(deriveActiveSessionStatuses().size).toBe(0);
  });
});

describe("keyOf（复合键单点，ZQ-1/ZQ-7）", () => {
  it("cliId 缺省回退：null/undefined → CLAUDE_CLI_ID", () => {
    expect(keyOf(null, "s1")).toBe("claude|s1");
    expect(keyOf(undefined, "s1")).toBe("claude|s1");
  });

  it("显式 cliId 原样透传（不依赖缺省回退）", () => {
    expect(keyOf("mockcli", "s1")).toBe("mockcli|s1");
  });

  it("cliId/sessionId 含竖线 → 两侧转义（`\\|` 两字符），生产消费两侧键一致", () => {
    // 转义形态：竖线 → 「反斜杠+竖线」两字符，拼接为 a|b（分隔符竖线不转义）
    expect(keyOf("a|b", "c|d")).toBe("a\\|b|c\\|d");
    expect(keyOf("a|b", "c")).toBe("a\\|b|c");
    expect(keyOf("a", "c|d")).toBe("a|c\\|d");
    // 缺省回退 + 转义组合
    expect(keyOf(null, "c|d")).toBe("claude|c\\|d");

    // 生产侧（deriveActiveSessionStatuses 键构造经 keyOf）与消费侧（keyOf 查键）键一致
    h.all.set("panel-1", {
      term: {}, sessionId: "p1", webglAddon: null, fitAddon: {},
      agentSession: makeAgentSession({
        cliId: "a|b",
        sessionId: "c|d",
        status: "working",
      }),
    });
    const key = keyOf("a|b", "c|d");
    expect(key).toBe("a\\|b|c\\|d");
    expect(deriveActiveSessionStatuses().get(key)).toBe("working");
    // 未转义的裸拼接键不得命中（转义确为键的一部分，而非装饰）
    expect(deriveActiveSessionStatuses().get("a|b|c|d")).toBeUndefined();
  });
});
