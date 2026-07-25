#!/usr/bin/env node
/**
 * 深度分层（确定性，可执行）
 *
 * 输入：方向数量 N（命令行参数）
 * 输出：stdout JSON { minSources, fetchTopN }
 *
 * 规则：方向数 → 参数，纯映射，无任何启发式。
 *   N <= 2 → { minSources: 3, fetchTopN: 3 }
 *   N <= 5 → { minSources: 5, fetchTopN: 5 }
 *   N >  5 → { minSources: 8, fetchTopN: 8 }
 *
 * 用法：node depth-tier.mjs <方向数>
 */

const n = Number.parseInt(process.argv[2], 10);

if (!Number.isFinite(n) || n < 1) {
  console.error("用法: node depth-tier.mjs <方向数>（正整数）");
  process.exit(1);
}

/** 方向数 → 检索深度配置（纯函数） */
function depthTier(directionCount) {
  if (directionCount <= 2) return { minSources: 3, fetchTopN: 3 };
  if (directionCount <= 5) return { minSources: 5, fetchTopN: 5 };
  return { minSources: 8, fetchTopN: 8 };
}

process.stdout.write(JSON.stringify(depthTier(n)) + "\n");
