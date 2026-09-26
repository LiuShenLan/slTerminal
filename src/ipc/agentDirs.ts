// Agent 全局配置目录 IPC — 后端 agent_dirs.rs 静态表解析（ADR-0024）
// 沙箱边界：前端不自行拼接 home 路径（无 OS 访问），绝对路径一律经本命令取回
import { invoke } from "@tauri-apps/api/core";
import type { AgentGlobalDir } from "../types/agentDirs";

/** 列出全部 agent 全局配置目录（后端静态表 + home 解析，含不存在项——exists=false 供占位展示） */
export function listAgentDirs(): Promise<AgentGlobalDir[]> {
  return invoke("agent_dirs_list");
}
