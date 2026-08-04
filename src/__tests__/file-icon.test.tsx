// FileIcon.test.tsx — fileIcon() 纯函数 + FileIcon 组件测试
//
// 覆盖：
// A 组：fileIcon() — 通过组件渲染间接验证各扩展名图标
// B 组：FileIcon 组件 — 文件/目录渲染 + git 状态着色

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { FileIcon } from "../features/explorer/FileIcon";
import { GIT_FILE_COLORS, EXPLORER_COLORS } from "../theme";

/** jsdom 将 style.color 规范化为 rgb() 格式，此函数将 hex 转为同格式用于比较 */
function hexToStyleRgb(hex: string): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgb(${r}, ${g}, ${b})`;
}

// ─── A 组: 文件图标渲染（各扩展名分支）───

describe("FileIcon 组件渲染", () => {
  it("渲染文件图标（TS 扩展名）", () => {
    const { container } = render(
      <FileIcon name="main.ts" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span).not.toBeNull();
    expect(span!.textContent?.length).toBeGreaterThan(0);
    expect(span!.style.color).toBe(hexToStyleRgb(EXPLORER_COLORS.fg));
  });

  it("渲染文件图标（RS 扩展名）", () => {
    const { container } = render(
      <FileIcon name="lib.rs" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent?.length).toBeGreaterThan(0);
  });

  it("渲染文件图标（JS 扩展名）", () => {
    const { container } = render(
      <FileIcon name="index.js" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent?.length).toBeGreaterThan(0);
  });

  it("渲染文件图标（JSON 扩展名）", () => {
    const { container } = render(
      <FileIcon name="package.json" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent?.length).toBeGreaterThan(0);
  });

  it("渲染文件图标（MD 扩展名）", () => {
    const { container } = render(
      <FileIcon name="README.md" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent?.length).toBeGreaterThan(0);
  });

  it("渲染文件图标（TOML 扩展名）", () => {
    const { container } = render(
      <FileIcon name="Cargo.toml" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent?.length).toBeGreaterThan(0);
  });

  it("渲染文件图标（无扩展名，默认图标）", () => {
    const { container } = render(
      <FileIcon name="Makefile" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent?.length).toBeGreaterThan(0);
    expect(span!.style.color).toBe(hexToStyleRgb(EXPLORER_COLORS.fg));
  });

  it("渲染文件图标（未知扩展名，默认图标）", () => {
    const { container } = render(
      <FileIcon name="data.bin" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent?.length).toBeGreaterThan(0);
  });

  it("渲染目录图标", () => {
    const { container } = render(
      <FileIcon name="src" isDir={true} />,
    );
    const span = container.querySelector("span");
    expect(span).not.toBeNull();
    expect(span!.textContent).toBe("\u{1F4C1}");
    expect(span!.style.color).toBe(hexToStyleRgb(EXPLORER_COLORS.fg));
  });

  it("目录即使有 gitStatus 也使用默认颜色", () => {
    const { container } = render(
      <FileIcon name="src" isDir={true} gitStatus="modified" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(EXPLORER_COLORS.fg));
  });
});

// ─── A2 组: 扩展名 → emoji 映射表驱动（EXP-05 分支补全）───

describe("FileIcon 扩展名 → emoji 映射（表驱动）", () => {
  // 现有单测仅覆盖 .ts/.rs/.js/.json/.md/.toml 且只断言文本非空；
  // 此处表驱动补齐全部未覆盖分支（.pyw/.markdown/.less/.scss/.gitattributes 等），
  // 断言精确图标，防止映射漂移。
  it.each([
    ["app.tsx", "\u{1F596}"], // TS 蓝
    ["app.jsx", "\u{1F4DC}"], // JS 黄
    ["app.mjs", "\u{1F4DC}"],
    ["app.cjs", "\u{1F4DC}"],
    ["main.py", "\u{1F40D}"], // Python
    ["main.pyw", "\u{1F40D}"],
    ["tsconfig.jsonc", "\u{1F4CB}"], // JSON
    ["README.markdown", "\u{1F4DD}"], // Markdown
    ["index.htm", "\u{1F310}"], // HTML
    ["style.css", "\u{1F3A8}"], // CSS
    ["style.scss", "\u{1F3A8}"],
    ["style.less", "\u{1F3A8}"],
    ["config.xml", "\u{1F4C4}"], // XML
    ["icon.svg", "\u{1F4C4}"],
    ["config.yaml", "\u{2699}\u{FE0F}"], // 配置
    ["config.yml", "\u{2699}\u{FE0F}"],
    [".gitignore", "\u{1F4E6}"], // Git
    [".gitattributes", "\u{1F4E6}"],
  ])("扩展名分支 %s → 返回对应图标", (name, expected) => {
    const { container } = render(<FileIcon name={name} isDir={false} />);
    const span = container.querySelector("span");
    expect(span).not.toBeNull();
    expect(span!.textContent).toBe(expected);
  });
});

// ─── B 组: git 状态着色 ───

describe("FileIcon git 状态着色", () => {
  it("modified 状态应用修改色", () => {
    const { container } = render(
      <FileIcon name="main.ts" isDir={false} gitStatus="modified" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.modified));
  });

  it("added 状态应用新增色", () => {
    const { container } = render(
      <FileIcon name="lib.rs" isDir={false} gitStatus="added" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.added));
  });

  it("untracked 状态应用未跟踪色", () => {
    const { container } = render(
      <FileIcon name="new.ts" isDir={false} gitStatus="untracked" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.untracked));
  });

  it("deleted 状态应用删除色", () => {
    const { container } = render(
      <FileIcon name="old.ts" isDir={false} gitStatus="deleted" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.deleted));
  });

  it("renamed 状态应用重命名色", () => {
    const { container } = render(
      <FileIcon name="moved.ts" isDir={false} gitStatus="renamed" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.renamed));
  });

  it("conflict 状态应用冲突色", () => {
    const { container } = render(
      <FileIcon name="conflict.ts" isDir={false} gitStatus="conflict" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.conflict));
  });

  it("ignored 状态应用忽略色", () => {
    const { container } = render(
      <FileIcon name="ignored.log" isDir={false} gitStatus="ignored" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.ignored));
  });

  it("无 gitStatus 时使用默认前景色", () => {
    const { container } = render(
      <FileIcon name="normal.ts" isDir={false} />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(EXPLORER_COLORS.fg));
  });
});
