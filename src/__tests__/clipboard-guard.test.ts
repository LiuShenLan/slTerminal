// clipboard-guard.test.ts — SEC-06 剪贴板读权限消费点守卫（L2 grep 级）
//
// 背景：capabilities/default.json 含 clipboard-manager:allow-read-text（D6 决策保留——
// 改后端命令不缩小攻击面，前端上下文被注入时同样能 invoke）。权限本身不动，
// 契约守卫锁死 readText（读剪贴板）的消费点集合：
//   1. src/ipc/clipboard.ts —— re-export 定义处（IPC 唯一入口，硬约束 #1）
//   2. src/panels/terminal/keyboard.ts —— 唯一业务消费点（Ctrl+Shift+V 显式手势的 terminal.paste）
//   3. 测试文件（src/__tests__/ 下，mock 声明）
// 实现：fs 遍历 src/ 递归读 .ts/.tsx grep "readText" 字面量，命中路径集合硬编码断言。
// 新增 readText 消费点（如新面板直接粘贴）必须显式改白名单，否则本测试 fail——
// 提醒评审新权限面（writeText 不受限：OSC 52 与 Ctrl+Shift+C 共用写入路径）。
// 守卫自身为自实现 grep（不 import 生产模块），照 no-claude-literals.test.ts 先例。

import { describe, it, expect, beforeAll } from "vitest";
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { resolve, join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../..");

/** 扫描根：src/ 全量（含 src/__tests__/） */
const SCAN_ROOT = "src";

/** readText 允许出现的生产文件（相对 repoRoot，正斜杠归一，硬编码） */
const ALLOWED_FILES = new Set([
  "src/ipc/clipboard.ts",
  "src/panels/terminal/keyboard.ts",
]);

/** 测试文件豁免前缀：src/__tests__/（mock 声明与守卫断言自身必然含 readText 字面量） */
const TEST_DIR_REL = "src/__tests__/";

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

describe("SEC-06 剪贴板 readText 消费点守卫（clipboard-manager:allow-read-text 契约）", () => {
  /** 命中 "readText" 字面量的文件（相对 repoRoot，正斜杠归一） */
  let hits: string[];

  beforeAll(() => {
    hits = collectTsFiles(resolve(repoRoot, SCAN_ROOT))
      .map((f) => relative(repoRoot, f))
      .filter((f) => readFileSync(resolve(repoRoot, f), "utf8").includes("readText"))
      .map((f) => f.replace(/\\/g, "/"));
  });

  it("扫描范围完整性：src/ 枚举到文件（防路径拼写错致静默空扫）", () => {
    expect(hits.length, "src/ 未枚举到任何文件").toBeGreaterThan(0);
  });

  it("readText 仅出现于 src/ipc/clipboard.ts、src/panels/terminal/keyboard.ts 与测试文件", () => {
    const unexpected = hits.filter(
      (f) => !ALLOWED_FILES.has(f) && !f.startsWith(TEST_DIR_REL),
    );
    expect(
      unexpected,
      `readText 出现在未登记消费点（新增读剪贴板面需评审并登记）：\n${unexpected.join("\n")}`,
    ).toEqual([]);
  });

  it("登记消费点均命中 readText（防白名单路径拼写错致守卫形同虚设）", () => {
    for (const f of ALLOWED_FILES) {
      expect(hits, `${f} 未包含 readText——白名单失效`).toContain(f);
    }
  });

  it("登记消费点文件均存在（防白名单路径拼写错致断言空转）", () => {
    for (const f of ALLOWED_FILES) {
      expect(existsSync(resolve(repoRoot, f)), `白名单文件不存在：${f}`).toBe(true);
    }
  });
});
