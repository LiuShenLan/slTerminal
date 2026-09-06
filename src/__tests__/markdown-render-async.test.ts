// markdown-render-async.test.ts — md 异步编排层测试
//
// renderMarkdownDocument：资源占位 → data: URL 替换 / 读取失败回退原 src /
// mermaid 渲染替换 / 取消语义（isCancelled 丢弃产物）/ 资源缓存复用。
// mermaid 模块 mock（同 markdown-mermaid 范式——jsdom 无布局）。

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  renderMarkdownDocument,
  _resetResourceCache,
} from "../panels/markdown/mdRenderAsync";
import { _resetMermaidCache } from "../panels/markdown/mermaidHost";

const mocks = vi.hoisted(() => {
  const mockReadResource = vi.fn();
  const mockMermaidRender = vi.fn();
  const mockMermaidInit = vi.fn();
  return {
    mockReadResource,
    mockMermaidRender,
    mockMermaidInit,
    resetAll() {
      mockReadResource.mockReset();
      mockMermaidRender.mockReset();
      mockMermaidInit.mockReset();
    },
  };
});

// 注：mock 真实源模块（ipc barrel 的命名 re-export 会被打包器静态展开直绑
// ./fs 模块——mock index 无效，须 mock "../ipc/fs"）
vi.mock("../ipc/fs", () => ({
  readResourceBase64: mocks.mockReadResource,
}));

vi.mock("mermaid", () => ({
  default: {
    initialize: mocks.mockMermaidInit,
    render: mocks.mockMermaidRender,
  },
}));

describe("renderMarkdownDocument 编排", () => {
  beforeEach(() => {
    mocks.resetAll();
    _resetResourceCache();
    _resetMermaidCache();
  });

  it("相对图片 → data: URL 替换（完整文档）", async () => {
    mocks.mockReadResource.mockResolvedValue("iVBORw0KGgo=");
    const doc = await renderMarkdownDocument({
      markdown: "![图](./a.png)",
      docDir: "C:/docs",
    });
    expect(mocks.mockReadResource).toHaveBeenCalledWith("C:/docs/a.png");
    expect(doc).toContain("data:image/png;base64,iVBORw0KGgo=");
    expect(doc.startsWith("<!doctype html>")).toBe(true);
  });

  it("资源读取失败（沙箱外/不存在）→ 回退原 src，不阻塞整篇", async () => {
    mocks.mockReadResource.mockRejectedValue(new Error("sandbox"));
    const doc = await renderMarkdownDocument({
      markdown: "![图](./secret.png)\n\n正文",
      docDir: "C:/docs",
    });
    expect(doc).toContain("./secret.png");
    expect(doc).toContain("正文");
  });

  it("资源缓存：同路径二次渲染只读一次盘", async () => {
    mocks.mockReadResource.mockResolvedValue("AAA=");
    await renderMarkdownDocument({ markdown: "![a](./a.png)", docDir: "C:/d" });
    await renderMarkdownDocument({ markdown: "![a](./a.png)", docDir: "C:/d" });
    expect(mocks.mockReadResource).toHaveBeenCalledTimes(1);
  });

  it("mermaid fence → 渲染 SVG 替换占位", async () => {
    mocks.mockMermaidRender.mockResolvedValue({ svg: "<svg>graph</svg>" });
    const doc = await renderMarkdownDocument({
      markdown: "```mermaid\ngraph TD\n  A --> B\n```",
      docDir: null,
    });
    expect(mocks.mockMermaidRender).toHaveBeenCalledTimes(1);
    expect(doc).toContain("<svg>graph</svg>");
    // 占位符无残留
    expect(doc).not.toContain("%%SLTERM_MERMAID");
  });

  it("mermaid 渲染失败 → 错误占位卡注入（不抛出）", async () => {
    mocks.mockMermaidRender.mockRejectedValue(new Error("boom"));
    const doc = await renderMarkdownDocument({
      markdown: "```mermaid\ngraph TD\n  A\n```",
      docDir: null,
    });
    expect(doc).toContain("slterm-mermaid-error");
  });

  it("取消语义：资源等待后被取消 → 返回空串（gen 过期产物丢弃）", async () => {
    let resolveRead: (v: string) => void = () => {};
    mocks.mockReadResource.mockReturnValue(
      new Promise<string>((r) => {
        resolveRead = r;
      }),
    );
    let cancelled = false;
    const pending = renderMarkdownDocument({
      markdown: "![a](./a.png)",
      docDir: "C:/d",
      isCancelled: () => cancelled,
    });
    cancelled = true;
    resolveRead("AAA=");
    await expect(pending).resolves.toBe("");
  });
});
