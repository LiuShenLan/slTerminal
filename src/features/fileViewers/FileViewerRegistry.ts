// FileViewerRegistry — 策略模式文件查看器注册表
//
// 根据文件路径决定用哪个面板类型打开。组合多个 FileViewerStrategy，
// 按优先级链式调用，首个非 null 结果即为选中面板类型。
// 所有策略返回 null 时回退到默认编辑器。
//
// 扩展新文件类型只需：实现面板组件 + register() 添加扩展名映射。

/**
 * 文件查看器策略接口。
 * 返回面板类型标识字符串，或 null 表示不处理此文件。
 */
export interface FileViewerStrategy {
  /** 解析文件路径 → 面板类型，null 表示不处理 */
  resolve(filePath: string): string | null;
}

/**
 * 基于文件扩展名的查看器策略。
 * 维护 Map<extension, panelType>，支持动态注册。
 * 扩展名大小写不敏感，取路径最后一个点号之后的段。
 */
export class ExtensionBasedViewerStrategy implements FileViewerStrategy {
  private mapping = new Map<string, string>();

  /** 注册扩展名映射。extension 不含点号（如 "html"），大小写不敏感。 */
  register(extension: string, panelType: string): void {
    const key = extension.toLowerCase();
    if (key === "") return; // 忽略空扩展名
    this.mapping.set(key, panelType);
  }

  resolve(filePath: string): string | null {
    // 取最后一个点号之后的扩展名
    const lastDot = filePath.lastIndexOf(".");
    if (lastDot === -1) return null;
    // 最后一个路径分隔符之后才是文件名——如果点号紧邻分隔符（或位于首字符），
    // 说明是隐藏文件（如 .gitignore），不匹配扩展名
    const afterLastSep = Math.max(
      filePath.lastIndexOf("/"),
      filePath.lastIndexOf("\\"),
    );
    if (lastDot <= afterLastSep + 1) return null;
    const ext = filePath.slice(lastDot + 1).toLowerCase();
    return this.mapping.get(ext) ?? null;
  }

  /** 仅测试用：清空注册表 */
  _reset(): void {
    this.mapping.clear();
  }
}

/**
 * 文件查看器注册表——组合多个策略，链式解析。
 * 全局单例 fileViewerRegistry 预注册 HTML 映射。
 */
export class FileViewerRegistry {
  private strategies: FileViewerStrategy[] = [];

  /** 添加策略（先添加的优先匹配） */
  addStrategy(strategy: FileViewerStrategy): void {
    this.strategies.push(strategy);
  }

  /**
   * 解析文件路径 → 面板类型。
   * 遍历所有策略，返回第一个非 null 结果。
   * 所有策略都返回 null 时返回 null（调用方回退默认编辑器）。
   */
  resolve(filePath: string): string | null {
    for (const strategy of this.strategies) {
      const result = strategy.resolve(filePath);
      if (result !== null) return result;
    }
    return null;
  }

  /** 仅测试用：清空所有策略 */
  _reset(): void {
    this.strategies = [];
  }
}

// ---- 初始化全局单例 ----

const extensionStrategy = new ExtensionBasedViewerStrategy();
extensionStrategy.register("html", "htmlviewer");
extensionStrategy.register("htm", "htmlviewer");
// 后续扩展示例:
// extensionStrategy.register("md", "markdownviewer");
// extensionStrategy.register("pdf", "pdfviewer");
// extensionStrategy.register("png", "imageviewer");

/** 全局文件查看器注册表单例 */
export const fileViewerRegistry = new FileViewerRegistry();
fileViewerRegistry.addStrategy(extensionStrategy);
