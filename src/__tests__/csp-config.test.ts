// CSP 配置不变量测试（L2 回归守卫）
//
// 背景（S10-②/④ → ADR-0021）：预览渲染载体 = 自定义协议宿主页（域级 CSP meta
// 放行内联脚本与 data: img/font——注入机制与数据通道在域内宽松执行）；ADR-0021
// 起宿主页改由主窗内跨源沙箱 iframe 承载（frame-src 收窄式新增该域）。
// 主窗口 CSP 终态 = 回收 script-src 'unsafe-inline' 与
// dangerousDisableAssetCspModification（CP-012）+ 回收 img-src/font-src 的
// data:（CP-035——data: 数据/字体仅存预览域渲染，主窗口零消费）。
// jsdom 不强制 CSP、无法验证真实执行，故本测试锁死 tauri.conf.json 的 CSP 决策，
// 任何回退/收紧立即失败。真实执行行为由 L4 E2E（真实 WebView2 强制 CSP）验证。

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

  it("script-src 终态：恰好 = ['self']——回收内联脚本放行（CP-012）", () => {
    // 预览迁独立 webview 后主窗口不再承载 srcdoc 内联注入——script-src 收为
    // 纯同源；'unsafe-inline' 若回潮（重新放行内联）立即红
    expect(directives["script-src"]).toBeDefined();
    expect(directives["script-src"]).toEqual(["'self'"]);
    // 正则冗余断言（防指令内混入写法绕过）
    expect(csp).not.toMatch(/script-src[^;]*'unsafe-inline'/);
  });

  it("dangerousDisableAssetCspModification 键不存在（CP-012 删除整键）", () => {
    // 键删除后 Tauri 资产页 CSP 恢复默认修改（nonce/哈希注入）——不再人为放宽
    const flag = (
      security as { dangerousDisableAssetCspModification?: unknown }
    ).dangerousDisableAssetCspModification;
    expect(flag).toBeUndefined();
  });

  it("default-src 保持严格同源（未被误放宽）", () => {
    expect(directives["default-src"]).toEqual(["'self'"]);
  });

  it("放宽范围仅限内联，未开外部资源（决策边界守卫）", () => {
    // script-src / default-src / style-src 不得含 https: 或通配 *，锁死「仅内联脚本/事件」决策
    for (const dir of ["default-src", "script-src", "style-src"]) {
      const sources = directives[dir] ?? [];
      expect(sources).not.toContain("*");
      expect(sources.some((s) => s.includes("https:"))).toBe(false);
    }
  });

  // ═══════════════════════════════════════════════════════════════
  // IHE-07④：style-src / connect-src / img-src 关键字段快照
  // ═══════════════════════════════════════════════════════════════

  it("style-src 放行同源 + 内联样式（React inline style / CM6 注入样式）", () => {
    expect(directives["style-src"]).toBeDefined();
    expect(directives["style-src"]).toContain("'self'");
    expect(directives["style-src"]).toContain("'unsafe-inline'");
    // 正则冗余断言（容忍空格/顺序）
    expect(csp).toMatch(/style-src[^;]*'unsafe-inline'/);
  });

  it("img-src 放行同源 + Tauri 资产协议（asset: + asset.localhost）", () => {
    // 打包资源（图标/内置文件）走 asset 协议——收紧会静默断图片
    expect(directives["img-src"]).toBeDefined();
    expect(directives["img-src"]).toContain("'self'");
    expect(directives["img-src"]).toContain("asset:");
    expect(directives["img-src"]).toContain("https://asset.localhost");
  });

  it("img-src 终态：恰好 = ['self', 'asset:', 'https://asset.localhost']——data: 已回收（CP-035）", () => {
    // 预览迁独立 webview 后主窗口不再承载预览文档（data: 图片内联消费仅存
    // 预览域——该域 CSP meta 放行 img/font data:）；主窗口 img-src 回收到恰好三项，
    // data: 若回潮立即红。blob: 维持否定（无生命周期管理点的通道不放行）。
    expect(directives["img-src"]).toBeDefined();
    expect(directives["img-src"]).toEqual(["'self'", "asset:", "https://asset.localhost"]);
    expect(directives["img-src"]).not.toContain("data:");
    expect(directives["img-src"]).not.toContain("blob:");
  });

  it("font-src 终态：恰好 = ['self']——data: 已回收（CP-035）", () => {
    // KaTeX 数学字体内联 data: font 仅装配于预览文档 head（预览域渲染，
    // ③ CP-033 实证真实加载）；主窗口 App 自身 @fontsource 走 'self'。
    expect(directives["font-src"]).toBeDefined();
    expect(directives["font-src"]).toEqual(["'self'"]);
  });

  it("data: 不在主窗口任何指令（CP-035 全指令守卫）", () => {
    // 主窗口 CSP 零 data:——任何指令（img/font/style/…）都不放行 data:；
    // 需要 data: 的通道只存在于预览域（CSP meta 放行 img/font data:），主窗口收紧不波及
    for (const sources of Object.values(directives)) {
      expect(sources).not.toContain("data:");
    }
    expect(csp).not.toContain("data:");
  });

  it("connect-src 未显式声明——回退 default-src 'self'（快照）", () => {
    // Tauri 2 IPC 走自定义协议（ipc:），不经过 CSP connect-src 约束；
    // 无显式 connect-src = 回退 default-src 'self' = 禁外部网络连出。
    // 若未来显式添加，此处断言该快照变化。
    expect(directives["connect-src"]).toBeUndefined();
  });

  it("frame-src 终态：恰好 = ['http://slterm-preview.localhost']（ADR-0021）", () => {
    // 预览回迁主窗 DOM——宿主页经主窗内跨源沙箱 iframe 承载（自定义协议域，
    // Windows 映射 http://slterm-preview.localhost）；frame-src 收窄式新增仅此
    // 一源（不放行通配/https:——其余帧源一律拒绝）
    expect(directives["frame-src"]).toEqual(["http://slterm-preview.localhost"]);
  });
});
