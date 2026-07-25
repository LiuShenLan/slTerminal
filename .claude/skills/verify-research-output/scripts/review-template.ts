/**
 * Review 输出模板生成（伪代码）
 *
 * 输入：源文件信息 + 方向编号
 * 输出：Review 文件的 Markdown 骨架
 *
 * 子代理在开始验证前调用此函数生成输出文件头部，
 * 然后逐条追加发现的错误。
 */

interface SourceFile {
  /** 文件路径（相对于检索输出根目录） */
  path: string;
  /** 方向编号，如 "D1" */
  direction: string;
}

interface ReviewTemplate {
  /** 输出文件路径 */
  outputPath: string;
  /** Markdown 内容 */
  content: string;
}

// 生成 review 文件路径——确定性
function reviewOutputPath(source: SourceFile): string {
  // 规则: review/{direction}/{basename}-review.md
  const parts = source.path.replace(/\\/g, "/").split("/");
  const basename = parts[parts.length - 1]!.replace(/\.md$/, "");
  return `review/${source.direction}/${basename}-review.md`;
}

// 生成 review 文件头部——确定性
function generateHeader(source: SourceFile): string {
  const date = new Date().toISOString().slice(0, 10);
  return [
    `# ${source.path.split("/").pop()!.replace(".md", "")} 事实核查报告`,
    "",
    `> 核查日期: ${date}`,
    `> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对`,
    `> 核查范围: 全量声称逐一核实`,
    "",
    "---",
    "",
  ].join("\n");
}

// 生成错误条目——确定性模板，AI 填充内容
function generateErrorEntry(params: {
  index: number;
  summary: string;
  fileLine: string;
  originalClaim: string;
  errorType: string;
  correctInfo: string;
  counterSource: string;
}): string {
  return [
    `## 错误 ${params.index}: ${params.summary}`,
    "",
    `- **文件+行号**: \`${params.fileLine}\``,
    `- **原声称**: ${params.originalClaim}`,
    `- **错误类型**: ${params.errorType}`,
    `- **正确信息**: ${params.correctInfo}`,
    `- **反证来源**: ${params.counterSource}`,
    "",
  ].join("\n");
}

// 生成无错误声明
function generateNoErrors(claimsVerified: number): string {
  return `未发现错误（已验证 ${claimsVerified} 项声称）。\n`;
}

// 主入口
function createReviewTemplate(source: SourceFile): ReviewTemplate {
  return {
    outputPath: reviewOutputPath(source),
    content: generateHeader(source),
  };
}

export { createReviewTemplate, generateErrorEntry, generateNoErrors };
export type { SourceFile, ReviewTemplate };
