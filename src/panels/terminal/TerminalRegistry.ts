// TerminalRegistry — 终端实例跨页面生命周期管理
//
// 模块级 Map<panelId, {term, sessionId, webglAddon, fitAddon}>
// 页面切换时 Terminal 实例存活（不 dispose），切回时复用（term.open(el)）。
// Phase 2 务实方案：切回时重新 spawn PTY（Channel 重连留 Phase 3）。

import { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";
import type { ClaudeStatus } from "../../lib/claudeStatus";

/** claude 会话信息——存在即运行中（二态模型，无 running 布尔） */
export interface ClaudeSessionInfo {
  /** 会话 UUID（hook 事件 payload.sessionId；matchedCommand-only 会话无此字段） */
  sessionId?: string;
  transcriptPath?: string;
  matchedCommand?: string;
  /** 四态（eventToStatus 结果；null 状态不存储——undefined 保留旧值） */
  status?: ClaudeStatus;
  lastEventAt: number;
}

export interface RegisteredTerminal {
  term: Terminal;
  sessionId: string;
  webglAddon: WebglAddon | null;
  fitAddon: FitAddon;
  /** claude 会话状态：存在即运行中，null = 明确无会话，undefined = 未设置（缺省保留旧值） */
  claudeSession?: ClaudeSessionInfo | null;
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
    // 幂等覆盖：claudeSession 缺省时保留旧值（StrictMode/重试场景不丢 session）
    const old = registry.get(panelId);
    if (old && entry.claudeSession === undefined) {
      entry = { ...entry, claudeSession: old.claudeSession };
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

  /** 设置面板的 claudeSession：patch 中 undefined 键不覆盖旧值（merge），
   *  null 清空为 null，panelId 不存在 no-op 不 notify，
   *  缺 lastEventAt 自动填 Date.now()。
   *  成功后 notify({ type: "sessionChange", panelId })。 */
  setClaudeSession(panelId: string, patch: Partial<ClaudeSessionInfo> | null): void {
    const entry = registry.get(panelId);
    if (!entry) return; // no-op，不 notify

    if (patch === null) {
      entry.claudeSession = null;
    } else {
      const prev = entry.claudeSession;
      entry.claudeSession = {
        sessionId: patch.sessionId !== undefined ? patch.sessionId : prev?.sessionId,
        transcriptPath: patch.transcriptPath !== undefined ? patch.transcriptPath : prev?.transcriptPath,
        matchedCommand: patch.matchedCommand !== undefined ? patch.matchedCommand : prev?.matchedCommand,
        status: patch.status !== undefined ? patch.status : prev?.status,
        lastEventAt: patch.lastEventAt ?? Date.now(),
      };
    }

    notify({ type: "sessionChange", panelId });
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
