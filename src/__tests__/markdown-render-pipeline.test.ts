// markdown-render-pipeline.test.ts — md 渲染管线测试
//
// 同步纯管线 renderMarkdownPlan / buildPreviewDocument：
// GFM 结构 / raw HTML 透传（信任模型 ADR-0017）/ hljs 围栏高亮 /
// KaTeX 数学就地渲染 / mermaid fence 占位收集 / 本地相对资源收集与跳过分类。
// 异步占位替换（资源读取/mermaid 渲染）在 mdRenderAsync 层测试。

import { describe, it, expect } from "vitest";
import {
  renderMarkdownPlan,
  buildPreviewDocument,
  _getParserConfig,
} from "../panels/markdown/mdPipeline";

describe("markdown 渲染管线", () => {
  it("解析器配置：html 透传 + linkify（信任模型锚点）", () => {
    const cfg = _getParserConfig();
    expect(cfg.html).toBe(true);
    expect(cfg.linkify).toBe(true);
  });

  it("GFM 结构：标题/表格/删除线/任务列表渲染为 HTML", () => {
    const src = [
      "# 标题",
      "",
      "| a | b |",
      "|---|---|",
      "| 1 | 2 |",
      "",
      "~~删除~~",
      "",
      "- [x] 完成项",
      "- [ ] 未完成项",
    ].join("\n");
    const { bodyHtml } = renderMarkdownPlan(src, null);
    expect(bodyHtml).toContain("<h1>标题</h1>");
    expect(bodyHtml).toContain("<table>");
    expect(bodyHtml).toContain("<s>删除</s>");
    expect(bodyHtml).toContain('type="checkbox"');
    expect(bodyHtml).toContain('checked=""');
  });

  it("raw HTML 透传（html:true——脚本语义与 htmlviewer 同态）", () => {
    const { bodyHtml } = renderMarkdownPlan('<div class="raw">块</div>\n\n正文', null);
    expect(bodyHtml).toContain('<div class="raw">块</div>');
  });

  it("fenced code 高亮：白名单语言输出 hljs span，未知语言原样转义", () => {
    const { bodyHtml } = renderMarkdownPlan(
      '```js\nconst x = 1;\n```\n\n```nolang-xyz\n<raw>\n```',
      null,
    );
    // js 高亮：关键字 span
    expect(bodyHtml).toContain('class="hljs-keyword"');
    expect(bodyHtml).toContain("<span");
    // 未知语言 → 转义原文（无 hljs class）
    expect(bodyHtml).toContain("&lt;raw&gt;");
  });

  it("KaTeX 数学：行内 $..$ 与块级 $$..$$ 就地渲染（throwOnError=false 失败不抛）", () => {
    const { bodyHtml } = renderMarkdownPlan("行内 $x^2$ 公式", null);
    expect(bodyHtml).toContain("katex");
    const { bodyHtml: block } = renderMarkdownPlan("$$\n\\int_0^1 x dx\n$$", null);
    expect(block).toContain("katex-display");
    // 渲染失败（非法公式）→ 不抛异常、不中断整篇
    const { bodyHtml: bad } = renderMarkdownPlan("$\\notacommand{}$", null);
    expect(bad).toContain("katex");
  });

  it("mermaid fence → 唯一占位符 + 原码入清单（正文无占位符残留内容）", () => {
    const src = "```mermaid\ngraph TD\n  A --> B\n```";
    const { bodyHtml, mermaid } = renderMarkdownPlan(src, null);
    expect(mermaid).toHaveLength(1);
    expect(mermaid[0]!.code).toContain("graph TD");
    // 占位符 = 128 位随机 hex 包裹的标记（防正文碰撞）
    const m = /%%SLTERM_MERMAID_([0-9a-f]{32})%%/.exec(bodyHtml);
    expect(m).not.toBeNull();
    expect(mermaid[0]!.placeholder).toBe(m![0]);
    // fence 原文不出现（被占位替换）
    expect(bodyHtml).not.toContain("graph TD");
  });

  it("非 mermaid fence 走默认渲染（不影响普通代码块）", () => {
    const { bodyHtml, mermaid } = renderMarkdownPlan("```text\nplain\n```", null);
    expect(mermaid).toHaveLength(0);
    expect(bodyHtml).toContain("plain");
  });

  it("相对图片（markdown 语法与 raw HTML）→ 占位 + 绝对路径清单", () => {
    const src = [
      "![图](./img/a.png)",
      "",
      '<img src="../b.webp" alt="raw">',
    ].join("\n");
    const { bodyHtml, images } = renderMarkdownPlan(src, "C:/docs/notes");
    expect(images).toHaveLength(2);
    expect(images[0]).toMatchObject({
      absPath: "C:/docs/notes/img/a.png",
      src: "./img/a.png",
    });
    expect(images[1]!.absPath).toBe("C:/docs/b.webp");
    expect(bodyHtml).toContain('src="%%SLTERM_ASSET_0%%"');
    expect(bodyHtml).toContain('src="%%SLTERM_ASSET_1%%"');
  });

  it("资源跳过分类：http/data/#/MIME 白名单外/盘符外相对 不收集", () => {
    const src = [
      "![外链](https://x.com/a.png)",
      "![数据](data:image/png;base64,AAAA)",
      '<img src="#frag">',
      '![非白名单](./doc.md)',
      '![绝对](C:/pics/b.jpg)',
    ].join("\n");
    const { bodyHtml, images } = renderMarkdownPlan(src, "C:/docs");
    // 只有绝对盘符图被收集（白名单内）；其余保持原样
    expect(images).toHaveLength(1);
    expect(images[0]!.absPath).toBe("C:/pics/b.jpg");
    expect(bodyHtml).toContain("https://x.com/a.png");
    expect(bodyHtml).toContain("./doc.md");
  });

  it("docDir=null 时仅盘符绝对路径可收集（无目录上下文）", () => {
    const { images } = renderMarkdownPlan("![相对](./a.png)", null);
    expect(images).toHaveLength(0);
    const { images: abs } = renderMarkdownPlan("![绝对](D:/x/b.png)", null);
    expect(abs).toHaveLength(1);
  });

  it("buildPreviewDocument：完整文档含排版/高亮/KaTeX 内联字体 CSS", () => {
    const doc = buildPreviewDocument("<h1>t</h1>");
    expect(doc.startsWith("<!doctype html>")).toBe(true);
    expect(doc).toContain("<style>");
    // 暗色排版（body 色值锚点）
    expect(doc).toContain("#b3aea6");
    // hljs 主题
    expect(doc).toContain(".hljs-keyword");
    // KaTeX 内联字体（data:font/woff2——ADR-0018 产物）
    expect(doc).toContain("data:font/woff2;base64,");
    expect(doc).toContain("KaTeX_Main");
    expect(doc).toContain("<h1>t</h1>");
    // 无相对 url(fonts 残留（未内联的字体引用）
    expect(doc).not.toContain("url(fonts/");
  });
});
