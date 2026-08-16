// tauri-plugin-dialog 薄封装——save/open() 自身就是 invoke() 包装器，走相同 IPC 通道
// 聚合到此以遵守架构约束 #1：invoke 只出现在 src/ipc/
//
// ask 已随 Stage 07 浮层统一（OV-02）删除：确认语义改经 src/lib 的 confirmDialog
// （统一浮层 UI-801/803），文件对话框（open/save）为原生保留。
export { save, open } from "@tauri-apps/plugin-dialog";
