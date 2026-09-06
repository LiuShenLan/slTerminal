// markdown-mermaid.test.ts — 宿主侧 mermaid 渲染编排测试
//
// mermaid v11 需真实浏览器 DOM 布局（jsdom 无）——vi.mock 隔离；本文件锁
// mermaidHost 编排：惰性 dynamic import 单例 / 缓存命中不重复渲染 / 失败占位卡
// 不抛 / 主题初始化参数。真实渲染由 L4 fixture + 视觉人工验收（豁免登记）。

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  renderDiagram,
  _resetMermaidCache,
} from "../panels/markdown/mermaidHost";

const { mockRender, mockInitialize } = vi.hoisted(() => ({
  mockRender: vi.fn(),
  mockInitialize: vi.fn(),
}));

vi.mock("mermaid", () => ({
  default: {
    initialize: mockInitialize,
    render: mockRender,
  },
}));

/** 使 randomHex 稳定（断言 render id 形态用正则，不必 mock） */

describe("mermaidHost 编排", () => {
  beforeEach(() => {
    _resetMermaidCache();
    mockInitialize.mockClear();
    mockRender.mockClear();
  });

  afterEach(() => {
    _resetMermaidCache();
  });

  it("渲染成功 → 返回 svg；initialize 主题参数（dark + 内容色变量）", async () => {
    mockRender.mockResolvedValue({ svg: "<svg>graph</svg>" });
    const svg = await renderDiagram("graph TD\n A-->B");
    expect(svg).toBe("<svg>graph</svg>");
    expect(mockRender).toHaveBeenCalledTimes(1);
    // render id 形态：mmd-<32hex>
    const id = mockRender.mock.calls[0]![0] as string;
    expect(id).toMatch(/^mmd-[0-9a-f]{32}$/);
    // initialize 暗色主题配置
    expect(mockInitialize).toHaveBeenCalledWith(
      expect.objectContaining({
        startOnLoad: false,
        securityLevel: "strict",
        theme: "dark",
      }),
    );
  });

  it("同 code 缓存命中：不重复渲染", async () => {
    mockRender.mockResolvedValue({ svg: "<svg>1</svg>" });
    const [a, b] = await Promise.all([
      renderDiagram("graph TD\n A-->B"),
      renderDiagram("graph TD\n A-->B"),
    ]);
    expect(a).toBe(b);
    expect(mockRender).toHaveBeenCalledTimes(1);
  });

  it("渲染失败 → 错误占位卡（含转义原码），不抛出", async () => {
    mockRender.mockRejectedValue(new Error("layout failed"));
    const out = await renderDiagram("graph TD\n <bad>");
    expect(out).toContain("slterm-mermaid-error");
    expect(out).toContain("渲染失败");
    // 原码转义（<bad> 不被当作 HTML 注入）
    expect(out).toContain("&lt;bad&gt;");
    expect(out).not.toContain("<bad>");
  });
});
