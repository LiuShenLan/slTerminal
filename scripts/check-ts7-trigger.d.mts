// check-ts7-trigger.mjs 的类型声明(CP-001 执行期补充)——src/__tests__/deps-ts7-trigger.test.ts
// import 本脚本经 tsc 门禁需可解析类型(.mjs 无声明即 TS7016);签名与 mjs 内实现保持同步。
// 若后续修改 mjs 的导出形态(evaluateTrigger 的入参/返回结构),此处一并更新。

export interface EvaluateTriggerResult {
  issueClosed: boolean;
  ts71Stable: boolean;
  triggered: boolean;
}

export function evaluateTrigger(
  issueState: string | undefined,
  latestVersion: string | undefined,
): EvaluateTriggerResult;
