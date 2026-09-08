// mdPipeline.ts — markdown-it 渲染管线（同步纯函数主体）
//
// 输入 md 源文本 → 输出带占位符的 body HTML + 资源/图表清单：
//   - mermaid fence（```mermaid）→ 随机占位符（crypto.randomUUID），原文进清单
//   - 本地相对资源（<img>/<source> 的 src，含 markdown 图片语法产物）→
//     %%SLTERM_ASSET_n%% 占位 + 绝对路径清单（MIME 白名单外不动）
// 占位符替换（异步：资源读取/mermaid 渲染）在 mdRenderAsync 完成；最终文档
// 装配（head + 暗色排版 CSS + hljs 主题 + KaTeX 内联字体）经 buildPreviewDocument。
//
// 信任模型（ADR-0017）：html: true 透传 raw HTML——与 htmlviewer 渲染同等信任
// （事件属性执行；<script> 因 injectScript 转义纪律与 htmlviewer 同态静态化）。

import MarkdownIt from "markdown-it";
import hljs from "highlight.js";
import type { LanguageFn } from "highlight.js";
import texmath from "markdown-it-texmath";
import taskLists from "markdown-it-task-lists";
import katex from "katex";
import javascript from "highlight.js/lib/languages/javascript";
import typescript from "highlight.js/lib/languages/typescript";
import json from "highlight.js/lib/languages/json";
import xml from "highlight.js/lib/languages/xml";
import css from "highlight.js/lib/languages/css";
import bash from "highlight.js/lib/languages/bash";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import sql from "highlight.js/lib/languages/sql";
import yaml from "highlight.js/lib/languages/yaml";
import diff from "highlight.js/lib/languages/diff";
import markdownLang from "highlight.js/lib/languages/markdown";
import { buildMdPreviewStyleCss } from "./mdPreviewStyle";
import { KATEX_INLINE_CSS } from "./generated/katexInlineCss";
import { mimeForPath, isLocalRef, absolutizeRef } from "./assets";

/** 图片资源占位前缀/后缀（防正文碰撞：随机段在占位符内） */
const ASSET_PLACEHOLDER_PREFIX = "%%SLTERM_ASSET_";
const ASSET_PLACEHOLDER_SUFFIX = "%%";

/** mermaid 占位符前缀/后缀 */
const MERMAID_PLACEHOLDER_PREFIX = "%%SLTERM_MERMAID_";
const MERMAID_PLACEHOLDER_SUFFIX = "%%";

/** 128 位随机 hex（crypto.getRandomValues——jsdom/WebView2 均有；randomUUID 在
 *  jsdom node 环境缺失，统一用此实现，createNonce 同款） */
export function randomHex(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  let out = "";
  for (const b of bytes) out += b.toString(16).padStart(2, "0");
  return out;
}

export interface MdImageAsset {
  /** 占位符（body html 中待替换文本） */
  placeholder: string;
  /** 原 src 值（解码后；读取失败时回退展示用） */
  src: string;
  /** 绝对化路径（docDir join / 盘符直用；后端沙箱最终把关） */
  absPath: string;
}

export interface MermaidSpec {
  placeholder: string;
  code: string;
}

/** 单次渲染计划（同步产出；占位替换在异步编排层） */
export interface MdRenderPlan {
  /** 含占位符的 body HTML 片段 */
  bodyHtml: string;
  images: MdImageAsset[];
  mermaid: MermaidSpec[];
}

/** hljs 语言白名单（静态 import——只挂 md 常见围栏语言，防全量注册体积） */
const HLJS_LANG_FNS: Record<string, LanguageFn> = {
  javascript,
  typescript,
  json,
  // html 经 xml 语法高亮（xml 语言定义覆盖 html——hljs 惯例）
  html: xml,
  xml,
  css,
  bash,
  python,
  rust,
  go,
  java,
  c,
  cpp,
  sql,
  yaml,
  diff,
  markdown: markdownLang,
};

/** hljs 语言是否已注册（幂等注册——模块级单例只执行一次） */
let hljsReady = false;
function ensureHljsLanguages(): void {
  if (hljsReady) return;
  hljsReady = true;
  for (const [name, fn] of Object.entries(HLJS_LANG_FNS)) {
    if (!hljs.getLanguage(name)) {
      try {
        hljs.registerLanguage(name, fn);
      } catch {
        // 语言注册失败静默（该语言走无高亮回退）
      }
    }
  }
}

/** markdown-it 高亮回调：白名单语言高亮，失败/未知回退空串（md-it 自动转义） */
function highlightFence(code: string, lang: string): string {
  if (!lang) return "";
  const name = lang.toLowerCase();
  // shell/console 等别名映射 bash；js/ts 常用别名归一
  const resolved =
    name === "shell" || name === "console" || name === "sh" || name === "zsh"
      ? "bash"
      : name === "js" || name === "node" || name === "jsx"
        ? "javascript"
        : name === "ts" || name === "tsx"
          ? "typescript"
          : name === "python3"
            ? "python"
            : name;
  if (!(resolved in HLJS_LANG_FNS) || !hljs.getLanguage(resolved)) return "";
  try {
    return hljs.highlight(code, { language: resolved, ignoreIllegals: true }).value;
  } catch {
    return "";
  }
}

/** 解析 HTML 标签内 src 属性值（&amp; 等实体解码后判定；占位替换在原文上做） */
function decodeAttr(s: string): string {
  return s
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");
}

/** 模块级单例 md-it 实例（解析器无状态可复用于多次渲染） */
type MarkdownItInstance = InstanceType<typeof MarkdownIt>;
let mdIt: MarkdownItInstance | null = null;
function getParser(): MarkdownItInstance {
  if (mdIt) return mdIt;

  ensureHljsLanguages();

  const md = new MarkdownIt({
    html: true, // raw HTML 透传——与 htmlviewer 同等信任（ADR-0017）
    linkify: true, // GFM 自动链接
    highlight: highlightFence,
  })
    // 数学：katex 引擎 tokenize 期就地渲染（renderToString 纯字符串，无 DOM 依赖）
    .use(texmath, {
      engine: katex,
      delimiters: "dollars",
      katexOptions: { throwOnError: false },
    })
    // GFM 任务列表（- [ ] / - [x]）
    .use(taskLists, { enabled: true, label: true });

  // mermaid fence 拦截：```mermaid → 占位符（原文 code 进 MermaidSpec 清单）
  const defaultFence = md.renderer.rules.fence!;
  md.renderer.rules.fence = (tokens, idx, options, env, self) => {
    const token = tokens[idx];
    const info = token.info ?? "";
    const first = info.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
    if (first === "mermaid") {
      const placeholder = `${MERMAID_PLACEHOLDER_PREFIX}${randomHex()}${MERMAID_PLACEHOLDER_SUFFIX}`;
      // 挂到 env 收集（render 时经 env 回传 mdPipeline 层）
      const collector = (env as { __sltermMermaid?: MermaidSpec[] }).__sltermMermaid;
      if (collector) {
        collector.push({ placeholder, code: token.content });
      }
      return `${placeholder}\n`;
    }
    return defaultFence(tokens, idx, options, env, self);
  };

  mdIt = md;
  return md;
}

/**
 * 渲染 md 源为带占位符的 body HTML（同步）。
 * docDir：md 文件所在目录（相对资源 join 基；null = 仅接受盘符绝对路径引用）。
 */
export function renderMarkdownPlan(markdown: string, docDir: string | null): MdRenderPlan {
  const parser = getParser();

  // mermaid 收集器经 env 通道（fence rule 与 render 调用同 env 对象）
  const mermaidCollector: MermaidSpec[] = [];
  const env: { __sltermMermaid?: MermaidSpec[] } = { __sltermMermaid: mermaidCollector };
  let bodyHtml = parser.render(markdown, env);

  // ── 本地相对资源收集（后处理 <img>/<source> src；markdown 图片语法产物同被扫到）──
  const images: MdImageAsset[] = [];
  const TAG_RE = /<(img|source)\b[^>]*>/gi;
  bodyHtml = bodyHtml.replace(TAG_RE, (tag: string) => {
    const srcMatch = /src="([^"]*)"/i.exec(tag);
    if (!srcMatch) return tag;
    const srcRaw = srcMatch[1] ?? "";
    const src = decodeAttr(srcRaw);
    if (!isLocalRef(src)) return tag;
    const absPath = absolutizeRef(src, docDir);
    if (!absPath) return tag;
    if (!mimeForPath(absPath)) return tag; // MIME 白名单外不收集（保持原样）
    const placeholder = `${ASSET_PLACEHOLDER_PREFIX}${images.length}${ASSET_PLACEHOLDER_SUFFIX}`;
    images.push({ placeholder, src, absPath });
    // 仅替换该 src 值（保留标签其余属性）
    return tag.replace(/src="[^"]*"/i, `src="${placeholder}"`);
  });

  return { bodyHtml, images, mermaid: mermaidCollector };
}

/** 装配完整预览文档（head：暗色排版 + hljs 主题 + KaTeX 内联字体；排版 CSS 每次渲染现拼取 active 方案） */
export function buildPreviewDocument(bodyHtml: string): string {
  return (
    `<!doctype html><html><head><meta charset="utf-8">` +
    `<style>${buildMdPreviewStyleCss()}${KATEX_INLINE_CSS}</style>` +
    `</head><body>${bodyHtml}</body></html>`
  );
}

/** 供测试断言解析器配置（配置项不随渲染变化） */
export function _getParserConfig(): { html: boolean; linkify: boolean } {
  const p = getParser();
  return { html: p.options.html ?? false, linkify: p.options.linkify ?? false };
}
