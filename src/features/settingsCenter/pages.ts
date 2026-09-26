// pages.ts —— 配置页注册触发点（F11；ADR-0023 全局/Agent 二分）
//
// side-effect import 注册：SettingsPanel 顶部显式 import 本文件即完成全部配置页注册
// （硬约束 #13：注册经 side-effect import 触发，禁止隐式初始化）。
// 现行注册：global 组 = 快捷键 / 后台定时任务 / 终端输入模式；
// agent 组 = 遍历 cliProfileRegistry 逐 profile 注册 `agent.<cliId>.basic`（恒有）
// 与 `agent.<cliId>.hooks`（hasConfigEditor 门控），closure 绑定 cliId。
// 新增全局配置页 = 在下方追加一条 register 调用；agent 页由注册表枚举自动出现。
//
// 时序红线：agent 页枚举依赖 cliProfileRegistry 注册完成——本文件显式 import
// profiles（幂等 side-effect），消除「SettingsPanel import 链与 Workspace 注册
// 触发点谁先谁后」的隐式时序依赖。

import "../../features/cliProfiles/profiles";
import React from "react";
import { getSettingsPageRegistry } from "./SettingsPageRegistry";
import { cliProfileRegistry } from "../../features/cliProfiles/cliProfileRegistry";
import BackgroundTasksPage from "../../panels/settings/pages/BackgroundTasksPage";
import ConptyInputModesPage from "../../panels/settings/pages/ConptyInputModesPage";
import KeybindingsPage from "../../panels/settings/pages/KeybindingsPage";
import AgentBasicPage from "../../panels/settings/pages/AgentBasicPage";
import AgentHooksPage from "../../panels/settings/pages/AgentHooksPage";
import type { SettingsPageProps } from "./types";

// 快捷键页（F11，SC-FE-09）——global 组（应用级单例）
getSettingsPageRegistry().register({
  id: "keybindings",
  title: "快捷键",
  group: "global",
  component: KeybindingsPage,
  order: 10,
});

// 后台定时任务页（F12：套餐余量/会话刷新统一配置）——global 组（应用级单例）
getSettingsPageRegistry().register({
  id: "backgroundTasks",
  title: "后台定时任务",
  group: "global",
  component: BackgroundTasksPage,
  order: 20,
});

// 终端输入模式页（CP-009：ConPTY 输入模式能力矩阵四开关）——global 组（应用级单例）
getSettingsPageRegistry().register({
  id: "conptyInputModes",
  title: "终端输入模式",
  group: "global",
  component: ConptyInputModesPage,
  order: 25,
});

// Agent 组（ADR-0023）：逐 profile 注册基础配置页（恒有）与 hooks 配置页（门控）。
// closure 绑定 cliId——页组件形态 = SettingsPageProps & { cliId }，注册表条目
// 组件为单参 closure（壳按 SettingsPageProps 渲染，cliId 由 closure 注入）。
// 幂等（register 同 id 覆盖）：顶层 side-effect 调用一次覆盖静态 profile；
// 动态注册的 profile（E2E mockcli 夹具）经再次调用本函数补注册。
export function syncAgentPagesFromProfiles(): void {
  for (const profile of cliProfileRegistry.getAll()) {
    const cliId = profile.id;

    const BasicPage: React.FC<SettingsPageProps> = (props) =>
      React.createElement(AgentBasicPage, { ...props, cliId });
    getSettingsPageRegistry().register({
      id: `agent.${cliId}.basic`,
      title: "基础配置",
      group: "agent",
      cliId,
      component: BasicPage,
    });

    if (profile.capabilities.hooks?.hasConfigEditor === true) {
      const HooksPage: React.FC<SettingsPageProps> = (props) =>
        React.createElement(AgentHooksPage, { ...props, cliId });
      getSettingsPageRegistry().register({
        id: `agent.${cliId}.hooks`,
        title: "Hooks 配置",
        group: "agent",
        cliId,
        component: HooksPage,
      });
    }
  }
}

syncAgentPagesFromProfiles();
