import { invoke } from "@tauri-apps/api/core";

export * as pty from "./pty";
export * as fs from "./fs";
export * as git from "./git";
export * as settings from "./settings";
export * as projects from "./projects";
export * as notify from "./notify";
export * as clipboard from "./clipboard";
export * as dialog from "./dialog";
export * as shell from "./shell";
export * as window from "./window";
export * as agentHooks from "./agentHooks";
export * as hooksConfig from "./hooksConfig";
export * as agentHistory from "./agentHistory";
export * as notification from "./notification";
export * as planBalance from "./planBalance";
export * as backgroundTasks from "./backgroundTasks";

/** 测试专用——验证 IPC 链路（仅 src/__tests__/ipc-ping.test.ts 与 ipc-contract.test.ts 消费，生产零消费，FE-35 保留） */
export async function ping(): Promise<string> {
  return await invoke("ping");
}
