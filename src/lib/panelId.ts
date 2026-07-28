// panelId — 终端 panelId 解析工具
//
// 终端 panelId 格式：terminal-{pageId}-{seq}（seq 为全数字序号）
// 例如 "terminal-page1-0" → pageId = "page1"
//      "terminal-my-page-2" → pageId = "my-page"

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
 */
export function parseTerminalPageId(panelId: string): string | null {
  const parts = panelId.split("-");
  if (parts.length < 3) return null;
  if (parts[0] !== "terminal") return null;
  const last = parts[parts.length - 1];
  if (!/^\d+$/.test(last)) return null;
  return parts.slice(1, -1).join("-");
}
