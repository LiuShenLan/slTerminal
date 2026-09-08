// injectScript.ts — 向 HTML 字符串中注入脚本标签的纯函数
//
// 用于 docViewer 预览面板：在设置 iframe srcdoc 前把注入脚本（buildInjectedScript
// 产物）拼入预览 HTML 内容。纯函数、不访问 DOM、不抛异常。
//
// 【S10-② 迁独立 webview 后语义（ADR-0019/CP-031）】宿主 HTML 的 <script> 段
// 不经任何字符串级转义进入渲染文档（原存量缺陷函数——无差别 `</script>` 转义
// 把宿主脚本闭合标签替换为 `<\/script>` 致脚本吞到 EOF 永不执行——已删除）；
// 注入段自身遵守「不输出 </script> 字面量」纪律（buildInjectedScript 拼接纪律
// #1），与宿主脚本的正常闭合标签互不干扰。宿主自带 <script> 现于新预览域
// （自定义协议宿主页 iframe，无全局 CSP）真实可执行——CP-031 消亡判定一/二
// 在 ② 即达成。

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
