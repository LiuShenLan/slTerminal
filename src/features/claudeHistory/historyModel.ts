// historyModel.ts — claude 历史会话纯函数模型（FE-05）
//
// 纯函数模块：禁 import react / 任何 hook，零副作用。
// 所有展示派生逻辑（当前项目匹配 / 分组 / 搜索 / 相对时间 / ⚡ 派生）集中于此，
// 供 UI 层（Stage 05）与 L2 测试共用。契约见 docs/claude-history-view/checklist.md FE-05。

import { normalizePath, basename } from "../../lib/path";
import { TerminalRegistry } from "../../panels/terminal/TerminalRegistry";
import type { HistorySession } from "../../types/claudeHistory";

/** 无 cwd 会话的分组键（null；展示文案「(未知目录)」由 UI 层负责） */
export const UNKNOWN_CWD_KEY: null = null;

/**
 * 判断会话 cwd 是否与当前项目 rootPath 匹配（决策 24）
 *
 * 两侧经 normalizePath（反斜杠 → 正斜杠）+ 忽略大小写后精确相等；
 * 任一侧为 null 或空串 → false（无法比较，不视为匹配）。
 */
export function isCurrentProject(
  sessionCwd: string | null,
  rootPath: string | null,
): boolean {
  if (!sessionCwd || !rootPath) return false;
  return (
    normalizePath(sessionCwd).toLowerCase() ===
    normalizePath(rootPath).toLowerCase()
  );
}

/** 分组结果：cwd 为 null 归 UNKNOWN_CWD_KEY 组（key = null） */
export interface CwdGroup {
  /** 组展示用 cwd（取组内 mtime 最大会话的原始 cwd）；null = 未知目录组 */
  cwd: string | null;
  /** 组内会话，mtimeMs 降序 */
  sessions: HistorySession[];
}

/**
 * 按目录分组（规格 4.1）：
 * - 分组键 = 规范化（斜杠统一 + 忽略大小写）cwd——同目录不同写法归一组
 * - cwd 为 null 归 UNKNOWN_CWD_KEY（null）组
 * - 组内 mtimeMs 降序；组间按组内最大 mtimeMs 降序（最近活动的组排前）
 * - 组展示 cwd 取组内最大 mtime 会话的原始 cwd（保留用户原始写法）
 */
export function groupByCwd(sessions: HistorySession[]): CwdGroup[] {
  const groups = new Map<string | null, HistorySession[]>();
  for (const s of sessions) {
    const key = s.cwd
      ? normalizePath(s.cwd).toLowerCase()
      : UNKNOWN_CWD_KEY;
    const list = groups.get(key);
    if (list) {
      list.push(s);
    } else {
      groups.set(key, [s]);
    }
  }

  // 组内按 mtimeMs 降序（稳定排序，同刻会话保持输入序）
  const result: CwdGroup[] = [];
  for (const [key, list] of groups) {
    list.sort((a, b) => b.mtimeMs - a.mtimeMs);
    result.push({ cwd: key === null ? null : list[0].cwd, sessions: list });
  }
  // 组间按组内最大 mtimeMs 降序（组内已排序，首元素即最大值）
  result.sort((a, b) => b.sessions[0].mtimeMs - a.sessions[0].mtimeMs);
  return result;
}

/**
 * 搜索匹配（规格 4.3.4）：标题 + firstPrompt，大小写不敏感 includes；
 * query 空白 → 恒 true（不参与过滤）。
 */
export function matchesSearch(session: HistorySession, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return (
    (session.title ?? "").toLowerCase().includes(q) ||
    (session.firstPrompt ?? "").toLowerCase().includes(q)
  );
}

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/**
 * 相对时间格式化（规格 4.2 时间粒度；决策 26 口径 = 文件 mtime）
 *
 * 六档：<1 分钟「刚刚」；<60 分钟「N 分钟前」；<24 小时「N 小时前」；
 *       <7 天「N 天前」；同年「MM-DD」；跨年「YYYY-MM-DD」。
 * mtimeMs <= 0（无有效时间）→ 「-」。
 */
export function formatRelativeTime(mtimeMs: number, nowMs: number): string {
  if (mtimeMs <= 0) return "-";
  const diff = nowMs - mtimeMs;
  if (diff < MINUTE_MS) return "刚刚"; // 含未来时间戳（时钟偏差容错）
  if (diff < HOUR_MS) return `${Math.floor(diff / MINUTE_MS)} 分钟前`;
  if (diff < DAY_MS) return `${Math.floor(diff / HOUR_MS)} 小时前`;
  if (diff < 7 * DAY_MS) return `${Math.floor(diff / DAY_MS)} 天前`;

  const pad = (n: number) => String(n).padStart(2, "0");
  const m = new Date(mtimeMs);
  const now = new Date(nowMs);
  const monthDay = `${pad(m.getMonth() + 1)}-${pad(m.getDate())}`;
  return m.getFullYear() === now.getFullYear()
    ? monthDay
    : `${m.getFullYear()}-${monthDay}`;
}

/**
 * 从 TerminalRegistry 派生运行中会话 id 集合（规格 4.1 两区关系，⚡ 标记）
 *
 * 取各注册终端的 claudeSession?.transcriptPath 的 basename（去 .jsonl 后缀）
 * → 会话 id；无 transcriptPath 的条目不产出 id。
 *
 * 已知局限（文档化）：matchedCommand-only 会话（无 transcriptPath）无法定位
 * 对应 transcript 文件，⚡ 标记无法覆盖——已接受（checklist FE-05）。
 */
export function deriveActiveSessionIds(): Set<string> {
  const ids = new Set<string>();
  for (const entry of TerminalRegistry.getAll().values()) {
    const transcriptPath = entry.claudeSession?.transcriptPath;
    if (!transcriptPath) continue;
    const base = basename(transcriptPath);
    ids.add(base.endsWith(".jsonl") ? base.slice(0, -".jsonl".length) : base);
  }
  return ids;
}
