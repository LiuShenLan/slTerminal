// ShortcutRegistry.ts — 快捷键注册表单例
//
// 职责：
// - 全局 capture-phase keydown 监听器（唯一，引用计数控制生命周期）
// - 命令注册/注销 + 指纹索引（按 默认键 ⊕ 用户覆盖层 合并后的"有效键"建索引）
// - 上下文栈管理（pushContext/popContext 带竞态防护）
// - 匹配算法（findWinner）：IME 守卫 → KeyStroke 严格匹配 → 上下文过滤 → 优先级排序
// - resolve()：供 xterm attachCustomKeyEventHandler 委托（强制 context，不依赖焦点栈）
// - setOverrides()：注入用户覆盖，校验+静默降级后重建绑定表
//
// 设计模式：命令模式（Command）+ 注册表单例 + 分层配置合并（默认 ⊕ 覆盖）+ 观察者（覆盖变更→重建）

import type {
  Command,
  CommandMeta,
  ShortcutContext,
  ShortcutRegistryAPI,
  KeyStroke,
  KeybindingOverrides,
  ExportedBinding,
} from "./types";
import { formatKeystroke, parseKeystroke } from "./keystroke";
import { isReserved } from "./reserved";

/** 从 KeyboardEvent 提取 KeyStroke */
function eventToKeyStroke(e: KeyboardEvent): KeyStroke {
  return {
    ctrlKey: e.ctrlKey,
    shiftKey: e.shiftKey,
    altKey: e.altKey,
    metaKey: e.metaKey,
    code: e.code,
  };
}

/** "global" 上下文常量 */
const GLOBAL = "global";

class ShortcutRegistry implements ShortcutRegistryAPI {
  /** 按 id 索引所有命令，O(1) 注销 */
  private commands = new Map<string, Command>();

  /** 指纹 → 命令列表索引，加速按键匹配（按有效键构建） */
  private fingerprintIndex = new Map<string, Command[]>();

  /** 活跃上下文栈（末尾 = 最后聚焦的面板） */
  private contextStack: ShortcutContext[] = [];

  /** 用户覆盖层（commandId → keystroke 字符串 | null） */
  private overrides: KeybindingOverrides = {};

  /** 是否已向 window 添加 keydown 监听器 */
  private installed = false;

  /** 已注册命令总数（= commands.size），归零时移除监听器 */
  private refCount = 0;

  // ---- 生命周期 ----

  private ensureListenerInstalled(): void {
    if (!this.installed && this.refCount > 0) {
      this.installed = true;
      window.addEventListener("keydown", this.handleKeyDown, { capture: true });
    }
  }

  private ensureListenerRemoved(): void {
    if (this.installed && this.refCount <= 0) {
      this.installed = false;
      window.removeEventListener("keydown", this.handleKeyDown, { capture: true });
    }
  }

  // ---- 公共 API ----

  /** 注册一组命令，返回注销函数 */
  register(commands: Command[]): () => void {
    const ids: string[] = [];
    for (const cmd of commands) {
      if (!this.commands.has(cmd.id)) {
        this.refCount++;
      }
      this.commands.set(cmd.id, cmd);
      ids.push(cmd.id);
    }
    this.rebuildIndex();
    this.ensureListenerInstalled();

    return () => {
      for (const id of ids) {
        this.unregister(id);
      }
    };
  }

  /** 注销单个命令（按 id），幂等 */
  unregister(id: string): void {
    if (!this.commands.has(id)) return;
    this.commands.delete(id);
    this.refCount--;
    this.rebuildIndex();
    this.ensureListenerRemoved();
  }

  /** 推送上下文到活跃栈（面板 focus 时调用），去重后追加到栈尾 */
  pushContext(context: ShortcutContext): void {
    const idx = this.contextStack.indexOf(context);
    if (idx !== -1) {
      this.contextStack.splice(idx, 1);
    }
    this.contextStack.push(context);
  }

  /**
   * 弹出上下文（面板 blur 时调用）。
   * 仅在栈顶匹配时弹出——防竞态：A blur → B focus 时，A 的 blur 不清 B。
   */
  popContext(context: ShortcutContext): void {
    if (
      this.contextStack.length > 0 &&
      this.contextStack[this.contextStack.length - 1] === context
    ) {
      this.contextStack.pop();
    }
  }

  /** 获取当前栈顶上下文（调试用） */
  getActiveContext(): ShortcutContext | null {
    return this.contextStack.length > 0
      ? this.contextStack[this.contextStack.length - 1]
      : null;
  }

  /** 设置用户覆盖层并重建绑定表 */
  setOverrides(overrides: KeybindingOverrides): void {
    this.overrides = overrides ?? {};
    this.rebuildIndex();
  }

  /**
   * 直接解析并执行键盘事件（供 xterm attachCustomKeyEventHandler 委托）。
   * forceContext 指定时强制在该 context（+global）下匹配，不依赖 contextStack。
   * 返回是否被命令消费；不调用 preventDefault（调用方自行处理）。
   */
  resolve(event: KeyboardEvent, forceContext?: ShortcutContext): boolean {
    const winner = this.findWinner(event, forceContext);
    return winner ? winner.handler(event) : false;
  }

  /** 导出某 context 当前生效的绑定（含 global，排除解绑），供未来 iframe 转发脚本用 */
  exportContextBindings(context: ShortcutContext): ExportedBinding[] {
    const out: ExportedBinding[] = [];
    for (const cmd of this.commands.values()) {
      if (cmd.context !== GLOBAL && cmd.context !== context) continue;
      const eff = this.effectiveKeystroke(cmd);
      if (eff) out.push({ id: cmd.id, keystroke: formatKeystroke(eff) });
    }
    return out;
  }

  /** 列出当前已注册命令的元数据（未来可视化 UI 用） */
  listCommands(): CommandMeta[] {
    return [...this.commands.values()].map((c) => ({
      id: c.id,
      title: c.title,
      category: c.category,
      context: c.context,
      defaultKey: c.defaultKey,
      priority: c.priority,
      allowDuringComposition: c.allowDuringComposition,
    }));
  }

  // ---- 内部方法 ----

  /**
   * 计算命令的有效键（默认 ⊕ 覆盖）。
   * 用户覆盖：null → 解绑（返回 null）；非法/保留键 → console.warn + 回退默认键；合法 → 用之。
   * 无覆盖 → 默认键（默认键可信，不过 isReserved 校验）。
   */
  private effectiveKeystroke(cmd: Command): KeyStroke | null {
    if (Object.prototype.hasOwnProperty.call(this.overrides, cmd.id)) {
      const raw = this.overrides[cmd.id];
      if (raw === null) return null; // 显式解绑
      const ks = parseKeystroke(raw);
      if (!ks) {
        console.warn(`[shortcuts] 非法 keystroke "${raw}"（命令 ${cmd.id}），回退默认键`);
        return cmd.defaultKey;
      }
      if (isReserved(ks, cmd.context)) {
        console.warn(
          `[shortcuts] 保留键 "${raw}"（命令 ${cmd.id}, context ${cmd.context}）不可绑定，回退默认键`,
        );
        return cmd.defaultKey;
      }
      return ks;
    }
    return cmd.defaultKey;
  }

  /** 重建指纹索引（按有效键） */
  private rebuildIndex(): void {
    this.fingerprintIndex.clear();
    for (const cmd of this.commands.values()) {
      const eff = this.effectiveKeystroke(cmd);
      if (!eff) continue; // 解绑的命令不进索引
      const fp = formatKeystroke(eff);
      const list = this.fingerprintIndex.get(fp);
      if (list) {
        if (list.some((c) => c.context === cmd.context)) {
          console.warn(`[shortcuts] 键位冲突：${fp} 在 context "${cmd.context}" 已被占用（新增 ${cmd.id}）`);
        }
        list.push(cmd);
      } else {
        this.fingerprintIndex.set(fp, [cmd]);
      }
    }
  }

  /**
   * 匹配算法（handleKeyDown 与 resolve 共用）。
   * 指纹查候选 → IME 守卫 + 上下文过滤 → priority DESC + 上下文优先排序 → 返回 winner。
   */
  private findWinner(e: KeyboardEvent, forceContext?: ShortcutContext): Command | null {
    const fp = formatKeystroke(eventToKeyStroke(e));
    const candidates = this.fingerprintIndex.get(fp);
    if (!candidates || candidates.length === 0) return null;

    const matching = candidates.filter((cmd) => {
      if (e.isComposing && !cmd.allowDuringComposition) return false;
      if (cmd.context === GLOBAL) return true;
      return forceContext ? cmd.context === forceContext : this.contextStack.includes(cmd.context);
    });
    if (matching.length === 0) return null;

    matching.sort((a, b) => {
      if (a.priority !== b.priority) return b.priority - a.priority;
      if (forceContext) {
        // 强制 context 下：forceContext 命令优先于 global
        const aForced = a.context === forceContext ? 1 : 0;
        const bForced = b.context === forceContext ? 1 : 0;
        return bForced - aForced;
      }
      // 焦点栈内越晚聚焦越优先（global 的 context 不在栈中，lastIndexOf 返回 -1，排最后）
      return this.contextStack.lastIndexOf(b.context) - this.contextStack.lastIndexOf(a.context);
    });

    return matching[0];
  }

  /** 全局 keydown 处理器（箭头函数属性，确保 this 绑定） */
  private handleKeyDown = (e: KeyboardEvent): void => {
    const winner = this.findWinner(e);
    if (winner && winner.handler(e)) {
      e.preventDefault();
      e.stopPropagation();
    }
  };

  // ---- 测试辅助 ----

  /** 返回已注册命令数量 */
  _commandCount(): number {
    return this.commands.size;
  }

  /** 返回当前上下文栈（副本） */
  _contextStack(): ShortcutContext[] {
    return [...this.contextStack];
  }

  /** 清空所有状态（仅测试用） */
  _reset(): void {
    this.commands.clear();
    this.fingerprintIndex.clear();
    this.contextStack = [];
    this.overrides = {};
    this.ensureListenerRemoved();
    this.refCount = 0;
  }
}

/** 模块级单例 */
let instance: ShortcutRegistry | null = null;

/** 获取 ShortcutRegistry 单例 */
export function getShortcutRegistry(): ShortcutRegistry {
  if (!instance) {
    instance = new ShortcutRegistry();
  }
  return instance;
}
