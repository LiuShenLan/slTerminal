#!/usr/bin/env node
/**
 * 跨文件冲突检测（确定性，可执行）
 *
 * 输入：stdin 读 claims JSON（由 AI 用语义理解从文件中提取）
 *   [{ "direction": "D1", "file": "a.md", "line": 12, "field": "事件总数", "value": "30 个事件", "source": "https://..." }]
 * 输出：stdout JSON ConflictGroup[]
 *   [{ "claimKey": "...", "kind": "boolean_inversion|count_discrepancy|value_mismatch", "claims": [...] }]
 *
 * 确定性部分：规范化比较键 → 按键分组 → 同键不同值即冲突 → 冲突类型分类。
 * AI 负责的部分（不在本脚本）：提取 claims、裁决冲突。
 *
 * 用法：cat claims.json | node detect-conflicts.mjs
 */

/** 规范化声称文本为比较键——数字归一化，"30 个"与"27 个"归为同键 */
function normalizeClaimKey(text) {
  return String(text)
    .toLowerCase()
    .replace(/\s+/g, " ")
    .replace(/[0-9]+/g, "N")
    .replace(/[“”]/g, '"')
    .trim();
}

/**
 * 分组键：优先用 field（AI 给同一事实点的各方向声称赋予相同 field 名），
 * field 缺失时回退 value 归一化键。
 * 注意：必须用 field 分组——若用 value 派生键，含"是"与含"否"的句子
 * 归一化后必然不同键，布尔反转永不触发（原伪代码的死分支 bug）。
 */
function groupKey(claim) {
  const field = String(claim.field ?? "").trim();
  return field !== "" ? `field:${field.toLowerCase()}` : `value:${normalizeClaimKey(claim.value)}`;
}

/** 冲突类型判定（纯函数） */
function classifyConflictKind(group) {
  const values = group.map((c) => String(c.value).toLowerCase());

  // 布尔反转——一方说"是"，另一方说"否"（保守词表，防"可见性"类误匹配）
  const hasYes = values.some((v) => v.includes("是") || v.includes("yes") || v.includes("true"));
  const hasNo = values.some((v) => v.includes("否") || v.includes("no") || v.includes("false"));
  if (hasYes && hasNo) return "boolean_inversion";

  // 数量差异——值中包含不同数字
  const numbers = values.flatMap((v) => v.match(/\d+/g)?.map(Number) ?? []);
  if (numbers.length >= 2 && new Set(numbers).size > 1) return "count_discrepancy";

  return "value_mismatch";
}

/** 主检测：按分组键聚类，同组多值即冲突 */
function detectConflicts(claims) {
  const groups = new Map();
  for (const claim of claims) {
    const key = groupKey(claim);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(claim);
  }

  const conflicts = [];
  for (const [key, group] of groups) {
    if (group.length < 2) continue;                       // 单方向提及，无矛盾
    const uniqueValues = new Set(group.map((c) => String(c.value)));
    if (uniqueValues.size <= 1) continue;                 // 说法一致，无矛盾

    conflicts.push({
      claimKey: key,
      kind: classifyConflictKind(group),
      claims: group,
    });
  }
  return conflicts;
}

// 读取 stdin
let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => {
  let claims;
  try {
    claims = JSON.parse(input);
  } catch {
    console.error('输入不是合法 JSON。期望: [{"direction":"D1","file":"a.md","line":1,"field":"...","value":"...","source":"..."}]');
    process.exit(1);
  }
  if (!Array.isArray(claims)) {
    console.error("输入须为数组。");
    process.exit(1);
  }
  process.stdout.write(JSON.stringify(detectConflicts(claims), null, 2) + "\n");
});
