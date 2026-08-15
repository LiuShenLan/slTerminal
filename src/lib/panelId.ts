// panelId — 终端 panelId 生成/解析单点
//
// 终端 panelId 格式：terminal-{pageId}-{seq}（seq 为全数字序号）
// 例如 "terminal-page1-0" → pageId = "page1"
//      "terminal-my-page-2" → pageId = "my-page"
//
// B14: 生成与解析成对收口于本文件——旧实现格式分散三处（PageDockviewHost 生成、
// restoreSession 生成、TerminalPanel/HistorySessionList 解析），恢复编排曾生成
// 含 Date.now 数字段的格式破坏贪婪正则/切分解析（历史恢复黑屏 + 幽灵页面导航根因）

/** 每页终端序号计数器（模块级——PageDockviewHost 与 restoreSession 跨上下文共享
 *  同页计数，防同页 id 碰撞；布局恢复的持久化面板不占用计数，由
 *  advanceTerminalPanelSeq 在布局恢复后推进） */
const seqByPage = new Map<string, number>();

/**
 * 生成终端面板 id：terminal-{pageId}-{seq}。
 * seq 缺省 → 每页独立递增消费；显式传入不消费计数（调用方自行保证不冲突）。
 */
export function makeTerminalPanelId(pageId: string, seq?: number): string {
  if (seq === undefined) {
    seq = seqByPage.get(pageId) ?? 0;
    seqByPage.set(pageId, seq + 1);
  }
  return `terminal-${pageId}-${seq}`;
}

/** 仅测试：重置每页序号计数（vitest 文件内模块态隔离 + beforeEach 调用） */
export function resetTerminalPanelSeq(pageId?: string): void {
  if (pageId !== undefined) seqByPage.delete(pageId);
  else seqByPage.clear();
}

/**
 * 把页面的终端序号计数推进到现有面板 id 最大序号 + 1——
 * 布局恢复的持久化面板不占用计数，不推进则新建/恢复终端可能与已存在面板 id 重号。
 * 由 PageDockviewHost 在 loadLayout 成功后调用。
 */
export function advanceTerminalPanelSeq(pageId: string, panelIds: string[]): void {
  const prefix = `terminal-${pageId}-`;
  let max = -1;
  for (const id of panelIds) {
    if (!id.startsWith(prefix)) continue;
    const tail = id.slice(prefix.length);
    if (!/^\d+$/.test(tail)) continue;
    const n = Number(tail);
    if (n > max) max = n;
  }
  const next = max + 1;
  if (next > (seqByPage.get(pageId) ?? 0)) {
    seqByPage.set(pageId, next);
  }
}

/**
 * 从终端 panelId 解析 pageId。
 * 格式 terminal-{pageId}-{seq}，seq 必须为全数字。
 * ≥3 段 + 首段 "terminal" + 末段全数字 → 返回中间段合并的 pageId；
 * 否则返回 null。
 *
 * "terminal-page1-0" → "page1"
 * "terminal-my-page-2" → "my-page"
 * "terminal-abc"（两段）→ null
 * "terminal-foo-bar"（尾段非数字）→ null
 * "editor-x-1"（非 terminal 前缀）→ null
 *
 * 注意（B14）：旧恢复格式（terminal-{pageId}-{Date.now}-{seq}，pageId 本身含
 * 数字段）无法语法判别——调用方应优先按已知 pageId 做前缀匹配（见 TerminalPanel
 * visible 判定与 HistorySessionList 防御），本函数仅兜底新格式。
 */
export function parseTerminalPageId(panelId: string): string | null {
  const parts = panelId.split("-");
  if (parts.length < 3) return null;
  if (parts[0] !== "terminal") return null;
  const last = parts[parts.length - 1];
  if (!/^\d+$/.test(last)) return null;
  const pageId = parts.slice(1, -1).join("-");
  if (!pageId) {
    console.warn(`[panelId] 解析出空 pageId（panelId=${panelId}）`);
    return null;
  }
  return pageId;
}
