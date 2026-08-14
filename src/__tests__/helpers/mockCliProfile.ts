// mockCliProfile.ts — mock CLI profile 测试夹具（AC-4 契约，spec 06 §7 + 决策 5）
//
// 跨边界契约（写死，见 docs/multi-cli/stages.md「跨边界契约」段）：
//   id "mockcli"、displayName "mockcli"、commands ["mockcli"]、tabTitle "mockcli"、
//   iconSrc "/cli-icons/mockcli.png"（Stage 01 已放资源，决策 5 真实最小 PNG）。
//   hooks 全能力：eventToStatus 恒等映射桩（任意事件恒 "working"，SessionEnd/Exit
//   除外返回 null——会话结束清图标语义）/ classifyNotification 桩（任意 payload 恒
//   "done"——通知类别可识别）/ computeUsagePercent 桩（任意 usage 恒 42——可识别
//   桩值，区别于 claude 官方口径取整钳位）/ restartHint 桩文案 /
//   hasConfigEditor=true / configEditor 桩组件（渲染可识别标记
//   data-e2e="mockcli-config-editor"，KZ-7 双向分派断言用）/ configLayers 单层桩声明
//   （hint 带 "mock" 可识别，区别于 claude 三层）。history 全能力：supportsFork=true /
//   buildResumeCommand / buildRestoreInput 桩输出带可识别前缀 "mockcli --resume"。
//
// 仅测试环境注册（AC-4）：registerMockCliProfile 供测试 beforeEach 调用，afterEach
// 经 resetCliProfileRegistry 清理（_reset + 恢复 claude 基线——_reset 后 side-effect
// 注册失效，claudeProfile 为模块级常量可显式补注册）。生产代码零引用本文件。

import { cliProfileRegistry } from "../../features/cliProfiles/cliProfileRegistry";
import { claudeProfile, CLAUDE_CLI_ID } from "../../features/cliProfiles/profiles/claude";
import type { CodingCliProfile } from "../../features/cliProfiles/types";
import type { HooksConfigEditorProps } from "../../features/cliProfiles/types";
import type { AgentHistorySession } from "../../types/agentHistory";
import { createElement } from "react";

/** mockcli 桩文案（hub 编辑器保存后提示条断言用，识别性区别于 claude 文案） */
export const MOCK_CLI_RESTART_HINT = "hooks 改动需重启 mockcli 会话生效";

/** mockcli 配置编辑器桩组件（KZ-7）：props 签名 = HooksConfigEditorProps（Stage 04
    契约 2），渲染可识别标记 data-e2e="mockcli-config-editor"——hub 双向分派断言用
    （选中 mockcli → 桩渲染；选中 claude → 桩标记不存在）。桩不消费
    onDirtyChange/askGuardRef（mockcli 无保存/守卫语义），profile.id 透传进文案
    证明渲染数据源来自 hub 选中态 */
const MockConfigEditor = ({ profile }: HooksConfigEditorProps) =>
  createElement(
    "div",
    { "data-e2e": "mockcli-config-editor" },
    `mockcli 配置编辑器桩（${profile.id}）`,
  );

/** mockcli profile 定义（AC-4 契约，跨边界写死） */
export const mockCliProfile: CodingCliProfile = {
  id: "mockcli",
  displayName: "mockcli",
  commands: ["mockcli"],
  iconSrc: "/cli-icons/mockcli.png",
  tabTitle: "mockcli",
  capabilities: {
    hooks: {
      // 恒等映射桩：任意事件恒返回 "working"（含未知事件，区别于 claude 未识别 → null）
      eventToStatus: (event, _notificationType) => {
        // 桩实现不消费 notificationType（恒等映射），显式引用规避 no-unused-vars
        void _notificationType;
        if (event === "SessionEnd" || event === "Exit") return null;
        return "working";
      },
      // 分类桩：任意 payload 恒 "done"（通知类别可识别，区别于 claude 五映射）
      classifyNotification: () => "done",
      // 百分比桩：任意 usage 恒 42（可识别桩值——区别于 claude 官方 used_percentage 取整钳位）
      computeUsagePercent: () => 42,
      restartHint: MOCK_CLI_RESTART_HINT,
      hasConfigEditor: true,
      // KZ-7：桩编辑器组件——hub 编辑器槽经本字段分派渲染（claude = ClaudeHooksConfigEditor）
      configEditor: MockConfigEditor,
      // KZ-7：单层桩声明（区别于 claude 三层——hint "mock" 可识别，证明层切换器
      // 数据源 = profile.configLayers）
      configLayers: [{ id: "user", label: "User", hint: "mock" }],
    },
    history: {
      supportsFork: true,
      // 桩输出带可识别前缀 "mockcli --resume"（契约示例形态）
      buildResumeCommand: (session: AgentHistorySession) =>
        `mockcli --resume ${session.sessionId}`,
      buildRestoreInput: (session: AgentHistorySession, opts: { fork: boolean }) =>
        `mockcli --resume ${session.sessionId}${
          opts.fork ? " --fork-session" : ""
        }\r`,
    },
  },
};

/** 注册 mockcli 到全局注册表（仅测试环境；claude 基线缺失时一并补注册） */
export function registerMockCliProfile(): void {
  if (!cliProfileRegistry.get(CLAUDE_CLI_ID)) {
    cliProfileRegistry.register(claudeProfile);
  }
  cliProfileRegistry.register(mockCliProfile);
}

/** afterEach 清理：清空注册表并恢复 claude 基线（全局单例隔离，照 cli-profile-claude 先例） */
export function resetCliProfileRegistry(): void {
  cliProfileRegistry._reset();
  cliProfileRegistry.register(claudeProfile);
}
