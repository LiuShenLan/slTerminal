// sessionRefreshTask.ts —— sessionRefresh 任务执行体（F12，规格 FR-4）
//
// 扫描执行体 = 历史会话扫描唯一执行路径：遍历 cliProfileRegistry 中声明 history
// 能力的 profile 逐个 scanAgentHistory(cliId, true) 聚合为扁平列表（恒 force=true——
// 后端 (目录 mtime, 文件数) 缓存对进行中会话不敏感，手动与定时同，规格 §8）。
// 多 provider 失败隔离：单 provider 失败 → 该 provider 保留旧数据、其余采用新值；
// 全部失败 → throw（调度器按触发来源走规格 §7）。
//
// runSessionRefresh 显式导出供测试在 _reset() 后重注册（注册触发点仍收敛于 ./tasks.ts）。

import { cliProfileRegistry } from "../cliProfiles/cliProfileRegistry";
import { scanAgentHistory } from "../../ipc/agentHistory";
import { SESSION_REFRESH_TASK_ID } from "../../types/backgroundTasks";
import type { AgentHistorySession } from "../../types/agentHistory";
import type { TriggerSource } from "./types";
import { backgroundTaskScheduler } from "./scheduler";

export async function runSessionRefresh(
  _source: TriggerSource,
  prev: AgentHistorySession[] | undefined,
): Promise<AgentHistorySession[]> {
  const prevSessions = prev ?? [];
  const profiles = cliProfileRegistry
    .getAll()
    .filter((p) => p.capabilities.history !== undefined);
  const results = await Promise.allSettled(
    profiles.map((p) => scanAgentHistory(p.id, true)),
  );
  const merged: AgentHistorySession[] = [];
  let failed = 0;
  results.forEach((r, i) => {
    const cliId = profiles[i].id;
    if (r.status === "fulfilled") {
      merged.push(...(Array.isArray(r.value) ? r.value : []));
    } else {
      failed++;
      console.error(`[slTerminal] 历史扫描失败（${cliId}）:`, r.reason);
      // 失败 provider 保留旧数据（按 cliId 过滤 prev）
      merged.push(...prevSessions.filter((s) => s.cliId === cliId));
    }
  });
  if (failed > 0 && failed === results.length) {
    throw new Error(`全部 history provider 扫描失败（${failed} 个）`);
  }
  return merged;
}

backgroundTaskScheduler.register({
  id: SESSION_REFRESH_TASK_ID,
  run: runSessionRefresh,
});
