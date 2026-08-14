// Agent hooks IPC — 注入/卸载/状态查询 + agent-event 事件订阅 + 关闭 statusline 恢复（MC-212 泛化）
//
// wrapper 全部加 cliId 首参（6 命令全表：agent_hooks_inject / agent_hooks_uninstall /
// agent_hooks_injection_status / agent_hooks_restore_statusline / agent_hooks_config_read /
// agent_hooks_config_write——后两条在 hooksConfig.ts）。未知 cliId → 后端 Validation。
// 原 agent_context_usage（transcript token 扫描）已整体移除——百分比经 ContextUsage
// 信号（statusline 桥接）走 agent-event 通道推送。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
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
 * 关闭清理：恢复 statusline 桥接（还原备份原配置，备份保留供重开重注入）
 *
 * 客户端关闭序列调用——用户在别处用 cli 时终端状态行不受桥接影响。
 * 非桥接/无备份 → 后端 no-op。
 */
export async function restoreStatusline(cliId: string): Promise<void> {
  return invoke("agent_hooks_restore_statusline", { cliId });
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
