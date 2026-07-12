import { invoke } from "@tauri-apps/api/core";

export * as pty from "./pty";
export * as fs from "./fs";
export * as git from "./git";
export * as settings from "./settings";
export * as notify from "./notify";
export * as clipboard from "./clipboard";
export * as dialog from "./dialog";
export * as window from "./window";

/** 验证 IPC 链路和测试基建 — 占位命令 */
export async function ping(): Promise<string> {
  return await invoke("ping");
}
