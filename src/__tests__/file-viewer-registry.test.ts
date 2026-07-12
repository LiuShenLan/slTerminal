// FileViewerRegistry.test.ts — 文件查看器注册表单元测试
//
// 覆盖路径：
//   1. ExtensionBasedViewerStrategy — 注册/解析/大小写/边界/多次注册覆盖
//   2. FileViewerRegistry — 空策略/单策略/多策略/短路/优先级/顺序语义
//   3. 单例初始化 — html + htm → htmlviewer

import { describe, it, expect, afterEach, beforeEach } from "vitest";
import {
  FileViewerRegistry,
  ExtensionBasedViewerStrategy,
  fileViewerRegistry,
} from "../features/fileViewers";

// 每个测试使用独立实例，避免单例状态污染
let strategy: ExtensionBasedViewerStrategy;
let registry: FileViewerRegistry;

beforeEach(() => {
  strategy = new ExtensionBasedViewerStrategy();
  registry = new FileViewerRegistry();
});

// ============================================================================
// ExtensionBasedViewerStrategy
// ============================================================================

describe("ExtensionBasedViewerStrategy", () => {
  // 1. 注册扩展名后 resolve 命中
  it("注册扩展名后 resolve 命中", () => {
    strategy.register("html", "hv");
    expect(strategy.resolve("a.html")).toBe("hv");
  });

  // 2. 未注册扩展名返回 null
  it("未注册扩展名返回 null", () => {
    expect(strategy.resolve("a.txt")).toBeNull();
  });

  // 3. 扩展名大小写不敏感
  it("扩展名大小写不敏感（注册大写）", () => {
    strategy.register("HTML", "hv");
    expect(strategy.resolve("a.HTML")).toBe("hv");
    expect(strategy.resolve("a.html")).toBe("hv");
  });

  it("扩展名大小写不敏感（注册小写）", () => {
    strategy.register("html", "hv");
    expect(strategy.resolve("a.HTML")).toBe("hv");
  });

  // 4. 路径含多个点取最后一个扩展名
  it("路径含多个点取最后一个扩展名", () => {
    strategy.register("html", "hv");
    expect(strategy.resolve("a.b.c.min.html")).toBe("hv");
  });

  // 5. 无扩展名文件返回 null
  it("无扩展名文件返回 null（Makefile）", () => {
    expect(strategy.resolve("Makefile")).toBeNull();
  });

  // 6. 隐藏文件（.gitignore）返回 null
  it("隐藏文件 .gitignore 返回 null", () => {
    strategy.register("gitignore", "hv");
    expect(strategy.resolve(".gitignore")).toBeNull();
  });

  // 7. 空字符串返回 null
  it("空字符串返回 null", () => {
    expect(strategy.resolve("")).toBeNull();
  });

  // 8. 注册空扩展名被忽略
  it("注册空扩展名不生效", () => {
    strategy.register("", "hv");
    expect(strategy.resolve("Makefile")).toBeNull();
  });

  // 9. 多次注册同一扩展名覆盖旧值
  it("多次注册同一扩展名覆盖旧值", () => {
    strategy.register("md", "a");
    strategy.register("md", "b");
    expect(strategy.resolve("x.md")).toBe("b");
  });

  // 10. 路径含空格正确解析
  it("路径含空格正确解析", () => {
    strategy.register("html", "hv");
    expect(strategy.resolve("my doc.html")).toBe("hv");
  });

  // 11. 多个扩展名独立映射不互相干扰
  it("多个扩展名独立映射不互相干扰", () => {
    strategy.register("html", "hv");
    expect(strategy.resolve("x.md")).toBeNull();
  });

  // 12. Windows 绝对路径正确解析
  it("Windows 绝对路径正确解析", () => {
    strategy.register("html", "hv");
    expect(strategy.resolve("C:\\Users\\a\\index.html")).toBe("hv");
  });

  // 13. UNC 路径正确解析
  it("UNC 路径正确解析", () => {
    strategy.register("html", "hv");
    expect(strategy.resolve("\\\\server\\share\\page.html")).toBe("hv");
  });

  // 14. 扩展名含特殊字符（.tar.gz → 只取最后一段 gz）
  it("压缩包扩展名 .tar.gz 只取最后一段", () => {
    strategy.register("gz", "archive");
    expect(strategy.resolve("a.tar.gz")).toBe("archive");
  });
});

// ============================================================================
// FileViewerRegistry 组合策略
// ============================================================================

describe("FileViewerRegistry", () => {
  // 15. 空策略列表返回 null
  it("空策略列表返回 null", () => {
    expect(registry.resolve("a.html")).toBeNull();
  });

  // 16. 单策略命中
  it("单策略命中", () => {
    const s = new ExtensionBasedViewerStrategy();
    s.register("html", "htmlviewer");
    registry.addStrategy(s);
    expect(registry.resolve("a.html")).toBe("htmlviewer");
  });

  // 17. 多策略按优先级链式调用
  it("多策略链式调用——第一个返回 null 继续第二个", () => {
    const s1 = new ExtensionBasedViewerStrategy();
    s1.register("html", "hv1");
    const s2 = new ExtensionBasedViewerStrategy();
    s2.register("html", "hv2");
    registry.addStrategy(s1);
    registry.addStrategy(s2);
    // s1 命中，不继续 s2
    expect(registry.resolve("a.html")).toBe("hv1");
  });

  // 18. 首个策略命中后短路，不调后续策略
  it("首个策略命中后短路", () => {
    const s1 = new ExtensionBasedViewerStrategy();
    s1.register("html", "hv1");
    const s2 = new ExtensionBasedViewerStrategy();
    s2.register("html", "hv2");
    // spy s2.resolve
    const originalResolve = s2.resolve.bind(s2);
    let s2Called = false;
    s2.resolve = (fp: string) => {
      s2Called = true;
      return originalResolve(fp);
    };
    registry.addStrategy(s1);
    registry.addStrategy(s2);
    registry.resolve("a.html");
    expect(s2Called).toBe(false);
  });

  // 19. 所有策略返回 null 时返回 null
  it("所有策略返回 null 时返回 null", () => {
    const s1 = new ExtensionBasedViewerStrategy();
    const s2 = new ExtensionBasedViewerStrategy();
    registry.addStrategy(s1);
    registry.addStrategy(s2);
    expect(registry.resolve("a.xyz")).toBeNull();
  });

  // 20. addStrategy 顺序即优先级
  it("addStrategy 顺序即优先级", () => {
    const s1 = new ExtensionBasedViewerStrategy();
    s1.register("html", "first");
    const s2 = new ExtensionBasedViewerStrategy();
    s2.register("html", "second");
    registry.addStrategy(s1);
    registry.addStrategy(s2);
    expect(registry.resolve("a.html")).toBe("first");

    // 反转顺序验证
    const r2 = new FileViewerRegistry();
    r2.addStrategy(s2);
    r2.addStrategy(s1);
    expect(r2.resolve("a.html")).toBe("second");
  });
});

// ============================================================================
// 单例初始化
// ============================================================================

describe("fileViewerRegistry 单例", () => {
  afterEach(() => {
    // 恢复单例状态
    fileViewerRegistry._reset();
    const es = new ExtensionBasedViewerStrategy();
    es.register("html", "htmlviewer");
    es.register("htm", "htmlviewer");
    fileViewerRegistry.addStrategy(es);
  });

  // 21. 导出为 FileViewerRegistry 实例
  it("导出为 FileViewerRegistry 实例", () => {
    expect(fileViewerRegistry).toBeInstanceOf(FileViewerRegistry);
  });

  // 22. 默认注册 html → htmlviewer
  it("默认注册 .html → htmlviewer", () => {
    expect(fileViewerRegistry.resolve("a.html")).toBe("htmlviewer");
  });

  // 23. 默认注册 htm → htmlviewer
  it("默认注册 .htm → htmlviewer", () => {
    expect(fileViewerRegistry.resolve("a.htm")).toBe("htmlviewer");
  });

  // 补充：单例对未知扩展名返回 null
  it("单例对未知扩展名返回 null", () => {
    expect(fileViewerRegistry.resolve("a.ts")).toBeNull();
  });
});
