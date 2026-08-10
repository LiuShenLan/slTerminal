// profiles/claude/index.ts — claude CLI profile 身份域（Stage 01 注册）
//
// claude 合法领地：本文件是 claude 身份域数据定义点（id/displayName/commands/
// tabTitle 均引用 CLAUDE_CLI_ID 常量）。CLAUDE_CLI_ID 供通用层缺省回退 import
// （AC-5 字面量守卫兼容——通用层禁写 "claude" 字面量，一律 import 本常量）。
// capabilities 本 Stage 为空（hooks 能力 Stage 02 迁入、history 能力 Stage 05 迁入）。

import { cliProfileRegistry } from "../../cliProfileRegistry";
import type { CodingCliProfile } from "../../types";

/** claude cliId 公共键（通用层缺省回退一律 import 此常量，禁止写 "claude" 字面量） */
export const CLAUDE_CLI_ID = "claude";

/** claude profile 身份域定义（导出供测试 _reset 后重注册） */
export const claudeProfile: CodingCliProfile = {
  id: CLAUDE_CLI_ID,
  displayName: CLAUDE_CLI_ID,
  commands: [CLAUDE_CLI_ID],
  iconSrc: "/cli-icons/claude.png",
  tabTitle: CLAUDE_CLI_ID,
  capabilities: {},
};

// side-effect 注册（import 即注册，照 tabRules 先例）
cliProfileRegistry.register(claudeProfile);
