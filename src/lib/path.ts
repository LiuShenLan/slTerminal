// path.ts — 路径工具函数
//
// 纯函数，无副作用。统一使用正斜杠 "/" 作为分隔符。
// Windows 路径在比较前规范化（反斜杠 → 正斜杠）。

/** 规范化路径分隔符：反斜杠 → 正斜杠 */
export function normalizePath(filePath: string): string {
  return filePath.replace(/\\/g, "/");
}

/** 提取文件名，如 "D:/a/b/index.ts" → "index.ts" */
export function basename(filePath: string): string {
  const normalized = normalizePath(filePath);
  const lastSlash = normalized.lastIndexOf("/");
  return lastSlash >= 0 ? normalized.slice(lastSlash + 1) : normalized;
}

/**
 * 判断 filePath 是否在 rootPath 子树中。
 * 规范化后比较前缀，要求 rootPath 以 "/" 结尾或以相同的路径分隔结束。
 */
export function isChildOf(filePath: string, rootPath: string): boolean {
  const normalizedFile = normalizePath(filePath);
  const normalizedRoot = normalizePath(rootPath);

  // root 是根目录或以 "/" 结尾时直接比较前缀
  const rootPrefix = normalizedRoot.endsWith("/")
    ? normalizedRoot
    : `${normalizedRoot}/`;

  return (
    normalizedFile.startsWith(rootPrefix) &&
    normalizedFile.length > rootPrefix.length
  );
}

/**
 * 计算相对路径（相对于 rootPath）。
 * 不在子树中返回 null。
 * 返回的路径使用正斜杠分隔。
 */
export function relativePath(
  filePath: string,
  rootPath: string,
): string | null {
  const normalizedFile = normalizePath(filePath);
  const normalizedRoot = normalizePath(rootPath);

  const rootPrefix = normalizedRoot.endsWith("/")
    ? normalizedRoot
    : `${normalizedRoot}/`;

  if (
    !normalizedFile.startsWith(rootPrefix) ||
    normalizedFile.length <= rootPrefix.length
  ) {
    return null;
  }

  return normalizedFile.slice(rootPrefix.length);
}
