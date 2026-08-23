// no-claude-literals.test.ts — AC-5 字面量守卫（Stage 07，L2 grep 守卫形态）
//
// 背景：multi-cli profile 重构后，claude 知识（"claude" 字符串字面量、claude
// 事件名字符串、~/.claude 路径）只允许存在于 profiles/claude/（claude 合法领地，
// MC-213/223）与后端 claude provider；通用层一律经 CliProfileRegistry 消费，
// 缺省回退经 profiles/claude 导出的常量引用（CLAUDE_CLI_ID / SESSION_END_EVENT
// / EXIT_EVENT，MC-205/313 豁免写法）。
//
// 扫描范围（TQ-C-04 扩全 src）：src/ 全量递归（readdirSync recursive）扫描
// .ts/.tsx——白名单制会让新增通用目录逃脱，全量扫描后新目录自动纳入。
// 目录级豁免（CS-2）：src/features/cliProfiles/profiles/claude/ 是 claude 合法领地
// （MC-213/223）——claude 知识唯一合法聚居地，整目录不参与扫描；豁免按相对路径
// 前缀匹配（正斜杠归一），拼写错会静默空扫，由自检用例守住。
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
//    转义形态（\uXXXX / 常见转义）解码后比较。当前八路径经宽松文本 grep 实证
//    零命中（2026-08-10，cliProfiles 根目录 2026-08-11 复核），守卫通过即基线。

import { describe, it, expect, beforeAll } from "vitest";
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { resolve, join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../..");

/** AC-5 扫描范围：全 src 递归，仅排除豁免目录（TQ-C-04——白名单制会让新增通用目录逃脱） */
const SCAN_ROOT = "src";

/** 目录级豁免：claude 合法领地（身份域），整目录不参与扫描 */
const EXEMPT_DIRS = [
  // MC-213/223：claude CLI 身份域（CLAUDE_CLI_ID 等常量/事件名/路径唯一合法聚居地）
  "src/features/cliProfiles/profiles/claude",
  // claude hooks 配置面板专属（MC-504 下移）：hub 经 configEditor 分派的 claude 专属编辑器
  // （KZ-7 mockcli 用桩编辑器不共用），eventsCatalog = 官方 schema 提取的 claude hooks
  // 事件目录，提示文案含 ~/.claude 路径——TQ-C-04 全量扫描判定的身份域漏标，豁免并注释
  "src/panels/hooksConfig",
  // 测试夹具域：模拟 claude CLI 身份/事件名/~/.claude 路径的样例数据，
  // 验证身份域行为之必需，非生产通用层——守卫目标 = 生产源码（TQ-C-04 判定）
  "src/__tests__",
];

/** 文件级豁免：本守卫自身 */
const EXEMPT_FILES = [
  "src/__tests__/no-claude-literals.test.ts",
];

/** 豁免判定：目录级按前缀、文件级精确匹配（正斜杠归一；拼写错会静默空扫，自检用例守住） */
function isExempt(fileRel: string): boolean {
  const rel = fileRel.replace(/\\/g, "/");
  if (EXEMPT_FILES.includes(rel)) return true;
  return EXEMPT_DIRS.some((dir) => rel.startsWith(dir + "/"));
}

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
 * - 模板字符串：含 ${}（表达式）不再整体跳过（CS-1）——提取表达式外的字面量
 *   片段拼接成单值后参与判定（cl${''}aude 形态可命中）；表达式体按花括号配对
 *   跳过，不识别表达式内嵌套模板/引号（极简词法器已知边界，保守方向，见口径 5）
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
      // 模板字符串：收集表达式外的字面量片段，拼接成单值后参与判定（CS-1）
      let j = i + 1;
      const frags: string[] = [];
      let raw = "";
      while (j < n) {
        const ch = src[j];
        if (ch === "\\") { raw += ch + (src[j + 1] ?? ""); j += 2; }
        else if (ch === "`") break;
        else if (ch === "$" && src[j + 1] === "{") {
          // 进入表达式：收尾当前字面量片段，按花括号配对跳过表达式体
          frags.push(raw);
          raw = "";
          let depth = 1;
          j += 2;
          while (j < n && depth > 0) {
            const c2 = src[j];
            if (c2 === "{") depth++;
            else if (c2 === "}") depth--;
            j++;
          }
        } else { raw += ch; j++; }
      }
      frags.push(raw);
      out.push(decodeLiteral(frags.join("")));
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

/** 单个字面量值的三类判定（claude 精确 / 10 事件名 / ~/.claude 路径），未命中返回 null */
function classifyLiteral(value: string): Violation["kind"] | null {
  if (value === "claude") return "claude-literal";
  if (CLAUDE_EVENT_NAMES.has(value)) return "claude-event";
  if (value.includes("~/.claude") || value.includes("~\\.claude")) return "claude-path";
  return null;
}

/** 全量扫描：收集三类违规（全 src 递归 × .ts/.tsx，豁免目录/文件除外） */
function scanViolations(): { violations: Violation[]; files: string[] } {
  const violations: Violation[] = [];
  const files: string[] = [];
  // Node 22 readdirSync recursive 全量递归——新增目录自动纳入，无法再逃脱（TQ-C-04）
  for (const entry of readdirSync(resolve(repoRoot, SCAN_ROOT), {
    recursive: true,
    withFileTypes: true,
  })) {
    if (!entry.isFile() || !/\.tsx?$/.test(entry.name)) continue;
    const fileRel = relative(repoRoot, join(entry.parentPath, entry.name));
    // CS-2：目录级豁免 claude 合法领地（profiles/claude/）+ 文件级豁免——不参与违规收集
    if (isExempt(fileRel)) continue;
    files.push(fileRel);
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
      const kind = classifyLiteral(value);
      if (kind) violations.push({ file, value, kind });
    }
  }
  return { violations, files };
}

/** 格式化违规列表为可读断言消息 */
function formatViolations(vs: Violation[]): string {
  return vs.map((v) => `${v.file}: ${v.kind} "${v.value}"`).join("\n");
}

describe("AC-5 字面量守卫（通用层八路径不出现 claude 字面量；profiles/claude 合法领地豁免）", () => {
  let violations: Violation[];
  let files: string[];

  beforeAll(() => {
    ({ violations, files } = scanViolations());
  });

  it("扫描范围完整性：全 src 递归枚举到 .ts(x) 文件（防静默空扫；白名单制已废，TQ-C-04）", () => {
    expect(files.length, "全 src 未枚举到任何 .ts/.tsx 文件").toBeGreaterThan(0);
    // 抽检此前白名单外的目录已纳入——新增通用目录无法再逃脱扫描（TQ-C-04）
    const normalized = files.map((f) => f.replace(/\\/g, "/"));
    for (const dir of ["src/features", "src/workspace", "src/stores", "src/panels", "src/theme"]) {
      const matches = normalized.filter((f) => f.startsWith(dir + "/"));
      expect(matches.length, `${dir} 未枚举到任何 .ts/.tsx 文件`).toBeGreaterThan(0);
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

  it("CS-1 自检：含 ${} 的模板字符串按字面量片段拼接判定——`cl${''}aude` 拼出 'claude' 报违规", () => {
    // 样例源码：模板拼接产出运行时值 "claude"（生产代码曾可用此形态绕过旧守卫）
    const sample = 'const x = `cl${\'\'}aude`;';
    const values = extractStringLiterals(sample);
    expect(values).toContain("claude"); // 片段 cl + aude 拼接后命中精确匹配
    expect(values.map((v) => classifyLiteral(v))).toContain("claude-literal");
    // 反向锚点：表达式内容不混入字面量片段（`pre-${x}post` → "pre-post"，不误报）
    expect(extractStringLiterals("const y = `pre-${x}post`;")).toEqual(["pre-post"]);
    expect(classifyLiteral("pre-post")).toBeNull();
  });

  it("CS-2 自检：profiles/claude 目录存在且被目录级豁免（豁免路径拼写错会静默空扫）", () => {
    // 存在性防线：豁免目录拼写错 → 不存在即暴露，claude 领地会被误扫
    const exemptDirAbs = resolve(repoRoot, EXEMPT_DIRS[0]);
    expect(existsSync(exemptDirAbs), `豁免目录不存在：${EXEMPT_DIRS[0]}`).toBe(true);
    // 非空豁免防线：该目录确有 .ts/.tsx 被枚举到（否则豁免形同虚设）
    expect(collectTsFiles(exemptDirAbs).length).toBeGreaterThan(0);
    // 豁免生效防线：该目录下样例路径不参与违规收集（files 即实际扫描集）
    expect(files.some((f) => isExempt(f))).toBe(false);
  });
});
