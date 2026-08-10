// no-claude-literals.test.ts — AC-5 字面量守卫（Stage 07，L2 grep 守卫形态）
//
// 背景：multi-cli profile 重构后，claude 知识（"claude" 字符串字面量、claude
// 事件名字符串、~/.claude 路径）只允许存在于 profiles/claude/（claude 合法领地，
// MC-213/223）与后端 claude provider；通用层一律经 CliProfileRegistry 消费，
// 缺省回退经 profiles/claude 导出的常量引用（CLAUDE_CLI_ID / SESSION_END_EVENT
// / EXIT_EVENT，MC-205/313 豁免写法）。
//
// 扫描范围（通用层七路径）：src/lib、src/panels/terminal、
// src/features/agentStatus、src/features/agentHistory、src/features/notifications、
// src/ipc、src/types。守卫自身用 fs 枚举目录递归扫描 .ts/.tsx，新增文件自动纳入。
//
// 断言口径（语义式）：
// 1. "claude" 字符串字面量——精确匹配（值 === "claude"）才算违规；子串
//    （如 "claude-cli"）与标识符（如常量名）不计。行注释/块注释内容一律跳过。
// 2. claude 事件名字符串字面量——下列事件名作为字符串字面量出现才算违规：
//    SessionStart / SessionEnd / UserPromptSubmit / Stop / StopFailure /
//    PreToolUse / PostToolUse / PostToolUseFailure / Notification /
//    PermissionRequest；作为标识符（如 SESSION_END_EVENT 常量）或注释出现不计。
//    通用层消费一律 import profiles/claude 导出常量（豁免形态见下）。
// 3. "~/.claude" 路径字面量——值包含 "~/.claude"（含 Windows 反斜杠变体
//    "~\.claude"）即违规。
// 4. 豁免：import / export-from / 动态 import 的路径字符串，凡指向
//    features/cliProfiles/profiles/claude/（CLAUDE_CLI_ID 等常量引用形态）不检查。
// 5. 词法近似说明：本守卫自实现极简词法器提取字符串字面量（~70 行），不识别
//    正则字面量——正则内出现引号包围的违规字样会误报（保守方向，宁可人工复核）；
//    转义形态（\uXXXX / 常见转义）解码后比较。当前七路径经宽松文本 grep 实证
//    零命中（2026-08-10），守卫通过即基线。

import { describe, it, expect, beforeAll } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { resolve, join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../..");

/** AC-5 扫描范围：通用层七路径（相对 repoRoot） */
const SCAN_DIRS = [
  "src/lib",
  "src/panels/terminal",
  "src/features/agentStatus",
  "src/features/agentHistory",
  "src/features/notifications",
  "src/ipc",
  "src/types",
];

/** 禁止作为字符串字面量出现的 claude 事件名（claude 协议知识专属 profiles/claude/） */
const CLAUDE_EVENT_NAMES = new Set([
  "SessionStart",
  "SessionEnd",
  "UserPromptSubmit",
  "Stop",
  "StopFailure",
  "PreToolUse",
  "PostToolUse",
  "PostToolUseFailure",
  "Notification",
  "PermissionRequest",
]);

/** 豁免：import 路径指向 features/cliProfiles/profiles/claude/（常量引用形态） */
const EXEMPT_IMPORT_PATH_MARK = "cliProfiles/profiles/claude";

interface Violation {
  file: string;
  value: string;
  kind: "claude-literal" | "claude-event" | "claude-path";
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

/** 轻量解码字符串字面量：\uXXXX + 常见转义（" \ ' \ n \ r \ t \ b） */
function decodeLiteral(raw: string): string {
  return raw.replace(/\\(u[0-9a-fA-F]{4}|["'\\nrtb])/g, (_m, esc: string) => {
    if (esc[0] === "u") return String.fromCharCode(parseInt(esc.slice(1), 16));
    switch (esc) {
      case "n": return "\n";
      case "r": return "\r";
      case "t": return "\t";
      case "b": return "\b";
      default: return esc;
    }
  });
}

/**
 * 极简词法器：提取源码中全部字符串字面量值（解码后）。
 * - 跳过行注释（// 开头）与块注释（斜杠星号开头、星号斜杠结尾成对）——
 *   注释内字样不计（口径 1/2）
 * - 双引号/单引号字符串：转义序列原样保留后统一解码
 * - 模板字符串：含 ${}（表达式）的跳过——值非纯字面量，不检查
 * - 正则字面量不识别：正则内引号包围字样会作为字符串提取（保守方向，见口径 5）
 */
function extractStringLiterals(src: string): string[] {
  const out: string[] = [];
  let i = 0;
  const n = src.length;
  while (i < n) {
    const c = src[i];
    if (c === "/" && src[i + 1] === "/") {
      const nl = src.indexOf("\n", i);
      i = nl === -1 ? n : nl + 1;
    } else if (c === "/" && src[i + 1] === "*") {
      const end = src.indexOf("*/", i + 2);
      i = end === -1 ? n : end + 2;
    } else if (c === '"' || c === "'") {
      const quote = c;
      let j = i + 1;
      let raw = "";
      while (j < n) {
        const ch = src[j];
        if (ch === "\\") { raw += ch + (src[j + 1] ?? ""); j += 2; }
        else if (ch === quote) break;
        else { raw += ch; j++; }
      }
      out.push(decodeLiteral(raw));
      i = j + 1;
    } else if (c === "`") {
      let j = i + 1;
      let raw = "";
      let hasExpr = false;
      while (j < n) {
        const ch = src[j];
        if (ch === "\\") { raw += ch + (src[j + 1] ?? ""); j += 2; }
        else if (ch === "`") break;
        else { if (ch === "$" && src[j + 1] === "{") hasExpr = true; raw += ch; j++; }
      }
      if (!hasExpr) out.push(decodeLiteral(raw));
      i = j + 1;
    } else {
      i++;
    }
  }
  return out;
}

/** 提取 import / export-from / 动态 import 的路径字符串（解码后） */
function collectImportPaths(src: string): string[] {
  const paths: string[] = [];
  const patterns = [
    /(?:^|[^\w.])import\s+[^;]*?\bfrom\s*['"]([^'"]+)['"]/g,
    /(?:^|[^\w.])export\s+[^;]*?\bfrom\s*['"]([^'"]+)['"]/g,
    /import\s*\(\s*['"]([^'"]+)['"]\s*\)/g,
  ];
  for (const re of patterns) {
    let m: RegExpExecArray | null;
    while ((m = re.exec(src)) !== null) paths.push(m[1]);
  }
  return paths;
}

/** 全量扫描：收集三类违规（七路径 × 全部 .ts/.tsx） */
function scanViolations(): { violations: Violation[]; files: string[] } {
  const violations: Violation[] = [];
  const files: string[] = [];
  for (const rel of SCAN_DIRS) {
    const abs = resolve(repoRoot, rel);
    files.push(...collectTsFiles(abs).map((f) => relative(repoRoot, f)));
  }
  for (const file of files) {
    const src = readFileSync(resolve(repoRoot, file), "utf8");
    // 豁免集合：指向 cliProfiles/profiles/claude 的 import 路径（常量引用形态）
    const exemptPaths = new Set(
      collectImportPaths(src)
        .map((p) => decodeLiteral(p).replace(/\\/g, "/"))
        .filter((p) => p.includes(EXEMPT_IMPORT_PATH_MARK)),
    );
    for (const value of extractStringLiterals(src)) {
      if (exemptPaths.has(value.replace(/\\/g, "/"))) continue; // 豁免 import 路径
      if (value === "claude") {
        violations.push({ file, value, kind: "claude-literal" });
      } else if (CLAUDE_EVENT_NAMES.has(value)) {
        violations.push({ file, value, kind: "claude-event" });
      } else if (value.includes("~/.claude") || value.includes("~\\.claude")) {
        violations.push({ file, value, kind: "claude-path" });
      }
    }
  }
  return { violations, files };
}

/** 格式化违规列表为可读断言消息 */
function formatViolations(vs: Violation[]): string {
  return vs.map((v) => `${v.file}: ${v.kind} "${v.value}"`).join("\n");
}

describe("AC-5 字面量守卫（通用层七路径不出现 claude 字面量）", () => {
  let violations: Violation[];
  let files: string[];

  beforeAll(() => {
    ({ violations, files } = scanViolations());
  });

  it("扫描范围完整性：七路径均存在且枚举到 .ts(x) 文件（防路径拼写错致静默空扫）", () => {
    // relative() 在 Windows 返回反斜杠路径，统一归一正斜杠后比较前缀
    const normalized = files.map((f) => f.replace(/\\/g, "/"));
    for (const rel of SCAN_DIRS) {
      const matches = normalized.filter((f) => f.startsWith(rel + "/"));
      expect(matches.length, `${rel} 未枚举到任何 .ts/.tsx 文件`).toBeGreaterThan(0);
    }
  });

  it("无值等于 'claude' 的字符串字面量（精确匹配；import 路径指向 cliProfiles/profiles/claude 豁免）", () => {
    const vs = violations.filter((v) => v.kind === "claude-literal");
    expect(vs, `通用层出现 "claude" 字符串字面量：\n${formatViolations(vs)}`).toEqual([]);
  });

  it("无 claude 事件名字符串字面量（SessionStart/PreToolUse 等 10 事件，标识符/注释不计）", () => {
    const vs = violations.filter((v) => v.kind === "claude-event");
    expect(vs, `通用层出现 claude 事件名字符串字面量：\n${formatViolations(vs)}`).toEqual([]);
  });

  it("无 '~/.claude' 路径字面量（含反斜杠变体）", () => {
    const vs = violations.filter((v) => v.kind === "claude-path");
    expect(vs, `通用层出现 ~/.claude 路径字面量：\n${formatViolations(vs)}`).toEqual([]);
  });
});
