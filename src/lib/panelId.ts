// panelId — 终端面板 localId 生成单点（CP-004/S11 页前缀协议化改造）
//
// 共享宿主架构下完整 panelId = "{pageId}:{localId}"（pageGroups.panelIdInPage
// 页前缀协议）；本模块只负责 localId 侧：终端 localId = terminal-{seq}（seq 为
// 每页独立全数字序号，与页签标题 terminal-N 同计数源——每页从 0 起，恢复布局
// 不占号）。完整 id 构造 = panelIdInPage(pageId, makeTerminalPanelId(pageId))，
// 见 workspace/PageDockviewHost.makeTerminalIdInPage。
//
// 历史（B14）：旧格式 terminal-{pageId}-{seq}（含曾引入的 Date.now 数字段，
// 破坏贪婪正则/切分解析——历史恢复黑屏 + 幽灵页面导航根因）。旧布局经
// layoutSerde 旧多实例格式迁移重写为页前缀协议形态，运行期不再产生旧格式。

/** 每页终端序号计数器（模块级——workspace 各新建入口与 restoreSession 跨
 *  上下文共享同页计数，防同页 localId 碰撞；布局恢复的持久化面板不占用计数，
 *  由 advanceTerminalPanelSeq 在布局恢复后推进） */
const seqByPage = new Map<string, number>();

/**
 * 生成终端面板 localId：terminal-{seq}（页内编号——每页从 0 起独立递增）。
 * seq 缺省 → 每页独立递增消费；显式传入不消费计数（调用方自行保证不冲突）。
 * 完整面板 id 须经 pageGroups.panelIdInPage 加页前缀。
 */
export function makeTerminalPanelId(pageId: string, seq?: number): string {
  if (seq === undefined) {
    seq = seqByPage.get(pageId) ?? 0;
    seqByPage.set(pageId, seq + 1);
  }
  return `terminal-${seq}`;
}

/** 测试专用：重置每页序号计数（panelId.test.ts / agent-history-restore.test.ts 等
 *  beforeEach 调用隔离模块态——生产零消费） */
export function resetTerminalPanelSeq(pageId?: string): void {
  if (pageId !== undefined) seqByPage.delete(pageId);
  else seqByPage.clear();
}

/**
 * 把页面的终端序号计数推进到现有面板 localId 最大序号 + 1——
 * 布局恢复的持久化面板不占用计数，不推进则新建/恢复终端可能与已存在面板
 * localId 重号。由 Workspace 宿主在页组恢复后调用。
 * @param panelIds 宿主内该页面板完整 id 集合（{pageId}:terminal-{seq} 形态）
 */
export function advanceTerminalPanelSeq(pageId: string, panelIds: string[]): void {
  const prefix = `${pageId}:terminal-`;
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
