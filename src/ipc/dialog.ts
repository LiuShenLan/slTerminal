// tauri-plugin-dialog 薄封装——save/open() 自身就是 invoke() 包装器，走相同 IPC 通道
// 聚合到此以遵守架构约束 #1：invoke 只出现在 src/ipc/
export { save, open } from "@tauri-apps/plugin-dialog";
