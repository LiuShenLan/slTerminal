// mdRenderAsync.ts — md 预览异步编排（资源读取 + mermaid 渲染 + 占位替换）
//
// 同步纯管线（mdPipeline）→ 异步资源层：
//   1. 图片占位逐个读资源（后端沙箱通道 base64 → data: URL；失败回退原 src——
//      相对引用在 opaque iframe 内不可显示，与「无资源支持」态一致，不弹错）
//   2. mermaid 占位经 mermaidHost 渲染（缓存命中零成本）
//   3. 最终完整文档（buildPreviewDocument 装配 head/CSS）
//
// 取消语义：isCancelled 每轮 await 后检查——过期渲染产物（300ms 防抖期间
// 再次击键）由调用方（MarkdownPanel gen 机制）丢弃。

import { fs } from "../../ipc";
import { buildPreviewDocument, renderMarkdownPlan } from "./mdPipeline";
import { renderDiagram } from "./mermaidHost";
import { buildDataUrl, mimeForPath } from "./assets";

/** 资源 base64 缓存（按绝对路径，LRU 50 项——防逐字重渲染反复读盘） */
const resourceCache = new Map<string, Promise<string | null>>();
const RESOURCE_CACHE_MAX = 50;

function readResourceCached(absPath: string): Promise<string | null> {
  const cached = resourceCache.get(absPath);
  if (cached) return cached;
  const run = fs
    .readResourceBase64(absPath)
    .then((b64) => b64)
    .catch((err) => {
      // 沙箱外/超限/不存在 → 图片缺口（console 提示不打断整篇）
      console.warn(`[slTerminal] 预览资源读取失败: ${absPath}`, err);
      return null;
    });
  resourceCache.set(absPath, run);
  if (resourceCache.size > RESOURCE_CACHE_MAX) {
    const toDelete = [...resourceCache.keys()].slice(0, Math.floor(RESOURCE_CACHE_MAX / 2));
    for (const k of toDelete) resourceCache.delete(k);
  }
  return run;
}

export interface RenderRequest {
  /** md 源文本（面板 docRef 快照——草稿优先磁盘） */
  markdown: string;
  /** md 文件所在目录（相对资源 join 基；null = 仅盘符绝对路径引用） */
  docDir: string | null;
  /** 每轮 await 后检查——true 则丢弃产物 */
  isCancelled?: () => boolean;
}

/** 渲染并返回完整预览文档（HTML 字符串） */
export async function renderMarkdownDocument(
  req: RenderRequest,
): Promise<string> {
  const plan = renderMarkdownPlan(req.markdown, req.docDir);
  let html = plan.bodyHtml;

  // ── 资源占位替换（并行读取）──
  if (plan.images.length > 0) {
    const results = await Promise.all(
      plan.images.map((img) => readResourceCached(img.absPath)),
    );
    if (req.isCancelled?.()) return "";
    plan.images.forEach((img, i) => {
      const b64 = results[i];
      if (b64 === null) {
        // 读取失败 → 回退原 src（opaque iframe 内相对引用不可显示，保持缺口语义）
        html = html.replace(img.placeholder, img.src);
        return;
      }
      const mime = mimeForPath(img.absPath);
      if (!mime) {
        html = html.replace(img.placeholder, img.src);
        return;
      }
      html = html.replace(img.placeholder, buildDataUrl(mime, b64));
    });
  }

  // ── mermaid 占位替换（并行渲染）──
  if (plan.mermaid.length > 0) {
    const results = await Promise.all(
      plan.mermaid.map((spec) => renderDiagram(spec.code)),
    );
    if (req.isCancelled?.()) return "";
    plan.mermaid.forEach((spec, i) => {
      html = html.replace(spec.placeholder, results[i] ?? "");
    });
  }

  return buildPreviewDocument(html);
}

/** 仅测试用：清空资源缓存 */
export function _resetResourceCache(): void {
  resourceCache.clear();
}
