// pages.ts —— 配置页注册触发点（F11）
//
// side-effect import 注册：SettingsPanel 顶部显式 import 本文件即完成全部配置页注册
// （硬约束 #13：注册经 side-effect import 触发，禁止隐式初始化）。
// Stage 02 仅注册 planBalance 一页；Stage 03（hooks）/ Stage 04（keybindings）追加。
// 新增配置页 = 在下方追加一条 register 调用 + 在 SettingsPanel 的 import 链中保持本文件被引用。

import { getSettingsPageRegistry } from "./SettingsPageRegistry";
import PlanBalancePage from "../../panels/settings/pages/PlanBalancePage";
import HooksSettingsPage from "../../panels/settings/pages/HooksSettingsPage";

getSettingsPageRegistry().register({
  id: "planBalance",
  title: "套餐余量",
  group: "global",
  component: PlanBalancePage,
  order: 20,
});

// hooks 配置页（F6 hub 迁入设置中心，SC-FE-05）——project 组（需项目上下文）
getSettingsPageRegistry().register({
  id: "hooks",
  title: "Hooks 配置",
  group: "project",
  component: HooksSettingsPage,
  order: 100,
});
