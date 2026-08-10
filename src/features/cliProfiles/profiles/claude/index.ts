// profiles/claude/index.ts — claude CLI profile 身份域 + hooks/history 能力
//
// claude 合法领地：本文件是 claude 身份域数据定义点（id/displayName/commands/
// tabTitle 均引用 CLAUDE_CLI_ID 常量）+ 策略能力挂载点（hooks：MC-214 前端半 +
// MC-422；history：MC-315/316，实现均见 strategies.ts）。CLAUDE_CLI_ID 供通用层
// 缺省回退 import（AC-5 字面量守卫兼容——通用层禁写 "claude" 字面量，一律
// import 本常量）。

import { cliProfileRegistry } from "../../cliProfileRegistry";
import type { CodingCliProfile } from "../../types";
import {
  buildResumeCommand,
  buildRestoreInput,
  classifyNotification,
  eventToStatus,
} from "./strategies";

/** claude cliId 公共键（通用层缺省回退一律 import 此常量，禁止写 "claude" 字面量） */
export const CLAUDE_CLI_ID = "claude";

// 会话生命周期事件名常量——AC-5 守卫：claude 事件名字面量（SessionEnd/Exit 等）
// 只允许出现在 profiles/claude/（claude 合法领地），通用层消费一律 import 本常量
// （参照 CLAUDE_CLI_ID 先例），禁止在通用层书写事件名字面量。

/** 会话结束事件名（SessionEnd → 清图标/删行分支判定用） */
export const SESSION_END_EVENT = "SessionEnd";

/** 会话退出事件名（Exit → 清会话分支判定用） */
export const EXIT_EVENT = "Exit";

/** claude profile 身份域定义（导出供测试 _reset 后重注册） */
export const claudeProfile: CodingCliProfile = {
  id: CLAUDE_CLI_ID,
  displayName: CLAUDE_CLI_ID,
  commands: [CLAUDE_CLI_ID],
  iconSrc: "/cli-icons/claude.png",
  tabTitle: CLAUDE_CLI_ID,
  capabilities: {
    hooks: {
      eventToStatus,
      classifyNotification,
      contextLimit: 200_000,
      restartHint: "hooks 改动需重启 claude 会话生效",
      hasConfigEditor: true,
    },
    history: {
      supportsFork: true,
      buildResumeCommand,
      buildRestoreInput,
    },
  },
};

// side-effect 注册（import 即注册，照 tabRules 先例）
cliProfileRegistry.register(claudeProfile);
