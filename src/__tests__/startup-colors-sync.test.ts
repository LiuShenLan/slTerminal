// startup-colors-sync.test.ts — 启动链 fail-safe 静态色单源一致守卫(L2,CP-027)
//
// 防复发对照:修复前 index.html / tauri.conf.json / main.tsx 三处硬编码靠人工与
// linear.ts 同步(ADR-0002 被否决备选)——本文件锁「三处消费点 == linear.ts 提取值」
// 与「main.tsx 无 fail-safe 字面量」,漂移即红。

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  extractStartupColors,
  renderStartupColorsModule,
} from "../../scripts/sync-startup-colors.mjs";

const root = join(__dirname, "../..");
const linearTs = readFileSync(join(root, "src/theme/schemes/linear.ts"), "utf8");
const colors = extractStartupColors(linearTs);

describe("extractStartupColors(CP-027 色源提取)", () => {
  it("linear.ts 现值提取:bg/fg/errorFg 三槽位", () => {
    expect(colors).toEqual({
      bg: "#0a0a0b",
      fg: "#ece9e4",
      errorFg: "#d9706b",
    });
  });

  it("槽位缺失 → 抛错(禁自估色值)", () => {
    expect(() => extractStartupColors("export const linear = {};")).toThrow(
      /appBgPrimary/,
    );
  });
});

describe("三处消费点与 linear.ts 一致(防漂移回归)", () => {
  it("index.html body background == appBgPrimary", () => {
    const html = readFileSync(join(root, "index.html"), "utf8");
    expect(html).toContain(`background: ${colors.bg};`);
  });

  it("tauri.conf.json backgroundColor == appBgPrimary", () => {
    const conf = readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8");
    expect(conf).toContain(`"backgroundColor": "${colors.bg}"`);
  });

  it("startupColors.ts == 渲染器输出(生成物未手改)", () => {
    const mod = readFileSync(join(root, "src/theme/startupColors.ts"), "utf8");
    expect(mod).toBe(renderStartupColorsModule(colors));
  });

  it("main.tsx 无 fail-safe 硬编码字面量(读 startupColors 常量)", () => {
    const mainTs = readFileSync(join(root, "src/main.tsx"), "utf8");
    expect(mainTs).not.toContain('"#0a0a0b"');
    expect(mainTs).not.toContain('"#ece9e4"');
    expect(mainTs).not.toContain('"#d9706b"');
    expect(mainTs).toContain("STARTUP_FAIL_SAFE_BG");
  });
});
