// scheduler.ts —— 后台定时任务调度器（F12，注册表家族契约 #13 模块级单例）
//
// 生命周期：首个订阅者出现 → 读配置（background_tasks_list）→ enabled 则立即执行
// 一轮（接管「挂载即扫」语义）+ 启动 interval；最后订阅者退订 → 停 interval
// （无订阅者不空转）。tick 防重入：上一轮未结束跳过本 tick；triggerNow 与 tick
// 互斥同一闸门。失败处理（规格 §7）：tick 失败静默（快照不变）；manual 失败置 error
// 态（保留旧 data）。applyConfig 运行期改配立即生效（设置页直调）。

import { listBackgroundTasks } from "../../ipc/backgroundTasks";
import type { BackgroundTaskDef, TaskSnapshot, TriggerSource } from "./types";

interface TaskRuntime {
  def: BackgroundTaskDef;
  listeners: Set<(snapshot: TaskSnapshot) => void>;
  timer: ReturnType<typeof setInterval> | null;
  /** 防重入闸门（tick 与 manual 共用） */
  running: boolean;
  enabled: boolean;
  intervalSec: number;
  /** 配置是否已成功从后端读取（list 失败 → 首轮仍执行但不启动 interval） */
  configReady: boolean;
  snapshot: TaskSnapshot;
}

class BackgroundTaskScheduler {
  private tasks = new Map<string, TaskRuntime>();

  /** 注册任务（同 id 覆盖旧条目——运行时状态随条目重建清零） */
  register<T>(def: BackgroundTaskDef<T>): void {
    this.tasks.set(def.id, {
      def: def as BackgroundTaskDef,
      listeners: new Set(),
      timer: null,
      running: false,
      enabled: true,
      intervalSec: 0,
      configReady: false,
      snapshot: { state: "idle", data: undefined },
    });
  }

  /** 全部任务定义，按注册序 */
  getAll(): BackgroundTaskDef[] {
    return [...this.tasks.values()].map((t) => t.def);
  }

  /** 清空全部任务（仅测试用——停全部 timer） */
  _reset(): void {
    for (const rt of this.tasks.values()) this.stopTimer(rt);
    this.tasks.clear();
  }

  /** 订阅任务快照：立即回调当前快照；首个订阅者触发激活（读配置 → 立即一轮 + interval） */
  subscribe<T>(id: string, listener: (snapshot: TaskSnapshot<T>) => void): () => void {
    const rt = this.tasks.get(id);
    if (!rt) {
      console.error(`[slTerminal] 后台任务未注册: ${id}`);
      return () => {};
    }
    const l = listener as (snapshot: TaskSnapshot) => void;
    rt.listeners.add(l);
    l(rt.snapshot);
    if (rt.listeners.size === 1) this.activate(rt);
    return () => {
      rt.listeners.delete(l);
      if (rt.listeners.size === 0) this.stopTimer(rt); // 最后退订停 interval（在途轮继续完成）
    };
  }

  /** 手动触发（刷新钮）：与 tick 共用同一执行体与防重入闸门，仅 source 不同 */
  async triggerNow(id: string): Promise<void> {
    const rt = this.tasks.get(id);
    if (!rt) return;
    await this.runOnce(rt, "manual");
  }

  /** 运行期改配（设置页 set_config 成功后直调）：启停/改频率立即生效 */
  applyConfig(id: string, cfg: { enabled: boolean; intervalSec: number }): void {
    const rt = this.tasks.get(id);
    if (!rt) return;
    const wasRunningTimer = rt.timer !== null;
    rt.enabled = cfg.enabled;
    rt.intervalSec = cfg.intervalSec;
    rt.configReady = true;
    if (!cfg.enabled) {
      this.stopTimer(rt);
      return;
    }
    if (rt.listeners.size === 0) return; // 无订阅者不空转（配置已记，订阅时生效）
    if (!wasRunningTimer) void this.runOnce(rt, "tick"); // 禁用→启用：立即一轮
    this.restartTimer(rt);
  }

  /** 本地变更透传（removeLocal 语义：删除会话后本地移除列表项不重扫） */
  applyLocal<T>(id: string, updater: (prev: T | undefined) => T): void {
    const rt = this.tasks.get(id);
    if (!rt) return;
    rt.snapshot = { ...rt.snapshot, data: updater(rt.snapshot.data as T | undefined) };
    this.broadcast(rt);
  }

  /** 首个订阅者激活：配置未读 → 先读配置再启动；已读 → 直接启动（切回立即一轮） */
  private activate(rt: TaskRuntime): void {
    if (rt.configReady) {
      this.startIfEnabled(rt);
      return;
    }
    void (async () => {
      try {
        const list = await listBackgroundTasks();
        const cfg = list.find((t) => t.taskId === rt.def.id);
        if (cfg) {
          rt.enabled = cfg.enabled;
          rt.intervalSec = cfg.intervalSec;
        }
        rt.configReady = true;
      } catch (e) {
        // 配置读取失败：保住首轮执行（数据可见），不启动 interval（无元数据第二来源）
        console.error(`[slTerminal] 后台任务配置读取失败（${rt.def.id}），仅执行首轮不启动定时`, e);
      }
      this.startIfEnabled(rt);
    })();
  }

  private startIfEnabled(rt: TaskRuntime): void {
    if (!rt.enabled) return; // 禁用：不执行首轮不启动定时
    void this.runOnce(rt, "tick");
    if (rt.intervalSec > 0 && rt.timer === null) this.startTimer(rt);
  }

  private startTimer(rt: TaskRuntime): void {
    rt.timer = setInterval(() => void this.runOnce(rt, "tick"), rt.intervalSec * 1000);
  }

  private stopTimer(rt: TaskRuntime): void {
    if (rt.timer !== null) {
      clearInterval(rt.timer);
      rt.timer = null;
    }
  }

  private restartTimer(rt: TaskRuntime): void {
    this.stopTimer(rt);
    if (rt.enabled && rt.intervalSec > 0) this.startTimer(rt);
  }

  /** 执行一轮（防重入：进行中直接返回）。状态机：开始 loading → 成功 ready /
      tick 失败静默（快照不变）/ manual 失败 error（保留旧 data） */
  private async runOnce(rt: TaskRuntime, source: TriggerSource): Promise<void> {
    if (rt.running) return;
    rt.running = true;
    rt.snapshot = { ...rt.snapshot, state: "loading" };
    this.broadcast(rt);
    try {
      const data = await rt.def.run(source, rt.snapshot.data);
      rt.snapshot = { state: "ready", data };
      this.broadcast(rt);
    } catch (e) {
      if (source === "manual") {
        rt.snapshot = { ...rt.snapshot, state: "error" };
        this.broadcast(rt);
      }
      console.error(`[slTerminal] 后台任务执行失败（${rt.def.id}, ${source}）:`, e);
    } finally {
      rt.running = false;
    }
  }

  private broadcast(rt: TaskRuntime): void {
    for (const l of rt.listeners) l(rt.snapshot);
  }
}

/** 模块级单例 */
export const backgroundTaskScheduler = new BackgroundTaskScheduler();
