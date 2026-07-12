// shell — 封装 @tauri-apps/plugin-opener 的 openUrl
//
// 架构约束 #1：前端不直接 import Tauri 插件，必须经 IPC 层。
// 用于 OSC 8 超链接点击等场景打开系统默认浏览器。

import { openUrl } from "@tauri-apps/plugin-opener";

export { openUrl };
