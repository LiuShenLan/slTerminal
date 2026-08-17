// FileIcon.test.tsx — FileIcon 组件测试（UI-602 同步：emoji 断言 → SVG/色系断言）
//
// 覆盖：
// A 组：文件渲染——svg 存在 + 左缘色块 fill = 扩展名映射色（六色盘）
// A2 组：扩展名 → 色块映射（表驱动全分支）
// B 组：git 状态着色——轮廓描边 stroke = git 状态色（GIT_FILE_COLORS 逻辑不变）
// C 组：目录渲染——IconFolder（svg）+ git 状态色（与文件分支同一映射，无状态/未命中回退默认色）

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

/** 取渲染出的 SVG 内第一个 path（文件轮廓）的 stroke 属性 */
function outlineStroke(container: HTMLElement): string | null {
  return container.querySelector("svg path")?.getAttribute("stroke") ?? null;
}

/** 取渲染出的 SVG 内左缘色块（rect）的 fill 属性 */
function blockFill(container: HTMLElement): string | null {
  return container.querySelector("svg rect")?.getAttribute("fill") ?? null;
}

// ─── A 组: 文件图标渲染（SVG + 色块）───

describe("FileIcon 组件渲染", () => {
  it("渲染文件图标（TS 扩展名，蓝色块）", () => {
    const { container } = render(<FileIcon name="main.ts" isDir={false} />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(blockFill(container)).toBe("#7fa8e8");
    expect(outlineStroke(container)).toBe(EXPLORER_COLORS.fg);
  });

  it("渲染文件图标（RS 扩展名，红色块）", () => {
    const { container } = render(<FileIcon name="lib.rs" isDir={false} />);
    expect(container.querySelector("svg")).not.toBeNull();
    expect(blockFill(container)).toBe("#d9706b");
  });

  it("渲染文件图标（JS 扩展名，黄色块）", () => {
    const { container } = render(<FileIcon name="index.js" isDir={false} />);
    expect(container.querySelector("svg")).not.toBeNull();
    expect(blockFill(container)).toBe("#d6b25e");
  });

  it("渲染文件图标（JSON 扩展名，黄色块）", () => {
    const { container } = render(<FileIcon name="package.json" isDir={false} />);
    expect(container.querySelector("svg")).not.toBeNull();
    expect(blockFill(container)).toBe("#d6b25e");
  });

  it("渲染文件图标（MD 扩展名，紫色块）", () => {
    const { container } = render(<FileIcon name="README.md" isDir={false} />);
    expect(container.querySelector("svg")).not.toBeNull();
    expect(blockFill(container)).toBe("#b48ce0");
  });

  it("渲染文件图标（TOML 配置扩展名，灰青块）", () => {
    const { container } = render(<FileIcon name="Cargo.toml" isDir={false} />);
    expect(container.querySelector("svg")).not.toBeNull();
    expect(blockFill(container)).toBe(EXPLORER_COLORS.fg);
  });

  it("渲染文件图标（无扩展名，默认：无彩块仅描边）", () => {
    const { container } = render(<FileIcon name="Makefile" isDir={false} />);
    expect(container.querySelector("svg")).not.toBeNull();
    expect(blockFill(container)).toBeNull();
    expect(outlineStroke(container)).toBe(EXPLORER_COLORS.fg);
  });

  it("渲染文件图标（未知扩展名，默认：无彩块仅描边）", () => {
    const { container } = render(<FileIcon name="data.bin" isDir={false} />);
    expect(container.querySelector("svg")).not.toBeNull();
    expect(blockFill(container)).toBeNull();
  });

  it("渲染目录图标（IconFolder 描边 SVG）", () => {
    const { container } = render(<FileIcon name="src" isDir={true} />);
    const span = container.querySelector("span");
    expect(span).not.toBeNull();
    expect(container.querySelector("svg")).not.toBeNull();
    expect(span!.style.color).toBe(hexToStyleRgb(EXPLORER_COLORS.fg));
  });

  it("目录 modified 状态应用修改色", () => {
    const { container } = render(
      <FileIcon name="src" isDir={true} gitStatus="modified" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(GIT_FILE_COLORS.modified));
  });

  it.each([
    ["added", GIT_FILE_COLORS.added], // 已新增
    ["untracked", GIT_FILE_COLORS.untracked], // 未跟踪
    ["deleted", GIT_FILE_COLORS.deleted], // 已删除
    ["renamed", GIT_FILE_COLORS.renamed], // 已重命名
    ["conflict", GIT_FILE_COLORS.conflict], // 冲突
    ["ignored", GIT_FILE_COLORS.ignored], // 已忽略
  ])("目录 %s 状态应用对应状态色", (status, expected) => {
    const { container } = render(
      <FileIcon name="src" isDir={true} gitStatus={status} />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(expected));
  });

  it("目录未知 gitStatus 回退默认前景色", () => {
    const { container } = render(
      <FileIcon name="src" isDir={true} gitStatus="unknown" />,
    );
    const span = container.querySelector("span");
    expect(span!.style.color).toBe(hexToStyleRgb(EXPLORER_COLORS.fg));
  });
});

// ─── A2 组: 扩展名 → 色系映射表驱动（全部分支，防止映射漂移）───

describe("FileIcon 扩展名 → 色系映射（表驱动）", () => {
  it.each([
    ["app.tsx", "#7fa8e8"], // TS 蓝
    ["app.jsx", "#d6b25e"], // JS 黄
    ["app.mjs", "#d6b25e"],
    ["app.cjs", "#d6b25e"],
    ["main.py", "#93b573"], // Python 绿
    ["main.pyw", "#93b573"],
    ["tsconfig.jsonc", "#d6b25e"], // JSON 黄（同 JS）
    ["README.markdown", "#b48ce0"], // Markdown 紫
    ["index.htm", "#6fbfc4"], // HTML 青
    ["style.css", "#7fa8e8"], // CSS 蓝（同 TS）
    ["style.scss", "#7fa8e8"],
    ["style.less", "#7fa8e8"],
    ["config.xml", EXPLORER_COLORS.fg], // 配置 → 灰青（主题灰）
    ["icon.svg", EXPLORER_COLORS.fg],
    ["config.yaml", EXPLORER_COLORS.fg],
    ["config.yml", EXPLORER_COLORS.fg],
    [".gitignore", EXPLORER_COLORS.fg],
    [".gitattributes", EXPLORER_COLORS.fg],
  ])("扩展名分支 %s → 色块 fill 为对应色系", (name, expected) => {
    const { container } = render(<FileIcon name={name} isDir={false} />);
    expect(blockFill(container)).toBe(expected);
  });
});

// ─── B 组: git 状态着色（描边 stroke，GIT_FILE_COLORS 逻辑不变）───

describe("FileIcon git 状态着色", () => {
  it("modified 状态应用修改色", () => {
    const { container } = render(
      <FileIcon name="main.ts" isDir={false} gitStatus="modified" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.modified);
  });

  it("added 状态应用新增色", () => {
    const { container } = render(
      <FileIcon name="lib.rs" isDir={false} gitStatus="added" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.added);
  });

  it("untracked 状态应用未跟踪色", () => {
    const { container } = render(
      <FileIcon name="new.ts" isDir={false} gitStatus="untracked" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.untracked);
  });

  it("deleted 状态应用删除色", () => {
    const { container } = render(
      <FileIcon name="old.ts" isDir={false} gitStatus="deleted" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.deleted);
  });

  it("renamed 状态应用重命名色", () => {
    const { container } = render(
      <FileIcon name="moved.ts" isDir={false} gitStatus="renamed" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.renamed);
  });

  it("conflict 状态应用冲突色", () => {
    const { container } = render(
      <FileIcon name="conflict.ts" isDir={false} gitStatus="conflict" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.conflict);
  });

  it("ignored 状态应用忽略色", () => {
    const { container } = render(
      <FileIcon name="ignored.log" isDir={false} gitStatus="ignored" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.ignored);
  });

  it("无 gitStatus 时使用默认前景色", () => {
    const { container } = render(<FileIcon name="normal.ts" isDir={false} />);
    expect(outlineStroke(container)).toBe(EXPLORER_COLORS.fg);
  });

  it("git 状态与类型色块叠加：描边用 git 色、色块保持类型色", () => {
    const { container } = render(
      <FileIcon name="main.ts" isDir={false} gitStatus="modified" />,
    );
    expect(outlineStroke(container)).toBe(GIT_FILE_COLORS.modified);
    expect(blockFill(container)).toBe("#7fa8e8");
  });
});
