// markdown-assets.test.ts — md 预览本地资源解析纯函数测试

import { describe, it, expect } from "vitest";
import {
  ASSET_MIME_BY_EXT,
  mimeForPath,
  buildDataUrl,
  isLocalRef,
  absolutizeRef,
} from "../panels/markdown/assets";

describe("资源 MIME 白名单", () => {
  it("常见图片扩展名映射", () => {
    expect(ASSET_MIME_BY_EXT.png).toBe("image/png");
    expect(ASSET_MIME_BY_EXT.jpg).toBe("image/jpeg");
  });

  it("svg MIME 不在资源内联白名单（image/svg+xml 显式禁用，CP-035）", () => {
    // svg 载体可嵌脚本，预览域 script-src 'unsafe-inline' 下内联 svg 风险面大——白名单剔除后本地
    // .svg 引用不收集（与白名单外扩展同语义，src 原样 → 缺口语义）
    expect(ASSET_MIME_BY_EXT.svg).toBeUndefined();
    expect(mimeForPath("C:/a/icon.svg")).toBeNull();
  });

  it("mimeForPath：大小写不敏感；白名单外 null", () => {
    expect(mimeForPath("C:/a/IMG.PNG")).toBe("image/png");
    expect(mimeForPath("C:/a/doc.md")).toBeNull();
    expect(mimeForPath("C:/a/noext")).toBeNull();
  });

  it("buildDataUrl 形态", () => {
    expect(buildDataUrl("image/png", "AAAA")).toBe("data:image/png;base64,AAAA");
  });
});

describe("isLocalRef 分类", () => {
  it("相对/裸名/盘符绝对 → 本地", () => {
    expect(isLocalRef("./a.png")).toBe(true);
    expect(isLocalRef("../b.png")).toBe(true);
    expect(isLocalRef("a.png")).toBe(true);
    expect(isLocalRef("C:/pics/b.png")).toBe(true);
    expect(isLocalRef("D:\\pics\\b.png")).toBe(true);
  });

  it("协议/锚点/空 → 非本地", () => {
    expect(isLocalRef("")).toBe(false);
    expect(isLocalRef("#frag")).toBe(false);
    expect(isLocalRef("https://x.com/a.png")).toBe(false);
    expect(isLocalRef("data:image/png;base64,AAAA")).toBe(false);
    expect(isLocalRef("file:///C:/a.png")).toBe(false);
    expect(isLocalRef("javascript:alert(1)")).toBe(false);
  });
});

describe("absolutizeRef 绝对化", () => {
  it("相对 join docDir（正斜杠归一）", () => {
    expect(absolutizeRef("./img/a.png", "C:/docs/notes")).toBe("C:/docs/notes/img/a.png");
    expect(absolutizeRef("../b.webp", "C:/docs/notes/")).toBe("C:/docs/b.webp");
    expect(absolutizeRef("a.png", "C:\\docs\\notes")).toBe("C:/docs/notes/a.png");
  });

  it("去 # 片段", () => {
    expect(absolutizeRef("./a.png#frag", "C:/d")).toBe("C:/d/a.png");
  });

  it("盘符绝对直用（归一斜杠）", () => {
    expect(absolutizeRef("D:/x/b.jpg", null)).toBe("D:/x/b.jpg");
    expect(absolutizeRef("D:\\x\\b.jpg", null)).toBe("D:/x/b.jpg");
  });

  it("docDir=null 时相对引用不可绝对化", () => {
    expect(absolutizeRef("./a.png", null)).toBeNull();
  });

  it("../ 越界不越盘符根（越界由后端沙箱拒绝）", () => {
    // C:/docs 内 ../../.. 越过盘符——栈保底不弹盘符
    const out = absolutizeRef("../../../x.png", "C:/docs");
    expect(out).toBe("C:/x.png");
  });
});
