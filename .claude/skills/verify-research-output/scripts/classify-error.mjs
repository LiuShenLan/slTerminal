#!/usr/bin/env node
/**
 * 错误分类（确定性规则映射，可执行）
 *
 * 输入：stdin 读 JSON
 *   { "kind": "<反证类型>", "correctInfo": "<正确信息>", "citedSource": "<原引用URL>" }
 *   kind 枚举：url_inaccessible | url_content_mismatch | official_doc_differs
 *             | other_file_contradicts | self_contradiction | outdated
 * 输出：stdout JSON { type, severity, reason }
 *   type: 事实错误 | 来源不支撑 | 过时歪曲 | 内部矛盾
 *   severity: P0 | P1 | P2
 *
 * AI 负责的部分（不在本脚本）：判断反证属于哪种 kind、提供 correctInfo 文本。
 * 本脚本只做 kind → 分类/严重程度的确定性映射。
 *
 * 用法：echo '{"kind":"url_inaccessible","correctInfo":"...","citedSource":"..."}' | node classify-error.mjs
 */

/** 严重程度判定——确定性规则 */
function assessSeverity(errorType, evidence) {
  // P0: 影响决策——虚构内容、不存在的 API、完全反向
  if (
    evidence.kind === "official_doc_differs" &&
    /不存在|虚构|完全相反/.test(evidence.correctInfo)
  ) {
    return "P0";
  }
  if (evidence.kind === "url_content_mismatch" && /零出现/.test(evidence.correctInfo)) {
    return "P0";
  }
  // P1: 来源不支撑或过时
  if (errorType === "来源不支撑" || errorType === "过时歪曲") return "P1";
  // P2: 内部矛盾或轻微不准确
  return "P2";
}

/** 主分类——确定性规则映射 */
function classify(evidence) {
  switch (evidence.kind) {
    case "url_inaccessible":
      return { type: "来源不支撑", severity: "P1", reason: `引用 URL 不可访问: ${evidence.citedSource ?? ""}` };

    case "url_content_mismatch":
      return { type: "来源不支撑", severity: assessSeverity("来源不支撑", evidence), reason: `引用内容不支撑声称: ${evidence.correctInfo}` };

    case "official_doc_differs":
      // 值曾正确但已更新 → 过时；否则 → 事实错误
      if (/已弃用|已更名|已关闭|已移除/.test(evidence.correctInfo)) {
        return { type: "过时歪曲", severity: assessSeverity("过时歪曲", evidence), reason: `信息已过时: ${evidence.correctInfo}` };
      }
      return { type: "事实错误", severity: assessSeverity("事实错误", evidence), reason: `与官方文档不符: ${evidence.correctInfo}` };

    case "other_file_contradicts":
      return { type: "内部矛盾", severity: "P2", reason: `与另一文件矛盾: ${evidence.correctInfo}` };

    case "self_contradiction":
      return { type: "内部矛盾", severity: "P2", reason: `文件内部自相矛盾: ${evidence.correctInfo}` };

    case "outdated":
    default:
      return { type: "过时歪曲", severity: "P1", reason: evidence.correctInfo };
  }
}

const VALID_KINDS = [
  "url_inaccessible", "url_content_mismatch", "official_doc_differs",
  "other_file_contradicts", "self_contradiction", "outdated",
];

// 读取 stdin
let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => {
  let evidence;
  try {
    evidence = JSON.parse(input);
  } catch {
    console.error('输入不是合法 JSON。期望: {"kind":"...","correctInfo":"...","citedSource":"..."}');
    process.exit(1);
  }
  if (!VALID_KINDS.includes(evidence.kind)) {
    console.error(`kind 须为: ${VALID_KINDS.join(" | ")}`);
    process.exit(1);
  }
  if (typeof evidence.correctInfo !== "string" || evidence.correctInfo.length === 0) {
    console.error("correctInfo 须为非空字符串。");
    process.exit(1);
  }
  process.stdout.write(JSON.stringify(classify(evidence), null, 2) + "\n");
});
