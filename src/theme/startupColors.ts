// startupColors.ts — 启动链 fail-safe 静态色(构建期生成物,勿手改)
// 色源单一 = schemes/linear.ts(appBgPrimary/sidebarFg/errorFg)——生成器 scripts/sync-startup-colors.mjs(CP-027),
// 经 package.json predev/prebuild 接线;改 linear.ts 三槽位后跑 npm run sync:startup-colors 即同步三处消费点。
export const STARTUP_FAIL_SAFE_BG = "#0a0a0b";
export const STARTUP_FAIL_SAFE_FG = "#ece9e4";
export const STARTUP_FAIL_SAFE_ERROR_FG = "#d9706b";
