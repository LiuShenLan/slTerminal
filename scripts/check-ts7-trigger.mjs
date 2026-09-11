// scripts/check-ts7-trigger.mjs — CP-001 双 TS 并存解除触发条件的机检脚本
//
// 双条件同时满足(ADR-0010 TE-07)时退出码 0 并打印解除清单:
//   ① typescript-eslint issue #10940 闭环(GitHub state === "closed")
//   ② typescript 7.1 稳定版发布(npm dist-tags.latest = 7.1.x,无预发布后缀)
// 未达成退出码 1;网络/解析失败(含非 2xx 响应——TE-01)退出码 2(未知态,不误导判定)。
// 用法:node scripts/check-ts7-trigger.mjs

import http from "node:http";
import https from "node:https";
import { pathToFileURL } from "node:url";

const ISSUE_URL = "https://api.github.com/repos/typescript-eslint/typescript-eslint/issues/10940";
const REGISTRY_URL = "https://registry.npmjs.org/typescript";

/** 纯判定:触发是否达成(两个入参均为已取回的事实,供测试直接驱动) */
export function evaluateTrigger(issueState, latestVersion) {
  const issueClosed = issueState === "closed";
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(latestVersion ?? "");
  const ts71Stable = m !== null && Number(m[1]) === 7 && Number(m[2]) >= 1;
  return { issueClosed, ts71Stable, triggered: issueClosed && ts71Stable };
}

/** 极简 GET JSON(无外部依赖)——非 2xx 一律视为查询失败(落退出码 2 未知态):
 *  限流 403 等错误页可能返回合法 JSON,误解析会假报「未达成」(退出码 1)(TE-01) */
export function getJson(url) {
  return new Promise((resolve, reject) => {
    const mod = url.startsWith("https:") ? https : http; // http 分支仅供测试本地桩
    mod
      .get(url, { headers: { "user-agent": "slterminal-trigger-check" } }, (res) => {
        const status = res.statusCode ?? 0;
        if (status < 200 || status >= 300) {
          res.resume(); // 排空响应防连接占用
          reject(new Error(`HTTP ${status}`));
          return;
        }
        let body = "";
        res.on("data", (c) => (body += c));
        res.on("end", () => {
          try {
            resolve(JSON.parse(body));
          } catch (e) {
            reject(e);
          }
        });
      })
      .on("error", reject);
  });
}

async function main() {
  let issue;
  let registry;
  try {
    [issue, registry] = await Promise.all([getJson(ISSUE_URL), getJson(REGISTRY_URL)]);
  } catch (e) {
    console.error(`[ts7-trigger] 查询失败(网络/HTTP/解析):${e.message}`);
    process.exit(2);
  }
  const latest = registry?.["dist-tags"]?.latest;
  const r = evaluateTrigger(issue?.state, latest);
  console.log(`[ts7-trigger] issue #10940 state=${issue?.state};typescript latest=${latest}`);
  if (r.triggered) {
    console.log(
      "[ts7-trigger] 双条件达成——执行 CP-001 解除:删 TS6 包装器与 @typescript/native 别名,typescript 直改 ^7.1.0",
    );
    process.exit(0);
  }
  console.log(`[ts7-trigger] 未达成(issue 闭环=${r.issueClosed},TS7.1 稳定=${r.ts71Stable})`);
  process.exit(1);
}

// 仅直跑时执行 main();被测试 import 时不触发网络
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
