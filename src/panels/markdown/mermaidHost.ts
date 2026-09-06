// mermaidHost.ts — 宿主侧 mermaid 渲染（动态 import + 缓存 + 失败占位）
//
// md 预览 iframe（opaque origin + 静态注入）不跑 mermaid 运行时——图在宿主侧
// 渲染成 SVG 字符串后经占位替换注入（iframe 每次重建零布局成本、CSP 零新增）。
// mermaid v11 render 需真实浏览器 DOM/字体度量（jsdom 无布局——L2 vi.mock
// 覆盖编排，真实渲染由 L4 fixture + 视觉人工验收，豁免登记 test-exemptions）。
//
// 主题：dark + themeVariables 映射 linear 内容色（暗色协调，双轨豁免同
// mdPreviewStyle）。失败 → 错误占位卡（不阻塞整篇，含原码便于排查）。

import { randomHex } from "./mdPipeline";

const SVG_CACHE = new Map<string, Promise<string>>();
/** 缓存上限（防逐字重渲染反复跑布局；超限清最旧一半） */
const CACHE_MAX = 50;

/** mermaid 模块惰性单例（vite 自动分包——启动不背 ~2MB） */
let mermaidPromise: Promise<{ default: typeof import("mermaid").default }> | null = null;
function loadMermaid() {
  if (!mermaidPromise) {
    mermaidPromise = import("mermaid").then((mod) => {
      const mermaid = mod.default;
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: "strict",
        theme: "dark",
        themeVariables: {
          background: "transparent",
          primaryColor: "rgba(110,159,242,0.14)",
          primaryBorderColor: "#6e9ff2",
          primaryTextColor: "#cfcac1",
          secondaryColor: "rgba(214,178,94,0.10)",
          secondaryBorderColor: "#d6b25e",
          tertiaryColor: "rgba(255,255,255,0.05)",
          lineColor: "#8a857d",
          textColor: "#cfcac1",
          fontSize: "14px",
          fontFamily: "Segoe UI, Microsoft YaHei UI, sans-serif",
        },
      });
      return mod;
    });
  }
  return mermaidPromise;
}

/** 渲染失败错误占位卡（含原码——与成功 SVG 同注入语义） */
function errorCard(code: string): string {
  const esc = code
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  return (
    `<div class="slterm-mermaid-error">mermaid 渲染失败` +
    `<pre>${esc}</pre></div>`
  );
}

/**
 * 渲染 mermaid 图为 SVG 字符串（Promise 缓存按 code 复用）。
 * 失败返回错误占位卡（不抛出）。
 */
export async function renderDiagram(code: string): Promise<string> {
  const cached = SVG_CACHE.get(code);
  if (cached) return cached;

  const run = (async () => {
    try {
      const mod = await loadMermaid();
      const id = `mmd-${randomHex()}`;
      const { svg } = await mod.default.render(id, code);
      return svg;
    } catch (err) {
      console.warn("[slTerminal] mermaid 渲染失败:", err);
      return errorCard(code);
    }
  })();

  SVG_CACHE.set(code, run);
  // 缓存上限维护：超限清最旧一半（Map 插入序）
  if (SVG_CACHE.size > CACHE_MAX) {
    const toDelete = [...SVG_CACHE.keys()].slice(0, Math.floor(CACHE_MAX / 2));
    for (const k of toDelete) SVG_CACHE.delete(k);
  }
  return run;
}

/** 仅测试用：清空缓存（测试隔离） */
export function _resetMermaidCache(): void {
  SVG_CACHE.clear();
  mermaidPromise = null;
}
