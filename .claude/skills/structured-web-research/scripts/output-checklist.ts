/**
 * 子代理输出自检清单（确定性——写入前逐条执行）
 *
 * 子代理在写入汇总文件前必须逐条核对此清单。
 * 这不是"建议"——是强制性检查，所有失败项必须在写入前修正。
 */

interface ChecklistItem {
  id: string;
  check: string;
  category: "source" | "accuracy" | "consistency" | "format";
}

const OUTPUT_CHECKLIST: ChecklistItem[] = [
  // 来源类
  { id: "S1", category: "source", check: "每条声称是否都附了来源 URL？" },
  { id: "S2", category: "source", check: "是否有虚构术语——在来源页面中逐词搜索确认该术语确实存在？" },
  { id: "S3", category: "source", check: "数字/版本号/日期是否标注了检索日期（防止过期）？" },

  // 准确性类
  { id: "A1", category: "accuracy", check: "环境变量/API 字段名/CLI 参数名等精确名称是否与官方文档逐字一致？" },
  { id: "A2", category: "accuracy", check: "\"30 个事件\"/\"5 种类型\"等数字声称是否可追溯到具体来源而非社区总结？" },
  { id: "A3", category: "accuracy", check: "GitHub issue 状态（Open/Closed）是否已通过直接访问 issue 页面确认？" },

  // 一致性类
  { id: "C1", category: "consistency", check: "同一概念在全文各处表述是否一致（如方向汇总和子文件之间）？" },
  { id: "C2", category: "consistency", check: "Markdown 表格是否连续、不被 blockquote/空行截断？" },
  { id: "C3", category: "consistency", check: "所有链接是否可达（非 404）？" },

  // 格式类
  { id: "F1", category: "format", check: "汇总文件是否以检索日期 + 来源优先级声明开头？" },
  { id: "F2", category: "format", check: "是否引用了所有子代理结果文件（如有）？" },
];

/**
 * 子代理写入前执行此函数。
 * 返回未通过的检查项列表。空列表 = 通过。
 */
function runChecklist(): ChecklistItem[] {
  const failures: ChecklistItem[] = [];
  for (const item of OUTPUT_CHECKLIST) {
    // 子代理逐条自问："我的输出满足这条吗？"
    // 不满足 → failures.push(item)
  }
  if (failures.length > 0) {
    console.error("输出自检失败，以下项目未通过:", failures.map(f => f.id));
    console.error("请在写入前修正所有失败项。");
  }
  return failures;
}
