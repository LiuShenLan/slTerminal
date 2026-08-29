// settingsCenter barrel —— 公共 API 出口
//
// 注意：本 barrel 不触发配置页注册 side-effect（注册触发点在 ./pages.ts，
// 由 SettingsPanel 显式 import——side-effect 注册，SC-FE-03/04）。

export type { SettingsPage, SettingsPageGroup, SettingsPageProps } from "./types";
export { getSettingsPageRegistry } from "./SettingsPageRegistry";
