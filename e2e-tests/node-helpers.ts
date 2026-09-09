// node-helpers.ts — E2E Node runner 侧公共件（与浏览器侧 helpers.ts 分工：
// helpers.ts 经 VITE_E2E 打包进前端挂 window 全局（禁 Node API）；
// 本文件仅在 wdio runner Node 进程内被 spec import，禁止经任何前端路径打包）

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

/** 写假值余量 env 到 user 层 settings.json（SEC-18 假值占位符；deepseek URL 命中
    QUERIES 匹配集 → 后端刷新后产出占位行 → 导航树余量 footer 可断言）。
    TE-06 收编单点（原 settings/background-tasks 两副本，后者缺 mkdirSync solo 必
    ENOENT——以 settings 版为基） */
export function writeFakePlanEnv(claudeSettingsPath: string): void {
  let root: Record<string, unknown>;
  try {
    root = JSON.parse(readFileSync(claudeSettingsPath, "utf8")) as Record<string, unknown>;
  } catch {
    root = {};
  }
  const env = (root.env ?? {}) as Record<string, unknown>;
  env.ANTHROPIC_BASE_URL = "https://api.deepseek.com/anthropic";
  env.ANTHROPIC_AUTH_TOKEN = "sk-test-e2e"; // 假值占位符（SEC-18，非真实凭据）
  root.env = env;
  // 假屋为 per-pid 唯一新目录——solo 跑时 .claude 未必存在，先建父目录（2026-09-08 实证：
  // 全量队列中 hooks spec 先行建目录故既往全量绿、单 spec 独立假屋必 ENOENT）
  mkdirSync(dirname(claudeSettingsPath), { recursive: true });
  writeFileSync(claudeSettingsPath, JSON.stringify(root, null, 2), "utf8");
}
