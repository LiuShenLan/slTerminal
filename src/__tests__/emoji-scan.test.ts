// emoji-scan.test.ts — IC-09 装饰 emoji 字面量守卫（UI-601 验收，L2 grep 守卫形态）
//
// 背景：UI 全面重设计（ADR-0003 linear 极黑克制）后，装饰 emoji 字符禁止作为
// UI 字面量出现——图标统一经 src/lib/icons.tsx（lucide 集中封装，单点）与
// src/lib/StatusDot.tsx 渲染，通知标题纯文本（IC-07），树箭头/杂项标记全部
// chevron/关闭图标化（IC-05/08）。
//
// 扫描口径：
// - 扫描范围：src/ 全部 .ts/.tsx（递归 fs 枚举，新增文件自动纳入）；
//   src/__tests__/ 整目录排除（测试自身可用字符做断言夹具，照 AC-5 守卫先例）。
// - 断言形态：原始文本逐字符比对（含注释与字符串字面量）——与 verify
//   stage-03 的 grep 口径一致（IC-05 grep ▶▼⏳、IC-08 grep ✗ 均含注释）。
// - 白名单机制：仅允许逐文件显式登记（文件相对路径 → 允许的字符集合），
//   初始为空；命中即 fail 并报告文件与字符。白名单仅允许终端输出语义字符
//   （如状态行桥接脚本向终端输出的字符），UI 装饰字符一律禁止。
// - 自检防线：白名单注册的文件必须存在且被扫描到（拼写错会静默空扫，
//   由自检用例守住，照 no-claude-literals CS-2 模式）。

import { describe, it, expect, beforeAll } from "vitest";
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { resolve, join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../..");

/** 扫描根：src/ 全树 */
const SCAN_ROOT_REL = "src";

/** 排除目录：src/__tests__（相对 repoRoot，正斜杠归一前缀匹配——files 为 repoRoot 相对路径） */
const EXCLUDED_DIR_REL = "src/__tests__";

/** 禁止的装饰 emoji/符号字符集合（IC-09 定稿清单） */
const BANNED_CHARS = "📁📂📋🤖🌿⭐🟠⚡✅❌🕐💾📄✏️🗑➕🔍⚙️🔄🖖📜🐍📝🌐🎨📦⏳🔐✗▶▼";

/**
 * 白名单：文件相对路径（repoRoot，正斜杠归一）→ 该文件允许出现的禁止字符。
 * 初始为空——仅允许终端输出语义字符（如注入脚本向终端输出的字符），
 * 且必须逐文件显式登记。新增条目须同步补自检用例。
 */
const WHITELIST: Record<string, string[]> = {};

interface Violation {
  file: string;
  char: string;
}

/** 递归枚举目录下全部 .ts/.tsx 文件 */
function collectTsFiles(dirAbs: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dirAbs, { withFileTypes: true })) {
    const full = join(dirAbs, entry.name);
    if (entry.isDirectory()) out.push(...collectTsFiles(full));
    else if (entry.isFile() && /\.tsx?$/.test(entry.name)) out.push(full);
  }
  return out;
}

/** 是否命中排除目录（相对 repoRoot，正斜杠归一前缀匹配——EXCLUDED_DIR_REL 为 repoRoot 相对路径 src/__tests__） */
function isExcludedRel(fileRel: string): boolean {
  return fileRel.replace(/\\/g, "/").startsWith(`${EXCLUDED_DIR_REL}/`);
}

/** 全量扫描：收集全部违规（src/ 全树 × .ts/.tsx，排除 __tests__） */
function scanViolations(): { violations: Violation[]; files: string[] } {
  const violations: Violation[] = [];
  const scanAbs = resolve(repoRoot, SCAN_ROOT_REL);
  const files = collectTsFiles(scanAbs)
    .map((f) => relative(repoRoot, f))
    .filter((f) => !isExcludedRel(f));

  for (const file of files) {
    const src = readFileSync(resolve(repoRoot, file), "utf8");
    const allowed = WHITELIST[file.replace(/\\/g, "/")] ?? [];
    for (const ch of src) {
      if (!BANNED_CHARS.includes(ch)) continue;
      if (allowed.includes(ch)) continue;
      violations.push({ file, char: ch });
    }
  }
  return { violations, files };
}

/** 格式化违规列表为可读断言消息 */
function formatViolations(vs: Violation[]): string {
  return vs.map((v) => `${v.file}: 字符 "${v.char}"`).join("\n");
}

describe("IC-09 装饰 emoji 字面量守卫（src/ 全树无装饰 emoji；白名单逐文件显式登记）", () => {
  let violations: Violation[];
  let files: string[];

  beforeAll(() => {
    ({ violations, files } = scanViolations());
  });

  it("扫描范围完整性：src 存在且枚举到 .ts(x) 文件，__tests__ 整目录排除（防路径拼写错致静默空扫）", () => {
    const scanAbs = resolve(repoRoot, SCAN_ROOT_REL);
    expect(existsSync(scanAbs), `扫描根不存在：${SCAN_ROOT_REL}`).toBe(true);
    // 排除目录确实存在且其中确实有 .ts/.tsx 被枚举到（否则排除形同虚设）
    const excludedAbs = resolve(repoRoot, EXCLUDED_DIR_REL);
    expect(existsSync(excludedAbs), `排除目录不存在：${EXCLUDED_DIR_REL}`).toBe(true);
    expect(
      collectTsFiles(excludedAbs).length,
      `排除目录下无 .ts/.tsx 文件：src/${EXCLUDED_DIR_REL}`,
    ).toBeGreaterThan(0);
    // 扫描集非空，且不包含 __tests__ 路径
    expect(files.length, "src/ 未枚举到任何 .ts/.tsx 文件").toBeGreaterThan(0);
    expect(files.some((f) => isExcludedRel(f))).toBe(false);
  });

  it("白名单自检：登记的文件必须存在且被扫描到（拼写错会静默空扫）", () => {
    for (const rel of Object.keys(WHITELIST)) {
      const abs = resolve(repoRoot, rel);
      expect(existsSync(abs), `白名单文件不存在：${rel}`).toBe(true);
      expect(
        files.includes(rel),
        `白名单文件未被扫描（可能位于排除目录或路径拼写错）：${rel}`,
      ).toBe(true);
      // 白名单允许字符必须是禁止集合的子集（防止白名单漂移成"允许一切"）
      for (const ch of WHITELIST[rel]) {
        expect(BANNED_CHARS.includes(ch), `白名单字符不在禁止集合中：${rel} "${ch}"`).toBe(true);
      }
    }
  });

  it("src/ 全树（含注释与字符串字面量）无装饰 emoji 字面量", () => {
    expect(
      violations,
      `src/ 出现装饰 emoji 字面量：\n${formatViolations(violations)}`,
    ).toEqual([]);
  });
});
