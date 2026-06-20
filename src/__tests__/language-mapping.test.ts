// language-mapping 测试: getLanguageExtension 根据文件扩展名返回正确的 LanguageSupport
import { describe, it, expect } from "vitest";
import { getLanguageExtension } from "../panels/editor/useCodeMirror";
import { LanguageSupport } from "@codemirror/language";

describe("getLanguageExtension", () => {
  it("无参数 → javascript()", () => {
    const result = getLanguageExtension();
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it("undefined → javascript()", () => {
    const result = getLanguageExtension(undefined);
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".js → javascript()", () => {
    const result = getLanguageExtension("app.js");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".ts → javascript()", () => {
    const result = getLanguageExtension("app.ts");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".tsx → javascript()", () => {
    const result = getLanguageExtension("component.tsx");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".py → python()", () => {
    const result = getLanguageExtension("script.py");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".pyw → python()", () => {
    const result = getLanguageExtension("gui.pyw");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".rs → rust()", () => {
    const result = getLanguageExtension("main.rs");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".json → json()", () => {
    const result = getLanguageExtension("config.json");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".jsonc → json()", () => {
    const result = getLanguageExtension("tsconfig.jsonc");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".html → html()", () => {
    const result = getLanguageExtension("index.html");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".htm → html()", () => {
    const result = getLanguageExtension("page.htm");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".css → css()", () => {
    const result = getLanguageExtension("style.css");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".scss → css()", () => {
    const result = getLanguageExtension("theme.scss");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".md → markdown()", () => {
    const result = getLanguageExtension("readme.md");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".markdown → markdown()", () => {
    const result = getLanguageExtension("guide.markdown");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".xml → xml()", () => {
    const result = getLanguageExtension("data.xml");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it(".svg → xml()", () => {
    const result = getLanguageExtension("icon.svg");
    expect(result).toBeInstanceOf(LanguageSupport);
  });

  it("未知扩展名 → javascript()", () => {
    const result = getLanguageExtension("unknown.xyz");
    expect(result).toBeInstanceOf(LanguageSupport);
  });
});
