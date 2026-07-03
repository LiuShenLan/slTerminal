// path.test.ts — 路径工具函数单元测试
//
// 测试 basename, relativePath, isChildOf, normalizePath

import { describe, it, expect } from "vitest";
import { normalizePath, basename, isChildOf, relativePath } from "../lib/path";

describe("normalizePath", () => {
  it("将 Windows 反斜杠替换为正斜杠", () => {
    expect(normalizePath("D:\\project\\src\\index.ts")).toBe(
      "D:/project/src/index.ts",
    );
  });

  it("纯正斜杠路径不变", () => {
    expect(normalizePath("/home/user/file.txt")).toBe("/home/user/file.txt");
  });

  it("混合分隔符统一为正斜杠", () => {
    expect(normalizePath("C:\\Users\\test/file.txt")).toBe(
      "C:/Users/test/file.txt",
    );
  });

  it("空字符串不变", () => {
    expect(normalizePath("")).toBe("");
  });
});

describe("basename", () => {
  it("提取 Windows 路径中的文件名", () => {
    expect(basename("D:\\a\\b\\index.ts")).toBe("index.ts");
  });

  it("提取 Unix 路径中的文件名", () => {
    expect(basename("/home/user/readme.md")).toBe("readme.md");
  });

  it("仅文件名的输入原样返回", () => {
    expect(basename("hello.txt")).toBe("hello.txt");
  });

  it("根目录文件名提取", () => {
    expect(basename("C:\\test.rs")).toBe("test.rs");
  });

  it("混合斜杠也能正确提取", () => {
    expect(basename("D:/project/src\\main.py")).toBe("main.py");
  });

  it("空字符串返回空", () => {
    expect(basename("")).toBe("");
  });

  it("以斜杠结尾的路径返回空", () => {
    expect(basename("D:/project/")).toBe("");
  });
});

describe("isChildOf", () => {
  it("filePath 在 rootPath 子树中返回 true", () => {
    expect(isChildOf("D:/project/src/index.ts", "D:/project")).toBe(true);
  });

  it("filePath 不在 rootPath 子树中返回 false", () => {
    expect(isChildOf("D:/other/index.ts", "D:/project")).toBe(false);
  });

  it("filePath 等于 rootPath 返回 false（非子树）", () => {
    expect(isChildOf("D:/project", "D:/project")).toBe(false);
  });

  it("rootPath 带末尾斜杠也能正确判断", () => {
    expect(isChildOf("D:/project/src/index.ts", "D:/project/")).toBe(true);
  });

  it("Windows 反斜杠路径也能正确判断", () => {
    expect(isChildOf("D:\\project\\src\\index.ts", "D:\\project")).toBe(true);
  });

  it("不同盘符返回 false", () => {
    expect(isChildOf("C:/other/file.txt", "D:/project")).toBe(false);
  });

  it("同一前缀但不是子目录返回 false", () => {
    expect(isChildOf("D:/project-extra/file.txt", "D:/project")).toBe(false);
  });

  it("Unix 路径也能正确判断", () => {
    expect(isChildOf("/home/user/docs/report.pdf", "/home/user")).toBe(true);
  });
});

describe("relativePath", () => {
  it("计算子目录中的相对路径", () => {
    expect(relativePath("D:/project/src/index.ts", "D:/project")).toBe(
      "src/index.ts",
    );
  });

  it("深层嵌套的相对路径", () => {
    expect(
      relativePath("D:/project/a/b/c/file.txt", "D:/project"),
    ).toBe("a/b/c/file.txt");
  });

  it("不在子树中返回 null", () => {
    expect(relativePath("D:/other/file.ts", "D:/project")).toBeNull();
  });

  it("等于 rootPath 返回 null", () => {
    expect(relativePath("D:/project", "D:/project")).toBeNull();
  });

  it("rootPath 带末尾斜杠也能正确计算", () => {
    expect(
      relativePath("D:/project/src/index.ts", "D:/project/"),
    ).toBe("src/index.ts");
  });

  it("Windows 反斜杠路径也能正确计算", () => {
    expect(
      relativePath("D:\\project\\lib\\utils.ts", "D:\\project"),
    ).toBe("lib/utils.ts");
  });

  it("Unix 路径的相对路径", () => {
    expect(
      relativePath("/home/user/docs/report.pdf", "/home/user"),
    ).toBe("docs/report.pdf");
  });

  it("根目录的直接子文件", () => {
    expect(relativePath("D:/project/README.md", "D:/project")).toBe(
      "README.md",
    );
  });
});
