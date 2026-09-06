// markdown-links.test.ts — md 预览链接点击分类测试

import { describe, it, expect } from "vitest";
import { classifyLink } from "../panels/markdown/linkPolicy";

describe("classifyLink", () => {
  it("http(s) → external（系统浏览器）", () => {
    expect(classifyLink("https://example.com", "C:/d")).toEqual({
      kind: "external",
      url: "https://example.com",
    });
    expect(classifyLink("http://x.com/a", "C:/d").kind).toBe("external");
  });

  it("mailto/tel → external", () => {
    expect(classifyLink("mailto:a@b.com", "C:/d").kind).toBe("external");
    expect(classifyLink("tel:+8613800000000", "C:/d").kind).toBe("external");
  });

  it("相对本地路径 → local（docDir join 绝对化）", () => {
    expect(classifyLink("./other.md", "C:/docs")).toEqual({
      kind: "local",
      absPath: "C:/docs/other.md",
    });
    expect(classifyLink("../doc.html", "C:/docs/notes")).toEqual({
      kind: "local",
      absPath: "C:/docs/doc.html",
    });
  });

  it("盘符绝对路径 → local（沙箱由打开链路把关）", () => {
    expect(classifyLink("C:/other/a.md", null)).toEqual({
      kind: "local",
      absPath: "C:/other/a.md",
    });
  });

  it("# 锚点 → fragment（md 渲染标题无 id，忽略点击）", () => {
    expect(classifyLink("#sec", "C:/d").kind).toBe("fragment");
  });

  it("危险/不可导航协议与空 → ignored", () => {
    expect(classifyLink("", "C:/d").kind).toBe("ignored");
    expect(classifyLink("javascript:alert(1)", "C:/d").kind).toBe("ignored");
    expect(classifyLink("data:text/html,x", "C:/d").kind).toBe("ignored");
    expect(classifyLink("file:///C:/x.md", "C:/d").kind).toBe("ignored");
  });

  it("docDir=null 且相对 → ignored（无基不可解析）", () => {
    expect(classifyLink("./a.md", null).kind).toBe("ignored");
  });

  it("首尾空白容忍", () => {
    expect(classifyLink("  https://x.com  ", "C:/d").kind).toBe("external");
  });
});
