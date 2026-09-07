// sync-startup-colors.mjs 的类型声明(CP-027 执行期补充)——src/__tests__/startup-colors-sync.test.ts
// import 本脚本经 tsc 门禁需可解析类型(.mjs 无声明即 TS7016);签名与 mjs 内实现保持同步。
// 若后续修改 mjs 的导出形态(extractStartupColors/renderStartupColorsModule),此处一并更新。

export interface StartupColors {
  bg: string;
  fg: string;
  errorFg: string;
}

export function extractStartupColors(linearTs: string): StartupColors;

export function renderStartupColorsModule(colors: StartupColors): string;
