/**
 * IPC 通信层 — 前端唯一的 invoke 调用入口。
 * 其他模块绝不直接 import { invoke } from '@tauri-apps/api/core'；
 * 一律通过本文件暴露的领域函数间接调用。
 */
export { invoke } from '@tauri-apps/api/core';
