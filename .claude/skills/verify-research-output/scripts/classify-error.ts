/**
 * 错误分类算法（伪代码）
 *
 * 输入：声称文本 + 反证信息 + 上下文
 * 输出：错误类型标签
 *
 * 此函数是确定性的——同样的输入永远产生同样的分类。
 * AI 仅负责提供"声称"和"反证"的语义内容。
 */

interface Claim {
  /** 文档中的原文表述 */
  text: string;
  /** 声称所在的文件路径 */
  sourceFile: string;
  /** 声称引用的来源 URL */
  citedSource: string;
}

interface CounterEvidence {
  /** 反证类型 */
  kind: "url_inaccessible" | "url_content_mismatch" | "official_doc_differs"
      | "other_file_contradicts" | "self_contradiction" | "outdated";
  /** 正确的信息 */
  correctInfo: string;
  /** 反证来源 URL */
  source: string;
}

type ErrorType = "事实错误" | "来源不支撑" | "过时歪曲" | "内部矛盾";
type Severity = "P0" | "P1" | "P2";

interface Classification {
  type: ErrorType;
  severity: Severity;
  reason: string;
}

// 严重程度判定——确定性规则
function assessSeverity(errorType: ErrorType, evidence: CounterEvidence): Severity {
  // P0: 影响架构决策——虚构的环境变量、不存在的 API、配置层级完全反向
  if (evidence.kind === "official_doc_differs" &&
      (evidence.correctInfo.includes("不存在") ||
       evidence.correctInfo.includes("虚构") ||
       evidence.correctInfo.includes("完全相反"))) {
    return "P0";
  }

  // P0: 虚构内容——来源中不存在的术语
  if (evidence.kind === "url_content_mismatch" &&
      evidence.correctInfo.includes("零出现")) {
    return "P0";
  }

  // P1: 来源不支撑或过时
  if (errorType === "来源不支撑" || errorType === "过时歪曲") {
    return "P1";
  }

  // P2: 内部矛盾或轻微不准确
  if (errorType === "内部矛盾") {
    return "P2";
  }

  return "P2";
}

// 主分类函数——确定性
function classify(claim: Claim, evidence: CounterEvidence): Classification {
  // 规则 1: URL 不可访问 → 来源不支撑
  if (evidence.kind === "url_inaccessible") {
    return {
      type: "来源不支撑",
      severity: "P1",
      reason: `引用 URL 不可访问: ${claim.citedSource}`,
    };
  }

  // 规则 2: URL 可访问但内容不符 → 来源不支撑
  if (evidence.kind === "url_content_mismatch") {
    return {
      type: "来源不支撑",
      severity: assessSeverity("来源不支撑", evidence),
      reason: `引用内容不支撑声称: ${evidence.correctInfo}`,
    };
  }

  // 规则 3: 官方文档明确不同 → 事实错误（若声称了错误的具体值）
  //         或过时歪曲（若声称的值之前正确但已更新）
  if (evidence.kind === "official_doc_differs") {
    if (evidence.correctInfo.includes("已弃用") ||
        evidence.correctInfo.includes("已更名") ||
        evidence.correctInfo.includes("已关闭")) {
      return {
        type: "过时歪曲",
        severity: assessSeverity("过时歪曲", evidence),
        reason: `信息已过时: ${evidence.correctInfo}`,
      };
    }
    return {
      type: "事实错误",
      severity: assessSeverity("事实错误", evidence),
      reason: `与官方文档不符: ${evidence.correctInfo}`,
    };
  }

  // 规则 4: 其他文件矛盾 → 内部矛盾
  if (evidence.kind === "other_file_contradicts") {
    return {
      type: "内部矛盾",
      severity: "P2",
      reason: `与另一文件矛盾: ${evidence.correctInfo}`,
    };
  }

  // 规则 5: 同一文件内部自相矛盾
  if (evidence.kind === "self_contradiction") {
    return {
      type: "内部矛盾",
      severity: "P2",
      reason: `文件内部自相矛盾: ${evidence.correctInfo}`,
    };
  }

  // 默认: 过时
  return {
    type: "过时歪曲",
    severity: "P1",
    reason: evidence.correctInfo,
  };
}

export { classify, assessSeverity };
export type { Claim, CounterEvidence, ErrorType, Severity, Classification };
