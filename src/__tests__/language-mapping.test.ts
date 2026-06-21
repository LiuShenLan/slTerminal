// language-mapping 测试：getLanguageExtension 根据文件扩展名返回正确的 LanguageSupport
// 断言 language.name 精确验证映射结果（而非仅 instanceof LanguageSupport）
import { describe, it, expect } from "vitest";
import { getLanguageExtension } from "../panels/editor/useCodeMirror";
import { LanguageSupport } from "@codemirror/language";

/** 快捷断言：getLanguageExtension 返回指定语言名 */
function expectLanguage(filename: string | undefined, langName: string) {
  const result = getLanguageExtension(filename);
  expect(result).toBeInstanceOf(LanguageSupport);
  // LanguageSupport.language.name 是运行时可靠属性（如 "python"/"javascript"）
  expect((result as Record<string, unknown>).language).toHaveProperty("name", langName);
}

describe("getLanguageExtension", () => {
  it("无参数 → javascript", () => expectLanguage(undefined, "javascript"));
  it("undefined → javascript", () => expectLanguage(undefined, "javascript"));
  it(".js → javascript", () => expectLanguage("app.js", "javascript"));
  it(".ts → javascript", () => expectLanguage("app.ts", "javascript"));
  it(".tsx → javascript", () => expectLanguage("component.tsx", "javascript"));
  it(".jsx → javascript", () => expectLanguage("component.jsx", "javascript"));
  it(".mjs → javascript", () => expectLanguage("module.mjs", "javascript"));
  it(".cjs → javascript", () => expectLanguage("module.cjs", "javascript"));
  it(".py → python", () => expectLanguage("script.py", "python"));
  it(".pyw → python", () => expectLanguage("gui.pyw", "python"));
  it(".rs → rust", () => expectLanguage("main.rs", "rust"));
  it(".json → json", () => expectLanguage("config.json", "json"));
  it(".jsonc → json", () => expectLanguage("tsconfig.jsonc", "json"));
  it(".html → html", () => expectLanguage("index.html", "html"));
  it(".htm → html", () => expectLanguage("page.htm", "html"));
  it(".css → css", () => expectLanguage("style.css", "css"));
  it(".scss → css", () => expectLanguage("theme.scss", "css"));
  it(".less → css", () => expectLanguage("theme.less", "css"));
  it(".md → markdown", () => expectLanguage("readme.md", "markdown"));
  it(".markdown → markdown", () => expectLanguage("guide.markdown", "markdown"));
  it(".xml → xml", () => expectLanguage("data.xml", "xml"));
  it(".svg → xml", () => expectLanguage("icon.svg", "xml"));
  it("未知扩展名 → javascript", () => expectLanguage("unknown.xyz", "javascript"));
});
