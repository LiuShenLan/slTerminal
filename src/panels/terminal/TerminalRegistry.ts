// TerminalRegistry — 终端实例跨页面生命周期管理
//
// 模块级 Map<panelId, {term, sessionId, webglAddon, fitAddon}>
// 页面切换时 Terminal 实例存活（不 dispose），切回时复用（term.open(el)）。
// Phase 2 务实方案：切回时重新 spawn PTY（Channel 重连留 Phase 3）。

import { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import type { WebglAddon } from "@xterm/addon-webgl";

export interface RegisteredTerminal {
  term: Terminal;
  sessionId: string;
  webglAddon: WebglAddon | null;
  fitAddon: FitAddon;
}

const registry = new Map<string, RegisteredTerminal>();

export const TerminalRegistry = {
  register(panelId: string, entry: RegisteredTerminal): void {
    // 幂等：覆盖旧条目（防御重复 mount）
    registry.set(panelId, entry);
  },

  get(panelId: string): RegisteredTerminal | undefined {
    return registry.get(panelId);
  },

  remove(panelId: string): boolean {
    return registry.delete(panelId);
  },

  has(panelId: string): boolean {
    return registry.has(panelId);
  },

  /** 返回所有已注册终端的 panelId → RegisteredTerminal 映射（只读） */
  getAll(): ReadonlyMap<string, RegisteredTerminal> {
    return registry;
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
  _clear(): void {
    registry.clear();
  },
};
