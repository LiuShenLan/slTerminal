// CSP 配置不变量测试（L2 回归守卫）
//
// 背景：HTML 预览面板用 <iframe srcDoc> 加载，about:srcdoc 按规范继承主窗口 CSP。
// 若主窗口 CSP 缺 script-src 'unsafe-inline'（或 nonce 注入未关），srcdoc 内联脚本/
// 事件属性会被静默拦截 —— 正是本次修复的目标。jsdom 不强制 CSP、无法验证真实执行，
// 故本测试锁死 tauri.conf.json 的 CSP 决策，任何回退/收紧立即失败。
// 真实执行行为由 L4 E2E（真实 WebView2 强制 CSP）验证。

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

// 运行时读取配置文件（相对本测试文件定位），不走 TS 模块解析，避免 include 边界问题
const here = dirname(fileURLToPath(import.meta.url));
const confPath = resolve(here, "../../src-tauri/tauri.conf.json");
const conf = JSON.parse(readFileSync(confPath, "utf8"));
const security: { csp: string; dangerousDisableAssetCspModification?: unknown } =
  conf.app.security;
const csp: string = security.csp;

/** 把 CSP 字符串解析成 { 指令名: [源列表] } */
function parseDirectives(policy: string): Record<string, string[]> {
  return Object.fromEntries(
    policy
      .split(";")
      .map((s) => s.trim())
      .filter(Boolean)
      .map((d) => {
        const [name, ...sources] = d.split(/\s+/);
        return [name, sources];
      }),
  );
}

describe("tauri.conf.json CSP 不变量", () => {
  const directives = parseDirectives(csp);

  it("script-src 放行同源 + 内联脚本/事件", () => {
    // 覆盖内联 <script> 与 onclick/onload 等内联事件属性
    expect(directives["script-src"]).toBeDefined();
    expect(directives["script-src"]).toContain("'self'");
    expect(directives["script-src"]).toContain("'unsafe-inline'");
    // 正则冗余断言（容忍空格/顺序）
    expect(csp).toMatch(/script-src[^;]*'self'/);
    expect(csp).toMatch(/script-src[^;]*'unsafe-inline'/);
  });

  it("关闭 script-src 的 nonce 注入（否则 unsafe-inline 被忽略）", () => {
    // 缺此项时 Tauri 注入 nonce → 按 CSP3 规范 'unsafe-inline' 失效 → srcdoc 仍被拦
    const flag = security.dangerousDisableAssetCspModification;
    expect(Array.isArray(flag)).toBe(true);
    expect(flag as string[]).toContain("script-src");
  });

  it("default-src 保持严格同源（未被误放宽）", () => {
    expect(directives["default-src"]).toEqual(["'self'"]);
  });

  it("放宽范围仅限内联，未开外部资源（决策边界守卫）", () => {
    // script-src / default-src 不得含 https: 或通配 *，锁死「仅内联脚本/事件」决策
    for (const dir of ["default-src", "script-src"]) {
      const sources = directives[dir] ?? [];
      expect(sources).not.toContain("*");
      expect(sources.some((s) => s.includes("https:"))).toBe(false);
    }
  });
});
