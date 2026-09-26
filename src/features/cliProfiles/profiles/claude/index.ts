// profiles/claude/index.ts — claude CLI profile 身份域 + hooks/history 能力
//
// claude 合法领地：本文件是 claude 身份域数据定义点（id/displayName/commands/
// tabTitle 均引用 CLAUDE_CLI_ID 常量）+ 策略能力挂载点（hooks：MC-214 前端半 +
// MC-422；history：MC-315/316，实现均见 strategies.ts）。CLAUDE_CLI_ID 供通用层
// 缺省回退 import（AC-5 字面量守卫兼容——通用层禁写 "claude" 字面量，一律
// import 本常量）。
//
// 依赖方向合法化（KZ-1，SC-FE-05 迁移后重写）：本文件 import ./configEditor/
// ClaudeHooksConfigEditor（configEditor/ 归域于 profiles/claude/ 领地内，SC-FE-05）——
// 编辑器组件是 claude 专属资产（MC-223 决策 2），hub 经本 profile 的 configEditor
// 字段分派渲染；不再跨 panels 引用，无新依赖方向；types.ts 仅类型 import（运行期
// 擦除）不构成运行循环。

import { cliProfileRegistry } from "../../cliProfileRegistry";
import type { CodingCliProfile } from "../../types";
import ClaudeHooksConfigEditor from "./configEditor/ClaudeHooksConfigEditor";
import {
  buildResumeCommand,
  buildRestoreInput,
  classifyNotification,
  computeUsagePercent,
  eventToStatus,
} from "./strategies";

/** claude cliId 公共键（通用层缺省回退一律 import 此常量，禁止写 "claude" 字面量） */
export const CLAUDE_CLI_ID = "claude";

/** claude 全局配置目录的运行时文件名单（globalFiles 能力域，AC-5 领地单点常量）：
 *  缓存/会话/日志等无配置价值的官方运行时产物——「Agent 全局文件」视图「全部」模式
 *  默认排除；「显示运行时文件」开关开启后展示并监听。随 claude 版本演进补齐。
 *  保留展示（来源/价值不确定或属配置资产，宁缺毋滥）：backups、daemon\*、jobs、tasks、
 *  plans、agent-memory、plugins、agents、skills、CLAUDE.md、settings.json 等不在此列 */
export const CLAUDE_RUNTIME_PATHS: string[] = [
  "projects",
  "sessions",
  "shell-snapshots",
  "history.jsonl",
  "telemetry",
  "stats-cache.json",
  "ide",
  "paste-cache",
  "cache",
  "file-history",
  "session-env",
];

/** context 用量信号事件名（statusline 桥接通道）——AC-5 守卫：claude 事件名字面量
 *  只允许出现在 profiles/claude/（claude 合法领地），通用层消费一律 import 本常量 */
export const CONTEXT_USAGE_EVENT = "ContextUsage";

// 会话生命周期事件名常量——AC-5 守卫：claude 事件名字面量（SessionEnd/Exit 等）
// 只允许出现在 profiles/claude/（claude 合法领地），通用层消费一律 import 本常量
// （参照 CLAUDE_CLI_ID 先例），禁止在通用层书写事件名字面量。

/** 会话开始事件名（B13：SessionStart → /resume 后页签标题按 profile.tabTitle 重设） */
export const SESSION_START_EVENT = "SessionStart";

/** 会话结束事件名（SessionEnd → 清图标/删行分支判定用） */
export const SESSION_END_EVENT = "SessionEnd";

/** 会话退出事件名（Exit → 清会话分支判定用） */
export const EXIT_EVENT = "Exit";

/** claude profile 身份域定义（导出供测试 _reset 后重注册） */
export const claudeProfile: CodingCliProfile = {
  id: CLAUDE_CLI_ID,
  displayName: "Claude Code",
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
      // KZ-1：hub 编辑器槽分派数据源——claude 专属编辑器（configEditor/
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
    // 「Agent 全局文件」侧栏视图能力：配置目录 home 相对路径 + 运行时文件名单
    // （沙箱放行真值源在后端 agent_dirs.rs 静态表——ADR-0024，此处仅能力声明）
    globalFiles: {
      configDir: ".claude",
      runtimePaths: CLAUDE_RUNTIME_PATHS,
    },
  },
};

// side-effect 注册（import 即注册）
cliProfileRegistry.register(claudeProfile);
