/**
 * 交叉验证冲突检测算法（伪代码）
 *
 * 输入：多个方向的 review 文件列表
 * 输出：矛盾矩阵——按声称主题分组的冲突方向
 *
 * 阶段 2 的裁决 agent 用此函数识别所有跨方向矛盾。
 */

interface ReviewFile {
  direction: string;
  path: string;
}

interface ClaimRecord {
  /** 声称的唯一键（规范化后的主题文本） */
  claimKey: string;
  /** 声称的主题类别 */
  category: string;
  /** 声称者（哪个方向） */
  direction: string;
  /** 声称的具体值 */
  value: string;
  /** 声称所在源文件 */
  sourceFile: string;
}

interface ConflictGroup {
  /** 声称主题 */
  claimKey: string;
  /** 矛盾的方向列表——每个方向声称了不同的值 */
  conflictingDirections: ClaimRecord[];
  /** 冲突类型 */
  kind: "value_mismatch" | "boolean_inversion" | "count_discrepancy";
}

// 规范化声称文本为比较键——确定性
function normalizeClaimKey(text: string): string {
  return text
    .toLowerCase()
    .replace(/\s+/g, " ")
    .replace(/[0-9]+/g, "N")  // 数字归一化——"30 个事件"和"27 个事件"归类为同键
    .replace(/[""]/g, '"')
    .trim();
}

// 提取声称类别——确定性规则
function extractCategory(claimKey: string): string {
  const patterns: [RegExp, string][] = [
    [/配置|层级|优先级|settings|config/, "配置结构"],
    [/事件|event|hook/, "事件/API 定义"],
    [/环境变量|env\s*var/, "环境变量"],
    [/版本|version/, "版本信息"],
    [/状态|status|open|closed/, "状态信息"],
    [/数量|count|number|个|种/, "数量统计"],
  ];

  for (const [pattern, category] of patterns) {
    if (pattern.test(claimKey)) {
      return category;
    }
  }
  return "其他";
}

// 主冲突检测——确定性算法
function detectConflicts(claims: ClaimRecord[]): ConflictGroup[] {
  // 步骤 1: 按 claimKey 分组
  const groups = new Map<string, ClaimRecord[]>();
  for (const claim of claims) {
    const key = normalizeClaimKey(claim.value);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(claim);
  }

  // 步骤 2: 过滤出有矛盾的分组（同键下不同方向的值不同）
  const conflicts: ConflictGroup[] = [];
  for (const [key, group] of groups) {
    if (group.length < 2) continue;  // 只有一个方向提过，无矛盾

    // 提取所有不同的值
    const uniqueValues = new Set(group.map(c => c.value));
    if (uniqueValues.size <= 1) continue;  // 所有方向说的一样，无矛盾

    // 判定冲突类型
    const kind = classifyConflictKind(group);

    conflicts.push({
      claimKey: key,
      conflictingDirections: group,
      kind,
    });
  }

  return conflicts;
}

// 冲突类型判定——确定性
function classifyConflictKind(
  group: ClaimRecord[]
): ConflictGroup["kind"] {
  const values = group.map(c => c.value.toLowerCase());

  // 布尔反转——一个说"是"，另一个说"否"
  if (values.some(v => v.includes("是") || v.includes("yes") || v.includes("true")) &&
      values.some(v => v.includes("否") || v.includes("no") || v.includes("false"))) {
    return "boolean_inversion";
  }

  // 数量差异——值中包含不同数字
  const numbers = values.map(v => v.match(/\d+/g)?.map(Number) ?? []);
  const flatNumbers = numbers.flat();
  if (flatNumbers.length >= 2 && new Set(flatNumbers).size > 1) {
    return "count_discrepancy";
  }

  return "value_mismatch";
}

export { detectConflicts, normalizeClaimKey, extractCategory, classifyConflictKind };
export type { ReviewFile, ClaimRecord, ConflictGroup };
