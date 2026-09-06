// linkPolicy.ts — md 预览链接点击分类（纯函数单点）
//
// iframe 内注入 linkRouter 段把 <a> 点击上行到父窗口，父侧按本分类分派：
//   external（http(s)/mailto/tel）→ 系统浏览器（plugin-opener）
//   local（相对/绝对本地路径）→ 应用内共享打开链路（openFileInActivePage）
//   fragment（# 锚点）→ 忽略（md 渲染无标题锚点 id，markdown-it 不产 id）
//   ignored（data/blob/javascript/空）→ 忽略
// 打开失败（目录/不存在/沙箱外）由打开链路静默忽略——不弹错。

import { absolutizeRef } from "./assets";

export type LinkKind =
  | { kind: "external"; url: string }
  | { kind: "local"; absPath: string }
  | { kind: "fragment" }
  | { kind: "ignored" };

/** href 分类（docDir = md 文件所在目录，相对引用绝对化的基） */
export function classifyLink(href: string, docDir: string | null): LinkKind {
  const h = href.trim();
  if (h === "") return { kind: "ignored" };

  // 锚点：md 渲染的标题无 id（markdown-it 默认不产），本地文档内锚点不可达 → 忽略
  if (h.startsWith("#")) return { kind: "fragment" };

  // 盘符绝对路径（Windows 路径与协议同形——须先于协议判定；docDir 无关，直接 local）
  if (/^[A-Za-z]:[\\/]/.test(h)) {
    return { kind: "local", absPath: h.replace(/\\/g, "/") };
  }

  // 协议形态（含 javascript:/data:/blob: 危险或不可导航形态）
  const schemeMatch = /^([a-z][a-z0-9+.-]*):/i.exec(h);
  if (schemeMatch) {
    const scheme = schemeMatch[1]!.toLowerCase();
    if (scheme === "http" || scheme === "https") {
      return { kind: "external", url: h };
    }
    if (scheme === "mailto" || scheme === "tel") {
      return { kind: "external", url: h };
    }
    // javascript:/data:/blob:/file: 等一律不导航
    return { kind: "ignored" };
  }

  // 本地路径：复用 assets 相对绝对化（相对 docDir join；盘符绝对直用；../ 越界
  // 不拦——打开链路与后端沙箱最终把关，失败静默）
  const absPath = absolutizeRef(h, docDir);
  if (!absPath) return { kind: "ignored" };
  return { kind: "local", absPath };
}
