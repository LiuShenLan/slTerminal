// matcherEngine.ts — matcher 语义引擎（P3-FE-08）
//
// 严格按 contract.md C13-5 matcher 语义表实现，纯函数零副作用，
// 供 MatcherTester 实时试测与保存前校验共用。
//
// 版本前提（写入注释供维护参考）：
//   - 逗号/空格作为分隔符需 claude v2.1.191+
//   - 连字符参与匹配值需 claude v2.1.195+
//   - FileChanged / StopFailure 的窄字符集限制与官方 matcher 演进相关，
//     低版本 claude 遇到受限事件中的连字符/空格/逗号 matcher 行为不可预期

import { hasRestrictedMatcherCharset } from "./eventsCatalog";

export interface MatchResult {
  /** 是否匹配 */
  matched: boolean;
  /** 命中的匹配模式 */
  mode: "exact-or" | "regex" | "all";
}

// 默认窄字符集：字母/数字/_/-/空格/|/,
// 仅含这些字符 → 精确匹配 OR（大小写敏感）
const DEFAULT_NARROW_CHARSET = /^[A-Za-z0-9_\- ,|]*$/;

// FileChanged/StopFailure 受限窄字符集：仅 字母/数字/_/|
// 连字符/空格/逗号出现即强制走 JS 正则
const RESTRICTED_NARROW_CHARSET = /^[A-Za-z0-9_|]*$/;

// OR 分隔符：|（官方）、, 与 空格（v2.1.191+）
const OR_SEPARATOR = /[|, ]+/;

/**
 * 判断 matcher 与目标值是否匹配（C13-5）：
 * - matcher 为 "*" / "" / 省略 → 全匹配（mode: "all"）
 * - matcher 仅含窄字符集 → 精确匹配 OR（mode: "exact-or"，大小写敏感）
 * - 含其他字符 → JS 正则非锚定（mode: "regex"，大小写敏感）
 *
 * @param matcher matcher 表达式（可为空串/省略，表示全匹配）
 * @param toolName 匹配目标值（工具名 / notification_type / source 等）
 * @param event 事件名——FileChanged/StopFailure 使用受限窄字符集，
 *              连字符/空格/逗号强制走正则
 */
export function matchHook(
  matcher: string | undefined,
  toolName: string,
  event?: string,
): MatchResult {
  // "*" / "" / 省略 → 全匹配
  if (matcher === undefined || matcher === "" || matcher === "*") {
    return { matched: true, mode: "all" };
  }

  const charset = hasRestrictedMatcherCharset(event)
    ? RESTRICTED_NARROW_CHARSET
    : DEFAULT_NARROW_CHARSET;

  // 窄字符集 → 精确匹配 OR（大小写敏感）
  if (charset.test(matcher)) {
    const parts = matcher.split(OR_SEPARATOR).filter(Boolean);
    return { matched: parts.includes(toolName), mode: "exact-or" };
  }

  // 其他字符 → JS 正则非锚定（大小写敏感）
  try {
    return { matched: new RegExp(matcher).test(toolName), mode: "regex" };
  } catch {
    // 非法正则防御：视为不匹配，不抛错
    return { matched: false, mode: "regex" };
  }
}
