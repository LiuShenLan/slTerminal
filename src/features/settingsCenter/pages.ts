// pages.ts —— 配置页注册触发点（F11）
//
// side-effect import 注册：SettingsPanel 顶部显式 import 本文件即完成全部配置页注册
// （硬约束 #13：注册经 side-effect import 触发，禁止隐式初始化）。
// 现行注册三页：global 组 = 快捷键 / 后台定时任务，project 组 = Hooks 配置。
// 新增配置页 = 在下方追加一条 register 调用 + 在 SettingsPanel 的 import 链中保持本文件被引用。

import { getSettingsPageRegistry } from "./SettingsPageRegistry";
import BackgroundTasksPage from "../../panels/settings/pages/BackgroundTasksPage";
import HooksSettingsPage from "../../panels/settings/pages/HooksSettingsPage";
import KeybindingsPage from "../../panels/settings/pages/KeybindingsPage";

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

// hooks 配置页（hooks 双模式面板 hub 迁入设置中心，SC-FE-05）——project 组（需项目上下文）
getSettingsPageRegistry().register({
  id: "hooks",
  title: "Hooks 配置",
  group: "project",
  component: HooksSettingsPage,
  order: 100,
});
