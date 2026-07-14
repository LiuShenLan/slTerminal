// inject-script.test.ts — injectScript 纯函数测试

import { describe, it, expect } from "vitest";
import { injectScript } from "../lib/injectScript";

const SCRIPT = "<script>/* FORWARDER */</script>";
const MARKER = "__slterm_key";

describe("injectScript", () => {
  // ==========================================================================
  // I1: 正常 HTML 插入到 </head> 之前
  // ==========================================================================
  it("正常 HTML 插入到 </head> 之前", () => {
    const html =
      "<html><head><title>T</title></head><body><p>Hello</p></body></html>";
    const result = injectScript(html, SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    // 脚本在 </head> 之前
    const headIdx = result.indexOf("</head>");
    const scriptIdx = result.indexOf(SCRIPT);
    expect(scriptIdx).toBeLessThan(headIdx);
    // 原始内容保留
    expect(result).toContain("<title>T</title>");
    expect(result).toContain("<p>Hello</p>");
  });

  // ==========================================================================
  // I2: 无 </head> 插入到 <body 之前
  // ==========================================================================
  it("无 </head> 时插入到 <body 之前", () => {
    const html = "<html><body>content</body></html>";
    const result = injectScript(html, SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    // 脚本在 <body 之前（SCRIPT 后紧跟 <body）
    const scriptIdx = result.indexOf(SCRIPT);
    const bodyIdx = result.indexOf("<body");
    expect(scriptIdx).toBeLessThan(bodyIdx);
    expect(result).toContain("content");
  });

  // ==========================================================================
  // I3: 无 head 无 body 追加到 </html> 之前
  // ==========================================================================
  it("无 head 无 body 追加到 </html> 之前", () => {
    const html = "<html></html>";
    const result = injectScript(html, SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    // 脚本在 </html> 之前
    const scriptIdx = result.indexOf(SCRIPT);
    const htmlCloseIdx = result.indexOf("</html>");
    expect(scriptIdx).toBeLessThan(htmlCloseIdx);
  });

  // ==========================================================================
  // I4: 空字符串 → 构造最小完整文档
  // ==========================================================================
  it("空字符串返回最小完整 HTML 文档", () => {
    const result = injectScript("", SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    expect(result).toContain("<html>");
    expect(result).toContain("<body>");
    expect(result).toContain("</body>");
    expect(result).toContain("</html>");
  });

  // ==========================================================================
  // I5: 含 <!DOCTYPE> 保留 DOCTYPE
  // ==========================================================================
  it("含 <!DOCTYPE> 保留声明并正确插入脚本", () => {
    const html =
      "<!DOCTYPE html>\n<html><head></head><body>test</body></html>";
    const result = injectScript(html, SCRIPT, MARKER);
    expect(result).toContain("<!DOCTYPE html>");
    expect(result).toContain(SCRIPT);
    // 脚本在 </head> 之前
    const headIdx = result.indexOf("</head>");
    const scriptIdx = result.indexOf(SCRIPT);
    expect(scriptIdx).toBeLessThan(headIdx);
  });

  // ==========================================================================
  // I6: script 内容含特殊字符 </script> 不破坏结构
  // ==========================================================================
  it("script 含 </script> 字符串不破坏注入", () => {
    const trickyScript = '<script>var x = "</script>";</script>';
    const html = "<html><head></head><body></body></html>";
    const result = injectScript(html, trickyScript, "UNIQUE_MARKER");
    expect(result).toContain(trickyScript);
    // HTML 结构完整
    expect(result).toContain("<html>");
    expect(result).toContain("</html>");
  });

  // ==========================================================================
  // I7: 已含 marker → 幂等，不重复注入
  // ==========================================================================
  it("已含 marker 不重复注入（幂等）", () => {
    const html = `<html><head>${MARKER}</head><body></body></html>`;
    const result = injectScript(html, SCRIPT, MARKER);
    // 不应包含新脚本——原样返回
    expect(result).not.toContain(SCRIPT);
    expect(result).toBe(html);
  });

  // ==========================================================================
  // I8: 大小写不敏感匹配 </HEAD>
  // ==========================================================================
  it("大小写不敏感匹配 </HEAD>", () => {
    const html =
      "<HTML><HEAD><TITLE>X</TITLE></HEAD><BODY>Y</BODY></HTML>";
    const result = injectScript(html, SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    // 脚本在 </HEAD> 之前
    const headIdx = result.indexOf("</HEAD>");
    const scriptIdx = result.indexOf(SCRIPT);
    expect(scriptIdx).toBeLessThan(headIdx);
  });

  // ==========================================================================
  // 边界：仅空白字符
  // ==========================================================================
  it("仅空白字符视为空 HTML", () => {
    const result = injectScript("   \n  ", SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    expect(result).toContain("<html>");
  });

  // ==========================================================================
  // 边界：null/undefined → 同空字符串
  // ==========================================================================
  it("null/undefined 视为空（falsy 走空分支）", () => {
    // @ts-expect-error 测试类型外的运行时行为
    const result = injectScript(null, SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    expect(result).toContain("<html>");
  });

  // ==========================================================================
  // 边界：只有 <body> 无闭合标签
  // ==========================================================================
  it("只有 <body 开标签无闭合时脚本插入在 body 前", () => {
    const html = "<html><body>unclosed";
    const result = injectScript(html, SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    const scriptIdx = result.indexOf(SCRIPT);
    const bodyIdx = result.indexOf("<body");
    expect(scriptIdx).toBeLessThan(bodyIdx);
  });

  // ==========================================================================
  // 边界 (E1-E5)
  // ==========================================================================

  // E1: marker 在 HTML 注释中 → 仍跳过（幂等）
  it("marker 在 HTML 注释中 → 不重复注入（幂等）", () => {
    const html = `<!-- ${MARKER} --><html><head></head><body></body></html>`;
    const result = injectScript(html, SCRIPT, MARKER);
    // includes 在注释中也能匹配——这是预期行为，防止重复注入
    expect(result).not.toContain(SCRIPT);
    expect(result).toBe(html);
  });

  // E2: 重复 <head> 标签 → 脚本在第一个 </head> 前
  it("重复 </head> → 脚本插入在第一个 </head> 前", () => {
    const html = "<html><head></head><head></head><body></body></html>";
    const result = injectScript(html, SCRIPT, MARKER);
    expect(result).toContain(SCRIPT);
    const firstHeadClose = result.indexOf("</head>");
    const scriptIdx = result.indexOf(SCRIPT);
    expect(scriptIdx).toBeLessThan(firstHeadClose);
    // 第二个 </head> 仍在
    const secondHead = result.indexOf("</head>", firstHeadClose + 7);
    expect(secondHead).toBeGreaterThan(firstHeadClose);
  });

  // E3: 超大输入不抛异常
  it("超大输入（~500KB）不抛异常且返回含脚本", () => {
    const big = "<html><head></head><body>" + "x".repeat(500_000) + "</body></html>";
    const start = Date.now();
    const result = injectScript(big, SCRIPT, MARKER);
    const elapsed = Date.now() - start;
    expect(result).toContain(SCRIPT);
    expect(result).toContain("x".repeat(500_000));
    expect(elapsed).toBeLessThan(500);
  });

  // E4: script 参数为空字符串
  it("script 参数为空字符串 → 不破坏 HTML 结构", () => {
    const html = "<html><head></head><body><p>x</p></body></html>";
    const result = injectScript(html, "", MARKER);
    expect(result).toContain("<p>x</p>");
    // 不应含空的 <script></script>
    expect(result).not.toMatch(/<script>\s*<\/script>/);
  });

  // E5: marker 为空字符串 → 幂等跳过（"".includes("") === true）
  it("marker 为空字符串 → 幂等跳过生效，不重复注入", () => {
    const html = "<html><head></head><body></body></html>";
    // 空 marker: "".includes("") → true → 跳过注入
    const r1 = injectScript(html, SCRIPT, "");
    expect(r1).toBe(html);
    expect(r1).not.toContain(SCRIPT);
  });
});
