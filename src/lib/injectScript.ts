// injectScript.ts — 向 HTML 字符串中注入脚本标签的纯函数
//
// 用于 HtmlPanel：在设置 srcDoc 前将键盘转发脚本注入到 HTML 内容中。
// 纯函数、不访问 DOM、不抛异常。

/**
 * 将脚本字符串注入到 HTML 的 </head> 之前或 <body 之前。
 * 优先级：</head> 之前 → <body 之前 → 追加到末尾。
 * 幂等：已含相同标记的 HTML 不会重复注入。
 *
 * @param html  原始 HTML 字符串
 * @param script  要注入的 <script>...</script> 字符串
 * @param marker  幂等标记字符串（出现即跳过注入）
 * @returns 注入后的 HTML 字符串
 */
export function injectScript(
  html: string,
  script: string,
  marker: string,
): string {
  // 幂等：已含 marker 则跳过
  if (html && html.includes(marker)) return html;

  // 空/falsy HTML → 构造最小完整文档
  if (!html || html.trim().length === 0) {
    return `<html><head>${script}</head><body></body></html>`;
  }

  // 策略 1: 在 </head> 之前插入（大小写不敏感）
  const headClose = /<\/head>/i.exec(html);
  if (headClose) {
    return (
      html.slice(0, headClose.index) + script + html.slice(headClose.index)
    );
  }

  // 策略 2: 在 <body 之前插入（大小写不敏感，匹配 <body 开头标签）
  const bodyOpen = /<body\b/i.exec(html);
  if (bodyOpen) {
    return (
      html.slice(0, bodyOpen.index) + script + html.slice(bodyOpen.index)
    );
  }

  // 策略 3: 在 </html> 之前插入
  const htmlClose = /<\/html>/i.exec(html);
  if (htmlClose) {
    return (
      html.slice(0, htmlClose.index) + script + html.slice(htmlClose.index)
    );
  }

  // 兜底：追加到末尾
  return html + script;
}
