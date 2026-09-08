// assets.ts — md 预览本地相对资源解析（纯函数单点）
//
// md 渲染产物中相对路径资源（markdown 图片语法与 raw HTML <img>/<source>）经
// 后端沙箱通道读取后以 data: URL 内联（opaque iframe 无相对路径概念）。本文件
// 只做同步纯函数：URL 分类 / 相对绝对化 / MIME 白名单——读取与替换在
// mdRenderAsync（异步编排）。实际沙箱边界由后端 fs_read_resource 把关
//（项目根外读取被拒 → 丢图缺口，不弹错）。

/** 资源 MIME 白名单（扩展名 → mime；白名单外资源不收集——保持 src 原样）
 *
 * svg 显式禁用（CP-035/S10-④）：image/svg+xml 不在白名单——svg 载体可嵌
 * 脚本，渲染域（预览 iframe，无 CSP）内联风险面大，<img> 惰性上下文仅为
 * W3C 行为单点，不做安全边界依据；本地 .svg 引用与白名单外扩展同语义
 * （src 原样 → 缺口语义）。恢复须重审并登记 ADR-0018/0019。 */
export const ASSET_MIME_BY_EXT: Readonly<Record<string, string>> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  avif: "image/avif",
  bmp: "image/bmp",
  ico: "image/x-icon",
};

/** 扩展名 → MIME；白名单外返回 null（跳过该资源） */
export function mimeForPath(absPath: string): string | null {
  const lastDot = absPath.lastIndexOf(".");
  if (lastDot < 0) return null;
  const ext = absPath.slice(lastDot + 1).toLowerCase();
  return ASSET_MIME_BY_EXT[ext] ?? null;
}

/** data: URL 构造 */
export function buildDataUrl(mime: string, base64: string): string {
  return `data:${mime};base64,${base64}`;
}

/** 裸文件名/相对引用（无协议、非锚点；含 C:/ 盘符绝对路径——仍可解析，沙箱由后端把关） */
export function isLocalRef(href: string): boolean {
  if (href === "") return false;
  // 盘符绝对路径（Windows 路径与协议同形——须先于协议判定）
  if (/^[A-Za-z]:[\\/]/.test(href)) return true;
  // 协议形态一律跳过（http/data/blob/javascript/mailto/tel/file/#）
  if (/^[a-z][a-z0-9+.-]*:/i.test(href)) return false;
  if (href.startsWith("#")) return false;
  return true;
}

/**
 * 相对引用绝对化：以 docDir（md 文件所在目录）为基 join（正斜杠输出）。
 * docDir 为 null（无目录上下文）时仅接受已绝对化的盘符路径，否则返回 null。
 * ../ 出根不做前端判定——沙箱边界在后端，打开失败/读取失败静默丢。
 */
export function absolutizeRef(href: string, docDir: string | null): string | null {
  if (/^[A-Za-z]:[\\/]/.test(href)) {
    // 已是盘符绝对路径 → 正斜杠归一
    return href.replace(/\\/g, "/");
  }
  if (!docDir) return null;
  const dir = docDir.replace(/\\/g, "/").replace(/\/+$/, "");
  const ref = href.replace(/\\/g, "/");
  // 去片段（资源引用不带 #）
  const hashIdx = ref.indexOf("#");
  const clean = hashIdx >= 0 ? ref.slice(0, hashIdx) : ref;
  const joined = `${dir}/${clean}`;
  // 归一 ./ 与 ../（不做越界解析——后端沙箱最终把关）
  const parts = joined.split("/");
  const stack: string[] = [];
  for (const p of parts) {
    if (p === "." || p === "") continue;
    if (p === "..") {
      // 盘符根（如 ["C:"]）不可再弹——越界 ../ 由后端沙箱拒绝
      if (stack.length > 1 || !/^[A-Za-z]:$/.test(stack[0] ?? "")) {
        stack.pop();
      }
    } else {
      stack.push(p);
    }
  }
  const out = stack.join("/");
  return /^[A-Za-z]:/.test(out) ? out : null;
}
