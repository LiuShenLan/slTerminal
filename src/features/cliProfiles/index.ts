// cliProfiles barrel — 公共 API 出口
//
// 注意：本 barrel 不触发 profile 注册 side-effect（注册触发点在 ./profiles，
// 由 Workspace.tsx 显式 import，照 tabRules 先例）。缺省回退常量经
// "./profiles/claude" 直接引用（CLAUDE_CLI_ID）。

export type {
  CodingCliProfile,
  HooksCapability,
  HistoryCapability,
} from "./types";
export { CliProfileRegistry, cliProfileRegistry } from "./cliProfileRegistry";
