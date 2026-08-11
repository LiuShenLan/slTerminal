// Agent hooks IPC — 注入/卸载/状态查询 + agent-event 事件订阅 + token 用量查询（MC-212 泛化）
//
// wrapper 全部加 cliId 首参（6 命令全表：agent_hooks_inject / agent_hooks_uninstall /
// agent_hooks_injection_status / agent_context_usage / agent_hooks_config_read /
// agent_hooks_config_write——后两条在 hooksConfig.ts）。未知 cliId → 后端 Validation。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ContextUsage,
  AgentEventPayload,
  AgentHookInjectionStatus,
} from "../types/agent";
// 类型再导出：原 ipc/hooks.ts 内嵌定义 HookInjectionStatus，消费方（E2E helpers 等）经
// wrapper 模块引类型的旧导入面保留（规范定义位置 = types/agent，DTO 双边对应硬约束 #4）
export type { AgentHookInjectionStatus } from "../types/agent";

/** 注入 hook 脚本到 claude settings.json，返回注入后状态 */
export async function inject(cliId: string): Promise<AgentHookInjectionStatus> {
  return invoke("agent_hooks_inject", { cliId });
}

/** 卸载 hook：移除配置段 + 删脚本目录 + 清信号目录 */
export async function uninstall(cliId: string): Promise<void> {
  return invoke("agent_hooks_uninstall", { cliId });
}

/** 查询当前注入状态（面板/入口显示用） */
export async function getInjectionStatus(
  cliId: string,
): Promise<AgentHookInjectionStatus> {
  return invoke("agent_hooks_injection_status", { cliId });
}

/**
 * 从 transcript 文件尾部扫描 token 用量
 *
 * 读取 transcript JSONL 文件尾部约 64KB，逆行扫描含 usage 的行，
 * 返回 input/output token 数。无 usage 或文件异常返回 null。
 */
export async function contextUsage(
  cliId: string,
  usageSourcePath: string,
): Promise<ContextUsage | null> {
  return invoke("agent_context_usage", { cliId, usageSourcePath });
}

/**
 * 订阅后端 agent-event 事件（照 onFsEvent 模式，MC-202）
 *
 * 返回取消监听的清理函数。
 */
export function onAgentEvent(
  callback: (payload: AgentEventPayload) => void,
): () => void {
  const unlisten = listen<AgentEventPayload>("agent-event", (event) =>
    callback(event.payload),
  );
  return () => {
    unlisten.then((fn) => fn());
  };
}
