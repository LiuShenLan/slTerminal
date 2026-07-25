#!/usr/bin/env node
/**
 * Review 文件骨架生成（确定性模板，可执行）
 *
 * 输入：命令行参数 <源文件路径> [方向编号]
 * 输出：stdout review 文件骨架 markdown（头部 + 日期 + 约定说明）
 *
 * 子代理在写入 review 文件前调用本脚本生成头部，然后逐条追加错误条目
 * （错误条目格式见 output-spec.md）。
 *
 * 用法：node review-template.mjs "docs/research/x/D1/01-hooks.md" D1
 */

const [sourcePath, direction] = process.argv.slice(2);

if (!sourcePath) {
  console.error('用法: node review-template.mjs <源文件路径> [方向编号]');
  process.exit(1);
}

const basename = sourcePath.replace(/\\/g, "/").split("/").pop().replace(/\.md$/, "");
const date = new Date().toISOString().slice(0, 10);

const lines = [
  `# ${basename} 事实核查报告`,
  "",
  `> 核查日期: ${date}`,
  `> 核查方式: WebSearch + WebFetch 外部验证 + 内部交叉比对`,
  `> 核查范围: 全量声称逐一核实`,
  direction ? `> 方向: ${direction}` : null,
  "",
  "---",
  "",
  "<!-- 逐条追加错误条目，格式见 output-spec.md「错误条目模板」。",
  "     全部正确则写: 未发现错误（已验证 N 项声称）。 -->",
  "",
].filter((l) => l !== null);

process.stdout.write(lines.join("\n"));
