// sync-startup-colors.mjs — 启动链 fail-safe 静态色同步(CP-027)
// 色源单一:src/theme/schemes/linear.ts 三个 ui 标量(appBgPrimary/sidebarFg/errorFg);
// 提取后改写三处消费点:index.html body background、src-tauri/tauri.conf.json backgroundColor、
// src/theme/startupColors.ts(生成物)。纯 Node ESM,零依赖;提取失败 → 非零退出即红。
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join, resolve } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** 从 linear.ts 文本提取三个 fail-safe 槽位(锚定标量键,值为小写 6 位 hex) */
export function extractStartupColors(linearTs) {
  const pick = (key) => {
    const m = linearTs.match(new RegExp(`\\b${key}:\\s*"(#[0-9a-f]{6})"`));
    if (!m) {
      throw new Error(`linear.ts 未找到槽位 ${key}(CP-027 锚定提取失败,禁止自估色值)`);
    }
    return m[1];
  };
  return {
    bg: pick("appBgPrimary"),
    fg: pick("sidebarFg"),
    errorFg: pick("errorFg"),
  };
}

/** 生成 startupColors.ts 全文(照抄锁形态,测试同款断言) */
export function renderStartupColorsModule({ bg, fg, errorFg }) {
  return [
    "// startupColors.ts — 启动链 fail-safe 静态色(构建期生成物,勿手改)",
    "// 色源单一 = schemes/linear.ts(appBgPrimary/sidebarFg/errorFg)——生成器 scripts/sync-startup-colors.mjs(CP-027),",
    "// 经 package.json predev/prebuild 接线;改 linear.ts 三槽位后跑 npm run sync:startup-colors 即同步三处消费点。",
    `export const STARTUP_FAIL_SAFE_BG = "${bg}";`,
    `export const STARTUP_FAIL_SAFE_FG = "${fg}";`,
    `export const STARTUP_FAIL_SAFE_ERROR_FG = "${errorFg}";`,
    "",
  ].join("\n");
}

function rewriteIfChanged(path, next) {
  const prev = readFileSync(path, "utf8");
  if (prev === next) return false;
  writeFileSync(path, next);
  return true;
}

function main() {
  const linearTs = readFileSync(join(root, "src/theme/schemes/linear.ts"), "utf8");
  const colors = extractStartupColors(linearTs);

  // index.html body 底色
  const indexHtmlPath = join(root, "index.html");
  const indexHtml = readFileSync(indexHtmlPath, "utf8");
  // 守卫语义修正(CP-027 执行期):改写失败 = 正则未命中槽位;稳态(值与色源一致)
  // 时 replace 无文本变化属幂等正常——若按文本变化判定,predev/prebuild 恒抛红
  if (!/background:\s*#[0-9a-fA-F]{6};/.test(indexHtml)) {
    throw new Error("index.html body background 改写失败");
  }
  const nextIndexHtml = indexHtml.replace(
    /background:\s*#[0-9a-fA-F]{6};/,
    `background: ${colors.bg};`,
  );

  // tauri.conf.json 窗口底色
  const confPath = join(root, "src-tauri/tauri.conf.json");
  const conf = readFileSync(confPath, "utf8");
  if (!/"backgroundColor":\s*"#[0-9a-fA-F]{6}"/.test(conf)) {
    throw new Error("tauri.conf.json backgroundColor 改写失败");
  }
  const nextConf = conf.replace(
    /"backgroundColor":\s*"#[0-9a-fA-F]{6}"/,
    `"backgroundColor": "${colors.bg}"`,
  );

  const changed = [
    rewriteIfChanged(indexHtmlPath, nextIndexHtml) && "index.html",
    rewriteIfChanged(confPath, nextConf) && "src-tauri/tauri.conf.json",
    rewriteIfChanged(
      join(root, "src/theme/startupColors.ts"),
      renderStartupColorsModule(colors),
    ) && "src/theme/startupColors.ts",
  ].filter(Boolean);

  console.log(
    changed.length
      ? `[sync-startup-colors] 已同步: ${changed.join(", ")}`
      : "[sync-startup-colors] 三处消费点与 linear.ts 一致,无改动",
  );
}

// 仅直接执行时跑主流程(vitest import 提取/渲染函数无副作用)
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main();
}
