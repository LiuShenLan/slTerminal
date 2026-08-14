// profiles/claude/index.ts — claude CLI profile 身份域 + hooks/history 能力
//
// claude 合法领地：本文件是 claude 身份域数据定义点（id/displayName/commands/
// tabTitle 均引用 CLAUDE_CLI_ID 常量）+ 策略能力挂载点（hooks：MC-214 前端半 +
// MC-422；history：MC-315/316，实现均见 strategies.ts）。CLAUDE_CLI_ID 供通用层
// 缺省回退 import（AC-5 字面量守卫兼容——通用层禁写 "claude" 字面量，一律
// import 本常量）。
//
// 依赖方向合法化（KZ-1）：本文件 import panels/hooksConfig/ClaudeHooksConfigEditor
// （features/cliProfiles → panels 新方向）——profiles/claude/ 是 claude 合法领地，
// 编辑器组件是 claude 专属资产（MC-223 决策 2），hub 经本 profile 的 configEditor
// 字段分派渲染；types.ts 仅类型 import（运行期擦除）不构成运行循环。

import { cliProfileRegistry } from "../../cliProfileRegistry";
import type { CodingCliProfile } from "../../types";
import ClaudeHooksConfigEditor from "../../../../panels/hooksConfig/ClaudeHooksConfigEditor";
import {
  buildResumeCommand,
  buildRestoreInput,
  classifyNotification,
  computeUsagePercent,
  eventToStatus,
} from "./strategies";

/** claude cliId 公共键（通用层缺省回退一律 import 此常量，禁止写 "claude" 字面量） */
export const CLAUDE_CLI_ID = "claude";

/** context 用量信号事件名（statusline 桥接通道）——AC-5 守卫：claude 事件名字面量
 *  只允许出现在 profiles/claude/（claude 合法领地），通用层消费一律 import 本常量 */
export const CONTEXT_USAGE_EVENT = "ContextUsage";

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
      computeUsagePercent,
      restartHint: "hooks 改动需重启 claude 会话生效",
      hasConfigEditor: true,
      // KZ-1：hub 编辑器槽分派数据源——claude 专属编辑器（panels/hooksConfig/
      // ClaudeHooksConfigEditor，依赖方向合法化见本文件头注释）
      configEditor: ClaudeHooksConfigEditor,
      // KZ-4：hooks 配置分层声明（编辑器层切换器数据源）——三层值 + label/hint
      // 文案迁自 ClaudeHooksConfigEditor 退役 LAYERS 常量（claude 领地知识入 profile）
      configLayers: [
        { id: "user", label: "User", hint: "用户级（全局生效，优先级最低）" },
        { id: "project", label: "Project", hint: "项目级（当前项目生效）" },
        { id: "local", label: "Local", hint: "本地级（当前项目生效，优先级最高）" },
      ],
    },
    history: {
      supportsFork: true,
      buildResumeCommand,
      buildRestoreInput,
    },
  },
};

// side-effect 注册（import 即注册）
cliProfileRegistry.register(claudeProfile);
