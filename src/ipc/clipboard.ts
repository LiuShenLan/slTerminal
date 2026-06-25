// 剪贴板 IPC 封装 — 前端调用 clipboard 命令的唯一入口
// @tauri-apps/plugin-* 只允许在本目录出现（硬约束 #1）

export { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";
