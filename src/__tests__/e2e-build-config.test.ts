// e2e-build-config.test.ts — E2E 构建/CI 配置不变量守卫（T3）
//
// 背景：E2E 二进制必须经 VITE_E2E=1 构建才含 helper，生产构建不设该开关。
// 本测试锁死 package.json 脚本与 ci.yml 步骤，任何回退（漏设 VITE_E2E、
// 删除生产剥离守卫、生产 build 误加开关）立即失败。仿 csp-config.test.ts 模式。

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(
  readFileSync(resolve(here, "../../package.json"), "utf8"),
) as {
  scripts: Record<string, string>;
  devDependencies: Record<string, string>;
};
const ci = readFileSync(
  resolve(here, "../../.github/workflows/ci.yml"),
  "utf8",
);
// IHE-04：E2E_ENABLED 必须为编译期内联字面量——读源文件做正则断言
const e2eEnabledSrc = readFileSync(
  resolve(here, "../lib/e2eEnabled.ts"),
  "utf8",
);

describe("package.json — E2E 构建脚本不变量", () => {
  it("build:e2e 用 cross-env 设 VITE_E2E=1 且走 tauri build", () => {
    const s = pkg.scripts["build:e2e"];
    expect(s).toBeDefined();
    expect(s).toContain("cross-env");
    expect(s).toContain("VITE_E2E=1");
    expect(s).toContain("tauri build");
  });

  it("e2e 脚本串联 build:e2e 与 wdio", () => {
    const s = pkg.scripts.e2e;
    expect(s).toBeDefined();
    expect(s).toContain("build:e2e");
    expect(s).toContain("wdio");
  });

  it("cross-env 在 devDependencies", () => {
    expect(pkg.devDependencies["cross-env"]).toBeDefined();
  });

  it("生产 build 脚本不含 VITE_E2E（安全属性：生产不启用 helper）", () => {
    expect(pkg.scripts.build).not.toContain("VITE_E2E");
  });
});

describe("src/lib/e2eEnabled.ts — E2E_ENABLED 编译期字面量守卫（IHE-04）", () => {
  it("E2E_ENABLED 直接内联 import.meta.env 字面量表达式（编译期常量 → Rollup DCE）", () => {
    // 若被包成函数调用/间接引用，Rollup 无法常量折叠 → 生产误带 helper
    expect(e2eEnabledSrc).toMatch(
      /export const E2E_ENABLED\s*=\s*import\.meta\.env\.DEV\s*\|\|\s*import\.meta\.env\.VITE_E2E\s*===\s*"1"/,
    );
  });

  it("E2E_ENABLED 不得经 computeE2eEnabled 函数调用定义（函数调用阻碍跨模块 DCE）", () => {
    expect(e2eEnabledSrc).not.toMatch(
      /E2E_ENABLED\s*=\s*(?:\(0,\s*)?computeE2eEnabled\(/,
    );
  });
});

describe("ci.yml — E2E 构建与生产剥离守卫", () => {
  it("E2E 前置构建设置 VITE_E2E 为 1", () => {
    expect(ci).toMatch(/VITE_E2E:\s*['"]?1['"]?/);
  });

  it("存在生产 dist helper 守卫 step（命中 installAllE2eHelpers 即 fail）", () => {
    expect(ci).toContain("installAllE2eHelpers");
    expect(ci).toMatch(/Select-String|grep/);
  });
});
