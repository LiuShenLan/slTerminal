// 项目数据持久化 IPC 封装 — 前端调用 projects 命令的唯一入口
// invoke 只允许在本文件出现（硬约束 #1）

import { invoke } from "@tauri-apps/api/core";

/** 加载持久化项目数据（exe 同级 slterminal-projects.json → JSON 字符串） */
export async function loadProjects(): Promise<string> {
  return invoke<string>("load_projects");
}

/** 保存项目数据到磁盘（JSON 字符串 → exe 同级 slterminal-projects.json） */
export async function saveProjects(data: string): Promise<void> {
  await invoke("save_projects", { data });
}
