// TerminalRegistry — 终端实例跨页面生命周期管理
//
// 模块级 Map<panelId, {term, sessionId, webglAddon, fitAddon}>
// 页面切换时 Terminal 实例存活（不 dispose），切回时复用（term.open(el)）。
// Phase 2 务实方案：切回时重新 spawn PTY（Channel 重连留 Phase 3）。

import { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";
import type { AgentStatus } from "../../lib/agentStatus";
import type { ShellKind } from "../../types/pty";

/** 会话信息——存在即运行中（二态模型，无 running 布尔） */
export interface AgentSessionInfo {
  /** 会话 UUID（hook 事件 payload.sessionId；matchedCommand-only 会话无此字段） */
  sessionId?: string;
  usageSourcePath?: string;
  matchedCommand?: string;
  /** 四态（profile.hooks.eventToStatus 结果；null 状态不存储——undefined 保留旧值） */
  status?: AgentStatus;
  /**
   * cliId（OSC 133 C 命中时写入，MC-107）。可选——hook 事件路径不经 setAgentSession
   * 写 cliId，消费方三级解析缺省回退 CLAUDE_CLI_ID（MC-205）。
   */
  cliId?: string;
  lastEventAt: number;
}

export interface RegisteredTerminal {
  term: Terminal;
  sessionId: string;
  webglAddon: WebglAddon | null;
  fitAddon: FitAddon;
  /** 实际解析的 shell 种类（pty.spawn 返回值）——恢复注入就绪闸门分派等待策略 */
  shellKind: ShellKind;
  /** 首个提示符已渲染（OSC 133;A 到达）——恢复注入就绪闸门信号；
   *  cmd 无 shell integration 恒 false（闸门对该种类走固定延迟，不读本字段） */
  promptReady: boolean;
  /** 最近一次 PTY 输出到达时间戳（Date.now()）——就绪闸门的输出沉淀判据：
   *  133;A 是「渲染开始」而非「渲染完成」，渲染期控制台输入模式切换窗口会
   *  吞首字节（Win10 捆绑 conhost 实测）；闸门要求输出静默 ≥100ms 才注入 */
  lastOutputAt: number;
  /** 会话状态：存在即运行中，null = 明确无会话，undefined = 未设置（缺省保留旧值） */
  agentSession?: AgentSessionInfo | null;
}

/** 注册表变更事件（sessionChange 仅携 panelId——listener 经 get() 读现值，防快照不一致） */
export type RegistryEvent = { type: "register" | "remove" | "sessionChange"; panelId: string };

const registry = new Map<string, RegisteredTerminal>();
const listeners = new Set<(e: RegistryEvent) => void>();

function notify(event: RegistryEvent): void {
  for (const fn of listeners) {
    fn(event);
  }
}

export const TerminalRegistry = {
  register(panelId: string, entry: RegisteredTerminal): void {
    // 幂等覆盖：agentSession 缺省时保留旧值（StrictMode/重试场景不丢 session）
    const old = registry.get(panelId);
    if (old && entry.agentSession === undefined) {
      entry = { ...entry, agentSession: old.agentSession };
    }
    registry.set(panelId, entry);
    notify({ type: "register", panelId });
  },

  get(panelId: string): RegisteredTerminal | undefined {
    return registry.get(panelId);
  },

  remove(panelId: string): boolean {
    const existed = registry.delete(panelId);
    if (existed) {
      notify({ type: "remove", panelId });
    }
    return existed;
  },

  has(panelId: string): boolean {
    return registry.has(panelId);
  },

  /** 返回所有已注册终端的 panelId → RegisteredTerminal 映射（只读副本，防止外部修改内部 Map） */
  getAll(): ReadonlyMap<string, RegisteredTerminal> {
    return new Map(registry);
  },

  /** 设置面板的 agentSession：patch 中 undefined 键不覆盖旧值（merge），
   *  null 清空为 null，panelId 不存在 no-op 不 notify，
   *  缺 lastEventAt 自动填 Date.now()。
   *  成功后 notify({ type: "sessionChange", panelId })。 */
  setAgentSession(panelId: string, patch: Partial<AgentSessionInfo> | null): void {
    const entry = registry.get(panelId);
    if (!entry) return; // no-op，不 notify

    if (patch === null) {
      entry.agentSession = null;
    } else {
      const prev = entry.agentSession;
      entry.agentSession = {
        sessionId: patch.sessionId !== undefined ? patch.sessionId : prev?.sessionId,
        usageSourcePath: patch.usageSourcePath !== undefined ? patch.usageSourcePath : prev?.usageSourcePath,
        matchedCommand: patch.matchedCommand !== undefined ? patch.matchedCommand : prev?.matchedCommand,
        status: patch.status !== undefined ? patch.status : prev?.status,
        cliId: patch.cliId !== undefined ? patch.cliId : prev?.cliId,
        lastEventAt: patch.lastEventAt ?? Date.now(),
      };
    }

    notify({ type: "sessionChange", panelId });
  },

  /** 标记首个提示符已渲染（OSC 133;A）——恢复注入就绪闸门信号源。
   *  幂等（重复 A 不重复 notify——闸门只消费首次置位，经 get() 轮询读取）；
   *  panelId 不存在时 no-op（终端已卸载的迟到 A 不建条目） */
  markPromptReady(panelId: string): void {
    const entry = registry.get(panelId);
    if (!entry || entry.promptReady) return;
    entry.promptReady = true;
  },

  /** PTY 输出到达打点（usePtyOutput 每个 output 事件调用）——
   *  恢复注入闸门的沉淀判据数据源；无 notify（闸门经 get() 轮询读取） */
  noteOutput(panelId: string): void {
    const entry = registry.get(panelId);
    if (!entry) return;
    entry.lastOutputAt = Date.now();
  },

  /** 订阅注册表变更：register/remove/sessionChange 后同步通知。返回退订函数 */
  subscribe(listener: (e: RegistryEvent) => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },

  /** 仅用于调试/测试 */
  _size(): number {
    return registry.size;
  },

  /** 仅用于调试/测试 */
  _dump(): string[] {
    return Array.from(registry.keys());
  },

  /** 仅用于调试/测试 */
  _reset(): void {
    registry.clear();
    listeners.clear();
  },
};
