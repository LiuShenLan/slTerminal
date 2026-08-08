// cli-icons.test.ts — CliIconRegistry（CLI → 品牌 logo 注册表）L2 测试
//
// 覆盖：默认 claude 注册、register/match/getSrc 全分支、同 command 覆盖、
// 首 token 匹配边界（带参/空白/空串/大小写）、_reset 隔离、
// public/cli-icons/claude.png 文件存在性守卫（守护用户已放图——缺失则构建产物 img 404 静默）。

import { describe, it, expect, afterEach } from "vitest";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { cliIconRegistry, CliIconRegistry } from "../lib/cliIcons";

/** 每用例后恢复默认 claude 注册（全局单例隔离，照 TerminalRegistry._reset 模式） */
afterEach(() => {
  cliIconRegistry._reset();
  cliIconRegistry.register({ command: "claude", src: "/cli-icons/claude.png" });
});

describe("CliIconRegistry", () => {
  it("默认注册 claude 条目 → match('claude') 返回 /cli-icons/claude.png", () => {
    expect(cliIconRegistry.match("claude")).toBe("/cli-icons/claude.png");
    expect(cliIconRegistry.getSrc("claude")).toBe("/cli-icons/claude.png");
  });

  it("match 首 token 命中带参变体（claude --resume abc / claude -p）", () => {
    expect(cliIconRegistry.match("claude --resume abc")).toBe(
      "/cli-icons/claude.png",
    );
    expect(cliIconRegistry.match("claude -p \"hi\"")).toBe(
      "/cli-icons/claude.png",
    );
  });

  it("match 仅空白 → null", () => {
    expect(cliIconRegistry.match("   ")).toBeNull();
  });

  it("match 空串 → null", () => {
    expect(cliIconRegistry.match("")).toBeNull();
  });

  it("match 未命中命令 → null", () => {
    expect(cliIconRegistry.match("codex --version")).toBeNull();
  });

  it("match 大小写敏感（Claude ≠ claude）→ null", () => {
    expect(cliIconRegistry.match("Claude")).toBeNull();
  });

  it("register 新条目 → match/getSrc 命中", () => {
    cliIconRegistry.register({ command: "codex", src: "/cli-icons/codex.png" });
    expect(cliIconRegistry.match("codex")).toBe("/cli-icons/codex.png");
    expect(cliIconRegistry.getSrc("codex")).toBe("/cli-icons/codex.png");
  });

  it("register 同 command 覆盖旧条目", () => {
    cliIconRegistry.register({ command: "claude", src: "/cli-icons/new.png" });
    expect(cliIconRegistry.match("claude")).toBe("/cli-icons/new.png");
  });

  it("getSrc 未注册命令 → null", () => {
    expect(cliIconRegistry.getSrc("gemini")).toBeNull();
  });

  it("_reset 清空 → match/getSrc 全 null；重新注册后恢复", () => {
    cliIconRegistry._reset();
    expect(cliIconRegistry.match("claude")).toBeNull();
    cliIconRegistry.register({ command: "claude", src: "/cli-icons/claude.png" });
    expect(cliIconRegistry.match("claude")).toBe("/cli-icons/claude.png");
  });

  it("独立实例互不共享（new CliIconRegistry 空表）", () => {
    const fresh = new CliIconRegistry();
    expect(fresh.match("claude")).toBeNull();
  });

  it("public/cli-icons/claude.png 文件存在（资源守卫——用户放置的 logo 缺失则 img 404 静默）", () => {
    const here = dirname(fileURLToPath(import.meta.url));
    const iconPath = resolve(here, "../../public/cli-icons/claude.png");
    expect(existsSync(iconPath)).toBe(true);
    // PNG 魔数校验（防误放非 PNG 文件）
    const head = readFileSync(iconPath).subarray(0, 8);
    expect(Array.from(head)).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  });
});
